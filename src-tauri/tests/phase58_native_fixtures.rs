//! Writes the fixtures used by the native Phase 58 preview-vs-export qualification.
//!
//! `cargo test --test phase58_native_fixtures -- --ignored` creates, under
//! `artifacts/phase58/native/` (untracked), 1920x1080 media, a graded `.cube` LUT and four
//! projects: `lut.tcproj` / `lut_base.tcproj` and `key.tcproj` / `key_base.tcproj`. The
//! `*_base` projects are identical without the effect, so the native run can separate the
//! effect's preview/export difference from the known resampling gap (G52).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AssetDimensions, ChromaKey, ClipLut, Despill, DespillScreen, ItemKind, MediaKind,
    Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::media::lut::Lut3D;
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};
use tinixcut_lib::modules::persistence::save_project_atomic;

fn ffmpeg() -> Command {
    Command::new(
        MediaBinaryResolver::resolve(BinaryKind::FFmpeg)
            .expect("bundled FFmpeg")
            .path,
    )
}

fn out_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts/phase58/native");
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// Encodes BT.709 limited-range video with every colour tag set. Untagged files make the
/// browser and FFmpeg pick different matrices, which would swamp the effect being measured.
fn make_video(path: &Path, source: &str, extra_filter: Option<&str>) {
    let tag = "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p";
    let filter = match extra_filter {
        Some(extra) => format!("{extra},{tag}"),
        None => tag.to_string(),
    };
    let status = ffmpeg()
        .args([
            "-v", "error", "-y", "-f", "lavfi", "-i", source, "-vf", &filter,
        ])
        .args(["-c:v", "libx264", "-crf", "12"])
        .args([
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-color_range",
            "tv",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "failed to create {}", path.display());
}

/// Natural footage (the Hoi An sample, if present) cropped to 1920x1080: saturated synthetic
/// patterns exaggerate the browser/FFmpeg decode difference, camera footage does not.
fn make_natural(path: &Path, fallback: &str) {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/media-samples/hoi-an-morning/hoi-an-one-day.mp4");
    if !sample.is_file() {
        return make_video(path, fallback, None);
    }
    let status = ffmpeg()
        .args(["-v", "error", "-y", "-ss", "2", "-t", "3", "-i"])
        .arg(&sample)
        .args([
            "-an",
            "-vf",
            "scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080,fps=30",
            "-c:v",
            "libx264",
            "-crf",
            "12",
            "-pix_fmt",
            "yuv420p",
            "-colorspace",
            "bt709",
            "-color_range",
            "tv",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "failed to create {}", path.display());
}

fn graded_cube(path: &Path) {
    let mut lut = Lut3D::identity(17);
    lut.title = "Phase58 graded".into();
    for s in &mut lut.samples {
        let [r, g, b] = *s;
        *s = [
            (0.75 * r.powf(0.8) + 0.25 * g).clamp(0.0, 1.0),
            (0.9 * g.powf(1.2) + 0.1 * b).clamp(0.0, 1.0),
            (0.8 * b + 0.2 * r * g).clamp(0.0, 1.0),
        ];
    }
    let mut text = format!("TITLE \"{}\"\nLUT_3D_SIZE {}\n", lut.title, lut.size);
    for [r, g, b] in &lut.samples {
        text.push_str(&format!("{r:.8} {g:.8} {b:.8}\n"));
    }
    fs::write(path, text).unwrap();
}

fn project(name: &str, clips: &[(&str, &Path, Option<ClipLut>, Option<ChromaKey>)]) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new(name.to_string());
    project.settings.width = 1920;
    project.settings.height = 1080;
    project.settings.frame_rate = rate;
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(90, rate));
    for (index, (clip_name, path, lut, key)) in clips.iter().enumerate() {
        let asset_id = format!("ast_{index}");
        project.assets.push(Asset {
            asset_id: asset_id.clone(),
            name: format!("{clip_name}.mp4"),
            path: path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::new(90, rate)),
            file_size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            dimensions: Some(AssetDimensions {
                width: 1920,
                height: 1080,
            }),
        });
        let mut track = Track::new(format!("V{}", index + 1), TrackKind::Video);
        let mut item = TimelineItem::new(
            Some(asset_id),
            (*clip_name).into(),
            ItemKind::Video,
            range,
            range,
        )
        .unwrap();
        item.color_lut = lut.clone();
        item.chroma_key = key.clone();
        track.items.push(item);
        project.tracks.push(track);
    }
    project
}

#[test]
#[ignore = "writes artifacts/phase58/native/*"]
fn write_native_fixtures() {
    let dir = out_dir();
    let base = dir.join("testsrc.mp4");
    let screen = dir.join("greenscreen.mp4");
    let cube = dir.join("graded.cube");
    make_natural(&base, "testsrc2=s=1920x1080:r=30:d=3");
    make_video(
        &screen,
        "color=c=0x00ff00:s=1920x1080:r=30:d=3",
        Some("drawbox=x=660:y=315:w=600:h=450:color=red:t=fill"),
    );
    graded_cube(&cube);
    let clip_lut = ClipLut::from_file(&cube).unwrap();
    let key = ChromaKey {
        key_color: "#00ff00".into(),
        similarity: 0.3,
        blend: 0.1,
        despill: Some(Despill {
            screen: DespillScreen::Green,
            mix: 0.5,
        }),
    };

    let fixtures: Vec<(&str, Project)> = vec![
        (
            "lut",
            project("p58 lut", &[("Test", &base, Some(clip_lut), None)]),
        ),
        (
            "lut_base",
            project("p58 lut base", &[("Test", &base, None, None)]),
        ),
        (
            "key",
            project(
                "p58 key",
                &[
                    ("Under", &base, None, None),
                    ("Screen", &screen, None, Some(key)),
                ],
            ),
        ),
        (
            "key_base",
            project(
                "p58 key base",
                &[
                    ("Under", &base, None, None),
                    ("Screen", &screen, None, None),
                ],
            ),
        ),
    ];
    for (name, project) in fixtures {
        save_project_atomic(&project, &dir.join(format!("{name}.tcproj"))).unwrap();
    }
}

/// Identity-LUT control: any preview difference against the baseline is the effect layer
/// itself (video upload, scaling), not LUT math.
#[test]
#[ignore = "writes artifacts/phase58/native/lut_identity.tcproj"]
fn write_identity_lut_fixture() {
    let dir = out_dir();
    let base = dir.join("testsrc.mp4");
    let cube = dir.join("identity.cube");
    let lut = Lut3D::identity(17);
    let mut text = format!("TITLE \"Identity\"\nLUT_3D_SIZE {}\n", lut.size);
    for [r, g, b] in &lut.samples {
        text.push_str(&format!("{r:.8} {g:.8} {b:.8}\n"));
    }
    fs::write(&cube, text).unwrap();
    let clip_lut = ClipLut::from_file(&cube).unwrap();
    let fixture = project("p58 lut identity", &[("Test", &base, Some(clip_lut), None)]);
    save_project_atomic(&fixture, &dir.join("lut_identity.tcproj")).unwrap();
}

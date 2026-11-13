//! Phase 58C acceptance: per-clip chroma key export semantics.
//!
//! 1. Filter level: FFmpeg `colorkey` + `despill` agree with a reference implementation of the
//!    published formulas (libavfilter/vf_colorkey.c, vf_despill.c), which the preview shader
//!    mirrors (`src/components/lutPreview.ts`, checked against the same measured vectors).
//! 2. Export level: a keyed clip lets the track below show through; unkeyed it does not.
//! 3. Negative controls: a different key color must fail the comparison.
//!
//! `p58c_write_golden_vectors` (ignored) regenerates
//! `src/components/__fixtures__/chromaKeyGolden.json` from real FFmpeg output.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ChromaKey, Despill, DespillScreen, ItemKind, MediaKind, Project, TimelineItem, Track,
    TrackKind,
};
use tinixcut_lib::modules::export::{ExportEngine, ExportProfile};
use tinixcut_lib::modules::media::resolver::{BinaryKind, MediaBinaryResolver};

fn ffmpeg() -> Command {
    let resolved = MediaBinaryResolver::resolve(BinaryKind::FFmpeg).expect("bundled FFmpeg");
    Command::new(resolved.path)
}

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p58c_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn filter_for(key: &ChromaKey) -> String {
    let [r, g, b] = key.rgb().unwrap();
    let mut filter = format!(
        "format=rgba,colorkey=color=0x{r:02x}{g:02x}{b:02x}:similarity={:.5}:blend={:.5}",
        key.similarity, key.blend
    );
    if let Some(despill) = &key.despill {
        let (screen, scales) = match despill.screen {
            DespillScreen::Green => ("green", "red=0:green=-1:blue=0"),
            DespillScreen::Blue => ("blue", "red=0:green=0:blue=-1"),
        };
        filter.push_str(&format!(
            ",despill=type={screen}:mix={:.5}:expand=0:{scales}",
            despill.mix
        ));
    }
    filter
}

/// Reference of FFmpeg colorkey + despill on one 8-bit RGB pixel. Returns (rgb8, alpha8) with
/// the truncation FFmpeg applies when storing.
fn reference(rgb: [u8; 3], key: &ChromaKey) -> ([u8; 3], u8) {
    let [kr, kg, kb] = key.rgb().unwrap();
    let d = [
        rgb[0] as f64 - kr as f64,
        rgb[1] as f64 - kg as f64,
        rgb[2] as f64 - kb as f64,
    ];
    let diff = ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) / (255.0 * 255.0 * 3.0)).sqrt();
    let alpha = if (1.0 / key.blend as f64) < 10000.0 {
        ((diff - key.similarity as f64) / key.blend as f64).clamp(0.0, 1.0) * 255.0
    } else if diff > key.similarity as f64 {
        255.0
    } else {
        0.0
    };
    let (mut r, mut g, mut b) = (
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    );
    if let Some(despill) = &key.despill {
        let mix = despill.mix;
        let factor = 1.0 - mix;
        match despill.screen {
            DespillScreen::Green => {
                let spill = (g - (r * mix + b * factor)).max(0.0);
                g = (g - spill).max(0.0);
            }
            DespillScreen::Blue => {
                let spill = (b - (r * mix + g * factor)).max(0.0);
                b = (b - spill).max(0.0);
            }
        }
    }
    let to8 = |v: f32| (v * 255.0).clamp(0.0, 255.0) as u8;
    ([to8(r), to8(g), to8(b)], alpha as u8)
}

const W: usize = 64;
const H: usize = 64;

/// RGB24 pattern that sweeps hues and brightness, including near-green and near-blue pixels.
fn pattern() -> Vec<u8> {
    let mut data = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            data.push((x * 255 / (W - 1)) as u8);
            data.push(((x * 3 + y * 5) % 256) as u8);
            data.push((y * 255 / (H - 1)) as u8);
        }
    }
    data
}

fn run_filter(dir: &Path, filter: &str) -> Vec<u8> {
    let input = dir.join("pattern.rgb");
    fs::write(&input, pattern()).unwrap();
    let output = ffmpeg()
        .args([
            "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", "64x64", "-i",
        ])
        .arg(&input)
        .args(["-vf", filter, "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "ffmpeg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.len(), W * H * 4);
    output.stdout
}

/// (max RGB error, max alpha error) in 8-bit levels of FFmpeg output vs the reference.
fn compare(key: &ChromaKey, rendered: &[u8]) -> (i32, i32) {
    let (mut max_rgb, mut max_alpha) = (0, 0);
    for (px, out) in pattern().chunks(3).zip(rendered.chunks(4)) {
        let (rgb, alpha) = reference([px[0], px[1], px[2]], key);
        for c in 0..3 {
            max_rgb = max_rgb.max((rgb[c] as i32 - out[c] as i32).abs());
        }
        max_alpha = max_alpha.max((alpha as i32 - out[3] as i32).abs());
    }
    (max_rgb, max_alpha)
}

fn green_key(blend: f32, despill: Option<Despill>) -> ChromaKey {
    ChromaKey {
        key_color: "#00ff00".into(),
        similarity: 0.3,
        blend,
        despill,
    }
}

#[test]
fn p58c_ffmpeg_colorkey_and_despill_match_reference_formulas() {
    let dir = test_dir("filter");
    let cases = [
        green_key(0.0, None),
        green_key(0.2, None),
        green_key(
            0.15,
            Some(Despill {
                screen: DespillScreen::Green,
                mix: 0.5,
            }),
        ),
        ChromaKey {
            key_color: "#0000ff".into(),
            similarity: 0.25,
            blend: 0.1,
            despill: Some(Despill {
                screen: DespillScreen::Blue,
                mix: 0.3,
            }),
        },
    ];
    for key in &cases {
        let (rgb, alpha) = compare(key, &run_filter(&dir, &filter_for(key)));
        assert!(rgb <= 1, "{key:?}: RGB error {rgb} levels");
        assert!(alpha <= 1, "{key:?}: alpha error {alpha} levels");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p58c_negative_control_wrong_key_fails_the_comparison() {
    let dir = test_dir("negative");
    let rendered_key = green_key(0.2, None);
    let rendered = run_filter(&dir, &filter_for(&rendered_key));
    let wrong = ChromaKey {
        key_color: "#ff0000".into(),
        ..rendered_key
    };
    let (_, alpha) = compare(&wrong, &rendered);
    assert!(alpha > 40, "gate is insensitive to the key color: {alpha}");
    fs::remove_dir_all(dir).unwrap();
}

fn color_video(path: &Path, color: &str) {
    let status = ffmpeg()
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("color=c={color}:s=64x64:r=30:d=1"))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn two_track_project(top_video: &Path, bottom_video: &Path, key: Option<ChromaKey>) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Chroma export".to_string());
    project.settings.width = 64;
    project.settings.height = 64;
    project.settings.frame_rate = rate;
    for (id, path) in [("ast_bottom", bottom_video), ("ast_top", top_video)] {
        project.assets.push(Asset {
            asset_id: id.into(),
            name: id.into(),
            path: path.to_string_lossy().into_owned(),
            media_kind: MediaKind::Video,
            duration: None,
            file_size: 1000,
            dimensions: None,
        });
    }
    let range = TimeRange::new(RationalTime::new(0, rate), RationalTime::new(30, rate));
    for (index, (asset, item_id)) in [("ast_bottom", "clip_bottom"), ("ast_top", "clip_top")]
        .into_iter()
        .enumerate()
    {
        let mut track = Track::new(format!("V{}", index + 1), TrackKind::Video);
        let mut item = TimelineItem::new(
            Some(asset.into()),
            item_id.into(),
            ItemKind::Video,
            range,
            range,
        )
        .unwrap();
        if item_id == "clip_top" {
            item.chroma_key = key.clone();
        }
        track.items.push(item);
        project.tracks.push(track);
    }
    project
}

fn center_pixel(project: &Project, out: &Path) -> [u8; 3] {
    let profile = ExportProfile {
        width: 64,
        height: 64,
        frame_rate: FrameRate::FPS_30,
        ..Default::default()
    };
    ExportEngine::render_snapshot(project, out, &profile, None).expect("export succeeds");
    let output = ffmpeg()
        .args(["-v", "error", "-i"])
        .arg(out)
        .args([
            "-vf",
            "format=rgb24,crop=1:1:32:32",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(
        output.stdout.len(),
        3,
        "one RGB pixel expected: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    [output.stdout[0], output.stdout[1], output.stdout[2]]
}

#[test]
fn p58c_export_keys_the_top_clip_and_shows_the_track_below() {
    let dir = test_dir("export");
    let green = dir.join("green.mp4");
    let red = dir.join("red.mp4");
    color_video(&green, "0x00ff00");
    color_video(&red, "red");

    // Unkeyed: the green clip covers the red one.
    let unkeyed = center_pixel(&two_track_project(&green, &red, None), &dir.join("a.mp4"));
    assert!(
        unkeyed[1] > 200 && unkeyed[0] < 60,
        "green expected, got {unkeyed:?}"
    );

    // Keyed on green: red shows through.
    let keyed = center_pixel(
        &two_track_project(&green, &red, Some(green_key(0.1, None))),
        &dir.join("b.mp4"),
    );
    assert!(
        keyed[0] > 200 && keyed[1] < 60,
        "red expected, got {keyed:?}"
    );

    // Negative control: keying a color the clip does not contain keeps the clip.
    let wrong_key = ChromaKey {
        key_color: "#0000ff".into(),
        similarity: 0.1,
        blend: 0.0,
        despill: None,
    };
    let unaffected = center_pixel(
        &two_track_project(&green, &red, Some(wrong_key)),
        &dir.join("c.mp4"),
    );
    assert!(unaffected[1] > 200, "green must stay, got {unaffected:?}");
    fs::remove_dir_all(dir).unwrap();
}

/// Regenerates the vectors the TypeScript preview reference is checked against. Each vector is
/// real FFmpeg output for a sampled input pixel, so the fixture ties the shader math to FFmpeg
/// and not to this file's reference.
#[test]
#[ignore = "writes src/components/__fixtures__/chromaKeyGolden.json"]
fn p58c_write_golden_vectors() {
    let dir = test_dir("golden");
    let keys = [
        green_key(0.0, None),
        green_key(0.2, None),
        green_key(
            0.15,
            Some(Despill {
                screen: DespillScreen::Green,
                mix: 0.5,
            }),
        ),
        ChromaKey {
            key_color: "#0000ff".into(),
            similarity: 0.25,
            blend: 0.1,
            despill: Some(Despill {
                screen: DespillScreen::Blue,
                mix: 0.3,
            }),
        },
    ];
    let mut entries = Vec::new();
    for key in &keys {
        let rendered = run_filter(&dir, &filter_for(key));
        let mut samples = Vec::new();
        // Every 37th pixel keeps the file small while covering the whole sweep.
        for (index, (px, out)) in pattern().chunks(3).zip(rendered.chunks(4)).enumerate() {
            if index % 37 == 0 {
                samples.push(serde_json::json!({
                    "input": [px[0], px[1], px[2]],
                    "output": [out[0], out[1], out[2]],
                    "alpha": out[3],
                }));
            }
        }
        entries.push(serde_json::json!({
            "key": {
                "key_color": key.key_color,
                "similarity": key.similarity,
                "blend": key.blend,
                "despill": key.despill.as_ref().map(|d| serde_json::json!({
                    "screen": match d.screen { DespillScreen::Green => "Green", DespillScreen::Blue => "Blue" },
                    "mix": d.mix,
                })),
            },
            "samples": samples,
        }));
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/components/__fixtures__");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("chromaKeyGolden.json"),
        serde_json::to_string_pretty(&entries).unwrap(),
    )
    .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

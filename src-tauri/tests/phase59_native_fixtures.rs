//! Writes fixtures for the native Phase 59 UI check under `artifacts/phase59/native/` (untracked).
//!
//! `cargo test --test phase59_native_fixtures -- --ignored` creates `mix.tcproj` (gradient video,
//! music and a voice-like tone on separate tracks) and `trans.tcproj` (red clip, blue clip and
//! a WipeLeft cut transition), both 1920x1080 at 30 fps.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AssetDimensions, CutTransition, ItemKind, MediaKind, Project, TimelineItem, Track,
    TrackKind, TransitionKind,
};
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts/phase59/native");
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn run(command: &mut Command, what: &str) {
    let status = command.status().unwrap();
    assert!(status.success(), "{what} failed");
}

fn solid(path: &Path, color: &str) {
    run(
        ffmpeg()
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("color=c={color}:s=1920x1080:r=30:d=6"))
            .args(["-c:v", "libx264", "-crf", "16", "-pix_fmt", "yuv420p"])
            .arg(path),
        "solid video",
    );
}

fn tone(path: &Path, frequency: u32, seconds: u32, gain_db: f64) {
    run(
        ffmpeg()
            .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!(
                "sine=frequency={frequency}:duration={seconds}:sample_rate=48000"
            ))
            .args(["-af"])
            .arg(format!("volume={gain_db}dB"))
            .args(["-c:a", "pcm_s16le"])
            .arg(path),
        "tone",
    );
}

fn asset(id: &str, path: &Path, kind: MediaKind, frames: i64, video: bool) -> Asset {
    Asset {
        asset_id: id.into(),
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        media_kind: kind,
        duration: Some(RationalTime::new(frames, FrameRate::FPS_30)),
        file_size: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        dimensions: video.then_some(AssetDimensions {
            width: 1920,
            height: 1080,
        }),
    }
}

fn item(
    asset_id: &str,
    name: &str,
    kind: ItemKind,
    start: i64,
    frames: i64,
    source_start: i64,
) -> TimelineItem {
    let rate = FrameRate::FPS_30;
    let timeline = TimeRange::new(
        RationalTime::new(start, rate),
        RationalTime::new(frames, rate),
    );
    let mut item =
        TimelineItem::new(Some(asset_id.into()), name.into(), kind, timeline, timeline).unwrap();
    item.source_range = TimeRange::new(
        RationalTime::new(source_start, rate),
        RationalTime::new(frames, rate),
    );
    item
}

fn base_project(name: &str) -> Project {
    let mut project = Project::new(name.to_string());
    project.settings.width = 1920;
    project.settings.height = 1080;
    project.settings.frame_rate = FrameRate::FPS_30;
    project
}

#[test]
#[ignore = "writes artifacts/phase59/native/*"]
fn write_native_fixtures() {
    let dir = out_dir();
    let red = dir.join("red.mp4");
    let blue = dir.join("blue.mp4");
    let music = dir.join("music.wav");
    let voice = dir.join("voice.wav");
    solid(&red, "0xFF0000");
    solid(&blue, "0x0000FF");
    tone(&music, 220, 8, -14.0);
    tone(&voice, 1000, 3, -10.0);

    // mix.tcproj: a red video clip, music underneath, a voice-like tone from 2 s.
    let mut mix = base_project("p59 mix");
    mix.assets
        .push(asset("ast_red", &red, MediaKind::Video, 180, true));
    mix.assets
        .push(asset("ast_music", &music, MediaKind::Audio, 240, false));
    mix.assets
        .push(asset("ast_voice", &voice, MediaKind::Audio, 90, false));
    for (name, kind, items) in [
        (
            "V1",
            TrackKind::Video,
            vec![item("ast_red", "Red", ItemKind::Video, 0, 180, 0)],
        ),
        (
            "Music",
            TrackKind::Audio,
            vec![item("ast_music", "Music", ItemKind::Audio, 0, 240, 0)],
        ),
        (
            "Voice",
            TrackKind::Audio,
            vec![item("ast_voice", "Voice", ItemKind::Audio, 60, 90, 0)],
        ),
    ] {
        let mut track = Track::new(name.to_string(), kind);
        track.items = items;
        mix.tracks.push(track);
    }
    save_project_atomic(&mix, &dir.join("mix.tcproj")).unwrap();

    // trans.tcproj: red then blue with a 30-frame WipeLeft transition at the cut (frame 90).
    let mut trans = base_project("p59 trans");
    trans
        .assets
        .push(asset("ast_red", &red, MediaKind::Video, 180, true));
    trans
        .assets
        .push(asset("ast_blue", &blue, MediaKind::Video, 180, true));
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    let left = item("ast_red", "Red", ItemKind::Video, 0, 90, 30);
    let right = item("ast_blue", "Blue", ItemKind::Video, 90, 90, 30);
    track.transitions.push(CutTransition {
        transition_id: "tr_wipe".into(),
        track_id: track.track_id.clone(),
        left_item_id: left.item_id.clone(),
        right_item_id: right.item_id.clone(),
        kind: TransitionKind::WipeLeft,
        duration: RationalTime::new(30, FrameRate::FPS_30),
    });
    track.items = vec![left, right];
    trans.tracks.push(track);
    save_project_atomic(&trans, &dir.join("trans.tcproj")).unwrap();
}

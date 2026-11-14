//! Writes the G52 registration project (full-canvas grid video, 1920x1080, 30 fps) next to
//! `artifacts/phase60/g52/grid.mp4` (see `scripts/qualification/native/make_registration_grid.py`).

use std::fs;
use std::path::Path;
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, AssetDimensions, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::persistence::save_project_atomic;

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
#[ignore = "writes artifacts/phase60/g52/grid.tcproj"]
fn write_registration_project() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/phase60/g52")
        .canonicalize()
        .expect("run make_registration_grid.py first");
    let grid = dir.join("grid.mp4");
    let mut project = base_project("g52 registration");
    project
        .assets
        .push(asset("ast_grid", &grid, MediaKind::Video, 180, true));
    let mut track = Track::new("V1".to_string(), TrackKind::Video);
    track.items = vec![item("ast_grid", "Grid", ItemKind::Video, 0, 180, 0)];
    project.tracks.push(track);
    save_project_atomic(&project, &dir.join("grid.tcproj")).unwrap();
}

//! Deterministic Schema-v3 Fixtures A and B for Phase 17 Qualification
//! Conforms strictly to PR-01, PR-06, PR-17, and P17-AC-06..15.

use std::fs;
use std::path::{Path, PathBuf};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, Crop, CutTransition, ItemKind, Keyframe, KeyframeEasing, KeyframeProperty, MediaKind,
    Project, ProjectSettings, TimelineItem, Track, TrackKind, Transform, TransitionKind,
    CURRENT_SCHEMA_VERSION,
};
use tinixcut_lib::modules::media::resolver::MediaBinaryResolver;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureAVariant {
    Resolution720p,
    Resolution1080p,
}

impl FixtureAVariant {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            FixtureAVariant::Resolution720p => (1280, 720),
            FixtureAVariant::Resolution1080p => (1920, 1080),
        }
    }
}

/// Generates deterministic synthetic media required for Fixture A export qualification.
pub fn ensure_fixture_a_synthetic_media(
    dir: &Path,
    variant: FixtureAVariant,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let (w, h) = variant.dimensions();
    let video_name = format!("fixture_a_video_{}x{}.mp4", w, h);
    let video_path = dir.join(&video_name);
    let audio_path = dir.join("fixture_a_audio_bed.mp3");

    if !video_path.exists() {
        let mut cmd = MediaBinaryResolver::command("ffmpeg")
            .map_err(|e| format!("Failed to resolve ffmpeg: {e}"))?;
        let status = cmd
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc=duration=6:size={}x{}:rate=30", w, h),
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=6",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-ar",
                "48000",
                video_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| format!("Failed to spawn ffmpeg: {e}"))?;
        if !status.status.success() {
            return Err(format!(
                "ffmpeg failed to create synthetic video: {}",
                String::from_utf8_lossy(&status.stderr)
            ));
        }
    }

    if !audio_path.exists() {
        let mut cmd = MediaBinaryResolver::command("ffmpeg")
            .map_err(|e| format!("Failed to resolve ffmpeg: {e}"))?;
        let status = cmd
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=220:duration=6",
                "-c:a",
                "mp3",
                "-b:a",
                "192k",
                "-ar",
                "48000",
                audio_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| format!("Failed to spawn ffmpeg: {e}"))?;
        if !status.status.success() {
            return Err(format!(
                "ffmpeg failed to create synthetic audio: {}",
                String::from_utf8_lossy(&status.stderr)
            ));
        }
    }

    Ok((video_path, audio_path))
}

/// Builds deterministic Fixture A (Semantic-Rich Schema-v3 Workload).
/// 6.0 seconds @ 30 fps, 2 video clips, 1 title/text overlay, 1 audio bed,
/// 1 cut-owned CrossDissolve, nonlinear Bézier keyframes, audio volume automation.
pub fn build_fixture_a(
    variant: FixtureAVariant,
    video_path: Option<&Path>,
    audio_path: Option<&Path>,
) -> Project {
    let rate = FrameRate::FPS_30;
    let (width, height) = variant.dimensions();

    let mut project = Project::new("Phase 17 Fixture A - Semantic Rich".to_string());
    project.schema_version = CURRENT_SCHEMA_VERSION;
    project.project_id = "proj-fixture-a-001".to_string();
    project.settings = ProjectSettings {
        width,
        height,
        canvas_background_color: "#000000".to_string(),
        frame_rate: rate,
        sample_rate: 48000,
        channels: 2,
    };

    let v_path_str = video_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "media/fixture_a_video.mp4".to_string());
    let a_path_str = audio_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "media/fixture_a_audio.mp3".to_string());

    let video_asset_id = "asset-fixa-video-01".to_string();
    project.assets.push(Asset {
        asset_id: video_asset_id.clone(),
        name: "fixture_a_video.mp4".to_string(),
        path: v_path_str,
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::from_frames(180, rate)), // 6.0s
        file_size: 1024 * 1024,
        dimensions: None,
    });

    let audio_asset_id = "asset-fixa-audio-01".to_string();
    project.assets.push(Asset {
        asset_id: audio_asset_id.clone(),
        name: "fixture_a_audio.mp3".to_string(),
        path: a_path_str,
        media_kind: MediaKind::Audio,
        duration: Some(RationalTime::from_frames(180, rate)), // 6.0s
        file_size: 512 * 1024,
        dimensions: None,
    });

    // Track 0: Main Video with 2 sequential clips and 1 cut-owned CrossDissolve
    let mut v_track = Track::new("Main Video".to_string(), TrackKind::Video);
    v_track.track_id = "trk-fixa-video-00".to_string();

    let mut clip1 = TimelineItem::new(
        Some(video_asset_id.clone()),
        "clip-fixa-v1".to_string(),
        ItemKind::Video,
        TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(90, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(90, rate),
        ),
    )
    .unwrap();
    clip1.item_id = "item-fixa-clip-01".to_string();
    clip1.transform = Transform {
        position_x: 0.0,
        position_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        rotation: 0.0,
    };
    clip1.opacity = 1.0;
    clip1.crop = Crop::default();
    let mut kf_v1_start = Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::from_frames(0, rate),
        1.0,
    );
    kf_v1_start.outgoing_easing = KeyframeEasing::CubicBezier {
        x1: 0.25,
        y1: 0.1,
        x2: 0.25,
        y2: 1.0,
    };
    let kf_v1_end = Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::from_frames(90, rate),
        0.9,
    );
    clip1.keyframes.push(kf_v1_start);
    clip1.keyframes.push(kf_v1_end);

    let mut clip2 = TimelineItem::new(
        Some(video_asset_id),
        "clip-fixa-v2".to_string(),
        ItemKind::Video,
        TimeRange::new(
            RationalTime::from_frames(90, rate),
            RationalTime::from_frames(90, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(90, rate),
            RationalTime::from_frames(90, rate),
        ),
    )
    .unwrap();
    clip2.item_id = "item-fixa-clip-02".to_string();
    clip2.transform = Transform {
        position_x: 0.0,
        position_y: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        rotation: 0.0,
    };
    clip2.opacity = 1.0;
    clip2.crop = Crop::default();
    let mut kf_v2_start = Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::from_frames(0, rate),
        0.95,
    );
    kf_v2_start.outgoing_easing = KeyframeEasing::EaseInOut;
    let kf_v2_end = Keyframe::new(
        KeyframeProperty::Opacity,
        RationalTime::from_frames(90, rate),
        1.0,
    );
    clip2.keyframes.push(kf_v2_start);
    clip2.keyframes.push(kf_v2_end);

    let cut_transition = CutTransition {
        transition_id: "trans-fixa-cd-01".to_string(),
        track_id: v_track.track_id.clone(),
        left_item_id: clip1.item_id.clone(),
        right_item_id: clip2.item_id.clone(),
        kind: TransitionKind::CrossDissolve,
        duration: RationalTime::from_frames(30, rate), // 1.0s cut transition
    };

    v_track.items.push(clip1);
    v_track.items.push(clip2);
    v_track.transitions.push(cut_transition);
    project.tracks.push(v_track);

    // Track 1: Text / Title Overlay Track
    let mut t_track = Track::new("Title Overlay".to_string(), TrackKind::Video);
    t_track.track_id = "trk-fixa-text-01".to_string();

    let mut text_item = TimelineItem::new(
        None,
        "title-card-fixa".to_string(),
        ItemKind::Text,
        TimeRange::new(
            RationalTime::from_frames(15, rate),
            RationalTime::from_frames(120, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(120, rate),
        ),
    )
    .unwrap();
    text_item.item_id = "item-fixa-text-01".to_string();
    text_item.text_content = Some("TinixCut Fixture A Release Candidate".to_string());
    let mut kf_t_start = Keyframe::new(
        KeyframeProperty::ScaleX,
        RationalTime::from_frames(0, rate),
        1.0,
    );
    kf_t_start.outgoing_easing = KeyframeEasing::CubicBezier {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    };
    let kf_t_end = Keyframe::new(
        KeyframeProperty::ScaleX,
        RationalTime::from_frames(120, rate),
        1.2,
    );
    text_item.keyframes.push(kf_t_start);
    text_item.keyframes.push(kf_t_end);
    t_track.items.push(text_item);
    project.tracks.push(t_track);

    // Track 2: Background Audio Bed with automation
    let mut a_track = Track::new("Audio Bed".to_string(), TrackKind::Audio);
    a_track.track_id = "trk-fixa-audio-02".to_string();

    let mut audio_item = TimelineItem::new(
        Some(audio_asset_id),
        "audio-bed-fixa".to_string(),
        ItemKind::Audio,
        TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(180, rate),
        ),
        TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(180, rate),
        ),
    )
    .unwrap();
    audio_item.item_id = "item-fixa-audio-01".to_string();
    audio_item.volume = 1.0;

    let mut kf_a1 = Keyframe::new(
        KeyframeProperty::Volume,
        RationalTime::from_frames(0, rate),
        0.2,
    );
    kf_a1.outgoing_easing = KeyframeEasing::EaseIn;

    let mut kf_a2 = Keyframe::new(
        KeyframeProperty::Volume,
        RationalTime::from_frames(45, rate),
        0.8,
    );
    kf_a2.outgoing_easing = KeyframeEasing::CubicBezier {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    };

    let mut kf_a3 = Keyframe::new(
        KeyframeProperty::Volume,
        RationalTime::from_frames(135, rate),
        0.8,
    );
    kf_a3.outgoing_easing = KeyframeEasing::EaseOut;

    let kf_a4 = Keyframe::new(
        KeyframeProperty::Volume,
        RationalTime::from_frames(180, rate),
        0.0,
    );

    audio_item.keyframes.push(kf_a1);
    audio_item.keyframes.push(kf_a2);
    audio_item.keyframes.push(kf_a3);
    audio_item.keyframes.push(kf_a4);
    a_track.items.push(audio_item);
    project.tracks.push(a_track);

    project.validate().expect("Fixture A must be valid");
    project
}

/// Builds deterministic Fixture B (Long-Project Scale Schema-v3 Workload).
/// 10.0 seconds @ 30 fps, 20 tracks (14 video, 4 audio, 2 text), 200 items (10 per track, 30 frames each),
/// 20 CrossDissolves across Video Tracks 0..3, 60+ nonlinear visual keyframes, 120 audio volume keyframes.
pub fn build_fixture_b(temp_dir: Option<&Path>) -> Project {
    let rate = FrameRate::FPS_30;

    let mut project = Project::new("Phase 17 Fixture B - Scale Workload".to_string());
    project.schema_version = CURRENT_SCHEMA_VERSION;
    project.project_id = "proj-fixture-b-scale".to_string();
    project.settings = ProjectSettings {
        width: 1920,
        height: 1080,
        canvas_background_color: "#000000".to_string(),
        frame_rate: rate,
        sample_rate: 48000,
        channels: 2,
    };

    let video_path = temp_dir
        .map(|p| {
            p.join("dummy_fixture_b_video.mp4")
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_else(|| "media/fixture_b_video.mp4".to_string());
    let audio_path = temp_dir
        .map(|p| {
            p.join("dummy_fixture_b_audio.mp3")
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_else(|| "media/fixture_b_audio.mp3".to_string());

    let video_asset_id = "asset-fixb-video-01".to_string();
    project.assets.push(Asset {
        asset_id: video_asset_id.clone(),
        name: "fixture_b_video.mp4".to_string(),
        path: video_path,
        media_kind: MediaKind::Video,
        duration: Some(RationalTime::from_frames(300, rate)), // 10.0s
        file_size: 10 * 1024 * 1024,
        dimensions: None,
    });

    let audio_asset_id = "asset-fixb-audio-01".to_string();
    project.assets.push(Asset {
        asset_id: audio_asset_id.clone(),
        name: "fixture_b_audio.mp3".to_string(),
        path: audio_path,
        media_kind: MediaKind::Audio,
        duration: Some(RationalTime::from_frames(300, rate)), // 10.0s
        file_size: 5 * 1024 * 1024,
        dimensions: None,
    });

    let mut total_visual_nonlinear_kfs = 0;
    let mut total_audio_kfs = 0;
    let mut total_cross_dissolves = 0;

    // 20 Tracks total:
    // Tracks 0..13: 14 video tracks
    // Tracks 14..17: 4 audio tracks
    // Tracks 18..19: 2 text tracks
    for t_idx in 0..20 {
        let (kind, track_name, trk_id) = if t_idx < 14 {
            (
                TrackKind::Video,
                format!("Video Track {:02}", t_idx),
                format!("trk-v-{:02}", t_idx),
            )
        } else if t_idx < 18 {
            (
                TrackKind::Audio,
                format!("Audio Track {:02}", t_idx - 14),
                format!("trk-a-{:02}", t_idx - 14),
            )
        } else {
            (
                TrackKind::Video,
                format!("Text Track {:02}", t_idx - 18),
                format!("trk-t-{:02}", t_idx - 18),
            )
        };

        let mut track = Track::new(track_name, kind);
        track.track_id = trk_id;

        // Exactly 10 items per track (30 frames each = 300 frames total)
        for c_idx in 0..10 {
            let start_frame = c_idx * 30;
            let item_dur_frames = 30;
            let t_range = TimeRange::new(
                RationalTime::from_frames(start_frame, rate),
                RationalTime::from_frames(item_dur_frames, rate),
            );
            let s_range = TimeRange::new(
                RationalTime::from_frames(0, rate),
                RationalTime::from_frames(item_dur_frames, rate),
            );

            let (item_kind, asset_id, item_name) = if t_idx < 14 {
                (
                    ItemKind::Video,
                    Some(video_asset_id.clone()),
                    format!("clip-v-t{:02}-c{:02}", t_idx, c_idx),
                )
            } else if t_idx < 18 {
                (
                    ItemKind::Audio,
                    Some(audio_asset_id.clone()),
                    format!("clip-a-t{:02}-c{:02}", t_idx - 14, c_idx),
                )
            } else {
                (
                    ItemKind::Text,
                    None,
                    format!("clip-t-t{:02}-c{:02}", t_idx - 18, c_idx),
                )
            };

            let mut item =
                TimelineItem::new(asset_id, item_name, item_kind, t_range, s_range).unwrap();
            item.item_id = format!("item-t{:02}-c{:02}", t_idx, c_idx);

            if item_kind == ItemKind::Text {
                item.text_content = Some(format!("Overlay Text T{} Item {}", t_idx, c_idx));
            }

            // Visual nonlinear keyframes: on Tracks 0..7 (8 tracks * 10 items = 80 items)
            if t_idx < 8 {
                let mut kf1 = Keyframe::new(
                    KeyframeProperty::Opacity,
                    RationalTime::from_frames(0, rate),
                    1.0,
                );
                kf1.outgoing_easing = KeyframeEasing::CubicBezier {
                    x1: 0.42,
                    y1: 0.0,
                    x2: 0.58,
                    y2: 1.0,
                };
                let kf2 = Keyframe::new(
                    KeyframeProperty::Opacity,
                    RationalTime::from_frames(30, rate),
                    0.95,
                );
                item.keyframes.push(kf1);
                item.keyframes.push(kf2);
                total_visual_nonlinear_kfs += 1;
            }

            // Audio volume keyframes: exactly 120 keyframes across 40 audio items (Tracks 14..17)
            if item_kind == ItemKind::Audio {
                let mut kf1 = Keyframe::new(
                    KeyframeProperty::Volume,
                    RationalTime::from_frames(0, rate),
                    0.3,
                );
                kf1.outgoing_easing = KeyframeEasing::EaseIn;

                let mut kf2 = Keyframe::new(
                    KeyframeProperty::Volume,
                    RationalTime::from_frames(15, rate),
                    0.9,
                );
                kf2.outgoing_easing = KeyframeEasing::CubicBezier {
                    x1: 0.25,
                    y1: 0.1,
                    x2: 0.25,
                    y2: 1.0,
                };

                let kf3 = Keyframe::new(
                    KeyframeProperty::Volume,
                    RationalTime::from_frames(30, rate),
                    0.6,
                );

                item.keyframes.push(kf1);
                item.keyframes.push(kf2);
                item.keyframes.push(kf3);
                total_audio_kfs += 3;
            }

            track.items.push(item);
        }

        // CrossDissolves: exactly 20 distributed across Video Tracks 0..3 (5 transitions per track)
        if t_idx < 4 {
            // Pairwise cuts: (0,1), (2,3), (4,5), (6,7), (8,9)
            for pair in 0..5 {
                let left_idx = pair * 2;
                let right_idx = left_idx + 1;
                let left_id = track.items[left_idx].item_id.clone();
                let right_id = track.items[right_idx].item_id.clone();

                let trans = CutTransition {
                    transition_id: format!("trans-t{:02}-p{:02}", t_idx, pair),
                    track_id: track.track_id.clone(),
                    left_item_id: left_id,
                    right_item_id: right_id,
                    kind: TransitionKind::CrossDissolve,
                    duration: RationalTime::from_frames(10, rate), // 10 frames (5 left, 5 right)
                };
                track.transitions.push(trans);
                total_cross_dissolves += 1;
            }
        }

        project.tracks.push(track);
    }

    assert_eq!(
        project.tracks.len(),
        20,
        "Fixture B must have exactly 20 tracks"
    );
    let total_items: usize = project.tracks.iter().map(|t| t.items.len()).sum();
    assert_eq!(total_items, 200, "Fixture B must have exactly 200 items");
    assert_eq!(
        total_cross_dissolves, 20,
        "Fixture B must have exactly 20 CrossDissolves"
    );
    assert!(
        total_visual_nonlinear_kfs >= 60,
        "Fixture B must have >= 60 visual nonlinear keyframes (found {total_visual_nonlinear_kfs})"
    );
    assert_eq!(
        total_audio_kfs, 120,
        "Fixture B must have exactly 120 audio keyframes"
    );

    project.validate().expect("Fixture B must be valid");
    project
}

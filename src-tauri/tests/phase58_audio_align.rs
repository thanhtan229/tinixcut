//! Phase 58F acceptance: audio-based clip alignment proposals on real decoded media.

use std::fs;
use std::path::{Path, PathBuf};
use tinixcut_lib::modules::domain::time::{FrameRate, RationalTime, TimeRange};
use tinixcut_lib::modules::domain::{
    Asset, ClipSpeed, ItemKind, MediaKind, Project, TimelineItem, Track, TrackKind,
};
use tinixcut_lib::modules::media::align::propose_alignment;

const SAMPLE_RATE: u32 = 48_000;

fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tinixcut_p58f_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Deterministic broadband noise in -1..1.
fn noise(len: usize, seed: u64) -> Vec<f32> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
        })
        .collect()
}

fn write_wav(path: &Path, samples: &[f32]) {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        bytes.extend_from_slice(&((s.clamp(-1.0, 1.0) * 30000.0) as i16).to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

fn audio_asset(id: &str, path: &Path) -> Asset {
    Asset {
        asset_id: id.into(),
        name: id.into(),
        path: path.to_string_lossy().into_owned(),
        media_kind: MediaKind::Audio,
        duration: None,
        file_size: 1000,
        dimensions: None,
    }
}

fn project_with(clips: &[(&str, &Path, i64)], seconds: i64) -> Project {
    let rate = FrameRate::FPS_30;
    let mut project = Project::new("Align".into());
    project.settings.frame_rate = rate;
    let mut track = Track::new("A1".into(), TrackKind::Audio);
    for (index, (id, path, start_frame)) in clips.iter().enumerate() {
        project
            .assets
            .push(audio_asset(&format!("ast_{index}"), path));
        let timeline = TimeRange::new(
            RationalTime::new(*start_frame, rate),
            RationalTime::new(seconds * 30, rate),
        );
        let source = TimeRange::new(
            RationalTime::new(0, rate),
            RationalTime::new(seconds * 30, rate),
        );
        track.items.push(
            TimelineItem::new(
                Some(format!("ast_{index}")),
                (*id).into(),
                ItemKind::Audio,
                timeline,
                source,
            )
            .unwrap(),
        );
    }
    project.tracks.push(track);
    project
}

/// Items are looked up by name; `TimelineItem::new` generates the item id.
fn id_of(project: &Project, name: &str) -> String {
    project
        .tracks
        .iter()
        .flat_map(|t| t.items.iter())
        .find(|i| i.name == name)
        .map(|i| i.item_id.clone())
        .unwrap_or_else(|| format!("missing_{name}"))
}

fn propose(
    project: &Project,
    reference: &str,
    others: &[&str],
) -> Result<tinixcut_lib::modules::media::align::AlignmentReport, String> {
    let ids: Vec<String> = others.iter().map(|n| id_of(project, n)).collect();
    propose_alignment(project, &id_of(project, reference), &ids, None)
}

#[test]
fn p58f_proposal_recovers_a_known_offset_within_one_frame() {
    let dir = test_dir("offset");
    let scene = noise(SAMPLE_RATE as usize * 6, 21);
    let reference_path = dir.join("camera_a.wav");
    write_wav(&reference_path, &scene);

    // Camera B heard the same scene 1.5 s later in its own recording, with extra room noise.
    let delay = (SAMPLE_RATE as f64 * 1.5) as usize;
    let mut delayed = vec![0.0f32; delay];
    delayed.extend(
        scene
            .iter()
            .zip(noise(scene.len(), 99))
            .map(|(s, n)| 0.7 * s + 0.1 * n),
    );
    let other_path = dir.join("camera_b.wav");
    write_wav(&other_path, &delayed);

    // The reference sits at frame 120; B was dropped at frame 0. B must start 1.5 s (45 frames)
    // before the reference start: 120 - 45 = 75.
    let project = project_with(
        &[("ref", &reference_path, 120), ("other", &other_path, 0)],
        6,
    );
    let report = propose(&project, "ref", &["other"]).unwrap();
    let proposal = &report.proposals[0];
    assert!(proposal.confident, "{proposal:?}");
    assert!(
        (proposal.proposed_start_frame - 75).abs() <= 1,
        "expected frame 75 +/- 1, got {proposal:?}"
    );
    assert_eq!(proposal.current_start_frame, 0);
    assert_eq!(
        proposal.delta_frames,
        proposal.proposed_start_frame - proposal.current_start_frame
    );
    assert!((proposal.offset_seconds - 1.5).abs() < 0.04);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p58f_unrelated_audio_is_reported_as_not_confident() {
    let dir = test_dir("unrelated");
    let a = dir.join("a.wav");
    let b = dir.join("b.wav");
    write_wav(&a, &noise(SAMPLE_RATE as usize * 4, 5));
    write_wav(&b, &noise(SAMPLE_RATE as usize * 4, 6));
    let project = project_with(&[("ref", &a, 0), ("other", &b, 30)], 4);
    let report = propose(&project, "ref", &["other"]).unwrap();
    let proposal = &report.proposals[0];
    assert!(
        !proposal.confident,
        "unrelated audio must not look aligned: {proposal:?}"
    );
    assert!(proposal.note.is_some());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn p58f_rejects_invalid_requests_without_touching_the_project() {
    let dir = test_dir("invalid");
    let a = dir.join("a.wav");
    write_wav(&a, &noise(SAMPLE_RATE as usize * 3, 1));
    let mut project = project_with(&[("ref", &a, 0), ("other", &a, 0)], 3);

    assert!(propose(&project, "ref", &[]).is_err());
    assert!(propose(&project, "ref", &["ref"]).is_err());
    assert!(propose(&project, "ref", &["missing"]).is_err());

    project.tracks[0].items[1].speed = ClipSpeed::Double;
    let err = propose(&project, "ref", &["other"]).unwrap_err();
    assert!(err.contains("1x speed"), "{err}");
    fs::remove_dir_all(dir).unwrap();
}

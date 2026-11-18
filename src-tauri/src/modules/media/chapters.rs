//! YouTube-style chapter list built from project markers.
//!
//! The list is a report: violations of the platform's rules are returned to the caller instead
//! of being silently corrected (no markers are moved, merged or invented).

use crate::modules::domain::Project;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

/// YouTube needs at least three chapters, the first at 0:00, each at least 10 seconds long.
pub const MIN_CHAPTERS: usize = 3;
pub const MIN_CHAPTER_SECONDS: u64 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub marker_id: String,
    pub title: String,
    pub start_seconds: u64,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChapterReport {
    pub chapters: Vec<Chapter>,
    /// One `timestamp title` line per chapter, ready to paste into a video description.
    pub text: String,
    /// Reasons the list would not be accepted as YouTube chapters. Empty when valid.
    pub violations: Vec<String>,
    pub duration_seconds: u64,
}

pub fn format_timestamp(total_seconds: u64) -> String {
    let (h, m, s) = (
        total_seconds / 3600,
        (total_seconds / 60) % 60,
        total_seconds % 60,
    );
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn project_chapters(project: &Project) -> ChapterReport {
    let duration_seconds = project
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| item.timeline_range.end().to_seconds())
        .fold(0.0f64, f64::max)
        .floor() as u64;

    let mut markers: Vec<_> = project.markers.iter().collect();
    markers.sort_by(|a, b| {
        a.position
            .to_seconds()
            .partial_cmp(&b.position.to_seconds())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.marker_id.cmp(&b.marker_id))
    });

    let chapters: Vec<Chapter> = markers
        .iter()
        .map(|marker| {
            let start_seconds = marker.position.to_seconds().floor().max(0.0) as u64;
            Chapter {
                marker_id: marker.marker_id.clone(),
                title: marker.label.trim().to_string(),
                start_seconds,
                timestamp: format_timestamp(start_seconds),
            }
        })
        .collect();

    let mut violations = Vec::new();
    if chapters.len() < MIN_CHAPTERS {
        violations.push(format!(
            "YouTube needs at least {MIN_CHAPTERS} chapters; the project has {} marker(s)",
            chapters.len()
        ));
    }
    if let Some(first) = chapters.first() {
        if first.start_seconds != 0 {
            violations.push(format!(
                "The first chapter must start at 00:00; the earliest marker is at {}",
                first.timestamp
            ));
        }
    }
    for pair in chapters.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.start_seconds == b.start_seconds {
            violations.push(format!(
                "Markers '{}' and '{}' fall in the same second ({})",
                a.title, b.title, a.timestamp
            ));
        } else if b.start_seconds - a.start_seconds < MIN_CHAPTER_SECONDS {
            violations.push(format!(
                "Chapter '{}' at {} lasts {} s; the minimum is {MIN_CHAPTER_SECONDS} s",
                a.title,
                a.timestamp,
                b.start_seconds - a.start_seconds
            ));
        }
    }
    if let Some(last) = chapters.last() {
        if duration_seconds > 0 && last.start_seconds >= duration_seconds {
            violations.push(format!(
                "Marker '{}' at {} is at or beyond the end of the timeline ({})",
                last.title,
                last.timestamp,
                format_timestamp(duration_seconds)
            ));
        } else if duration_seconds > last.start_seconds
            && duration_seconds - last.start_seconds < MIN_CHAPTER_SECONDS
        {
            violations.push(format!(
                "The last chapter '{}' at {} lasts {} s; the minimum is {MIN_CHAPTER_SECONDS} s",
                last.title,
                last.timestamp,
                duration_seconds - last.start_seconds
            ));
        }
    }
    for chapter in &chapters {
        if chapter.title.is_empty() {
            violations.push(format!("The marker at {} has no title", chapter.timestamp));
        }
    }

    let text = chapters
        .iter()
        .map(|c| format!("{} {}", c.timestamp, c.title))
        .collect::<Vec<_>>()
        .join("\n");
    ChapterReport {
        chapters,
        text,
        violations,
        duration_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{ItemKind, ProjectMarker, TimelineItem, Track, TrackKind};

    fn project_with(duration_secs: i64, markers: &[(&str, i64)]) -> Project {
        let rate = FrameRate::FPS_30;
        let mut project = Project::new("Chapters".into());
        project.settings.frame_rate = rate;
        let mut track = Track::new("V1".into(), TrackKind::Video);
        let range = TimeRange::new(
            RationalTime::new(0, rate),
            RationalTime::new(duration_secs * 30, rate),
        );
        track
            .items
            .push(TimelineItem::new(None, "clip".into(), ItemKind::Video, range, range).unwrap());
        project.tracks.push(track);
        for (index, (label, seconds)) in markers.iter().enumerate() {
            project.markers.push(ProjectMarker {
                marker_id: format!("mrk_{index}"),
                label: (*label).into(),
                position: RationalTime::new(seconds * 30, rate),
            });
        }
        project
    }

    #[test]
    fn formats_minutes_and_hours() {
        assert_eq!(format_timestamp(0), "00:00");
        assert_eq!(format_timestamp(75), "01:15");
        assert_eq!(format_timestamp(3661), "1:01:01");
    }

    #[test]
    fn valid_markers_produce_a_pasteable_list_without_violations() {
        let project = project_with(120, &[("Outro", 90), ("Intro", 0), ("Main", 30)]);
        let report = project_chapters(&project);
        assert_eq!(report.violations, Vec::<String>::new());
        assert_eq!(report.text, "00:00 Intro\n00:30 Main\n01:30 Outro");
        assert_eq!(report.duration_seconds, 120);
    }

    #[test]
    fn reports_each_rule_violation_without_fixing_anything() {
        let project = project_with(100, &[("A", 5), ("B", 9), ("C", 98)]);
        let report = project_chapters(&project);
        let joined = report.violations.join(" | ");
        assert!(joined.contains("must start at 00:00"), "{joined}");
        assert!(joined.contains("lasts 4 s"), "{joined}");
        assert!(joined.contains("last chapter"), "{joined}");
        assert_eq!(
            report.chapters.len(),
            3,
            "markers are never dropped or moved"
        );
        assert_eq!(report.chapters[0].start_seconds, 5);

        let few = project_chapters(&project_with(60, &[("Only", 0)]));
        assert!(few.violations.iter().any(|v| v.contains("at least 3")));
        let same = project_chapters(&project_with(60, &[("A", 0), ("B", 0), ("C", 30)]));
        assert!(same.violations.iter().any(|v| v.contains("same second")));
        let beyond = project_chapters(&project_with(30, &[("A", 0), ("B", 12), ("C", 40)]));
        assert!(beyond
            .violations
            .iter()
            .any(|v| v.contains("beyond the end")));
    }

    #[test]
    fn empty_project_reports_missing_chapters() {
        let report = project_chapters(&Project::new("Empty".into()));
        assert!(report.chapters.is_empty());
        assert!(report.text.is_empty());
        assert!(!report.violations.is_empty());
    }
}

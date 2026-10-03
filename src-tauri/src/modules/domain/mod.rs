//! Project Domain Model v1
//! The canonical, application-owned source of truth for video editing.
//! Invariant: No renderer-specific state, no arbitrary float seconds, stable IDs only.

pub mod id;
pub mod time;

use self::id::*;
use self::time::{FrameRate, RationalTime, TimeRange};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainError {
    #[error("Schema version mismatch: expected {expected}, found {found}")]
    SchemaVersionMismatch { expected: u32, found: u32 },

    #[error("Invalid ID '{0}': ID cannot be empty or invalid")]
    InvalidId(String),

    #[error("Item duration must be positive")]
    NonPositiveDuration,

    #[error(
        "Source range duration ({source_dur}) must match timeline range duration ({timeline_dur})"
    )]
    DurationMismatch {
        source_dur: String,
        timeline_dur: String,
    },

    #[error("Asset with ID '{0}' not found in project")]
    AssetNotFound(String),

    #[error("Track with ID '{0}' not found in project")]
    TrackNotFound(String),

    #[error("Item with ID '{0}' not found in project")]
    ItemNotFound(String),

    #[error("Track '{track_id}' has overlapping items: '{item1_id}' and '{item2_id}'")]
    OverlappingItems {
        track_id: String,
        item1_id: String,
        item2_id: String,
    },

    #[error("Domain validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Video,
    Audio,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Video,
    Audio,
    Image,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    pub position_x: f32, // Pixel offset or normalized (-1920 to +1920)
    pub position_y: f32,
    pub scale_x: f32, // 1.0 is default
    pub scale_y: f32,
    pub rotation: f32, // Degrees (-360 to +360)
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Crop {
    pub top: f32, // 0.0 to 1.0
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Self {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_30,
            sample_rate: 48000,
            channels: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub asset_id: String,
    pub name: String,
    pub path: String,
    pub media_kind: MediaKind,
    pub duration: Option<RationalTime>,
    pub file_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyframeProperty {
    Opacity,
    PositionX,
    PositionY,
    ScaleX,
    ScaleY,
    Rotation,
    Volume,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub property: KeyframeProperty,
    pub offset: RationalTime,
    pub value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    CrossDissolve,
    FadeBlack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration: RationalTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineItem {
    pub item_id: String,
    pub asset_id: Option<String>,
    pub name: String,
    pub kind: ItemKind,
    pub timeline_range: TimeRange,
    pub source_range: TimeRange,
    pub transform: Transform,
    pub crop: Crop,
    pub opacity: f32,
    pub volume: f32,
    pub text_content: Option<String>,
    pub fade_in: Option<RationalTime>,
    pub fade_out: Option<RationalTime>,
    pub transition: Option<Transition>,
    pub keyframes: Vec<Keyframe>,
}

impl TimelineItem {
    pub fn new(
        asset_id: Option<String>,
        name: String,
        kind: ItemKind,
        timeline_range: TimeRange,
        source_range: TimeRange,
    ) -> Result<Self, DomainError> {
        let item_id = generate_item_id();
        let item = Self {
            item_id,
            asset_id,
            name,
            kind,
            timeline_range,
            source_range,
            transform: Transform::default(),
            crop: Crop::default(),
            opacity: 1.0,
            volume: 1.0,
            text_content: None,
            fade_in: None,
            fade_out: None,
            transition: None,
            keyframes: Vec::new(),
        };
        item.validate()?;
        Ok(item)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.item_id) {
            return Err(DomainError::InvalidId(self.item_id.clone()));
        }
        if self.timeline_range.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        if self.source_range.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }

        // Compare duration in same rate
        let src_dur_aligned = self
            .source_range
            .duration
            .rescaled_to(self.timeline_range.duration.rate);
        if src_dur_aligned.value != self.timeline_range.duration.value {
            return Err(DomainError::DurationMismatch {
                source_dur: format!("{} frames", self.source_range.duration.value),
                timeline_dur: format!("{} frames", self.timeline_range.duration.value),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub track_id: String,
    pub name: String,
    pub kind: TrackKind,
    pub muted: bool,
    pub locked: bool,
    pub items: Vec<TimelineItem>,
}

impl Track {
    pub fn new(name: String, kind: TrackKind) -> Self {
        Self {
            track_id: generate_track_id(),
            name,
            kind,
            muted: false,
            locked: false,
            items: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.track_id) {
            return Err(DomainError::InvalidId(self.track_id.clone()));
        }

        for item in &self.items {
            item.validate()?;
        }

        // Check for overlaps between items on the same track
        for i in 0..self.items.len() {
            for j in (i + 1)..self.items.len() {
                let item_a = &self.items[i];
                let item_b = &self.items[j];
                if item_a.timeline_range.overlaps(&item_b.timeline_range) {
                    return Err(DomainError::OverlappingItems {
                        track_id: self.track_id.clone(),
                        item1_id: item_a.item_id.clone(),
                        item2_id: item_b.item_id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn find_item(&self, item_id: &str) -> Option<&TimelineItem> {
        self.items.iter().find(|i| i.item_id == item_id)
    }

    pub fn find_item_mut(&mut self, item_id: &str) -> Option<&mut TimelineItem> {
        self.items.iter_mut().find(|i| i.item_id == item_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub schema_version: u32,
    pub project_id: String,
    pub name: String,
    pub revision: u64,
    pub settings: ProjectSettings,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
    pub created_at: String,
    pub updated_at: String,
}

impl Project {
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: generate_project_id(),
            name,
            revision: 0,
            settings: ProjectSettings::default(),
            assets: Vec::new(),
            tracks: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(DomainError::SchemaVersionMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: self.schema_version,
            });
        }
        if !is_valid_id(&self.project_id) {
            return Err(DomainError::InvalidId(self.project_id.clone()));
        }

        // Validate all tracks
        for track in &self.tracks {
            track.validate()?;
        }

        // Validate asset references in clips
        for track in &self.tracks {
            for item in &track.items {
                if let Some(ref asset_id) = item.asset_id {
                    if !self.assets.iter().any(|a| &a.asset_id == asset_id) {
                        return Err(DomainError::AssetNotFound(asset_id.clone()));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn find_track(&self, track_id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.track_id == track_id)
    }

    pub fn find_track_mut(&mut self, track_id: &str) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.track_id == track_id)
    }

    pub fn find_item_and_track(&self, item_id: &str) -> Option<(&Track, &TimelineItem)> {
        for track in &self.tracks {
            if let Some(item) = track.find_item(item_id) {
                return Some((track, item));
            }
        }
        None
    }

    pub fn total_duration(&self) -> RationalTime {
        let rate = self.settings.frame_rate;
        let mut max_end = RationalTime::zero(rate);
        for track in &self.tracks {
            for item in &track.items {
                let item_end = item.timeline_range.end().rescaled_to(rate);
                if item_end > max_end {
                    max_end = item_end;
                }
            }
        }
        max_end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation_and_validation() {
        let project = Project::new("Test Project".to_string());
        assert_eq!(project.schema_version, 1);
        assert_eq!(project.revision, 0);
        assert!(project.validate().is_ok());
    }

    #[test]
    fn test_item_duration_and_range_validation() {
        let rate = FrameRate::FPS_30;
        let t_range = TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(30, rate),
        );
        let s_range = TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(30, rate),
        );

        let item = TimelineItem::new(
            None,
            "Clip 1".to_string(),
            ItemKind::Video,
            t_range,
            s_range,
        )
        .unwrap();
        assert!(item.validate().is_ok());

        // Mismatched durations
        let s_bad_range = TimeRange::new(
            RationalTime::zero(rate),
            RationalTime::from_frames(60, rate),
        );
        let bad_item = TimelineItem::new(
            None,
            "Bad Clip".to_string(),
            ItemKind::Video,
            t_range,
            s_bad_range,
        );
        assert!(bad_item.is_err());
    }

    #[test]
    fn test_track_overlapping_items_rejected() {
        let rate = FrameRate::FPS_30;
        let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);

        let t1 = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(30, rate),
        );
        let s1 = t1;
        let item1 = TimelineItem::new(None, "Clip 1".to_string(), ItemKind::Video, t1, s1).unwrap();

        // Item 2 overlaps item 1 (starts at frame 15, ends at 45)
        let t2 = TimeRange::new(
            RationalTime::from_frames(15, rate),
            RationalTime::from_frames(30, rate),
        );
        let s2 = t2;
        let item2 = TimelineItem::new(None, "Clip 2".to_string(), ItemKind::Video, t2, s2).unwrap();

        track.items.push(item1);
        track.items.push(item2);

        assert!(matches!(
            track.validate(),
            Err(DomainError::OverlappingItems { .. })
        ));
    }
}

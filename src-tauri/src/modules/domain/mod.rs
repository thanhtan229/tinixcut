//! Project Domain Model v1
//! The canonical, application-owned source of truth for video editing.
//! Invariant: No renderer-specific state, no arbitrary float seconds, stable IDs only.

pub mod evaluator;
pub mod id;
pub mod time;

use self::id::*;
use self::time::{FrameRate, RationalTime, TimeRange};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 3;

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

    #[error("Transition with ID '{0}' not found on track")]
    TransitionNotFound(String),

    #[error("Cut transition participants must be video items (found non-video item)")]
    UnsupportedTransitionMedia,

    #[error("Transition required handle exceeds available frames: postroll {available_postroll} < {required_postroll} or preroll {available_preroll} < {required_preroll}")]
    InsufficientTransitionHandle {
        required_preroll: RationalTime,
        available_preroll: RationalTime,
        required_postroll: RationalTime,
        available_postroll: RationalTime,
    },

    #[error("Transition handle availability cannot be verified: media duration or stream unprobed/missing")]
    TransitionHandleUnverifiable { reason: String },

    #[error("Cut transition participants '{left_id}' and '{right_id}' are not immediately adjacent on track '{track_id}'")]
    NonAdjacentTransitionParticipants {
        track_id: String,
        left_id: String,
        right_id: String,
    },

    #[error("A cut transition already exists at cut between '{left_id}' and '{right_id}'")]
    DuplicateCutTransition { left_id: String, right_id: String },

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum KeyframeEasing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}

impl KeyframeEasing {
    pub fn validate(&self) -> Result<(), DomainError> {
        match *self {
            KeyframeEasing::Linear
            | KeyframeEasing::EaseIn
            | KeyframeEasing::EaseOut
            | KeyframeEasing::EaseInOut => Ok(()),
            KeyframeEasing::CubicBezier { x1, y1, x2, y2 } => {
                if !x1.is_finite() || !y1.is_finite() || !x2.is_finite() || !y2.is_finite() {
                    return Err(DomainError::Validation(
                        "CubicBezier control points must be finite numbers".to_string(),
                    ));
                }
                if !(0.0..=1.0).contains(&x1)
                    || !(0.0..=1.0).contains(&y1)
                    || !(0.0..=1.0).contains(&x2)
                    || !(0.0..=1.0).contains(&y2)
                {
                    return Err(DomainError::Validation(
                        "CubicBezier control points must be in range [0.0, 1.0]".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }

    pub fn is_linear(&self) -> bool {
        matches!(self, KeyframeEasing::Linear)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub property: KeyframeProperty,
    pub offset: RationalTime,
    pub value: f32,
    #[serde(default)]
    pub outgoing_easing: KeyframeEasing,
}

impl Keyframe {
    pub const fn new(property: KeyframeProperty, offset: RationalTime, value: f32) -> Self {
        Self {
            property,
            offset,
            value,
            outgoing_easing: KeyframeEasing::Linear,
        }
    }

    pub const fn with_easing(
        property: KeyframeProperty,
        offset: RationalTime,
        value: f32,
        outgoing_easing: KeyframeEasing,
    ) -> Self {
        Self {
            property,
            offset,
            value,
            outgoing_easing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    FadeIn,
    FadeOut,
    CrossDissolve,
    FadeBlack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration: RationalTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CutTransition {
    pub transition_id: String,
    pub track_id: String,
    pub left_item_id: String,
    pub right_item_id: String,
    pub kind: TransitionKind,
    pub duration: RationalTime,
}

impl CutTransition {
    pub fn new(
        track_id: String,
        left_item_id: String,
        right_item_id: String,
        kind: TransitionKind,
        duration: RationalTime,
    ) -> Result<Self, DomainError> {
        let transition_id = generate_transition_id();
        let ct = Self {
            transition_id,
            track_id,
            left_item_id,
            right_item_id,
            kind,
            duration,
        };
        ct.validate_basic()?;
        Ok(ct)
    }

    pub fn validate_basic(&self) -> Result<(), DomainError> {
        if !is_valid_id(&self.transition_id) {
            return Err(DomainError::InvalidId(self.transition_id.clone()));
        }
        if !is_valid_id(&self.track_id) {
            return Err(DomainError::InvalidId(self.track_id.clone()));
        }
        if !is_valid_id(&self.left_item_id) {
            return Err(DomainError::InvalidId(self.left_item_id.clone()));
        }
        if !is_valid_id(&self.right_item_id) {
            return Err(DomainError::InvalidId(self.right_item_id.clone()));
        }
        if self.duration.value <= 0 {
            return Err(DomainError::NonPositiveDuration);
        }
        if self.kind != TransitionKind::CrossDissolve {
            return Err(DomainError::Validation(
                "Only CrossDissolve is supported for cut transitions in Phase 13".to_string(),
            ));
        }
        Ok(())
    }
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

        // Validate keyframes: easing validity and last-keyframe invariant per property channel
        let all_props = [
            KeyframeProperty::Opacity,
            KeyframeProperty::PositionX,
            KeyframeProperty::PositionY,
            KeyframeProperty::ScaleX,
            KeyframeProperty::ScaleY,
            KeyframeProperty::Rotation,
            KeyframeProperty::Volume,
        ];

        for kf in &self.keyframes {
            kf.outgoing_easing.validate()?;
        }

        for prop in all_props {
            let mut prop_kfs: Vec<&Keyframe> = self
                .keyframes
                .iter()
                .filter(|k| k.property == prop)
                .collect();
            if prop_kfs.is_empty() {
                continue;
            }
            prop_kfs.sort_by_key(|a| a.offset);

            // Check for collision at exact same semantic timestamp
            for w in prop_kfs.windows(2) {
                if w[0].offset.is_same_time(&w[1].offset) {
                    return Err(DomainError::Validation(format!(
                        "Collision: duplicate keyframes on property {:?} at semantic offset {:?}",
                        prop, w[0].offset
                    )));
                }
            }

            // Invariant KF-05: The last keyframe in each property channel has no outgoing segment
            // and its canonical outgoing_easing is Linear.
            let last_kf = prop_kfs[prop_kfs.len() - 1];
            if !last_kf.outgoing_easing.is_linear() {
                return Err(DomainError::Validation(format!(
                    "Last keyframe on property {:?} at offset {:?} must have Linear outgoing easing, found {:?}",
                    prop, last_kf.offset, last_kf.outgoing_easing
                )));
            }
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
    #[serde(default)]
    pub transitions: Vec<CutTransition>,
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
            transitions: Vec::new(),
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

        // Validate cut transitions
        for trans in &self.transitions {
            trans.validate_basic()?;
            if trans.track_id != self.track_id {
                return Err(DomainError::Validation(format!(
                    "CutTransition track_id '{}' does not match track '{}'",
                    trans.track_id, self.track_id
                )));
            }

            let left = self
                .find_item(&trans.left_item_id)
                .ok_or_else(|| DomainError::ItemNotFound(trans.left_item_id.clone()))?;
            let right = self
                .find_item(&trans.right_item_id)
                .ok_or_else(|| DomainError::ItemNotFound(trans.right_item_id.clone()))?;

            // CD-04: Video item check
            if left.kind != ItemKind::Video || right.kind != ItemKind::Video {
                return Err(DomainError::UnsupportedTransitionMedia);
            }

            // CD-02: Immediate Adjacency Invariant: left.end == right.start (canonical semantic equality)
            let rate = trans.duration.rate;
            if !left
                .timeline_range
                .is_adjacent_to_with_rate(&right.timeline_range, rate)
            {
                return Err(DomainError::NonAdjacentTransitionParticipants {
                    track_id: self.track_id.clone(),
                    left_id: left.item_id.clone(),
                    right_id: right.item_id.clone(),
                });
            }

            // CD-15: Deterministic L/R tick rule & editorial extent
            let dur_ticks = trans.duration.value;
            let l_ticks = dur_ticks / 2;
            let r_ticks = dur_ticks - l_ticks;
            let left_extent = left.timeline_range.duration.rescaled_to(rate).value;
            let right_extent = right.timeline_range.duration.rescaled_to(rate).value;
            if left_extent < l_ticks || right_extent < r_ticks {
                return Err(DomainError::Validation(format!(
                    "CutTransition duration (L={}, R={}) exceeds participant editorial extent (left={}, right={})",
                    l_ticks, r_ticks, left_extent, right_extent
                )));
            }
        }

        // CD-07: Transition singularity per cut
        for i in 0..self.transitions.len() {
            for j in (i + 1)..self.transitions.len() {
                let t_a = &self.transitions[i];
                let t_b = &self.transitions[j];
                if t_a.transition_id == t_b.transition_id {
                    return Err(DomainError::Validation(format!(
                        "Duplicate transition ID '{}'",
                        t_a.transition_id
                    )));
                }
                if t_a.left_item_id == t_b.left_item_id || t_a.right_item_id == t_b.right_item_id {
                    return Err(DomainError::DuplicateCutTransition {
                        left_id: t_b.left_item_id.clone(),
                        right_id: t_b.right_item_id.clone(),
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

    pub fn find_transition(&self, transition_id: &str) -> Option<&CutTransition> {
        self.transitions
            .iter()
            .find(|t| t.transition_id == transition_id)
    }

    pub fn find_transition_mut(&mut self, transition_id: &str) -> Option<&mut CutTransition> {
        self.transitions
            .iter_mut()
            .find(|t| t.transition_id == transition_id)
    }

    pub fn find_transition_at_cut(&self, left_id: &str, right_id: &str) -> Option<&CutTransition> {
        self.transitions
            .iter()
            .find(|t| t.left_item_id == left_id && t.right_item_id == right_id)
    }

    pub fn find_transition_at_cut_mut(
        &mut self,
        left_id: &str,
        right_id: &str,
    ) -> Option<&mut CutTransition> {
        self.transitions
            .iter_mut()
            .find(|t| t.left_item_id == left_id && t.right_item_id == right_id)
    }

    pub fn find_transitions_for_item(&self, item_id: &str) -> Vec<&CutTransition> {
        self.transitions
            .iter()
            .filter(|t| t.left_item_id == item_id || t.right_item_id == item_id)
            .collect()
    }

    pub fn remove_transition(&mut self, transition_id: &str) -> Option<CutTransition> {
        if let Some(pos) = self
            .transitions
            .iter()
            .position(|t| t.transition_id == transition_id)
        {
            Some(self.transitions.remove(pos))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientRecord {
    pub client_epoch: u64,
    pub highest_sequence: u64,
    pub last_active_revision: u64,
    pub registered_at_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientRegistry {
    pub min_accepted_epoch: u64,
    pub current_epoch: u64,
    pub max_active_clients: usize,
    pub clients: std::collections::HashMap<String, ClientRecord>,
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self {
            min_accepted_epoch: 0,
            current_epoch: 1,
            max_active_clients: 64,
            clients: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandLedgerEntry {
    pub operation_id: String,
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_epoch: Option<u64>,
    #[serde(default)]
    pub sequence_number: Option<u64>,
    pub fingerprint: String,
    pub status: String,
    pub applied_at_revision: u64,
    pub affected_item_id: Option<String>,
    pub affected_track_id: Option<String>,
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
    #[serde(default)]
    pub client_registry: ClientRegistry,
    #[serde(default)]
    pub command_ledger: Vec<CommandLedgerEntry>,
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
            client_registry: ClientRegistry::default(),
            command_ledger: Vec::new(),
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

        // Validate client registry capacity
        if self.client_registry.max_active_clients > 0
            && self.client_registry.clients.len() > self.client_registry.max_active_clients
        {
            return Err(DomainError::Validation(format!(
                "Client registry capacity exceeded: {} > {}",
                self.client_registry.clients.len(),
                self.client_registry.max_active_clients
            )));
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

    pub fn find_transition(&self, transition_id: &str) -> Option<(&Track, &CutTransition)> {
        for track in &self.tracks {
            if let Some(trans) = track.find_transition(transition_id) {
                return Some((track, trans));
            }
        }
        None
    }

    pub fn find_transition_mut(&mut self, transition_id: &str) -> Option<&mut CutTransition> {
        for track in &mut self.tracks {
            if let Some(trans) = track.find_transition_mut(transition_id) {
                return Some(trans);
            }
        }
        None
    }

    pub fn find_asset(&self, asset_id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.asset_id == asset_id)
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
        assert_eq!(project.schema_version, CURRENT_SCHEMA_VERSION);
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

    #[test]
    fn test_phase13_cut_transition_validation_rules() {
        let rate = FrameRate::FPS_30;
        let mut track = Track::new("Video Track 1".to_string(), TrackKind::Video);

        // Clip 1: [0, 30)
        let t1 = TimeRange::new(
            RationalTime::from_frames(0, rate),
            RationalTime::from_frames(30, rate),
        );
        let item1 = TimelineItem::new(None, "Clip 1".to_string(), ItemKind::Video, t1, t1).unwrap();
        let item1_id = item1.item_id.clone();

        // Clip 2: [30, 60) (immediate adjacent at cut C = 30)
        let t2 = TimeRange::new(
            RationalTime::from_frames(30, rate),
            RationalTime::from_frames(30, rate),
        );
        let item2 = TimelineItem::new(None, "Clip 2".to_string(), ItemKind::Video, t2, t2).unwrap();
        let item2_id = item2.item_id.clone();

        track.items.push(item1);
        track.items.push(item2);

        // 1. Valid CutTransition of 10 frames (L=5, R=5, within editorial extents 30 and 30)
        let ct_valid = CutTransition::new(
            track.track_id.clone(),
            item1_id.clone(),
            item2_id.clone(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_valid);
        assert!(track.validate().is_ok());

        // 2. Duplicate transition at same cut rejected (CD-07)
        let ct_dup = CutTransition::new(
            track.track_id.clone(),
            item1_id.clone(),
            item2_id.clone(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_dup);
        assert!(matches!(
            track.validate(),
            Err(DomainError::DuplicateCutTransition { .. })
        ));
        track.transitions.pop();

        // 3. Non-video item rejected (CD-04)
        let t3 = TimeRange::new(
            RationalTime::from_frames(60, rate),
            RationalTime::from_frames(30, rate),
        );
        let audio_item =
            TimelineItem::new(None, "Audio Clip".to_string(), ItemKind::Audio, t3, t3).unwrap();
        let audio_id = audio_item.item_id.clone();
        track.items.push(audio_item);

        let ct_audio = CutTransition::new(
            track.track_id.clone(),
            item2_id.clone(),
            audio_id,
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_audio);
        assert_eq!(
            track.validate(),
            Err(DomainError::UnsupportedTransitionMedia)
        );
        track.transitions.pop();
        track.items.pop();

        // 4. Non-adjacent items rejected (CD-02)
        let t_gap = TimeRange::new(
            RationalTime::from_frames(100, rate),
            RationalTime::from_frames(30, rate),
        );
        let item_gap =
            TimelineItem::new(None, "Gap Clip".to_string(), ItemKind::Video, t_gap, t_gap).unwrap();
        let gap_id = item_gap.item_id.clone();
        track.items.push(item_gap);

        let ct_gap = CutTransition::new(
            track.track_id.clone(),
            item2_id.clone(),
            gap_id,
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_gap);
        assert!(matches!(
            track.validate(),
            Err(DomainError::NonAdjacentTransitionParticipants { .. })
        ));
        track.transitions.pop();
        track.items.pop();

        // 5. Exceeding editorial extent rejected (CD-21)
        // Item 1 has 30 frames. If D = 70, L = 35 > 30 extent -> rejected!
        let ct_too_long = CutTransition::new(
            track.track_id.clone(),
            item1_id.clone(),
            item2_id.clone(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(70, rate),
        )
        .unwrap();
        track.transitions.push(ct_too_long);
        assert!(matches!(track.validate(), Err(DomainError::Validation(_))));
        track.transitions.pop();

        // 6. Dangling item reference rejected
        let ct_dangling = CutTransition::new(
            track.track_id.clone(),
            item1_id,
            "clip_nonexistent".to_string(),
            TransitionKind::CrossDissolve,
            RationalTime::from_frames(10, rate),
        )
        .unwrap();
        track.transitions.push(ct_dangling);
        assert!(matches!(
            track.validate(),
            Err(DomainError::ItemNotFound(_))
        ));
    }
}

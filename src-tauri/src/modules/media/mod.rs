//! Media Catalog, Asset Identity, Offline State, and Relink
//! Architecture Invariant: Missing media does not delete editorial state.
//! Clips remain in timeline with offline status until relinked.

pub mod cache;
pub mod captions;
pub mod ffmpeg;
pub mod qualification;
pub mod resolver;
pub mod text_overlay;
pub mod text_renderer_poc;
pub mod transcription;
pub mod waveform;

pub use qualification::{Phase12QualificationHarness, Poc5MatrixReport};
pub use resolver::{BinaryKind, BinaryOrigin, MediaBinaryResolver, ResolvedBinary};

use crate::modules::domain::time::RationalTime;
use crate::modules::domain::{Asset, AssetDimensions, MediaKind, Project};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("Asset with ID '{0}' not found")]
    AssetNotFound(String),

    #[error("Relink target file does not exist: {0}")]
    TargetNotFound(PathBuf),

    #[error("Relink verification failed: {0}")]
    VerificationFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Bundled media binary missing or inaccessible: {0}")]
    BundledBinaryMissing(String),

    #[error("Media binary resolution failed: {0}")]
    BinaryResolutionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetOnlineStatus {
    Online,
    Offline { last_known_path: String },
}

pub struct MediaCatalog;

impl MediaCatalog {
    /// Check whether an asset is currently reachable on disk.
    pub fn check_asset_status(asset: &Asset) -> AssetOnlineStatus {
        let path = Path::new(&asset.path);
        if path.exists() && path.is_file() {
            AssetOnlineStatus::Online
        } else {
            AssetOnlineStatus::Offline {
                last_known_path: asset.path.clone(),
            }
        }
    }

    /// Register a new asset into the project.
    pub fn register_asset(
        project: &mut Project,
        asset_id: Option<String>,
        name: String,
        path: String,
        media_kind: MediaKind,
        duration: Option<RationalTime>,
        file_size: u64,
        dimensions: Option<AssetDimensions>,
    ) -> String {
        let id = asset_id.unwrap_or_else(crate::modules::domain::id::generate_asset_id);
        let asset = Asset {
            asset_id: id.clone(),
            name,
            path,
            media_kind,
            duration,
            file_size,
            dimensions,
        };
        project.assets.push(asset);
        id
    }

    /// Relink an existing asset to a new file location.
    /// Invariant: All timeline clips referencing this asset remain completely intact!
    pub fn relink_asset(
        project: &mut Project,
        asset_id: &str,
        new_path: &Path,
    ) -> Result<bool, MediaError> {
        if !new_path.exists() || !new_path.is_file() {
            return Err(MediaError::TargetNotFound(new_path.to_path_buf()));
        }

        let asset = project
            .assets
            .iter_mut()
            .find(|a| a.asset_id == asset_id)
            .ok_or_else(|| MediaError::AssetNotFound(asset_id.to_string()))?;

        // Update path to new location
        asset.path = new_path.to_string_lossy().to_string();
        if let Some(file_name) = new_path.file_name() {
            asset.name = file_name.to_string_lossy().to_string();
        }

        if let Ok(metadata) = new_path.metadata() {
            asset.file_size = metadata.len();
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::domain::time::{FrameRate, RationalTime, TimeRange};
    use crate::modules::domain::{ItemKind, TimelineItem, Track, TrackKind};

    #[test]
    fn test_offline_media_preserves_timeline_clips() {
        let mut project = Project::new("Media Relink Test".to_string());
        let missing_path = "C:/non_existent_folder/sample_video.mp4";

        let asset_id = MediaCatalog::register_asset(
            &mut project,
            Some("ast_sample".to_string()),
            "sample_video.mp4".to_string(),
            missing_path.to_string(),
            MediaKind::Video,
            None,
            1024 * 1024 * 50,
            None,
        );

        // Asset is offline
        let status = MediaCatalog::check_asset_status(&project.assets[0]);
        assert_eq!(
            status,
            AssetOnlineStatus::Offline {
                last_known_path: missing_path.to_string()
            }
        );

        // Add track and clip referencing this offline asset
        let mut track = Track::new("Video Track".to_string(), TrackKind::Video);
        let t_range = TimeRange::new(
            RationalTime::zero(FrameRate::FPS_30),
            RationalTime::from_frames(90, FrameRate::FPS_30),
        );
        let s_range = t_range;

        let clip = TimelineItem::new(
            Some(asset_id.clone()),
            "Sample Clip".to_string(),
            ItemKind::Video,
            t_range,
            s_range,
        )
        .unwrap();

        track.items.push(clip);
        project.tracks.push(track);

        // Invariant: Project validation succeeds even though media is offline!
        assert!(project.validate().is_ok());

        // Verify clip is intact on timeline
        let item = &project.tracks[0].items[0];
        assert_eq!(item.asset_id, Some("ast_sample".to_string()));
        assert_eq!(item.timeline_range.duration.value, 90);
    }
}

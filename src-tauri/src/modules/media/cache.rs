//! Rebuildable Cache Architecture
//! Architecture Invariant: Deleting cache must never corrupt or invalidate project correctness.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CacheCategoryStatus {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaCacheStatus {
    pub total_files: u64,
    pub total_bytes: u64,
    pub categories: BTreeMap<String, CacheCategoryStatus>,
}

/// Bump when proxy encoding changes so older cache entries are not reused.
pub const PREVIEW_PROXY_PROFILE: &str = "seek05";

pub struct CacheManager {
    root_dir: PathBuf,
}

impl CacheManager {
    pub fn new(root_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(root_dir.join("thumbnails"));
        let _ = fs::create_dir_all(root_dir.join("proxies"));
        let _ = fs::create_dir_all(root_dir.join("waveforms"));
        let _ = fs::create_dir_all(root_dir.join("transcriptions"));
        let _ = fs::create_dir_all(root_dir.join("scene-analysis"));
        let _ = fs::create_dir_all(root_dir.join("visual-scenes"));
        Self { root_dir }
    }

    pub fn thumbnail_path(&self, asset_id: &str, timestamp_secs: f64) -> PathBuf {
        let ts_key = (timestamp_secs * 1000.0).round() as u64;
        self.root_dir
            .join("thumbnails")
            .join(format!("{}_{}ms.jpg", asset_id, ts_key))
    }

    /// Proxy cache entries are keyed by source fingerprint and encoding profile, so a
    /// relinked/replaced source or a changed encoder setting never reuses a stale proxy.
    pub fn proxy_path(
        &self,
        asset_id: &str,
        source_fingerprint: &str,
        target_height: u32,
    ) -> PathBuf {
        let fingerprint: String = source_fingerprint.chars().take(16).collect();
        self.root_dir.join("proxies").join(format!(
            "{}_{}_{}p_{}.mp4",
            asset_id, fingerprint, target_height, PREVIEW_PROXY_PROFILE
        ))
    }

    /// Reversed preview of one source range (derived cache; the project never references it).
    pub fn reverse_proxy_path(
        &self,
        asset_id: &str,
        source_fingerprint: &str,
        start_micros: i64,
        duration_micros: i64,
        target_height: u32,
    ) -> PathBuf {
        let fingerprint: String = source_fingerprint.chars().take(16).collect();
        self.root_dir.join("proxies").join(format!(
            "{asset_id}_{fingerprint}_rev_{start_micros}_{duration_micros}_{target_height}p.mp4"
        ))
    }

    pub fn waveform_path(&self, asset_id: &str, source_fingerprint: &str) -> PathBuf {
        self.root_dir
            .join("waveforms")
            .join(format!("{}_{}_waveform.json", asset_id, source_fingerprint))
    }

    pub fn transcription_audio_path(&self, job_id: &str) -> PathBuf {
        self.root_dir
            .join("transcriptions")
            .join(format!("{job_id}.wav"))
    }

    pub fn transcript_analysis_path(
        &self,
        source_fingerprint: &str,
        config_fingerprint: &str,
    ) -> PathBuf {
        self.root_dir.join("transcriptions").join(format!(
            "{}_{}.json",
            source_fingerprint, config_fingerprint
        ))
    }

    pub fn load_transcript_analysis(
        &self,
        source_fingerprint: &str,
        config_fingerprint: &str,
    ) -> Option<crate::modules::media::transcription::TranscriptAnalysisReport> {
        let report: crate::modules::media::transcription::TranscriptAnalysisReport =
            serde_json::from_slice(
                &fs::read(self.transcript_analysis_path(source_fingerprint, config_fingerprint))
                    .ok()?,
            )
            .ok()?;
        (report.schema_version
            == crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_SCHEMA_VERSION
            && report.source_fingerprint == source_fingerprint
            && report.config_fingerprint == config_fingerprint)
            .then_some(report)
    }

    pub fn save_transcript_analysis(
        &self,
        report: &crate::modules::media::transcription::TranscriptAnalysisReport,
    ) -> std::io::Result<()> {
        let path =
            self.transcript_analysis_path(&report.source_fingerprint, &report.config_fingerprint);
        let bytes = serde_json::to_vec(report).map_err(std::io::Error::other)?;
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
        fs::write(&temp, bytes)?;
        match fs::rename(&temp, &path) {
            Ok(()) => Ok(()),
            Err(_) if path.exists() => {
                let _ = fs::remove_file(temp);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(temp);
                Err(error)
            }
        }
    }

    pub fn scene_analysis_path(
        &self,
        source_fingerprint: &str,
        config_fingerprint: &str,
    ) -> PathBuf {
        self.root_dir.join("scene-analysis").join(format!(
            "{}_{}.json",
            source_fingerprint, config_fingerprint
        ))
    }

    pub fn load_scene_analysis(
        &self,
        source_fingerprint: &str,
        config_fingerprint: &str,
    ) -> Option<crate::modules::media::scene::SceneAnalysisReport> {
        let path = self.scene_analysis_path(source_fingerprint, config_fingerprint);
        let bytes = fs::read(path).ok()?;
        let report: crate::modules::media::scene::SceneAnalysisReport =
            serde_json::from_slice(&bytes).ok()?;
        (report.schema_version == crate::modules::media::scene::SCENE_ANALYSIS_SCHEMA_VERSION
            && report.source_fingerprint == source_fingerprint
            && report.config_fingerprint == config_fingerprint)
            .then_some(report)
    }

    pub fn save_scene_analysis(
        &self,
        report: &crate::modules::media::scene::SceneAnalysisReport,
    ) -> std::io::Result<()> {
        let path = self.scene_analysis_path(&report.source_fingerprint, &report.config_fingerprint);
        let bytes = serde_json::to_vec(report).map_err(std::io::Error::other)?;
        let temp = path.with_file_name(format!(
            "{}.{}.tmp",
            path.file_name().unwrap_or_default().to_string_lossy(),
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&temp, bytes)?;
        match fs::rename(&temp, &path) {
            Ok(()) => Ok(()),
            Err(_) if path.exists() => {
                let _ = fs::remove_file(temp);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(temp);
                Err(error)
            }
        }
    }

    pub fn visual_scene_analysis_path(
        &self,
        source_fingerprint: &str,
        config_fingerprint: &str,
    ) -> PathBuf {
        self.root_dir.join("visual-scenes").join(format!(
            "{}_{}.json",
            source_fingerprint, config_fingerprint
        ))
    }

    pub fn load_visual_scene_analysis(
        &self,
        source_fingerprint: &str,
        config_fingerprint: &str,
    ) -> Option<crate::modules::media::visual_scene::VisualSceneAnalysisReport> {
        let report: crate::modules::media::visual_scene::VisualSceneAnalysisReport =
            serde_json::from_slice(
                &fs::read(self.visual_scene_analysis_path(source_fingerprint, config_fingerprint))
                    .ok()?,
            )
            .ok()?;
        (report.schema_version == crate::modules::media::visual_scene::VISUAL_SCENE_SCHEMA_VERSION
            && report.source_fingerprint == source_fingerprint
            && report.config_fingerprint == config_fingerprint)
            .then_some(report)
    }

    pub fn save_visual_scene_analysis(
        &self,
        report: &crate::modules::media::visual_scene::VisualSceneAnalysisReport,
    ) -> std::io::Result<()> {
        let path =
            self.visual_scene_analysis_path(&report.source_fingerprint, &report.config_fingerprint);
        fs::create_dir_all(path.parent().unwrap_or(&self.root_dir))?;
        let bytes = serde_json::to_vec(report).map_err(std::io::Error::other)?;
        let temp = path.with_file_name(format!(
            "{}.{}.tmp",
            path.file_name().unwrap_or_default().to_string_lossy(),
            uuid::Uuid::new_v4().simple()
        ));
        fs::write(&temp, bytes)?;
        match fs::rename(&temp, &path) {
            Ok(()) => Ok(()),
            Err(_) if path.exists() => {
                let _ = fs::remove_file(temp);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(temp);
                Err(error)
            }
        }
    }

    pub fn has_thumbnail(&self, asset_id: &str, timestamp_secs: f64) -> bool {
        self.thumbnail_path(asset_id, timestamp_secs).exists()
    }

    pub fn has_waveform(&self, asset_id: &str, source_fingerprint: &str) -> bool {
        self.waveform_path(asset_id, source_fingerprint).exists()
    }

    pub fn load_waveform(
        &self,
        asset_id: &str,
        source_fingerprint: &str,
    ) -> Option<crate::modules::media::waveform::WaveformData> {
        let path = self.waveform_path(asset_id, source_fingerprint);
        if !path.exists() {
            return None;
        }
        let data = fs::read_to_string(&path).ok()?;
        let waveform: crate::modules::media::waveform::WaveformData =
            serde_json::from_str(&data).ok()?;
        (waveform.schema_version == crate::modules::media::waveform::WAVEFORM_SCHEMA_VERSION
            && waveform.source_fingerprint == source_fingerprint)
            .then_some(waveform)
    }

    pub fn has_proxy(&self, asset_id: &str, source_fingerprint: &str, target_height: u32) -> bool {
        self.proxy_path(asset_id, source_fingerprint, target_height)
            .is_file()
    }

    pub fn status(&self) -> std::io::Result<MediaCacheStatus> {
        let mut status = MediaCacheStatus::default();
        let categories = [
            "thumbnails",
            "proxies",
            "waveforms",
            "transcriptions",
            "scene-analysis",
            "visual-scenes",
        ];
        match fs::symlink_metadata(&self.root_dir) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Ok(status),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                for category in categories {
                    status
                        .categories
                        .insert(category.to_string(), CacheCategoryStatus::default());
                }
                return Ok(status);
            }
            Err(error) => return Err(error),
        }
        for category in categories {
            let path = self.root_dir.join(category);
            let mut category_status = CacheCategoryStatus::default();
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                    collect_cache_files(&path, &mut category_status)?;
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            status.total_files += category_status.files;
            status.total_bytes += category_status.bytes;
            status
                .categories
                .insert(category.to_string(), category_status);
        }
        Ok(status)
    }

    pub fn clear_cache(&self) -> std::io::Result<()> {
        match fs::symlink_metadata(&self.root_dir) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "media cache root is not a real directory",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }

        for category in [
            "thumbnails",
            "proxies",
            "waveforms",
            "transcriptions",
            "scene-analysis",
            "visual-scenes",
        ] {
            let path = self.root_dir.join(category);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                    fs::remove_dir_all(&path)?;
                }
                Ok(_) => fs::remove_file(&path)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            }
            fs::create_dir(&path)?;
        }
        Ok(())
    }
}

fn collect_cache_files(
    path: &std::path::Path,
    status: &mut CacheCategoryStatus,
) -> std::io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        // Do not follow symlinks when accounting for or later clearing disposable cache data.
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_cache_files(&entry.path(), status)?;
        } else if file_type.is_file() {
            status.files += 1;
            status.bytes += entry.metadata()?.len();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::media::waveform::{WaveformData, WAVEFORM_SCHEMA_VERSION};

    #[test]
    fn waveform_cache_is_isolated_by_source_fingerprint_and_schema() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-waveform-cache-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager::new(root.clone());
        let fingerprint = "source-a";
        let data = WaveformData {
            schema_version: WAVEFORM_SCHEMA_VERSION,
            asset_id: "asset-1".to_string(),
            source_fingerprint: fingerprint.to_string(),
            source_duration: 1.0,
            bucket_rate: 50,
            channels: 1,
            peaks: vec![0.0, 0.5],
        };
        let path = cache.waveform_path("asset-1", fingerprint);
        std::fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();

        assert_eq!(
            cache.load_waveform("asset-1", fingerprint),
            Some(data.clone())
        );
        assert!(cache.load_waveform("asset-1", "source-b").is_none());

        let mut stale_schema = serde_json::to_value(data).unwrap();
        stale_schema["schema_version"] = serde_json::json!(WAVEFORM_SCHEMA_VERSION - 1);
        std::fs::write(&path, serde_json::to_vec(&stale_schema).unwrap()).unwrap();
        assert!(cache.load_waveform("asset-1", fingerprint).is_none());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cache_status_counts_only_regular_files_in_known_derived_categories() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-cache-status-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager::new(root.clone());
        fs::write(cache.proxy_path("asset-a", "source-a", 360), b"proxy-bytes").unwrap();
        fs::write(cache.waveform_path("asset-a", "source-a"), b"waveform").unwrap();

        let status = cache.status().unwrap();
        assert_eq!(status.total_files, 2);
        assert_eq!(status.total_bytes, 19);
        assert_eq!(status.categories["proxies"].files, 1);
        assert_eq!(status.categories["proxies"].bytes, 11);
        assert_eq!(status.categories["waveforms"].files, 1);
        assert_eq!(status.categories["waveforms"].bytes, 8);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn proxy_cache_is_isolated_by_source_fingerprint_and_profile() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-proxy-key-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager::new(root.clone());
        fs::write(cache.proxy_path("asset-a", "source-a", 720), b"proxy").unwrap();

        assert!(cache.has_proxy("asset-a", "source-a", 720));
        // A relinked or replaced source must not reuse the old proxy.
        assert!(!cache.has_proxy("asset-a", "source-b", 720));
        assert!(cache
            .proxy_path("asset-a", "source-a", 720)
            .to_string_lossy()
            .contains(PREVIEW_PROXY_PROFILE));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cache_status_for_missing_root_is_empty() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-cache-missing-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager { root_dir: root };
        let status = cache.status().unwrap();
        assert_eq!(status.total_files, 0);
        assert_eq!(status.total_bytes, 0);
        assert_eq!(status.categories.len(), 6);
    }

    #[test]
    fn clearing_cache_preserves_unmanaged_root_files() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-cache-clear-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager::new(root.clone());
        fs::write(cache.proxy_path("asset-a", "source-a", 360), b"derived").unwrap();
        let unmanaged = root.join("keep.txt");
        fs::write(&unmanaged, b"keep").unwrap();

        cache.clear_cache().unwrap();

        assert!(unmanaged.is_file());
        assert_eq!(cache.status().unwrap().total_files, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scene_analysis_cache_is_isolated_by_source_and_algorithm_config() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-scene-cache-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager::new(root.clone());
        let report = crate::modules::media::scene::SceneAnalysisReport {
            schema_version: crate::modules::media::scene::SCENE_ANALYSIS_SCHEMA_VERSION,
            algorithm_version: crate::modules::media::scene::SCENE_ANALYSIS_ALGORITHM_VERSION
                .to_string(),
            source_fingerprint: "source-a".to_string(),
            config_fingerprint: "config-a".to_string(),
            threshold: 0.3,
            analyzed_duration_micros: 2_000_000,
            truncated: false,
            candidates: vec![crate::modules::media::scene::SceneCandidate {
                source_time_micros: 1_000_000,
                score: 0.4,
            }],
        };

        cache.save_scene_analysis(&report).unwrap();
        assert_eq!(
            cache.load_scene_analysis("source-a", "config-a"),
            Some(report)
        );
        assert!(cache.load_scene_analysis("source-b", "config-a").is_none());
        assert!(cache.load_scene_analysis("source-a", "config-b").is_none());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transcript_analysis_cache_is_isolated_by_source_and_model_config() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-transcript-cache-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager::new(root.clone());
        let report = crate::modules::media::transcription::TranscriptAnalysisReport {
            schema_version:
                crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_SCHEMA_VERSION,
            algorithm_version:
                crate::modules::media::transcription::TRANSCRIPT_ANALYSIS_ALGORITHM_VERSION
                    .to_string(),
            source_fingerprint: "source-a".to_string(),
            config_fingerprint: "model-and-language-a".to_string(),
            analyzed_duration_micros: 1_000_000,
            result: crate::modules::media::transcription::TranscriptionResult {
                language: "en".to_string(),
                segments: vec![crate::modules::media::transcription::TranscriptionSegment {
                    start: crate::modules::domain::time::RationalTime::new(
                        0,
                        crate::modules::domain::time::FrameRate::new(1_000, 1),
                    ),
                    duration: crate::modules::domain::time::RationalTime::new(
                        500,
                        crate::modules::domain::time::FrameRate::new(1_000, 1),
                    ),
                    text: "fixture transcript".to_string(),
                }],
            },
        };
        cache.save_transcript_analysis(&report).unwrap();
        assert_eq!(
            cache.load_transcript_analysis("source-a", "model-and-language-a"),
            Some(report.clone())
        );
        assert!(cache
            .load_transcript_analysis("source-b", "model-and-language-a")
            .is_none());
        assert!(cache
            .load_transcript_analysis("source-a", "model-and-language-b")
            .is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn visual_scene_cache_is_isolated_by_source_and_query_config() {
        let root = std::env::temp_dir().join(format!(
            "tinixcut-visual-scene-cache-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let cache = CacheManager::new(root.clone());
        let report = crate::modules::media::visual_scene::VisualSceneAnalysisReport {
            schema_version: crate::modules::media::visual_scene::VISUAL_SCENE_SCHEMA_VERSION,
            algorithm_version: crate::modules::media::visual_scene::VISUAL_SCENE_ALGORITHM_VERSION
                .to_string(),
            provider: "lm-studio-local".to_string(),
            model: "google/gemma-4-e4b".to_string(),
            query: "beach".to_string(),
            source_fingerprint: "source-a".to_string(),
            config_fingerprint: "query-config-a".to_string(),
            truncated: false,
            scenes: vec![
                crate::modules::media::visual_scene::VisualSceneDescription {
                    scene_id: "scene-01".to_string(),
                    asset_id: "asset-1".to_string(),
                    asset_name: "fixture.mp4".to_string(),
                    source_start_micros: 0,
                    source_end_micros: 1_000_000,
                    sampled_at_micros: 500_000,
                    caption: "A beach at sunset".to_string(),
                    tags: vec!["beach".to_string()],
                    relevance_score: 0.8,
                    rationale: "Sand and water are visible".to_string(),
                },
            ],
            warnings: vec![],
        };
        cache.save_visual_scene_analysis(&report).unwrap();
        assert_eq!(
            cache.load_visual_scene_analysis("source-a", "query-config-a"),
            Some(report)
        );
        assert!(cache
            .load_visual_scene_analysis("source-b", "query-config-a")
            .is_none());
        assert!(cache
            .load_visual_scene_analysis("source-a", "query-config-b")
            .is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}

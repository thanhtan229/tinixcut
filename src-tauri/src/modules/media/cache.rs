//! Rebuildable Cache Architecture
//! Architecture Invariant: Deleting cache must never corrupt or invalidate project correctness.

use std::fs;
use std::path::PathBuf;

pub struct CacheManager {
    root_dir: PathBuf,
}

impl CacheManager {
    pub fn new(root_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(root_dir.join("thumbnails"));
        let _ = fs::create_dir_all(root_dir.join("proxies"));
        let _ = fs::create_dir_all(root_dir.join("waveforms"));
        Self { root_dir }
    }

    pub fn thumbnail_path(&self, asset_id: &str, timestamp_secs: f64) -> PathBuf {
        let ts_key = (timestamp_secs * 1000.0).round() as u64;
        self.root_dir
            .join("thumbnails")
            .join(format!("{}_{}ms.jpg", asset_id, ts_key))
    }

    pub fn proxy_path(&self, asset_id: &str, target_height: u32) -> PathBuf {
        self.root_dir
            .join("proxies")
            .join(format!("{}_{}p_proxy.mp4", asset_id, target_height))
    }

    pub fn waveform_path(&self, asset_id: &str) -> PathBuf {
        self.root_dir
            .join("waveforms")
            .join(format!("{}_waveform.json", asset_id))
    }

    pub fn has_thumbnail(&self, asset_id: &str, timestamp_secs: f64) -> bool {
        self.thumbnail_path(asset_id, timestamp_secs).exists()
    }

    pub fn has_proxy(&self, asset_id: &str, target_height: u32) -> bool {
        self.proxy_path(asset_id, target_height).exists()
    }

    pub fn clear_cache(&self) -> std::io::Result<()> {
        if self.root_dir.exists() {
            fs::remove_dir_all(&self.root_dir)?;
            fs::create_dir_all(self.root_dir.join("thumbnails"))?;
            fs::create_dir_all(self.root_dir.join("proxies"))?;
            fs::create_dir_all(self.root_dir.join("waveforms"))?;
        }
        Ok(())
    }
}

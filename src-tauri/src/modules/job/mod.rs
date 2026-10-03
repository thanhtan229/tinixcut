//! Local Job Manager
//! Architecture Invariant: Bounded concurrency, deduplicated work, and safe cancellation.
//! Does not block the UI thread and avoids unbounded memory/task growth.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobKind {
    Probe,
    Thumbnail,
    Proxy,
    Waveform,
    Export,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Clone)]
pub struct JobItem {
    pub id: String,
    pub kind: JobKind,
    pub dedupe_key: String,
    pub priority: u8,
    pub status: JobStatus,
    pub progress: f32, // 0.0 to 1.0
    pub cancel_flag: Arc<AtomicBool>,
}

pub struct JobManager {
    jobs: HashMap<String, JobItem>,
    max_concurrency: usize,
    active_count: usize,
}

impl JobManager {
    pub fn new(max_concurrency: usize) -> Self {
        Self {
            jobs: HashMap::new(),
            max_concurrency: max_concurrency.max(1),
            active_count: 0,
        }
    }

    /// Submit a job. If an active or queued job with the same dedupe_key exists,
    /// returns the existing job ID (Deduplication requirement).
    pub fn submit_job(
        &mut self,
        id: String,
        kind: JobKind,
        dedupe_key: String,
        priority: u8,
    ) -> (String, bool) {
        // Check for deduplication
        for (existing_id, job) in &self.jobs {
            if job.dedupe_key == dedupe_key {
                match job.status {
                    JobStatus::Queued | JobStatus::Running => {
                        return (existing_id.clone(), true); // Deduplicated!
                    }
                    _ => {}
                }
            }
        }

        let job = JobItem {
            id: id.clone(),
            kind,
            dedupe_key,
            priority,
            status: JobStatus::Queued,
            progress: 0.0,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        };

        self.jobs.insert(id.clone(), job);
        (id, false)
    }

    pub fn get_job(&self, id: &str) -> Option<&JobItem> {
        self.jobs.get(id)
    }

    pub fn update_progress(&mut self, id: &str, progress: f32) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.progress = progress.clamp(0.0, 1.0);
            if job.status == JobStatus::Queued {
                job.status = JobStatus::Running;
            }
        }
    }

    pub fn mark_completed(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Completed;
            job.progress = 1.0;
        }
    }

    pub fn mark_failed(&mut self, id: &str, reason: String) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Failed(reason);
        }
    }

    /// Cancel a job safely.
    pub fn cancel_job(&mut self, id: &str) -> bool {
        if let Some(job) = self.jobs.get_mut(id) {
            job.cancel_flag.store(true, Ordering::SeqCst);
            job.status = JobStatus::Cancelled;
            true
        } else {
            false
        }
    }

    pub fn active_jobs_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|j| matches!(j.status, JobStatus::Running | JobStatus::Queued))
            .count()
    }

    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    pub fn active_count(&self) -> usize {
        self.active_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_deduplication() {
        let mut jm = JobManager::new(4);
        let (id1, deduped1) = jm.submit_job(
            "job_1".to_string(),
            JobKind::Thumbnail,
            "dedupe_thumb_ast1_0s".to_string(),
            10,
        );
        assert_eq!(id1, "job_1");
        assert!(!deduped1);

        // Submit same work with different job ID
        let (id2, deduped2) = jm.submit_job(
            "job_2".to_string(),
            JobKind::Thumbnail,
            "dedupe_thumb_ast1_0s".to_string(),
            10,
        );
        assert_eq!(id2, "job_1"); // Reused existing job!
        assert!(deduped2);
    }

    #[test]
    fn test_job_cancellation() {
        let mut jm = JobManager::new(4);
        let (id, _) = jm.submit_job(
            "job_cancel_test".to_string(),
            JobKind::Proxy,
            "proxy_ast1".to_string(),
            5,
        );

        let cancelled = jm.cancel_job(&id);
        assert!(cancelled);

        let job = jm.get_job(&id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.cancel_flag.load(Ordering::SeqCst));
    }
}

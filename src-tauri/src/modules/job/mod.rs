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
    Transcription,
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

    pub fn running_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|j| matches!(j.status, JobStatus::Running))
            .count()
    }

    pub fn queued_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|j| matches!(j.status, JobStatus::Queued))
            .count()
    }

    pub fn can_start(&self) -> bool {
        self.running_count() < self.max_concurrency
    }

    pub fn has_active_kind(&self, kind: JobKind) -> bool {
        self.jobs.values().any(|job| {
            job.kind == kind && matches!(job.status, JobStatus::Queued | JobStatus::Running)
        })
    }

    /// Transitions a Queued job to Running if concurrency permits, returning its cancel flag
    pub fn start_job(&mut self, id: &str) -> Result<Arc<AtomicBool>, String> {
        if !self.can_start() {
            return Err(format!(
                "Max concurrency limit ({}) reached",
                self.max_concurrency
            ));
        }
        let cancel_flag = {
            let job = self
                .jobs
                .get_mut(id)
                .ok_or_else(|| format!("Job '{}' not found", id))?;
            if job.status == JobStatus::Cancelled {
                return Err("Cannot start cancelled job".to_string());
            }
            job.status = JobStatus::Running;
            job.cancel_flag.clone()
        };
        self.active_count = self.running_count();
        Ok(cancel_flag)
    }

    pub fn update_progress(&mut self, id: &str, progress: f32) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.progress = progress.clamp(0.0, 1.0);
            if job.status == JobStatus::Queued {
                job.status = JobStatus::Running;
                self.active_count = self.running_count();
            }
        }
    }

    pub fn mark_completed(&mut self, id: &str) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Completed;
            job.progress = 1.0;
            self.active_count = self.running_count();
        }
    }

    pub fn mark_failed(&mut self, id: &str, reason: String) {
        if let Some(job) = self.jobs.get_mut(id) {
            job.status = JobStatus::Failed(reason);
            self.active_count = self.running_count();
        }
    }

    /// Cancel a job safely.
    pub fn cancel_job(&mut self, id: &str) -> bool {
        if let Some(job) = self.jobs.get_mut(id) {
            job.cancel_flag.store(true, Ordering::SeqCst);
            job.status = JobStatus::Cancelled;
            self.active_count = self.running_count();
            true
        } else {
            false
        }
    }

    /// Cancel all currently active or queued jobs.
    pub fn cancel_all_active(&mut self) -> usize {
        let active_ids: Vec<String> = self
            .jobs
            .iter()
            .filter(|(_, j)| matches!(j.status, JobStatus::Running | JobStatus::Queued))
            .map(|(id, _)| id.clone())
            .collect();
        let count = active_ids.len();
        for id in active_ids {
            self.cancel_job(&id);
        }
        count
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
    fn test_job_concurrency_bound() {
        let mut jm = JobManager::new(2);
        let (id1, _) = jm.submit_job(
            "job_wf_1".to_string(),
            JobKind::Waveform,
            "wf_ast1".to_string(),
            10,
        );
        let (id2, _) = jm.submit_job(
            "job_wf_2".to_string(),
            JobKind::Waveform,
            "wf_ast2".to_string(),
            10,
        );
        let (id3, _) = jm.submit_job(
            "job_wf_3".to_string(),
            JobKind::Waveform,
            "wf_ast3".to_string(),
            10,
        );

        assert_eq!(jm.queued_count(), 3);
        assert_eq!(jm.running_count(), 0);

        // Start job 1
        assert!(jm.start_job(&id1).is_ok());
        assert_eq!(jm.running_count(), 1);

        // Start job 2
        assert!(jm.start_job(&id2).is_ok());
        assert_eq!(jm.running_count(), 2);
        assert!(!jm.can_start());

        // Attempting to start job 3 exceeds limit of 2
        let err = jm.start_job(&id3);
        assert!(err.is_err());
        assert_eq!(jm.running_count(), 2);

        // Complete job 1
        jm.mark_completed(&id1);
        assert_eq!(jm.running_count(), 1);
        assert!(jm.can_start());

        // Now job 3 can start
        assert!(jm.start_job(&id3).is_ok());
        assert_eq!(jm.running_count(), 2);
    }

    #[test]
    fn test_waveform_job_lifecycle_and_cancellation() {
        let mut jm = JobManager::new(4);
        let (id, deduped) = jm.submit_job(
            "job_wf_cancel".to_string(),
            JobKind::Waveform,
            "wf_cancel_key".to_string(),
            10,
        );
        assert!(!deduped);

        let cancel_flag = jm.start_job(&id).expect("job should start");
        assert_eq!(jm.get_job(&id).unwrap().status, JobStatus::Running);
        assert!(!cancel_flag.load(Ordering::SeqCst));

        // Update progress
        jm.update_progress(&id, 0.45);
        assert_eq!(jm.get_job(&id).unwrap().progress, 0.45);

        // Cancel job
        let cancelled = jm.cancel_job(&id);
        assert!(cancelled);
        assert_eq!(jm.get_job(&id).unwrap().status, JobStatus::Cancelled);
        assert!(cancel_flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_waveform_job_failure_state() {
        let mut jm = JobManager::new(4);
        let (id, _) = jm.submit_job(
            "job_wf_fail".to_string(),
            JobKind::Waveform,
            "wf_fail_key".to_string(),
            10,
        );
        let _ = jm.start_job(&id);
        jm.mark_failed(&id, "No audio stream detected".to_string());

        let job = jm.get_job(&id).unwrap();
        assert_eq!(
            job.status,
            JobStatus::Failed("No audio stream detected".to_string())
        );
    }

    #[test]
    fn completed_work_can_be_requested_again() {
        let mut jm = JobManager::new(1);
        let (first_id, _) = jm.submit_job(
            "job_caption_first".to_string(),
            JobKind::Transcription,
            "caption_clip_a".to_string(),
            10,
        );
        jm.start_job(&first_id).unwrap();
        jm.mark_completed(&first_id);

        let (second_id, deduped) = jm.submit_job(
            "job_caption_second".to_string(),
            JobKind::Transcription,
            "caption_clip_a".to_string(),
            10,
        );
        assert_eq!(second_id, "job_caption_second");
        assert!(!deduped);
    }
}

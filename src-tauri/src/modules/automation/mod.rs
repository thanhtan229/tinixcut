//! Headless Automation Surface for TinixCut
//! Architecture Invariant: Automation is a client of the editor, NOT UI mouse/keyboard simulation
//! and NOT a second project reducer. It routes exclusively through the Command Service.

use crate::modules::command::{
    CommandEnvelope, CommandError, CommandReceipt, CommandService, DiffReport,
};
use crate::modules::domain::time::{FrameRate, RationalTime};
use crate::modules::domain::{MediaKind, Project, TrackKind};
use crate::modules::export::{ExportEngine, ExportError, ExportProfile, ExportReport};
use crate::modules::persistence::{load_project, save_project_atomic};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomationError {
    #[error("Failed to load project: {0}")]
    ProjectLoad(String),

    #[error("Command execution error: {0}")]
    Command(#[from] CommandError),

    #[error("Export render error: {0}")]
    Export(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("CLI argument error: {0}")]
    Cli(String),
}

impl From<ExportError> for AutomationError {
    fn from(err: ExportError) -> Self {
        AutomationError::Export(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackSummary {
    pub track_id: String,
    pub name: String,
    pub kind: TrackKind,
    pub muted: bool,
    pub items_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetSummary {
    pub asset_id: String,
    pub name: String,
    pub path: String,
    pub media_kind: MediaKind,
    pub duration: Option<RationalTime>,
}

/// Structured inspection report returned by the Inspect API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectInspectReport {
    pub project_id: String,
    pub name: String,
    pub revision: u64,
    pub duration_frames: i64,
    pub duration_seconds: f64,
    pub frame_rate: FrameRate,
    pub resolution: (u32, u32),
    pub tracks_count: usize,
    pub clips_count: usize,
    pub assets_count: usize,
    pub tracks: Vec<TrackSummary>,
    pub assets: Vec<AssetSummary>,
    pub supported_operations: Vec<String>,
}

pub struct AutomationAdapter;

impl AutomationAdapter {
    /// Inspect an in-memory Project
    pub fn inspect_project(project: &Project) -> ProjectInspectReport {
        let mut total_duration = RationalTime::zero(project.settings.frame_rate);
        let mut clips_count = 0;

        let tracks = project
            .tracks
            .iter()
            .map(|t| {
                clips_count += t.items.len();
                for item in &t.items {
                    if item.timeline_range.end().value > total_duration.value {
                        total_duration = item.timeline_range.end();
                    }
                }
                TrackSummary {
                    track_id: t.track_id.clone(),
                    name: t.name.clone(),
                    kind: t.kind,
                    muted: t.muted,
                    items_count: t.items.len(),
                }
            })
            .collect();

        let assets = project
            .assets
            .iter()
            .map(|a| AssetSummary {
                asset_id: a.asset_id.clone(),
                name: a.name.clone(),
                path: a.path.clone(),
                media_kind: a.media_kind,
                duration: a.duration,
            })
            .collect();

        let supported_operations = vec![
            "TrackCreate".to_string(),
            "TrackDelete".to_string(),
            "TrackReorder".to_string(),
            "TrackMuteSet".to_string(),
            "ClipCreate".to_string(),
            "ClipMove".to_string(),
            "ClipTrim".to_string(),
            "ClipSplit".to_string(),
            "ClipDelete".to_string(),
            "ClipDuplicate".to_string(),
            "ClipTransformSet".to_string(),
            "ClipCropSet".to_string(),
            "ClipOpacitySet".to_string(),
            "ClipVolumeSet".to_string(),
            "ClipTextSet".to_string(),
            "ClipAudioFadeSet".to_string(),
            "ClipTransitionSet".to_string(),
            "ClipKeyframeAdd".to_string(),
            "ClipKeyframeRemove".to_string(),
            "AssetImport".to_string(),
        ];

        ProjectInspectReport {
            project_id: project.project_id.clone(),
            name: project.name.clone(),
            revision: project.revision,
            duration_frames: total_duration.value,
            duration_seconds: total_duration.to_seconds(),
            frame_rate: project.settings.frame_rate,
            resolution: (project.settings.width, project.settings.height),
            tracks_count: project.tracks.len(),
            clips_count,
            assets_count: project.assets.len(),
            tracks,
            assets,
            supported_operations,
        }
    }

    /// Headless Inspect API for project files
    pub fn inspect_file(project_path: &Path) -> Result<ProjectInspectReport, AutomationError> {
        let project =
            load_project(project_path).map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        Ok(Self::inspect_project(&project))
    }

    /// Headless Diff / Dry-run API for project files
    pub fn dry_run_file(
        project_path: &Path,
        envelope: &CommandEnvelope,
    ) -> Result<DiffReport, AutomationError> {
        let project =
            load_project(project_path).map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        let service = CommandService::new(project);
        let diff = service.dry_run(envelope)?;
        Ok(diff)
    }

    /// Headless Apply API for project files
    pub fn apply_file(
        project_path: &Path,
        service: &mut CommandService,
        envelope: CommandEnvelope,
        save_after: bool,
    ) -> Result<CommandReceipt, AutomationError> {
        let receipt = service.execute(envelope)?;
        if save_after {
            save_project_atomic(service.project(), project_path)
                .map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        }
        Ok(receipt)
    }

    /// Headless Render / Export API for project files
    pub fn render_file(
        project_path: &Path,
        output_path: &Path,
        profile: Option<ExportProfile>,
    ) -> Result<ExportReport, AutomationError> {
        let project =
            load_project(project_path).map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
        let prof = profile.unwrap_or_default();
        let report = ExportEngine::render_snapshot(&project, output_path, &prof, None)?;
        Ok(report)
    }

    /// CLI argument dispatcher for headless workflows
    pub fn handle_cli(args: &[String]) -> Result<String, AutomationError> {
        if args.is_empty() {
            return Err(AutomationError::Cli(
                "Usage: tinixcut [inspect|diff|apply|render] <args...>".to_string(),
            ));
        }

        match args[0].as_str() {
            "inspect" => {
                if args.len() < 2 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut inspect <path>".to_string(),
                    ));
                }
                let report = Self::inspect_file(Path::new(&args[1]))?;
                serde_json::to_string_pretty(&report)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "diff" => {
                if args.len() < 3 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut diff <project_path> <envelope_json_path>".to_string(),
                    ));
                }
                let env_content = std::fs::read_to_string(&args[2])
                    .map_err(|e| AutomationError::Cli(format!("Cannot read envelope file: {e}")))?;
                let envelope: CommandEnvelope = serde_json::from_str(&env_content)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))?;
                let diff = Self::dry_run_file(Path::new(&args[1]), &envelope)?;
                serde_json::to_string_pretty(&diff)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "apply" => {
                if args.len() < 3 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut apply <project_path> <envelope_json_path>".to_string(),
                    ));
                }
                let project_path = Path::new(&args[1]);
                let project = load_project(project_path)
                    .map_err(|e| AutomationError::ProjectLoad(e.to_string()))?;
                let mut service = CommandService::new(project);
                let env_content = std::fs::read_to_string(&args[2])
                    .map_err(|e| AutomationError::Cli(format!("Cannot read envelope file: {e}")))?;
                let envelope: CommandEnvelope = serde_json::from_str(&env_content)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))?;
                let receipt = Self::apply_file(project_path, &mut service, envelope, true)?;
                serde_json::to_string_pretty(&receipt)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "render" => {
                if args.len() < 3 {
                    return Err(AutomationError::Cli(
                        "Usage: tinixcut render <project_path> <output_path>".to_string(),
                    ));
                }
                let report = Self::render_file(Path::new(&args[1]), Path::new(&args[2]), None)?;
                serde_json::to_string_pretty(&report)
                    .map_err(|e| AutomationError::Serialization(e.to_string()))
            }
            "help" | "--help" | "-h" => Ok(
                "TinixCut Automation CLI\nUsage: tinixcut [inspect|diff|apply|render] <args...>\n\nCommands:\n  inspect <project_path>                     Inspect a project and output JSON summary\n  diff <project_path> <envelope_json_path>   Run a dry-run diff of commands against a project\n  apply <project_path> <envelope_json_path>  Apply commands transactionally to a project\n  render <project_path> <output_path>        Render project to MP4 headlessly\n  help, --help                               Show this help message".to_string()
            ),
            other => Err(AutomationError::Cli(format!("Unknown command '{}'", other))),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::modules::command::{CommandOperation, OperationEnvelope};
    use crate::modules::domain::time::{FrameRate, RationalTime};
    use crate::modules::domain::{
        Asset, ItemKind, MediaKind, Project, ProjectSettings, TrackKind, CURRENT_SCHEMA_VERSION,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn get_fixture_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("tinixcut_test_fixtures_phase10");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn generate_synthetic_mp4(path: &Path, duration_secs: u32, color: &str) {
        if path.exists() {
            return;
        }
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("color=c={}:s=640x360:r=30:d={}", color, duration_secs),
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={}", duration_secs),
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                &path.to_string_lossy(),
            ])
            .output();

        if let Ok(res) = output {
            if !res.status.success() {
                eprintln!(
                    "FFmpeg synthetic generator warning: {}",
                    String::from_utf8_lossy(&res.stderr)
                );
            }
        }
    }

    /// Complete 10-step Automation E2E Acceptance scenario defined in PLAN.md Phase 10
    #[test]
    fn test_phase10_automation_surface_e2e_acceptance() {
        let fixture_dir = get_fixture_dir();
        let project_path = fixture_dir.join("automation_fixture.tcproj");
        let media_file = fixture_dir.join("automation_media.mp4");
        generate_synthetic_mp4(&media_file, 4, "yellow");

        // 1. Open fixture project (create and save initial project with media)
        let rate = FrameRate::FPS_30;
        let mut initial_project = Project::new("Automation Fixture Project".to_string());
        initial_project.settings = ProjectSettings {
            width: 640,
            height: 360,
            frame_rate: rate,
            sample_rate: 48000,
            channels: 2,
        };
        initial_project.assets.push(Asset {
            asset_id: "ast_auto_1".to_string(),
            name: "automation_media.mp4".to_string(),
            path: media_file.to_string_lossy().to_string(),
            media_kind: MediaKind::Video,
            duration: Some(RationalTime::from_frames(120, rate)),
            file_size: 2048,
        });

        save_project_atomic(&initial_project, &project_path).unwrap();

        // 2. Inspect revision through Inspect API
        let inspect_report = AutomationAdapter::inspect_file(&project_path).unwrap();
        assert_eq!(inspect_report.revision, 0);
        assert_eq!(inspect_report.project_id, initial_project.project_id);
        assert_eq!(inspect_report.assets_count, 1);
        assert_eq!(inspect_report.tracks_count, 0);
        assert!(inspect_report
            .supported_operations
            .contains(&"ClipCreate".to_string()));

        // Prepare an automation plan: Create video track and place a clip
        let plan_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: initial_project.project_id.clone(),
            base_revision: 0,
            dry_run: false,
            transaction_id: Some("tx_auto_01".to_string()),
            operations: vec![
                OperationEnvelope {
                    operation_id: "op_auto_trk1".to_string(),
                    operation: CommandOperation::TrackCreate {
                        track_id: Some("trk_auto_v".to_string()),
                        name: "Auto Video Track".to_string(),
                        kind: TrackKind::Video,
                    },
                },
                OperationEnvelope {
                    operation_id: "op_auto_clip1".to_string(),
                    operation: CommandOperation::ClipCreate {
                        track_id: "trk_auto_v".to_string(),
                        item_id: Some("clip_auto_1".to_string()),
                        name: "Auto Clip 1".to_string(),
                        kind: ItemKind::Video,
                        asset_id: Some("ast_auto_1".to_string()),
                        timeline_start: RationalTime::from_frames(0, rate),
                        duration: RationalTime::from_frames(60, rate), // 2.0s
                        source_start: Some(RationalTime::zero(rate)),
                    },
                },
            ],
        };

        // 3. Dry-run plan: check diff, confirm project file on disk is NOT modified
        let dry_run_envelope = CommandEnvelope {
            dry_run: true,
            ..plan_envelope.clone()
        };
        let diff_report =
            AutomationAdapter::dry_run_file(&project_path, &dry_run_envelope).unwrap();
        assert_eq!(diff_report.operations_evaluated, 2);
        assert_eq!(diff_report.changes.len(), 2);
        // Verify disk state remains at revision 0
        let unmutated = load_project(&project_path).unwrap();
        assert_eq!(unmutated.revision, 0);
        assert_eq!(unmutated.tracks.len(), 0);

        // 4. Apply plan
        let mut service = CommandService::new(unmutated);
        let receipt = AutomationAdapter::apply_file(
            &project_path,
            &mut service,
            plan_envelope.clone(),
            true, // persist after apply
        )
        .unwrap();

        assert_eq!(receipt.prior_revision, 0);
        assert_eq!(receipt.new_revision, 1);
        assert_eq!(receipt.operations_applied, 2);

        // 5. Save/reopen: confirm committed state
        let reopened = load_project(&project_path).unwrap();
        assert_eq!(reopened.revision, 1);
        assert_eq!(reopened.tracks.len(), 1);
        assert_eq!(reopened.tracks[0].items.len(), 1);
        assert_eq!(reopened.tracks[0].items[0].item_id, "clip_auto_1");

        // 6. Retry same operation ID: test idempotency deduplication
        let retry_receipt = service.execute(plan_envelope.clone()).unwrap();
        assert_eq!(retry_receipt.operations_applied, 0);
        for r in &retry_receipt.receipts {
            assert_eq!(r.status, "deduplicated_noop");
        }

        // 7. Confirm no duplicate edit: revision and item count are unchanged
        assert_eq!(service.project().revision, 1);
        assert_eq!(service.project().tracks[0].items.len(), 1);

        // 8. Attempt stale base revision: submit new edit with baseRevision = 0 when project is at 1
        let stale_envelope = CommandEnvelope {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id: initial_project.project_id.clone(),
            base_revision: 0, // Stale! Current is 1
            dry_run: false,
            transaction_id: Some("tx_stale".to_string()),
            operations: vec![OperationEnvelope {
                operation_id: "op_stale_new".to_string(),
                operation: CommandOperation::ClipDelete {
                    item_id: "clip_auto_1".to_string(),
                },
            }],
        };

        // 9. Confirm explicit conflict: StaleRevision error returned deterministically
        let stale_result = service.execute(stale_envelope);
        assert_eq!(
            stale_result,
            Err(CommandError::StaleRevision {
                expected: 1,
                received: 0,
            })
        );

        // 10. Export through job API: render the applied project
        let output_render = fixture_dir.join("automation_rendered_output.mp4");
        let mut profile = ExportProfile::default();
        profile.width = 640;
        profile.height = 360;

        let export_report =
            AutomationAdapter::render_file(&project_path, &output_render, Some(profile)).unwrap();

        assert!(output_render.exists());
        assert_eq!(export_report.duration_secs, 2.0);
        assert!(export_report.file_size_bytes > 0);
    }
}

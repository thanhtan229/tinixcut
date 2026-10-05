//! Transcription / Speech-to-Text Integration Boundary
//! Architecture Invariant: Provider output never mutates the Project directly.
//! All transcripts convert to typed domain commands through the Command Service.

use crate::modules::command::{CommandOperation, OperationEnvelope};
use crate::modules::domain::id::generate_item_id;
use crate::modules::domain::time::RationalTime;
use crate::modules::domain::ItemKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionSegment {
    pub start: RationalTime,
    pub duration: RationalTime,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionResult {
    pub language: String,
    pub segments: Vec<TranscriptionSegment>,
}

pub struct TranscriptionBoundary;

impl TranscriptionBoundary {
    /// Adapts raw transcription provider output into canonical CommandOperations.
    /// Enforces the invariant: External AI/STT adapters produce commands, not direct model mutations.
    pub fn create_caption_commands(
        track_id: &str,
        result: &TranscriptionResult,
    ) -> Vec<OperationEnvelope> {
        let mut ops = Vec::new();
        for seg in &result.segments {
            let item_id = generate_item_id();
            ops.push(OperationEnvelope {
                operation_id: format!("op_trans_{}", uuid::Uuid::new_v4().simple()),
                operation: CommandOperation::ClipCreate {
                    track_id: track_id.to_string(),
                    item_id: Some(item_id.clone()),
                    name: format!("Caption: {}", seg.text.chars().take(20).collect::<String>()),
                    kind: ItemKind::Text,
                    asset_id: None,
                    timeline_start: seg.start,
                    duration: seg.duration,
                    source_start: Some(RationalTime::zero(seg.start.rate)),
                },
            });
            ops.push(OperationEnvelope {
                operation_id: format!("op_text_{}", uuid::Uuid::new_v4().simple()),
                operation: CommandOperation::ClipTextSet {
                    item_id,
                    text: seg.text.clone(),
                },
            });
        }
        ops
    }
}

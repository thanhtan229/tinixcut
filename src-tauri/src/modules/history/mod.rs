//! History & Undo/Redo Engine
//! Architecture Invariant: History entries represent committed domain edits / transactions,
//! never transient pointer events.

use crate::modules::domain::Project;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub description: String,
    pub transaction_id: Option<String>,
    pub prior_snapshot: Project,
    pub resulting_snapshot: Project,
}

pub struct HistoryManager {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    max_depth: usize,
}

impl HistoryManager {
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: max_depth.max(10),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_stack_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_stack_depth(&self) -> usize {
        self.redo_stack.len()
    }

    /// Record a committed transaction.
    /// Invariant: Clears the redo stack when a new edit is performed.
    pub fn record_edit(&mut self, entry: HistoryEntry) {
        self.redo_stack.clear();
        self.undo_stack.push(entry);
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }

    /// Perform Undo: Restores project to the prior state before the edit.
    pub fn undo(&mut self, current_project: &mut Project) -> Option<String> {
        let entry = self.undo_stack.pop()?;
        let desc = entry.description.clone();
        *current_project = entry.prior_snapshot.clone();
        self.redo_stack.push(entry);
        Some(desc)
    }

    /// Perform Redo: Restores project to the state produced by the edit.
    pub fn redo(&mut self, current_project: &mut Project) -> Option<String> {
        let entry = self.redo_stack.pop()?;
        let desc = entry.description.clone();
        *current_project = entry.resulting_snapshot.clone();
        self.undo_stack.push(entry);
        Some(desc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_undo_redo_round_trip() {
        let mut history = HistoryManager::new(50);
        let mut p1 = Project::new("History Test".to_string());
        p1.revision = 0;

        let mut p2 = p1.clone();
        p2.name = "Renamed Project".to_string();
        p2.revision = 1;

        history.record_edit(HistoryEntry {
            description: "Rename Project".to_string(),
            transaction_id: Some("tx_rename".to_string()),
            prior_snapshot: p1.clone(),
            resulting_snapshot: p2.clone(),
        });

        assert!(history.can_undo());
        assert!(!history.can_redo());

        let mut active = p2.clone();

        // Undo
        let desc = history.undo(&mut active).expect("undo succeeded");
        assert_eq!(desc, "Rename Project");
        assert_eq!(active.name, "History Test");
        assert_eq!(active.revision, 0);
        assert!(!history.can_undo());
        assert!(history.can_redo());

        // Redo
        let redo_desc = history.redo(&mut active).expect("redo succeeded");
        assert_eq!(redo_desc, "Rename Project");
        assert_eq!(active.name, "Renamed Project");
        assert_eq!(active.revision, 1);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }
}

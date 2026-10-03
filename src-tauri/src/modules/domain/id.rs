//! Stable Identifier Generation and Validation
//! Architecture Invariant: Array index is not identity. IDs are durable and stable.

use uuid::Uuid;

pub fn generate_project_id() -> String {
    format!("proj_{}", Uuid::new_v4().simple())
}

pub fn generate_asset_id() -> String {
    format!("ast_{}", Uuid::new_v4().simple())
}

pub fn generate_track_id() -> String {
    format!("trk_{}", Uuid::new_v4().simple())
}

pub fn generate_item_id() -> String {
    format!("clip_{}", Uuid::new_v4().simple())
}

pub fn generate_transaction_id() -> String {
    format!("tx_{}", Uuid::new_v4().simple())
}

pub fn generate_operation_id() -> String {
    format!("op_{}", Uuid::new_v4().simple())
}

pub fn is_valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generation_uniqueness_and_prefixes() {
        let p1 = generate_project_id();
        let p2 = generate_project_id();
        assert_ne!(p1, p2);
        assert!(p1.starts_with("proj_"));

        let a1 = generate_asset_id();
        assert!(a1.starts_with("ast_"));

        let t1 = generate_track_id();
        assert!(t1.starts_with("trk_"));

        let c1 = generate_item_id();
        assert!(c1.starts_with("clip_"));

        let tx1 = generate_transaction_id();
        assert!(tx1.starts_with("tx_"));

        let op1 = generate_operation_id();
        assert!(op1.starts_with("op_"));

        assert!(is_valid_id(&p1));
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("   "));
    }
}

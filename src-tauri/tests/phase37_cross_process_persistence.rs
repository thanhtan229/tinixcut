use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tinixcut_lib::modules::domain::Project;
use tinixcut_lib::modules::persistence::{
    load_project, save_project_atomic, save_project_atomic_if_revision, PersistenceError,
};

const CHILD_PROJECT_PATH: &str = "TINIXCUT_PHASE37_CHILD_PROJECT_PATH";
const CHILD_SNAPSHOT_PATH: &str = "TINIXCUT_PHASE37_CHILD_SNAPSHOT_PATH";

#[test]
fn stale_writer_child() {
    let (Ok(project_path), Ok(snapshot_path)) = (
        std::env::var(CHILD_PROJECT_PATH),
        std::env::var(CHILD_SNAPSHOT_PATH),
    ) else {
        return;
    };

    let stale_snapshot: Project =
        serde_json::from_slice(&fs::read(snapshot_path).expect("read stale snapshot"))
            .expect("deserialize stale snapshot");
    let error =
        save_project_atomic_if_revision(&stale_snapshot, &PathBuf::from(project_path), Some(0))
            .expect_err("the child process must reject a stale revision");
    assert!(matches!(
        error,
        PersistenceError::RevisionConflict {
            expected: Some(0),
            actual: Some(1)
        }
    ));
}

#[test]
fn stale_snapshot_in_another_process_cannot_replace_the_newer_project() {
    let dir = std::env::temp_dir().join(format!(
        "tinixcut_phase37_cross_process_{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&dir).expect("create test directory");
    let project_path = dir.join("shared.tinix");
    let snapshot_path = dir.join("stale-snapshot.json");

    let original = Project::new("Initial".to_string());
    save_project_atomic(&original, &project_path).expect("save initial project");

    let mut stale_snapshot = original.clone();
    stale_snapshot.revision = 1;
    stale_snapshot.name = "Stale UI snapshot".to_string();
    fs::write(
        &snapshot_path,
        serde_json::to_vec(&stale_snapshot).expect("serialize stale snapshot"),
    )
    .expect("write stale snapshot");

    let mut newer_project = original;
    newer_project.revision = 1;
    newer_project.name = "External update".to_string();
    save_project_atomic_if_revision(&newer_project, &project_path, Some(0))
        .expect("write the newer revision");

    let output = Command::new(std::env::current_exe().expect("resolve test executable"))
        .args(["--exact", "stale_writer_child", "--nocapture"])
        .env(CHILD_PROJECT_PATH, &project_path)
        .env(CHILD_SNAPSHOT_PATH, &snapshot_path)
        .output()
        .expect("run stale writer in a separate process");
    assert!(
        output.status.success(),
        "child process failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let saved = load_project(&project_path).expect("load resulting project");
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.name, "External update");
    fs::remove_dir_all(dir).expect("remove test directory");
}

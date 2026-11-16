use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc,
    },
};
use tinixcut_lib::modules::media::silence::{analyze, SilenceConfig};
#[test]
#[ignore = "requires local Phase 26 JFK sample"]
fn audit_real_jfk() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/phase26-whisper/test-input-jfk.wav");
    for threshold in [-50.0, -40.0, -30.0] {
        let r = analyze(
            &p,
            SilenceConfig {
                threshold_dbfs: threshold,
                minimum_silence_ms: 600,
                padding_ms: 150,
            },
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU64::new(0)),
        )
        .unwrap();
        println!("{}", serde_json::to_string(&r).unwrap());
    }
}

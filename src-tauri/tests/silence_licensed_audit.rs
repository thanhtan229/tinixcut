use std::{path::PathBuf, sync::{Arc, atomic::{AtomicBool, AtomicU64}}};
use tinixcut_lib::modules::media::silence::{analyze, SilenceConfig};

#[test]
#[ignore = "requires licensed controlled fixtures"]
fn licensed_audit() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/background/licensed-controlled");
    for sample in 0..4 {
        for variant in ["clean", "quiet", "noise"] {
            let name = format!("{sample}-{variant}");
            let report = analyze(&root.join(format!("{name}.wav")), SilenceConfig { threshold_dbfs: -40., minimum_silence_ms: 600, padding_ms: 150 }, Arc::new(AtomicBool::new(false)), Arc::new(AtomicU64::new(0))).unwrap();
            println!("{}", serde_json::json!({"name":name,"report":report}));
        }
    }
}

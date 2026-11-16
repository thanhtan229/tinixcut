use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc,
    },
};
use tinixcut_lib::modules::media::silence::{analyze, SilenceConfig};
#[test]
#[ignore = "requires generated controlled fixtures"]
fn audit_controlled() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/background/silence-controlled");
    for name in ["clean", "quiet", "noise", "music"] {
        for threshold in [-50., -40., -30.] {
            for minimum in [400, 600, 1000] {
                for padding in [100, 150] {
                    let cfg = SilenceConfig {
                        threshold_dbfs: threshold,
                        minimum_silence_ms: minimum,
                        padding_ms: padding,
                    };
                    let r = analyze(
                        &root.join(format!("{name}.wav")),
                        cfg,
                        Arc::new(AtomicBool::new(false)),
                        Arc::new(AtomicU64::new(0)),
                    )
                    .unwrap();
                    println!("{}", serde_json::json!({"name":name,"report":r}));
                }
            }
        }
    }
}

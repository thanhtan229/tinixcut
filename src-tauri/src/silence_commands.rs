//! Thin Tauri client for read-only silence jobs; project edits remain commands.
use super::*;
use crate::modules::media::silence::{self, SilenceAnalysis, SilenceConfig};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilenceJobReport {
    pub job_id: String,
    pub project_id: String,
    pub revision: u64,
    pub asset_id: String,
    pub source_path: String,
    pub state: String,
    pub progress: f64,
    pub message: String,
    pub analysis: Option<SilenceAnalysis>,
    pub duration_micros: u64,
    #[serde(skip)]
    pub decoded_samples: Arc<AtomicU64>,
}

#[tauri::command]
pub fn media_silence_start<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    asset_id: String,
    config: SilenceConfig,
) -> Result<SilenceJobReport, String> {
    config.validate()?;
    let (report, cancel, engine_arc) = {
        let mut engine = state.0.lock().map_err(|e| e.to_string())?;
        if engine.jobs.has_active_kind(JobKind::SilenceAnalysis) {
            return Err("A silence analysis is already active; cancel it or wait".into());
        }
        if !engine.jobs.can_start() {
            return Err("Background workers are busy; try again shortly".into());
        }
        let project = engine.service.project();
        let asset = project
            .assets
            .iter()
            .find(|a| a.asset_id == asset_id)
            .ok_or("Source asset no longer exists")?;
        if asset.media_kind == MediaKind::Image {
            return Err("Silence analysis requires audio/video".into());
        }
        let duration = asset
            .duration
            .ok_or("Source duration is unknown; import/probe the source again")?
            .to_seconds();
        if !duration.is_finite() || duration <= 0.0 || duration > silence::MAX_SECONDS {
            return Err("Source must be between one sample and 30 minutes".into());
        }
        let path = PathBuf::from(&asset.path);
        require_explicit_file_scope(
            &path,
            asset_scope_allows_path(&app.asset_protocol_scope(), &path),
            "Silence analysis",
        )?;
        let id = format!("silence_{}", uuid::Uuid::new_v4().simple());
        let report = SilenceJobReport {
            job_id: id.clone(),
            project_id: project.project_id.clone(),
            revision: project.revision,
            asset_id: asset_id.clone(),
            source_path: asset.path.clone(),
            state: "running".into(),
            progress: 0.0,
            message: "Decoding source audio; project is unchanged".into(),
            analysis: None,
            duration_micros: (duration * 1_000_000.0).round() as u64,
            decoded_samples: Arc::new(AtomicU64::new(0)),
        };
        // Bound retained reports/jobs. Only this feature's terminal jobs are removed.
        if engine.silence_reports.len() >= 16 {
            let ids: Vec<_> = engine.silence_reports.keys().cloned().collect();
            for old in ids {
                engine.jobs.forget_terminal(&old);
                engine.silence_reports.remove(&old);
            }
        }
        engine
            .jobs
            .submit_job(id.clone(), JobKind::SilenceAnalysis, id.clone(), 50);
        let cancel = engine.jobs.start_job(&id)?;
        engine.silence_reports.insert(id, report.clone());
        (report, cancel, Arc::clone(&state.0))
    };
    let receipt = report.clone();
    std::thread::spawn(move || {
        let worker_progress = report.decoded_samples.clone();
        let source = PathBuf::from(&report.source_path);
        let result = silence::analyze(&source, config, cancel.clone(), worker_progress);
        if let Ok(mut engine) = engine_arc.lock() {
            let mut final_report = report;
            if cancel.load(Ordering::SeqCst) {
                engine.jobs.cancel_job(&final_report.job_id);
                final_report.state = "cancelled".into();
                final_report.message = "Cancelled; project unchanged".into();
            } else {
                match result {
                    Ok(analysis) => {
                        engine.jobs.mark_completed(&final_report.job_id);
                        final_report.state = "completed".into();
                        final_report.progress = 1.0;
                        final_report.message="Review low-amplitude intervals before applying; this is not speech detection".into();
                        final_report.analysis = Some(analysis);
                    }
                    Err(error) => {
                        engine.jobs.mark_failed(&final_report.job_id, error.clone());
                        final_report.state = "failed".into();
                        final_report.message = error;
                    }
                }
            }
            engine
                .silence_reports
                .insert(final_report.job_id.clone(), final_report);
        }
    });
    Ok(receipt)
}

#[tauri::command]
pub fn media_silence_status(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<SilenceJobReport, String> {
    let engine = state.0.lock().map_err(|e| e.to_string())?;
    let mut report = engine
        .silence_reports
        .get(&job_id)
        .cloned()
        .ok_or("Silence analysis report expired or was not found")?;
    if report.state == "running" {
        report.progress = (report.decoded_samples.load(Ordering::Relaxed) as f64
            / silence::SAMPLE_RATE as f64
            / (report.duration_micros as f64 / 1_000_000.0))
            .clamp(0.0, 0.99);
    }
    Ok(report)
}
#[tauri::command]
pub fn media_silence_cancel(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let mut engine = state.0.lock().map_err(|e| e.to_string())?;
    if !engine.silence_reports.contains_key(&job_id) {
        return Err("Silence job not found".into());
    }
    engine.jobs.request_cancel(&job_id);
    Ok(())
}
#[tauri::command]
pub fn media_silence_validate<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    job_id: String,
) -> Result<SilenceJobReport, String> {
    let report = {
        let engine = state.0.lock().map_err(|e| e.to_string())?;
        let report = engine
            .silence_reports
            .get(&job_id)
            .ok_or("Silence report expired")?;
        let p = engine.service.project();
        if p.project_id != report.project_id
            || p.revision != report.revision
            || !p
                .assets
                .iter()
                .any(|a| a.asset_id == report.asset_id && a.path == report.source_path)
        {
            return Err("Project/source changed; analyze again before applying".into());
        }
        report.clone()
    };
    let analysis = report
        .analysis
        .as_ref()
        .ok_or("Silence analysis has not completed")?;
    let path = PathBuf::from(&report.source_path);
    require_explicit_file_scope(
        &path,
        asset_scope_allows_path(&app.asset_protocol_scope(), &path),
        "Silence draft validation",
    )?;
    if FFmpegService::source_fingerprint(&path).map_err(|e| e.to_string())?
        != analysis.source_fingerprint
    {
        return Err("Source file changed; analyze again".into());
    }
    Ok(report)
}

#[cfg(all(test, feature = "mock-ipc-tests"))]
#[test]
fn mock_tauri_ipc_silence_job_scope_staleness_apply_history_and_persistence() {
    use serde_json::json;
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
        webview::InvokeRequest,
        WebviewWindowBuilder,
    };
    let dir = std::env::temp_dir().join(format!("tinix-silence-ipc-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("source.wav");
    let mut bytes = Vec::new();
    let size = 16000u32 * 3 * 4;
    bytes.extend(b"RIFF");
    bytes.extend((size + 36).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(64000u32.to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for n in 0..48000 {
        let v = if (16000..32000).contains(&n) {
            0i16
        } else {
            1000i16
        };
        bytes.extend(v.to_le_bytes());
        bytes.extend((-v).to_le_bytes());
    }
    fs::write(&path, &bytes).unwrap();
    let mut p = Project::new("Silence IPC".into());
    p.assets.push(Asset {
        asset_id: "source".into(),
        name: "Source".into(),
        path: path.to_string_lossy().into(),
        media_kind: MediaKind::Audio,
        duration: Some(RationalTime::from_frames(90, p.settings.frame_rate)),
        file_size: bytes.len() as u64,
        dimensions: None,
    });
    let range = crate::modules::domain::time::TimeRange::new(
        RationalTime::zero(p.settings.frame_rate),
        RationalTime::from_frames(90, p.settings.frame_rate),
    );
    let mut original =
        crate::modules::domain::Track::new("Original voice".into(), TrackKind::Audio);
    original.items.push(
        TimelineItem::new(
            Some("source".into()),
            "Original".into(),
            ItemKind::Audio,
            range.clone(),
            range,
        )
        .unwrap(),
    );
    p.tracks.push(original);
    let app = mock_builder()
        .manage(AppState(
            Arc::new(Mutex::new(EditorEngine::new(p))),
            Default::default(),
            Arc::new(Mutex::new(JobManager::new(1))),
        ))
        .invoke_handler(tauri::generate_handler![
            media_silence_start,
            media_silence_status,
            media_silence_cancel,
            media_silence_validate,
            crate::project_get_current,
            crate::project_execute_command,
            crate::project_save,
            crate::project_open,
            crate::project_undo,
            crate::project_redo
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, body: serde_json::Value| {
        get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.into(),
            },
        )
        .map(|r| r.deserialize::<serde_json::Value>().unwrap())
    };
    let args = json!({"assetId":"source","config":{"thresholdDbfs":-40,"minimumSilenceMs":400,"paddingMs":100}});
    assert!(invoke("media_silence_start", args.clone()).is_err());
    app.asset_protocol_scope().allow_file(&path).unwrap();
    let before = invoke("project_get_current", json!({})).unwrap();
    let start = invoke("media_silence_start", args).unwrap();
    let id = start["jobId"].as_str().unwrap();
    let mut result = start.clone();
    for _ in 0..200 {
        result = invoke("media_silence_status", json!({"jobId":id})).unwrap();
        if result["state"] != "running" {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(result["state"], "completed", "{result}");
    assert_eq!(result["analysis"]["cuts"].as_array().unwrap().len(), 1);
    assert_eq!(invoke("project_get_current", json!({})).unwrap(), before);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    invoke("media_silence_validate", json!({"jobId":id})).unwrap();
    // Source identity is validated independently of project revision.
    let mut changed = bytes.clone();
    changed.extend([0, 0]);
    fs::write(&path, &changed).unwrap();
    assert!(invoke("media_silence_validate", json!({"jobId":id})).is_err());
    fs::write(&path, &bytes).unwrap();
    let rate = before["settings"]["frame_rate"].clone();
    let time = |v| json!({"value":v,"rate":rate});
    let operations = json!([{"operation_id":"track","operation":{"type":"TrackCreate","payload":{"track_id":"cut","name":"Silence Cut","kind":"Audio"}}},{"operation_id":"first","operation":{"type":"ClipCreate","payload":{"item_id":"first","track_id":"cut","name":"First","kind":"Audio","asset_id":"source","timeline_start":time(0),"source_start":time(0),"duration":time(33)}}},{"operation_id":"last","operation":{"type":"ClipCreate","payload":{"item_id":"last","track_id":"cut","name":"Last","kind":"Audio","asset_id":"source","timeline_start":time(33),"source_start":time(57),"duration":time(33)}}}]);
    invoke("project_execute_command",json!({"envelope":{"schema_version":before["schema_version"],"project_id":before["project_id"],"base_revision":before["revision"],"transaction_id":"silence-cut","operations":operations}})).unwrap();
    let after = invoke("project_get_current", json!({})).unwrap();
    assert_eq!(after["tracks"].as_array().unwrap().len(), 2);
    assert_eq!(
        after["tracks"][0], before["tracks"][0],
        "Original track/clip must be untouched"
    );
    assert!(invoke("media_silence_validate", json!({"jobId":id})).is_err());
    invoke("project_undo", json!({})).unwrap();
    assert_eq!(
        invoke("project_get_current", json!({})).unwrap()["tracks"],
        before["tracks"]
    );
    invoke("project_redo", json!({})).unwrap();
    assert_eq!(
        invoke("project_get_current", json!({})).unwrap()["tracks"],
        after["tracks"]
    );
    let saved = dir.join("cut.tcproj");
    app.asset_protocol_scope().allow_file(&saved).unwrap();
    invoke("project_save", json!({"path":saved})).unwrap();
    assert_eq!(
        invoke("project_open", json!({"path":saved})).unwrap()["tracks"],
        after["tracks"]
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    if let Ok(report_path) = std::env::var("TINIX_MULTI_CUT_REPORT") {
        let report: serde_json::Value = serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
        assert_eq!(report["status"], "passed");
        let long_selection = std::env::var_os("TINIX_LONG_CUT_SELECTION").is_some();
        let operations = report[if long_selection { "longOperations" } else { "multiOperations" }].as_array().unwrap();
        assert_eq!(operations.len(), 4);
        let baseline = invoke("project_get_current", json!({})).unwrap();
        invoke("project_execute_command", json!({"envelope":{"schema_version":baseline["schema_version"],"project_id":baseline["project_id"],"base_revision":baseline["revision"],"transaction_id":"multi-silence-cut","operations":operations}})).unwrap();
        let applied = invoke("project_get_current", json!({})).unwrap();
        assert_eq!(applied["tracks"].as_array().unwrap().len(), 3);
        assert_eq!(applied["tracks"][0], before["tracks"][0]);
        let items = applied["tracks"][2]["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        if long_selection {
            for operation in &operations[1..] {
                let payload = &operation["operation"]["payload"];
                let item = items.iter().find(|item| item["item_id"] == payload["item_id"]).unwrap();
                assert_eq!(item["timeline_range"]["start"], payload["timeline_start"]);
                assert_eq!(item["timeline_range"]["duration"], payload["duration"]);
                assert_eq!(item["source_range"]["start"], payload["source_start"]);
            }
        }
        invoke("project_undo", json!({})).unwrap();
        assert_eq!(invoke("project_get_current", json!({})).unwrap()["tracks"], baseline["tracks"]);
        invoke("project_redo", json!({})).unwrap();
        assert_eq!(invoke("project_get_current", json!({})).unwrap()["tracks"], applied["tracks"]);
        invoke("project_save", json!({"path":saved})).unwrap();
        assert_eq!(invoke("project_open", json!({"path":saved})).unwrap()["tracks"], applied["tracks"]);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        println!("MULTI_CUT_UI_COMMAND_HISTORY_PERSISTENCE_PASS");
    }
    if let Ok(report_path) = std::env::var("TINIX_TRANSCRIPT_SELECTION_REPORT") {
        let report: serde_json::Value = serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
        assert_eq!(report["ranges"], json!([{"sourceInSeconds":2,"sourceOutSeconds":3}]));
        let operations = report["operations"].as_array().unwrap();
        assert_eq!(operations.len(), 2);
        let baseline = invoke("project_get_current", json!({})).unwrap();
        invoke("project_execute_command", json!({"envelope":{"schema_version":baseline["schema_version"],"project_id":baseline["project_id"],"base_revision":baseline["revision"],"transaction_id":"transcript-selected-subset","operations":operations}})).unwrap();
        let applied = invoke("project_get_current", json!({})).unwrap();
        let track = applied["tracks"].as_array().unwrap().iter().find(|track| track["track_id"] == "trk_transcript_cut_42").unwrap();
        let items = track["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["asset_id"], "source");
        assert_eq!(items[0]["source_range"]["start"]["value"], 60);
        assert_eq!(items[0]["source_range"]["duration"]["value"], 30);
        assert_eq!(items[0]["timeline_range"]["start"]["value"], 60);
        invoke("project_undo", json!({})).unwrap();
        assert_eq!(invoke("project_get_current", json!({})).unwrap()["tracks"], baseline["tracks"]);
        invoke("project_redo", json!({})).unwrap();
        assert_eq!(invoke("project_get_current", json!({})).unwrap()["tracks"], applied["tracks"]);
        invoke("project_save", json!({"path":saved})).unwrap();
        assert_eq!(invoke("project_open", json!({"path":saved})).unwrap()["tracks"], applied["tracks"]);
        println!("TRANSCRIPT_SELECTION_UI_COMMAND_HISTORY_PERSISTENCE_PASS");
    }
    if let Ok(report_path) = std::env::var("TINIX_TRANSCRIPT_REVEAL_REPORT") {
        let report: serde_json::Value = serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
        assert_eq!(report["ranges"], json!([{"sourceInSeconds":0,"sourceOutSeconds":1},{"sourceInSeconds":2,"sourceOutSeconds":3}]));
        assert_eq!(report["labels"], json!(["voice.wav · 0.00–1.00 s", "voice.wav · 2.00–3.00 s"]));
        assert!(report["hiddenSummary"].as_str().unwrap().contains("source: voice.wav"));
        assert_eq!(report["assetId"], "source");
        let operations = report["operations"].as_array().unwrap();
        assert_eq!(operations.len(), 3);
        let baseline = invoke("project_get_current", json!({})).unwrap();
        invoke("project_execute_command", json!({"envelope":{"schema_version":baseline["schema_version"],"project_id":baseline["project_id"],"base_revision":baseline["revision"],"transaction_id":"transcript-revealed","operations":operations}})).unwrap();
        let applied = invoke("project_get_current", json!({})).unwrap();
        let track = applied["tracks"].as_array().unwrap().iter().find(|track| track["track_id"] == "trk_transcript_cut_42").unwrap();
        let items = track["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        for operation in &operations[1..] {
            let payload = &operation["operation"]["payload"];
            let item = items.iter().find(|item| item["item_id"] == payload["item_id"]).unwrap();
            assert_eq!(item["source_range"]["start"], payload["source_start"]);
            assert_eq!(item["timeline_range"]["start"], payload["timeline_start"]);
            assert_eq!(item["timeline_range"]["duration"], payload["duration"]);
        }
        invoke("project_undo", json!({})).unwrap();
        assert_eq!(invoke("project_get_current", json!({})).unwrap()["tracks"], baseline["tracks"]);
        invoke("project_redo", json!({})).unwrap();
        assert_eq!(invoke("project_get_current", json!({})).unwrap()["tracks"], applied["tracks"]);
        invoke("project_save", json!({"path":saved})).unwrap();
        assert_eq!(invoke("project_open", json!({"path":saved})).unwrap()["tracks"], applied["tracks"]);
        println!("TRANSCRIPT_REVEAL_UI_COMMAND_HISTORY_PERSISTENCE_PASS");
    }
    fs::remove_dir_all(dir).unwrap();
}

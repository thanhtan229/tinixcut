use crate::modules::{
    command::{CommandEnvelope, CommandService},
    history::{HistoryEntry, HistoryManager},
    persistence::{load_project, save_project_atomic},
};
use std::{fs, path::PathBuf};

#[test]
#[ignore = "requires fresh headless draft review operations"]
fn draft_review_source_start_apply_undo_redo() {
    use crate::modules::domain::{Asset, MediaKind, Project, time::{FrameRate, RationalTime}};
    let evidence = std::env::var_os("TINIX_DRAFT_SOURCE_REPORT").expect("set fresh review evidence");
    let report: serde_json::Value = serde_json::from_slice(&fs::read(evidence).unwrap()).unwrap();
    let dir = std::env::temp_dir().join(format!("tinixcut-draft-source-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let mut initial = Project::new("Draft review".into());
    for id in ["a", "b", "c"] {
        let path = dir.join(format!("{id}.webm"));
        fs::write(&path, b"fixture").unwrap();
        initial.assets.push(Asset { asset_id: id.into(), name: id.into(), path: path.to_string_lossy().into_owned(), media_kind: MediaKind::Video, duration: Some(RationalTime::new(300, FrameRate::FPS_30)), file_size: 7, dimensions: None });
    }
    let envelope: CommandEnvelope = serde_json::from_value(serde_json::json!({"schema_version":initial.schema_version,"project_id":initial.project_id,"base_revision":initial.revision,"transaction_id":"draft-source-test","operations":report["operations"]})).unwrap();
    let mut service = CommandService::new(initial.clone());
    let result = service.execute(envelope).unwrap();
    let applied = service.project().clone();
    let clip = applied.tracks.iter().flat_map(|track| &track.items).find(|item| item.item_id == "draft_r0_clip_1").unwrap();
    assert_eq!(clip.source_range.start.value, 30);
    assert_eq!(clip.source_range.duration.value, 150);
    assert!(report["reviewRows"][1].as_str().unwrap().contains("source start ~1.000 s (30 frames at 30/1 fps)"));
    let mut history = HistoryManager::new(20);
    history.record_edit(HistoryEntry { description:"Draft".into(), transaction_id:result.transaction_id, prior_snapshot:initial.clone(), resulting_snapshot:applied.clone() });
    let mut current = applied.clone();
    assert!(history.undo(&mut current).is_some()); assert_eq!(current.tracks, initial.tracks);
    assert!(history.redo(&mut current).is_some()); assert_eq!(current.tracks, applied.tracks);
    let saved = dir.join("draft.tcproj"); save_project_atomic(&current, &saved).unwrap(); assert_eq!(load_project(&saved).unwrap().tracks, applied.tracks);
    fs::remove_dir_all(dir).unwrap();
    println!("DRAFT_REVIEW_SOURCE_START_HISTORY_PASS");
}

#[cfg(feature = "mock-ipc-tests")]
#[test]
fn mock_tauri_ipc_media_import_scope_and_history() {
    use crate::{AppState, EditorEngine, JobManager};
    use std::sync::{Arc, Mutex};
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
        webview::InvokeRequest,
        Manager, WebviewWindowBuilder,
    };
    let dir = std::env::temp_dir().join(format!("tinixcut-mock-import-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let valid = dir.join("valid.wav");
    let corrupt = dir.join("corrupt.mp4");
    let outside = dir.join("outside.wav");
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&1636u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&8000u32.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&1600u32.to_le_bytes());
    wav.resize(wav.len() + 1600, 0);
    fs::write(&valid, &wav).unwrap();
    fs::write(&outside, &wav).unwrap();
    fs::write(&corrupt, b"not an MP4").unwrap();
    let initial = crate::modules::domain::Project::new("Mock import".into());
    let state = AppState(
        Arc::new(Mutex::new(EditorEngine::new(initial))),
        Arc::new(Mutex::new(Default::default())),
        Arc::new(Mutex::new(JobManager::new(1))),
    );
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            crate::media_import,
            crate::project_execute_command,
            crate::project_save,
            crate::project_open,
            crate::project_collect_media,
            crate::project_get_current,
            crate::project_undo,
            crate::project_redo
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    app.asset_protocol_scope().allow_file(&valid).unwrap();
    app.asset_protocol_scope().allow_file(&corrupt).unwrap();
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
        .map(|response| response.deserialize::<serde_json::Value>().unwrap())
    };
    let import = |path: &std::path::Path| {
        invoke(
            "media_import",
            serde_json::json!({"filePath":path.to_string_lossy()}),
        )
    };
    assert!(import(&outside).is_err());
    assert!(import(&corrupt).is_err());
    let unchanged = invoke("project_get_current", serde_json::json!({})).unwrap();
    assert_eq!(unchanged["revision"], 0);
    assert_eq!(unchanged["assets"].as_array().unwrap().len(), 0);
    let asset = import(&valid).unwrap();
    assert_eq!(asset["media_kind"], "Audio");
    let imported = invoke("project_get_current", serde_json::json!({})).unwrap();
    assert_eq!(imported["revision"], 1);
    assert_eq!(imported["assets"].as_array().unwrap().len(), 1);
    invoke("project_undo", serde_json::json!({})).unwrap();
    let undone = invoke("project_get_current", serde_json::json!({})).unwrap();
    assert_eq!(undone["revision"], 2);
    assert!(undone["assets"].as_array().unwrap().is_empty());
    invoke("project_redo", serde_json::json!({})).unwrap();
    let redone = invoke("project_get_current", serde_json::json!({})).unwrap();
    assert_eq!(redone["revision"], 3);
    assert_eq!(redone["assets"], imported["assets"]);
    let replacement = dir.join("replacement.wav");
    fs::rename(&valid, &replacement).unwrap();
    app.asset_protocol_scope().allow_file(&replacement).unwrap();
    let relink = serde_json::json!({"schema_version":redone["schema_version"],"project_id":redone["project_id"],"base_revision":3,"transaction_id":"tx_relink","operations":[{"operation_id":"op_relink","operation":{"type":"AssetRelink","payload":{"asset_id":asset["asset_id"],"path":replacement.to_string_lossy()}}}]});
    invoke(
        "project_execute_command",
        serde_json::json!({"envelope":relink}),
    )
    .unwrap();
    let relinked = invoke("project_get_current", serde_json::json!({})).unwrap();
    assert_eq!(relinked["assets"][0]["asset_id"], asset["asset_id"]);
    assert_eq!(
        relinked["assets"][0]["path"],
        replacement.to_string_lossy().as_ref()
    );
    invoke("project_undo", serde_json::json!({})).unwrap();
    assert_eq!(
        invoke("project_get_current", serde_json::json!({})).unwrap()["assets"],
        imported["assets"]
    );
    invoke("project_redo", serde_json::json!({})).unwrap();
    app.asset_protocol_scope()
        .allow_directory(&dir, true)
        .unwrap();
    let project_path = dir.join("relinked.tcproj");
    invoke(
        "project_save",
        serde_json::json!({"path":project_path.to_string_lossy()}),
    )
    .unwrap();
    let reopened = invoke(
        "project_open",
        serde_json::json!({"path":project_path.to_string_lossy()}),
    )
    .unwrap();
    assert_eq!(reopened["assets"], relinked["assets"]);
    let collection = dir.join("collection");
    fs::create_dir(&collection).unwrap();
    let collected = invoke("project_collect_media", serde_json::json!({"destinationDirectory":collection.to_string_lossy(),"expectedProjectId":reopened["project_id"],"expectedRevision":reopened["revision"]})).unwrap();
    assert_eq!(collected["copiedAssets"], 1);
    let copied = collected["project"]["assets"][0]["path"].as_str().unwrap();
    assert_eq!(fs::read(copied).unwrap(), fs::read(&replacement).unwrap());
    let collected_project = invoke(
        "project_open",
        serde_json::json!({"path":collected["projectPath"]}),
    )
    .unwrap();
    assert_eq!(collected_project["assets"], collected["project"]["assets"]);
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(feature = "mock-ipc-tests")]
#[test]
fn mock_tauri_ipc_caption_merge_undo_redo() {
    use crate::{AppState, EditorEngine, JobManager};
    use std::sync::{Arc, Mutex};
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
        webview::InvokeRequest,
        WebviewWindowBuilder,
    };
    let initial = load_project(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../scripts/qualification/background/caption-merge.tcproj"),
    )
    .unwrap();
    let rate = &initial.settings.frame_rate;
    let range =
        serde_json::json!({"start":{"value":0,"rate":rate},"duration":{"value":60,"rate":rate}});
    let envelope = serde_json::json!({"schema_version":initial.schema_version,"project_id":initial.project_id,"base_revision":initial.revision,"transaction_id":"tx_ipc_caption_merge","operations":[
        {"operation_id":"ipc_text","operation":{"type":"ClipTextSet","payload":{"item_id":"caption_left","text":"Xin chao\nViet Nam"}}},
        {"operation_id":"ipc_delete","operation":{"type":"ClipDelete","payload":{"item_id":"caption_right"}}},
        {"operation_id":"ipc_trim","operation":{"type":"ClipTrim","payload":{"item_id":"caption_left","new_timeline_range":range,"new_source_range":range}}}
    ]});
    let state = AppState(
        Arc::new(Mutex::new(EditorEngine::new(initial.clone()))),
        Arc::new(Mutex::new(Default::default())),
        Arc::new(Mutex::new(JobManager::new(1))),
    );
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            crate::project_execute_command,
            crate::project_get_current,
            crate::project_undo,
            crate::project_redo,
            crate::project_save,
            crate::project_open,
            crate::project_check_recovery,
            crate::project_save_recovery_snapshot,
            crate::project_restore_recovery,
            crate::captions_import,
            crate::captions_export
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    use tauri::Manager;
    let dir = std::env::temp_dir().join(format!("tinixcut-mock-save-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let saved_path = dir.join("caption.tcproj");
    app.asset_protocol_scope()
        .allow_directory(&dir, true)
        .unwrap();
    let invoke = |cmd: &str, body: serde_json::Value| -> serde_json::Value {
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
        .unwrap()
        .deserialize()
        .unwrap()
    };
    let receipt = invoke(
        "project_execute_command",
        serde_json::json!({"envelope":envelope}),
    );
    assert_eq!(receipt["operationsApplied"], 3);
    let merged = invoke("project_get_current", serde_json::json!({}));
    assert_eq!(merged["revision"], 1);
    assert_eq!(merged["tracks"][0]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        merged["tracks"][0]["items"][0]["text_content"],
        "Xin chao\nViet Nam"
    );
    assert_eq!(
        merged["tracks"][0]["items"][0]["timeline_range"]["duration"]["value"],
        60
    );
    invoke("project_undo", serde_json::json!({}));
    let undone = invoke("project_get_current", serde_json::json!({}));
    assert_eq!(undone["revision"], 2);
    let undone_project: crate::modules::domain::Project = serde_json::from_value(undone).unwrap();
    assert_eq!(undone_project.tracks, initial.tracks);
    invoke("project_redo", serde_json::json!({}));
    let redone = invoke("project_get_current", serde_json::json!({}));
    assert_eq!(redone["revision"], 3);
    assert_eq!(redone["tracks"], merged["tracks"]);
    let mut invalid = envelope.clone();
    invalid["base_revision"] = serde_json::json!(3);
    invalid["transaction_id"] = serde_json::json!("tx_invalid");
    invalid["operations"][1]["operation"]["payload"]["item_id"] = serde_json::json!("missing");
    let failed = get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: "project_execute_command".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(serde_json::json!({"envelope":invalid})),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    );
    assert!(failed.is_err());
    assert_eq!(
        invoke("project_get_current", serde_json::json!({}))["revision"],
        3
    );
    invoke("project_undo", serde_json::json!({}));
    let again: crate::modules::domain::Project =
        serde_json::from_value(invoke("project_get_current", serde_json::json!({}))).unwrap();
    assert_eq!(again.tracks, undone_project.tracks);
    invoke("project_redo", serde_json::json!({}));
    let after_redo = invoke("project_get_current", serde_json::json!({}));
    invoke(
        "project_save",
        serde_json::json!({"path":saved_path.to_string_lossy()}),
    );
    let reopened = invoke(
        "project_open",
        serde_json::json!({"path":saved_path.to_string_lossy()}),
    );
    assert_eq!(reopened["revision"], after_redo["revision"]);
    assert_eq!(reopened["tracks"], after_redo["tracks"]);
    let revision = reopened["revision"].as_u64().unwrap();
    let saved_bytes = fs::read(&saved_path).unwrap();
    let receipt = invoke(
        "project_save_recovery_snapshot",
        serde_json::json!({"path":saved_path.to_string_lossy(),"expectedRevision":revision}),
    );
    assert_eq!(receipt["projectRevision"], revision);
    let recovery_path = saved_path.with_extension("tcproj.recovery");
    let recovery_bytes = fs::read(&recovery_path).unwrap();
    let stale = get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: "project_save_recovery_snapshot".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(
                serde_json::json!({"path":saved_path.to_string_lossy(),"expectedRevision":revision+1}),
            ),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    );
    assert!(stale.is_err());
    assert_eq!(fs::read(&saved_path).unwrap(), saved_bytes);
    assert_eq!(fs::read(&recovery_path).unwrap(), recovery_bytes);
    let _status = invoke(
        "project_check_recovery",
        serde_json::json!({"path":saved_path.to_string_lossy()}),
    );
    let mut recovery = load_project(&recovery_path).unwrap();
    recovery.revision = revision + 1;
    recovery.name = "Recovered Vietnamese captions".into();
    save_project_atomic(&recovery, &recovery_path).unwrap();
    let fingerprint =
        crate::modules::persistence::recovery_snapshot_fingerprint(&recovery).unwrap();
    let rejected = get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: "project_restore_recovery".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(
                serde_json::json!({"path":saved_path.to_string_lossy(),"expectedProjectRevision":revision,"expectedRecoveryRevision":revision+1,"expectedRecoveryFingerprint":"stale-fingerprint"}),
            ),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    );
    assert!(rejected.is_err());
    assert_eq!(fs::read(&saved_path).unwrap(), saved_bytes);
    let restored = invoke(
        "project_restore_recovery",
        serde_json::json!({"path":saved_path.to_string_lossy(),"expectedProjectRevision":revision,"expectedRecoveryRevision":revision+1,"expectedRecoveryFingerprint":fingerprint}),
    );
    assert_eq!(restored["project"]["name"], "Recovered Vietnamese captions");
    assert_eq!(restored["project"]["revision"], revision + 2);
    assert_eq!(fs::read(&saved_path).unwrap(), saved_bytes);
    invoke(
        "project_save",
        serde_json::json!({"path":saved_path.to_string_lossy()}),
    );
    let reopened_recovery = invoke(
        "project_open",
        serde_json::json!({"path":saved_path.to_string_lossy()}),
    );
    assert_eq!(reopened_recovery["name"], "Recovered Vietnamese captions");
    assert_eq!(reopened_recovery["revision"], revision + 2);
    let subtitle = dir.join("vietnamese.srt");
    fs::write(
        &subtitle,
        "1\n00:00:03,000 --> 00:00:04,000\nXin chào Việt Nam\nDòng thứ hai\n",
    )
    .unwrap();
    invoke(
        "captions_import",
        serde_json::json!({"filePath":subtitle.to_string_lossy()}),
    );
    let output = dir.join("captions.vtt");
    invoke(
        "captions_export",
        serde_json::json!({"outputPath":output.to_string_lossy()}),
    );
    let text = fs::read_to_string(&output).unwrap();
    assert!(text.contains("Xin chào Việt Nam"));
    assert!(text.contains("00:00:03.000 --> 00:00:04.000"));
    assert!(text.contains("Dòng thứ hai"));
    invoke("project_undo", serde_json::json!({}));
    invoke("project_redo", serde_json::json!({}));
    let before_roundtrip = invoke("project_get_current", serde_json::json!({}));
    invoke(
        "captions_import",
        serde_json::json!({"filePath":output.to_string_lossy()}),
    );
    let roundtrip = invoke("project_get_current", serde_json::json!({}));
    assert_eq!(
        roundtrip["tracks"].as_array().unwrap().len(),
        before_roundtrip["tracks"].as_array().unwrap().len() + 1
    );
    let last = roundtrip["tracks"].as_array().unwrap().last().unwrap();
    assert!(last["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["text_content"]
            .as_str()
            .is_some_and(|t| t.contains("Xin chào Việt Nam\nDòng thứ hai"))));
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn caption_text_and_timing_one_transaction_history_and_reopen() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/qualification/background/caption-merge.tcproj");
    let initial = load_project(&fixture).expect("load caption fixture");
    let range = serde_json::json!({"start":{"value":0,"rate":initial.settings.frame_rate},"duration":{"value":29,"rate":initial.settings.frame_rate}});
    let envelope: CommandEnvelope = serde_json::from_value(serde_json::json!({
        "schema_version":initial.schema_version,"project_id":initial.project_id,"base_revision":initial.revision,
        "transaction_id":"combined-caption-draft","operations":[
            {"operation_id":"text","operation":{"type":"ClipTextSet","payload":{"item_id":"caption_left","text":"Combined draft"}}},
            {"operation_id":"timing","operation":{"type":"ClipTrim","payload":{"item_id":"caption_left","new_timeline_range":range,"new_source_range":range}}}
        ]
    })).unwrap();
    let mut service = CommandService::new(initial.clone());
    let receipt = service.execute(envelope).expect("apply combined draft");
    assert_eq!(receipt.operations_applied, 2);
    let applied = service.project().clone();
    assert_eq!(applied.tracks[0].items[0].text_content.as_deref(), Some("Combined draft"));
    assert_eq!(applied.tracks[0].items[0].timeline_range.duration.value, 29);
    assert_eq!(applied.tracks[0].items[1], initial.tracks[0].items[1]);
    let mut history = HistoryManager::new(20);
    history.record_edit(HistoryEntry { description: "Caption edits".into(), transaction_id: receipt.transaction_id, prior_snapshot: initial.clone(), resulting_snapshot: applied.clone() });
    let mut current = applied.clone();
    assert_eq!(history.undo_stack_depth(), 1);
    assert!(history.undo(&mut current).is_some());
    assert_eq!(current.tracks, initial.tracks);
    assert!(history.redo(&mut current).is_some());
    assert_eq!(current.tracks, applied.tracks);
    let dir = std::env::temp_dir().join(format!("tinixcut-caption-combined-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("caption.tcproj");
    save_project_atomic(&current, &path).unwrap();
    let reopened = load_project(&path).unwrap();
    assert_eq!(reopened.tracks, applied.tracks);
    assert_eq!(reopened.revision, current.revision);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn caption_merge_is_one_history_entry_and_survives_save_reopen() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/qualification/background/caption-merge.tcproj");
    let initial = load_project(&fixture).expect("load caption fixture");
    let rate = &initial.settings.frame_rate;
    let range = serde_json::json!({
        "start": { "value": 0, "rate": rate },
        "duration": { "value": 60, "rate": rate }
    });
    let envelope: CommandEnvelope = serde_json::from_value(serde_json::json!({
        "schema_version": initial.schema_version,
        "project_id": initial.project_id,
        "base_revision": initial.revision,
        "dry_run": false,
        "transaction_id": "tx_background_caption_history",
        "operations": [
            {"operation_id": "caption_text", "operation": {"type": "ClipTextSet", "payload": {"item_id": "caption_left", "text": "Xin chao\nViet Nam"}}},
            {"operation_id": "caption_delete", "operation": {"type": "ClipDelete", "payload": {"item_id": "caption_right"}}},
            {"operation_id": "caption_trim", "operation": {"type": "ClipTrim", "payload": {"item_id": "caption_left", "new_timeline_range": range, "new_source_range": range}}}
        ]
    })).expect("valid merge envelope");

    let mut service = CommandService::new(initial.clone());
    let receipt = service.execute(envelope).expect("merge transaction");
    assert_eq!(receipt.operations_applied, 3);
    assert_eq!(receipt.new_revision, initial.revision + 1);
    let merged = service.project().clone();
    assert_eq!(merged.tracks[0].items.len(), 1);
    assert_eq!(
        merged.tracks[0].items[0].text_content.as_deref(),
        Some("Xin chao\nViet Nam")
    );
    assert_eq!(merged.tracks[0].items[0].timeline_range.duration.value, 60);

    let mut history = HistoryManager::new(20);
    history.record_edit(HistoryEntry {
        description: "Merge adjacent captions".into(),
        transaction_id: receipt.transaction_id,
        prior_snapshot: initial.clone(),
        resulting_snapshot: merged.clone(),
    });
    assert_eq!(history.undo_stack_depth(), 1);
    let mut current = merged.clone();
    assert!(history.undo(&mut current).is_some());
    assert_eq!(current.tracks, initial.tracks);
    assert!(!history.can_undo());
    assert!(history.redo(&mut current).is_some());
    assert_eq!(current.tracks, merged.tracks);
    assert!(!history.can_redo());

    let dir =
        std::env::temp_dir().join(format!("tinixcut-caption-history-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).expect("create temporary directory");
    let path = dir.join("caption.tcproj");
    save_project_atomic(&current, &path).expect("save merged project");
    let reopened = load_project(&path).expect("reopen merged project");
    assert_eq!(reopened.revision, merged.revision);
    assert_eq!(reopened.tracks, merged.tracks);
    fs::remove_dir_all(dir).expect("remove temporary directory");
}

#[test]
fn timeline_group_move_survives_save_reopen() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/qualification/background/caption-merge.tcproj");
    let initial = load_project(&fixture).expect("load timeline fixture");
    let envelope: CommandEnvelope = serde_json::from_value(serde_json::json!({
        "schema_version": initial.schema_version,
        "project_id": initial.project_id,
        "base_revision": initial.revision,
        "transaction_id": "tx_background_group_move",
        "operations": [{"operation_id": "move_both", "operation": {"type": "ClipGroupMove", "payload": {
            "item_ids": ["caption_left", "caption_right"],
            "delta": {"value": 30, "rate": initial.settings.frame_rate}
        }}}]
    })).expect("valid group move envelope");
    let mut service = CommandService::new(initial.clone());
    service.execute(envelope).expect("move transaction");
    let moved = service.project().clone();
    assert_eq!(moved.tracks[0].items[0].timeline_range.start.value, 30);
    assert_eq!(moved.tracks[0].items[1].timeline_range.start.value, 60);
    let mut history = HistoryManager::new(20);
    history.record_edit(HistoryEntry {
        description: "Move captions".into(),
        transaction_id: Some("tx_background_group_move".into()),
        prior_snapshot: initial.clone(),
        resulting_snapshot: moved.clone(),
    });
    let mut current = moved.clone();
    assert!(history.undo(&mut current).is_some());
    assert_eq!(current.tracks, initial.tracks);
    assert!(history.redo(&mut current).is_some());
    assert_eq!(current.tracks, moved.tracks);
    let dir = std::env::temp_dir().join(format!("tinixcut-group-move-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).expect("create temporary directory");
    let path = dir.join("timeline.tcproj");
    save_project_atomic(&current, &path).expect("save timeline");
    let reopened = load_project(&path).expect("reopen timeline");
    assert_eq!(reopened.revision, initial.revision + 1);
    assert_eq!(reopened.tracks, moved.tracks);
    fs::remove_dir_all(dir).expect("remove temporary directory");
}

#[test]
#[ignore = "requires fresh headless multi-caption UI command"]
fn multi_caption_ui_apply_undo_redo() {
    let evidence_path = std::env::var_os("TINIXCUT_CAPTION_MULTI_COMMAND").expect("set fresh UI command path");
    let value: serde_json::Value = serde_json::from_slice(&fs::read(evidence_path).unwrap()).unwrap();
    let initial: crate::modules::domain::Project = serde_json::from_value(value["project"].clone()).unwrap();
    let envelope: CommandEnvelope = serde_json::from_value(serde_json::json!({
        "schema_version":initial.schema_version,"project_id":initial.project_id,
        "base_revision":initial.revision,"transaction_id":"multi-caption-ui",
        "operations":value["operations"]
    })).unwrap();
    let mut service = CommandService::new(initial.clone());
    let receipt = service.execute(envelope).unwrap();
    assert_eq!(receipt.operations_applied, 3);
    let changed = service.project().clone();
    assert_eq!(changed.tracks[0].items.iter().find(|x| x.item_id == "long-120").unwrap().text_content.as_deref(), Some("changed without match"));
    assert_eq!(changed.tracks[0].items.iter().find(|x| x.item_id == "long-170").unwrap().text_content.as_deref(), Some("unique-find beta"));
    assert_eq!(changed.tracks[1].items[1].timeline_range.start.value, 35);
    assert_eq!(changed.tracks[1].items[0], initial.tracks[1].items[0]);
    for (before, after) in initial.tracks[0].items.iter().zip(&changed.tracks[0].items) {
        if before.item_id != "long-120" && before.item_id != "long-170" { assert_eq!(before, after); }
    }
    let mut history = HistoryManager::new(20);
    history.record_edit(HistoryEntry { description: "Edit captions".into(), transaction_id: receipt.transaction_id, prior_snapshot: initial.clone(), resulting_snapshot: changed.clone() });
    let mut current = changed.clone();
    assert!(history.undo(&mut current).is_some());
    assert_eq!(current.tracks, initial.tracks);
    assert!(history.redo(&mut current).is_some());
    assert_eq!(current.tracks, changed.tracks);
    println!("MULTI_CAPTION_UI_APPLY_UNDO_REDO_PASS");
}

#[test]
#[ignore = "requires fresh headless caption scoped draft evidence"]
fn scoped_caption_ui_apply_and_undo() {
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut value:serde_json::Value=serde_json::from_slice(&fs::read(root.join("scripts/qualification/background/caption-merge.tcproj")).unwrap()).unwrap();
    let mut track=value["tracks"][0].clone();track["track_id"]="other-track".into();track["name"]="Other language".into();
    for item in track["items"].as_array_mut().unwrap() {item["item_id"]=format!("other-{}",item["item_id"].as_str().unwrap()).into();}
    value["tracks"].as_array_mut().unwrap().push(track);
    let initial:crate::modules::domain::Project=serde_json::from_value(value).unwrap();
    let drafts:serde_json::Value=serde_json::from_slice(&fs::read(root.join("artifacts/background/caption-scoped-drafts.json")).unwrap()).unwrap();
    let operations:Vec<_>=drafts["text"].as_array().unwrap().iter().enumerate().map(|(i,d)|serde_json::json!({"operation_id":format!("scoped-{i}"),"operation":{"type":"ClipTextSet","payload":{"item_id":d["item_id"],"text":d["text"]}}})).collect();
    let envelope:CommandEnvelope=serde_json::from_value(serde_json::json!({"schema_version":initial.schema_version,"project_id":initial.project_id,"base_revision":initial.revision,"transaction_id":"scoped-caption","operations":operations})).unwrap();
    let mut service=CommandService::new(initial.clone());let receipt=service.execute(envelope).unwrap();assert_eq!(receipt.operations_applied,1);
    let changed=service.project().clone();assert_eq!(changed.tracks[0],initial.tracks[0]);assert_eq!(changed.tracks[1].items[0].text_content.as_deref(),Some("Hello chao"));
    let mut history=HistoryManager::new(20);history.record_edit(HistoryEntry{description:"Scoped captions".into(),transaction_id:receipt.transaction_id,prior_snapshot:initial.clone(),resulting_snapshot:changed.clone()});
    let mut current=changed;assert!(history.undo(&mut current).is_some());assert_eq!(current.tracks,initial.tracks);println!("SCOPED_CAPTION_APPLY_UNDO_PASS");
}

#[test]
#[ignore = "requires fresh headless draft order report"]
fn ui_order_deterministic_plan_apply_undo() {
    use crate::modules::{automation::edit_plan::{build_deterministic_video_draft,validate_edit_plan},domain::{Asset,MediaKind,Project,time::{RationalTime,FrameRate}}};
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let report:serde_json::Value=serde_json::from_slice(&fs::read(root.join("artifacts/background/draft-order/report.json")).unwrap()).unwrap();
    assert_eq!(report["status"],"passed");let ids:Vec<String>=serde_json::from_value(report["requests"][0].clone()).unwrap();assert_eq!(ids,["a","c","b"]);
    let dir=std::env::temp_dir().join(format!("draft-order-{}",uuid::Uuid::new_v4()));fs::create_dir(&dir).unwrap();let mut initial=Project::new("Ordered draft".into());
    for id in ["a","b","c"] {let path=if let Ok(source)=std::env::var("TINIX_DRAFT_REAL_DIR") {PathBuf::from(source).join(match id {"a"=>"misty.webm","b"=>"stream.webm",_=>"grasshopper.webm"})} else if let Ok(source)=std::env::var("TINIX_DRAFT_REAL_MEDIA") {PathBuf::from(source)} else {let path=dir.join(format!("{id}.mp4"));fs::write(&path,b"planner fixture").unwrap();path};initial.assets.push(Asset{asset_id:id.into(),name:match id {"a"=>"Misty river","b"=>"Flowing stream",_=>"Grasshopper"}.into(),path:path.to_string_lossy().into(),media_kind:MediaKind::Video,duration:Some(RationalTime::new(300,FrameRate::FPS_30)),file_size:fs::metadata(&path).unwrap().len(),dimensions:None});}
    let plan=build_deterministic_video_draft(&initial,"A 15-second nature montage: misty river, grasshopper, flowing stream",&ids,450).unwrap();let validation=validate_edit_plan(&initial,&plan);assert!(validation.valid);fs::write(root.join("artifacts/background/draft-real/receipt.json"),serde_json::to_vec_pretty(&serde_json::json!({"provider":"deterministic-mock-v1","plan":plan,"validation":validation})).unwrap()).unwrap();
    let envelope:CommandEnvelope=serde_json::from_value(serde_json::json!({"schema_version":initial.schema_version,"project_id":initial.project_id,"base_revision":initial.revision,"transaction_id":"ordered-draft","operations":plan.operations})).unwrap();
    let mut service=CommandService::new(initial.clone());let receipt=service.execute(envelope).unwrap();let applied=service.project().clone();
    let clips=&applied.tracks.iter().find(|t|t.name=="Draft Video").unwrap().items;assert_eq!(clips.iter().map(|i|i.asset_id.as_deref().unwrap()).collect::<Vec<_>>(),vec!["a","c","b"]);
    let mut history=HistoryManager::new(20);history.record_edit(HistoryEntry{description:"Ordered draft".into(),transaction_id:receipt.transaction_id,prior_snapshot:initial.clone(),resulting_snapshot:applied.clone()});let mut current=applied;assert!(history.undo(&mut current).is_some());assert_eq!(current.tracks,initial.tracks);assert_eq!(current.settings,initial.settings);fs::remove_dir_all(dir).unwrap();println!("UI_DRAFT_ORDER_APPLY_UNDO_PASS");
}

#[test]
#[ignore = "requires generated audio mix fixture"]
fn mix_real_levels_evidence() {
 let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/background/mix-review");
 let levels=crate::modules::media::ffmpeg::FFmpegService::analyze_audio_levels(&root.join("music.wav")).unwrap();
 fs::write(root.join("real-levels.json"),serde_json::to_vec_pretty(&levels).unwrap()).unwrap();
 assert!(levels.mean_dbfs.is_some() && levels.peak_dbfs.is_some());
}

#[test]
#[ignore = "requires fresh audio mix plan and fixtures"]
fn real_audio_mix_plan_apply_undo() {
 use crate::modules::domain::{Project,Asset,MediaKind,Track,TrackKind,TimelineItem,ItemKind,time::{RationalTime,FrameRate,TimeRange}};
 let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/background/mix-review");let mut initial=Project::new("Audio mix".into());
 for (id,file,kind) in [("voice","voice.mp4",MediaKind::Video),("music","music.wav",MediaKind::Audio)] {let path=root.join(file);initial.assets.push(Asset{asset_id:id.into(),name:id.into(),path:path.to_string_lossy().into(),media_kind:kind,duration:Some(RationalTime::new(if id=="voice" {90} else {360},FrameRate::FPS_30)),file_size:fs::metadata(path).unwrap().len(),dimensions:None});}
 for (id,kind,start,duration) in [("voice",ItemKind::Video,90,90),("music",ItemKind::Audio,0,360)] {let mut track=Track::new(id.into(),if id=="voice" {TrackKind::Video} else {TrackKind::Audio});let range=|v,d|TimeRange::new(RationalTime::new(v,FrameRate::FPS_30),RationalTime::new(d,FrameRate::FPS_30));let mut item=TimelineItem::new(Some(id.into()),id.into(),kind,range(start,duration),range(0,duration)).unwrap();item.item_id=id.into();track.items.push(item);initial.tracks.push(track);}
 let receipt:serde_json::Value=serde_json::from_slice(&fs::read(root.join("real-plan.json")).unwrap()).unwrap();let operations:Vec<_>=receipt["plan"]["operations"].as_array().unwrap().iter().map(|v|serde_json::json!({"operation_id":v["operationId"],"operation":v["operation"]})).collect();assert!(!operations.is_empty());
 let envelope:CommandEnvelope=serde_json::from_value(serde_json::json!({"schema_version":initial.schema_version,"project_id":initial.project_id,"base_revision":initial.revision,"transaction_id":"audio-mix","operations":operations})).unwrap();let mut service=CommandService::new(initial.clone());let result=service.execute(envelope).unwrap();let applied=service.project().clone();assert_eq!(applied.tracks[0],initial.tracks[0]);assert_ne!(applied.tracks[1],initial.tracks[1]);let mut history=HistoryManager::new(20);history.record_edit(HistoryEntry{description:"Audio mix".into(),transaction_id:result.transaction_id,prior_snapshot:initial.clone(),resulting_snapshot:applied.clone()});let mut current=applied;assert!(history.undo(&mut current).is_some());assert_eq!(current.tracks,initial.tracks);println!("REAL_LEVEL_MIX_APPLY_UNDO_PASS");
}

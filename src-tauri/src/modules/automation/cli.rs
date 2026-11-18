//! JSON commands over the shared automation service.
use super::operations::{AutomationOperations, OperationContext, OperationError};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

async fn invoke(
    service: &AutomationOperations,
    request: Value,
    context: OperationContext,
) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let name = request.get("command").and_then(Value::as_str).unwrap_or("");
    let arguments = request.get("arguments").cloned().unwrap_or(json!({}));
    match service.execute(name, arguments, context).await {
        Ok(result) => json!({"id": id, "result": result}),
        Err(OperationError::InvalidParams(message)) if !message.starts_with("Unknown command:") => {
            json!({"id": id, "result": {"resultType": "complete", "content": [{"type": "text", "text": format!("failed to deserialize parameters: {message}")}], "isError": true}})
        }
        Err(OperationError::InvalidParams(message)) => {
            json!({"id": id, "error": {"code": -32602, "message": message}})
        }
        Err(OperationError::Internal(message)) => {
            json!({"id": id, "error": {"code": -32603, "message": message}})
        }
    }
}

pub async fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service = AutomationOperations::new();
    if args.first().map(String::as_str) == Some("commands") {
        let mut schemas = AutomationOperations::command_schemas();
        for schema in &mut schemas {
            if let Some(input) = schema["inputSchema"].as_object_mut() {
                input.remove("title");
            }
        }
        let selected = match args.get(1) {
            Some(name) => schemas
                .into_iter()
                .filter(|s| s["name"] == name.trim_start_matches("tinixcut_"))
                .collect::<Vec<_>>(),
            None => schemas,
        };
        if selected.is_empty() {
            return Err("Unknown command".into());
        }
        println!("{}", serde_json::to_string(&selected)?);
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("call") {
        let name = args
            .get(1)
            .ok_or("Usage: tinixcut call <command> [arguments.json|-]")?;
        if name.trim_start_matches("tinixcut_") == "export_start" {
            return Err("export_start requires api-session so its job remains alive; use render for a blocking one-shot export".into());
        }
        let input = match args.get(2).map(String::as_str) {
            None => "{}".to_string(),
            Some("-") => std::io::read_to_string(std::io::stdin())?,
            Some(path) => std::fs::read_to_string(path)?,
        };
        let response = invoke(
            &service,
            json!({"command": name, "arguments": serde_json::from_str::<Value>(&input)?}),
            OperationContext::default(),
        )
        .await;
        println!("{}", response);
        service.shutdown_and_cleanup();
        if response.get("error").is_some() || response["result"]["isError"] == true {
            return Err("Command failed; see JSON response".into());
        }
        return Ok(());
    }
    // A persistent process owns jobs and cancellation tokens. Responses can arrive out of order.
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    let writer = tokio::spawn(async move {
        let mut out = tokio::io::stdout();
        while let Some(value) = rx.recv().await {
            out.write_all(format!("{value}\n").as_bytes()).await?;
            out.flush().await?;
        }
        Ok::<_, std::io::Error>(())
    });
    let mut active = std::collections::HashMap::new();
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        let line = tokio::select! {
            line = lines.next_line() => match line? { Some(line) => line, None => break },
            completed = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(completed) = completed {
                    let (key, response) = completed?;
                    active.remove(&key);
                    tx.send(response)?;
                }
                continue;
            }
        };
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tx.send(json!({"error": {"code": "INVALID_JSON", "message": e.to_string()}}))?;
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if request["command"] == "cancel" {
            let key = request["arguments"]["id"].to_string();
            let cancelled = if let Some(ct) = active.get(&key) {
                tokio_util::sync::CancellationToken::cancel(ct);
                true
            } else {
                false
            };
            tx.send(json!({"id": id, "result": {"cancelled": cancelled}}))?;
            continue;
        }
        let key = id.to_string();
        if id.is_null() || active.contains_key(&key) {
            tx.send(json!({"id": id, "error": {"code": "INVALID_ID", "message": "Supply a unique non-null request id"}}))?;
            continue;
        }
        if active.len() >= 64 {
            tx.send(json!({"id": id, "error": {"code": "BUSY", "message": "At most 64 active requests"}}))?;
            continue;
        }
        let context = OperationContext::default();
        active.insert(key.clone(), context.ct.clone());
        let service = service.clone();
        tasks.spawn(async move {
            let response = invoke(&service, request, context).await;
            (key, response)
        });
    }
    for ct in active.values() {
        ct.cancel();
    }
    while let Some(completed) = tasks.join_next().await {
        let (_, response) = completed?;
        tx.send(response)?;
    }
    service.shutdown_and_cleanup();
    drop(tx);
    writer.await??;
    Ok(())
}

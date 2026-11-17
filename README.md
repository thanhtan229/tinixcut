# TinixCut

Automation-first desktop video editor for Windows (Tauri 2 + React/TypeScript + Rust).
The versioned project model is edited only through the Command Service, so the UI and CLI share one mutation path.

## Run

```bash
pnpm install
pnpm tauri dev      # development app
pnpm tauri build    # NSIS installer (src-tauri/target/release/bundle/nsis)
```

CLI (the same binary): `tinixcut inspect|diff|apply|render|session|commands|call|api-session`.

## Verify

```bash
pnpm typecheck && pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
```

Native qualification harness: `scripts/qualification/native/README.md`.

## Where to read

| Topic | Document |
|---|---|
| Release scope, gate status, non-claims | [RELEASE_READINESS.md](RELEASE_READINESS.md) |
| Current checkpoint | [PROJECT_STATUS.md](PROJECT_STATUS.md) |
| Architecture / plan / workflow | [ARCHITECTURE.md](ARCHITECTURE.md), [PLAN.md](PLAN.md), [DEVELOPMENT_WORKFLOW.md](DEVELOPMENT_WORKFLOW.md) |
| Users | [USER_GUIDE.md](USER_GUIDE.md), [TUTORIAL.md](TUTORIAL.md) |


### Complete JSON CLI surface

- `tinixcut commands` lists all shared operations with descriptions and JSON schemas. `tinixcut commands media_probe` retrieves one schema.
- `tinixcut call <command> [arguments.json|-]` executes an operation; omit arguments for `{}` or use `-` to read JSON from stdin. Command names also accept the `tinixcut_` prefix. Failure exits nonzero.
- `tinixcut api-session` accepts JSONL `{ "id": 1, "command": "capabilities", "arguments": {} }` and returns `{id,result}` or `{id,error}`. Results preserve `isError`, content and structuredContent. Responses can arrive out of order. IDs must be unique while active. Keep stdin open until responses arrive; EOF cancels requests and export jobs.
- Cancel a request using `{ "id": 2, "command": "cancel", "arguments": { "id": 1 } }`. Export jobs use `export_start`, `export_status`, `export_cancel` in the same persistent session. One-shot `call export_start` is rejected because that process cannot retain the job; use `render` for blocking export.
- Explicit project paths, revision checks and caller-supplied durable command identity follow the discovered schemas. All 36 automation operations are available through the CLI.
- MCP has been retired. Automation clients should discover schemas with `tinixcut commands` and use `tinixcut api-session` for persistent JSONL requests and cancellation.

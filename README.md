# TinixCut

Automation-first desktop video editor for Windows (Tauri 2 + React/TypeScript + Rust).
The versioned project model is edited only through the Command Service, so the UI, CLI and MCP server share one mutation path.

## Run

```bash
pnpm install
pnpm tauri dev      # development app
pnpm tauri build    # NSIS installer (src-tauri/target/release/bundle/nsis)
```

CLI (the same binary): `tinixcut inspect|diff|apply|render|session|mcp`.

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

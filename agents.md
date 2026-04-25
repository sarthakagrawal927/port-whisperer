# agents.md — port-whisperer

## Purpose
A developer-friendly Rust CLI (`ports`) that lists listening ports with framework/process detection, and can open, free, inspect, watch, and export port info.

## Stack
- Framework: None (Rust CLI binary)
- Language: Rust (edition 2024)
- Styling: N/A
- DB: None
- Auth: None
- Testing: Rust integration tests (`tests/integration_tests.rs`)
- Deploy: GitHub Actions release workflow (`.github/workflows/release.yml`); install via `install.sh`
- Package manager: Cargo

## Repo structure
```
src/
  main.rs       CLI entry — arg parsing and command dispatch
  scanner.rs    Port scanning logic (reads /proc or lsof/netstat on macOS)
  display.rs    Colored table output and help text
tests/
  integration_tests.rs  Integration tests
docs/
  index.html    Static landing/docs page
install.sh      Curl-pipe installer script
Cargo.toml      Binary named `ports`; deps: clap, colored, serde_json, comfy-table
Cargo.lock
```

## Key commands
```bash
cargo build             # Debug build
cargo build --release   # Release build (output: target/release/ports)
cargo run               # Run (lists ports, no args)
cargo run -- --all      # Show all ports including privileged
cargo run -- <port>     # Inspect a specific port
cargo run -- open <port>
cargo run -- free <port>
cargo run -- watch      # Live-refresh mode
cargo run -- json       # JSON output
cargo run -- ps         # Process list mode
cargo run -- clean      # Kill all dev ports
cargo run -- log [port] # Log mode
cargo test              # Run all tests (unit + integration)
```

## Architecture notes
- Binary name is `ports` (not `port-whisperer`) — set via `[[bin]]` in Cargo.toml.
- Three modules: `main` (dispatch), `scanner` (data collection), `display` (rendering).
- Uses `clap` for arg parsing (derive feature), `colored` for terminal colors, `comfy-table` for table rendering, `serde_json` for JSON output.
- macOS: uses `lsof` or `netstat` under the hood (check `scanner.rs` for exact syscall strategy).
- Release pipeline in `.github/workflows/release.yml` — creates GitHub releases with pre-built binaries.
- `install.sh` is a curl-pipe script for end-user installation.

## Active context

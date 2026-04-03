# port-whisperer (Rust)

Developer-friendly port scanner with framework detection, Docker awareness, and process health monitoring.

Rust port of [LarsenCundric/port-whisperer](https://github.com/LarsenCundric/port-whisperer) — single static binary, zero runtime dependencies.

> Written by [Claude Code](https://claude.ai/claude-code) (Anthropic's AI coding agent).

## Install

```bash
cargo install --git https://github.com/sarthakagrawal927/port-whisperer
```

Or build from source:

```bash
git clone https://github.com/sarthakagrawal927/port-whisperer
cd port-whisperer
cargo install --path .
```

## Usage

```
ports              Show dev server ports (filtered)
ports --all        Show all listening ports
ports <port>       Inspect a specific port (+ interactive kill)
ports open <port>  Open localhost:<port> in browser
ports free <port>  Kill whatever's on that port (SIGTERM → SIGKILL)
ports json         JSON output for scripting
ports json --all   JSON output including system ports
ports log          Show port history
ports log <port>   History for a specific port
ports ps           Show running dev processes sorted by CPU%
ports ps --all     Show all processes
ports clean        Find & kill orphaned/zombie dev processes
ports watch        Monitor port changes in real-time
ports help         Show help
```

## Example

```
╭────────┬───────┬───────────┬───────────┬─────────┬─────────────┬────────╮
│ PORT   ┆ PID   ┆ PROCESS   ┆ FRAMEWORK ┆ PROJECT ┆ HEALTH      ┆ UPTIME │
╞════════╪═══════╪═══════════╪═══════════╪═════════╪═════════════╪════════╡
│ :3000  ┆ 12345 ┆ node      ┆ Next.js   ┆ my-app  ┆ ●  healthy  ┆ 2h15m  │
├╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┤
│ :5432  ┆ 789   ┆ postgres  ┆ PostgreSQL┆         ┆ ●  healthy  ┆ 5d3h   │
╰────────┴───────┴───────────┴───────────┴─────────┴─────────────┴────────╯
```

## Features

- **Framework detection** — identifies Next.js, Vite, Express, Django, Rails, FastAPI, and 30+ others via command-line inspection, `package.json` dependencies, and config files
- **Docker awareness** — maps host ports to container names and images
- **Project resolution** — walks up from process cwd to find project root (`package.json`, `Cargo.toml`, `go.mod`, etc.)
- **Process health** — color-coded: green (healthy), yellow (orphaned), red (zombie). Only dev servers with PPID 1 are flagged as orphaned — daemons, apps, and Homebrew services are correctly classified as healthy
- **Smart filtering** — hides system apps (Spotify, Chrome, Slack, Warp, etc.) by default; `--all` shows everything
- **Graceful kill** — always tries SIGTERM first, waits up to 2s, then SIGKILL as fallback
- **Quick free** — `ports free 3000` kills whatever's on that port, no questions asked
- **Browser open** — `ports open 3000` opens localhost in your default browser
- **JSON output** — `ports json` for scripting and piping to `jq`; all fields included (port, pid, framework, health, memory, uptime, command, cwd, docker info)
- **Port history** — every `ports` scan logs a timestamped snapshot to `~/.ports-history/`; `ports log` shows the history, `ports log <port>` filters to a specific port
- **Watch mode** — real-time monitoring of port open/close events (polls every 2s)

## How it works

Three batched shell calls, not N per-process:

1. `lsof -iTCP -sTCP:LISTEN` — find all listening ports
2. `ps -p <pids> -o pid,ppid,stat,rss,lstart,command` — single call for all PIDs
3. `lsof -a -d cwd -p <pids>` — resolve working directories for project detection

Framework detection is layered: known server processes first, then command-line keywords, then `package.json` dependencies, config files, Docker image names, and finally process name fallback.

## Testing

```bash
cargo test
```

57 tests: 36 unit tests (etime parsing, system app detection, orphan classification, framework detection, JSON output, log filtering) + 21 integration tests covering every command and edge case.

## Platform

macOS only (uses `lsof` and `ps`).

## Credits

- Original: [LarsenCundric/port-whisperer](https://github.com/LarsenCundric/port-whisperer) (Node.js)
- Rust port written by [Claude Code](https://claude.ai/claude-code) (Anthropic)

## License

MIT

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
ports ps           Show running dev processes sorted by CPU%
ports ps --all     Show all processes
ports clean        Find & kill orphaned/zombie processes
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
- **Process health** — color-coded: green (healthy), yellow (orphaned), red (zombie)
- **Smart filtering** — hides system apps (Spotify, Chrome, Slack, etc.) by default
- **Interactive kill** — inspect any port and kill the process with a prompt
- **Watch mode** — real-time monitoring of port open/close events

## Platform

macOS only (uses `lsof` and `ps`).

## Credits

- Original: [LarsenCundric/port-whisperer](https://github.com/LarsenCundric/port-whisperer) (Node.js)
- Rust port written by [Claude Code](https://claude.ai/claude-code) (Anthropic)

## License

MIT

# Architecture Decision Records — port-whisperer

Canonical design decisions for the `ports` CLI. Lessons that emerged from these decisions live in [lessons.md](./lessons.md).

---

## ADR-001 · Rust over Node.js / Python / Go

**Date:** 2026-04-04
**Context:** The original [LarsenCundric/port-whisperer](https://github.com/LarsenCundric/port-whisperer) is a Node.js CLI. A port was wanted that ships as a single static binary with no runtime dependency.
**Decision:** Rust, with the binary named `ports` via `[[bin]]` in Cargo.toml.
**Rationale:** Single `cargo install` or `curl | sh` install with no interpreter requirement. Rust's `std::process::Command` gives direct, typed access to subprocess spawning without a shell layer.
**Alternatives:**
- Node.js — original, but requires Node on PATH; startup cost matters for a command run many times per day.
- Python — same runtime-dependency problem; packaging as a portable binary is complex.
- Go — single binary too, but the project author is in the Rust ecosystem and Go's cross-compilation story is different.
**Tradeoffs:** Compilation is slow for CI; the project mitigates this by publishing prebuilt binaries via GitHub Actions and an `install.sh` curl-pipe installer.

---

## ADR-002 · Batched lsof / ps calls instead of per-port spawning

**Date:** 2026-04-04
**Context:** Naive approach would spawn one `lsof -i :<port>` per discovered port. With 20+ listening ports, this is 20+ sequential subprocesses.
**Decision:** Three total subprocess calls for any scan, regardless of port count:
1. `lsof -iTCP -sTCP:LISTEN -P -n` — all listening ports in one shot.
2. `ps -p <comma-joined-pids> -o pid,ppid,stat,rss,lstart,command` — all PIDs in one call.
3. `lsof -a -d cwd -p <pids>` — all working directories in one call.
**Rationale:** Subprocess spawn cost on macOS (~30–50ms each) dominates scan time. Batching reduces it to a constant three calls. README documents this architecture explicitly.
**Alternatives:** Per-port `lsof -i :<port>` — simple but O(n) spawns.
**Tradeoffs:** All parsing must happen in Rust. The `lsof` output format (columnar, whitespace-delimited, with quirks for truncated names and IPv6) has to be handled in one place rather than per-process.

---

## ADR-003 · Framework detection: command-line heuristic first, package.json second

**Date:** 2026-04-04
**Context:** Two sources of truth available: the process command line (always available) and `package.json` / config files (only for Node/JS projects in the project root).
**Decision:** Detection is layered, tried in order:
1. Known server process names (`mysqld`, `redis-server`, etc.) — exact process name match.
2. Command-line keyword matching (`next dev`, `vite`, `flask`, `django`, etc.) — ~35 patterns, ordered most-specific first.
3. `package.json` dependency inspection — reads the file, parses JSON, checks `dependencies` + `devDependencies` keys.
4. Config-file presence (`vite.config.ts`, `next.config.js`, `Cargo.toml`, etc.).
5. Docker image name matching.
6. Process name fallback (`node` → "Node.js", `python` → "Python", etc.).
**Rationale:** Command-line is cheapest and most reliable for the common case. `package.json` handles monorepos where a `node` process doesn't have the framework name in its argv. Config-file detection catches projects where the dev script doesn't name the framework. Docker image names cover containerized services.
**Alternatives:** Only read `package.json` — misses non-JS frameworks entirely and requires cwd resolution first.
**Tradeoffs:** False positives are possible (e.g., `engine` contains `gin` in a MySQL flag; see test `test_detect_gin_not_false_positive`). Known-servers table runs first to short-circuit these cases. TBD: capture any additional false-positive patterns that emerged in practice.

---

## ADR-004 · comfy-table over manual ANSI formatting

**Date:** 2026-04-04
**Context:** Output needs column alignment, colored cells, and Unicode box-drawing characters.
**Decision:** Use `comfy-table` with the `UTF8_FULL` preset and `UTF8_ROUND_CORNERS` modifier.
**Rationale:** comfy-table handles `ContentArrangement::Dynamic` (columns resize to terminal width), Unicode box-drawing, and per-cell color attributes. Writing equivalent logic by hand would require tracking column widths, handling Unicode character widths, and managing ANSI escape codes separately.
**Alternatives:** Manual ANSI + fixed column widths — brittle for variable-length values like framework names and project paths.
**Tradeoffs:** Adds a binary dependency. The package is small and compilation is fast. comfy-table doesn't expose the raw terminal width for other uses, but that's not needed here.

---

## ADR-005 · Orphan detection: conservative PPID-1 heuristic

**Date:** 2026-04-04 (initial); revised 2026-04-04 (commit `717d4d7`)
**Context:** A process is "orphaned" when its parent exited (e.g., terminal closed while `npm run dev` was running) and it was re-parented to PID 1 (launchd on macOS). But on macOS, PPID 1 is also normal for every user app launched from the Dock, every Homebrew-managed daemon (Redis, MySQL, Postgres), and `/Applications/*` binaries.
**Decision:** A PPID-1 process is only flagged as `Orphaned` if it matches dev-server indicators (`node`, `python`, `cargo`, `npm`, `vite`, etc.) AND does NOT match known exceptions:
- Known daemon names (`mysqld`, `postgres`, `redis-server`, `nginx`, etc.)
- Paths under `/Applications/`
- Paths under `/opt/homebrew/` or `/usr/local/Cellar/`
**Rationale:** The first version flagged everything with PPID 1, which caused `ports clean` to offer killing Warp, MySQL, and Redis. The conservative heuristic avoids killing legitimate services at the cost of possibly missing unusual runtimes.
**Alternatives:** Check if the process has a controlling terminal (STAT column `s` flag) — more accurate but requires additional `ps` field and parsing logic.
**Tradeoffs:** Unusual runtimes (JVM apps without `java` in the name, custom launchers) may not be flagged. The README documents this explicitly: "better to miss an orphan than kill a legit process."

---

## ADR-006 · Subprocess timeout + stuck-lsof guard

**Date:** 2026-05-23 (commit `b94a9d6`)
**Context:** macOS `lsof` can wedge inside `close()` syscall when orphaned Network Extension (utun) sockets exist — typically from crashed VPN apps (Tailscale, WireGuard, Cloudflare WARP). The process becomes unkillable (SIGKILL queued but never delivered while in kernel mode), and any subsequent `lsof` spawn hangs indefinitely, eventually stacking up leaked processes.
**Decision:** Two-layer defense:
1. **Timeout in `run_cmd`** — every subprocess has a 5s deadline. A background thread drains stdout to prevent pipe-backpressure deadlock. On timeout, the child is killed (best-effort), a process-scoped `SUBPROC_STUCK` atomic flag is latched, and subsequent subprocess calls in the same scan are no-ops.
2. **Pre-flight check** — before any `lsof` scan, `check_subproc_health()` calls `pgrep` (safe, doesn't hit the bad code path) to detect leaked `lsof` processes from prior runs. If any have been alive >30s, the scan is refused with a human-readable message instructing the user to reboot.
**Rationale:** No other fix exists — SIGKILL cannot reap a thread stuck in kernel mode. Leaking more `lsof` processes makes recovery harder.
**Alternatives:** Retry with `netstat` as fallback — TBD: this wasn't implemented; `netstat` has different output format on macOS and Linux.
**Tradeoffs:** A scan that hits the timeout returns empty results with a warning, which is worse than partial results. The `ports doctor` command exists for diagnosis.

---

## ADR-007 · IPv4 and IPv6 unified via rsplit(':') port extraction

**Date:** 2026-04-04
**Context:** `lsof -iTCP` output shows the address field in two formats: `*:3000` (IPv4) and `[::]:3000` (IPv6, with brackets). Both need to produce port `3000`.
**Decision:** Extract port using `name_col.rsplit(':').next()` — split from the right, take the last segment.
**Rationale:** `rsplit(':')` naturally handles both `*:3000` (last segment = `3000`) and `[::]:3000` (last segment = `3000`). A left-split would break on IPv6 addresses since the address itself contains colons.
**Tradeoffs:** If lsof ever emits a format with a port range or additional colon-suffixed metadata, the parse would silently produce a wrong value. This hasn't occurred in practice; the `name_col.rsplit(':').next()` pattern is idiomatic for this use case.

---

## ADR-008 · Custom arg parsing over clap subcommands

**Date:** 2026-04-04
**Context:** Cargo.toml lists `clap` as a dependency (derive feature), but `main.rs` uses a hand-rolled `split_flags` + `match args.first()` dispatch instead of clap's derive macros.
**Decision:** Manual arg parsing. Clap is listed as a dependency but is not invoked at runtime.
**Rationale:** TBD: capture rationale. The `run` subcommand requires a passthrough mode where everything after `run` is the user's command and must not be interpreted as flags — this is easier with a hand-rolled parser than with clap's derive macros. The `split_flags` function explicitly handles the passthrough sentinel.
**Tradeoffs:** Help text is manually maintained in `display::print_help()`. Error messages for unknown commands fall through to a generic `eprintln!`. Clap's dependency is dead weight in the binary.

---

## ADR-009 · Log format: tab-delimited, port field as `:PORT\t`

**Date:** 2026-04-04 (revised from earlier approach per commit `6a8ec58`)
**Context:** The history log at `~/.ports-history/history.log` needs to be filterable by port number. A naive `line.contains(":80")` would match `:8080` entries.
**Decision:** Log each entry tab-delimited as `timestamp\t:PORT\tpid\tname\tframework\thealth`. Filter by constructing the exact string `\t:PORT\t` and using `line.contains(...)`.
**Rationale:** Tab delimiter makes the `:PORT` field unambiguous — `:80` surrounded by tabs cannot match `:8080`. The fix is documented in commit `6a8ec58` ("Fix log filter") and validated by unit tests `test_log_filter_exact_port_match` and `test_log_filter_33060_vs_3306`.
**Tradeoffs:** Log format is not self-describing; anyone parsing it externally needs to know the column order. A JSON-per-line format would be unambiguous but larger.

# New things to learn — port-whisperer

Novel techniques and surprises from building a macOS/Linux port-scanner CLI in Rust.

---

## Rust: shelling out with `std::process::Command`
- What: Spawn subprocesses synchronously, capturing stdout as bytes, without a shell layer.
- Why here: TBD
- Gotcha (from code): Every lsof/ps/docker call goes through `run_cmd` → `run_cmd_timeout` in `src/scanner.rs:111-185`. Spawning with `.stdout(Stdio::piped())` is mandatory; without it there is no pipe to drain and back-pressure will deadlock.
- Source: https://doc.rust-lang.org/std/process/struct.Command.html

---

## Pipe back-pressure deadlock
- What: If the parent never drains a child's stdout pipe, the child blocks on a full pipe buffer (~64 KB) and never exits.
- Why here: TBD
- Gotcha (from code): Fix is a background thread draining stdout via `mpsc::channel` while the main thread polls `try_wait()` — `src/scanner.rs:140-184`. The `mpsc` channel lets the drain thread send the accumulated string back once the child exits; the main thread does `rx.recv_timeout(200ms)` after the loop.
- Source: https://doc.rust-lang.org/std/process/struct.ChildStdout.html (verified live)

---

## `lsof` output quirks (truncation, IPv6, column count)
- What: `lsof` truncates COMMAND to ~9 chars, wraps IPv6 listeners as `[::]:PORT`, and emits variable column counts on continuation lines.
- Why here: TBD
- Gotcha (from code): IPv6 port extraction requires `rsplit(':').next()` — a left-split returns `[` from the address (`src/scanner.rs:441`). Truncation is handled by `is_system_app` matching both directions with `app.starts_with(name)` (`src/scanner.rs:807`). See ADR-007 in docs/archive/decisions.md.
- Source: https://man7.org/linux/man-pages/man8/lsof.8.html — see external-references.md

---

## `lsof` kernel hang from orphaned utun sockets
- What: macOS `lsof` can get stuck forever inside a kernel `close()` syscall when a crashed VPN (Tailscale, WARP) leaves a utun socket open; SIGKILL is queued but never processed.
- Why here: TBD
- Gotcha (from code): No software fix exists — the only recovery is a reboot. `run_cmd_timeout` detects the hang via a 5 s deadline, kills the child (which fails silently), latches `SUBPROC_STUCK` so no further subprocs spawn, and prints the reboot advisory (`src/scanner.rs:159-175`). `leaked_lsofs()` (`src/scanner.rs:191-226`) uses `pgrep` + `ps etime` to find already-stuck survivors from prior runs; `check_subproc_health()` (`src/scanner.rs:404-420`) gates every scan. `ports doctor` runs all four checks. See ADR-006 in docs/archive/decisions.md.
- Source: https://developer.apple.com/documentation/networkextension — see external-references.md (Apple TN3178 link still TBD)

---

## `ps` time-format quirks (`lstart`, `etime`)
- What: `ps -o lstart` emits a 5-token string (`Thu Jan  2 15:04:05 2025`); `ps -o etime` is variable-width with format `[[dd-]hh:]mm:ss`.
- Why here: TBD
- Gotcha (from code): `lstart` cannot be parsed with a simple field index — tokens 4..9 must be joined (`src/scanner.rs:486`). `parse_etime` (`src/scanner.rs:1206-1233`) handles all three width variants (days, hours, minutes-only) and is covered by 5 unit tests.
- Source: https://man7.org/linux/man-pages/man1/ps.1.html — see external-references.md

---

## PPID 1 on macOS is not "orphaned"
- What: On macOS, launchd (PID 1) is the parent of every Dock-launched app and every Homebrew daemon — not just orphaned processes.
- Why here: TBD
- Gotcha (from code): First version flagged Redis, MySQL, and Warp as orphans. Fix (`is_likely_orphaned_dev_process`, `src/scanner.rs:812-851`) requires PPID-1 AND dev-server keyword match AND not on the daemon/Applications/Homebrew allowlist; 9 unit tests cover the exact cases (e.g. `test_warp_not_orphaned`, `test_mysql_not_orphaned`). See ADR-005 in docs/archive/decisions.md.
- Source: https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/Introduction.html

---

## `clap` declared but never called
- What: `clap` (derive feature) is listed in `Cargo.toml:13` but `main.rs` has zero `use clap` imports or invocations — confirmed by code search. A hand-rolled `split_flags` + `match` parser is used instead (`src/main.rs:74-98`).
- Why here: TBD
- Gotcha (from code): The `run` subcommand must treat everything after `run` as verbatim passthrough (`src/main.rs:72-73` doc comment, `src/main.rs:92-94` passthrough latch). Clap derive macros consume all args eagerly and have no direct equivalent to this mode. `clap` currently bloats the binary with dead code; see ADR-008 in external-references.md.
- Source: https://docs.rs/clap/latest/clap/ (verified live — derive feature confirmed)

---

## `CARGO_BIN_EXE_<name>` test-harness trick
- What: `env!("CARGO_BIN_EXE_ports")` in integration tests resolves to the absolute path of the already-compiled binary — set by Cargo only when building integration tests or benchmarks.
- Why here: TBD
- Gotcha (from code): Original tests called `cargo run --release --` per test case; 21 parallel tests triggered 21 simultaneous `cargo` invocations fighting the build lock. Fixed in commit `b94a9d6`. Usage is in `tests/integration_tests.rs:4`.
- Source: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates (verified live)

---

## Log filter precision: tab-delimited port field
- What: Framing a port number with tab characters (`\t:80\t`) makes substring match unambiguous — `:80` cannot match `:8080`.
- Why here: TBD
- Gotcha (from code): The regression (`line.contains(":80")` matching `:8080`) was caught by tests `test_log_filter_exact_port_match` and `test_log_filter_33060_vs_3306` (`src/scanner.rs:1569,1635`). Log lines are written tab-separated in `log_snapshot` (`src/scanner.rs:1167-1173`); the filter constructs `"\t:PORT\t"` (`src/scanner.rs:1186`). See ADR-009 in docs/archive/decisions.md.
- Source: No external source needed — this is a first-principles string-framing technique.

---

## `comfy-table` for dynamic terminal output
- What: Crate for columnar terminal output with Unicode box-drawing, per-cell color, and `ContentArrangement::Dynamic` (resizes to terminal width).
- Why here: TBD
- Gotcha (from code): `UTF8_FULL` preset + `UTF8_ROUND_CORNERS` modifier + `ContentArrangement::Dynamic` are combined on every table in `src/display.rs:16-29`. All three APIs are confirmed in comfy-table 7.2.2 docs.
- Source: https://docs.rs/comfy-table/latest/comfy_table/ (verified live — v7.2.2, all three APIs confirmed) — see external-references.md

---

## Framework detection heuristics (layered)
- What: Detect which framework owns a port using 6 layers: known daemon names → argv keywords → `package.json` deps → config-file presence → Docker image name → process name fallback.
- Why here: TBD
- Gotcha (from code): Leading-slash prefix (`"/gin"` at `src/scanner.rs:976`) is the only thing preventing MySQL's `--default-storage-engine=InnoDB` from matching the Go Gin framework — verified by `test_detect_gin_not_false_positive` (`src/scanner.rs:1463`). Layers 3-4 require `project_root` to be resolved first via `find_project_root` (`src/scanner.rs:865`). See ADR-003 in docs/archive/decisions.md.
- Source: See external-references.md

---

## `CommandExt::exec` — true process replacement (Unix `execvp`)
- What: `std::os::unix::process::CommandExt::exec()` replaces the current process image in-place with the child, rather than forking a subprocess. The calling process's PID, open file-descriptors, and terminal session are inherited by the new program.
- Why here: TBD
- Gotcha (from code): `cmd_run` (`src/main.rs:252`) calls `.exec()` after clearing conflicting ports so the user's dev command owns the shell session cleanly. If `.exec()` fails (program not found etc.) it returns an `io::Error` — the line after it (`src/main.rs:253`) is only reachable on that error path, which is why `exit(127)` follows immediately.
- Source: https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html

---

*See also: [external-references.md](./external-references.md)*

# Lessons — port-whisperer

Concrete gotchas hit during development. Design decisions that produced these lessons live in [decisions.md](./decisions.md).

---

## lsof output quirks

### Process name truncation (~9 chars)
`lsof` truncates the COMMAND column to approximately 9 characters. `redis-server` becomes `redis-ser`, `ControlCenter` becomes `ControlCe`. The framework detection and system-app filtering tables must account for both the full name and the truncated form.

The `is_system_app` check handles this with a two-direction test:
```rust
SYSTEM_APPS.iter().any(|app| name.contains(app) || app.starts_with(name))
```
`app.starts_with(name)` catches `controlcenter` starting with truncated `controlce`.

The unit test `test_system_app_truncated_lsof_name` pins this behavior.

### IPv6 brackets in address column
`lsof -iTCP` emits `[::]:3000` for IPv6 listeners and `*:3000` for IPv4. Port extraction via `rsplit(':').next()` handles both; a naive left-split on `:` would return `[` from the IPv6 address. See ADR-007.

### Column count assumption
The scanner assumes at least 9 columns in `lsof` output (columns 0–8, where column 8 is the address). Lines with fewer columns (header continuation lines, truncated lines) are skipped with `if cols.len() < 9 { continue; }`. This is fragile if `lsof` output format changes.

---

## ps output quirks

### lstart has embedded spaces
`ps -o lstart=` emits `Thu Jan  2 15:04:05 2025` — five whitespace-separated tokens for a single field. When parsing with `split_whitespace()`, tokens 4..9 must be joined to reconstruct the start time. The parser in `scan_ports` handles this but it required a comment (`// lstart is like "Thu Jan  2 15:04:05 2025" - 5 tokens`). Using `splitn` for the whole line doesn't work because `lstart` is embedded, not terminal.

### etime format is variable-width
`ps -o etime=` returns `[[dd-]hh:]mm:ss`. The number of colon-separated segments varies (1 through 3), and the days component uses a dash separator embedded in the first segment (`3-04:15:30`). The `parse_etime` function handles all four variants; tests cover seconds-only, minutes:seconds, hours:minutes:seconds, and days.

### Dead PIDs between lsof and ps
There is a TOCTOU window: `lsof` captures a PID, then the process exits before the batched `ps` call. The scanner handles this gracefully — `ps_data.get(pid)` returns `None` and the code falls back to `unwrap_or_default()` (zeroed-out fields). The port still appears in output with empty command/stat fields.

---

## macOS-specific behaviors

### PPID 1 is not "orphaned" on macOS
On Linux, PPID 1 often means a process was orphaned from its parent. On macOS, launchd (PID 1) is the parent of every user app launched from the Dock, every Homebrew-managed daemon, and every `/Applications/*` binary. The first version of the orphan check flagged all of them. Fixed in commit `717d4d7`: the check now requires both PPID 1 AND dev-server-looking command keywords. See ADR-005.

### lsof can wedge in kernel close() on macOS
When a VPN or Network Extension provider (Tailscale, Cloudflare WARP, WireGuard) crashes without tearing down its `utun` socket, the kernel's `close()` syscall on that fd never returns. Any `lsof` spawned while these orphaned sockets exist gets stuck in an unkillable kernel thread. SIGKILL is delivered but not processed until the thread exits the syscall — which it never does. The only fix is a reboot.

The scanner detects this via `pgrep` (which doesn't traverse fds) and refuses to spawn more `lsof` calls if stuck ones from prior runs are detected. The `ports doctor` command surfaces this diagnosis. See ADR-006.

### utun interface accumulation
Each VPN/NE provider creates a `utun` interface. A clean Mac has 0–4. More than 4 indicates orphaned tunnels from apps that didn't shut down cleanly. `ports doctor` counts them via `ifconfig -l` and warns above threshold 4.

---

## Framework detection false positives

### `/gin` pattern vs. MySQL storage engine
The pattern `"/gin"` (with leading slash, intended to match `/gin` as a Go Gin framework binary path) is present in the framework table. MySQL startup flags like `--default-storage-engine=InnoDB` do not match because `"engine"` does not contain `"/gin"`. However, any binary path containing `/gin` would be incorrectly labeled as Gin. The leading slash is the only disambiguation.

### Process name column 0 in `ps aux` is just the binary
`ps aux` column 10 is the command. `cols[10].rsplit('/').next()` extracts just the filename (e.g., `python3` from `/usr/bin/python3`). A process like `stable` (Warp terminal) will fall through all framework detections to the raw process name fallback. The test `test_warp_not_orphaned` verifies Warp paths under `/Applications/` are not flagged as orphaned, but the framework column will show `stable` (the Warp binary name). This is the documented "falls back to showing the raw process name" behavior.

---

## Rust ergonomics for shelling out

### Pipe back-pressure deadlock
A subprocess writing to stdout faster than the parent reads will block on the pipe buffer (typically 64KB). If the parent is busy-polling `try_wait()` without reading stdout, the child blocks and never exits — the parent loops forever. The fix: spawn a background thread that drains stdout to a `String` via a channel, then `try_wait()` in the main thread until exit or timeout. The background thread's result is received after the child exits.

```rust
let (tx, rx) = mpsc::channel();
if let Some(mut stdout) = child.stdout.take() {
    thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
}
```

### SUBPROC_STUCK atomic prevents cascade hangs
Once one subprocess times out (e.g., `lsof`), subsequent calls in the same scan are skipped via a process-scoped `AtomicBool`. Without this, a single hung `lsof` call would cause the ps and cwd-resolution calls to also spawn and hang, multiplying the damage.

### Integration tests vs. cargo run
The initial integration tests called `cargo run --release --` for every test case. With 21 parallel tests, this caused 21 simultaneous `cargo` invocations fighting for the build lock. Fixed in commit `b94a9d6`: tests use `env!("CARGO_BIN_EXE_ports")` to reference the already-compiled binary directly.

### stdin must be nulled for tests
Commands with interactive confirmation prompts (`clean`, `nuke`, `killall`) read from stdin. Integration tests inherit the parent's TTY by default, causing the confirmation prompts to block. Fixed by adding `.stdin(Stdio::null())` to all test invocations.

---

## Log filter precision

An early version of `ports log 80` would match `:8080` lines because `line.contains(":80")` is a substring match. Fixed by framing the port field with tab characters on both sides: `line.contains("\t:80\t")`. The unit tests `test_log_filter_exact_port_match` and `test_log_filter_33060_vs_3306` pin this. See ADR-009.

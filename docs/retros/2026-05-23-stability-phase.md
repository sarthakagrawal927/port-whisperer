# Retro: Stability phase — 2026-05-23

Three commits landed on the same day that shifted the project from "feature-complete" to "production-stable." This retro captures what broke, why, and what was learned.

## What happened

The project shipped feature-complete on 2026-04-04 (Rust port, batched scans, orphan detection, framework detection, tests). It was dormant for ~7 weeks. On 2026-05-23, three bugs surfaced in quick succession — all related to running the tool on a machine with VPN software installed.

**Commit b94a9d6 — Subprocess timeout and test parallelism fix**

`lsof` was hanging indefinitely on machines with orphaned `utun` sockets (crashed Tailscale / WARP). There was no timeout, so `ports` would hang forever. Additionally, the 21 integration tests all called `cargo run --release --`, creating a build-lock stampede that slowed the test suite dramatically.

Fix: 5s subprocess deadline via `try_wait()` + background stdout drain thread + `SUBPROC_STUCK` atomic latch. Tests switched to `CARGO_BIN_EXE_ports`.

**Commit cf7a6d0 — Refuse to run if prior lsof processes are stuck**

Even with the timeout, each `ports` invocation before the user rebooted would leak one more unkillable `lsof` process. The fix: run a `pgrep` pre-flight (pgrep doesn't traverse fds so it's safe) and refuse to scan if any `lsof` from a prior run has been alive >30s.

**Commit 07e183c — `ports doctor` command**

The pre-flight error message was informative but not diagnostic. Added `ports doctor`: four bounded checks (leaked lsofs, utun count, known tunnel apps, live lsof probe) with color-coded output and actionable hints ("reboot to clear").

## What went well

- Batched subprocess architecture meant the timeout fix was a single change point (`run_cmd`) rather than spread across many call sites.
- The `SUBPROC_STUCK` atomic was a clean way to prevent cascade hangs without threading complexity.
- All three fixes were small, targeted, and independently shippable.

## What was hard

- The macOS kernel bug (SIGKILL not delivered to a thread stuck in `close()`) has no programmatic fix. The only option is user education ("reboot"). This is frustrating because the symptoms (port scanner that hangs) look like a bug in the tool.
- The VPN/utun interaction is undocumented by Apple. The threshold of ">4 utun = suspicious" is empirical, not from any official source.

## Lessons (cross-reference)

- Subprocess pipe back-pressure deadlock → [lessons.md](../lessons.md#pipe-back-pressure-deadlock)
- SIGKILL and macOS kernel close() → [lessons.md](../lessons.md#lsof-can-wedge-in-kernel-close-on-macos)
- Integration test build-lock stampede → [lessons.md](../lessons.md#integration-tests-vs-cargo-run)

# External References — port-whisperer

One-liner entries. For context on why each reference matters to this project, see [decisions.md](../decisions.md) and [lessons.md](../lessons.md).

---

## Core tools

- **lsof man page** — `man lsof` or https://man7.org/linux/man-pages/man8/lsof.8.html — output format, column layout, `-iTCP -sTCP:LISTEN`, `-a -d cwd` flags used in the batched scan.
- **ps man page (macOS)** — `man ps` — `etime=` format (`[[dd-]hh:]mm:ss`), `lstart=` format, `stat=` zombie flag (`Z`), `rss=` field (kilobytes, not bytes).
- **pgrep man page** — `man pgrep` — used in `leaked_lsofs()` because `pgrep` doesn't open file descriptors and is safe to run when `lsof` is wedged.
- **ifconfig man page** — `man ifconfig` — `-l` flag used to enumerate `utun` interfaces in `ports doctor`.

## Comparison tools (not used, but context)

- **ss(8)** — Linux socket utility, the modern replacement for `netstat`. Different output format; a Linux port of this tool would need `ss -tlnp` instead of `lsof`. https://man7.org/linux/man-pages/man8/ss.8.html
- **netstat(8)** — legacy, available on both macOS and Linux but with different flags. `netstat -an` is cross-platform but doesn't include PID information without `-p` (Linux) or `-v` (macOS).

## Rust crates

- **comfy-table** — https://docs.rs/comfy-table — `UTF8_FULL` preset, `UTF8_ROUND_CORNERS` modifier, `ContentArrangement::Dynamic`. Used in `display.rs`.
- **colored** — https://docs.rs/colored — `.green()`, `.cyan()`, `.bold()` extension traits on `&str`. Used throughout `display.rs`.
- **serde_json** — https://docs.rs/serde_json — `serde_json::json!` macro and `from_str` for `package.json` parsing in framework detection.
- **clap** — https://docs.rs/clap — listed as a dependency but not called at runtime; see ADR-008.

## macOS internals

- **Apple TN3178 — Resolving macOS 13 lsof hangs** — https://developer.apple.com/news/releases/ — TBD: find official Apple documentation on the `lsof`/`close()` kernel hang. Current knowledge is empirical (scanner.rs comment: "macOS kernel bug — lsof is stuck inside close()").
- **Network Extension framework** — https://developer.apple.com/documentation/networkextension — `utun` interfaces are created by NE providers (VPNs). Orphaned `utun` sockets after a crash cause the `lsof` wedge described in [lessons.md](../lessons.md#lsof-can-wedge-in-kernel-close-on-macos).

## Upstream project

- **LarsenCundric/port-whisperer** (Node.js original) — https://github.com/LarsenCundric/port-whisperer — this Rust project is a port of it.

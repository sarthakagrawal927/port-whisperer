mod display;
mod scanner;

use std::collections::HashMap;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let (flags, args) = split_flags(&raw);

    match args.first().map(|s| s.as_str()) {
        None => cmd_ports(false),
        Some("--version" | "-V") => println!("ports {}", env!("CARGO_PKG_VERSION")),
        Some("--all" | "-a") => cmd_ports(true),
        Some("--help" | "-h" | "help") => display::print_help(),
        Some("open") => match args.get(1).and_then(|s| s.parse::<u16>().ok()) {
            Some(port) => cmd_open(port),
            None => eprintln!("  Usage: ports open <port>"),
        },
        Some("free") | Some("kill-port") => cmd_free_many(&args[1..], flags.force),
        Some("kill") => cmd_kill_pids(&args[1..], flags.force),
        Some("killall") => match args.get(1) {
            Some(name) => cmd_killall(name, flags.full, flags.force),
            None => eprintln!("  Usage: ports killall <name>"),
        },
        Some("kill-project") => match args.get(1) {
            Some(name) => cmd_kill_project(name, flags.force),
            None => eprintln!("  Usage: ports kill-project <name>"),
        },
        Some("nuke") => cmd_nuke(flags.force),
        Some("run") => cmd_run(&args[1..], flags.force),
        Some("json") => {
            let show_all = args.get(1).is_some_and(|a| a == "--all" || a == "-a");
            cmd_json(show_all);
        }
        Some("log") => {
            let port_filter = args.get(1).and_then(|s| s.parse::<u16>().ok());
            cmd_log(port_filter);
        }
        Some("ps") => {
            let show_all = args.get(1).is_some_and(|a| a == "--all" || a == "-a");
            cmd_ps(show_all);
        }
        Some("clean") => cmd_clean(),
        Some("doctor") => cmd_doctor(),
        Some("watch") => cmd_watch(),
        Some(port_str) => {
            if port_str.chars().all(|c| c.is_ascii_digit()) {
                match port_str.parse::<u16>() {
                    Ok(port) => cmd_inspect(port),
                    Err(_) => eprintln!("  Invalid port: {port_str} (must be 1-65535)"),
                }
            } else {
                eprintln!("Unknown command: {port_str}");
                display::print_help();
            }
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct Flags {
    force: bool,
    full: bool,
}

/// Pull global flags out of the arg list. `run` is a passthrough — anything
/// after `run` is treated as the user's command and not interpreted as a flag.
fn split_flags(raw: &[String]) -> (Flags, Vec<String>) {
    let mut flags = Flags::default();
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    let mut passthrough = false;

    for arg in raw {
        if !passthrough {
            match arg.as_str() {
                "-f" | "--force" => {
                    flags.force = true;
                    continue;
                }
                "-F" | "--full" => {
                    flags.full = true;
                    continue;
                }
                _ => {}
            }
            if arg == "run" {
                passthrough = true;
            }
        }
        out.push(arg.clone());
    }
    (flags, out)
}

fn cmd_ports(show_all: bool) {
    let ports = scanner::scan_ports(show_all);
    scanner::log_snapshot(&ports);
    display::print_ports_table(&ports);
}

fn cmd_open(port: u16) {
    if scanner::open_in_browser(port) {
        println!("  Opened http://localhost:{}", port);
    } else {
        eprintln!("  Failed to open http://localhost:{}", port);
    }
}

fn cmd_free_many(tokens: &[String], force: bool) {
    if tokens.is_empty() {
        eprintln!("  Usage: ports free <port(s)>");
        return;
    }
    let (ports, errors) = scanner::parse_port_list(tokens);
    for bad in &errors {
        eprintln!("  Invalid port: {bad}");
    }
    if ports.is_empty() {
        return;
    }
    for port in ports {
        match scanner::scan_port_detail(port) {
            Some(info) => {
                if scanner::kill_process_force(info.pid, force) {
                    println!("  Killed {} (PID {}) on :{}", info.name, info.pid, port);
                } else {
                    eprintln!("  Failed to kill {} (PID {}) on :{}", info.name, info.pid, port);
                }
            }
            None => eprintln!("  Nothing on :{}", port),
        }
    }
}

fn cmd_kill_pids(tokens: &[String], force: bool) {
    if tokens.is_empty() {
        eprintln!("  Usage: ports kill <pid(s)>");
        return;
    }
    let mut any = false;
    for tok in tokens {
        match tok.parse::<u32>() {
            Ok(pid) => {
                any = true;
                if scanner::kill_process_force(pid, force) {
                    println!("  Killed PID {}", pid);
                } else {
                    eprintln!("  Failed to kill PID {}", pid);
                }
            }
            Err(_) => eprintln!("  Invalid PID: {tok}"),
        }
    }
    if !any {
        eprintln!("  No valid PIDs given.");
    }
}

fn cmd_killall(name: &str, full: bool, force: bool) {
    let hits = scanner::find_pids_by_name(name, full);
    if hits.is_empty() {
        eprintln!("  No processes matching '{}'.", name);
        return;
    }

    println!("  Found {} process(es) matching '{}':", hits.len(), name);
    for (pid, comm) in &hits {
        println!("    PID {} — {}", pid, comm);
    }

    if !confirm(&format!("  Kill all {} process(es)?", hits.len())) {
        println!("  Aborted.");
        return;
    }

    for (pid, comm) in hits {
        if scanner::kill_process_force(pid, force) {
            println!("  Killed PID {} ({})", pid, comm);
        } else {
            eprintln!("  Failed to kill PID {} ({})", pid, comm);
        }
    }
}

fn cmd_kill_project(name: &str, force: bool) {
    let ports = scanner::ports_by_project(name);
    if ports.is_empty() {
        eprintln!("  No listening ports for project '{}'.", name);
        return;
    }
    println!("  Killing {} port(s) for project '{}':", ports.len(), name);
    for p in &ports {
        if scanner::kill_process_force(p.pid, force) {
            println!("    Killed {} (PID {}) on :{}", p.name, p.pid, p.port);
        } else {
            eprintln!("    Failed to kill {} (PID {}) on :{}", p.name, p.pid, p.port);
        }
    }
}

fn cmd_nuke(force: bool) {
    let ports = scanner::scan_ports(false);
    if ports.is_empty() {
        println!("  Nothing to nuke.");
        return;
    }
    println!("  About to kill {} dev server(s):", ports.len());
    for p in &ports {
        println!("    :{}  {}  (PID {})", p.port, p.name, p.pid);
    }
    if !confirm("  Proceed?") {
        println!("  Aborted.");
        return;
    }
    for p in ports {
        if scanner::kill_process_force(p.pid, force) {
            println!("  Killed {} (PID {}) on :{}", p.name, p.pid, p.port);
        } else {
            eprintln!("  Failed to kill {} (PID {}) on :{}", p.name, p.pid, p.port);
        }
    }
}

fn cmd_run(args: &[String], force: bool) {
    if args.is_empty() {
        eprintln!("  Usage: ports run <cmd...>");
        return;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let root = scanner::project_root_for(cwd.to_str().unwrap_or("")).unwrap_or(cwd);

    let victims = scanner::ports_under_path(&root);
    for p in &victims {
        if scanner::kill_process_force(p.pid, force) {
            println!("  Freed :{} (was {} PID {})", p.port, p.name, p.pid);
        }
    }
    if !victims.is_empty() {
        // Tiny breather so sockets release before child binds.
        thread::sleep(Duration::from_millis(300));
    }

    let program = &args[0];
    let rest = &args[1..];
    let err = Command::new(program).args(rest).exec();
    eprintln!("  Failed to exec {}: {}", program, err);
    std::process::exit(127);
}

fn cmd_json(show_all: bool) {
    let ports = scanner::scan_ports(show_all);
    println!("{}", scanner::ports_to_json(&ports));
}

fn cmd_log(port_filter: Option<u16>) {
    let entries = scanner::read_log(port_filter, 50);
    display::print_log(&entries, port_filter);
}

fn cmd_ps(show_all: bool) {
    let procs = scanner::scan_processes(show_all);
    display::print_processes_table(&procs);
}

fn cmd_inspect(port: u16) {
    match scanner::scan_port_detail(port) {
        Some(info) => {
            let branch = scanner::get_git_branch(&info.cwd);
            display::print_port_detail(&info, branch.as_deref());

            print!("  Kill process {}? [y/N] ", info.pid);
            io::stdout().flush().ok();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok()
                && input.trim().eq_ignore_ascii_case("y")
            {
                if scanner::kill_process(info.pid) {
                    println!("  Process {} killed.", info.pid);
                } else {
                    eprintln!("  Failed to kill process {}.", info.pid);
                }
            }
        }
        None => {
            eprintln!("  No process found on port :{port}");
        }
    }
}

fn cmd_clean() {
    display::print_clean_header();
    let ports = scanner::scan_ports(false);
    let unhealthy = scanner::find_unhealthy(&ports);
    display::print_clean_result(&unhealthy);

    if unhealthy.is_empty() {
        return;
    }
    if !confirm("  Kill all unhealthy processes?") {
        return;
    }
    for p in &unhealthy {
        if scanner::kill_process(p.pid) {
            println!("  Killed PID {} (:{}).", p.pid, p.port);
        } else {
            eprintln!("  Failed to kill PID {}.", p.pid);
        }
    }
}

fn cmd_doctor() {
    display::print_doctor(&scanner::run_doctor());
}

fn cmd_watch() {
    println!("\n  Watching for port changes... (Ctrl+C to stop)\n");

    let mut prev_ports: HashMap<u16, String> = HashMap::new();
    for p in scanner::watch_ports() {
        prev_ports.insert(p.port, p.name.clone());
    }

    loop {
        thread::sleep(Duration::from_secs(2));

        let current: HashMap<u16, String> = scanner::watch_ports()
            .into_iter()
            .map(|p| (p.port, p.name))
            .collect();

        for (port, name) in &current {
            if !prev_ports.contains_key(port) {
                display::print_watch_event(*port, name, "opened");
            }
        }
        for (port, name) in &prev_ports {
            if !current.contains_key(port) {
                display::print_watch_event(*port, name, "closed");
            }
        }

        prev_ports = current;
    }
}

fn confirm(prompt: &str) -> bool {
    print!("{} [y/N] ", prompt);
    io::stdout().flush().ok();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    input.trim().eq_ignore_ascii_case("y")
}

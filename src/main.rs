mod display;
mod scanner;

use std::collections::HashMap;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        None => cmd_ports(false),
        Some("--all" | "-a") => cmd_ports(true),
        Some("--help" | "-h" | "help") => display::print_help(),
        Some("ps") => {
            let show_all = args.get(1).is_some_and(|a| a == "--all" || a == "-a");
            cmd_ps(show_all);
        }
        Some("clean") => cmd_clean(),
        Some("watch") => cmd_watch(),
        Some(port_str) => {
            if let Ok(port) = port_str.parse::<u16>() {
                cmd_inspect(port);
            } else {
                eprintln!("Unknown command: {port_str}");
                display::print_help();
            }
        }
    }
}

fn cmd_ports(show_all: bool) {
    let ports = scanner::scan_ports(show_all);
    display::print_ports_table(&ports);
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

            // Interactive kill prompt
            print!("  Kill process {}? [y/N] ", info.pid);
            io::stdout().flush().ok();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                if input.trim().eq_ignore_ascii_case("y") {
                    if scanner::kill_process(info.pid) {
                        println!("  Process {} killed.", info.pid);
                    } else {
                        eprintln!("  Failed to kill process {}.", info.pid);
                    }
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

    if !unhealthy.is_empty() {
        print!("  Kill all unhealthy processes? [y/N] ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            if input.trim().eq_ignore_ascii_case("y") {
                for p in &unhealthy {
                    if scanner::kill_process(p.pid) {
                        println!("  Killed PID {} (:{}).", p.pid, p.port);
                    } else {
                        eprintln!("  Failed to kill PID {}.", p.pid);
                    }
                }
            }
        }
    }
}

fn cmd_watch() {
    println!("\n  Watching for port changes... (Ctrl+C to stop)\n");

    let mut prev_ports: HashMap<u16, String> = HashMap::new();

    // Initial scan
    for p in scanner::watch_ports() {
        prev_ports.insert(p.port, p.name.clone());
    }

    loop {
        thread::sleep(Duration::from_secs(2));

        let current: HashMap<u16, String> = scanner::watch_ports()
            .into_iter()
            .map(|p| (p.port, p.name))
            .collect();

        // Detect new ports
        for (port, name) in &current {
            if !prev_ports.contains_key(port) {
                display::print_watch_event(*port, name, "opened");
            }
        }

        // Detect closed ports
        for (port, name) in &prev_ports {
            if !current.contains_key(port) {
                display::print_watch_event(*port, name, "closed");
            }
        }

        prev_ports = current;
    }
}

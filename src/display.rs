use crate::scanner::{Health, PortInfo, ProcessInfo};
use colored::Colorize;
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use std::time::Duration;

pub fn print_ports_table(ports: &[PortInfo]) {
    if ports.is_empty() {
        println!("{}", "No listening ports found.".dimmed());
        println!(
            "{}",
            "Try 'ports --all' to include system services.".dimmed()
        );
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("PORT").add_attribute(Attribute::Bold),
            Cell::new("PID").add_attribute(Attribute::Bold),
            Cell::new("PROCESS").add_attribute(Attribute::Bold),
            Cell::new("FRAMEWORK").add_attribute(Attribute::Bold),
            Cell::new("PROJECT").add_attribute(Attribute::Bold),
            Cell::new("HEALTH").add_attribute(Attribute::Bold),
            Cell::new("UPTIME").add_attribute(Attribute::Bold),
        ]);

    for port in ports {
        let health_cell = match port.health {
            Health::Healthy => Cell::new("●  healthy").fg(Color::Green),
            Health::Orphaned => Cell::new("●  orphaned").fg(Color::Yellow),
            Health::Zombie => Cell::new("●  zombie").fg(Color::Red),
        };

        let docker_suffix = if port.docker_container.is_some() {
            " 🐳"
        } else {
            ""
        };

        table.add_row(vec![
            Cell::new(format!(":{}", port.port)).fg(Color::Cyan),
            Cell::new(port.pid),
            Cell::new(format!("{}{}", port.name, docker_suffix)),
            Cell::new(&port.framework).fg(Color::Magenta),
            Cell::new(&port.project).fg(Color::Blue),
            health_cell,
            Cell::new(format_duration(port.uptime)),
        ]);
    }

    println!();
    println!("{}", table);
    println!(
        "\n  {} ports active  |  {} 'ports <number>' to inspect",
        ports.len().to_string().cyan(),
        "tip:".dimmed()
    );
}

pub fn print_port_detail(port: &PortInfo, git_branch: Option<&str>) {
    println!();
    println!(
        "  {} {}",
        "Port".bold(),
        format!(":{}", port.port).cyan().bold()
    );
    println!("  {}", "─".repeat(40).dimmed());
    println!("  {} {}", "PID:".dimmed(), port.pid);
    println!("  {} {}", "Process:".dimmed(), port.name);
    println!("  {} {}", "Framework:".dimmed(), port.framework.magenta());
    if !port.project.is_empty() {
        println!("  {} {}", "Project:".dimmed(), port.project.blue());
    }
    if let Some(branch) = git_branch {
        println!("  {} {}", "Branch:".dimmed(), branch.green());
    }
    println!(
        "  {} {}",
        "Health:".dimmed(),
        match port.health {
            Health::Healthy => "healthy".green().to_string(),
            Health::Orphaned => "orphaned".yellow().to_string(),
            Health::Zombie => "zombie".red().to_string(),
        }
    );
    println!("  {} {:.1} MB", "Memory:".dimmed(), port.memory_mb);
    println!("  {} {}", "Uptime:".dimmed(), format_duration(port.uptime));
    println!("  {} {}", "PPID:".dimmed(), port.ppid);
    if let Some(ref container) = port.docker_container {
        println!("  {} {}", "Container:".dimmed(), container);
    }
    if let Some(ref image) = port.docker_image {
        println!("  {} {}", "Image:".dimmed(), image);
    }
    if !port.cwd.is_empty() {
        println!("  {} {}", "CWD:".dimmed(), port.cwd.dimmed());
    }
    println!("  {} {}", "Command:".dimmed(), truncate(&port.command, 80).dimmed());
    println!();
}

pub fn print_processes_table(procs: &[ProcessInfo]) {
    if procs.is_empty() {
        println!("{}", "No dev processes found.".dimmed());
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("PID").add_attribute(Attribute::Bold),
            Cell::new("PROCESS").add_attribute(Attribute::Bold),
            Cell::new("CPU %").add_attribute(Attribute::Bold),
            Cell::new("MEM (MB)").add_attribute(Attribute::Bold),
            Cell::new("COMMAND").add_attribute(Attribute::Bold),
        ]);

    let mut docker_count = 0u32;

    for proc in procs {
        if proc.is_docker {
            docker_count += 1;
            continue;
        }

        let cpu_cell = if proc.cpu > 50.0 {
            Cell::new(format!("{:.1}", proc.cpu)).fg(Color::Red)
        } else if proc.cpu > 10.0 {
            Cell::new(format!("{:.1}", proc.cpu)).fg(Color::Yellow)
        } else {
            Cell::new(format!("{:.1}", proc.cpu))
        };

        table.add_row(vec![
            Cell::new(proc.pid),
            Cell::new(&proc.name).fg(Color::Cyan),
            cpu_cell,
            Cell::new(format!("{:.1}", proc.memory_mb)),
            Cell::new(truncate(&proc.command, 60)).fg(Color::DarkGrey),
        ]);
    }

    println!();
    println!("{}", table);

    if docker_count > 0 {
        println!(
            "\n  {} {} Docker processes running",
            "🐳".to_string(),
            docker_count
        );
    }

    println!(
        "\n  {} processes  |  {} 'ports' to see port bindings",
        procs.len().to_string().cyan(),
        "tip:".dimmed()
    );
}

pub fn print_watch_event(port: u16, name: &str, event: &str) {
    let now = chrono_now();
    match event {
        "opened" => println!(
            "  {} [{}] :{} {} ({})",
            "▲".green(),
            now.dimmed(),
            port.to_string().cyan(),
            "opened".green(),
            name
        ),
        "closed" => println!(
            "  {} [{}] :{} {} ({})",
            "▼".red(),
            now.dimmed(),
            port.to_string().cyan(),
            "closed".red(),
            name
        ),
        _ => {}
    }
}

pub fn print_clean_header() {
    println!();
    println!("  {} Scanning for unhealthy processes...", "🔍".to_string());
}

pub fn print_clean_result(unhealthy: &[&PortInfo]) {
    if unhealthy.is_empty() {
        println!("  {} All processes are healthy!", "✓".green());
        return;
    }

    println!(
        "  Found {} unhealthy processes:\n",
        unhealthy.len().to_string().yellow()
    );

    for p in unhealthy {
        let status = match p.health {
            Health::Orphaned => "orphaned".yellow(),
            Health::Zombie => "zombie".red(),
            Health::Healthy => "healthy".green(),
        };
        println!(
            "    :{} {} (PID {}) - {}",
            p.port.to_string().cyan(),
            p.name,
            p.pid,
            status
        );
    }
    println!();
}

pub fn print_log(entries: &[String], port_filter: Option<u16>) {
    if entries.is_empty() {
        let msg = match port_filter {
            Some(p) => format!("No history for port :{}.", p),
            None => "No port history yet. Run 'ports' to start logging.".to_string(),
        };
        println!("  {}", msg.dimmed());
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("TIMESTAMP").add_attribute(Attribute::Bold),
            Cell::new("PORT").add_attribute(Attribute::Bold),
            Cell::new("PID").add_attribute(Attribute::Bold),
            Cell::new("PROCESS").add_attribute(Attribute::Bold),
            Cell::new("FRAMEWORK").add_attribute(Attribute::Bold),
            Cell::new("HEALTH").add_attribute(Attribute::Bold),
        ]);

    for entry in entries {
        let cols: Vec<&str> = entry.split('\t').collect();
        if cols.len() >= 6 {
            let health_cell = match cols[5] {
                "healthy" => Cell::new("healthy").fg(Color::Green),
                "orphaned" => Cell::new("orphaned").fg(Color::Yellow),
                "zombie" => Cell::new("zombie").fg(Color::Red),
                other => Cell::new(other),
            };
            table.add_row(vec![
                Cell::new(cols[0]).fg(Color::DarkGrey),
                Cell::new(cols[1]).fg(Color::Cyan),
                Cell::new(cols[2]),
                Cell::new(cols[3]),
                Cell::new(cols[4]).fg(Color::Magenta),
                health_cell,
            ]);
        }
    }

    println!();
    println!("{}", table);
    println!();
}

pub fn print_help() {
    println!();
    println!("  {} {}", "ports".bold().cyan(), "— developer port scanner".dimmed());
    println!();
    println!("  {}", "USAGE".bold());
    println!("    ports              Show dev server ports");
    println!("    ports --all        Show all listening ports");
    println!("    ports <port>       Inspect a specific port");
    println!("    ports open <port>  Open localhost:<port> in browser");
    println!("    ports free <port>  Kill whatever's on that port");
    println!("    ports json         JSON output for scripting");
    println!("    ports log          Show port history");
    println!("    ports log <port>   Show history for a specific port");
    println!("    ports ps           Show running dev processes");
    println!("    ports ps --all     Show all processes");
    println!("    ports clean        Find & kill orphaned processes");
    println!("    ports watch        Monitor port changes in real-time");
    println!("    ports help         Show this help");
    println!();
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    if total_secs < 60 {
        format!("{}s", total_secs)
    } else if total_secs < 3600 {
        format!("{}m", total_secs / 60)
    } else if total_secs < 86400 {
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        format!("{}h{}m", h, m)
    } else {
        let d = total_secs / 86400;
        let h = (total_secs % 86400) / 3600;
        format!("{}d{}h", d, h)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}…")
    }
}

fn chrono_now() -> String {
    let output = std::process::Command::new("date")
        .args(["+%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "??:??:??".to_string());
    output
}

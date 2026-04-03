use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub port: u16,
    pub pid: u32,
    pub name: String,
    pub framework: String,
    pub project: String,
    pub health: Health,
    pub ppid: u32,
    pub memory_mb: f64,
    pub uptime: Duration,
    pub command: String,
    pub cwd: String,
    pub docker_container: Option<String>,
    pub docker_image: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Health {
    Healthy,
    Orphaned,
    Zombie,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f64,
    pub memory_mb: f64,
    pub command: String,
    pub is_docker: bool,
}

const SYSTEM_APPS: &[&str] = &[
    "spotify",
    "raycast",
    "slack",
    "discord",
    "chrome",
    "firefox",
    "safari",
    "1password",
    "iterm2",
    "ghostty",
    "wezterm",
    "alacritty",
    "kitty",
    "figma",
    "notion",
    "zoom",
    "teams",
    "arc",
    "brave",
    "opera",
    "vivaldi",
    "dropbox",
    "googledrivehelper",
    "onedrive",
    "controlcenter",
    "systemuiserver",
    "windowserver",
    "loginwindow",
    "finder",
    "dock",
    "airplayuiagent",
    "sharingd",
    "rapportd",
    "identityservicesd",
    "assistantd",
    "cloudd",
    "mds_stores",
    "bird",
    "secd",
    "trustd",
    "syspolicyd",
    "endpointsecurityd",
];

const PROJECT_MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "setup.py",
    "Gemfile",
    "pom.xml",
    "build.gradle",
    "build.sbt",
    "mix.exs",
    "deno.json",
    "composer.json",
];

fn run_cmd(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

pub fn scan_ports(show_all: bool) -> Vec<PortInfo> {
    let lsof_output = run_cmd("lsof", &["-iTCP", "-sTCP:LISTEN", "-P", "-n"]);

    let mut port_pid_map: HashMap<u16, (u32, String)> = HashMap::new();
    for line in lsof_output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }
        let name = cols[0].to_string();
        let pid: u32 = match cols[1].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let name_col = cols[8];
        if let Some(port_str) = name_col.rsplit(':').next() {
            if let Ok(port) = port_str.parse::<u16>() {
                port_pid_map.entry(port).or_insert((pid, name));
            }
        }
    }

    if port_pid_map.is_empty() {
        return Vec::new();
    }

    let pids: Vec<u32> = port_pid_map.values().map(|(pid, _)| *pid).collect();
    let unique_pids: HashSet<u32> = pids.into_iter().collect();
    let pid_list: String = unique_pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");

    // Batch ps call
    let ps_output = run_cmd(
        "ps",
        &[
            "-p",
            &pid_list,
            "-o",
            "pid=,ppid=,stat=,rss=,lstart=,command=",
        ],
    );

    let mut ps_data: HashMap<u32, (u32, String, u64, String, String)> = HashMap::new();
    for line in ps_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.splitn(6, char::is_whitespace).collect();
        if parts.len() < 6 {
            // Try to parse more carefully - lstart has spaces
            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.len() >= 9 {
                let pid: u32 = match tokens[0].parse() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let ppid: u32 = tokens[1].parse().unwrap_or(0);
                let stat = tokens[2].to_string();
                let rss: u64 = tokens[3].parse().unwrap_or(0);
                // lstart is like "Thu Jan  2 15:04:05 2025" - 5 tokens
                let lstart = tokens[4..9].join(" ");
                let command = tokens[9..].join(" ");
                ps_data.insert(pid, (ppid, stat, rss, lstart, command));
            }
            continue;
        }
        // Fallback parse
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 9 {
            let pid: u32 = match tokens[0].parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let ppid: u32 = tokens[1].parse().unwrap_or(0);
            let stat = tokens[2].to_string();
            let rss: u64 = tokens[3].parse().unwrap_or(0);
            let lstart = tokens[4..9].join(" ");
            let command = tokens[9..].join(" ");
            ps_data.insert(pid, (ppid, stat, rss, lstart, command));
        }
    }

    // Batch cwd resolution
    let cwd_output = run_cmd("lsof", &["-a", "-d", "cwd", "-p", &pid_list]);
    let mut cwd_map: HashMap<u32, String> = HashMap::new();
    for line in cwd_output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 9 {
            if let Ok(pid) = cols[1].parse::<u32>() {
                let path = cols[8..].join(" ");
                cwd_map.insert(pid, path);
            }
        }
    }

    // Docker mapping
    let docker_map = get_docker_mapping();

    let mut results: Vec<PortInfo> = Vec::new();

    for (port, (pid, name)) in &port_pid_map {
        if !show_all && is_system_app(&name.to_lowercase()) {
            continue;
        }

        let (ppid, stat, rss, _lstart, command) =
            ps_data.get(pid).cloned().unwrap_or((0, String::new(), 0, String::new(), String::new()));

        let health = if stat.contains('Z') {
            Health::Zombie
        } else if ppid == 1 {
            Health::Orphaned
        } else {
            Health::Healthy
        };

        let cwd = cwd_map.get(pid).cloned().unwrap_or_default();
        let project_root = find_project_root(&cwd);
        let project = project_root
            .as_ref()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let (docker_container, docker_image) = docker_map
            .get(port)
            .cloned()
            .unwrap_or((None, None));

        let framework = detect_framework(&command, &cwd, &project_root, &docker_image, &name.to_lowercase());

        let memory_mb = rss as f64 / 1024.0;
        let uptime = parse_uptime_from_pid(*pid);

        results.push(PortInfo {
            port: *port,
            pid: *pid,
            name: name.clone(),
            framework,
            project,
            health,
            ppid,
            memory_mb,
            uptime,
            command,
            cwd,
            docker_container,
            docker_image,
        });
    }

    results.sort_by_key(|p| p.port);
    results
}

pub fn scan_port_detail(port: u16) -> Option<PortInfo> {
    let all = scan_ports(true);
    all.into_iter().find(|p| p.port == port)
}

pub fn scan_processes(show_all: bool) -> Vec<ProcessInfo> {
    let output = run_cmd("ps", &["aux"]);
    let mut processes: Vec<ProcessInfo> = Vec::new();

    for line in output.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 11 {
            continue;
        }
        let pid: u32 = match cols[1].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let cpu: f64 = cols[2].parse().unwrap_or(0.0);
        let rss: u64 = cols[5].parse().unwrap_or(0);
        let command = cols[10..].join(" ");
        let name = cols[10]
            .rsplit('/')
            .next()
            .unwrap_or(cols[10])
            .to_string();

        let is_docker = name.contains("docker") || command.contains("docker");
        let lower_name = name.to_lowercase();

        if !show_all && (is_system_app(&lower_name) || is_system_process(&command)) {
            continue;
        }

        processes.push(ProcessInfo {
            pid,
            name,
            cpu,
            memory_mb: rss as f64 / 1024.0,
            command,
            is_docker,
        });
    }

    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    processes
}

pub fn find_unhealthy(ports: &[PortInfo]) -> Vec<&PortInfo> {
    ports
        .iter()
        .filter(|p| p.health != Health::Healthy)
        .collect()
}

pub fn watch_ports() -> Vec<PortInfo> {
    scan_ports(false)
}

pub fn kill_process(pid: u32) -> bool {
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn get_git_branch(cwd: &str) -> Option<String> {
    if cwd.is_empty() {
        return None;
    }
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn is_system_app(name: &str) -> bool {
    // lsof truncates names to ~9 chars, so "ControlCenter" becomes "ControlCe"
    // Check both directions: app.contains(name_fragment) and name.contains(app)
    SYSTEM_APPS.iter().any(|app| name.contains(app) || app.starts_with(name))
}

fn is_system_process(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    lower.contains("/system/")
        || lower.contains("/usr/libexec/")
        || lower.contains("/usr/sbin/")
        || lower.contains("com.apple.")
        || lower.contains("launchd")
        || lower.contains("kernel_task")
        || lower.contains("mds_stores")
        || lower.contains("windowserver")
}

fn find_project_root(cwd: &str) -> Option<PathBuf> {
    if cwd.is_empty() {
        return None;
    }
    let mut current = Path::new(cwd).to_path_buf();
    for _ in 0..15 {
        for marker in PROJECT_MARKERS {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn get_docker_mapping() -> HashMap<u16, (Option<String>, Option<String>)> {
    let output = run_cmd(
        "docker",
        &["ps", "--format", "{{.Names}}|{{.Image}}|{{.Ports}}"],
    );
    let mut map: HashMap<u16, (Option<String>, Option<String>)> = HashMap::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 3 {
            continue;
        }
        let container_name = parts[0].to_string();
        let image = parts[1].to_string();
        let ports_str = parts[2];

        // Parse ports like "0.0.0.0:5432->5432/tcp"
        for segment in ports_str.split(',') {
            let segment = segment.trim();
            if let Some(arrow_idx) = segment.find("->") {
                let host_part = &segment[..arrow_idx];
                if let Some(port_str) = host_part.rsplit(':').next() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        map.insert(port, (Some(container_name.clone()), Some(image.clone())));
                    }
                }
            }
        }
    }
    map
}

fn detect_framework(
    command: &str,
    _cwd: &str,
    project_root: &Option<PathBuf>,
    docker_image: &Option<String>,
    process_name: &str,
) -> String {
    let cmd_lower = command.to_lowercase();

    // Well-known server processes — check early before substring matching
    let known_servers = [
        ("mysqld", "MySQL"),
        ("postgres", "PostgreSQL"),
        ("redis-server", "Redis"),
        ("redis-se", "Redis"),
        ("mongod", "MongoDB"),
        ("memcached", "Memcached"),
        ("nginx", "Nginx"),
        ("httpd", "Apache"),
        ("caddy", "Caddy"),
        ("traefik", "Traefik"),
        ("elasticsearch", "Elasticsearch"),
        ("rabbitmq", "RabbitMQ"),
    ];
    let name_lower = process_name.to_lowercase();
    for (pattern, name) in &known_servers {
        if name_lower.starts_with(pattern) || cmd_lower.starts_with(pattern) {
            return name.to_string();
        }
    }

    // Command-line detection (ordered most-specific first)
    let cmd_frameworks = [
        ("next dev", "Next.js"),
        ("next start", "Next.js"),
        ("next build", "Next.js"),
        ("nuxt", "Nuxt"),
        ("remix", "Remix"),
        ("astro", "Astro"),
        ("svelte", "SvelteKit"),
        ("vite", "Vite"),
        ("webpack serve", "Webpack"),
        ("angular", "Angular"),
        ("ng serve", "Angular"),
        ("gatsby", "Gatsby"),
        ("expo", "Expo"),
        ("react-scripts", "Create React App"),
        ("fastify", "Fastify"),
        ("nest start", "NestJS"),
        ("hono", "Hono"),
        ("express", "Express"),
        ("flask", "Flask"),
        ("django", "Django"),
        ("uvicorn", "FastAPI"),
        ("gunicorn", "Python"),
        ("rails", "Rails"),
        ("puma", "Rails"),
        ("phoenix", "Phoenix"),
        ("mix phx", "Phoenix"),
        ("cargo run", "Rust"),
        ("go run", "Go"),
        ("/gin", "Gin"),
        ("fiber", "Fiber"),
        ("spring", "Spring Boot"),
        ("gradlew", "Gradle"),
        ("hugo", "Hugo"),
        ("jekyll", "Jekyll"),
        ("eleventy", "Eleventy"),
        ("storybook", "Storybook"),
    ];

    for (pattern, framework) in &cmd_frameworks {
        if cmd_lower.contains(pattern) {
            return framework.to_string();
        }
    }

    // package.json dependency detection
    if let Some(root) = project_root {
        let pkg_path = root.join("package.json");
        if let Ok(contents) = fs::read_to_string(&pkg_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                let deps = [
                    json.get("dependencies"),
                    json.get("devDependencies"),
                ];
                let dep_frameworks = [
                    ("next", "Next.js"),
                    ("nuxt", "Nuxt"),
                    ("@remix-run/dev", "Remix"),
                    ("astro", "Astro"),
                    ("@sveltejs/kit", "SvelteKit"),
                    ("svelte", "Svelte"),
                    ("@angular/core", "Angular"),
                    ("gatsby", "Gatsby"),
                    ("vite", "Vite"),
                    ("react-scripts", "Create React App"),
                    ("fastify", "Fastify"),
                    ("@nestjs/core", "NestJS"),
                    ("hono", "Hono"),
                    ("express", "Express"),
                    ("react", "React"),
                    ("vue", "Vue"),
                ];

                for dep_map in deps.iter().flatten() {
                    if let Some(obj) = dep_map.as_object() {
                        for (key, framework) in &dep_frameworks {
                            if obj.contains_key(*key) {
                                return framework.to_string();
                            }
                        }
                    }
                }
            }
        }

        // Config file detection
        let config_frameworks = [
            ("vite.config.ts", "Vite"),
            ("vite.config.js", "Vite"),
            ("next.config.js", "Next.js"),
            ("next.config.mjs", "Next.js"),
            ("next.config.ts", "Next.js"),
            ("angular.json", "Angular"),
            ("svelte.config.js", "SvelteKit"),
            ("nuxt.config.ts", "Nuxt"),
            ("astro.config.mjs", "Astro"),
            ("remix.config.js", "Remix"),
            ("gatsby-config.js", "Gatsby"),
            ("Cargo.toml", "Rust"),
            ("go.mod", "Go"),
            ("mix.exs", "Elixir"),
            ("Gemfile", "Ruby"),
        ];

        for (file, framework) in &config_frameworks {
            if root.join(file).exists() {
                return framework.to_string();
            }
        }
    }

    // Docker image detection
    if let Some(image) = docker_image {
        let img_lower = image.to_lowercase();
        let docker_frameworks = [
            ("postgres", "PostgreSQL"),
            ("mysql", "MySQL"),
            ("mariadb", "MariaDB"),
            ("mongo", "MongoDB"),
            ("redis", "Redis"),
            ("memcached", "Memcached"),
            ("elasticsearch", "Elasticsearch"),
            ("opensearch", "OpenSearch"),
            ("rabbitmq", "RabbitMQ"),
            ("kafka", "Kafka"),
            ("zookeeper", "Zookeeper"),
            ("nginx", "Nginx"),
            ("traefik", "Traefik"),
            ("caddy", "Caddy"),
            ("localstack", "LocalStack"),
            ("minio", "MinIO"),
            ("grafana", "Grafana"),
            ("prometheus", "Prometheus"),
            ("jaeger", "Jaeger"),
            ("mailhog", "MailHog"),
        ];
        for (pattern, name) in &docker_frameworks {
            if img_lower.contains(pattern) {
                return name.to_string();
            }
        }
    }

    // Fallback: process name
    match &*name_lower {
        n if n.contains("node") => "Node.js".to_string(),
        n if n.contains("python") => "Python".to_string(),
        n if n.contains("ruby") => "Ruby".to_string(),
        n if n.contains("java") && !n.contains("javascript") => "Java".to_string(),
        n if n.contains("beam") => "Erlang/Elixir".to_string(),
        "go" => "Go".to_string(),
        n if n.contains("deno") => "Deno".to_string(),
        n if n.contains("bun") => "Bun".to_string(),
        n if n.contains("stable") || n.contains("diffusion") => process_name.to_string(),
        _ => process_name.to_string(),
    }
}

pub fn open_in_browser(port: u16) {
    let url = format!("http://localhost:{}", port);
    Command::new("open").arg(&url).spawn().ok();
}

pub fn free_port(port: u16) -> Option<(u32, String)> {
    if let Some(info) = scan_port_detail(port) {
        let pid = info.pid;
        let name = info.name.clone();
        if kill_process(pid) {
            return Some((pid, name));
        }
    }
    None
}

pub fn ports_to_json(ports: &[PortInfo]) -> String {
    let entries: Vec<serde_json::Value> = ports
        .iter()
        .map(|p| {
            serde_json::json!({
                "port": p.port,
                "pid": p.pid,
                "name": p.name,
                "framework": p.framework,
                "project": p.project,
                "health": match p.health {
                    Health::Healthy => "healthy",
                    Health::Orphaned => "orphaned",
                    Health::Zombie => "zombie",
                },
                "ppid": p.ppid,
                "memory_mb": (p.memory_mb * 10.0).round() / 10.0,
                "uptime_secs": p.uptime.as_secs(),
                "command": p.command,
                "cwd": p.cwd,
                "docker_container": p.docker_container,
                "docker_image": p.docker_image,
            })
        })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
}

fn log_dir() -> PathBuf {
    let dir = dirs_home().join(".ports-history");
    fs::create_dir_all(&dir).ok();
    dir
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn log_snapshot(ports: &[PortInfo]) {
    let path = log_dir().join("history.log");
    let timestamp = run_cmd("date", &["+%Y-%m-%d %H:%M:%S"]);
    let timestamp = timestamp.trim();

    let mut file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(_) => return,
    };

    for p in ports {
        let health = match p.health {
            Health::Healthy => "healthy",
            Health::Orphaned => "orphaned",
            Health::Zombie => "zombie",
        };
        writeln!(
            file,
            "{}\t:{}\t{}\t{}\t{}\t{}",
            timestamp, p.port, p.pid, p.name, p.framework, health
        )
        .ok();
    }
}

pub fn read_log(port_filter: Option<u16>, limit: usize) -> Vec<String> {
    let path = log_dir().join("history.log");
    let contents = fs::read_to_string(&path).unwrap_or_default();
    let lines: Vec<&str> = contents.lines().rev().collect();

    let filtered: Vec<String> = lines
        .into_iter()
        .filter(|line| {
            if let Some(port) = port_filter {
                line.contains(&format!(":{}", port))
            } else {
                true
            }
        })
        .take(limit)
        .map(|s| s.to_string())
        .collect();

    filtered.into_iter().rev().collect()
}

fn parse_uptime_from_pid(pid: u32) -> Duration {
    // Use ps to get elapsed time
    let output = run_cmd("ps", &["-p", &pid.to_string(), "-o", "etime="]);
    let etime = output.trim();
    parse_etime(etime)
}

fn parse_etime(etime: &str) -> Duration {
    // Format: [[dd-]hh:]mm:ss
    let parts: Vec<&str> = etime.split(':').collect();
    let (days, hours, minutes, seconds) = match parts.len() {
        3 => {
            let first = parts[0];
            if let Some(dash_idx) = first.find('-') {
                let d: u64 = first[..dash_idx].parse().unwrap_or(0);
                let h: u64 = first[dash_idx + 1..].parse().unwrap_or(0);
                let m: u64 = parts[1].parse().unwrap_or(0);
                let s: u64 = parts[2].parse().unwrap_or(0);
                (d, h, m, s)
            } else {
                let h: u64 = first.trim().parse().unwrap_or(0);
                let m: u64 = parts[1].parse().unwrap_or(0);
                let s: u64 = parts[2].parse().unwrap_or(0);
                (0, h, m, s)
            }
        }
        2 => {
            let m: u64 = parts[0].trim().parse().unwrap_or(0);
            let s: u64 = parts[1].parse().unwrap_or(0);
            (0, 0, m, s)
        }
        _ => (0, 0, 0, 0),
    };
    Duration::from_secs(days * 86400 + hours * 3600 + minutes * 60 + seconds)
}

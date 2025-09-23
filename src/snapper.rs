use anyhow::{Context, Result};
use std::{fs, process::Command};

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: u64,
    pub config: String,
    // 'type' is a reserved word in Rust; use 'kind' to represent snapper's Type column
    pub kind: String,
    pub cleanup: String,
    pub user: String,
    pub date: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
}

pub struct Snapper;

impl Snapper {
    fn run_snapper(args: &[&str], use_sudo: bool) -> Result<std::process::Output> {
        let out = if use_sudo {
            // Non-interactive: if sudo needs a password, fail fast so UI can show a hint.
            Command::new("sudo").args(["-n", "snapper"]).args(args).output()
        } else {
            Command::new("snapper").args(args).output()
        }
        .context("Failed to spawn snapper")?;
        Ok(out)
    }
    pub fn available_configs_fs() -> Vec<String> {
        let mut v = Vec::new();
        if let Ok(entries) = fs::read_dir("/etc/snapper/configs") {
            for e in entries.flatten() {
                if let Some(name) = e.file_name().to_str() { v.push(name.to_string()); }
            }
            v.sort();
        }
        v
    }

    pub fn config_exists(name: &str) -> bool {
        let path = format!("/etc/snapper/configs/{name}");
        fs::metadata(&path).is_ok()
    }
    pub fn list_configs() -> Result<Vec<Config>> {
        // Preferred: read names from /etc/snapper/configs (avoids headers/formatting)
        let mut fs_configs: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir("/etc/snapper/configs") {
            for e in entries.flatten() {
                if let Some(name) = e.file_name().to_str() { fs_configs.push(name.to_string()); }
            }
            fs_configs.sort();
            fs_configs.dedup();
        }
        if !fs_configs.is_empty() {
            return Ok(fs_configs.into_iter().map(|name| Config { name }).collect());
        }

        // Fallback: parse `snapper list-configs`
        let out = Command::new("snapper").arg("list-configs").output().context("Failed to run snapper list-configs")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let hint = if err.to_ascii_lowercase().contains("permission") || err.to_ascii_lowercase().contains("dbus") {
                " (hint: try running with sudo)"
            } else { "" };
            anyhow::bail!("snapper list-configs failed: {err}{hint}");
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut names: Vec<String> = Vec::new();
        for raw in stdout.lines() {
            let line = raw.trim();
            if line.is_empty() { continue; }
            let low = line.to_ascii_lowercase();
            // Skip common header/separator lines
            if low.starts_with("config") || low.contains("subvolume") || low.contains("type") || line.starts_with("---") || line.starts_with('#') { continue; }
            // Grab first token or first column before '|'
            let token = if let Some((first, _)) = line.split_once('|') { first.trim() } else { line.split_whitespace().next().unwrap_or("") };
            if token.is_empty() { continue; }
            // Filter out obvious non-names
            if token.eq_ignore_ascii_case("name") || token.eq_ignore_ascii_case("configs") { continue; }
            names.push(token.trim_matches('*').to_string());
        }
        names.retain(|n| !n.is_empty() && Self::config_exists(n));
        names.sort();
        names.dedup();
        Ok(names.into_iter().map(|name| Config { name }).collect())
    }

    pub fn list_snapshots(config: &str, use_sudo: bool) -> Result<Vec<Snapshot>> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        // Prefer a narrow, stable set of columns for robust parsing
        // Columns (to match SnapperGUI layout): number | date | user | description | cleanup | type
        let out = Self::run_snapper(&["-c", config, "list", "--columns", "number,date,user,description,cleanup,type"], use_sudo)
            .with_context(|| format!("Failed to run snapper list for config {config}"))?;
        if !out.status.success() {
            // Fallback for older snapper without --columns support: try plain 'list'
            let fallback = Self::run_snapper(&["-c", config, "list"], use_sudo)?;
            if !fallback.status.success() {
                let err = String::from_utf8_lossy(&fallback.stderr);
                let err_lower = err.to_ascii_lowercase();
                let hint = if err_lower.contains("unknown config") || err_lower.contains("config not found") {
                    " (hint: check your config name; see /etc/snapper/configs)"
                } else if err_lower.contains("a password is required") && use_sudo {
                    " (hint: run 'sudo -v' to cache credentials)"
                } else if err_lower.contains("permission") || err_lower.contains("dbus") {
                    if use_sudo { " (hint: run 'make sudo-run')" } else { " (hint: try running with sudo)" }
                } else { "" };
                anyhow::bail!("snapper list failed: {err}{hint}");
            }
            // parse fallback wide table
            let stdout = String::from_utf8_lossy(&fallback.stdout);
            let mut snaps = Vec::new();
            for line in stdout.lines() {
                let lt = line.trim();
                if lt.is_empty() || lt.starts_with('#') || lt.starts_with("---") || lt.contains('┼') || lt.chars().all(|c| c == '─' || c == '┼' || c.is_whitespace()) {
                    continue;
                }
                let parts: Vec<&str> = lt
                    .split(|c| c == '|' || c == '│')
                    .map(|s| s.trim())
                    .collect();
                if parts.len() >= 7 {
                    if let Ok(id) = parts[0].parse::<u64>() {
                        let kind = parts.get(1).copied().unwrap_or("").to_string();
                        let date = parts.get(3).unwrap_or(&"").to_string();
                        let cleanup = parts.get(5).copied().unwrap_or("").to_string();
                        let mut description = parts.get(6).copied().unwrap_or("").to_string();
                        let user = String::new();
                        if description.is_empty() {
                            // fallback to type or cleanup hint if description missing
                            description = if !cleanup.is_empty() && cleanup != "-" { format!("[{}]", cleanup) }
                                          else if !kind.is_empty() && kind != "-" { format!("[{}]", kind) }
                                          else { String::from("(no description)") };
                        }
                        snaps.push(Snapshot { id, config: config.to_string(), kind, cleanup, user, date, description });
                    }
                } else if parts.len() >= 4 {
                    if let Ok(id) = parts[0].parse::<u64>() {
                        let kind = parts.get(1).copied().unwrap_or("").to_string();
                        let date = parts.get(3).unwrap_or(&"").to_string();
                        let description = parts.last().copied().unwrap_or("").to_string();
                        let description = if description.is_empty() { String::from("(no description)") } else { description };
                        let cleanup = String::new();
                        let user = String::new();
                        snaps.push(Snapshot { id, config: config.to_string(), kind, cleanup, user, date, description });
                    }
                }
            }
            return Ok(snaps);
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut snaps = Vec::new();
    for line in stdout.lines() {
            // Expected columns now: number | type | cleanup | date | description
            let lt = line.trim();
            if lt.is_empty() || lt.starts_with('#') || lt.starts_with("---") || lt.contains('┼') || lt.chars().all(|c| c == '─' || c == '┼' || c.is_whitespace()) {
                continue;
            }
            // Normalize to ASCII '|' and split into at most five fields: number | type | cleanup | date | description
            let normalized = lt.replace('│', "|");
            let mut it = normalized.splitn(6, '|').map(|s| s.trim());
            let c1 = it.next();
            let c2 = it.next(); // date
            let c3 = it.next(); // user
            let c4 = it.next(); // description
            let c5 = it.next(); // cleanup
            let c6 = it.next(); // type
            if let Some(id_str) = c1 {
                if let Ok(id) = id_str.parse::<u64>() {
                    if let (Some(date), Some(user), Some(desc), Some(cleanup_col), Some(type_col)) = (c2, c3, c4, c5, c6) {
                        let mut description = desc.to_string();
                        if description.is_empty() {
                            let cleanup = cleanup_col.trim();
                            let t = type_col.trim();
                            description = if !cleanup.is_empty() && cleanup != "-" {
                                format!("[{}]", cleanup)
                            } else if !t.is_empty() && t != "-" {
                                format!("[{}]", t)
                            } else {
                                String::from("(no description)")
                            };
                        }
                        snaps.push(Snapshot {
                            id,
                            config: config.to_string(),
                            kind: type_col.to_string(),
                            cleanup: cleanup_col.to_string(),
                            user: user.to_string(),
                            date: date.to_string(),
                            description,
                        });
                    } else if let (Some(date), Some(desc)) = (c2, c3) {
                        // Fallback for three columns: number | date | description (older formats)
                        let description = if desc.is_empty() { String::from("(no description)") } else { desc.to_string() };
                        snaps.push(Snapshot { id, config: config.to_string(), kind: String::new(), cleanup: String::new(), user: String::new(), date: date.to_string(), description });
                    }
                }
            }
        }
        Ok(snaps)
    }

    pub fn snapshot_status(config: &str, from: u64, to: u64, use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let range = format!("{}..{}", from, to);
        let out = Self::run_snapper(&["-c", config, "status", &range], use_sudo)
            .with_context(|| format!("Failed to run snapper status for {config} {range}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let hint = if err.to_ascii_lowercase().contains("permission") || err.to_ascii_lowercase().contains("dbus") {
                " (hint: try running with sudo)"
            } else { "" };
            anyhow::bail!("snapper status failed: {err}{hint}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn create(config: &str, description: &str, use_sudo: bool) -> Result<()> {
        let out = Self::run_snapper(&["-c", config, "create", "-d", description], use_sudo)
            .with_context(|| format!("Failed to run snapper create for {config}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper create failed: {}", stderr);
        }
        Ok(())
    }

    pub fn modify(config: &str, id: u64, description: &str, use_sudo: bool) -> Result<()> {
        let out = Self::run_snapper(&["-c", config, "modify", &id.to_string(), "-d", description], use_sudo)
            .with_context(|| format!("Failed to run snapper modify for {config}#{id}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper modify failed: {}", stderr);
        }
        Ok(())
    }

    pub fn delete(config: &str, id: u64, use_sudo: bool) -> Result<()> {
        let out = Self::run_snapper(&["-c", config, "delete", &id.to_string()], use_sudo)
            .with_context(|| format!("Failed to run snapper delete for {config}#{id}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper delete failed: {}", stderr);
        }
        Ok(())
    }

    pub fn diff(config: &str, from: u64, to: u64, use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let range = format!("{}..{}", from, to);
        let out = Self::run_snapper(&["-c", config, "diff", &range], use_sudo)
            .with_context(|| format!("Failed to run snapper diff for {config} {range}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper diff failed: {err}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn mount(config: &str, id: u64, use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let out = Self::run_snapper(&["-c", config, "mount", &id.to_string()], use_sudo)
            .with_context(|| format!("Failed to run snapper mount for {config}#{id}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper mount failed: {err}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn umount(config: &str, id: u64, use_sudo: bool) -> Result<()> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let out = Self::run_snapper(&["-c", config, "umount", &id.to_string()], use_sudo)
            .with_context(|| format!("Failed to run snapper umount for {config}#{id}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper umount failed: {err}");
        }
        Ok(())
    }

    pub fn rollback(config: &str, id: u64, use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let out = Self::run_snapper(&["-c", config, "rollback", &id.to_string()], use_sudo)
            .with_context(|| format!("Failed to run snapper rollback for {config}#{id}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper rollback failed: {err}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn cleanup(config: &str, algorithm: &str, use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let out = Self::run_snapper(&["-c", config, "cleanup", algorithm], use_sudo)
            .with_context(|| format!("Failed to run snapper cleanup {algorithm} for {config}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper cleanup failed: {err}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn get_config(config: &str, use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let out = Self::run_snapper(&["-c", config, "get-config"], use_sudo)
            .with_context(|| format!("Failed to run snapper get-config for {config}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper get-config failed: {err}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn set_config(config: &str, kv_pairs: &[String], use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let mut args = vec!["-c", config, "set-config"];
        for kv in kv_pairs.iter() { args.push(kv.as_str()); }
        let out = Self::run_snapper(&args, use_sudo)
            .with_context(|| format!("Failed to run snapper set-config for {config}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper set-config failed: {err}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn setup_quota(config: &str, use_sudo: bool) -> Result<String> {
        if !Self::config_exists(config) {
            anyhow::bail!("Unknown config '{config}' (not found in /etc/snapper/configs)");
        }
        let out = Self::run_snapper(&["-c", config, "setup-quota"], use_sudo)
            .with_context(|| format!("Failed to run snapper setup-quota for {config}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("snapper setup-quota failed: {err}");
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

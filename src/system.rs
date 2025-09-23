use anyhow::{Context, Result};
use std::process::Command;

pub fn has_cmd(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

pub fn run(cmd: &str, args: &[&str], use_sudo: bool) -> Result<std::process::Output> {
    if use_sudo {
        let out = Command::new("sudo")
            .arg("-n")
            .arg(cmd)
            .args(args)
            .output()
            .with_context(|| format!("Failed to run sudo {cmd}"))?;
        Ok(out)
    } else {
        Command::new(cmd)
            .args(args)
            .output()
            .with_context(|| format!("Failed to run {cmd}"))
    }
}

pub fn run_string(cmd: &str, args: &[&str], use_sudo: bool) -> Result<String> {
    let out = run(cmd, args, use_sudo)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("{} failed: {}", cmd, err.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

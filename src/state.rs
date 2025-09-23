use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub use_sudo: bool,
    pub snaps_fullscreen: bool,
    pub last_config: Option<String>,
    pub filter: Option<String>,
}

fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let mut p = PathBuf::from(xdg);
        p.push("snapper-tui");
        return p;
    }
    if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".config");
        p.push("snapper-tui");
        return p;
    }
    // fallback: current directory
    PathBuf::from(".snapper-tui")
}

fn state_path() -> PathBuf {
    let mut p = config_dir();
    p.push("state.json");
    p
}

impl State {
    pub fn load() -> Self {
        let path = state_path();
        if let Ok(data) = fs::read(&path) {
            if let Ok(s) = serde_json::from_slice::<State>(&data) {
                return s;
            }
        }
        State::default()
    }

    pub fn save(&self) {
        let dir = config_dir();
        let _ = fs::create_dir_all(&dir);
        let path = state_path();
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            if let Ok(mut f) = fs::File::create(path) {
                let _ = f.write_all(&json);
            }
        }
    }
}

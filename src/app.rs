use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::snapper::{Config, Snapshot, Snapper};
use crate::limine::Limine;
use crate::state::State as PersistedState;
use std::collections::HashMap;
use anyhow::Result;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Configs,
    Snapshots,
}

impl Default for Focus {
    fn default() -> Self { Focus::Snapshots }
}

#[derive(Debug, Default, Clone)]
pub struct ListState {
    pub selected: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum Mode {
    Normal,
    Input(InputKind),
    ConfirmDelete(u64),
    ConfirmRollback(u64),
    ConfirmCleanup(String),
    Help,
    Details,
    Loading,
    ConfigForm,
}

impl Default for Mode {
    fn default() -> Self { Mode::Normal }
}

#[derive(Debug, Clone)]
pub enum InputKind {
    Create,
    Edit(u64),
    CleanupAlgorithm, // expects algorithms like number|timeline|empty-pre-post
    DetailsSearch,
    ConfigFieldEdit(usize),
    Filter,
}

#[derive(Debug, Clone)]
pub enum PendingOp {
    Status { from: u64, to: u64 },
    Diff { from: u64, to: u64 },
    Mount { id: u64 },
    Umount { id: u64 },
    Rollback { id: u64 },
    Cleanup { algorithm: String },
    SetupQuota,
    SetConfig,
    GetConfigForEdit,
    LimineSync { id: u64, name: String },
}

#[derive(Default)]
pub struct App {
    pub status: String,
    pub focus: Focus,
    pub configs: Vec<Config>,
    // raw snapshots fetched from snapper (unfiltered)
    pub snapshots: Vec<Snapshot>,
    // filtered view
    pub filtered_snaps: Vec<Snapshot>,
    pub configs_state: ListState,
    pub snaps_state: ListState,
    pub mode: Mode,
    pub input: String,
    pub use_sudo: bool,
    pub input_cursor: usize, // cursor position in chars within input
    // Details overlay state
    pub details_text: String,
    pub details_scroll: u16,
    pub details_lines: u16,
    // View options
    pub snaps_fullscreen: bool,
    pub filter_text: String,
    // Animation / background work
    pub tick: u64,
    pub status_rx: Option<Receiver<Result<String>>>,
    pub snaps_rx: Option<Receiver<Result<Vec<Snapshot>>>>,
    pub snaps_loading_for: Option<String>,
    // cache snapshots with a freshness timestamp
    pub snaps_cache: HashMap<String, (Vec<Snapshot>, Instant)>,
    pub snaps_cache_ttl: Duration,
    pub pending: Option<PendingOp>,
    pub loading_message: String,
    pub details_title: String,
    pub details_query: String,
    // Config form editor state
    pub cfg_fields: Vec<ConfigField>,
    pub cfg_field_idx: Option<usize>,
    // Layout toggles resembling SnapperGUI bottom bar
    pub show_userdata: bool,
}
#[derive(Debug, Clone)]
pub struct ConfigField {
    pub key: String,
    pub value: String,
    pub original: String,
    pub modified: bool,
}

impl App {
    pub fn new() -> Self {
        let mut s = Self::default();
        // load persisted state
        let persisted = PersistedState::load();
        s.use_sudo = persisted.use_sudo;
        s.input_cursor = 0;
        s.details_scroll = 0;
        s.details_lines = 0;
        s.snaps_fullscreen = persisted.snaps_fullscreen;
        s.tick = 0;
        s.status_rx = None;
    s.snaps_rx = None;
    s.snaps_loading_for = None;
    // no deferred scheduling to keep navigation snappy
    s.snaps_cache = HashMap::new();
    s.snaps_cache_ttl = Duration::from_secs(3);
        s.pending = None;
        s.loading_message.clear();
        s.details_title = String::from("Snapshot status");
        s.details_query = String::new();
        s.cfg_fields = Vec::new();
        s.cfg_field_idx = None;
        s.filtered_snaps = Vec::new();
        s.filter_text = persisted.filter.unwrap_or_default();
    s.show_userdata = false;
        s.refresh_all();
        // try to restore last selected config
        if let Some(last) = persisted.last_config {
            if let Some(idx) = s.configs.iter().position(|c| c.name == last) {
                s.configs_state.selected = Some(idx);
                s.load_snapshots_for_selected();
            }
        }
        s
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        match &mut self.mode {
            Mode::Normal => {
                match key.code {
                    KeyCode::Tab => { if key.modifiers.contains(KeyModifiers::SHIFT) { self.select_prev_config(); } else { self.select_next_config(); } },
                    KeyCode::Up => self.on_up(),
                    KeyCode::Down => self.on_down(),
                    KeyCode::PageUp => self.on_page_up(),
                    KeyCode::PageDown => self.on_page_down(),
                    KeyCode::Home => self.on_home(),
                    KeyCode::End => self.on_end(),
                    KeyCode::Char('r') => self.refresh_all(),
                    KeyCode::Char('c') => self.start_create(),
                    KeyCode::Char('e') => self.start_edit(),
                    KeyCode::Char('d') => self.start_delete_confirm(),
                    KeyCode::Char('g') => { self.start_config_edit(); },
                    KeyCode::Char('?') => { self.mode = Mode::Help; },
                    KeyCode::Enter => { self.on_enter(); },
                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => { self.start_filter_input(); },
                    KeyCode::Char('F') => { self.start_filter_input(); },
                    KeyCode::Char('f') => { self.snaps_fullscreen = !self.snaps_fullscreen; self.persist_state(); },
                    KeyCode::Char('x') => { self.on_diff(); },
                    KeyCode::Char('m') => { self.on_mount(); },
                    KeyCode::Char('U') => { self.on_umount(); },
                    KeyCode::Char('R') => { self.start_rollback_confirm(); },
                    KeyCode::Char('K') => { self.start_cleanup_input(); },
                    KeyCode::Char('C') => { self.view_config(); },
                    KeyCode::Char('Q') => { self.setup_quota(); },
                    KeyCode::Char('Y') => { self.sync_limine_for_selected(); },
                    KeyCode::Char('u') => { self.show_userdata = !self.show_userdata; },
                    KeyCode::Char('[') => { self.select_prev_config(); },
                    KeyCode::Char(']') => { self.select_next_config(); },
                    KeyCode::Left => { self.select_prev_config(); },
                    KeyCode::Right => { self.select_next_config(); },
                    KeyCode::Char('S') => { self.use_sudo = !self.use_sudo; self.status = format!("sudo: {}", if self.use_sudo { "on" } else { "off" }); self.persist_state(); self.snaps_cache.clear(); self.refresh_all(); },
                    _ => {}
                }
            }
            Mode::Input(kind) => {
                match key.code {
                    KeyCode::Esc => {
                        match kind {
                            InputKind::ConfigFieldEdit(_) => {
                                // Return to the config form instead of closing entirely
                                self.mode = Mode::ConfigForm;
                                self.input.clear();
                                // keep status unchanged; no pending op to cancel here
                            }
                            _ => {
                                self.mode = Mode::Normal;
                                self.input.clear();
                                self.status = "Cancelled".into();
                                self.status_rx = None;
                                self.pending = None;
                            }
                        }
                    }
                    KeyCode::Enter => {
                        let text = self.input.trim().to_string();
                        match kind.clone() {
                            InputKind::Create => self.finish_create(&text),
                            InputKind::Edit(id) => self.finish_edit(id, &text),
                            InputKind::CleanupAlgorithm => self.finish_cleanup(&text),
                            InputKind::DetailsSearch => self.finish_details_search(&text),
                            InputKind::ConfigFieldEdit(idx) => self.finish_config_field_edit(idx, &text),
                            InputKind::Filter => { self.filter_text = text; self.apply_filter(); self.snaps_state.selected = if self.filtered_snaps.is_empty() { None } else { Some(0) }; self.persist_state(); self.mode = Mode::Normal; }
                        }
                    }
                    KeyCode::Backspace => { self.input_backspace(); }
                    KeyCode::Delete => { self.input_delete(); }
                    KeyCode::Left => { self.input_move_left(); }
                    KeyCode::Right => { self.input_move_right(); }
                    KeyCode::Home => { self.input_move_home(); }
                    KeyCode::End => { self.input_move_end(); }
                    KeyCode::Char(c) => {
                        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                            self.input_insert_char(c);
                        }
                    }
                    _ => {}
                }
            }
            Mode::ConfirmDelete(id) => {
                match key.code {
                    KeyCode::Esc => { self.mode = Mode::Normal; self.status = "Delete cancelled".into(); }
                    KeyCode::Char('y') => { let id = *id; self.mode = Mode::Normal; self.on_delete_confirmed(id); }
                    KeyCode::Char('n') => { self.mode = Mode::Normal; self.status = "Delete cancelled".into(); }
                    _ => {}
                }
            }
            Mode::ConfirmRollback(id) => {
                match key.code {
                    KeyCode::Esc => { self.mode = Mode::Normal; self.status = "Rollback cancelled".into(); }
                    KeyCode::Char('y') => { let id = *id; self.mode = Mode::Normal; self.on_rollback_confirmed(id); }
                    KeyCode::Char('n') => { self.mode = Mode::Normal; self.status = "Rollback cancelled".into(); }
                    _ => {}
                }
            }
            Mode::ConfirmCleanup(alg) => {
                match key.code {
                    KeyCode::Esc => { self.mode = Mode::Normal; self.status = "Cleanup cancelled".into(); }
                    KeyCode::Char('y') => { let alg = alg.clone(); self.mode = Mode::Normal; self.on_cleanup_confirmed(&alg); }
                    KeyCode::Char('n') => { self.mode = Mode::Normal; self.status = "Cleanup cancelled".into(); }
                    _ => {}
                }
            }
            Mode::Help => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) { self.mode = Mode::Normal; }
            }
            Mode::Details => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => { self.mode = Mode::Normal; }
                    KeyCode::Up => { self.details_scroll = self.details_scroll.saturating_sub(1); }
                    KeyCode::Down => {
                        let max = self.details_lines.saturating_sub(1);
                        if self.details_scroll < max { self.details_scroll = self.details_scroll.saturating_add(1); }
                    }
                    KeyCode::PageUp => { self.details_scroll = self.details_scroll.saturating_sub(10); }
                    KeyCode::PageDown => { self.details_scroll = self.details_scroll.saturating_add(10); }
                    KeyCode::Home => { self.details_scroll = 0; }
                    KeyCode::End => { self.details_scroll = self.details_lines.saturating_sub(1); }
                    KeyCode::Char('/') => { self.start_details_search(); }
                    KeyCode::Char('n') => { self.find_next(); }
                    KeyCode::Char('N') => { self.find_prev(); }
                    KeyCode::Char('e') => { self.start_config_edit(); }
                    _ => {}
                }
            }
            Mode::Loading => {
                // Allow cancel while loading
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => { self.mode = Mode::Normal; self.status_rx = None; self.pending = None; self.status = "Cancelled".into(); }
                    _ => {}
                }
            }
            Mode::ConfigForm => {
                match key.code {
                    KeyCode::Esc => { self.mode = Mode::Normal; self.status = "Config edit cancelled".into(); }
                    KeyCode::Up => {
                        if let Some(i) = self.cfg_field_idx { self.cfg_field_idx = Some(i.saturating_sub(1)); } else if !self.cfg_fields.is_empty() { self.cfg_field_idx = Some(0); }
                    }
                    KeyCode::Down => {
                        if let Some(i) = self.cfg_field_idx { let last = self.cfg_fields.len().saturating_sub(1); self.cfg_field_idx = Some((i + 1).min(last)); } else if !self.cfg_fields.is_empty() { self.cfg_field_idx = Some(0); }
                    }
                    KeyCode::Home => { if !self.cfg_fields.is_empty() { self.cfg_field_idx = Some(0); } }
                    KeyCode::End => { let last = self.cfg_fields.len().saturating_sub(1); if !self.cfg_fields.is_empty() { self.cfg_field_idx = Some(last); } }
                    KeyCode::Enter | KeyCode::Char('e') => { if let Some(i) = self.cfg_field_idx { if let Some(f) = self.cfg_fields.get(i) { self.input = f.value.clone(); self.input_cursor = self.input.chars().count(); self.mode = Mode::Input(InputKind::ConfigFieldEdit(i)); } } }
                    KeyCode::Char('s') | KeyCode::Char('y') => { self.apply_config_form_changes(); }
                    _ => {}
                }
            }
        }
    }

    fn sync_limine_for_selected(&mut self) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
        let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot".into(); return; };
        let Some(s) = self.filtered_snaps.get(sidx) else { return; };
        let id = s.id;
        let name = format!("{}-{}", cfg, id);
        // Spawn background thread calling Limine logic
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let use_sudo = self.use_sudo;
        let name_clone = name.clone();
        thread::spawn(move || {
            let res = Limine::sync_snapshot_to_limine(id, &name_clone, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::LimineSync { id, name: name.clone() });
        self.loading_message = format!("Syncing snapshot #{} to Limine…", id);
        self.details_title = format!("Limine sync for {}", name);
        self.mode = Mode::Loading;
        self.status.clear();
    }
    fn persist_state(&self) {
        let st = PersistedState {
            use_sudo: self.use_sudo,
            snaps_fullscreen: self.snaps_fullscreen,
            last_config: self.selected_config_name().map(|s| s.to_string()),
            filter: if self.filter_text.trim().is_empty() { None } else { Some(self.filter_text.clone()) },
        };
        st.save();
    }

    fn start_filter_input(&mut self) {
        self.input = self.filter_text.clone();
        self.input_cursor = self.input.chars().count();
        self.mode = Mode::Input(InputKind::Filter);
    }
    fn finish_config_field_edit(&mut self, idx: usize, val: &str) {
        if let Some(field) = self.cfg_fields.get_mut(idx) {
            let new_val = val.to_string();
            field.modified = new_val != field.original;
            field.value = new_val;
        }
        self.mode = Mode::ConfigForm;
    }

    fn apply_config_form_changes(&mut self) {
        let Some(cfg_name) = self.selected_config_name().map(|s| s.to_string()) else { self.status = "Select a config first".into(); self.mode = Mode::Normal; return; };
        let pairs: Vec<String> = self.cfg_fields.iter().filter(|f| f.modified).map(|f| format!("{}={}", f.key, f.value)).collect();
        if pairs.is_empty() { self.status = "No changes to apply".into(); self.mode = Mode::Normal; return; }
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::set_config(&cfg_name, &pairs, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::SetConfig);
        self.loading_message = String::from("Applying config changes…");
        self.status.clear();
        self.mode = Mode::Loading;
    }

    fn on_up(&mut self) {
        // Up navigates snapshot selection
        let len = self.filtered_snaps.len();
        if len == 0 { return; }
        let idx = self.snaps_state.selected.unwrap_or(0);
        let new = idx.saturating_sub(1);
        self.snaps_state.selected = Some(new);
    }

    fn on_down(&mut self) {
        // Down navigates snapshot selection
        let len = self.filtered_snaps.len();
        if len == 0 { return; }
        let idx = self.snaps_state.selected.unwrap_or(0);
        let new = (idx + 1).min(len - 1);
        self.snaps_state.selected = Some(new);
    }

    fn on_page_up(&mut self) {
        match self.focus {
            Focus::Configs => {
                let len = self.configs.len();
                if len == 0 { return; }
                let idx = self.configs_state.selected.unwrap_or(0);
                let new = idx.saturating_sub(10);
                self.configs_state.selected = Some(new);
                self.load_snapshots_for_selected();
                self.persist_state();
            }
            Focus::Snapshots => {
                let len = self.filtered_snaps.len();
                if len == 0 { return; }
                let idx = self.snaps_state.selected.unwrap_or(0);
                let new = idx.saturating_sub(10);
                self.snaps_state.selected = Some(new);
            }
        }
    }

    fn on_page_down(&mut self) {
        match self.focus {
            Focus::Configs => {
                let len = self.configs.len();
                if len == 0 { return; }
                let idx = self.configs_state.selected.unwrap_or(0);
                let new = (idx + 10).min(len.saturating_sub(1));
                self.configs_state.selected = Some(new);
                self.load_snapshots_for_selected();
                self.persist_state();
            }
            Focus::Snapshots => {
                let len = self.filtered_snaps.len();
                if len == 0 { return; }
                let idx = self.snaps_state.selected.unwrap_or(0);
                let new = (idx + 10).min(len.saturating_sub(1));
                self.snaps_state.selected = Some(new);
            }
        }
    }

    fn on_home(&mut self) {
        match self.focus {
            Focus::Configs => {
                if self.configs.is_empty() { return; }
                self.configs_state.selected = Some(0);
                self.load_snapshots_for_selected();
            }
            Focus::Snapshots => {
                if self.filtered_snaps.is_empty() { return; }
                self.snaps_state.selected = Some(0);
            }
        }
    }

    fn on_end(&mut self) {
        match self.focus {
            Focus::Configs => {
                let len = self.configs.len();
                if len == 0 { return; }
                self.configs_state.selected = Some(len - 1);
                self.load_snapshots_for_selected();
            }
            Focus::Snapshots => {
                let len = self.filtered_snaps.len();
                if len == 0 { return; }
                self.snaps_state.selected = Some(len - 1);
            }
        }
    }

    pub fn refresh_all(&mut self) {
        match Snapper::list_configs() {
            Ok(configs) => {
                self.configs = configs;
                if self.configs.is_empty() {
                    self.status = "No snapper configs found".into();
                    self.configs_state.selected = None;
                    self.snapshots.clear();
                    self.snaps_state.selected = None;
                } else {
                    // Clamp selection to valid range
                    let max = self.configs.len().saturating_sub(1);
                    self.configs_state.selected = Some(self.configs_state.selected.unwrap_or(0).min(max));
                    self.persist_state();
                    self.status.clear();
                    // Validate selected config exists in filesystem to avoid header/garbage entries
                    if let Some(i) = self.configs_state.selected {
                        if self.configs.get(i).map(|c| Snapper::config_exists(&c.name)).unwrap_or(false) {
                            self.load_snapshots_for_selected();
                        } else {
                            self.status = "Selected config is invalid; choose another".into();
                            self.snapshots.clear();
                            self.snaps_state.selected = None;
                        }
                    }
                }
            }
            Err(e) => {
                self.status = format!("Failed to list configs: {e}");
            }
        }
    }

    fn load_snapshots_for_selected(&mut self) {
        let Some(idx) = self.configs_state.selected else { return; };
        let Some(cfg) = self.configs.get(idx) else { return; };
        let cfg_name = cfg.name.clone();
        // apply cache immediately for responsiveness
        if let Some((cached, seen_at)) = self.snaps_cache.get(&cfg_name).cloned() {
            self.snapshots = cached;
            self.apply_filter();
            self.snaps_state.selected = if self.filtered_snaps.is_empty() { None } else { Some(0) };
            // If cache is fresh, avoid immediate refresh to reduce churn
            if seen_at.elapsed() < self.snaps_cache_ttl {
                self.status = format!("Cached snapshots for {} ({}s old)", cfg_name, seen_at.elapsed().as_secs());
                return;
            }
        } else {
            self.snapshots.clear();
            self.filtered_snaps.clear();
            self.snaps_state.selected = None;
        }
        // start background refresh; drop previous receiver if any
        self.snaps_rx = None;
        self.snaps_loading_for = None;
        let use_sudo = self.use_sudo;
        let (tx, rx) = mpsc::channel::<Result<Vec<Snapshot>>>();
        let cfg_name_for_thread = cfg_name.clone();
        thread::spawn(move || {
            let res = Snapper::list_snapshots(&cfg_name_for_thread, use_sudo);
            let _ = tx.send(res);
        });
        self.snaps_rx = Some(rx);
        self.snaps_loading_for = Some(cfg_name.clone());
    let status_prefix = if self.snaps_cache.contains_key(&cfg_name) { "Refreshing" } else { "Loading" };
        self.status = format!("{} snapshots for {}…", status_prefix, cfg_name);
    }

    fn selected_config_name(&self) -> Option<&str> {
        let idx = self.configs_state.selected?;
        Some(self.configs.get(idx)?.name.as_str())
    }

    fn start_create(&mut self) {
        if self.selected_config_name().is_none() { self.status = "Select a config first".into(); return; }
        self.input.clear();
        self.input_cursor = 0;
        self.mode = Mode::Input(InputKind::Create);
    }

    fn finish_create(&mut self, desc: &str) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
        match Snapper::create(cfg, if desc.is_empty() { "Created via snapper-tui" } else { desc }, self.use_sudo) {
            Ok(_) => {
                self.status = format!("Created snapshot in {cfg}");
                self.mode = Mode::Normal;
                self.input.clear();
                self.snaps_cache.clear();
                self.load_snapshots_for_selected();
            }
            Err(e) => { self.status = format!("Create failed: {e}"); self.mode = Mode::Normal; }
        }
    }

    fn start_edit(&mut self) {
        let Some(_cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
    let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot to edit".into(); return; };
    let Some(s) = self.filtered_snaps.get(sidx) else { return; };
        self.input = s.description.clone();
        self.input_cursor = self.input.chars().count();
        self.mode = Mode::Input(InputKind::Edit(s.id));
    }

    fn finish_edit(&mut self, id: u64, desc: &str) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
        match Snapper::modify(cfg, id, desc, self.use_sudo) {
            Ok(_) => {
                self.status = format!("Edited snapshot #{}", id);
                self.mode = Mode::Normal;
                self.input.clear();
                self.input_cursor = 0;
                self.snaps_cache.clear();
                self.load_snapshots_for_selected();
            }
            Err(e) => { self.status = format!("Edit failed: {e}"); self.mode = Mode::Normal; }
        }
    }

    fn start_delete_confirm(&mut self) {
        let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot to delete".into(); return; };
        let Some(s) = self.filtered_snaps.get(sidx) else { return; };
        self.mode = Mode::ConfirmDelete(s.id);
    }

    fn on_delete_confirmed(&mut self, id: u64) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
        match Snapper::delete(cfg, id, self.use_sudo) {
            Ok(_) => {
                self.status = format!("Deleted snapshot #{}", id);
                self.snaps_cache.clear();
                self.load_snapshots_for_selected();
            }
            Err(e) => self.status = format!("Delete failed: {e}"),
        }
    }

    fn on_enter(&mut self) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
    let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot".into(); return; };
    let Some(s) = self.filtered_snaps.get(sidx) else { return; };
        // Choose a sensible comparison range: previous -> current if possible, otherwise 0..current
        let (from, to) = if sidx > 0 {
            let prev = self.filtered_snaps.get(sidx - 1).map(|p| p.id).unwrap_or(0);
            (prev, s.id)
        } else {
            (0, s.id)
        };
        // Spawn background job to fetch status so UI can show a loading indicator
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg.to_string();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::snapshot_status(&cfg_owned, from, to, use_sudo);
            let _ = tx.send(res);
        });
    self.status_rx = Some(rx);
    self.pending = Some(PendingOp::Status { from, to });
    self.loading_message = format!("Fetching status {}..{}", from, to);
    self.details_title = format!("Status {}..{}", from, to);
        self.details_text.clear();
        self.details_lines = 0;
        self.details_scroll = 0;
        self.mode = Mode::Loading;
        self.status.clear();
    }

    pub fn on_tick(&mut self) {
        // advance animations (throttle)
        self.tick = self.tick.wrapping_add(1);
        // check background status work
        if let Some(rx) = &self.status_rx {
            match rx.try_recv() {
                Ok(Ok(text)) => {
                    match self.pending.clone() {
                        Some(PendingOp::Status { from, to }) => {
                            self.details_title = format!("Status {}..{}", from, to);
                            self.details_lines = text.lines().count() as u16;
                            self.details_text = text;
                            self.details_scroll = 0;
                            self.mode = Mode::Details;
                        }
                        Some(PendingOp::Diff { from, to }) => {
                            self.details_title = format!("Diff {}..{}", from, to);
                            self.details_lines = text.lines().count() as u16;
                            self.details_text = text;
                            self.details_scroll = 0;
                            self.mode = Mode::Details;
                        }
                        Some(PendingOp::Cleanup { algorithm }) => {
                            self.details_title = format!("Cleanup: {}", algorithm);
                            self.details_lines = text.lines().count() as u16;
                            self.details_text = text;
                            self.details_scroll = 0;
                            self.mode = Mode::Details;
                        }
                        Some(PendingOp::Mount { id }) => {
                            self.status = if text.trim().is_empty() { format!("Mounted #{}", id) } else { format!("Mounted #{}: {}", id, text.lines().next().unwrap_or("")) };
                            self.mode = Mode::Normal;
                        }
                        Some(PendingOp::Umount { id }) => {
                            self.status = if text.trim().is_empty() { format!("Unmounted #{}", id) } else { format!("Unmounted #{}: {}", id, text.lines().next().unwrap_or("")) };
                            self.mode = Mode::Normal;
                        }
                        Some(PendingOp::SetupQuota) => {
                            self.status = if text.trim().is_empty() { "Quota setup completed".into() } else { format!("Quota: {}", text.lines().next().unwrap_or("")) };
                            self.mode = Mode::Normal;
                            self.snaps_cache.clear();
                            self.refresh_all();
                        }
                        Some(PendingOp::Rollback { id }) => {
                            self.status = if text.trim().is_empty() { format!("Rollback to #{} completed", id) } else { format!("Rollback #{}: {}", id, text.lines().next().unwrap_or("")) };
                            self.mode = Mode::Normal;
                            self.snaps_cache.clear();
                            self.refresh_all();
                        }
                        Some(PendingOp::LimineSync { id, name }) => {
                            self.details_title = format!("Limine sync for {} (#{}): result", name, id);
                            self.details_lines = text.lines().count() as u16;
                            self.details_text = text;
                            self.details_scroll = 0;
                            self.mode = Mode::Details;
                        }
                        Some(PendingOp::SetConfig) => {
                            self.status = if text.trim().is_empty() { "Config updated".into() } else { format!("Set-config: {}", text.lines().next().unwrap_or("")) };
                            self.mode = Mode::Normal;
                            // config change may influence listing; keep conservative
                            self.snaps_cache.clear();
                        }
                        Some(PendingOp::GetConfigForEdit) => {
                            // Build form fields from get-config; accept multiple formats
                            // Formats seen: 'Key | Value', 'key=value', 'Key: Value', 'Key<TAB>Value', or aligned with 2+ spaces
                            self.cfg_fields.clear();
                            for line in text.lines() {
                                let raw = line.trim();
                                if raw.is_empty() { continue; }
                                let lower = raw.to_ascii_lowercase();
                                if raw.starts_with('#') || lower.starts_with("key") || lower.starts_with("config") { continue; }
                                if raw.chars().all(|c| c == '-' || c == '+' || c == '|' || c == '┼' || c == '─' || c.is_whitespace()) { continue; }
                                // Normalize various vertical bars
                                let t = raw
                                    .replace('│', "|")
                                    .replace('┃', "|")
                                    .replace('┆', "|")
                                    .replace('¦', "|");

                                let mut key: Option<String> = None;
                                let mut val: Option<String> = None;

                                // 1) Table with '|'
                                if key.is_none() && t.contains('|') {
                                    let mut it = t.splitn(2, '|');
                                    let k = it.next().unwrap_or("").trim();
                                    let v = it.next().unwrap_or("").trim();
                                    if !k.is_empty() { key = Some(k.to_string()); val = Some(v.to_string()); }
                                }
                                // 2) key=value
                                if key.is_none() && t.contains('=') {
                                    if let Some((k, v)) = t.split_once('=') {
                                        key = Some(k.trim().trim_end_matches(':').to_string());
                                        val = Some(v.trim().to_string());
                                    }
                                }
                                // 3) key: value
                                if key.is_none() && t.contains(':') {
                                    if let Some((k, v)) = t.split_once(':') {
                                        // avoid capturing leading 'Config: ...' lines
                                        if !k.trim().eq_ignore_ascii_case("config") {
                                            key = Some(k.trim().to_string());
                                            val = Some(v.trim().to_string());
                                        }
                                    }
                                }
                                // 4) key<TAB>value
                                if key.is_none() && t.contains('\t') {
                                    let mut it = t.splitn(2, '\t');
                                    let k = it.next().unwrap_or("").trim();
                                    let v = it.next().unwrap_or("").trim();
                                    if !k.is_empty() { key = Some(k.to_string()); val = Some(v.to_string()); }
                                }
                                // 5) key  value  (2+ spaces as delimiter)
                                if key.is_none() {
                                    let bytes = t.as_bytes();
                                    let mut i = 0usize;
                                    while i + 1 < bytes.len() {
                                        if bytes[i] == b' ' && bytes[i + 1] == b' ' {
                                            // advance to end of this run of spaces
                                            let mut j = i + 2;
                                            while j < bytes.len() && bytes[j] == b' ' { j += 1; }
                                            let k = t[..i].trim();
                                            let v = t[j..].trim();
                                            if !k.is_empty() && !v.is_empty() {
                                                key = Some(k.to_string());
                                                val = Some(v.to_string());
                                            }
                                            break;
                                        }
                                        i += 1;
                                    }
                                }

                                if let (Some(mut k), Some(v)) = (key, val) {
                                    if k.ends_with(':') { k.pop(); }
                                    if !k.is_empty() {
                                        self.cfg_fields.push(ConfigField { key: k, value: v.clone(), original: v, modified: false });
                                    }
                                }
                            }
                            self.cfg_field_idx = if self.cfg_fields.is_empty() { None } else { Some(0) };
                            self.mode = Mode::ConfigForm;
                            self.status.clear();
                        }
                        None => { self.mode = Mode::Normal; }
                    }
                    self.status_rx = None;
                    self.pending = None;
                }
                Ok(Err(e)) => {
                    self.status = format!("Operation failed: {e}");
                    self.mode = Mode::Normal;
                    self.status_rx = None;
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => { /* still loading */ }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Status failed (disconnected)".into();
                    self.mode = Mode::Normal;
                    self.status_rx = None;
                    self.pending = None;
                }
            }
        }
        // check background snapshots load
        if let Some(rx) = &self.snaps_rx {
            match rx.try_recv() {
                Ok(Ok(snaps)) => {
                    // store in cache
                    if let Some(cfg_name) = self.snaps_loading_for.clone() {
                        self.snaps_cache.insert(cfg_name.clone(), (snaps.clone(), Instant::now()));
                        // only apply to UI if the loaded config is still selected
                        if self.selected_config_name() == Some(cfg_name.as_str()) {
                            self.snapshots = snaps;
                            self.apply_filter();
                            self.snaps_state.selected = if self.filtered_snaps.is_empty() { None } else { Some(0) };
                            self.status.clear();
                        }
                    }
                    self.snaps_rx = None;
                    self.snaps_loading_for = None;
                }
                Ok(Err(e)) => {
                    let cfg_name = self.selected_config_name().unwrap_or("(unknown)");
                    let mut msg = format!("Failed to list snapshots for {}: {e}", cfg_name);
                    let lower = msg.to_ascii_lowercase();
                    if lower.contains("unknown config") || lower.contains("config not found") {
                        let known = Snapper::available_configs_fs();
                        if !known.is_empty() {
                            msg.push_str(&format!(" | Known configs: {}", known.join(", ")));
                        }
                    }
                    self.status = msg;
                    self.snapshots.clear();
                    self.filtered_snaps.clear();
                    self.snaps_state.selected = None;
                    self.snaps_rx = None;
                    self.snaps_loading_for = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = "Loading snapshots failed (disconnected)".into();
                    self.snaps_rx = None;
                    self.snaps_loading_for = None;
                }
            }
        }
        // no deferred loader; refresh spawned immediately on selection change
    }

    pub fn on_diff(&mut self) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
        let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot".into(); return; };
        let Some(s) = self.filtered_snaps.get(sidx) else { return; };
    let (from, to) = if sidx > 0 { (self.filtered_snaps.get(sidx - 1).map(|p| p.id).unwrap_or(0), s.id) } else { (0, s.id) };
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg.to_string();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::diff(&cfg_owned, from, to, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::Diff { from, to });
        self.loading_message = format!("Fetching diff {}..{}", from, to);
        self.details_title = format!("Diff {}..{}", from, to);
        self.mode = Mode::Loading;
        self.status.clear();
    }

    pub fn on_mount(&mut self) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
    let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot".into(); return; };
    let Some(s) = self.filtered_snaps.get(sidx) else { return; };
        let id = s.id;
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg.to_string();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::mount(&cfg_owned, id, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::Mount { id });
        self.loading_message = format!("Mounting #{}", id);
        self.mode = Mode::Loading;
        self.status.clear();
    }

    pub fn on_umount(&mut self) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
    let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot".into(); return; };
    let Some(s) = self.filtered_snaps.get(sidx) else { return; };
        let id = s.id;
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg.to_string();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::umount(&cfg_owned, id, use_sudo).map(|_| String::new());
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::Umount { id });
        self.loading_message = format!("Unmounting #{}", id);
        self.mode = Mode::Loading;
        self.status.clear();
    }

    fn start_rollback_confirm(&mut self) {
        let Some(sidx) = self.snaps_state.selected else { self.status = "Select a snapshot to rollback".into(); return; };
        let Some(s) = self.filtered_snaps.get(sidx) else { return; };
        self.mode = Mode::ConfirmRollback(s.id);
    }

    fn on_rollback_confirmed(&mut self, id: u64) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg.to_string();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::rollback(&cfg_owned, id, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::Rollback { id });
        self.loading_message = format!("Rolling back to #{}", id);
        self.status.clear();
        self.mode = Mode::Loading;
    }

    fn start_cleanup_input(&mut self) {
        if self.selected_config_name().is_none() { self.status = "Select a config first".into(); return; }
        self.input.clear();
        self.input_cursor = 0;
        self.mode = Mode::Input(InputKind::CleanupAlgorithm);
    }

    fn finish_cleanup(&mut self, alg: &str) {
        let alg = alg.trim();
        if alg.is_empty() { self.status = "Enter cleanup algorithm (e.g., number, timeline, empty-pre-post)".into(); self.mode = Mode::Normal; return; }
        self.mode = Mode::ConfirmCleanup(alg.to_string());
    }

    // Details search helpers (simple substring search)
    fn start_details_search(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
        self.mode = Mode::Input(InputKind::DetailsSearch);
    }

    fn finish_details_search(&mut self, q: &str) {
        let q = q.trim();
        if q.is_empty() { self.mode = Mode::Details; return; }
        self.details_query = q.to_string();
        self.mode = Mode::Details;
        self.jump_to_match_forward(true);
    }

    fn jump_to_match_forward(&mut self, wrap: bool) {
        if self.details_query.is_empty() { return; }
        let q = self.details_query.to_lowercase();
        let lines: Vec<&str> = self.details_text.lines().collect();
        let start = (self.details_scroll as usize).saturating_add(1).min(lines.len());
        for (i, line) in lines.iter().enumerate().skip(start) {
            if line.to_lowercase().contains(&q) { self.details_scroll = i as u16; return; }
        }
        if wrap {
            for (i, line) in lines.iter().enumerate() {
                if line.to_lowercase().contains(&q) { self.details_scroll = i as u16; return; }
            }
        }
    }

    fn jump_to_match_backward(&mut self, wrap: bool) {
        if self.details_query.is_empty() { return; }
        let q = self.details_query.to_lowercase();
        let lines: Vec<&str> = self.details_text.lines().collect();
        let start = self.details_scroll as isize - 1;
        for i in (0..=start.max(0) as usize).rev() {
            if lines[i].to_lowercase().contains(&q) { self.details_scroll = i as u16; return; }
        }
        if wrap {
            for i in (0..lines.len()).rev() {
                if lines[i].to_lowercase().contains(&q) { self.details_scroll = i as u16; return; }
            }
        }
    }

    fn find_next(&mut self) { self.jump_to_match_forward(true); }
    fn find_prev(&mut self) { self.jump_to_match_backward(true); }

    fn on_cleanup_confirmed(&mut self, alg: &str) {
        let Some(cfg) = self.selected_config_name() else { self.status = "Select a config first".into(); return; };
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg.to_string();
        let use_sudo = self.use_sudo;
        let alg_owned = alg.to_string();
        thread::spawn(move || {
            let res = Snapper::cleanup(&cfg_owned, &alg_owned, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::Cleanup { algorithm: alg.to_string() });
        self.loading_message = format!("Cleaning up: {}", alg);
        self.details_title = format!("Cleanup: {}", alg);
        self.status.clear();
        self.mode = Mode::Loading;
    }

    fn start_config_edit(&mut self) {
        // Form-based editor: load config first
        let Some(cfg_name) = self.selected_config_name().map(|s| s.to_string()) else { self.status = "Select a config first".into(); return; };
        self.cfg_fields.clear();
        self.cfg_field_idx = None;
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let use_sudo = self.use_sudo;
        std::thread::spawn(move || {
            let res = Snapper::get_config(&cfg_name, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::GetConfigForEdit);
        self.loading_message = String::from("Loading config for edit…");
        self.status.clear();
        self.mode = Mode::Loading;
    }


    fn view_config(&mut self) {
        let Some(cfg_name) = self.selected_config_name().map(|s| s.to_string()) else { self.status = "Select a config first".into(); return; };
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg_name.clone();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::get_config(&cfg_owned, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::Cleanup { algorithm: String::from("get-config") });
        self.loading_message = format!("Loading config: {}", cfg_name);
        self.details_title = format!("Config: {}", cfg_name);
        self.mode = Mode::Loading;
    }

    fn setup_quota(&mut self) {
        let Some(cfg_name) = self.selected_config_name().map(|s| s.to_string()) else { self.status = "Select a config first".into(); return; };
        let (tx, rx) = mpsc::channel::<Result<String>>();
        let cfg_owned = cfg_name.clone();
        let use_sudo = self.use_sudo;
        thread::spawn(move || {
            let res = Snapper::setup_quota(&cfg_owned, use_sudo);
            let _ = tx.send(res);
        });
        self.status_rx = Some(rx);
        self.pending = Some(PendingOp::SetupQuota);
        self.loading_message = format!("Setting up quota for {}", cfg_name);
        self.status.clear();
        self.mode = Mode::Loading;
    }

    // --- Input editing helpers ---
    fn input_len_chars(&self) -> usize { self.input.chars().count() }

    fn byte_index_of_char_pos(&self, pos: usize) -> usize {
        if pos == 0 { return 0; }
        let mut count = 0usize;
        for (i, _) in self.input.char_indices() {
            if count == pos { return i; }
            count += 1;
        }
        self.input.len()
    }

    fn input_move_left(&mut self) {
        if self.input_cursor > 0 { self.input_cursor -= 1; }
    }
    fn input_move_right(&mut self) {
        let len = self.input_len_chars();
        if self.input_cursor < len { self.input_cursor += 1; }
    }
    fn input_move_home(&mut self) { self.input_cursor = 0; }
    fn input_move_end(&mut self) { self.input_cursor = self.input_len_chars(); }

    fn input_backspace(&mut self) {
        if self.input_cursor == 0 { return; }
        let pos = self.input_cursor - 1;
        let start = self.byte_index_of_char_pos(pos);
        let end = self.byte_index_of_char_pos(self.input_cursor);
        self.input.replace_range(start..end, "");
        self.input_cursor = pos;
    }

    fn input_delete(&mut self) {
        let len = self.input_len_chars();
        if self.input_cursor >= len { return; }
        let start = self.byte_index_of_char_pos(self.input_cursor);
        let end = self.byte_index_of_char_pos(self.input_cursor + 1);
        self.input.replace_range(start..end, "");
    }

    fn input_insert_char(&mut self, c: char) {
        let idx = self.byte_index_of_char_pos(self.input_cursor);
        self.input.insert(idx, c);
        self.input_cursor += 1;
    }
}

impl App {
    pub fn on_mouse(&mut self, me: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseButton, MouseEventKind};
        match me.kind {
            MouseEventKind::ScrollUp => {
                match self.mode {
                    Mode::Details => { self.details_scroll = self.details_scroll.saturating_sub(3); }
                    _ => {
                        if self.focus == Focus::Snapshots {
                            if let Some(sel) = self.snaps_state.selected { self.snaps_state.selected = Some(sel.saturating_sub(1)); }
                        } else {
                            if let Some(sel) = self.configs_state.selected { self.configs_state.selected = Some(sel.saturating_sub(1)); self.load_snapshots_for_selected(); self.persist_state(); }
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                match self.mode {
                    Mode::Details => { self.details_scroll = self.details_scroll.saturating_add(3); }
                    _ => {
                        if self.focus == Focus::Snapshots {
                            let len = self.filtered_snaps.len();
                            if len > 0 { let sel = self.snaps_state.selected.unwrap_or(0); self.snaps_state.selected = Some((sel + 1).min(len - 1)); }
                        } else {
                            let len = self.configs.len();
                            if len > 0 { let sel = self.configs_state.selected.unwrap_or(0); self.configs_state.selected = Some((sel + 1).min(len - 1)); self.load_snapshots_for_selected(); self.persist_state(); }
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // No selection-by-click mapping yet; safe no-op for now
            }
            _ => {}
        }
    }
    fn apply_filter(&mut self) {
        let q = self.filter_text.trim().to_lowercase();
        if q.is_empty() {
            self.filtered_snaps = self.snapshots.clone();
        } else {
            self.filtered_snaps = self
                .snapshots
                .iter()
                .filter(|s|
                    s.description.to_lowercase().contains(&q)
                    || s.date.to_lowercase().contains(&q)
                    || s.kind.to_lowercase().contains(&q)
                    || s.cleanup.to_lowercase().contains(&q)
                    || s.user.to_lowercase().contains(&q)
                    || s.id.to_string().contains(&q)
                )
                .cloned()
                .collect();
        }
    }

    fn select_prev_config(&mut self) {
        let len = self.configs.len();
        if len == 0 { return; }
        let idx = self.configs_state.selected.unwrap_or(0);
        let new = idx.saturating_sub(1);
        self.configs_state.selected = Some(new);
        self.load_snapshots_for_selected();
        self.persist_state();
    }
    fn select_next_config(&mut self) {
        let len = self.configs.len();
        if len == 0 { return; }
        let idx = self.configs_state.selected.unwrap_or(0);
        let new = (idx + 1).min(len - 1);
        self.configs_state.selected = Some(new);
        self.load_snapshots_for_selected();
        self.persist_state();
    }
}

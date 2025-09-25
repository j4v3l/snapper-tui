// Integration test for State::load and State::save
// Uses a temporary directory to avoid polluting the user's config.
use snapper_tui::state::State;
use std::env;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_state_load_save_integration() {
    // Use a temp dir for XDG_CONFIG_HOME
    let tmp = tempfile::tempdir().expect("tempdir");
    let xdg_config = tmp.path().to_path_buf();
    env::set_var("XDG_CONFIG_HOME", &xdg_config);
    // Save a custom state
    let s = State {
        use_sudo: true,
        last_config: Some("integration_test".to_string()),
        filter: Some("bar".to_string()),
        show_userdata: true,
    };
    s.save();
    // Load it back
    let loaded = State::load();
    assert_eq!(loaded.use_sudo, true);
    assert_eq!(loaded.last_config.as_deref(), Some("integration_test"));
    assert_eq!(loaded.filter.as_deref(), Some("bar"));
    assert_eq!(loaded.show_userdata, true);
    // Check file exists
    let mut state_path = xdg_config;
    state_path.push("snapper-tui");
    state_path.push("state.json");
    assert!(state_path.exists());
    // Clean up
    let _ = fs::remove_file(&state_path);
}

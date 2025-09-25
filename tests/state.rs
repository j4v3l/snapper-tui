// Unit test scaffolding for state.rs
// Note: File system and env-dependent functions require integration/mocking for full coverage.
use snapper_tui::state::State;

#[test]
fn test_state_struct_default() {
    let s = State::default();
    assert!(!s.use_sudo);
    assert!(s.last_config.is_none());
    assert!(s.filter.is_none());
    assert!(!s.show_userdata);
}

#[test]
fn test_state_struct_custom() {
    let s = State {
        use_sudo: true,
        last_config: Some("root".to_string()),
        filter: Some("foo".to_string()),
        show_userdata: true,
    };
    assert!(s.use_sudo);
    assert_eq!(s.last_config.as_deref(), Some("root"));
    assert_eq!(s.filter.as_deref(), Some("foo"));
    assert!(s.show_userdata);
}

// TODO: Integration/mocking required for the following:
// - State::load
// - State::save

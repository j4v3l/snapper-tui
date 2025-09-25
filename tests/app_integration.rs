// Integration test for App::new and basic event handling
// Simulates key events and checks state transitions or for no panic.
use snapper_tui::app::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn test_app_new_and_on_key() {
    let mut app = App::new();
    // Initial mode should be Normal
    match app.mode {
        Mode::Normal => {},
        _ => panic!("App mode should be Normal after new()"),
    }
    // Simulate Tab key (should not panic)
    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    app.on_key(tab);
    // Simulate Up key
    let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    app.on_key(up);
    // Simulate Down key
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    app.on_key(down);
    // Simulate '?' key (should enter Help mode)
    let help = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    app.on_key(help);
    match app.mode {
        Mode::Help => {},
        _ => panic!("App mode should be Help after '?' key"),
    }
}

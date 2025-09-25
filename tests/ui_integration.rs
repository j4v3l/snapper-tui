// Integration test for ui.rs pure logic helpers
// Tests normalize_text_for_ui and prewrap_text for expected output.
use snapper_tui::ui;

#[test]
fn test_normalize_text_for_ui() {
    let input = "Hello\tWorld\u{1b}[31mRed\u{1b}[0m";
    let output = ui::normalize_text_for_ui(input);
    // Should replace tab with spaces and strip ANSI
    assert!(output.contains("    World"));
    assert!(output.contains("Red"));
    assert!(!output.contains("\u{1b}"));
}

#[test]
fn test_prewrap_text() {
    let input = "This is a long line that should be wrapped.";
    let output = ui::prewrap_text(input, 10);
    // Should insert newlines so that no line is longer than 10 chars
    for line in output.lines() {
        assert!(line.chars().count() <= 10);
    }
}

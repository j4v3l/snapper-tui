// Integration test for Theme block rendering methods
// Checks that block, modal_block, modal_warn_block, and modal_error_block construct without panic.
use snapper_tui::theme::Theme;
use ratatui::widgets::Block;

#[test]
fn test_theme_block_rendering_methods() {
    let t = Theme::default();
    let b: Block = t.block("Test");
    let m: Block = t.modal_block("Modal");
    let w: Block = t.modal_warn_block("Warn");
    let e: Block = t.modal_error_block("Error");
    // Type check: ensure all are Block
    let _ = b;
    let _ = m;
    let _ = w;
    let _ = e;
}

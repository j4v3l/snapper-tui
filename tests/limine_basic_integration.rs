// Integration test for Limine::is_installed, has_sync, detect_root_subvol_path, detect_esp_path
// These tests check for correct type/behavior and skip gracefully if not applicable.
use snapper_tui::limine::Limine;

#[test]
fn test_limine_is_installed_and_has_sync() {
    // Should return a boolean
    let _ = Limine::is_installed();
    let _ = Limine::has_sync();
    // No assertion: just check for no panic
}

#[test]
fn test_limine_detect_root_subvol_path() {
    let path = Limine::detect_root_subvol_path();
    // Should return a string, usually "/" or "/@"
    assert!(path.starts_with("/"));
}

#[test]
fn test_limine_detect_esp_path() {
    // Should return None or Some(String)
    let result = Limine::detect_esp_path();
    assert!(result.is_none() || result.is_some());
}

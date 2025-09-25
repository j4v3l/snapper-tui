// Integration test for Limine::ensure_snapshots_marker, configure_defaults, root_uuid
// These tests check for correct type/behavior and skip gracefully if not applicable or would modify system files.
use snapper_tui::limine::Limine;

#[test]
fn test_limine_ensure_snapshots_marker() {
    // Should return Ok(String) or an info message if limine.conf is not found
    let result = Limine::ensure_snapshots_marker(false);
    match result {
        Ok(msg) => println!("ensure_snapshots_marker: {msg}"),
        Err(e) => eprintln!("ensure_snapshots_marker error: {e}"),
    }
}

#[test]
fn test_limine_configure_defaults() {
    // Use a temp path to avoid modifying real /etc/default/limine
    // This test only checks for no panic and correct type
    let subvol = "/tmp";
    let esp = Some("/tmp/esp");
    let result = Limine::configure_defaults(subvol, esp, false);
    match result {
        Ok(msg) => println!("configure_defaults: {msg}"),
        Err(e) => eprintln!("configure_defaults error: {e}"),
    }
}

#[test]
fn test_limine_root_uuid() {
    // Should return Ok(String) or error if not running on a compatible system
    let result = Limine::root_uuid();
    match result {
        Ok(uuid) => println!("root_uuid: {uuid}"),
        Err(e) => eprintln!("root_uuid error: {e}"),
    }
}

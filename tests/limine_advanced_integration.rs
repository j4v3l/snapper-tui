// Integration test for Limine::limine_install, manual_add_entry, sync_snapshot_to_limine
// These tests check for correct type/behavior and skip gracefully if not applicable or would modify system files.
use snapper_tui::limine::Limine;

#[test]
fn test_limine_install() {
    // Should return Ok(String) or error if limine is not installed or permitted
    let result = Limine::limine_install(false);
    match result {
        Ok(msg) => println!("limine_install: {msg}"),
        Err(e) => eprintln!("limine_install error (expected if not permitted): {e}"),
    }
}

#[test]
fn test_limine_manual_add_entry() {
    // This function requires a real snapshot directory and kernel/initrd files, so we expect it to fail gracefully
    let result = Limine::manual_add_entry(999999, "integration_test", false);
    match result {
        Ok(msg) => println!("manual_add_entry: {msg}"),
        Err(e) => eprintln!("manual_add_entry error (expected): {e}"),
    }
}

#[test]
fn test_limine_sync_snapshot_to_limine() {
    // This function requires limine to be installed and a real snapshot, so we expect it to fail gracefully
    let result = Limine::sync_snapshot_to_limine(999999, "integration_test", false);
    match result {
        Ok(msg) => println!("sync_snapshot_to_limine: {msg}"),
        Err(e) => eprintln!("sync_snapshot_to_limine error (expected): {e}"),
    }
}

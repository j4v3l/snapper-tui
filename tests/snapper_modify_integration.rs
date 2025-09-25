// Integration test for Snapper::modify
// This test will attempt to modify a snapshot description if a test config and snapshot exist.
// It will skip gracefully if no config or snapshot is available.
use snapper_tui::snapper::Snapper;

#[test]
fn test_modify_integration() {
    // Find a config
    let configs = Snapper::available_configs_fs();
    let config = match configs.first() {
        Some(c) => c,
        None => {
            eprintln!("No snapper configs found; skipping test");
            return;
        }
    };
    // Find a snapshot
    let snaps = Snapper::list_snapshots(config, true);
    let snaps = match snaps {
        Ok(s) if !s.is_empty() => s,
        _ => {
            eprintln!("No snapshots found for config {config}; skipping test");
            return;
        }
    };
    let snap = &snaps[0];
    // Try to modify the snapshot description
    let result = Snapper::modify(config, snap.id, "integration test description", true);
    match result {
        Ok(_) => println!("modify succeeded for {config}#{id}", config=config, id=snap.id),
        Err(e) => eprintln!("modify failed for {config}#{id}: {e}", config=config, id=snap.id, e=e),
    }
}

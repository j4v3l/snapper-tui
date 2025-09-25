// Integration test for Snapper::list_snapshots
use snapper_tui::snapper::Snapper;

#[test]
fn test_list_snapshots_integration() {
    // Try to get a config name from the system
    let configs = Snapper::available_configs_fs();
    if configs.is_empty() {
        eprintln!("No snapper configs found; skipping test");
        return;
    }
    let config = &configs[0];
    let result = Snapper::list_snapshots(config, false);
    match result {
        Ok(snaps) => {
            // Should not panic, and should return a Vec (possibly empty)
            assert!(snaps.iter().all(|s| s.config == *config));
        }
        Err(e) => {
            // Acceptable if snapper is not installed or permission denied
            let msg = e.to_string();
            assert!(
                msg.contains("Failed to run snapper")
                    || msg.contains("not found")
                    || msg.contains("permission")
                    || msg.contains("dbus")
                    || msg.contains("Unknown config")
            );
        }
    }
}

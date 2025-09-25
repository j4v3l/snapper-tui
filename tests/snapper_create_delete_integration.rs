// Integration test for Snapper::config_exists, create, and delete
use snapper_tui::snapper::Snapper;

#[test]
fn test_config_exists_and_create_delete() {
    // This test will only run meaningfully if /etc/snapper/configs exists and is writable
    let configs = Snapper::available_configs_fs();
    if configs.is_empty() {
        eprintln!("No snapper configs found; skipping test");
        return;
    }
    let config = &configs[0];
    // config_exists should be true for a real config
    assert!(Snapper::config_exists(config));
    // Try to create and delete a snapshot (may require sudo and may fail if not permitted)
    let create_result = Snapper::create(config, "test snapshot", false);
    match create_result {
        Ok(_) => {
            // Try to get the latest snapshot and delete it
            let snaps = Snapper::list_snapshots(config, false).unwrap_or_default();
            if let Some(last) = snaps.last() {
                let del_result = Snapper::delete(config, last.id, false);
                // Accept Ok or permission errors
                if let Err(e) = del_result {
                    let msg = e.to_string();
                    assert!(
                        msg.contains("permission")
                            || msg.contains("not allowed")
                            || msg.contains("Failed to run snapper")
                    );
                }
            }
        }
        Err(e) => {
            // Acceptable if not permitted
            let msg = e.to_string();
            assert!(
                msg.contains("permission")
                    || msg.contains("not allowed")
                    || msg.contains("Failed to run snapper")
            );
        }
    }
}

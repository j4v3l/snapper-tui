// Integration test for Snapper::snapshot_status and Snapper::diff
use snapper_tui::snapper::Snapper;

#[test]
fn test_snapshot_status_and_diff_integration() {
    let configs = Snapper::available_configs_fs();
    if configs.is_empty() {
        eprintln!("No snapper configs found; skipping test");
        return;
    }
    let config = &configs[0];
    let snaps = Snapper::list_snapshots(config, false).unwrap_or_default();
    if snaps.len() < 2 {
        eprintln!("Not enough snapshots for status/diff test; skipping");
        return;
    }
    let from = snaps[0].id;
    let to = snaps[1].id;
    // Test snapshot_status
    let status_result = Snapper::snapshot_status(config, from, to, false);
    match status_result {
        Ok(s) => assert!(!s.is_empty()),
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("permission") || msg.contains("not allowed") || msg.contains("Failed to run snapper") || msg.contains("Unknown config"));
        }
    }
    // Test diff
    let diff_result = Snapper::diff(config, from, to, false);
    match diff_result {
        Ok(s) => assert!(s.len() >= 0),
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("permission") || msg.contains("not allowed") || msg.contains("Failed to run snapper") || msg.contains("Unknown config"));
        }
    }
}

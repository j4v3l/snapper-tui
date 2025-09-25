// Integration test for Snapper::list_configs
use snapper_tui::snapper::Snapper;

#[test]
fn test_list_configs_integration() {
    // This test will only run meaningfully if /etc/snapper/configs exists
    let configs = Snapper::list_configs();
    match configs {
        Ok(cfgs) => {
            // Should not panic, and should return a Vec (possibly empty)
            assert!(cfgs.iter().all(|c| !c.name.is_empty()));
        }
        Err(e) => {
            // Acceptable if snapper is not installed or configs missing
            let msg = e.to_string();
            assert!(
                msg.contains("Failed to run snapper")
                    || msg.contains("No such file")
                    || msg.contains("not found")
                    || msg.contains("permission")
                    || msg.contains("dbus")
            );
        }
    }
}

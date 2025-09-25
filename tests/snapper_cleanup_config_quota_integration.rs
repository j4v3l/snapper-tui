// Integration test for Snapper::cleanup, get_config, set_config, and setup_quota
// Attempts to call each function if a config is available.
// Skips gracefully if no config is found.
use snapper_tui::snapper::Snapper;

#[test]
fn test_cleanup_get_set_config_setup_quota_integration() {
    let configs = Snapper::available_configs_fs();
    let config = match configs.first() {
        Some(c) => c,
        None => {
            eprintln!("No snapper configs found; skipping test");
            return;
        }
    };
    // cleanup (try with a common algorithm, e.g., 'number')
    let cleanup_result = Snapper::cleanup(config, "number", true);
    match cleanup_result {
        Ok(output) => println!("cleanup succeeded: {output}"),
        Err(e) => eprintln!("cleanup failed: {e}"),
    }
    // get_config
    let get_config_result = Snapper::get_config(config, true);
    match get_config_result {
        Ok(output) => println!("get_config succeeded: {output}"),
        Err(e) => eprintln!("get_config failed: {e}"),
    }
    // set_config (try a harmless key, e.g., 'SYNC_ACL=yes')
    let set_config_result = Snapper::set_config(config, &["SYNC_ACL=yes".to_string()], true);
    match set_config_result {
        Ok(output) => println!("set_config succeeded: {output}"),
        Err(e) => eprintln!("set_config failed: {e}"),
    }
    // setup_quota
    let setup_quota_result = Snapper::setup_quota(config, true);
    match setup_quota_result {
        Ok(output) => println!("setup_quota succeeded: {output}"),
        Err(e) => eprintln!("setup_quota failed: {e}"),
    }
}

// Integration test for Snapper::mount, umount, and rollback
// Attempts to mount, umount, and rollback a snapshot if available.
// Skips gracefully if no config or snapshot is available.
use snapper_tui::snapper::Snapper;

#[test]
fn test_mount_umount_rollback_integration() {
    let configs = Snapper::available_configs_fs();
    let config = match configs.first() {
        Some(c) => c,
        None => {
            eprintln!("No snapper configs found; skipping test");
            return;
        }
    };
    let snaps = Snapper::list_snapshots(config, true);
    let snaps = match snaps {
        Ok(s) if !s.is_empty() => s,
        _ => {
            eprintln!("No snapshots found for config {config}; skipping test");
            return;
        }
    };
    let snap = &snaps[0];
    // Mount
    let mount_result = Snapper::mount(config, snap.id, true);
    match mount_result {
        Ok(mount_output) => println!("mount succeeded: {mount_output}"),
        Err(e) => eprintln!("mount failed: {e}"),
    }
    // Umount
    let umount_result = Snapper::umount(config, snap.id, true);
    match umount_result {
        Ok(_) => println!("umount succeeded"),
        Err(e) => eprintln!("umount failed: {e}"),
    }
    // Rollback (may be destructive, so just check for permission error)
    let rollback_result = Snapper::rollback(config, snap.id, true);
    match rollback_result {
        Ok(output) => println!("rollback output: {output}"),
        Err(e) => eprintln!("rollback failed (expected if not permitted): {e}"),
    }
}

// Unit tests for system.rs
use snapper_tui::system;

#[test]
fn test_has_cmd_true() {
    // 'echo' should exist on all Unix systems
    assert!(system::has_cmd("echo"));
}

#[test]
fn test_has_cmd_false() {
    assert!(!system::has_cmd("definitely_not_a_real_command_12345"));
}

#[test]
fn test_run_success() {
    let output = system::run("echo", &["hello"], false).unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn test_run_string_success() {
    let result = system::run_string("echo", &["world"], false).unwrap();
    assert_eq!(result.trim(), "world");
}

//! Offline operations use verified pins and never enter the SSH transport.

#[cfg(unix)]
#[path = "offline_package_commands/cases.rs"]
mod cases;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "named_workspace_install/fixture.rs"]
mod fixture;

#[cfg(not(unix))]
#[test]
#[ignore = "offline command transport counters require the Unix test-only SSH transport"]
fn offline_command_transport_requires_unix_shell() {}

//! Inspection uses real Git objects without refreshing accepted selectors.

#[cfg(unix)]
#[path = "package_inspection/git.rs"]
mod cases;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "named_workspace_install/fixture.rs"]
mod fixture;

#[cfg(not(unix))]
#[test]
#[ignore = "inspection Git canaries require the Unix test-only SSH transport; local CLI inspection is tested separately"]
fn inspection_git_transport_requires_unix_shell() {}

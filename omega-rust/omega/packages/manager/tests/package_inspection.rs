//! Inspection uses real Git objects without refreshing accepted selectors.

#[cfg(unix)]
#[path = "package_inspection/git.rs"]
mod cases;
#[cfg(unix)]
use crate::named_workspace_fixture as fixture;

#[cfg(not(unix))]
#[test]
#[ignore = "inspection Git canaries require the Unix test-only SSH transport; local CLI inspection is tested separately"]
fn inspection_git_transport_requires_unix_shell() {}

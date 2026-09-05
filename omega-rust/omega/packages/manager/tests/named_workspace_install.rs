//! Named command selection reaches ordinary Git workspace discovery and review.

#[cfg(unix)]
#[path = "named_workspace_install/cases.rs"]
mod cases;
#[cfg(unix)]
#[path = "named_workspace_install/fixture.rs"]
mod fixture;

#[cfg(not(unix))]
#[test]
#[ignore = "named Git command integration uses a Unix test-only SSH transport; portable CLI parsing and declaration tests run separately"]
fn named_git_transport_fixture_requires_unix_shell() {}

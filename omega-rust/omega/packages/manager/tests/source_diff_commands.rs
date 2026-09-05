//! Command source patches use real Git objects through the manager test transport.

#[cfg(unix)]
#[path = "source_diff_commands/git.rs"]
mod cases;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "named_workspace_install/fixture.rs"]
mod fixture;

#[cfg(not(unix))]
#[test]
#[ignore = "Git command source-diff tests require the Unix test-only SSH transport; local command source-diff tests run separately"]
fn source_diff_git_transport_requires_unix_shell() {}

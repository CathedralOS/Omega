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

// Local filesystem sources exercise the same source-diff commands without the
// Git transport, so these cases run on hosts without the test-only SSH
// transport.
mod local {
    use crate::named_workspace_install::local::Fixture;
    use package_manager::{PackageCommand, PackageCommandStatus};
    use package_source::ImmutableSourceResolution;

    #[test]
    fn local_update_writes_a_source_diff_for_changed_sources() {
        let fixture = Fixture::new("source-diff-local-update");
        fixture.package("local", "local-library", "");
        let installed = fixture
            .execute(PackageCommand::Install {
                source: "../local".into(),
                revision: None,
                alias: None,
                package: None,
            })
            .unwrap();
        assert_eq!(
            installed.status,
            PackageCommandStatus::Published,
            "{}",
            installed.report
        );
        fixture.write("local/main.omg", "pub machine value() -> u64 { 99 }\n");
        let updated = fixture
            .execute(PackageCommand::Update {
                packages: vec!["local_library".into()],
                revision: None,
            })
            .unwrap();
        assert_eq!(
            updated.status,
            PackageCommandStatus::Published,
            "{}",
            updated.report
        );
        let source = fixture.read("root/build/package-manager/source-diff.txt");
        assert!(
            source.contains("{ 7 }") && source.contains("{ 99 }"),
            "{source}"
        );
        assert!(
            fixture.lock().targets()[0]
                .source()
                .packages()
                .iter()
                .any(|package| matches!(
                    package.resolution(),
                    ImmutableSourceResolution::ExternalLocal { .. }
                )),
            "expected an external-local lock pin"
        );
    }

    #[test]
    fn local_update_to_a_revision_rejects_non_git_edges() {
        let fixture = Fixture::new("source-diff-local-to");
        fixture.package("local", "local-library", "");
        fixture
            .execute(PackageCommand::Install {
                source: "../local".into(),
                revision: None,
                alias: None,
                package: None,
            })
            .unwrap();
        let before = fixture.pair();
        let error = fixture
            .execute(PackageCommand::Update {
                packages: vec!["local_library".into()],
                revision: Some("0000000000000000000000000000000000000000".into()),
            })
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("local paths have no Git revision"),
            "{error}"
        );
        assert_eq!(fixture.pair(), before);
    }
}

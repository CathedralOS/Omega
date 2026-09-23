//! Offline operations use verified pins and never enter the SSH transport.

#[cfg(unix)]
#[path = "offline_package_commands/cases.rs"]
mod cases;
#[cfg(unix)]
use crate::named_workspace_fixture as fixture;

#[cfg(not(unix))]
#[test]
#[ignore = "offline command transport counters require the Unix test-only SSH transport"]
fn offline_command_transport_requires_unix_shell() {}

// Local filesystem sources exercise the same offline commands without the Git
// transport, so these cases run on hosts without the test-only SSH transport.
mod local {
    use crate::named_workspace_install::local::Fixture;
    use package_manager::{PackageCommand, PackageCommandStatus};

    const REPOSITORY: &str = "git@offline-fixture.invalid:workspace.git";

    #[test]
    fn offline_new_git_install_fails_without_changing_project_files() {
        let fixture = Fixture::new("offline-git-install");
        let before = fixture.pair();
        let error = fixture
            .execute_with_offline(
                PackageCommand::Install {
                    source: REPOSITORY.into(),
                    revision: None,
                    alias: None,
                    package: None,
                },
                true,
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("offline"), "{error}");
        assert_eq!(fixture.pair(), before);
        assert!(!fixture.path("root/build/package-manager/proposal").exists());
    }

    #[test]
    fn offline_local_install_and_update_publish_current_source() {
        let fixture = Fixture::new("offline-local-update");
        fixture.package("local", "local-library", "");
        let installed = fixture
            .execute_with_offline(
                PackageCommand::Install {
                    source: "../local".into(),
                    revision: None,
                    alias: None,
                    package: None,
                },
                true,
            )
            .unwrap();
        assert_eq!(
            installed.status,
            PackageCommandStatus::Published,
            "{}",
            installed.report
        );
        let before = fixture.pair();
        fixture.write("local/main.omg", "pub machine value() -> u64 { 99 }\n");
        let updated = fixture
            .execute_with_offline(
                PackageCommand::Update {
                    packages: vec!["local_library".into()],
                    revision: None,
                },
                true,
            )
            .unwrap();
        assert_eq!(
            updated.status,
            PackageCommandStatus::Published,
            "{}",
            updated.report
        );
        let after = fixture.pair();
        assert_eq!(after.0, before.0);
        assert_ne!(after.1, before.1);
        let source = fixture.read("root/build/package-manager/source-diff.txt");
        assert!(
            source.contains("{ 7 }") && source.contains("{ 99 }"),
            "{source}"
        );
    }
}

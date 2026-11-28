//! Network consent for a dependency retrieved from an immutable Git revision.
use super::{Fixture, assert_send_path, report, sender};
use std::{fs, process::Command};

#[test]
fn a_downloaded_build_names_its_registration_and_waits_for_consent() {
    const LOCATOR: &str = "ssh://git@board-fixture.invalid/board.git";
    let mut fixture = Fixture::new();
    fixture.package("repository", &sender("board"));
    let repository = fixture.packages.path().join("repository");
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&repository)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", report(&output));
        String::from_utf8(output.stdout).unwrap()
    };
    git(&["init", "--quiet"]);
    git(&["add", "build.omg", "main.omg"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@localhost",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "-m",
        "board",
    ]);
    let revision = git(&["rev-parse", "HEAD"]);
    let revision = revision.trim();
    fixture
        .environment
        .insert("GIT_SSH_VARIANT", "simple".into());
    fixture.environment.insert("GIT_SSH_COMMAND", r#"sh -c 'test "$1" = git@board-fixture.invalid && exec git upload-pack "$OMEGA_BOARD_REPOSITORY"' ssh"#.into());
    fixture
        .environment
        .insert("OMEGA_BOARD_REPOSITORY", repository.into_os_string());
    fixture.environment.insert(
        "XDG_CACHE_HOME",
        fixture.packages.path().join("cache").into_os_string(),
    );

    let root = |network| {
        format!(
            r#"machine build(b: @mutable BuildPlan) suspends {{
        let board: DependencyRegistration = Build::dependency(b, Source::Git {{ repository: "{LOCATOR}", revision: "{revision}", selection: PackageSelection::Root }}{network});
        suspend Build::prepare_dependency(b, board);
        b.product = BuildProduct::Package {{ name: "release" }};
    }}"#
        )
    };
    fixture.package("root", &root(""));
    let peer = Some(fixture.peer.local_addr().unwrap());
    let denied = fixture.cli(&["packages", "update"], true, peer);
    let message = report(&denied);
    assert_eq!(denied.status.code(), Some(1), "{message}");
    assert!(
        message.contains(&format!(
            "dependency registration `{LOCATOR} at {revision}`"
        )) && message.contains("requires Network before entry"),
        "{message}"
    );
    assert_send_path(&message, "call path: ");
    assert!(fixture.packets().is_empty());
    assert!(!fixture.root().join("omega.lock").exists());

    fixture.package("root", &root(", Networking::Inherit"));
    let pending = fixture.cli(&["packages", "update"], true, peer);
    assert_eq!(pending.status.code(), Some(3), "{}", report(&pending));
    let review = fs::read_to_string(fixture.build_review()).unwrap();
    assert!(
        review.contains("build-request send-datagram")
            && review.contains("physical-authority HostNetworkStack"),
        "{review}"
    );
    assert!(fixture.packets().is_empty());

    let published = fixture.finish_update(pending, true);
    assert!(published.status.success(), "{}", report(&published));
    let packets = fixture.packets();
    assert!(
        !packets.is_empty() && packets.iter().all(|packet| packet == "board"),
        "{packets:?}"
    );
    assert!(fixture.root().join("omega.lock").is_file());
}

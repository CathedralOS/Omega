use super::*;
use omega_package_manager::declarations::{PackageName, PackageSelection};
use omega_package_manager::resolution::graph::resolve_selected_git_project_closure_with_storage;
use omega_package_manager::resolution::source::GitPackageSourceRequest;
use omega_package_source::GitSourceRequest;

fn git(repository: &Path, arguments: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(repository)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture Git: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn moved_git_root_and_named_member_recover_old_lock_pins_with_fresh_custody() {
    for named in [false, true] {
        let tree = Tree::new();
        let repository = tree.path("repository");
        let selected = if named {
            repository.join("packages/member")
        } else {
            repository.clone()
        };
        package(
            &selected,
            "locked-git",
            if named {
                " builder.depend_as(\"sibling\", Source::Path { location: \"../sibling\" });\n"
            } else {
                ""
            },
        );
        if named {
            package(&repository.join("packages/sibling"), "locked-sibling", "");
            fs::write(
                repository.join("build.omg"),
                concat!(
                    "machine build(builder: &mut Build) {\n",
                    " builder.member(\"packages/member\");\n",
                    " builder.member(\"packages/sibling\");\n",
                    "}\n",
                ),
            )
            .unwrap();
        }
        git(&repository, &["init", "--quiet"]);
        git(
            &repository,
            &["config", "user.email", "omega@example.invalid"],
        );
        git(&repository, &["config", "user.name", "Omega Tests"]);
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "--quiet", "-m", "accepted source"]);
        let acquisition = GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            None,
            "https://github.com/CathedralOS/locked-source-fixture.git",
        )
        .unwrap();
        let selected_request = GitPackageSourceRequest::new(
            acquisition,
            if named {
                PackageSelection::Named(PackageName::parse("locked-git").unwrap())
            } else {
                PackageSelection::Root
            },
        );
        let warm_storage = tree.storage("warm-cache");
        let (lock, request) = {
            let closure = resolve_selected_git_project_closure_with_storage(
                &selected_request,
                &warm_storage,
                LocalSourceLimits::default(),
                PackageSourceClosureLimits::default(),
            )
            .unwrap();
            capture_lock(&closure, &tree.path("build"))
        };
        let text = lock.canonical_text().unwrap();
        fs::write(
            selected.join("main.omg"),
            "pub machine value() -> u64 { 9 }\n",
        )
        .unwrap();
        if named {
            fs::write(
                repository.join("packages/sibling/main.omg"),
                "pub machine value() -> u64 { 11 }\n",
            )
            .unwrap();
            fs::write(
                repository.join("build.omg"),
                "machine build(builder: &mut Build) { builder.member(\"packages/replaced\"); }\n",
            )
            .unwrap();
        }
        git(&repository, &["add", "."]);
        git(
            &repository,
            &["commit", "--quiet", "-m", "move authored selector"],
        );

        let warm = recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &warm_storage,
            LockedSourceRecoveryOptions::default(),
        )
        .unwrap();
        assert_fresh_matches(&lock, &warm);
        let cold_storage = tree.storage("cold-cache");
        let cold = recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &cold_storage,
            LockedSourceRecoveryOptions {
                git_acquisition: GitExactRevisionAcquisition::AllowFetch,
                ..LockedSourceRecoveryOptions::default()
            },
        )
        .unwrap();
        assert_fresh_matches(&lock, &cold);
        let root = cold.custody(cold.graph().root()).unwrap();
        assert_eq!(
            fs::read(root.snapshot_root().join("main.omg")).unwrap(),
            b"pub machine value() -> u64 { 7 }\n"
        );
        if named {
            assert_eq!(
                root.selection_evidence()
                    .git_workspace()
                    .unwrap()
                    .selected_member_path()
                    .as_str(),
                "packages/member"
            );
            assert_eq!(cold.custodies().len(), 2);
            let sibling = cold
                .custodies()
                .iter()
                .find(|custody| custody.key().name().as_str() == "locked-sibling")
                .unwrap();
            assert_eq!(
                sibling
                    .selection_evidence()
                    .git_workspace()
                    .unwrap()
                    .selected_member_path()
                    .as_str(),
                "packages/sibling"
            );
            assert_eq!(
                fs::read(sibling.snapshot_root().join("main.omg")).unwrap(),
                b"pub machine value() -> u64 { 7 }\n"
            );
            assert_eq!(
                root.resolution(),
                sibling.resolution(),
                "both member selections retain the recorded repository root revision"
            );
        }
        fs::rename(&repository, tree.path("offline-repository")).unwrap();
        let offline = recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &cold_storage,
            LockedSourceRecoveryOptions::default(),
        )
        .unwrap();
        assert_fresh_matches(&lock, &offline);
        assert_eq!(lock.canonical_text().unwrap(), text);
    }
}

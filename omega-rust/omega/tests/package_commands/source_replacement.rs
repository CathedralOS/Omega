use super::fixture::{Fixture, assert_status};
use package_manager::declarations::PackageSelection;
use package_manager::lock::HistoricalPackagePolicyDecisionSubject;
use package_manager::resolution::graph::CanonicalDependencySourceRequest;
use package_manager::review::ReviewOnlyRootPolicyDisposition;
use package_source::ImmutableSourceResolution;
use std::fs;

const TARGET: target::TargetProfile = target::TargetProfile::LinuxX64;
const ORIGINAL_REPOSITORY: &str = "git@github.com:CathedralOS/arithmetic-kernels.git";
const ORIGINAL: &str = "b65cc9b062f69ef02a586c82cd260d51bf28945c";
const REPLACEMENT_REPOSITORY: &str = "git@github.com:CathedralOS/library-workbench.git";
const REPLACEMENT: &str = "3b597ba19431e504e9fcd3eb9cb74f7566ed865f";

#[test]
#[ignore = "requires network and private CathedralOS arithmetic-kernels/library-workbench access over SSH"]
fn pinned_ssh_same_name_and_api_do_not_bypass_source_replacement_review() {
    let pins = include_str!("../../../../tests/fixtures/packages/REMOTE_PINS.md");
    assert!(pins.contains(ORIGINAL) && pins.contains(REPLACEMENT));
    let fixture = Fixture::new();
    assert_status(
        &fixture.omega(&[
            "install",
            ORIGINAL_REPOSITORY,
            "--rev",
            ORIGINAL,
            "--target",
            "linux_x86_64",
        ]),
        0,
    );
    check_import(&fixture);
    let original_lock = fixture.lock();
    let original_target = original_lock.target(TARGET).unwrap();
    let original = original_target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "arithmetic-kernels")
        .unwrap();
    assert_eq!(original_target.source().packages().len(), 2);
    let ImmutableSourceResolution::Git { commit, .. } = original.resolution() else {
        panic!("original package must be an exact Git revision");
    };
    assert_eq!(commit.to_hex(), ORIGINAL);

    // Source changes are authored with ordinary existing Build vocabulary.
    // The workspace member declares the same name; its folder is not identity.
    let replacement_build = format!(
        r#"machine build(builder: &mut Build) {{
    builder.package("cli-project");
    builder.depend(Source::Git {{
        repository: "{REPLACEMENT_REPOSITORY}",
        revision: "{REPLACEMENT}",
        selection: PackageSelection::Named {{ package: "arithmetic-kernels" }}
    }});
}}
"#
    );
    fixture.write("root/build.omg", &replacement_build);
    let before = fixture.accepted_files();
    let output = fixture.omega(&["update"]);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let paths = fixture.review_paths(&output);
    let [path] = paths.as_slice() else {
        panic!("one accepted target");
    };
    let document = fs::read_to_string(path).unwrap();
    let replacements = document
        .split("\nsource-replacement\n")
        .skip(1)
        .collect::<Vec<_>>();
    let [replacement] = replacements.as_slice() else {
        panic!("same-name change must retain one explicit source replacement: {document}");
    };
    let replacement = replacement.split("\npackage ").next().unwrap();
    assert!(
        replacement.contains("binding \"arithmetic_kernels\"\n"),
        "{replacement}"
    );
    assert!(
        replacement.contains("- package \"arithmetic-kernels\""),
        "{replacement}"
    );
    assert!(
        replacement.contains("+ package \"arithmetic-kernels\""),
        "{replacement}"
    );
    let decision = replacement
        .lines()
        .find(|line| line.starts_with("decision source-replacement ") && line.ends_with(" pending"))
        .expect("source replacement requires its own choice");
    let accepted = document
        .lines()
        .map(|line| {
            if line.starts_with("decision ") {
                format!("{} accept\n", line.strip_suffix(" pending").unwrap())
            } else {
                format!("{line}\n")
            }
        })
        .collect::<String>();
    let accepted_decision = decision.replace(" pending", " accept");
    for choice in ["pending", "reject"] {
        fs::write(
            path,
            accepted.replace(
                &accepted_decision,
                &decision.replace(" pending", &format!(" {choice}")),
            ),
        )
        .unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 3);
        assert_eq!(fixture.accepted_files(), before);
    }
    // Accepting every API row does not replace acceptance of source identity.
    let other = accepted
        .lines()
        .find(|line| line.starts_with("decision row "))
        .unwrap();
    fs::write(path, accepted.replace(&accepted_decision, other)).unwrap();
    assert_status(&fixture.omega(&["update", "--resume"]), 1);
    assert_eq!(fixture.accepted_files(), before);
    fs::write(path, &accepted).unwrap();
    assert_status(&fixture.omega(&["update", "--resume"]), 0);
    assert_eq!(fixture.read("root/build.omg"), replacement_build);
    assert_ne!(fixture.accepted_files().1, before.1);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());

    let lock = fixture.lock();
    assert_eq!(lock.targets().len(), 1);
    let target = lock.target(TARGET).unwrap();
    assert_eq!(target.source().packages().len(), 2);
    let replacement = target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "arithmetic-kernels")
        .unwrap();
    assert_eq!(replacement.key().name(), original.key().name());
    assert_ne!(replacement.key(), original.key());
    assert_ne!(replacement.key().identity(), original.key().identity());
    assert_ne!(
        replacement.key().source_lineage(),
        original.key().source_lineage()
    );
    assert!(
        !target
            .source()
            .packages()
            .iter()
            .any(|package| package.key() == original.key())
    );
    let ImmutableSourceResolution::Git { commit, .. } = replacement.resolution() else {
        panic!("selected member must retain its repository pin");
    };
    assert_eq!(commit.to_hex(), REPLACEMENT);
    let [edge] = target.source().dependency_requests() else {
        panic!("one root dependency");
    };
    assert_eq!(edge.requester(), target.source().root().selected().key());
    assert_eq!(edge.selected().key(), replacement.key());
    assert_eq!(edge.selected().resolution(), replacement.resolution());
    assert_eq!(edge.alias().as_str(), "arithmetic_kernels");
    let CanonicalDependencySourceRequest::Git {
        repository,
        revision,
        selection,
        explicit_alias,
    } = edge.request()
    else {
        panic!("replacement must use the exact SSH request");
    };
    assert_eq!(repository, REPLACEMENT_REPOSITORY);
    assert_eq!(revision, REPLACEMENT);
    assert!(explicit_alias.is_none());
    assert!(
        matches!(selection, PackageSelection::Named(name) if name.as_str() == "arithmetic-kernels")
    );

    let previous_policy = original_target
        .baselines()
        .iter()
        .find(|policy| policy.package() == original.key().identity())
        .unwrap();
    let policy = target
        .baselines()
        .iter()
        .find(|policy| policy.package() == replacement.key().identity())
        .unwrap();
    assert!(previous_policy.dangerous_capabilities().is_empty());
    assert!(policy.dangerous_capabilities().is_empty());
    let previous_callable = previous_policy
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("add_u64"))
        .unwrap();
    let callable = policy
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("add_u64"))
        .unwrap();
    assert_eq!(previous_callable.parameters(), callable.parameters());
    assert_eq!(previous_callable.return_type(), callable.return_type());
    assert_eq!(
        previous_callable.declared_service_reach(),
        callable.declared_service_reach()
    );
    assert!(
        callable
            .checked_service_reach()
            .realized()
            .unwrap()
            .is_empty()
    );
    assert_ne!(
        previous_callable.identity().owner(),
        callable.identity().owner()
    );
    let recorded = target
        .decisions()
        .decisions()
        .iter()
        .filter_map(|decision| {
            let HistoricalPackagePolicyDecisionSubject::SourceReplacement(digest) =
                decision.subject()
            else {
                return None;
            };
            assert_eq!(
                decision.disposition(),
                ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
            );
            Some(format!(
                "decision source-replacement {} pending",
                digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(recorded, [decision]);
    check_import(&fixture);
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages", "--target", "linux_x86_64"]);
    assert_status(&output, 0);
    assert_eq!(fixture.accepted_files(), before);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(REPLACEMENT), "{stdout}");
    assert!(
        !stdout.contains(ORIGINAL),
        "retired source remains selected: {stdout}"
    );
}

fn check_import(fixture: &Fixture) {
    fixture.write(
        "root/main.omg",
        "use arithmetic_kernels::main;\nmachine main() -> u64 { add_u64(2, 3) }\n",
    );
    let before = fixture.accepted_files();
    assert_status(
        &fixture.omega(&["--check", "--target", "linux_x86_64", "main.omg"]),
        0,
    );
    assert_eq!(fixture.accepted_files(), before);
}

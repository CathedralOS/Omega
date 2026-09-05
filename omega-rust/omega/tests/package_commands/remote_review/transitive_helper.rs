use super::{
    Fixture, TARGET, assert_dependency_path, assert_locked_graph, assert_no_proposal,
    assert_status, audit, check_import, fresh_package, install, package_section,
};
use std::fs;

const GRAPH_BEFORE: &str = "3b1b6679f8adad85bb09a65d67ddd0a23f8180d5";
const GRAPH_AFTER: &str = "7484bc3e56ab153a2e5459dd51054f90b4f22215";
const LEAF_BEFORE: &str = "a89f7a50e4b7c0db8f09bf9c9e73fa54a576a4e2";
const LEAF_AFTER: &str = "db5081e2b294d177a749fc8f5a5edac8cf83c357";
const ENTRY: &str = "machine main() -> u64 { run_graph_probe() }";

#[test]
#[ignore = "requires network and private CathedralOS graph-workbench/capability-vault access over SSH"]
fn pinned_ssh_transitive_private_helper_change_requires_review_with_fixed_public_ceiling() {
    let fixture = Fixture::new();
    fixture.write(
        "root/main.omg",
        &format!("use graph_workbench::main;\n{ENTRY}\n"),
    );
    install(&fixture, "graph-workbench", GRAPH_BEFORE, 0);
    assert_locked_graph(
        &fixture,
        &[
            ("cli-project", "graph-workbench", GRAPH_BEFORE),
            ("graph-workbench", "capability-vault", LEAF_BEFORE),
        ],
    );
    let original = fixture.lock();
    let old_target = original.target(TARGET).unwrap();
    let leaf = old_target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "capability-vault")
        .unwrap();
    let old_policy = old_target
        .baselines()
        .iter()
        .find(|policy| policy.package() == leaf.key().identity())
        .unwrap();
    let public = old_policy
        .callables()
        .callables()
        .iter()
        .filter(|callable| format!("{:?}", callable.role()) == "Public")
        .collect::<Vec<_>>();
    let [old_callable] = public.as_slice() else {
        panic!("one public leaf callable");
    };
    assert!(
        old_callable
            .identity()
            .path()
            .contains("Vault::open_and_keep")
    );
    let [service] = old_callable.declared_service_reach().unwrap() else {
        panic!("one fixed declared service ceiling");
    };
    assert_eq!(service.path(), "SecretHost");
    let declaration = old_policy
        .public_traits()
        .iter()
        .find(|declaration| declaration.identity().path() == "SecretHost")
        .unwrap();
    assert_eq!(service, declaration.identity());
    assert_eq!(
        old_callable.checked_service_reach().realized().unwrap(),
        std::slice::from_ref(service)
    );
    assert!(
        old_callable.capability_flows().is_empty(),
        "acquisition belongs to the private helper"
    );
    for kind in ["uses", "stores", "acquires"] {
        assert!(
            old_callable
                .reachable_capability_flows()
                .iter()
                .any(|flow| { flow.capability() == service && flow.kind().as_str() == kind }),
            "missing helper {kind}"
        );
    }
    assert!(
        old_policy.dangerous_capabilities().is_empty(),
        "nominal SecretHost is not a concrete secret mechanism"
    );
    let report = audit(&fixture, 0);
    let section = fresh_package(&report, "capability-vault");
    assert_dependency_path(section, &["graph_workbench", "capability_vault"]);
    assert!(
        section.contains("reachable-capability-flow Acquires"),
        "{section}"
    );
    check_import(&fixture, "graph-workbench", ENTRY);

    let before = fixture.accepted_files();
    let output = fixture.omega(&["update", "graph_workbench", "--to", GRAPH_AFTER]);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let paths = fixture.review_paths(&output);
    let [path] = paths.as_slice() else {
        panic!("one reviewed target");
    };
    let document = fs::read_to_string(path).unwrap();
    let section = package_section(&document, "capability-vault");
    assert!(section.contains("source-changed true\n"), "{section}");
    let rows = section.split("\nchange ").skip(1).collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        panic!("only the leaf callable's checked policy changes: {section}");
    };
    assert!(row.starts_with("callable changed\n"), "{row}");
    assert!(row.contains("Vault::open_and_keep"), "{row}");
    let decision = row
        .lines()
        .find(|line| line.starts_with("decision row ") && line.ends_with(" pending"))
        .unwrap();
    let paths = section
        .lines()
        .filter(|line| line.starts_with("- path ") || line.starts_with("+ path "))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2);
    assert_eq!(
        &paths[0][2..],
        &paths[1][2..],
        "before/after dependency identities must stay fixed"
    );
    assert!(paths[0].contains(" -> \"graph_workbench\" "));
    assert!(paths[0].contains(" -> \"capability_vault\" "));
    assert_eq!(
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count(),
        1
    );
    for choice in ["pending", "reject"] {
        fs::write(
            path,
            document.replace(
                decision,
                &decision.replace(" pending", &format!(" {choice}")),
            ),
        )
        .unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 3);
        assert_eq!(fixture.accepted_files(), before);
    }
    fs::write(
        path,
        document.replace(decision, &decision.replace(" pending", " accept")),
    )
    .unwrap();
    assert_status(&fixture.omega(&["update", "--resume"]), 0);
    assert_ne!(fixture.accepted_files().0, before.0);
    assert_ne!(fixture.accepted_files().1, before.1);
    assert_no_proposal(&fixture);
    assert_locked_graph(
        &fixture,
        &[
            ("cli-project", "graph-workbench", GRAPH_AFTER),
            ("graph-workbench", "capability-vault", LEAF_AFTER),
        ],
    );
    let updated = fixture.lock();
    let target = updated.target(TARGET).unwrap();
    let policy = target
        .baselines()
        .iter()
        .find(|policy| policy.package() == leaf.key().identity())
        .unwrap();
    let callable = policy
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity() == old_callable.identity())
        .unwrap();
    assert_eq!(old_policy.public_api(), policy.public_api());
    assert_eq!(old_callable.parameters(), callable.parameters());
    assert_eq!(old_callable.return_type(), callable.return_type());
    assert_eq!(
        old_callable.declared_service_reach(),
        callable.declared_service_reach()
    );
    assert_eq!(
        old_callable.declared_synchronous_invocations(),
        callable.declared_synchronous_invocations()
    );
    assert!(
        callable
            .checked_service_reach()
            .realized()
            .unwrap()
            .is_empty()
    );
    assert!(callable.reachable_capability_flows().is_empty());
    assert_ne!(old_policy, policy);
    for previous in old_target
        .baselines()
        .iter()
        .filter(|policy| policy.package() != leaf.key().identity())
    {
        assert_eq!(
            target
                .baselines()
                .iter()
                .find(|policy| policy.package() == previous.package()),
            Some(previous),
            "ancestors must not inherit the leaf's own public API policy"
        );
    }
    let patch = fixture.read("root/build/package-manager/source-diff.txt");
    let leaf_patch = patch
        .split("OMEGA_PACKAGE_SOURCE_PATCH_V1\n")
        .find(|section| section.starts_with("mode update\npackage capability-vault\n"))
        .unwrap();
    assert!(
        leaf_patch.contains(&format!("baseline_git_commit {LEAF_BEFORE}\n")),
        "{leaf_patch}"
    );
    assert!(
        leaf_patch.contains(&format!("candidate_git_commit {LEAF_AFTER}\n")),
        "{leaf_patch}"
    );
    assert!(leaf_patch.contains("changed_entries 1\n"), "{leaf_patch}");
    assert!(
        leaf_patch.contains("removed lf     _ = self.open_secret();\n"),
        "{leaf_patch}"
    );
    let report = audit(&fixture, 0);
    let section = fresh_package(&report, "capability-vault");
    assert_dependency_path(section, &["graph_workbench", "capability_vault"]);
    assert!(!section.contains("reachable-capability-flow"), "{section}");
    check_import(&fixture, "graph-workbench", ENTRY);
}

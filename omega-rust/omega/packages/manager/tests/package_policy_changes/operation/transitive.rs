use super::*;
use package_evidence::record::{PackagePolicyCallableRole, PackageReviewNominalOwner};

pub(super) fn source_chain(tree: &Tree, leaf_name: &str, leaf_source: &str) {
    source(
        tree,
        PURE,
        " builder.depend_as(\"middle_api\", Source::Path { location: \"../middle\" });\n",
    );
    package(
        &tree.path("sources/middle"),
        "middle",
        " builder.depend_as(\"leaf_api\", Source::Path { location: \"../leaf\" });\n",
    );
    package(&tree.path("sources/leaf"), leaf_name, "");
    fs::write(tree.path("sources/leaf/main.omg"), leaf_source).unwrap();
}

const LEAF: &str = r#"pub boundary trait Folder { machine touch() reaches Folder; }
pub boundary trait RootDir { machine open() -> Folder reaches RootDir; }
pub data Vault { root: RootDir; }
machine Vault::open_folder(&self) -> Folder { self.root.open() }
machine Vault::keep(&self) { _ = self.open_folder(); }
pub machine Vault::work(&self)
reaches RootDir
invokes RootDir;
{ self.keep(); }
"#;

#[test]
fn transitive_helper_authority_changes_policy_with_the_same_public_ceiling() {
    let tree = Tree::new();
    source_chain(&tree, "authority-leaf", LEAF);
    let initial = review(&tree, "initial-authority", None);
    let closure = initial.source_closure();
    assert_eq!(closure.graph().packages().len(), 3);
    assert_eq!(initial.reviews().reviews().len(), 3);
    let root = closure.graph().root();
    let middle = closure
        .custodies()
        .iter()
        .find(|custody| custody.key().name().as_str() == "middle")
        .unwrap()
        .key();
    let leaf = closure
        .custodies()
        .iter()
        .find(|custody| custody.key().name().as_str() == "authority-leaf")
        .unwrap()
        .key();
    let leaf_changes = initial
        .changes()
        .packages()
        .iter()
        .find(|package| package.key() == leaf)
        .unwrap();
    let path = leaf_changes.candidate_path().unwrap();
    assert_eq!(path.root(), root.identity());
    assert_eq!(
        path.steps()
            .iter()
            .map(|step| (
                step.requester(),
                step.dependency_index(),
                step.alias(),
                step.target()
            ))
            .collect::<Vec<_>>(),
        [
            (root.identity(), 0, "middle_api", middle.identity()),
            (middle.identity(), 0, "leaf_api", leaf.identity()),
        ]
    );
    let policy = initial.reviews().review(leaf).unwrap().policy();
    assert_eq!(policy.package(), leaf.identity());
    let public = policy
        .callables()
        .callables()
        .iter()
        .filter(|callable| callable.role() == PackagePolicyCallableRole::Public)
        .collect::<Vec<_>>();
    let [work] = public.as_slice() else {
        panic!("one public callable owned by the leaf: {public:?}");
    };
    assert!(work.identity().path().contains("Vault::work"));
    assert_eq!(
        work.identity().owner(),
        PackageReviewNominalOwner::Package(leaf.identity())
    );
    let [service] = work.declared_service_reach().unwrap() else {
        panic!("the public ceiling names exactly RootDir");
    };
    assert_eq!(service.path(), "RootDir");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(leaf.identity())
    );
    assert!(
        work.checked_service_reach()
            .realized()
            .unwrap()
            .contains(service)
    );
    assert!(
        work.capability_flows().is_empty(),
        "the helper keeps acquired authority"
    );
    for kind in ["uses", "stores", "acquires"] {
        assert!(
            work.reachable_capability_flows()
                .iter()
                .any(|flow| { flow.capability() == service && flow.kind().as_str() == kind }),
            "the leaf's private helper must retain {kind}"
        );
    }
    assert!(work.reachable_capability_flows().iter().all(|flow| {
        flow.capability().owner() == PackageReviewNominalOwner::Package(leaf.identity())
    }));

    let accepted = propose(&initial);
    assert_round_trip(&initial, accepted.clone());
    // The call and public promises stay fixed; the helper now leaves its own
    // authority-bearing callee unreachable.
    fs::write(
        tree.path("sources/leaf/main.omg"),
        LEAF.replace("{ _ = self.open_folder(); }", "{}"),
    )
    .unwrap();
    let updated = review(&tree, "removed-authority", Some(&accepted));
    let updated_policy = updated.reviews().review(leaf).unwrap().policy();
    let updated_work = updated_policy
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity() == work.identity())
        .unwrap();
    assert_eq!(policy.public_api(), updated_policy.public_api());
    assert_eq!(work.parameters(), updated_work.parameters());
    assert_eq!(work.return_type(), updated_work.return_type());
    assert_eq!(
        work.declared_service_reach(),
        updated_work.declared_service_reach()
    );
    assert_eq!(
        work.declared_synchronous_invocations(),
        updated_work.declared_synchronous_invocations()
    );
    assert!(updated_work.reachable_capability_flows().is_empty());
    assert!(
        updated_work
            .checked_service_reach()
            .realized()
            .unwrap()
            .is_empty()
    );
    assert_ne!(policy, updated_policy);
    let changes = updated
        .changes()
        .packages()
        .iter()
        .find(|package| package.key() == leaf)
        .unwrap();
    assert_eq!(changes.baseline_path(), Some(path));
    assert_eq!(changes.candidate_path(), Some(path));
    assert!(changes.source_changed());
    let [row] = changes.rows() else {
        panic!(
            "only the public callable's checked body policy changes: {:?}",
            changes.rows()
        );
    };
    assert_eq!(row.kind(), PackagePolicyRowKind::Callable);
    assert_eq!(row.change(), PackagePolicyChangeKind::Changed);
    assert!(row.requires_decision());
    let baseline_row = row.baseline().unwrap();
    let candidate_row = row.candidate().unwrap();
    assert_eq!(baseline_row.key_bytes(), candidate_row.key_bytes());
    assert_ne!(
        baseline_row.canonical_bytes(),
        candidate_row.canonical_bytes()
    );
    let initial_row = leaf_changes
        .rows()
        .iter()
        .find(|initial_row| initial_row.key_bytes() == row.key_bytes())
        .unwrap();
    assert_eq!(initial_row.candidate(), Some(baseline_row));
    assert!(baseline_row.canonical_text().contains("Vault::work"));
    assert!(candidate_row.canonical_text().contains("Vault::work"));
    for ancestor in [root, middle] {
        assert_eq!(
            initial.reviews().review(ancestor).unwrap().policy(),
            updated.reviews().review(ancestor).unwrap().policy()
        );
        assert!(
            updated
                .changes()
                .packages()
                .iter()
                .find(|package| package.key() == ancestor)
                .unwrap()
                .rows()
                .is_empty()
        );
    }

    let report = render_package_policy_review(updated.changes(), MAXIMUM_DOCUMENT_BYTES).unwrap();
    let hex = |identity: semantic_vocabulary::PackageKeyIdentity| {
        identity
            .digest()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let rendered_path = format!(
        "{} -> \"middle_api\" {} -> \"leaf_api\" {}",
        hex(root.identity()),
        hex(middle.identity()),
        hex(leaf.identity())
    );
    assert!(report.contains(&format!("- path {rendered_path}\n")));
    assert!(report.contains(&format!("+ path {rendered_path}\n")));
    assert!(report.contains("change callable changed\n"));
    for (prefix, row) in [("-", baseline_row), ("+", candidate_row)] {
        for line in row.canonical_text().lines() {
            assert!(report.contains(&format!("{prefix} {line}\n")));
        }
    }
    assert_round_trip(&updated, propose(&updated));
    assert!(!tree.path("sources/root/omega.lock").exists());
}

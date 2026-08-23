use omega_packages::{
    AliasName, CapabilityFlowSummary, DependencyAlias, LocalSourceLimits,
    PackageCapabilityManifest, PackageLock, PackageName, SourceIdentity, audit_package_graph,
    resolve_local_source,
};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect("omega-packages should live under the Omega workspace")
        .to_path_buf()
}

fn fixture_root(package: &str) -> PathBuf {
    workspace_root().join("fixtures/packages").join(package)
}

fn package(name: &str) -> PackageName {
    PackageName::parse(name).unwrap()
}

fn alias(name: &str) -> AliasName {
    AliasName::parse(name).unwrap()
}

fn fixture_manifest(package_name: &str) -> PackageCapabilityManifest {
    let root = fixture_root(package_name);
    let resolved =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("fixture should resolve");
    PackageCapabilityManifest::new(
        package(package_name),
        SourceIdentity {
            kind: "local-path".to_owned(),
            locator: root.display().to_string(),
            resolved: resolved.content_identity,
        },
    )
}

#[test]
fn graph_workbench_fixture_audit_reports_file_journal_path() {
    let arithmetic = fixture_manifest("arithmetic-kernels");
    let mut file_journal = fixture_manifest("file-journal");
    file_journal
        .exported_service_reach
        .push("FilesystemHost".to_owned());
    file_journal.capability_flows.push(CapabilityFlowSummary {
        capability: "FilesystemHost".to_owned(),
        verb: "stores".to_owned(),
        count: 1,
    });

    let mut graph = fixture_manifest("graph-workbench");
    graph.dependency_aliases.push(DependencyAlias {
        alias: alias("arithmetic_kernels"),
        package: package("arithmetic-kernels"),
        source_fingerprint: arithmetic.source.resolved.clone(),
    });
    graph.dependency_aliases.push(DependencyAlias {
        alias: alias("file_journal"),
        package: package("file-journal"),
        source_fingerprint: file_journal.source.resolved.clone(),
    });

    let manifests = vec![graph.clone(), arithmetic, file_journal.clone()];
    let lock = PackageLock::from_manifests(package("graph-workbench"), &manifests)
        .expect("fixture manifests should assemble a closed lock");
    let audit = audit_package_graph(&lock, &manifests).expect("fixture graph should audit");
    let text = audit.to_text();

    assert!(text.contains("FilesystemHost via graph-workbench -> file-journal"));
    assert!(text.contains("dependency aliases: arithmetic_kernels -> arithmetic-kernels"));
    assert!(text.contains("file_journal -> file-journal"));
    assert!(text.contains("capability flows: FilesystemHost stores x1"));
    assert_eq!(
        lock.package(&package("file-journal"))
            .expect("file-journal should be locked")
            .source_identity,
        file_journal.source.resolved
    );
}

use super::*;
use psi_effects::CapabilityFlowKind;

const SOURCE: &str = r#"
pub boundary trait Folder { machine touch() reaches Folder; }
pub boundary trait SubFolder { machine touch() reaches SubFolder; }
pub boundary trait RootDir { machine open() -> Folder reaches RootDir; }
pub boundary trait Workspace { machine narrow(parent: Folder) -> SubFolder reaches Workspace; }
pub data Vault { root: RootDir; }
pub machine Vault::direct(&self) -> Folder
reaches RootDir
invokes RootDir;
{ self.root.open() }
machine Vault::open_folder(&self) -> Folder { self.root.open() }
machine Vault::relay(&self) -> Folder { self.open_folder() }
pub machine Vault::expose(&self) -> Folder
reaches RootDir
invokes RootDir;
{ self.relay() }
pub data Broker { workspace: Workspace; }
machine Broker::narrow(&self, folder: Folder) -> SubFolder { self.workspace.narrow(folder) }
pub machine Broker::delegate(&self, folder: Folder) -> SubFolder
reaches Workspace
invokes Workspace;
{ self.narrow(folder) }
"#;

#[test]
fn transitive_capability_flow_retains_all_five_verbs_without_helper_coordinates() {
    let fixture = Fixture::local(SOURCE);
    let original = project(&fixture);
    let direct = callable(&original, "Vault::direct").capability_flows();
    assert!(
        direct
            .iter()
            .any(|flow| flow.kind() == CapabilityFlowKind::Uses)
    );
    assert!(
        direct
            .iter()
            .any(|flow| flow.kind() == CapabilityFlowKind::Stores)
    );
    let transitive = callable(&original, "Vault::expose").capability_flows();
    let reachable = callable(&original, "Vault::expose").reachable_capability_flows();
    for kind in [CapabilityFlowKind::Uses, CapabilityFlowKind::Stores] {
        assert!(
            !transitive.iter().any(|flow| flow.kind() == kind),
            "caller-local compiler flows retain their existing scope"
        );
        assert!(
            reachable
                .iter()
                .any(|flow| flow.kind() == kind && flow.capability().path() == "RootDir"),
            "reachable private helper retains {kind:?}"
        );
    }
    assert!(transitive.iter().all(|flow| reachable.contains(flow)));
    for kind in [CapabilityFlowKind::Returns, CapabilityFlowKind::Acquires] {
        assert!(
            transitive.iter().any(|flow| flow.kind() == kind),
            "missing propagated {kind:?}"
        );
    }
    assert!(
        callable(&original, "Broker::delegate")
            .capability_flows()
            .iter()
            .any(|flow| flow.kind() == CapabilityFlowKind::Derives)
    );
    let flows = original
        .callables()
        .iter()
        .flat_map(|callable| callable.capability_flows())
        .collect::<Vec<_>>();
    for kind in CapabilityFlowKind::ALL {
        assert!(
            flows.iter().any(|flow| flow.kind() == kind),
            "missing {kind:?}: {original:#?}"
        );
    }
    for flow in &flows {
        assert_eq!(
            flow.capability().owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
    }
    let renamed = project(&Fixture::local(
        &SOURCE
            .replace("open_folder", "obtain")
            .replace("relay", "forward"),
    ));
    assert_eq!(original, renamed);
    assert_eq!(
        original.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );
    let mut missing = fixture.checked.clone();
    assert!(!missing.facts.capabilities.is_empty());
    missing.facts.capabilities = Default::default();
    assert!(project_checked_callable_policy(&missing, fixture.target, package_identity()).is_err());
}

#[test]
fn reachable_flow_keeps_private_authority_without_including_unreachable_helpers() {
    let source = format!(
        r#"{SOURCE}
machine Vault::keep(&self) {{ _ = self.open_folder(); }}
pub machine Vault::work(&self)
reaches RootDir
invokes RootDir;
{{ self.keep(); }}
"#
    );
    let original = project(&Fixture::local(&source));
    let work = callable(&original, "Vault::work");
    assert!(
        work.capability_flows().is_empty(),
        "helper-kept authority is not handed back to the Unit caller"
    );
    for kind in [
        CapabilityFlowKind::Uses,
        CapabilityFlowKind::Stores,
        CapabilityFlowKind::Acquires,
    ] {
        assert!(
            work.reachable_capability_flows()
                .iter()
                .any(|flow| flow.kind() == kind && flow.capability().path() == "RootDir"),
            "reachable helper must retain {kind:?}"
        );
    }
    let renamed = project(&Fixture::local(
        &source
            .replace("open_folder", "obtain")
            .replace("keep", "retain_privately"),
    ));
    assert_eq!(original, renamed);
    let expanded_source = format!(
        r#"{source}
boundary trait Unused {{ machine touch() reaches Unused; }}
machine unreachable_helper() {{ Unused::touch(); }}
"#
    );
    let expanded = Fixture::local(&expanded_source);
    let unused = expanded
        .checked
        .traits()
        .iter()
        .find(|service| service.name.as_str() == "Unused")
        .unwrap()
        .symbol;
    assert!(
        expanded
            .checked
            .facts
            .capabilities
            .flows()
            .any(|flow| flow.capability_symbol == unused),
        "control includes real checked authority in an unreachable helper"
    );
    let expanded = project(&expanded);
    assert!(
        callable(&expanded, "Vault::work")
            .reachable_capability_flows()
            .iter()
            .all(|flow| flow.capability().path() != "Unused")
    );
    assert_eq!(original, expanded);
    assert_eq!(
        original.canonical_bytes().unwrap(),
        expanded.canonical_bytes().unwrap()
    );
}

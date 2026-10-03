//! Pins the review-cache input and snapshot digests to their exact bytes. A
//! persisted compiled-review record is filed under a key built from them, so
//! a changed digest silently strands every record written before it.

use super::{TempTree, identity};
use crate::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind,
    BuildSnapshotRequest, IndependentComponentDescription, PackageCompilationInputs,
    PackageDependencyBinding, PackageGeneratedSourceBundle, PackageSourceBinding,
    PackageSourceConsumptionCommitment, review_cache_inputs_digest, review_cache_snapshot_digest,
};
use build_declarations::DependencyPurpose;
use effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use std::sync::Arc;
use target::TargetProfile;

const PINNED_INPUTS_DIGEST: &str =
    "202fa3ac72f0d627a894cea97749ca36111da63ad23c827cf62469aac0c9fc82";
const PINNED_SNAPSHOT_DIGEST: &str =
    "df96be62af51166178a6fb63f8b4fce58f7d3d10c22713fb8cec07c6a36914aa";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A root application with one product dependency and one build dependency,
/// every target attachment present, and no host-dependent value.
fn attached_inputs(tree: &TempTree) -> PackageCompilationInputs {
    let inputs = PackageCompilationInputs::new(
        identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(identity(1), "root", tree.package("root")),
            PackageSourceBinding::new(identity(2), "leaf", tree.package("leaf")),
            PackageSourceBinding::new(identity(3), "tool", tree.package("tool")),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "leaf", identity(2)),
            PackageDependencyBinding::for_purpose(
                identity(1),
                "tool",
                identity(3),
                DependencyPurpose::Build,
            ),
        ],
    )
    .expect("closed application graph");
    let bundle = |package, purpose, marker: u8| {
        PackageGeneratedSourceBundle::from_checked(
            package,
            purpose,
            TargetProfile::LinuxX64,
            Some(TargetProfile::MacosArm64),
            inputs.dependency_closure_for(package),
            PackageSourceConsumptionCommitment::from_retained_digest([marker; 32]),
            vec![build_output::PackageGeneratedSource::from_retained_parts(
                b"generated/leaf.omg".to_vec(),
                Arc::from(&[marker; 5][..]),
            )],
        )
    };
    let bundles = vec![
        bundle(identity(2), DependencyPurpose::Product, 0x42),
        bundle(identity(3), DependencyPurpose::Build, 0x43),
    ];
    let service_schema = ServiceSchemaDigest::from_digest([0x51; 32]);
    let permitted = TerminalAuthorityDisposition::try_from_canonical_classes(vec![
        TerminalAuthorityClass::FilesystemContentRead,
        TerminalAuthorityClass::ProcessOutput,
    ])
    .expect("canonical authority classes");
    let bindings = vec![
        AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            identity(1),
            "Console",
            ServiceSchemaDigest::from_digest([0x55; 32]),
            ProviderPlanDigest::from_digest([0x56; 32]),
        )
        .expect("console binding"),
        AcceptedSemanticBinding::new_service(
            AcceptedSemanticBindingRole::FilesystemHostService,
            identity(2),
            "FilesystemHost",
            service_schema,
        )
        .expect("service binding")
        .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
            service_schema,
            "filesystem.read",
            permitted,
        )])
        .expect("permission names the bound schema"),
    ];
    let description = IndependentComponentDescription::new(
        identity(2),
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x88; 32]),
        },
        b"component".to_vec(),
    );
    inputs
        .with_accepted_semantic_bindings(bindings)
        .and_then(|inputs| inputs.with_complete_dependency_generated_sources(bundles))
        .and_then(|inputs| inputs.with_independent_component_descriptions(vec![description]))
        .expect("every attachment belongs to the graph")
}

#[test]
fn review_cache_digests_keep_their_exact_bytes() {
    let tree = TempTree::new();
    let inputs = attached_inputs(&tree);
    assert_eq!(
        hex(&review_cache_inputs_digest(&inputs)),
        PINNED_INPUTS_DIGEST
    );

    let snapshot = BuildSnapshotRequest::new([b"out/app".to_vec(), b"out/lib".to_vec()]);
    assert_eq!(
        hex(&review_cache_snapshot_digest(&snapshot)),
        PINNED_SNAPSHOT_DIGEST
    );
}

#[test]
fn semantic_role_discovery_is_a_distinct_review_input_without_authority() {
    let tree = TempTree::new();
    let ordinary = attached_inputs(&tree);
    let discovery = ordinary.clone().with_semantic_role_discovery(true);
    assert_ne!(ordinary, discovery);
    assert_ne!(
        review_cache_inputs_digest(&ordinary),
        review_cache_inputs_digest(&discovery),
        "a proposal-bearing review must not reuse an ordinary compile"
    );
    assert_eq!(
        ordinary.accepted_semantic_bindings().collect::<Vec<_>>(),
        discovery.accepted_semantic_bindings().collect::<Vec<_>>(),
        "requesting proposals cannot create accepted role or permission rows"
    );
    let (source, target) = discovery.clone().into_parts();
    assert_eq!(
        PackageCompilationInputs::from_parts(source, target).unwrap(),
        discovery,
        "target input handoff retains the review phase"
    );
    assert_eq!(discovery.with_semantic_role_discovery(false), ordinary);
}

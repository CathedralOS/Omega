//! Pins one persisted review record's exact bytes and the record key's byte
//! layout. A record written by an earlier build of the same stamp must decode
//! unchanged, so changing the pinned bytes requires an explicit
//! `RECORD_VERSION` or key-domain bump.

use super::record_codec::{decode_record, encode_record};
use super::{CachedCompiledReview, CachedOccurrenceFacts, compiled_review_key};
use crate::declarations::DependencyPurpose;
use crate::review::candidate::semantic_bindings::SemanticBindingReviewCandidate;
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, ProviderPlanDigest, ServiceEntryAuthorityFlow,
    ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim, ServiceSchema, ServiceSchemaDigest,
};
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use language_semantics::{
    CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, DomainPredicateBody,
};
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind, ConsumedSourceUnit,
    ConsumedSourceUnitKind, IndependentComponentDescription, PackageCompilationInputs,
    PackageDependencyBinding, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceBinding, PackageSourceConsumptionCommitment, RestrictedBuildBounds,
    RestrictedBuildGrant, RestrictedBuildGrantRoot, RestrictedBuildOperation,
    RestrictedBuildRequest, review_cache_inputs_digest, review_cache_snapshot_digest,
};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use target::TargetProfile;

/// The encoded payload of `sample_record()`, captured before the review cache
/// split into key, store and record-codec modules. `{vocabulary}` stands for
/// the current Terminal Psi vocabulary marker, which a vocabulary bump changes
/// independently of this record layout.
const PINNED_RECORD_PAYLOAD: &str = concat!(
    "4f4d47435245560104000101010101010101010101010101010101010101010101010101010101010101060000000000",
    "000073616d706c6522222222222222222222222222222222222222222222222222222222222222223333333333333333",
    "333333333333333333333333333333333333333333333333010c000000000000006c696e75785f7838365f3634010b00",
    "0000000000006d61636f735f61726d363407000000000000006d616368696e6501950000000000000003000000040000",
    "00050000000000000001060000000107000000000000000001090000000000000000010b00000000000000010c000000",
    "0000000000010e000000000000000f000000000000001000000000000000110000000000000012000000000000001300",
    "000000000000140000000000000015000000000000001600000000000000170000000000000018000000000000000001",
    "000000000000005000000000000000000100000000000000000100010000000000000002070000000000010900000000",
    "0000000010000000000000000200000000000000010000000000000007000000000000006f75742f6170700101020002",
    "000000000000000001010101010101010101010101010101010101010101010101010101010101010700000000000000",
    "436f6e736f6c655555555555555555555555555555555555555555555555555555555555555555015656565656565656",
    "565656565656565656565656565656565656565656565656000000000000000002020202020202020202020202020202",
    "02020202020202020202020202020202020e0000000000000046696c6573797374656d486f7374bd90bc7226ae872cec",
    "9bfdc85b6b6b795197de5c9f750c7be3fd860c2501534f000100000000000000bd90bc7226ae872cec9bfdc85b6b6b79",
    "5197de5c9f750c7be3fd860c2501534f0f0000000000000066696c6573797374656d2e72656164020000000000000000",
    "0601000000000000000202020202020202020202020202020202020202020202020202020202020202020e0000000000",
    "000046696c6573797374656d486f7374bd90bc7226ae872cec9bfdc85b6b6b795197de5c9f750c7be3fd860c2501534f",
    "000100000000000000bd90bc7226ae872cec9bfdc85b6b6b795197de5c9f750c7be3fd860c2501534f0f000000000000",
    "0066696c6573797374656d2e72656164020000000000000000060e0000000000000046696c6573797374656d486f7374",
    "010202020202020202020202020202020202020202020202020202020202020202010000000000000004000000000000",
    "00726561640e0000000000000046696c6573797374656d486f7374010202020202020202020202020202020202020202",
    "0202020202020202020202020f0000000000000066696c6573797374656d2e7265616401000000000000000100000000",
    "000000040000000000000050617468010000000000000000000000000000000400000000000000506174680800000000",
    "0000007265616461626c6501010100010700000000000000616363657074730001010500000000000000427974657301",
    "00000000000000040000000000000072656164000100000100000000000000130000000000000046696c657379737465",
    "6d486f73742e7265616401000000000000000900000000000000686f73742e7265616400010101000000000000000700",
    "000000000000626f756e646564010000000000000000010000000000000006000000000000006c656e67746801000000",
    "0000000001170000000000000066696c6573797374656d2e726561642e626f756e646564010807060504030201012000",
    "000000000000444444444444444444444444444444444444444444444444444444444444444401010101010101010101",
    "01010101010101010101010101010101010101010101010c000000000000006c696e75785f7838365f3634010b000000",
    "000000006d61636f735f61726d3634010101010101010101010101010101010101010101010101010101010101010100",
    "020000000000000001010101010101010101010101010101010101010101010101010101010101010202020202020202",
    "020202020202020202020202020202020202020202020202010000000000000001010101010101010101010101010101",
    "0101010101010101010101010101010104000000000000006c6561660202020202020202020202020202020202020202",
    "020202020202020202020202006666666666666666666666666666666666666666666666666666666666666666010000",
    "0000000000120000000000000067656e6572617465642f6c6561662e6f6d670c00000000000000666e206c6561662829",
    "207b7d100000000000000063616e6f6e6963616c2072657669657702000000000000000700000000000000726f77206f",
    "6e650700000000000000726f772074776f0600000000000000706f6c69637906000000000000006c6564676572000400",
    "000000000000000101010101010101010101010101010101010101010101010101010101010101010002000000000000",
    "00030000000000000073726308000000000000006d61696e2e6f6d672800000000000000707070707070707070707070",
    "707070707070707070707070707070707070707001010101010101010101010101010101010101010101010101010101",
    "01010101010101000200000000000000090000000000000067656e65726174656408000000000000006c6561662e6f6d",
    "672900000000000000717171717171717171717171717171717171717171717171717171717171717103000103000000",
    "0000000073746401000000000000000600000000000000696f2e6f6d672a000000000000007272727272727272727272",
    "72727272727272727272727272727272727272727202000001000000000000000b000000000000007072656c7564652e",
    "6f6d672b0000000000000073737373737373737373737373737373737373737373737373737373737373736666666666",
    "666666666666666666666666666666666666666666666666666666010202020202020202020202020202020202020202",
    "020202020202020202020202{vocabulary}888888888888888888888888888888888888888888888888888888888888",
    "88880900000000000000636f6d706f6e656e740102020202020202020202020202020202020202020202020202020202",
    "02020202020e0000000000000046696c6573797374656d486f7374bd90bc7226ae872cec9bfdc85b6b6b795197de5c9f",
    "750c7be3fd860c2501534f000100000000000000bd90bc7226ae872cec9bfdc85b6b6b795197de5c9f750c7be3fd860c",
    "2501534f0f0000000000000066696c6573797374656d2e72656164020000000000000000060e0000000000000046696c",
    "6573797374656d486f737401020202020202020202020202020202020202020202020202020202020202020201000000",
    "000000000400000000000000726561640e0000000000000046696c6573797374656d486f737401020202020202020202",
    "02020202020202020202020202020202020202020202020f0000000000000066696c6573797374656d2e726561640100",
    "000000000000010000000000000004000000000000005061746801000000000000000000000000000000040000000000",
    "00005061746808000000000000007265616461626c650101010001070000000000000061636365707473000101050000",
    "000000000042797465730100000000000000040000000000000072656164000100000100000000000000130000000000",
    "000046696c6573797374656d486f73742e7265616401000000000000000900000000000000686f73742e726561640001",
    "0101000000000000000700000000000000626f756e646564010000000000000000010000000000000006000000000000",
    "006c656e677468010000000000000001170000000000000066696c6573797374656d2e726561642e626f756e64656401",
    "080706050403020101200000000000000044444444444444444444444444444444444444444444444444444444444444",
    "4434120000000000007856000000000000",
);

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn carry(suspension: CarrySuspension, address: CarryAddress) -> CarryPolicy {
    CarryPolicy {
        suspension,
        cpu: CarryCpu::Any,
        host_thread: CarryHostThread::Origin,
        address,
    }
}

fn service_schema() -> ServiceSchema {
    ServiceSchema {
        trait_name: "FilesystemHost".to_owned(),
        trait_package_identity: Some(identity(2)),
        methods: vec![ServiceMethod {
            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
            name: "read".to_owned(),
            requirement_owner: "FilesystemHost".to_owned(),
            requirement_owner_package_identity: Some(identity(2)),
            requirement_identity: "filesystem.read".to_owned(),
            parameter_count: 1,
            parameter_type_identities: vec!["Path".to_owned()],
            entry_claims: vec![ServiceEntryClaim {
                parameter_index: 0,
                carrier_identity: "Path".to_owned(),
                domain: "readable".to_owned(),
                predicate_body: DomainPredicateBody::Present,
                effective_carry: carry(CarrySuspension::Allowed, CarryAddress::Movable),
                authority_flow: ServiceEntryAuthorityFlow::Accepts,
            }],
            has_result: true,
            result_type_identity: Some("Bytes".to_owned()),
            result_claims: vec![ServiceResultClaim {
                domain: "read".to_owned(),
                effective_carry: carry(CarrySuspension::Forbidden, CarryAddress::Stable),
            }],
            service_reach: vec!["FilesystemHost.read".to_owned()],
            synchronous_invocations: vec!["host.read".to_owned()],
            may_suspend: false,
            may_block: true,
            terminates_guarantee: true,
            termination_premises: vec![ServiceProgressPremise {
                profile: "bounded".to_owned(),
                subject: ServiceProgressSubject::Parameter(0),
                subject_projections: vec!["length".to_owned()],
                establishment_routes: vec![ServiceProgressEstablishmentRoute {
                    kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                    requirement_identity: "filesystem.read.bounded".to_owned(),
                }],
            }],
            calling_plan_report_fingerprint: Some(0x0102_0304_0506_0708),
            calling_plan_commitment: Some(BoundaryCallingPlanCommitment::from_digest([0x44; 32])),
        }],
    }
}

fn filesystem_candidate() -> SemanticBindingReviewCandidate {
    let role = AcceptedSemanticBindingRole::FilesystemHostService;
    let schema = service_schema();
    let digest = package_compilation::accepted_service_schema_digest(role, &schema);
    let permitted = TerminalAuthorityDisposition::try_from_canonical_classes(vec![
        TerminalAuthorityClass::FilesystemContentRead,
        TerminalAuthorityClass::ProcessOutput,
    ])
    .expect("canonical authority classes");
    let binding = AcceptedSemanticBinding::new_service(role, identity(2), "FilesystemHost", digest)
        .expect("service binding")
        .with_terminal_authority_permissions(vec![ServiceTerminalAuthorityPermission::new(
            digest,
            "filesystem.read",
            permitted,
        )])
        .expect("permission names the bound schema");
    SemanticBindingReviewCandidate::new(binding, schema)
}

fn console_binding() -> AcceptedSemanticBinding {
    AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        identity(1),
        "Console",
        ServiceSchemaDigest::from_digest([0x55; 32]),
        ProviderPlanDigest::from_digest([0x56; 32]),
    )
    .expect("console binding")
}

fn generated_source_bundle() -> PackageGeneratedSourceBundle {
    let closure = PackageDependencyClosure::from_canonical_parts(
        identity(1),
        BuildDeclarationKind::Package,
        vec![identity(1), identity(2)],
        vec![PackageDependencyBinding::new(
            identity(1),
            "leaf",
            identity(2),
        )],
    )
    .expect("closed canonical closure");
    PackageGeneratedSourceBundle::from_checked(
        identity(1),
        DependencyPurpose::Build,
        TargetProfile::LinuxX64,
        Some(TargetProfile::MacosArm64),
        closure,
        PackageSourceConsumptionCommitment::from_retained_digest([0x66; 32]),
        vec![build_output::PackageGeneratedSource::from_retained_parts(
            b"generated/leaf.omg".to_vec(),
            Arc::from(&b"fn leaf() {}"[..]),
        )],
    )
}

fn consumed_units() -> Vec<ConsumedSourceUnit> {
    [
        (
            ConsumedSourceUnitKind::PackageAuthored,
            Some(identity(1)),
            None,
            vec!["src", "main.omg"],
        ),
        (
            ConsumedSourceUnitKind::PackageGenerated(source::DependencyScope::Build),
            Some(identity(1)),
            None,
            vec!["generated", "leaf.omg"],
        ),
        (
            ConsumedSourceUnitKind::ToolchainOwned,
            None,
            Some("std"),
            vec!["io.omg"],
        ),
        (
            ConsumedSourceUnitKind::ToolchainVirtual,
            None,
            None,
            vec!["prelude.omg"],
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (kind, package, namespace, path))| {
        ConsumedSourceUnit::from_retained(
            kind,
            package,
            namespace.map(str::to_owned),
            path.into_iter().map(str::to_owned).collect(),
            40 + index as u64,
            [0x70 + index as u8; 32],
        )
        .expect("canonical consumed unit")
    })
    .collect()
}

fn restricted_build_request() -> RestrictedBuildRequest {
    RestrictedBuildRequest::from_retained(
        RestrictedBuildOperation::ScopedFilesystemExecution,
        vec![RestrictedBuildGrant::from_retained(
            RestrictedBuildGrantRoot::SourceInventory,
            true,
            None,
        )],
        vec![RestrictedBuildGrant::from_retained(
            RestrictedBuildGrantRoot::Other(7),
            false,
            None,
        )],
        RestrictedBuildBounds::from_retained(
            Some(checked_interpreter::FilesystemSponsorLimits {
                maximum_entries: 9,
                maximum_total_logical_bytes: 4096,
                maximum_object_extent: 512,
            }),
            None,
            vec![b"out/app".to_vec()],
            true,
        ),
        Some(TargetProfile::LinuxX64),
        None,
    )
}

fn build_evaluation_usage() -> build_evaluation::BuildEvaluationUsage {
    build_evaluation::BuildEvaluationUsage {
        usage_schema_version: 3,
        step_schedule_marker: 4,
        invocation_fuel_ceiling: 5,
        sponsor_schema_version: Some(6),
        session_fuel_ceiling: Some(7),
        session_build_log_byte_ceiling: None,
        session_filesystem_attempt_ceiling: Some(9),
        session_live_filesystem_handle_ceiling: None,
        session_live_cell_ceiling: Some(11),
        session_live_text_byte_ceiling: Some(12),
        session_result_cell_ceiling: None,
        session_result_text_byte_ceiling: Some(14),
        session_peak_live_filesystem_handles: 15,
        session_peak_live_cells: 16,
        session_peak_live_text_bytes: 17,
        fuel_units: 18,
        build_log_bytes: 19,
        filesystem_operation_attempts: 20,
        peak_live_cells: 21,
        peak_live_text_bytes: 22,
        result_cells: 23,
        result_text_bytes: 24,
    }
}

/// One record exercising every present-value branch the codec writes.
fn sample_record() -> CachedCompiledReview {
    CachedCompiledReview {
        package_identity: identity(1),
        name: "sample".to_owned(),
        lineage: [0x22; 32],
        resolution: [0x33; 32],
        purpose: DependencyPurpose::Build,
        target: TargetProfile::LinuxX64,
        build_execution_profile: Some(TargetProfile::MacosArm64),
        selected_build_machine_identity: "machine".to_owned(),
        build_evaluation_usage: Some(build_evaluation_usage()),
        build_observation_summary: None,
        restricted_build_requests: vec![restricted_build_request()],
        semantic_bindings: vec![console_binding(), filesystem_candidate().binding().clone()],
        semantic_binding_candidates: vec![filesystem_candidate()],
        generated_source_bundle: generated_source_bundle(),
        canonical_review_bytes: b"canonical review".to_vec(),
        canonical_row_bytes: vec![b"row one".to_vec(), b"row two".to_vec()],
        policy_bytes: b"policy".to_vec(),
        obligation_ledger_bytes_payload: b"ledger".to_vec(),
        facts: CachedOccurrenceFacts {
            tolerated_unsettled_service_fields: false,
            consumed_units: consumed_units(),
            source_consumption_commitment: PackageSourceConsumptionCommitment::from_retained_digest(
                [0x66; 32],
            ),
            component_description: Some(IndependentComponentDescription::new(
                identity(2),
                terminal_psi::TerminalPsiIdentity {
                    vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                    program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x88; 32]),
                },
                b"component".to_vec(),
            )),
            target_entry_candidate: Some(filesystem_candidate()),
        },
        policy_canonical_bytes: 0x1234,
        obligation_ledger_bytes: 0x5678,
    }
}

#[test]
fn compiled_review_record_keeps_its_exact_bytes() {
    let mut payload = Vec::new();
    encode_record(&sample_record(), &mut payload);
    let vocabulary = terminal_psi::VocabularyMarker::CURRENT.get().to_le_bytes();
    assert_eq!(
        hex(&payload),
        PINNED_RECORD_PAYLOAD.replace("{vocabulary}", &hex(&vocabulary))
    );

    let mut stored = payload.clone();
    stored.extend_from_slice(&Sha256::digest(&payload));
    let decoded = decode_record(&stored).expect("the pinned record decodes");
    let mut reencoded = Vec::new();
    encode_record(&decoded, &mut reencoded);
    assert_eq!(reencoded, payload);

    // Version 3 lacks hosted nominations in the candidate roster. Even an
    // otherwise intact record with a valid trailer must be rebuilt.
    let mut previous = payload.clone();
    previous[8..10].copy_from_slice(&3u16.to_le_bytes());
    let previous_digest = Sha256::digest(&previous);
    previous.extend_from_slice(&previous_digest);
    assert!(decode_record(&previous).is_none());

    // A flipped payload byte fails the trailer check rather than decoding.
    let mut corrupt = stored;
    corrupt[20] ^= 1;
    assert!(decode_record(&corrupt).is_none());
}

#[test]
fn compiled_review_key_keeps_its_byte_layout() {
    let root = std::env::temp_dir().join(format!(
        "omega-review-cache-key-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create package root");
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "sample",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("single-package inputs");
    let snapshot = build_evaluation::BuildSnapshotRequest::new([b"out/app".to_vec()].into_iter());
    let entry = Path::new("/review/sample/main.omg");
    let key = compiled_review_key(
        identity(1),
        DependencyPurpose::Build,
        TargetProfile::LinuxX64,
        Some(TargetProfile::MacosArm64),
        entry,
        &inputs,
        &snapshot,
    );
    let _ = std::fs::remove_dir_all(&root);

    fn framed(digest: &mut Sha256, bytes: &[u8]) {
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    let mut expected = Sha256::new();
    expected.update(b"OMEGA-COMPILED-REVIEW-CACHE-KEY-V2\0");
    expected.update([1; 32]);
    expected.update([1]);
    framed(&mut expected, b"linux_x86_64");
    expected.update([1]);
    framed(&mut expected, b"macos_arm64");
    framed(&mut expected, b"/review/sample/main.omg");
    framed(&mut expected, super::key::stamp_string().as_bytes());
    expected.update(review_cache_inputs_digest(&inputs));
    expected.update(review_cache_snapshot_digest(&snapshot));
    let expected: [u8; 32] = expected.finalize().into();
    assert_eq!(key, expected);
}

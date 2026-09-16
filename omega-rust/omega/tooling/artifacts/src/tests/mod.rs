//! Artifact construction and projection regression tests.

use std::collections::BTreeSet;

use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStack,
    EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineRegister, MachineState,
    MachineStateSet, Preemption, ProviderExitRealization, RegisterSet, StackDomainRef,
    StateFootprintEvidence, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_entry_stack_realization,
};
use executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactAuthorityCommitments,
    ArtifactEntry, ArtifactId, CodePlacementAuthority, CodePlacementId, ContainerLimits,
    DecodedArtifactContainer, EntrySetId, FinalValidationCertificate, FinalValidationId,
    InstallAuthority, InstallationAudience, InstallationDiagnostic, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    decode_executable_container, install_validated, materialize_admitted_artifact,
    materialize_and_freeze, normalized_decoded_content_digest, validate_final_placement,
};
use extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use external_roots::{
    AcknowledgementPolicyId, ComponentArtifactId, ComponentContractId, ComponentProviderId,
    ComponentVersionPin, ComponentVersionPinId, ExternalRootDiagnostic, ExternalRootId,
    FixedFuelCall, FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity,
    FuelValidationReceiptId, InstalledRootRecord, LogicalFuelResourceColumn,
    MachineStateResourceColumn, NestingRelationId, ObjectEvidence, OpaqueProviderExitAssurance,
    ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId, ProviderPlanId,
    ProviderStackSummary, RootAdmissionId, RootEffectId, RootProviderId, RootSlotId,
    RootSlotOwnerId, StackDemandEvidence, StackNestingRelation, StackResourceColumn,
    StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId, bind_installed_entry_stack,
    bind_opaque_adapter_stack_realization, compose_bound_entry_stack_epochs, compose_fixed_fuel,
};
use layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, PlacementSite,
};
use target::Architecture;

use super::external_root_report::external_root_records_manifest_json;
use super::{ArtifactWriter, value_placement_json};

#[test]
fn value_placement_json_retains_indirect_copy_geometry() {
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("Microsoft x64 aggregate placement");
    let json = value_placement_json(&boundary.plan().call.parameters[0]);

    assert!(json.contains("\"indirect\""));
    assert!(json.contains("\"copy_stack_byte_offset\": 32"));
    assert!(json.contains("\"byte_size\": 16"));
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
}

fn install_id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn extent_provider_issuance(seed: u64) -> extents::ExtentProviderIssuance {
    let base = seed * 16;
    extents::ExtentProviderIssuance::from_normalized_identities([
        base + 1,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 7,
        base + 8,
        base + 9,
        base + 10,
        base + 11,
        base + 12,
        base + 13,
    ])
    .expect("normalized provider issuance")
}

#[derive(Debug)]
struct ExternalRootReportObject {
    psi: terminal_psi::TerminalPsiIdentity,
    machine: semantic_vocabulary::MachineId,
    text: Vec<u8>,
}

impl ObjectEvidence for ExternalRootReportObject {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    fn target(&self) -> target::NativeTarget {
        target::NativeTarget::linux_x64()
    }

    fn text_bytes(&self) -> &[u8] {
        &self.text
    }

    fn function_text_offset(&self, machine: semantic_vocabulary::MachineId) -> Option<usize> {
        (machine == self.machine).then_some(16)
    }
}

#[derive(Debug)]
struct ExternalRootReportStackDemand {
    psi: terminal_psi::TerminalPsiIdentity,
    machine: semantic_vocabulary::MachineId,
    contributing_machines: BTreeSet<semantic_vocabulary::MachineId>,
    admitted_reports: BTreeSet<u64>,
    admitted_commitments: BTreeSet<[u8; 32]>,
}

impl StackDemandEvidence for ExternalRootReportStackDemand {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    fn architecture(&self) -> Architecture {
        Architecture::X86_64
    }

    fn entry(&self) -> semantic_vocabulary::MachineId {
        self.machine
    }

    fn ceiling_bytes(&self) -> u64 {
        2048
    }

    fn stack_alignment(&self) -> u32 {
        16
    }

    fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId> {
        &self.contributing_machines
    }

    fn admitted_stack_contribution_report_identities(&self) -> BTreeSet<u64> {
        self.admitted_reports.clone()
    }

    fn admitted_stack_contribution_commitments(&self) -> BTreeSet<[u8; 32]> {
        self.admitted_commitments.clone()
    }
}

fn installed_code_fixture(entry: EntryStubId) -> InstalledCode {
    let scope =
        ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope");
    let constraints = PlacementConstraints::new(
        Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
        4096,
        PlacementPhase::PostHandoff,
        None,
        Some(scope),
    )
    .expect("placement constraints");
    let contracts = install_id(30, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(31, MachineFootprintId::from_normalized_identity);
    let authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
        contracts,
        b"test imported contract set",
        footprint,
        b"test declared footprint",
        None,
        Some((scope, b"test installation scope")),
    );
    let artifact = Artifact::from_canonical_decode(
        install_id(3, ArtifactId::from_normalized_identity),
        Architecture::X86_64,
        vec![0; 64],
        contracts,
        footprint,
        install_id(32, PlacementPlanId::from_normalized_identity),
        constraints,
        install_id(33, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        install_id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        authority_commitments,
    )
    .expect("artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(40, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        51,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(100),
        extent_id(100, ExtentLineageId::from_normalized_identity),
        extent_id(50, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(52, ExtentProvenanceId::from_normalized_identity),
        extent_id(53, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(100, CodePlacementId::from_normalized_identity),
        install_id(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::FutureFetcher,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("artifact without relocations materializes");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(71, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen placement");
    let certificate = FinalValidationCertificate::from_validator(
        install_id(180, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &certificate).expect("validated placement");
    let install_authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(300, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, install_authority, receipt).expect("installed code")
}

fn entry_id(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
}

fn executable_container_fixture() -> Artifact {
    let artifact_id = install_id(900, ArtifactId::from_normalized_identity);
    let contracts = install_id(901, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(902, MachineFootprintId::from_normalized_identity);
    let placement_plan = install_id(903, PlacementPlanId::from_normalized_identity);
    let entry_set = install_id(904, EntrySetId::from_normalized_identity);
    let relocation_set = install_id(905, RelocationSetId::from_normalized_identity);
    let code = vec![0xc3];
    let entries = vec![ArtifactEntry::from_canonical_decode(entry_id(906), 0)];
    let placement_constraints =
        PlacementConstraints::new(None, 1, PlacementPhase::Load, None, None)
            .expect("placement constraints");
    let authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
        contracts,
        b"test imported contract set",
        footprint,
        b"test declared footprint",
        None,
        None,
    );
    let mut decoded = DecodedArtifactContainer {
        format_marker: executable_installation::OMEGA_EXECUTABLE_CONTAINER_MARKER,
        total_length: 1,
        artifact: artifact_id,
        content_fingerprint:
            executable_installation::NonAuthoritativeContainerFingerprint64::from_compatibility_value(1)
                .unwrap(),
        architecture: Architecture::X86_64,
        code_length: code.len() as u64,
        code: code.clone(),
        contracts,
        declared_footprint: footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries: entries.clone(),
        relocation_set,
        relocations: Vec::new(),
        proof_payload: executable_installation::normalized_proof_payload_digest(b""),
        proof: Vec::new(),
        authority_commitments: Some(authority_commitments),
        sections: Vec::new(),
    };
    decoded.content_fingerprint =
        executable_installation::non_authoritative_decoded_container_fingerprint(&decoded)
            .expect("normalized content fingerprint");
    let content = normalized_decoded_content_digest(&decoded).expect("normalized content digest");
    let artifact = Artifact::from_canonical_decode(
        artifact_id,
        Architecture::X86_64,
        code,
        contracts,
        footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries,
        relocation_set,
        Vec::new(),
        authority_commitments,
    )
    .expect("canonical artifact");
    assert_eq!(artifact.content(), content);
    artifact
}

#[test]
fn writes_canonical_executable_container_atomically() {
    let root = std::env::temp_dir().join(format!(
        "omega-artifact-container-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let limits = ContainerLimits {
        max_total_bytes: 64 * 1024,
        max_sections: 16,
        max_section_bytes: 32 * 1024,
        max_relocations: 64,
    };
    let artifact = executable_container_fixture();

    let path = writer
        .write_executable_container("program.omega-artifact", &artifact, b"proof", limits)
        .expect("canonical artifact output");
    let bytes = std::fs::read(&path).expect("written artifact bytes");
    let decoded =
        decode_executable_container(&bytes, limits).expect("written bytes remain canonical");

    assert_eq!(decoded.artifact(), &artifact);
    assert_eq!(decoded.proof(), b"proof");
    assert!(!root.join(".program.omega-artifact.tmp").exists());
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn external_root_manifest_is_complete_normalized_and_address_free() {
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("boundary plan");
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        root_id(21, ProviderFuelSummaryId::from_normalized_identity),
        root_id(22, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            23,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let work_root = FixedFuelProviderSummary::from_admitted_provider(
        root_id(20, ProviderFuelSummaryId::from_normalized_identity),
        root_id(8, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf.identity,
            maximum_invocations: 2,
        }]),
        root_id(
            24,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let composed_fuel =
        compose_fixed_fuel(work_root.identity, [&work_root, &leaf]).expect("fixed fuel");
    let root_identity = root_id(1, ExternalRootId::from_normalized_identity);
    let nesting_identity = root_id(11, NestingRelationId::from_normalized_identity);
    let entry = entry_id(2);
    let installed_code = installed_code_fixture(entry);
    let terminal_machine = semantic_vocabulary::MachineId::new(1).expect("terminal machine");
    let terminal_psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x71; 32]),
    };
    let stack_binding = bind_installed_entry_stack(
        &ExternalRootReportStackDemand {
            psi: terminal_psi,
            machine: terminal_machine,
            contributing_machines: BTreeSet::from([terminal_machine]),
            admitted_reports: BTreeSet::from([0x81, 0x82]),
            admitted_commitments: BTreeSet::from([[0xa1; 32], [0xa2; 32]]),
        },
        &ExternalRootReportObject {
            psi: terminal_psi,
            machine: terminal_machine,
            text: vec![0; 64],
        },
        &installed_code,
        entry,
    )
    .expect("installed terminal stack evidence");
    let stack_summary = ProviderStackSummary::from_entry(
        root_identity,
        root_id(8, RootProviderId::from_normalized_identity),
        EntryStack::ProviderSelected,
        stack_binding,
    );
    let stack_realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain: StackDomainRef::Interrupted,
                occupancy_by_domain: Vec::new(),
                nesting: Preemption::NotApplicable,
            }],
        }],
    })
    .expect("entry stack realization");
    let arrival_contexts = external_roots::admit_opaque_arrival_context_set(
        &stack_summary,
        &boundary,
        &installed_code,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(30, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("arrival-context admission");
    let bound_stack = bind_opaque_adapter_stack_realization(
        &stack_summary,
        &boundary,
        &installed_code,
        entry,
        stack_realization,
        arrival_contexts,
    )
    .expect("bound stack realization");
    let composed_stack = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: nesting_identity,
            edges: BTreeSet::new(),
        },
        [&bound_stack],
    )
    .expect("stack composition");
    let record = InstalledRootRecord {
        root: root_identity,
        normalized_root_report_identity: 0x101,
        entry,
        installed_code: installed_code.identity(),
        artifact: installed_code.artifact(),
        slot: root_id(5, RootSlotId::from_normalized_identity),
        owner: root_id(6, RootSlotOwnerId::from_normalized_identity),
        admission: root_id(7, RootAdmissionId::from_normalized_identity),
        provider_execution: root_id(30, ProviderExecutionId::from_normalized_identity),
        provider_execution_report_fingerprint: 0x3030,
        provider_exit_assurance: OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: boundary.plan().call.entry_control,
                restored_state: boundary.plan().state.restored_state,
            },
            validation_receipt: root_id(10, TrustReceiptId::from_normalized_identity),
        },
        provider_exit_assurance_report_fingerprint: 0x3031,
        provider_plan: root_id(31, ProviderPlanId::from_normalized_identity),
        requirement_identity: "TestRoot::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: vec!["MachineControl".into()],
        selected_provider_closure_report_fingerprint: 0x3033,
        selected_provider_closure_digest: effects::SelectedProviderPlanFacts::default()
            .identity_digest(),
        installation_reach_resolutions: Vec::new(),
        boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
        boundary: boundary.plan().clone(),
        provider: root_id(8, RootProviderId::from_normalized_identity),
        effects: BTreeSet::from([root_id(9, RootEffectId::from_normalized_identity)]),
        trust_receipts: BTreeSet::from([root_id(10, TrustReceiptId::from_normalized_identity)]),
        nesting_relation: nesting_identity,
        acknowledgement_policy: Some(root_id(
            12,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: composed_stack,
            validation_receipt: root_id(25, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
            provision: root_id(28, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: composed_fuel,
            validation_receipt: root_id(26, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(27, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: BTreeSet::from([ComponentVersionPin {
            contract: root_id(13, ComponentContractId::from_normalized_identity),
            artifact: root_id(14, ComponentArtifactId::from_normalized_identity),
            provider: root_id(15, ComponentProviderId::from_normalized_identity),
            version: root_id(16, ComponentVersionPinId::from_normalized_identity),
        }]),
    };

    let first = external_root_records_manifest_json(0x202, &[&record]);
    let second = external_root_records_manifest_json(0x202, &[&record]);
    let parsed: serde_json::Value = serde_json::from_str(&first).expect("valid JSON manifest");

    assert_eq!(first, second);
    assert_eq!(parsed["root_count"], 1);
    assert_eq!(
        parsed["roots"][0]["normalized_root_report_identity"],
        "0x0000000000000101"
    );
    assert_eq!(parsed["roots"][0]["entry"], "0x0000000000000002");
    assert_eq!(
        parsed["roots"][0]["provider_execution"],
        "0x000000000000001e"
    );
    assert_eq!(parsed["roots"][0]["provider_plan"], "0x000000000000001f");
    assert_eq!(
        parsed["roots"][0]["selected_provider_closure_report_fingerprint"],
        "0x0000000000003033"
    );
    assert_eq!(
        parsed["roots"][0]["selected_provider_closure_digest"]
            .as_str()
            .expect("selected closure digest")
            .len(),
        66
    );
    assert_eq!(
        parsed["roots"][0]["boundary_plan"]["call"]["policy"],
        "system_v_amd64"
    );
    assert_eq!(
        parsed["roots"][0]["boundary_plan"]["state"]["stack"],
        "provider_selected"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["composed_wcsu_bytes"],
        2048
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["origin"],
        "entry"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["admitted_stack_contribution_report_identities"],
        serde_json::json!(["0x0000000000000081", "0x0000000000000082"])
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["admitted_stack_contribution_commitments"],
        serde_json::json!([
            format!("0x{}", "a1".repeat(32)),
            format!("0x{}", "a2".repeat(32)),
        ])
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["composed_domains"][0]["domain"]["kind"],
        "interrupted"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["arrival_contexts"][0]["epochs"]
            [0]["stage"],
        "body"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["entry_installed_code"],
        "0x000000000000012c"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["adapter_origin"],
        "opaque_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["arrival_origin"],
        "opaque_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["target_arrival_rule_fingerprint"],
        serde_json::Value::Null
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["body_domains"][0]["context"],
        "0x0000000000000001"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["body_domains"][0]["domain"]
            ["kind"],
        "interrupted"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["opaque_validation_receipt"],
        "0x000000000000001e"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["composed_units"],
        11
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["schedule_marker"],
        1
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["provision"],
        "0x000000000000001c"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][0]["origin"],
        "admitted_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][1]["origin"],
        "admitted_provider"
    );
    assert!(
        parsed["roots"][0]["resources"]
            .get("structural_work")
            .is_none()
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["machine_state"]["realized_registers"][0],
        "x86_rax"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["machine_state"]["ceiling"]["permitted_transitive_use_bits"],
        "0x0007"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["validation_receipt"],
        "0x0000000000000019"
    );
    assert_eq!(parsed["roots"][0]["effects"][0], "0x0000000000000009");
    assert_eq!(
        parsed["roots"][0]["component_pins"][0]["version"],
        "0x0000000000000010"
    );
    assert!(!first.contains("entry_address"));
    assert!(!first.contains("code_address"));
    assert!(!first.contains("ranking"));
    assert!(!first.contains("codegen"));
}

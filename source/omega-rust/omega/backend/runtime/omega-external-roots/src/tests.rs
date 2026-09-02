use super::*;
use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, CallSignature,
    CallingPolicy, EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineRegime,
    MachineState, MachineStateSet, Preemption, RegisterSet, StackDomainRef, StatePlan,
    ValidatedEntryStackDomainClosure, ValueShape, X86_64ArrivalMechanism, X86_64GateKind,
    X86_64InstalledGateArrival, X86_64InstalledGateRealization, X86_64InstalledGateTssRealization,
    X86_64InstalledPrivilegeStack, X86_64InstalledTaskStateSegmentRealization,
    derive_x86_64_hardware_arrival, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan, validate_entry_stack_domain_closure,
    validate_entry_stack_realization, validate_x86_64_installed_hardware_entry_facts,
};
use omega_effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceSchema,
};
use omega_effects::{
    CheckedComponentProgressDemand, ComponentEraCandidate, ComponentEraEntryLedger,
    ComponentEraLedgerId, ComponentEraPublicationReceipt, ComponentProgressManifest,
    ExecutableTcbManifest, ExecutableTcbProfile, ExecutableTcbProfileAcceptance, ExecutionScope,
    IncompleteScopePolicy, ProgramLocalRootEpochLeaseId, ScopeCompleteness,
    SelectedProviderPlanFacts, evaluate_executable_tcb_profile,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, CodePlacementAuthority,
    CodePlacementId, EntrySetId, FinalValidationCertificate, FinalValidationId, InstallAuthority,
    InstallationAudience, InstallationReceipt, InstallationScopeId, MachineContractSetId,
    MachineFootprintId, MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement,
    admit_executable, install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_installation_evidence::{ObjectEvidence, StackDemandEvidence};
use omega_isa_x86_64::{
    canonical_x86_64_semantic_unit_wrapper_encoding_request,
    encode_x86_64_semantic_unit_wrapper_template,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
    validate_x86_64_resolved_semantic_unit_wrapper,
};
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, MaterializationWrite, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, PlacementSite, PostHandoffWriterPlan,
    PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    BoundaryMachineDeclaration, InstallationReachDependency, ServiceDeclaration,
    StructuralContentProjection, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
    TerminalRootServiceReach, VocabularyMarker,
    program_local_root_introduction_compatibility_report_identity,
};

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
}

fn install_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn extent_provider_issuance(seed: u64) -> psi_extents::ExtentProviderIssuance {
    let base = seed * 16;
    psi_extents::ExtentProviderIssuance::from_normalized_identities([
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

fn entry_id(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
}

fn constraints() -> PlacementConstraints {
    PlacementConstraints::new(
        Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
        4096,
        PlacementPhase::PostHandoff,
        None,
        Some(
            ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope"),
        ),
    )
    .expect("placement constraints")
}

fn installed_code(artifact_identity: u64, entry: EntryStubId) -> InstalledCode {
    installed_code_with_fill(artifact_identity, entry, 0)
}

pub(crate) fn installed_code_with_fill(
    artifact_identity: u64,
    entry: EntryStubId,
    fill: u8,
) -> InstalledCode {
    installed_code_with_fill_and_installation_identity(artifact_identity, entry, fill, 300)
}

fn installed_code_with_fill_and_installation_identity(
    artifact_identity: u64,
    entry: EntryStubId,
    fill: u8,
    installed_code_identity: u64,
) -> InstalledCode {
    installed_code_with_bytes_and_installation_identity(
        artifact_identity,
        entry,
        vec![fill; 64],
        installed_code_identity,
    )
}

fn installed_code_with_bytes_and_installation_identity(
    artifact_identity: u64,
    entry: EntryStubId,
    bytes: Vec<u8>,
    installed_code_identity: u64,
) -> InstalledCode {
    let artifact_constraints = constraints();
    let contracts = install_id(30, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(31, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(artifact_identity, ArtifactId::from_normalized_identity),
        omega_target::Architecture::X86_64,
        bytes,
        contracts,
        footprint,
        install_id(32, PlacementPlanId::from_normalized_identity),
        artifact_constraints,
        install_id(33, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        install_id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        omega_executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            None,
            artifact_constraints
                .installation_scope()
                .map(|scope| (scope, b"test-installation-scope-v1".as_slice())),
        ),
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
        constraints(),
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(
                ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
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
    let installation_receipt = InstallationReceipt::from_provider(
        install_id(
            installed_code_identity,
            InstalledCodeId::from_normalized_identity,
        ),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, install_authority, installation_receipt).expect("installed code")
}

fn installed_program_storage_wrapper(
    artifact_identity: u64,
    entry: EntryStubId,
    resolved_wrapper: &[u8],
) -> (InstalledCode, Vec<u8>) {
    let mut image = vec![0; 16 + resolved_wrapper.len() + 16];
    image[16..16 + resolved_wrapper.len()].copy_from_slice(resolved_wrapper);
    let installed = installed_code_with_bytes_and_installation_identity(
        artifact_identity,
        entry,
        image.clone(),
        300,
    );
    (installed, image)
}

pub(crate) fn boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("validated boundary")
}

fn provider_selected_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary boundary");
    let mut plan = ordinary.plan().clone();
    plan.state.stack = EntryStack::ProviderSelected;
    validate_boundary_entry_plan(plan, &signature).expect("provider-selected boundary")
}

fn provider_selected_masked_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let mut plan = provider_selected_boundary().plan().clone();
    plan.state.preemption = Preemption::Masked;
    validate_boundary_entry_plan(plan, &signature).expect("provider-selected masked boundary")
}

fn interrupted_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary boundary");
    let mut plan = ordinary.plan().clone();
    plan.state.stack = EntryStack::Interrupted;
    validate_boundary_entry_plan(plan, &signature).expect("interrupted boundary")
}

fn generated_program_storage_boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8), ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("receiver-free ProgramStorage semantic continuation boundary")
}

pub(crate) fn generated_program_storage_adapter_live_frame_demand()
-> GeneratedProgramStorageAdapterLiveFrameDemand {
    derive_generated_program_storage_adapter_live_frame_demand(
        &generated_program_storage_adapter_bound_input(),
    )
    .expect("canonical installed generated wrapper derives its live frame demand")
}

fn generated_program_storage_adapter_bound_input() -> BoundEpochStackCompositionInput {
    let entry = entry_id(0x8f1);
    let boundary = generated_program_storage_boundary();
    let request = canonical_x86_64_semantic_unit_wrapper_encoding_request(
        omega_target::NativeTarget::uefi_x64(),
    );
    let template =
        encode_x86_64_semantic_unit_wrapper_template(request).expect("canonical wrapper template");
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        &template,
        template.relocation(),
        16,
        32,
    )
    .expect("resolved private continuation call");
    let (code, installed_image) = installed_program_storage_wrapper(0x8f2, entry, resolved.bytes());
    let machine = psi_core::MachineId::new(1).expect("machine identity");
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x8f; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: installed_image,
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x8f3, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x8f4, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);
    bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect("generated adapter binds exact installed entry and body evidence")
}

fn body_domains(
    boundary: &ValidatedBoundaryEntryPlan,
    contexts: &[(u64, StackDomainRef)],
) -> ValidatedEntryStackDomainClosure {
    validate_entry_stack_domain_closure(
        boundary.plan().state.stack,
        contexts
            .iter()
            .map(|(context, domain)| ArrivalContextStackDomain {
                context: ArrivalContextId::new(*context).expect("arrival context"),
                domain: *domain,
            })
            .collect(),
    )
    .expect("test stack-domain closure")
}

fn admitted_arrival_contexts(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    contexts: &[u64],
    receipt: StackValidationReceiptId,
) -> AdmittedOpaqueArrivalContextSet {
    admit_opaque_arrival_context_set(
        summary,
        boundary,
        code,
        entry,
        contexts
            .iter()
            .map(|context| ArrivalContextId::new(*context).expect("arrival context"))
            .collect(),
        receipt,
    )
    .expect("admitted opaque arrival-context closure")
}

#[derive(Debug)]
struct TestObject {
    identity: psi_terminal::TerminalPsiIdentity,
    entry: psi_core::MachineId,
    bytes: Vec<u8>,
}

impl ObjectEvidence for TestObject {
    fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.identity
    }

    fn target(&self) -> omega_target::NativeTarget {
        omega_target::NativeTarget::linux_x64()
    }

    fn text_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize> {
        (machine == self.entry).then_some(16)
    }
}

struct TestStackDemand {
    identity: psi_terminal::TerminalPsiIdentity,
    entry: psi_core::MachineId,
    contributing: BTreeSet<psi_core::MachineId>,
    admitted_reports: BTreeSet<u64>,
    admitted_commitments: BTreeSet<[u8; 32]>,
}

impl StackDemandEvidence for TestStackDemand {
    fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.identity
    }

    fn architecture(&self) -> omega_target::Architecture {
        omega_target::Architecture::X86_64
    }

    fn entry(&self) -> psi_core::MachineId {
        self.entry
    }

    fn ceiling_bytes(&self) -> u64 {
        64
    }

    fn stack_alignment(&self) -> u32 {
        16
    }

    fn contributing_machines(&self) -> &BTreeSet<psi_core::MachineId> {
        &self.contributing
    }

    fn admitted_stack_contribution_report_identities(&self) -> BTreeSet<u64> {
        self.admitted_reports.clone()
    }

    fn admitted_stack_contribution_commitments(&self) -> BTreeSet<[u8; 32]> {
        self.admitted_commitments.clone()
    }
}

pub(crate) fn fixed_fuel() -> ComposedFuelDemand {
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        root_id(31, ProviderFuelSummaryId::from_normalized_identity),
        root_id(12, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        5,
        BTreeSet::new(),
        root_id(
            41,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_id(30, ProviderFuelSummaryId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        2,
        BTreeSet::from([FixedFuelCall {
            callee: leaf.identity,
            maximum_invocations: 1,
        }]),
        root_id(
            40,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    compose_fixed_fuel(root.identity, [&root, &leaf]).expect("fixed-fuel composition")
}

fn stack_demand(
    root: ExternalRootId,
    provider: RootProviderId,
    relation: NestingRelationId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    resolved_stack: EntryStack,
    local_wcsu_bytes: u64,
) -> BoundEpochStackComposition {
    let active_domain = StackDomainRef::from(resolved_stack);
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain,
                occupancy_by_domain: Vec::new(),
                nesting: boundary.plan().state.preemption,
            }],
        }],
    })
    .expect("test epoch realization");
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        local_wcsu_bytes,
        16,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    let contexts = admitted_arrival_contexts(
        &summary,
        boundary,
        code,
        entry,
        &[1],
        root_id(48, StackValidationReceiptId::from_normalized_identity),
    );
    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        boundary,
        code,
        entry,
        realization,
        contexts,
    )
    .expect("test epoch evidence binding");
    compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation,
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("bound epoch stack composition")
}

fn candidate(entry: EntryStubId) -> ExternalRootCandidate {
    candidate_for_code(entry, &installed_code(1, entry))
}

fn candidate_for_code(entry: EntryStubId, code: &InstalledCode) -> ExternalRootCandidate {
    candidate_for_code_with_root(entry, code, 1)
}

pub(crate) fn candidate_for_code_with_root(
    entry: EntryStubId,
    code: &InstalledCode,
    root_identity: u64,
) -> ExternalRootCandidate {
    let root = root_id(root_identity, ExternalRootId::from_normalized_identity);
    let provider = root_id(2, RootProviderId::from_normalized_identity);
    let nesting_relation = root_id(6, NestingRelationId::from_normalized_identity);
    let boundary = boundary();
    ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(55, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "TestRoot::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &omega_effects::SelectedProviderPlanFacts::default(),
        )
        .expect("empty root service reach"),
        effects: [root_id(3, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [root_id(4, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation,
        acknowledgement_policy: Some(root_id(
            7,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: stack_demand(
                root,
                provider,
                nesting_relation,
                &boundary,
                code,
                entry,
                EntryStack::Interrupted,
                2048,
            ),
            validation_receipt: root_id(50, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
            provision: root_id(53, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: fixed_fuel(),
            validation_receipt: root_id(51, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(52, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: [ComponentVersionPin {
            contract: root_id(8, ComponentContractId::from_normalized_identity),
            artifact: root_id(9, ComponentArtifactId::from_normalized_identity),
            provider: root_id(10, ComponentProviderId::from_normalized_identity),
            version: root_id(11, ComponentVersionPinId::from_normalized_identity),
        }]
        .into_iter()
        .collect(),
    }
}

fn selected_interrupt_completion_for(
    name: &str,
    provider_type: &str,
    machine_identity: &str,
    resolved_row: &[&str],
) -> omega_effects::SelectedProviderPlanFacts {
    let requirement_identity = "InterruptCompletion::complete".to_owned();
    let plan = ProviderPlan {
        name: name.into(),
        provider_type: provider_type.into(),
        provider_type_package_identity: None,
        target: "x86_64-unknown-none".into(),
        schema: ServiceSchema {
            trait_name: "InterruptCompletion".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "complete".into(),
                requirement_owner: "InterruptCompletion".into(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement_identity.clone(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["InterruptCompletion".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "complete".into(),
            requirement_identity: requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: machine_identity.into(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    };
    let identity = plan.report_fingerprint();
    omega_effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("selected interrupt completion provider")
    .with_installation_reach_resolutions(vec![omega_effects::InstallationReachResolution {
        requirement_identity,
        provider_plan_report_identity: identity,
        upper_bound: vec!["PortIo".into(), "MachineControl".into()],
        resolved_row: resolved_row
            .iter()
            .map(|service| (*service).to_owned())
            .collect(),
    }])
    .expect("provider reach refines the interrupt completion bound")
}

fn selected_interrupt_completion() -> omega_effects::SelectedProviderPlanFacts {
    selected_interrupt_completion_for(
        "LegacyPic",
        "LegacyPicController",
        "LegacyPicController::complete",
        &["PortIo"],
    )
}

#[test]
fn root_service_reach_substitutes_selected_rows_and_rejects_absence() {
    let selected = selected_interrupt_completion();
    let requirement = "InterruptCompletion::complete".to_owned();
    let reach = ResolvedRootServiceReach::from_selected_provider_closure(
        vec!["Timer".into()],
        vec![requirement.clone()],
        &selected,
    )
    .expect("selected provider closes the root reach");

    assert_eq!(reach.concrete(), ["Timer"]);
    assert_eq!(
        reach.installation_requirements(),
        ["InterruptCompletion::complete"]
    );
    assert_eq!(reach.effective(), ["PortIo", "Timer"]);
    assert_eq!(reach.resolutions().len(), 1);
    assert_eq!(
        reach.selected_provider_closure_report_fingerprint(),
        selected.report_fingerprint()
    );
    assert_eq!(
        reach.selected_provider_closure_digest(),
        selected.identity_digest()
    );

    let error = ResolvedRootServiceReach::from_selected_provider_closure(
        Vec::new(),
        vec!["Missing::requirement".into()],
        &selected,
    )
    .expect_err("an installed root cannot retain an unresolved reach row");
    assert!(error.0.contains("remains unresolved at final admission"));
}

#[test]
fn direct_generated_entry_derives_one_exact_body_epoch_without_provider_attestation() {
    let entry = entry_id(0x801);
    let code = installed_code(0x802, entry);
    let boundary = boundary();
    let machine = psi_core::MachineId::new(1).expect("machine identity");
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x81; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: vec![0; 64],
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::from([0x812]),
        admitted_commitments: BTreeSet::from([[0x81; 32], [0x82; 32]]),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x803, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x804, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);
    let bound = bind_direct_generated_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
    )
    .expect("direct generated entry derives its realization");

    assert_eq!(
        bound.realization_evidence().arrival_origin(),
        ArrivalStackRealizationOrigin::NoHardwareArrival
    );
    assert_eq!(
        bound.realization_evidence().adapter_origin(),
        AdapterStackRealizationOrigin::None
    );
    assert_eq!(bound.realization_evidence().validation_receipt(), None);
    let StackLocalEvidence::TerminalEntry(bound_body) = bound.body_evidence() else {
        panic!("direct generated entry retains terminal body evidence")
    };
    assert_eq!(
        bound_body.admitted_stack_contribution_report_identities(),
        &BTreeSet::from([0x812]),
    );
    assert_eq!(
        bound_body.admitted_stack_contribution_commitments(),
        &BTreeSet::from([[0x81; 32], [0x82; 32]]),
    );
    let contexts = &bound
        .realization_evidence()
        .realization()
        .realization()
        .contexts;
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].epochs.len(), 1);
    assert_eq!(contexts[0].epochs[0].stage, EntryStackStage::Body);
    assert_eq!(
        contexts[0].epochs[0].active_domain,
        StackDomainRef::Interrupted
    );

    let fixed_boundary = interrupted_boundary();
    let error = bind_direct_generated_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&fixed_boundary, &[(1, StackDomainRef::Interrupted)]),
    )
    .expect_err("direct entry cannot replay a closure for another public stack disposition");
    assert!(
        error
            .0
            .contains("drifted from the boundary stack disposition")
    );

    let exact_composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x805, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("exact installed body provenance composes");
    let compose_alternate = |admitted_reports: BTreeSet<u64>,
                             admitted_commitments: BTreeSet<[u8; 32]>| {
        let demand = TestStackDemand {
            identity: psi,
            entry: machine,
            contributing: BTreeSet::from([machine]),
            admitted_reports,
            admitted_commitments,
        };
        let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
            .expect("alternate demand binds its own exact projection");
        let summary = ProviderStackSummary::from_entry(
            root,
            provider,
            boundary.plan().state.stack,
            installed,
        );
        let bound = bind_direct_generated_entry_stack_realization(
            &summary,
            &boundary,
            &code,
            entry,
            body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        )
        .expect("alternate body evidence remains structurally valid");
        compose_bound_entry_stack_epochs(
            &StackNestingRelation {
                identity: root_id(0x805, NestingRelationId::from_normalized_identity),
                edges: BTreeSet::new(),
            },
            [&bound],
        )
        .expect("alternate installed body provenance composes separately")
    };
    let substitute_composition = compose_alternate(
        BTreeSet::from([0x812]),
        BTreeSet::from([[0x81; 32], [0x91; 32]]),
    );
    assert_eq!(
        exact_composition.composition(),
        substitute_composition.composition(),
        "pure epoch composition remains numeric and structural",
    );
    assert_ne!(
        exact_composition, substitute_composition,
        "exact bound composition retains the strong provenance distinction",
    );
    assert_ne!(
        exact_composition.report_fingerprint(),
        substitute_composition.report_fingerprint(),
        "compact-equal admitted reports cannot hide a strong commitment substitution",
    );
    let omitted_composition = compose_alternate(BTreeSet::new(), BTreeSet::new());
    assert_eq!(
        exact_composition.composition(),
        omitted_composition.composition(),
        "foreign-free and admitted-foreign bodies may have equal numeric demand",
    );
    assert_ne!(
        exact_composition, omitted_composition,
        "omitting both provenance sets remains distinguishable from a foreign-bearing body",
    );
    assert_ne!(
        exact_composition.report_fingerprint(),
        omitted_composition.report_fingerprint(),
        "the bound report retains presence versus omission of admitted foreign premises",
    );
}

#[test]
fn installed_entry_stack_rejects_incomplete_or_zero_foreign_provenance() {
    let entry = entry_id(0x806);
    let code = installed_code(0x807, entry);
    let machine = psi_core::MachineId::new(1).expect("machine identity");
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x82; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: vec![0; 64],
    };

    for (admitted_reports, admitted_commitments) in [
        (BTreeSet::from([7]), BTreeSet::new()),
        (BTreeSet::new(), BTreeSet::from([[7; 32]])),
        (BTreeSet::from([0]), BTreeSet::from([[7; 32]])),
        (BTreeSet::from([7]), BTreeSet::from([[0; 32]])),
    ] {
        let demand = TestStackDemand {
            identity: psi,
            entry: machine,
            contributing: BTreeSet::from([machine]),
            admitted_reports,
            admitted_commitments,
        };
        let error = bind_installed_entry_stack(&demand, &artifact, &code, entry)
            .expect_err("invalid admitted foreign provenance must fail closed");
        assert!(
            error
                .0
                .contains("incomplete or zero admitted same-stack provenance")
        );
    }
}

#[test]
fn generated_program_storage_adapter_replays_emitted_operations_and_composes_three_epochs() {
    let entry = entry_id(0x831);
    let boundary = generated_program_storage_boundary();
    let request = canonical_x86_64_semantic_unit_wrapper_encoding_request(
        omega_target::NativeTarget::uefi_x64(),
    );
    let template =
        encode_x86_64_semantic_unit_wrapper_template(request).expect("canonical wrapper template");
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        &template,
        template.relocation(),
        16,
        32,
    )
    .expect("resolved private continuation call");
    let (code, installed_image) = installed_program_storage_wrapper(0x832, entry, resolved.bytes());
    let machine = psi_core::MachineId::new(1).expect("machine identity");
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x83; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: installed_image,
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x833, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x834, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);

    let bound = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect("generated adapter binds exact installed entry and body evidence");

    assert_eq!(
        bound.realization_evidence().arrival_origin(),
        ArrivalStackRealizationOrigin::NoHardwareArrival
    );
    assert_eq!(
        bound.realization_evidence().adapter_origin(),
        AdapterStackRealizationOrigin::GeneratedProgramStorageSemanticWrapper
    );
    assert_eq!(bound.realization_evidence().validation_receipt(), None);
    let generated = bound
        .realization_evidence()
        .generated_adapter()
        .expect("exact generated-adapter evidence");
    assert_eq!(generated.request(), request);
    assert_eq!(generated.resolved_bytes(), resolved.bytes());
    assert_eq!(generated.resolution(), resolved.resolution());
    assert_ne!(generated.report_fingerprint(), 0);
    let epochs = &bound
        .realization_evidence()
        .realization()
        .realization()
        .contexts[0]
        .epochs;
    assert_eq!(
        epochs.iter().map(|epoch| epoch.stage).collect::<Vec<_>>(),
        [
            EntryStackStage::Enter,
            EntryStackStage::Body,
            EntryStackStage::Exit,
        ]
    );
    for epoch in epochs {
        assert_eq!(epoch.active_domain, StackDomainRef::Interrupted);
        assert_eq!(
            epoch.occupancy_by_domain,
            [omega_calling_conventions::StackOccupancy {
                domain: StackDomainRef::Interrupted,
                bytes: 72,
                alignment: 16,
            }]
        );
    }

    let composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x835, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("generated adapter epochs compose with the Terminal body");
    assert_eq!(
        composition.domain(StackDomain::Interrupted),
        Some(DomainStackDemand {
            bytes: 144,
            alignment: 16,
        }),
        "the live 72-byte wrapper frame aligns before the 64-byte body WCSU",
    );
}

#[test]
fn generated_program_storage_adapter_derives_live_frame_demand_from_exact_epochs() {
    let bound = generated_program_storage_adapter_bound_input();
    let demand = derive_generated_program_storage_adapter_live_frame_demand(&bound)
        .expect("exact generated wrapper derives one live-frame contribution");
    assert_eq!(demand.bytes(), 72);
    assert_eq!(demand.alignment(), 16);
    assert_eq!(
        demand.installed_code(),
        bound.realization_evidence().installed_code()
    );
    assert_eq!(demand.entry(), bound.realization_evidence().entry());
    assert_eq!(
        demand.semantic_boundary_commitment(),
        bound.realization_evidence().boundary_contract_commitment(),
    );

    let missing_origin = bound.clone().without_generated_adapter_origin_for_test();
    let error = derive_generated_program_storage_adapter_live_frame_demand(&missing_origin)
        .expect_err("a generated-frame claim without generated origin must reject");
    assert!(error.0.contains("wrong realization origin or custody"));

    let drifted_epoch = bound.with_first_adapter_epoch_bytes_for_test(80);
    let error = derive_generated_program_storage_adapter_live_frame_demand(&drifted_epoch)
        .expect_err("an independently valid but drifted epoch must reject");
    assert!(error.0.contains("exact Enter/Body/Exit occupancy"));
}

#[test]
fn generated_program_storage_adapter_rejects_mutation_and_installed_subject_substitution() {
    let entry = entry_id(0x841);
    let boundary = generated_program_storage_boundary();
    let request = canonical_x86_64_semantic_unit_wrapper_encoding_request(
        omega_target::NativeTarget::uefi_x64(),
    );
    let template =
        encode_x86_64_semantic_unit_wrapper_template(request).expect("canonical wrapper template");
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        &template,
        template.relocation(),
        16,
        32,
    )
    .expect("resolved wrapper");
    let (code, installed_image) = installed_program_storage_wrapper(0x842, entry, resolved.bytes());
    let machine = psi_core::MachineId::new(1).expect("machine identity");
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x84; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: installed_image,
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure");
    let root = root_id(0x843, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x844, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);

    let mut mutated_template = template.bytes().to_vec();
    mutated_template[3] ^= 8;
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: &mutated_template,
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("reserve-byte mutation must reject before epoch derivation");
    assert!(
        error
            .0
            .contains("template failed exact emitted-operation replay")
    );

    let mut mutated_resolved = resolved.bytes().to_vec();
    mutated_resolved[81] ^= 1;
    assert!(
        validate_x86_64_resolved_semantic_unit_wrapper(
            &template,
            template.relocation(),
            16,
            32,
            &mutated_resolved,
        )
        .is_err(),
        "redirecting the resolved private call must fail target replay",
    );
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: &mutated_resolved,
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("resolved call redirection must reject before epoch derivation");
    assert!(
        error
            .0
            .contains("continuation call failed exact emitted-operation replay")
    );

    let mut changed_installed_wrapper = resolved.bytes().to_vec();
    changed_installed_wrapper[4] ^= 1;
    let (changed_code, changed_image) =
        installed_program_storage_wrapper(0x842, entry, &changed_installed_wrapper);
    let changed_artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: changed_image,
    };
    let changed_installed =
        bind_installed_entry_stack(&demand, &changed_artifact, &changed_code, entry)
            .expect("mutated installed image has its own exact Terminal body evidence");
    let changed_summary = ProviderStackSummary::from_entry(
        root,
        provider,
        boundary.plan().state.stack,
        changed_installed,
    );
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &changed_summary,
        &boundary,
        &changed_code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("validated emitted bytes cannot substitute a mutated installed entry interval");
    assert!(error.0.contains("exact installed entry interval"));

    let foreign_entry = entry_id(0x845);
    let foreign_code = installed_code(0x846, foreign_entry);
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &foreign_code,
        foreign_entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("body evidence cannot cross to another installed artifact and entry");
    assert!(error.0.contains("different installed entry"));

    let incompatible_boundary = crate::tests::boundary();
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &incompatible_boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("the generated wrapper cannot bind a different continuation ABI");
    assert!(error.0.contains("receiver-free Microsoft-x64"));
}

#[test]
fn x86_target_arrival_binds_exact_installation_and_composes_mixed_contexts() {
    let entry = entry_id(0x821);
    let code = installed_code(0x822, entry);
    let mut plan = provider_selected_masked_boundary().plan().clone();
    plan.call.entry_control = EntryControl::InterruptReturn;
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let boundary = validate_boundary_entry_plan(plan, &signature)
        .expect("provider-selected interrupt boundary");
    let machine = psi_core::MachineId::new(1).expect("machine identity");
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x82; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: vec![0; 64],
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x823, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x824, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);
    let realization = X86_64InstalledGateTssRealization {
        gate: X86_64InstalledGateRealization {
            vector: 14,
            gate: X86_64GateKind::Interrupt,
            entry_identity: entry.normalized_identity(),
            entry_privilege: 0,
            interrupt_stack_table_slot: None,
        },
        tss: X86_64InstalledTaskStateSegmentRealization {
            privilege_stacks: vec![X86_64InstalledPrivilegeStack {
                entry_privilege: 0,
                dedicated_class: 7,
            }],
            interrupt_stacks: Vec::new(),
        },
        arrivals: vec![
            X86_64InstalledGateArrival {
                context: ArrivalContextId::new(2).expect("kernel arrival context"),
                mechanism: X86_64ArrivalMechanism::Exception,
                interrupted_privilege: 0,
            },
            X86_64InstalledGateArrival {
                context: ArrivalContextId::new(1).expect("user arrival context"),
                mechanism: X86_64ArrivalMechanism::Exception,
                interrupted_privilege: 3,
            },
        ],
    };
    let roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &code,
        entry,
        realization.clone(),
        root_id(
            0x826,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("table/profile validation seals the complete arrival roster");
    let installed_target_arrival = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        realization.clone(),
    )
    .expect("installed gate/TSS produces exact x86 target facts");
    let facts = installed_target_arrival.facts().facts().clone();
    assert_eq!(facts.contexts.len(), 2);
    assert!(matches!(
        facts.contexts[0].stack_selection,
        omega_calling_conventions::X86_64HardwareStackSelection::PrivilegeTransition {
            dedicated_class: 7
        }
    ));
    assert_eq!(facts.contexts[0].nesting, Preemption::Masked);
    assert!(matches!(
        facts.contexts[1].stack_selection,
        omega_calling_conventions::X86_64HardwareStackSelection::Current
    ));

    let mut omitted_context = realization.clone();
    omitted_context.arrivals.pop();
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        omitted_context,
    )
    .expect_err("a public gate/TSS roster cannot omit one validated context");
    assert!(
        error
            .0
            .contains("complete validated gate/profile realization")
    );

    let mut padded_context = realization.clone();
    padded_context.arrivals.push(X86_64InstalledGateArrival {
        context: ArrivalContextId::new(3).expect("padded arrival context"),
        mechanism: X86_64ArrivalMechanism::Exception,
        interrupted_privilege: 0,
    });
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        padded_context,
    )
    .expect_err("a public gate/TSS roster cannot pad the validated context set");
    assert!(
        error
            .0
            .contains("complete validated gate/profile realization")
    );

    let mut changed_vector = realization.clone();
    changed_vector.gate.vector = 13;
    let mut changed_kind = realization.clone();
    changed_kind.gate.gate = X86_64GateKind::Trap;
    let mut changed_entry_privilege = realization.clone();
    changed_entry_privilege.gate.entry_privilege = 1;
    changed_entry_privilege
        .tss
        .privilege_stacks
        .push(X86_64InstalledPrivilegeStack {
            entry_privilege: 1,
            dedicated_class: 7,
        });
    let mut changed_ist = realization.clone();
    changed_ist.gate.interrupt_stack_table_slot = Some(3);
    changed_ist.tss.interrupt_stacks.push(
        omega_calling_conventions::X86_64InstalledInterruptStack {
            slot: 3,
            dedicated_class: 7,
        },
    );
    let mut changed_tss_class = realization.clone();
    changed_tss_class.tss.privilege_stacks[0].dedicated_class = 8;
    for (label, changed) in [
        ("vector", changed_vector),
        ("gate kind", changed_kind),
        ("entry privilege", changed_entry_privilege),
        ("IST selection", changed_ist),
        ("TSS class", changed_tss_class),
    ] {
        let error = produce_x86_64_installed_hardware_entry_facts(
            &boundary, &code, entry, 16, &roster, changed,
        )
        .expect_err("gate/TSS substitution under one validation receipt must reject");
        assert!(
            error
                .0
                .contains("complete validated gate/profile realization"),
            "{label} substitution escaped exact gate/profile replay: {}",
            error.0
        );
    }

    let foreign_code = installed_code(0x828, entry);
    let foreign_roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &foreign_code,
        entry,
        realization.clone(),
        root_id(
            0x829,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("foreign table/profile validation seals its own exact occurrence");
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &foreign_roster,
        realization.clone(),
    )
    .expect_err("a foreign installed occurrence roster cannot cross into production");
    assert!(error.0.contains("different exact installed occurrence"));

    let compact_equal_code = installed_code_with_fill(0x822, entry, 1);
    assert_eq!(compact_equal_code.identity(), code.identity());
    assert_eq!(compact_equal_code.artifact(), code.artifact());
    assert_ne!(compact_equal_code.receipt_context(), code.receipt_context());
    let compact_equal_roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &compact_equal_code,
        entry,
        realization.clone(),
        root_id(
            0x82a,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("compact-equal occurrence seals a distinct exact roster");
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &compact_equal_roster,
        realization.clone(),
    )
    .expect_err("compact-equal installed occurrence substitution must reject");
    assert!(error.0.contains("different exact installed occurrence"));
    let compact_equal_arrival = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &compact_equal_code,
        entry,
        16,
        &compact_equal_roster,
        realization.clone(),
    )
    .expect("compact-equal occurrence produces only its own retained arrival");
    let error = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &compact_equal_arrival,
    )
    .expect_err("binder must compare retained InstalledCodeContext, not compact IDs");
    assert!(error.0.contains("different exact installed occurrence"));

    let mut missing_privilege_stack = realization.clone();
    missing_privilege_stack.tss.privilege_stacks.clear();
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        missing_privilege_stack,
    )
    .expect_err("a privilege transition cannot invent an absent installed TSS stack");
    assert!(error.0.contains("absent TSS privilege-0 stack"));

    let mut repeated_privilege_stack = realization.clone();
    repeated_privilege_stack
        .tss
        .privilege_stacks
        .push(repeated_privilege_stack.tss.privilege_stacks[0]);
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        repeated_privilege_stack,
    )
    .expect_err("a repeated installed TSS privilege stack must reject");
    assert!(error.0.contains("repeated privilege-stack selection"));

    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        17,
        &roster,
        realization.clone(),
    )
    .expect_err("installed entry-offset drift must reject before target derivation");
    assert!(error.0.contains("different installed entry offset"));

    let mut wrong_entry = realization.clone();
    wrong_entry.gate.entry_identity ^= 1;
    let error = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &roster,
        wrong_entry,
    )
    .expect_err("installed descriptor entry substitution must reject");
    assert!(error.0.contains("different compiler entry identity"));

    let error = produce_x86_64_installed_hardware_entry_facts(
        &provider_selected_masked_boundary(),
        &code,
        entry,
        16,
        &roster,
        realization.clone(),
    )
    .expect_err("ordinary CallReturn cannot masquerade as a hardware gate");
    assert!(error.0.contains("InterruptReturn"));

    let mut dedicated_plan = boundary.plan().clone();
    dedicated_plan.state.stack = EntryStack::Dedicated { class: 9 };
    let dedicated_boundary = validate_boundary_entry_plan(dedicated_plan, &signature)
        .expect("dedicated interrupt boundary");
    let ist_realization = X86_64InstalledGateTssRealization {
        gate: X86_64InstalledGateRealization {
            interrupt_stack_table_slot: Some(3),
            ..realization.gate
        },
        tss: X86_64InstalledTaskStateSegmentRealization {
            privilege_stacks: Vec::new(),
            interrupt_stacks: vec![omega_calling_conventions::X86_64InstalledInterruptStack {
                slot: 3,
                dedicated_class: 9,
            }],
        },
        arrivals: realization.arrivals.clone(),
    };
    let dedicated_roster = validate_x86_64_installed_gate_profile_roster(
        &dedicated_boundary,
        &code,
        entry,
        ist_realization.clone(),
        root_id(
            0x827,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("dedicated table/profile validation seals the complete roster");
    let ist_facts = produce_x86_64_installed_hardware_entry_facts(
        &dedicated_boundary,
        &code,
        entry,
        16,
        &dedicated_roster,
        ist_realization.clone(),
    )
    .expect("installed gate resolves its exact TSS IST selection");
    assert!(
        ist_facts
            .facts()
            .facts()
            .contexts
            .iter()
            .all(|context| matches!(
                context.stack_selection,
                omega_calling_conventions::X86_64HardwareStackSelection::InterruptStackTable {
                    slot: 3,
                    dedicated_class: 9
                }
            ))
    );
    let mut missing_ist = ist_realization.clone();
    missing_ist.tss.interrupt_stacks.clear();
    let error = produce_x86_64_installed_hardware_entry_facts(
        &dedicated_boundary,
        &code,
        entry,
        16,
        &dedicated_roster,
        missing_ist,
    )
    .expect_err("an installed gate cannot select an absent TSS IST slot");
    assert!(error.0.contains("absent TSS interrupt-stack slot 3"));

    let mut repeated_ist = ist_realization;
    repeated_ist
        .tss
        .interrupt_stacks
        .push(repeated_ist.tss.interrupt_stacks[0]);
    let error = produce_x86_64_installed_hardware_entry_facts(
        &dedicated_boundary,
        &code,
        entry,
        16,
        &dedicated_roster,
        repeated_ist,
    )
    .expect_err("a repeated installed TSS IST row must reject");
    assert!(error.0.contains("repeated interrupt-stack selection"));

    let bound = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &installed_target_arrival,
    )
    .expect("target arrival binds exact emitted body");

    let alternate_roster = validate_x86_64_installed_gate_profile_roster(
        &boundary,
        &code,
        entry,
        realization.clone(),
        root_id(
            0x82b,
            X86_64GateProfileValidationReceiptId::from_normalized_identity,
        ),
    )
    .expect("a second validation retains its distinct typed receipt");
    let alternate_arrival = produce_x86_64_installed_hardware_entry_facts(
        &boundary,
        &code,
        entry,
        16,
        &alternate_roster,
        realization.clone(),
    )
    .expect("the same exact realization may carry a later validation receipt");
    let alternate_bound = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &alternate_arrival,
    )
    .expect("alternate typed receipt binds the same exact target facts");
    assert_eq!(
        bound
            .realization_evidence()
            .target_installation_validation_receipt(),
        Some(roster.validation_receipt())
    );
    assert_eq!(
        alternate_bound
            .realization_evidence()
            .target_installation_validation_receipt(),
        Some(alternate_roster.validation_receipt())
    );
    assert_ne!(
        bound
            .realization_evidence()
            .target_installation_validation_receipt(),
        alternate_bound
            .realization_evidence()
            .target_installation_validation_receipt()
    );
    assert_eq!(
        bound
            .realization_evidence()
            .target_rule_report_fingerprint(),
        alternate_bound
            .realization_evidence()
            .target_rule_report_fingerprint(),
        "receipt identity is provenance, not a different architectural rule"
    );

    assert_eq!(
        bound.realization_evidence().arrival_origin(),
        ArrivalStackRealizationOrigin::X86_64TargetRule
    );
    assert_eq!(
        bound.realization_evidence().adapter_origin(),
        AdapterStackRealizationOrigin::None
    );
    assert_eq!(
        bound
            .realization_evidence()
            .target_rule_report_fingerprint(),
        Some(installed_target_arrival.report_fingerprint())
    );
    let composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x825, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("mixed target contexts compose");
    let alternate_composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x825, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&alternate_bound],
    )
    .expect("the alternate typed receipt composes the same numeric demand");
    assert_ne!(
        composition.report_fingerprint(),
        alternate_composition.report_fingerprint(),
        "typed table/profile receipt identity must survive in bound report provenance"
    );
    let demand = composition.demand(root).expect("root demand");
    assert_eq!(
        demand
            .domain(StackDomain::Interrupted)
            .expect("same-CPL domain")
            .bytes,
        96
    );
    assert_eq!(
        demand
            .domain(StackDomain::Dedicated { class: 7 })
            .expect("privilege-transition domain")
            .bytes,
        112
    );

    let mut wrong_boundary = facts.clone();
    wrong_boundary.identity.boundary_plan_report_fingerprint ^= 1;
    let wrong_arrival = derive_x86_64_hardware_arrival(
        &validate_x86_64_installed_hardware_entry_facts(wrong_boundary)
            .expect("structurally valid but foreign boundary identity"),
    )
    .expect("target rule still derives its own exact claim");
    let wrong_arrival = installed_target_arrival
        .clone()
        .with_target_arrival_for_test(wrong_arrival);
    let error = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &wrong_arrival,
    )
    .expect_err("target arrival cannot replay across a boundary contract");
    assert!(
        error
            .0
            .contains("different installed artifact, entry, or boundary")
    );

    let mut compact_equal_wrong_commitment = facts;
    compact_equal_wrong_commitment
        .identity
        .boundary_plan_commitment[0] ^= 1;
    assert_eq!(
        compact_equal_wrong_commitment
            .identity
            .boundary_plan_report_fingerprint,
        boundary.contract_report_fingerprint(),
        "the adversary deliberately retains the compact report coordinate",
    );
    let wrong_arrival = derive_x86_64_hardware_arrival(
        &validate_x86_64_installed_hardware_entry_facts(compact_equal_wrong_commitment)
            .expect("nonzero compact-equal commitment substitute remains structurally decodable"),
    )
    .expect("target rule retains the caller-supplied installed-fact commitment");
    let wrong_arrival = installed_target_arrival
        .clone()
        .with_target_arrival_for_test(wrong_arrival);
    let error = bind_x86_64_target_direct_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        &wrong_arrival,
    )
    .expect_err("compact-equal boundary commitment substitution must reject");
    assert!(
        error
            .0
            .contains("different installed artifact, entry, or boundary")
    );
}

#[test]
fn opaque_epoch_realization_binds_exact_installed_entry_plan_and_body_evidence() {
    let entry = entry_id(0x811);
    let code = installed_code(0x812, entry);
    let boundary = interrupted_boundary();
    let root = root_id(0x813, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x814, RootProviderId::from_normalized_identity);
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        64,
        16,
        root_id(0x815, StackValidationReceiptId::from_normalized_identity),
    );
    let realization = |nesting| {
        validate_entry_stack_realization(EntryStackRealization {
            contexts: vec![ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("arrival context"),
                epochs: vec![EntryStackEpoch {
                    stage: EntryStackStage::Body,
                    active_domain: StackDomainRef::Interrupted,
                    occupancy_by_domain: Vec::new(),
                    nesting,
                }],
            }],
        })
        .expect("structurally valid epoch realization")
    };
    let context_evidence = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[1],
        root_id(0x816, StackValidationReceiptId::from_normalized_identity),
    );
    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::NotApplicable),
        context_evidence.clone(),
    )
    .expect("opaque realization binds to exact installed root");

    for substituted_commitment in [[0; 32], [0xa5; 32]] {
        let compact_equal_substitute = context_evidence
            .clone()
            .with_boundary_contract_commitment_for_test(substituted_commitment);
        let error = bind_opaque_adapter_stack_realization(
            &summary,
            &boundary,
            &code,
            entry,
            realization(Preemption::NotApplicable),
            compact_equal_substitute,
        )
        .expect_err("zero or compact-equal wrong boundary commitment must reject");
        assert!(error.0.contains("different installed root"));
    }

    let composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x817, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("bound epoch evidence composes");
    assert_eq!(
        composition.composition().domain(StackDomain::Interrupted),
        Some(DomainStackDemand {
            bytes: 64,
            alignment: 16,
        })
    );
    assert_eq!(
        composition
            .input(root)
            .expect("retained exact evidence")
            .realization_evidence()
            .validation_receipt(),
        Some(root_id(
            0x816,
            StackValidationReceiptId::from_normalized_identity
        ))
    );

    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::Nestable { maximum_depth: 2 }),
        context_evidence.clone(),
    )
    .expect_err("opaque epoch evidence cannot widen the published nesting ceiling");
    assert!(
        error
            .0
            .contains("widens the boundary plan's nesting ceiling")
    );

    let error = admit_opaque_arrival_context_set(
        &summary,
        &boundary,
        &code,
        entry_id(0x819),
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(0x81a, StackValidationReceiptId::from_normalized_identity),
    )
    .expect_err("opaque context evidence cannot name an absent installed entry");
    assert!(error.0.contains("names no exact installed entry"));

    let aarch64_boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::Aapcs64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("AArch64 boundary plan");
    let error = admit_opaque_arrival_context_set(
        &summary,
        &aarch64_boundary,
        &code,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(0x81b, StackValidationReceiptId::from_normalized_identity),
    )
    .expect_err("opaque context evidence cannot cross target architectures");
    assert!(
        error
            .0
            .contains("differs from the installed artifact architecture")
    );

    let wrong_body_domain = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain: StackDomainRef::Dedicated { class: 1 },
                occupancy_by_domain: Vec::new(),
                nesting: Preemption::NotApplicable,
            }],
        }],
    })
    .expect("structurally valid wrong-domain realization");
    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        wrong_body_domain,
        context_evidence.clone(),
    )
    .expect_err("opaque epoch evidence cannot move the handler body to another stack");
    assert!(
        error
            .0
            .contains("body stack domain differs from the fixed boundary stack disposition")
    );

    let omitted = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[2],
        root_id(0x81d, StackValidationReceiptId::from_normalized_identity),
    );
    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::NotApplicable),
        omitted,
    )
    .expect_err("an admitted but different opaque context set cannot license the realization");
    assert!(error.0.contains("different context sets"));

    let padded = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[1, 2],
        root_id(0x81e, StackValidationReceiptId::from_normalized_identity),
    );
    let error = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization(Preemption::NotApplicable),
        padded,
    )
    .expect_err("a padded opaque context claim cannot license the realization");
    assert!(error.0.contains("different context sets"));
}

#[test]
fn opaque_arrival_context_admission_is_nonempty_unique_and_canonical() {
    let entry = entry_id(0x831);
    let code = installed_code(0x832, entry);
    let boundary = interrupted_boundary();
    let summary = ProviderStackSummary::from_admitted_provider(
        root_id(0x833, ExternalRootId::from_normalized_identity),
        root_id(0x834, RootProviderId::from_normalized_identity),
        boundary.plan().state.stack,
        64,
        16,
        root_id(0x835, StackValidationReceiptId::from_normalized_identity),
    );
    let receipt = root_id(0x836, StackValidationReceiptId::from_normalized_identity);

    let empty =
        admit_opaque_arrival_context_set(&summary, &boundary, &code, entry, Vec::new(), receipt)
            .expect_err("an empty opaque arrival-context claim cannot be complete");
    assert!(empty.0.contains("contains no context"));

    let context = ArrivalContextId::new(1).expect("arrival context");
    let duplicate = admit_opaque_arrival_context_set(
        &summary,
        &boundary,
        &code,
        entry,
        vec![context, context],
        receipt,
    )
    .expect_err("duplicate context identities are not a set");
    assert!(duplicate.0.contains("repeats a context identity"));

    let second = ArrivalContextId::new(2).expect("arrival context");
    let admitted = admit_opaque_arrival_context_set(
        &summary,
        &boundary,
        &code,
        entry,
        vec![second, context],
        receipt,
    )
    .expect("input order does not change the admitted context set");
    assert_eq!(admitted.contexts(), [context, second]);
}

#[test]
fn provider_selected_stack_closes_independently_in_each_arrival_context() {
    let entry = entry_id(0x821);
    let code = installed_code(0x822, entry);
    let boundary = provider_selected_boundary();
    let root = root_id(0x823, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x824, RootProviderId::from_normalized_identity);
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        EntryStack::ProviderSelected,
        64,
        16,
        root_id(0x825, StackValidationReceiptId::from_normalized_identity),
    );
    let body_epoch = |active_domain| EntryStackEpoch {
        stage: EntryStackStage::Body,
        active_domain,
        occupancy_by_domain: Vec::new(),
        nesting: Preemption::NotApplicable,
    };
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![
            ArrivalContextRealization {
                context: ArrivalContextId::new(1).expect("arrival context"),
                epochs: vec![body_epoch(StackDomainRef::Interrupted)],
            },
            ArrivalContextRealization {
                context: ArrivalContextId::new(2).expect("arrival context"),
                epochs: vec![body_epoch(StackDomainRef::Dedicated { class: 3 })],
            },
        ],
    })
    .expect("context-specific body domains are structurally closed");
    let context_evidence = admitted_arrival_contexts(
        &summary,
        &boundary,
        &code,
        entry,
        &[2, 1],
        root_id(0x826, StackValidationReceiptId::from_normalized_identity),
    );

    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        realization,
        context_evidence,
    )
    .expect("provider-selected stack may close differently per arrival context");
    assert_eq!(
        bound.realization_evidence().body_domains(),
        [
            (
                ArrivalContextId::new(1).expect("arrival context"),
                StackDomainRef::Interrupted,
            ),
            (
                ArrivalContextId::new(2).expect("arrival context"),
                StackDomainRef::Dedicated { class: 3 },
            ),
        ]
    );
}

#[test]
fn terminal_root_service_reach_closes_without_frontend_handles_or_bound_subtraction() {
    let selected = selected_interrupt_completion();
    let timer = psi_core::ServiceId::new(1).expect("service identity");
    let machine_control = psi_core::ServiceId::new(2).expect("service identity");
    let port_io = psi_core::ServiceId::new(3).expect("service identity");
    let services = vec![
        ServiceDeclaration {
            id: timer,
            identity: "Timer".into(),
            parents: Vec::new(),
        },
        ServiceDeclaration {
            id: machine_control,
            identity: "MachineControl".into(),
            parents: Vec::new(),
        },
        ServiceDeclaration {
            id: port_io,
            identity: "PortIo".into(),
            parents: Vec::new(),
        },
    ];
    let closure = TerminalRootServiceReach {
        concrete: vec![timer],
        installation_dependencies: vec![InstallationReachDependency {
            requirement_identity: "InterruptCompletion::complete".into(),
            upper_bound: vec![machine_control, port_io],
        }],
    };
    let reach = ResolvedRootServiceReach::from_root_service_reach(&closure, &services, &selected)
        .expect("terminal closure resolves at final admission");
    assert_eq!(reach.effective(), ["PortIo", "Timer"]);

    let drifted = TerminalRootServiceReach {
        installation_dependencies: vec![InstallationReachDependency {
            requirement_identity: "InterruptCompletion::complete".into(),
            upper_bound: vec![port_io],
        }],
        ..closure
    };
    let error = ResolvedRootServiceReach::from_root_service_reach(&drifted, &services, &selected)
        .expect_err("terminal and selected provider bounds must agree exactly");
    assert!(error.0.contains("changed its published upper bound"));
}

fn slot() -> RootSlotAuthority {
    RootSlotAuthority::from_admitted_owner(
        root_id(20, RootSlotId::from_normalized_identity),
        root_id(21, RootSlotOwnerId::from_normalized_identity),
    )
}

pub(crate) fn provider_execution(root: &ValidatedExternalRoot) -> ProviderExecution {
    ProviderExecution::from_admitted_provider(
        root_id(54, ProviderExecutionId::from_normalized_identity),
        root,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: root.boundary().call.entry_control,
                restored_state: root.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("admitted provider exit")
}

fn entry_writer(entry: EntryStubId) -> PostHandoffWriterPlan {
    let target = RelocationTarget::Entry(entry);
    PostHandoffWriterPlan {
        byte_len: 16,
        byte_order: ByteOrder::LittleEndian,
        placement: constraints(),
        steps: vec![PostHandoffWriterStep {
            write: MaterializationWrite {
                field: "entry".into(),
                target,
                container_byte_offset: 0,
                container_width_bits: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 64,
                stored_integer_fit: None,
            },
            source: PostHandoffWriterSource::Resolve(target),
        }],
    }
}

fn writer_site(base_address: u64) -> PlacementSite {
    PlacementSite {
        base_address,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: Some(
            ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope"),
        ),
    }
}

pub(crate) fn install_test_root<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    install_test_root_with_ids(code, entry, 1, 20, 21, 22, Vec::new())
}

fn install_test_root_with_ids<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    root_identity: u64,
    slot_identity: u64,
    owner_identity: u64,
    admission_identity: u64,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    let mut ledger = InstalledRootLedger::claim(code).expect("canonical root ledger");
    let installed = install_test_root_in_ledger(
        &mut ledger,
        code,
        entry,
        root_identity,
        slot_identity,
        owner_identity,
        admission_identity,
        entry_claims,
    );
    (ledger, installed)
}

#[allow(clippy::too_many_arguments)]
fn install_test_root_in_ledger<'code>(
    ledger: &mut InstalledRootLedger,
    code: &'code InstalledCode,
    entry: EntryStubId,
    root_identity: u64,
    slot_identity: u64,
    owner_identity: u64,
    admission_identity: u64,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> InstalledExternalRoot<'code> {
    let mut candidate = candidate_for_code_with_root(entry, code, root_identity);
    candidate.entry_claims = entry_claims;
    let validated = validate_external_root(candidate, &boundary()).expect("root plan");
    let authority = RootSlotAuthority::from_admitted_owner(
        root_id(slot_identity, RootSlotId::from_normalized_identity),
        root_id(owner_identity, RootSlotOwnerId::from_normalized_identity),
    );
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(
            admission_identity,
            RootAdmissionId::from_normalized_identity,
        ),
        &validated,
        &execution,
        code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    ledger
        .install(code, validated, authority, admission)
        .expect("installed external root")
}

#[allow(clippy::too_many_arguments)]
fn install_test_root_pair_with_ids_unsealed<'code>(
    code: &'code mut InstalledCode,
    first: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    second: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    entry: EntryStubId,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    let mut first_candidate = candidate_for_code_with_root(entry, code, first.0);
    first_candidate.entry_claims = first.4;
    let mut second_candidate = candidate_for_code_with_root(entry, code, second.0);
    second_candidate.entry_claims = second.4;
    let first_input = first_candidate
        .stack
        .realization
        .input(first_candidate.identity)
        .expect("first root stack input")
        .clone();
    let second_input = second_candidate
        .stack
        .realization
        .input(second_candidate.identity)
        .expect("second root stack input")
        .clone();
    let relation = StackNestingRelation {
        identity: first_candidate.nesting_relation,
        edges: BTreeSet::new(),
    };
    let composition = compose_bound_entry_stack_epochs(&relation, [&first_input, &second_input])
        .expect("artifact-wide two-root stack composition");
    first_candidate.stack.realization = composition.clone();
    second_candidate.stack.realization = composition;

    let boundary = boundary();
    let first_validated =
        validate_external_root(first_candidate, &boundary).expect("first root plan");
    let second_validated =
        validate_external_root(second_candidate, &boundary).expect("second root plan");
    let target_profile = omega_target::TargetProfile::UefiX64;
    let target_slot = target_profile.program_entry_slot();
    let first_slot = RootSlotAuthority::for_target_program_entry(target_slot)
        .expect("target program-entry authority");
    let second_slot = RootSlotAuthority::from_admitted_owner(
        root_id(second.1, RootSlotId::from_normalized_identity),
        root_id(second.2, RootSlotOwnerId::from_normalized_identity),
    );
    let first_execution = provider_execution(&first_validated);
    let second_execution = provider_execution(&second_validated);
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(first.3, RootAdmissionId::from_normalized_identity),
        &first_validated,
        &first_execution,
        code,
        &first_slot,
        first_validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("first root admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(second.3, RootAdmissionId::from_normalized_identity),
        &second_validated,
        &second_execution,
        code,
        &second_slot,
        second_validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("second root admission");
    let mut ledger = InstalledRootLedger::claim(code).expect("canonical two-root ledger");
    let first_installed = ledger
        .install(code, first_validated, first_slot, first_admission)
        .expect("first installed root");
    let second_installed = ledger
        .install(code, second_validated, second_slot, second_admission)
        .expect("second installed root");
    (ledger, first_installed, second_installed)
}

#[allow(clippy::too_many_arguments)]
fn install_test_root_pair_with_ids<'code>(
    code: &'code mut InstalledCode,
    first: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    second: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    entry: EntryStubId,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    let (mut ledger, first_installed, second_installed) =
        install_test_root_pair_with_ids_unsealed(code, first, second, entry);
    ledger
        .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
        .expect("installed required root-slot closure");
    (ledger, first_installed, second_installed)
}

fn program_local_required_root_slot_closure(entry: EntryStubId) -> VerifiedRequiredRootSlotClosure {
    let target_profile = omega_target::TargetProfile::UefiX64;
    verify_target_required_root_slot_closure(
        target_profile,
        [TargetRequiredRootSlotSelection::for_program_entry(
            target_profile.program_entry_slot(),
            entry,
            "TestRoot::entry",
        )
        .expect("required program-entry selection")],
    )
    .expect("complete required root-slot closure")
}

fn install_program_local_required_root<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    install_test_root_pair_with_ids(
        code,
        (1, 20, 21, 22, entry_claims.clone()),
        (101, 120, 121, 122, entry_claims),
        entry,
    )
}

fn program_local_root_module() -> TerminalModule {
    let entry = psi_core::MachineId::new(1).expect("machine identity");
    let carrier = psi_core::StructuralTypeId::new(1).expect("carrier identity");
    let qualification = psi_core::StructuralDomainId::new(1).expect("domain identity");
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::CountedQuantity,
        parameter: "ByteUnit".into(),
    };
    let capacity = psi_core::ContentProjectionExpression::CountedQuantity(
        psi_core::ContentProjectionScalar::Add(
            Box::new(psi_core::ContentProjectionScalar::SubjectField(vec![
                "length".into(),
            ])),
            Box::new(psi_core::ContentProjectionScalar::Natural("1".into())),
        ),
    );
    let mut schema = psi_terminal::ProgramLocalRootIntroductionSchema {
        argument_index: 0,
        source_parameter_position: 0,
        qualification,
        carrier,
        projection: psi_core::ContentProjectionIdentity {
            domain: psi_core::ContentDomainId::new(1).expect("content domain identity"),
            projection_report_fingerprint:
                psi_language_semantics::content::terminal_projection_report_fingerprint(
                    &algebra, &capacity,
                ),
        },
        algebra,
        capacity,
        compatibility_report_identity: 0,
    };
    schema.compatibility_report_identity =
        program_local_root_introduction_compatibility_report_identity(
            "TestRoot::entry",
            "Region::Owned",
            "Region",
            &schema,
        );
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry,
        structural_types: vec![StructuralTypeDeclaration {
            id: carrier,
            identity: "Region".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: psi_core::StructuralFieldId::new(1).expect("field identity"),
                    identity: "length".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(psi_core::ScalarType::Integer(
                        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64)
                            .expect("u64 type"),
                    )),
                }],
            },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: qualification,
            semantic_domain: psi_core::DomainSemanticId::new(1).expect("semantic domain identity"),
            identity: "Region::Owned".into(),
            carrier,
            content_projection: Some(StructuralContentProjection {
                identity: schema.projection,
                algebra: schema.algebra.clone(),
                expression: schema.capacity.clone(),
            }),
        }],
        services: Vec::new(),
        root_service_reach: TerminalRootServiceReach::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: psi_core::BoundaryMachineId::new(1).expect("boundary identity"),
            identity: "TestRoot::entry".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: psi_core::PlaceId::new(1).expect("place identity"),
                position: 0,
                is_self: false,
                structural_type: carrier,
                multiplicity: StructuralMultiplicity::Linear,
                access: psi_terminal::StructuralAccess::Owned,
                qualifications: vec![qualification],
                projected_qualifications: Vec::new(),
            }],
            result: None,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain: qualification,
            }],
            program_local_root_introductions: vec![schema],
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        quotient_correspondences: Vec::new(),
        machines: vec![psi_terminal::TerminalMachine {
            id: entry,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: psi_terminal::TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: psi_core::BlockId::new(1).expect("block identity"),
            blocks: vec![psi_terminal::Block {
                id: psi_core::BlockId::new(1).expect("block identity"),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: psi_terminal::Terminator::ReturnUnit {
                    edge: psi_core::EdgeId::new(1).expect("edge identity"),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: psi_terminal::MachineContract {
                id: psi_core::ContractId::new(1).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn program_local_extent_module() -> TerminalModule {
    let mut module = program_local_root_module();
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::IntervalSet,
        parameter: "Nat".into(),
    };
    let capacity = psi_core::ContentProjectionExpression::IntervalSet(vec![(
        psi_core::ContentProjectionScalar::SubjectField(vec!["base".into()]),
        psi_core::ContentProjectionScalar::Add(
            Box::new(psi_core::ContentProjectionScalar::SubjectField(vec![
                "base".into(),
            ])),
            Box::new(psi_core::ContentProjectionScalar::SubjectField(vec![
                "length".into(),
            ])),
        ),
    )]);
    let schema = &mut module.boundary_machines[0].program_local_root_introductions[0];
    schema.algebra = algebra;
    schema.capacity = capacity;
    schema.projection.projection_report_fingerprint =
        psi_language_semantics::content::terminal_projection_report_fingerprint(
            &schema.algebra,
            &schema.capacity,
        );
    schema.compatibility_report_identity =
        program_local_root_introduction_compatibility_report_identity(
            "TestRoot::entry",
            "Region::Owned",
            "Region",
            schema,
        );
    module.structural_domains[0].content_projection = Some(StructuralContentProjection {
        identity: schema.projection,
        algebra: schema.algebra.clone(),
        expression: schema.capacity.clone(),
    });
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &mut module.structural_types[0].shape
    else {
        panic!("program-local test carrier is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(2).expect("base field identity"),
        identity: "base".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(psi_core::ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).expect("u64 type"),
        )),
    });
    module
}

fn program_local_terminal_object(module: &TerminalModule) -> TestObject {
    TestObject {
        identity: psi_terminal_codec::terminal_psi_identity(module)
            .expect("terminal program identity"),
        entry: module.entry,
        bytes: vec![0; 64],
    }
}

fn program_local_root_catalog(
    module: &TerminalModule,
) -> psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog {
    let proof = psi_terminal_verifier::ProofBundle::default();
    let verified =
        psi_terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default())
            .expect("program-local terminal module verifies");
    psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog::from_verified(&verified)
        .expect("verified program-local producer catalog")
}

fn program_local_claim() -> ExternalRootEntryClaim {
    ExternalRootEntryClaim {
        parameter_index: 0,
        domain: "Region::Owned".into(),
        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
    }
}

fn program_local_tcb_acceptance(seed: u64) -> ExecutableTcbProfileAcceptance {
    evaluate_executable_tcb_profile(
        &ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_report_identity: seed,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        },
        &ExecutableTcbProfile {
            name: format!("program-local-era-{seed}"),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        },
    )
    .expect("component-era TCB profile acceptance")
}

fn program_local_lifecycle(
    ledger_identity: u64,
    era_identity: u64,
    artifact_occurrence_digest: omega_installation_evidence::InstalledArtifactOccurrenceDigest,
    artifact_instance_compatibility_report_identity: u64,
    entry_contract_identity: &str,
) -> ComponentEraEntryLedger {
    let mut ledger = ComponentEraEntryLedger::new(
        ComponentEraLedgerId::from_normalized_identity(ledger_identity)
            .expect("component-era ledger identity"),
        "TestRootBinding/v1".into(),
        entry_contract_identity.into(),
        2,
        program_local_tcb_acceptance(ledger_identity),
    )
    .expect("component-era ledger");
    publish_program_local_era(
        &mut ledger,
        era_identity,
        artifact_occurrence_digest,
        artifact_instance_compatibility_report_identity,
        entry_contract_identity,
        era_identity + 100,
        false,
    );
    ledger
}

fn publish_program_local_era(
    ledger: &mut ComponentEraEntryLedger,
    era_identity: u64,
    artifact_occurrence_digest: omega_installation_evidence::InstalledArtifactOccurrenceDigest,
    artifact_instance_compatibility_report_identity: u64,
    entry_contract_identity: &str,
    publication_identity: u64,
    previous_era_closed: bool,
) {
    let candidate = ComponentEraCandidate {
        era_identity,
        artifact_occurrence_digest,
        artifact_instance_compatibility_report_identity,
        binding_contract_identity: "TestRootBinding/v1".into(),
        entry_contract_identity: entry_contract_identity.into(),
        entry_plan_identity: format!("entry-plan:{era_identity}"),
        entry_plan_admission_receipt_identity: format!("entry-plan-receipt:{era_identity}"),
        executable_tcb_acceptance: program_local_tcb_acceptance(era_identity),
    };
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        publication_identity,
        ledger,
        &candidate,
        true,
        previous_era_closed,
    );
    ledger
        .publish(candidate, receipt)
        .expect("publish component era");
}

fn program_local_epoch_lease(
    ledger: &mut ComponentEraEntryLedger,
    lease_identity: u64,
    era_identity: u64,
    entry_contract_identity: &str,
) -> omega_effects::ProgramLocalRootEpochLease {
    ledger
        .acquire_program_local_root_epoch_lease(
            ProgramLocalRootEpochLeaseId::from_normalized_identity(lease_identity)
                .expect("program-local epoch lease identity"),
            era_identity,
            entry_contract_identity,
        )
        .expect("program-local epoch lease")
}

fn program_local_subject<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    invocation: u64,
    subject_place: u64,
    length: Option<u64>,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    let scalars = length
        .into_iter()
        .map(|length| {
            ProgramLocalRootScalarBinding::subject_field(
                ["length"],
                psi_numerics::bignum::BigInt::from_u64(length),
            )
            .expect("natural subject field")
        })
        .collect::<Vec<_>>();
    InstalledProgramLocalRootSubject::from_generated_entry(
        root,
        ProgramLocalRootEntryInvocationId::from_normalized_identity(invocation)
            .expect("entry invocation identity"),
        0,
        0,
        "Region::Owned",
        "Region",
        ProgramLocalRootSubjectPlaceId::from_normalized_identity(subject_place)
            .expect("subject place identity"),
        scalars,
    )
    .expect("exact installed program-local subject")
}

fn program_local_extent_subject<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    invocation: u64,
    subject_place: u64,
    base: u64,
    length: u64,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    InstalledProgramLocalRootSubject::from_generated_entry(
        root,
        ProgramLocalRootEntryInvocationId::from_normalized_identity(invocation)
            .expect("entry invocation identity"),
        0,
        0,
        "Region::Owned",
        "Region",
        ProgramLocalRootSubjectPlaceId::from_normalized_identity(subject_place)
            .expect("subject place identity"),
        [
            ProgramLocalRootScalarBinding::subject_field(
                ["base"],
                psi_numerics::bignum::BigInt::from_u64(base),
            )
            .expect("natural base field"),
            ProgramLocalRootScalarBinding::subject_field(
                ["length"],
                psi_numerics::bignum::BigInt::from_u64(length),
            )
            .expect("natural length field"),
        ],
    )
    .expect("exact installed program-local Extent subject")
}

fn join_program_local<'root, 'code>(
    installation: &mut ProgramLocalRootInstallationLedger,
    prebinding: ProgramLocalRootPrebindingId,
    root: &'root InstalledExternalRoot<'code>,
    lifecycle: &mut ComponentEraEntryLedger,
    lease_identity: u64,
    era_identity: u64,
    entry_contract_identity: &str,
) -> Result<
    InstalledProgramLocalRootOccurrence<'root, 'code>,
    Box<ProgramLocalRootCohortSealError<'root, 'code>>,
> {
    let lease = program_local_epoch_lease(
        lifecycle,
        lease_identity,
        era_identity,
        entry_contract_identity,
    );
    let cohort = installation.seal_epoch_cohort(
        lifecycle,
        [ProgramLocalRootCohortMember::new(prebinding, root, lease)],
    )?;
    let [occurrence]: [InstalledProgramLocalRootOccurrence<'root, 'code>; 1] = cohort
        .into_runtime()
        .cancel()
        .try_into()
        .expect("single-prebinding test cohort");
    Ok(occurrence)
}

fn sole_rejected_cohort_lease(
    error: ProgramLocalRootCohortSealError<'_, '_>,
) -> omega_effects::ProgramLocalRootEpochLease {
    let [member]: [ProgramLocalRootCohortMember<'_, '_>; 1] = error
        .into_members()
        .try_into()
        .expect("single-member rejected cohort");
    member.into_parts().2
}

#[test]
fn installed_required_root_closure_owns_one_cohort_verifier_and_freezes_members() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (mut root_ledger, required_root, open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);

    let installed_closure = root_ledger
        .required_root_slots()
        .expect("required root-slot closure retained in the installation");
    assert_eq!(installed_closure.slots().len(), 1);
    assert_eq!(
        installed_closure
            .slots()
            .next()
            .expect("required member")
            .root(),
        required_root.root()
    );
    assert!(
        root_ledger
            .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
            .expect_err("installed closure sealing is one-shot")
            .0
            .contains("already sealed")
    );

    let cohort_verifier = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("one cohort verifier");
    drop(cohort_verifier);
    assert!(
        root_ledger
            .claim_program_local_root_installation_ledger()
            .expect_err("dropping the verifier never reopens issuance")
            .0
            .contains("already issued")
    );

    let open_receipt = RootRemovalReceipt::from_provider(
        root_id(900, RootRemovalReceiptId::from_normalized_identity),
        &open_root,
        true,
        true,
    );
    root_ledger
        .remove(open_root, open_receipt)
        .expect("an unrelated runtime-open root is not frozen");
    let required_receipt = RootRemovalReceipt::from_provider(
        root_id(901, RootRemovalReceiptId::from_normalized_identity),
        &required_root,
        true,
        true,
    );
    let removal = root_ledger
        .remove(required_root, required_receipt)
        .expect_err("a sealed required root remains frozen");
    assert!(
        removal
            .diagnostic()
            .0
            .contains("keeps that installed root frozen")
    );
}

#[test]
fn complete_installed_root_teardown_preserves_history_and_returns_slots_in_caller_order() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (root_ledger, required_root, open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let required_slot = required_root.slot();
    let open_slot = open_root.slot();
    let required_receipt = RootRemovalReceipt::from_provider(
        root_id(900, RootRemovalReceiptId::from_normalized_identity),
        &required_root,
        true,
        true,
    );
    let open_receipt = RootRemovalReceipt::from_provider(
        root_id(901, RootRemovalReceiptId::from_normalized_identity),
        &open_root,
        true,
        true,
    );

    let (root_ledger, returned_slots) = root_ledger
        .teardown_installed_roots(vec![
            InstalledRootRemoval::new(open_root, open_receipt),
            InstalledRootRemoval::new(required_root, required_receipt),
        ])
        .expect("complete teardown may retire a sealed required-root cohort");

    assert!(root_ledger.live_external_roots_are_empty());
    assert_eq!(returned_slots.len(), 2);
    assert_eq!(returned_slots[0].slot(), open_slot);
    assert_eq!(returned_slots[1].slot(), required_slot);
    assert!(root_ledger.required_root_slots().is_some());
    assert!(root_ledger.binds_installed_code(&code));
    drop(code);
}

#[test]
fn installed_root_teardown_is_atomic_and_returns_exact_rows_for_retry() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (root_ledger, first, second) = install_test_root_pair_with_ids_unsealed(
        &mut code,
        (1, 20, 21, 22, Vec::new()),
        (101, 120, 121, 122, Vec::new()),
        entry,
    );
    let first_receipt = RootRemovalReceipt::from_provider(
        root_id(900, RootRemovalReceiptId::from_normalized_identity),
        &first,
        true,
        true,
    );
    let second_receipt = RootRemovalReceipt::from_provider(
        root_id(901, RootRemovalReceiptId::from_normalized_identity),
        &second,
        true,
        false,
    );
    let error = root_ledger
        .teardown_installed_roots(vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ])
        .expect_err("a late non-quiesced row rejects the whole teardown");
    assert!(error.diagnostic().0.contains("quiesced"));
    let (root_ledger, mut rows) = error.into_parts();
    assert_eq!(root_ledger.records().count(), 2);
    assert!(!root_ledger.live_external_roots_are_empty());
    rows[1].receipt.executions_quiesced = true;

    let (root_ledger, returned_slots) = root_ledger
        .teardown_installed_roots(rows)
        .expect("returned exact custody retries successfully");
    assert!(root_ledger.live_external_roots_are_empty());
    assert_eq!(returned_slots.len(), 2);
}

#[test]
fn installed_root_teardown_preflight_rejects_incomplete_ambiguous_and_stale_state() {
    let entry = entry_id(1);

    {
        let mut code = installed_code(1, entry);
        let (ledger, first, _second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let rows = vec![InstalledRootRemoval::new(first, receipt)];
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("missing row")
                .0
                .contains("completely cover")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        let duplicate = rows[0].root.root;
        rows[1].root.root = duplicate;
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("duplicate row")
                .0
                .contains("duplicate")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        rows[1].root.root = root_id(999, ExternalRootId::from_normalized_identity);
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("extra row")
                .0
                .contains("outside the live ledger")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let other_code = installed_code_with_fill_and_installation_identity(1, entry, 1, 300);
        let (ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        rows[1].root.installed_code = &other_code;
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("wrong full code context")
                .0
                .contains("different installed-code occurrence")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (mut ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            false,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("reachable entry")
                .0
                .contains("unreachable")
        );
        ledger.active_interrupts.insert((
            rows[0].root.root,
            root_id(999, InterruptInvocationId::from_normalized_identity),
        ));
        rows[0].receipt.entry_unreachable = true;
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("active interrupt")
                .0
                .contains("active interrupt")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (mut ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        let stale = ledger
            .root_evidence
            .values()
            .next()
            .expect("live evidence")
            .clone();
        ledger.root_evidence.insert(
            root_id(999, ExternalRootId::from_normalized_identity),
            stale,
        );
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("stale evidence")
                .0
                .contains("exact live root, evidence, and slot state")
        );
    }
}

#[test]
fn failed_installed_required_root_replay_is_transactional() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (mut root_ledger, _required_root, _open_root) = install_test_root_pair_with_ids_unsealed(
        &mut code,
        (1, 20, 21, 22, vec![program_local_claim()]),
        (101, 120, 121, 122, vec![program_local_claim()]),
        entry,
    );
    let profile = omega_target::TargetProfile::UefiX64;
    let wrong = verify_target_required_root_slot_closure(
        profile,
        [TargetRequiredRootSlotSelection::for_program_entry(
            profile.program_entry_slot(),
            entry,
            "OtherRoot::entry",
        )
        .expect("wrong requirement selection remains descriptive")],
    )
    .expect("wrong requirement still forms a target closure");
    assert!(
        root_ledger
            .seal_required_root_slot_closure(wrong)
            .expect_err("installed requirement substitution rejects")
            .0
            .contains("does not match the exact required slot")
    );
    assert!(root_ledger.required_root_slots().is_none());
    root_ledger
        .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
        .expect("failed replay leaves exact closure sealable");
}

#[test]
fn program_local_cohort_verifier_requires_an_installed_required_closure() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (mut root_ledger, _root) = install_test_root(&mut code, entry);
    assert!(
        root_ledger
            .claim_program_local_root_installation_ledger()
            .expect_err("an orphan root ledger has no program-local cohort")
            .0
            .contains("requires a sealed required root-slot closure")
    );
}

#[test]
fn epoch_cohort_seals_exact_members_and_derives_aggregate_schema() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("required root prebinding")
        .try_into()
        .expect("one producer schema");
    let prebinding = prebinding.identity();
    let mut lifecycle = program_local_lifecycle(
        730,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );

    let omitted = installation
        .seal_epoch_cohort(&lifecycle, std::iter::empty())
        .expect_err("omitting the eligible prebinding rejects");
    assert!(omitted.diagnostic().0.contains("omits or adds"));
    assert!(omitted.into_members().is_empty());

    let duplicate_lease_a = program_local_epoch_lease(&mut lifecycle, 830, 10, "TestRoot::entry");
    let duplicate_lease_b = program_local_epoch_lease(&mut lifecycle, 831, 10, "TestRoot::entry");
    let duplicate = installation
        .seal_epoch_cohort(
            &lifecycle,
            [
                ProgramLocalRootCohortMember::new(prebinding, &root, duplicate_lease_a),
                ProgramLocalRootCohortMember::new(prebinding, &root, duplicate_lease_b),
            ],
        )
        .expect_err("duplicate cohort members reject transactionally");
    assert!(duplicate.diagnostic().0.contains("repeats one prebinding"));
    for member in duplicate.into_members() {
        lifecycle
            .release_program_local_root_epoch_lease(member.into_parts().2)
            .expect("duplicate rejection returns every lease");
    }

    let lease = program_local_epoch_lease(&mut lifecycle, 832, 10, "TestRoot::entry");
    let cohort = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(prebinding, &root, lease)],
        )
        .expect("exact epoch cohort");
    assert_eq!(cohort.identity().lifecycle_epoch(), 10);
    assert_eq!(cohort.installed_required_slots().slots().len(), 1);
    assert_eq!(cohort.occurrences().len(), 1);
    let aggregates = cohort.aggregates().collect::<Vec<_>>();
    let [aggregate] = aggregates.as_slice() else {
        panic!("one closed aggregate schema")
    };
    assert_eq!(aggregate.cardinality().get(), 1);
    assert_eq!(
        aggregate.per_occurrence_capacity(),
        &module.boundary_machines[0].program_local_root_introductions[0].capacity
    );
    let snapshot = cohort.aggregate_snapshot();
    assert_eq!(snapshot.identity(), cohort.identity());
    assert_eq!(
        snapshot.installed_required_slots(),
        cohort.installed_required_slots()
    );
    let snapshot_rows = snapshot.aggregates().collect::<Vec<_>>();
    let [snapshot_row] = snapshot_rows.as_slice() else {
        panic!("one snapshotted aggregate row")
    };
    assert_eq!(snapshot_row, aggregate);
    assert_eq!(snapshot_row.cardinality().get(), 1);
    assert_eq!(
        snapshot_row.per_occurrence_capacity(),
        &module.boundary_machines[0].program_local_root_introductions[0].capacity
    );
    let cloned_snapshot = snapshot.clone();
    assert_eq!(cloned_snapshot, snapshot);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    assert!(
        installation
            .prebind(&catalog, &terminal, &root)
            .expect_err("sealing freezes the eligible prebinding set")
            .0
            .contains("prebindings are frozen")
    );
    let runtime = cohort.into_runtime();
    assert_eq!(runtime.aggregate_snapshot(), snapshot);
    assert_eq!(runtime.pending_occurrences().len(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    let [occurrence]: [InstalledProgramLocalRootOccurrence<'_, '_>; 1] =
        runtime.cancel().try_into().expect("one cohort occurrence");
    installation
        .retire(occurrence, &mut lifecycle)
        .expect("sealed occurrence remains retireable");
}

#[test]
fn coexistence_report_requires_every_exact_live_epoch_without_reducing_rows() {
    let entry = entry_id(1);
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);

    let mut first_code = installed_code_with_fill_and_installation_identity(1, entry, 0, 300);
    let first_code_identity = first_code.identity().normalized_identity();
    let (mut first_root_ledger, first_root, _first_open_root) =
        install_program_local_required_root(&mut first_code, entry, vec![program_local_claim()]);
    let mut first_installation = first_root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("first program-local cohort verifier");
    let [first_prebinding] = first_installation
        .prebind(&catalog, &terminal, &first_root)
        .expect("first required root prebinding")
        .try_into()
        .expect("one first producer schema");

    let mut lifecycle = program_local_lifecycle(
        760,
        10,
        first_root.installed_artifact_occurrence_digest(),
        first_code_identity,
        "TestRoot::entry",
    );
    let first_lease = program_local_epoch_lease(&mut lifecycle, 860, 10, "TestRoot::entry");
    let first_cohort = first_installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                first_prebinding.identity(),
                &first_root,
                first_lease,
            )],
        )
        .expect("first exact epoch cohort");
    let first_snapshot = first_cohort.aggregate_snapshot();

    let mut second_code = installed_code_with_fill_and_installation_identity(2, entry, 0, 301);
    let second_code_identity = second_code.identity().normalized_identity();
    publish_program_local_era(
        &mut lifecycle,
        20,
        second_code.occurrence_digest(),
        second_code_identity,
        "TestRoot::entry",
        120,
        true,
    );
    let (mut second_root_ledger, second_root, _second_open_root) =
        install_program_local_required_root(&mut second_code, entry, vec![program_local_claim()]);
    let mut second_installation = second_root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("second program-local cohort verifier");
    let [second_prebinding] = second_installation
        .prebind(&catalog, &terminal, &second_root)
        .expect("second required root prebinding")
        .try_into()
        .expect("one second producer schema");
    let second_lease = program_local_epoch_lease(&mut lifecycle, 861, 20, "TestRoot::entry");
    let second_cohort = second_installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                second_prebinding.identity(),
                &second_root,
                second_lease,
            )],
        )
        .expect("second exact epoch cohort");
    let second_snapshot = second_cohort.aggregate_snapshot();

    let missing =
        compose_program_local_root_coexistence_report(&lifecycle, std::iter::once(&first_snapshot))
            .expect_err("one of two live epochs cannot be omitted");
    assert!(missing.0.contains("omits or adds"));

    let duplicate = compose_program_local_root_coexistence_report(
        &lifecycle,
        [&first_snapshot, &first_snapshot, &second_snapshot],
    )
    .expect_err("one live epoch cannot be repeated");
    assert!(duplicate.0.contains("repeats one lifecycle epoch"));

    let report = compose_program_local_root_coexistence_report(
        &lifecycle,
        [&second_snapshot, &first_snapshot],
    )
    .expect("both exact coexisting epochs compose");
    assert_eq!(report.lifecycle_ledger(), lifecycle.identity());
    assert_eq!(
        report
            .epoch_snapshots()
            .map(|snapshot| snapshot.identity().lifecycle_epoch())
            .collect::<Vec<_>>(),
        vec![10, 20]
    );
    assert_eq!(
        report
            .aggregates()
            .map(|(epoch, aggregate)| (epoch, aggregate.cardinality().get()))
            .collect::<Vec<_>>(),
        vec![(10, 1), (20, 1)]
    );
    assert!(report.aggregates().all(|(_, aggregate)| {
        aggregate.per_occurrence_capacity()
            == &module.boundary_machines[0].program_local_root_introductions[0].capacity
    }));

    let other_lifecycle = program_local_lifecycle(
        761,
        10,
        first_root.installed_artifact_occurrence_digest(),
        first_code_identity,
        "TestRoot::entry",
    );
    let substituted = compose_program_local_root_coexistence_report(
        &other_lifecycle,
        std::iter::once(&first_snapshot),
    )
    .expect_err("a snapshot from another lifecycle ledger rejects");
    assert!(substituted.0.contains("another lifecycle ledger"));

    let later_only_lifecycle = program_local_lifecycle(
        760,
        20,
        second_root.installed_artifact_occurrence_digest(),
        second_code_identity,
        "TestRoot::entry",
    );
    let stale = compose_program_local_root_coexistence_report(
        &later_only_lifecycle,
        std::iter::once(&first_snapshot),
    )
    .expect_err("an old snapshot cannot stand in for the current live epoch");
    assert!(stale.0.contains("non-live lifecycle epoch"));

    let [first_occurrence]: [InstalledProgramLocalRootOccurrence<'_, '_>; 1] = first_cohort
        .into_runtime()
        .cancel()
        .try_into()
        .expect("one first occurrence");
    first_installation
        .retire(first_occurrence, &mut lifecycle)
        .expect("first coexistence occurrence remains retireable");
    let [second_occurrence]: [InstalledProgramLocalRootOccurrence<'_, '_>; 1] = second_cohort
        .into_runtime()
        .cancel()
        .try_into()
        .expect("one second occurrence");
    second_installation
        .retire(second_occurrence, &mut lifecycle)
        .expect("second coexistence occurrence remains retireable");
}

#[test]
fn installed_subject_establishes_exact_capacity_lineage_once_and_pins_the_epoch() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("verified installed prebinding")
        .try_into()
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        740,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 840, 10, "TestRoot::entry");
    let cohort = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("exact epoch cohort");
    let mut runtime = cohort.into_runtime();

    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            program_local_subject(&root, 940, 1040, Some(8)),
        )
        .expect("exact installed subject establishes its root");
    assert_eq!(
        established
            .capacity()
            .counted_quantity()
            .expect("counted capacity"),
        &psi_numerics::bignum::BigInt::from_u64(9)
    );
    assert_eq!(
        established.lineage().occurrence(),
        established.occurrence_identity()
    );
    assert_eq!(established.prebinding().argument_index(), 0);
    assert_eq!(established.scalar_observations().len(), 1);
    assert_eq!(runtime.pending_occurrences().len(), 0);
    assert_eq!(runtime.aggregates().len(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    let replay = installation
        .establish(
            &mut runtime,
            &lifecycle,
            program_local_subject(&root, 941, 1041, Some(8)),
        )
        .expect_err("the exact installed occurrence establishes at most once");
    assert!(replay.diagnostic().0.contains("no pending exact cohort"));
    assert_eq!(
        replay.into_subject().invocation().normalized_identity(),
        941
    );

    let mut substituted = program_local_lifecycle(
        741,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let retirement = installation
        .retire_established(established, &mut substituted)
        .expect_err("a foreign lifecycle cannot retire the established root");
    assert!(retirement.diagnostic().0.contains("lifecycle lease"));
    let established = (*retirement).into_root();
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    installation
        .retire_established(established, &mut lifecycle)
        .expect("the exact lifecycle retires the root");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn program_local_extent_registry_retains_exact_account_through_split_and_retirement() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_extent_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("verified installed Extent prebinding")
        .try_into()
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        780,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 880, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("exact Extent epoch cohort")
        .into_runtime();
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            program_local_extent_subject(&root, 980, 1080, 0x4000, 0x100),
        )
        .expect("exact interval subject establishes its root");
    let plan = ProgramLocalExtentMaterializationPlan::new(
        "Region",
        "Region::Owned",
        "Nat",
        0x4000,
        0x100,
        extent_id(10, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([
            extent_id(100, ExtentRightId::from_normalized_identity),
            extent_id(101, ExtentRightId::from_normalized_identity),
        ]),
        extent_id(20, ExtentProvenanceId::from_normalized_identity),
        extent_id(30, MappingEraId::from_normalized_identity),
    )
    .expect("checked Extent materialization plan");
    let mut registry = ProgramLocalExtentRegistry::new();
    let extent = registry
        .materialize(established, plan)
        .expect("established interval materializes one Extent");
    let origin = extent
        .program_local_origin()
        .expect("passive program-local origin");
    assert_eq!(origin.installed_code(), code_identity);
    assert_eq!(origin.lifecycle_ledger(), 780);
    assert_eq!(origin.lifecycle_epoch(), 10);
    assert_eq!(origin.entry_invocation(), 980);
    assert_eq!(origin.subject_place(), 1080);
    assert_eq!(registry.held_accounts(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    let (lower, upper) = extent.split_at(0x40).expect("split program-local Extent");
    let rejected = registry
        .retire(lower, &mut installation, &mut lifecycle)
        .expect_err("a split descendant cannot retire the account");
    assert!(rejected.diagnostic().0.contains("recombined root"));
    let lower = (*rejected).into_extent();
    assert_eq!(registry.held_accounts(), 1);
    let extent = lower.merge(upper).expect("recombine exact root Extent");

    let mut substituted = program_local_lifecycle(
        781,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let rejected = registry
        .retire(extent, &mut installation, &mut substituted)
        .expect_err("a foreign lifecycle cannot release the retained occurrence");
    assert!(rejected.diagnostic().0.contains("lifecycle lease"));
    let extent = (*rejected).into_extent();
    assert_eq!(registry.held_accounts(), 1);
    registry
        .retire(extent, &mut installation, &mut lifecycle)
        .expect("exact recombined root releases its lifecycle account");
    assert_eq!(registry.held_accounts(), 0);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn counted_program_local_capacity_cannot_mint_an_extent() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("verified installed counted prebinding")
        .try_into()
        .expect("one producer schema");
    let mut lifecycle = program_local_lifecycle(
        782,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 882, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding.identity(),
                &root,
                lease,
            )],
        )
        .expect("counted epoch cohort")
        .into_runtime();
    let established = installation
        .establish(
            &mut runtime,
            &lifecycle,
            program_local_subject(&root, 982, 1082, Some(8)),
        )
        .expect("counted root establishes");
    let plan = ProgramLocalExtentMaterializationPlan::new(
        "Region",
        "Region::Owned",
        "ByteUnit",
        0x5000,
        9,
        extent_id(10, AddressSpaceId::from_normalized_identity),
        ExtentRights::none(),
        extent_id(20, ExtentProvenanceId::from_normalized_identity),
        extent_id(30, MappingEraId::from_normalized_identity),
    )
    .expect("syntactically valid Extent plan");
    let mut registry = ProgramLocalExtentRegistry::new();
    let rejected = registry
        .materialize(established, plan)
        .expect_err("counted authority has no one-Extent interpretation");
    assert!(rejected.diagnostic().0.contains("counted"));
    let [(established, _)]: [(EstablishedProgramLocalRoot<'_, '_>, _); 1] = (*rejected)
        .into_inputs()
        .try_into()
        .expect("rejection returns the exact account and plan");
    assert_eq!(registry.held_accounts(), 0);
    installation
        .retire_established(established, &mut lifecycle)
        .expect("rejected materialization returns retireable account");
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

#[test]
fn subject_capacity_rejection_is_transactional_and_a_later_epoch_is_fresh() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("verified installed prebinding")
        .try_into()
        .expect("one producer schema");
    let prebinding = prebinding.identity();
    let mut lifecycle = program_local_lifecycle(
        750,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let lease = program_local_epoch_lease(&mut lifecycle, 850, 10, "TestRoot::entry");
    let mut runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(prebinding, &root, lease)],
        )
        .expect("first epoch cohort")
        .into_runtime();

    let rejected_batch = installation
        .establish_batch(
            &mut runtime,
            &lifecycle,
            [
                program_local_subject(&root, 948, 1048, Some(3)),
                program_local_subject(&root, 949, 1049, Some(3)),
            ],
        )
        .expect_err("a batch cannot establish one pending occurrence twice");
    assert!(rejected_batch.diagnostic().0.contains("repeats one exact"));
    let returned = rejected_batch.into_subjects();
    assert_eq!(returned.len(), 2);
    assert_eq!(returned[0].invocation().normalized_identity(), 948);
    assert_eq!(returned[1].invocation().normalized_identity(), 949);
    assert_eq!(runtime.pending_occurrences().len(), 1);

    let rejected = installation
        .establish(
            &mut runtime,
            &lifecycle,
            program_local_subject(&root, 950, 1050, None),
        )
        .expect_err("missing subject-dependent capacity rejects");
    assert!(rejected.diagnostic().0.contains("omits or adds"));
    assert_eq!(runtime.pending_occurrences().len(), 1);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));
    let first = installation
        .establish(
            &mut runtime,
            &lifecycle,
            program_local_subject(&root, 951, 1051, Some(3)),
        )
        .expect("failed evaluation did not burn the pending occurrence");
    let first_lineage = first.lineage();
    installation
        .retire_established(first, &mut lifecycle)
        .expect("first epoch root retirement");

    publish_program_local_era(
        &mut lifecycle,
        20,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        125,
        true,
    );
    let next_lease = program_local_epoch_lease(&mut lifecycle, 851, 20, "TestRoot::entry");
    let mut next_runtime = installation
        .seal_epoch_cohort(
            &lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding, &root, next_lease,
            )],
        )
        .expect("later epoch cohort")
        .into_runtime();
    let next = installation
        .establish(
            &mut next_runtime,
            &lifecycle,
            program_local_subject(&root, 952, 1052, Some(3)),
        )
        .expect("later epoch establishes a fresh root");
    assert_ne!(first_lineage, next.lineage());
    assert_eq!(next.lineage().occurrence().lifecycle_epoch(), 20);
    installation
        .retire_established(next, &mut lifecycle)
        .expect("later epoch root retirement");
}

#[test]
fn program_local_root_schemas_prebind_exact_installed_slots_without_minting() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, first, second) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut bindings = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let first_occurrences = bindings
        .prebind(&catalog, &terminal, &first)
        .expect("first exact slot prebinding");
    let [first_occurrence] = first_occurrences.as_slice() else {
        panic!("one producer schema")
    };
    assert!(
        bindings
            .prebind(&catalog, &terminal, &second)
            .expect_err("a runtime-open root is outside the required cohort")
            .0
            .contains("outside the sealed required closure")
    );
    let counts = bindings.counts();
    let [count] = counts.as_slice() else {
        panic!("one exact installed schema count")
    };
    assert_eq!(count.installed_slot_count.get(), 1);
    assert_eq!(count.prebinding_identities, [first_occurrence.identity()]);
    assert_eq!(
        count.per_occurrence_capacity,
        module.boundary_machines[0].program_local_root_introductions[0].capacity
    );
    assert!(bindings.prebind(&catalog, &terminal, &first).is_err());
}

#[test]
fn program_local_root_prebinding_rejects_catalog_object_and_claim_substitution() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let mut narrowed_module = module.clone();
    let boundary_identity = narrowed_module.boundary_machines[0].identity.clone();
    let qualification_identity = narrowed_module.structural_domains[0].identity.clone();
    let carrier_identity = narrowed_module.structural_types[0].identity.clone();
    let narrowed_schema = {
        let schema = &mut narrowed_module.boundary_machines[0].program_local_root_introductions[0];
        schema.capacity = psi_core::ContentProjectionExpression::CountedQuantity(
            psi_core::ContentProjectionScalar::Natural("1".into()),
        );
        schema.projection.projection_report_fingerprint =
            psi_language_semantics::content::terminal_projection_report_fingerprint(
                &schema.algebra,
                &schema.capacity,
            );
        schema.compatibility_report_identity =
            program_local_root_introduction_compatibility_report_identity(
                &boundary_identity,
                &qualification_identity,
                &carrier_identity,
                schema,
            );
        schema.clone()
    };
    narrowed_module.structural_domains[0].content_projection = Some(StructuralContentProjection {
        identity: narrowed_schema.projection,
        algebra: narrowed_schema.algebra.clone(),
        expression: narrowed_schema.capacity.clone(),
    });
    let narrowed_catalog = program_local_root_catalog(&narrowed_module);
    let narrowed_terminal = program_local_terminal_object(&narrowed_module);
    assert_ne!(catalog.terminal_psi(), narrowed_catalog.terminal_psi());
    let (mut root_ledger, root, wrong_claim_root) = install_test_root_pair_with_ids(
        &mut code,
        (1, 20, 21, 22, vec![program_local_claim()]),
        (
            201,
            220,
            221,
            222,
            vec![ExternalRootEntryClaim {
                domain: "Region::Other".into(),
                ..program_local_claim()
            }],
        ),
        entry,
    );

    let mut tampered_module = module.clone();
    tampered_module.boundary_machines[0].program_local_root_introductions[0]
        .compatibility_report_identity ^= 1;
    assert!(
        psi_terminal_verifier::verify_module(
            &tampered_module,
            &psi_terminal_verifier::ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .is_err()
    );

    let mut wrong_object = program_local_terminal_object(&module);
    wrong_object.identity = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([9; 32]),
    };
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let original_catalog_narrowed_artifact = installation
        .prebind(&catalog, &narrowed_terminal, &root)
        .expect_err("a narrowed artifact cannot carry the original producer catalog");
    assert_eq!(
        original_catalog_narrowed_artifact.0,
        "program-local root catalog does not match the terminal artifact identity"
    );
    assert_eq!(installation.prebindings().count(), 0);

    let narrowed_catalog_original_artifact = installation
        .prebind(&narrowed_catalog, &terminal, &root)
        .expect_err("a narrowed producer catalog cannot describe the original artifact");
    assert_eq!(
        narrowed_catalog_original_artifact.0,
        "program-local root catalog does not match the terminal artifact identity"
    );
    assert_eq!(installation.prebindings().count(), 0);

    assert!(
        installation
            .prebind(&catalog, &wrong_object, &root)
            .is_err()
    );

    assert!(
        installation
            .prebind(&catalog, &terminal, &wrong_claim_root)
            .is_err()
    );
    installation
        .prebind(&catalog, &terminal, &root)
        .expect("failed substitutions leave the exact prebinding available");
}

#[test]
fn program_local_root_join_pins_exact_root_artifact_contract_and_epoch_once() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("verified installed prebinding")
        .try_into()
        .expect("one program-local prebinding");
    let prebinding = prebinding.identity();
    let mut lifecycle = program_local_lifecycle(
        700,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let occurrence = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        800,
        10,
        "TestRoot::entry",
    )
    .expect("exact lifecycle join");
    assert_eq!(occurrence.identity().prebinding(), prebinding);
    assert_eq!(occurrence.identity().lifecycle_epoch(), 10);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(1));

    let mut foreign_lifecycle = program_local_lifecycle(
        701,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let foreign = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut foreign_lifecycle,
        850,
        10,
        "TestRoot::entry",
    )
    .expect_err("one installed prebinding family has one lifecycle ledger");
    assert!(foreign.diagnostic().0.contains("another lifecycle ledger"));
    let foreign_lease = sole_rejected_cohort_lease(*foreign);
    foreign_lifecycle
        .release_program_local_root_epoch_lease(foreign_lease)
        .expect("lifecycle substitution returns its lease");

    let replay = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        801,
        10,
        "TestRoot::entry",
    )
    .expect_err("one exact occurrence cannot join twice in one era");
    let replay_lease = sole_rejected_cohort_lease(*replay);
    lifecycle
        .release_program_local_root_epoch_lease(replay_lease)
        .expect("rejected join returns its lease intact");

    let retired = installation
        .retire(occurrence, &mut lifecycle)
        .expect("exact occurrence retirement");
    assert_eq!(retired.identity().prebinding(), prebinding);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));

    let replay = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        802,
        10,
        "TestRoot::entry",
    )
    .expect_err("retirement never makes the same-era origin reusable");
    let replay_lease = sole_rejected_cohort_lease(*replay);
    lifecycle
        .release_program_local_root_epoch_lease(replay_lease)
        .expect("used-key rejection returns its lease");

    publish_program_local_era(
        &mut lifecycle,
        20,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        120,
        true,
    );
    let next_epoch = join_program_local(
        &mut installation,
        prebinding,
        &root,
        &mut lifecycle,
        803,
        20,
        "TestRoot::entry",
    )
    .expect("a later lifecycle epoch is a fresh occurrence");
    assert_eq!(next_epoch.identity().lifecycle_epoch(), 20);
    installation
        .retire(next_epoch, &mut lifecycle)
        .expect("later epoch occurrence retires independently");
}

#[test]
fn program_local_root_failed_join_returns_lease_without_burning_the_occurrence() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, first, other) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &first)
        .expect("verified installed prebinding")
        .try_into()
        .expect("one program-local prebinding");
    let prebinding = prebinding.identity();

    let mut lifecycle = program_local_lifecycle(
        710,
        10,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let substituted = join_program_local(
        &mut installation,
        prebinding,
        &other,
        &mut lifecycle,
        810,
        10,
        "TestRoot::entry",
    )
    .expect_err("a different installed root cannot satisfy the prebinding");
    let lease = sole_rejected_cohort_lease(*substituted);
    lifecycle
        .release_program_local_root_epoch_lease(lease)
        .expect("root substitution returns the lease");

    let mut wrong_artifact = program_local_lifecycle(
        711,
        10,
        omega_installation_evidence::InstalledArtifactOccurrenceDigest::from_sha256([0xaa; 32]),
        code_identity,
        "TestRoot::entry",
    );
    let substituted = join_program_local(
        &mut installation,
        prebinding,
        &first,
        &mut wrong_artifact,
        811,
        10,
        "TestRoot::entry",
    )
    .expect_err("artifact occurrence substitution rejects");
    let lease = sole_rejected_cohort_lease(*substituted);
    wrong_artifact
        .release_program_local_root_epoch_lease(lease)
        .expect("artifact substitution returns the lease");

    let mut wrong_contract = program_local_lifecycle(
        712,
        10,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "OtherRoot::entry",
    );
    let substituted = join_program_local(
        &mut installation,
        prebinding,
        &first,
        &mut wrong_contract,
        812,
        10,
        "OtherRoot::entry",
    )
    .expect_err("entry-contract substitution rejects");
    let lease = sole_rejected_cohort_lease(*substituted);
    wrong_contract
        .release_program_local_root_epoch_lease(lease)
        .expect("contract substitution returns the lease");

    let mut stale_lifecycle = program_local_lifecycle(
        713,
        10,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let stale_lease = program_local_epoch_lease(&mut stale_lifecycle, 814, 10, "TestRoot::entry");
    publish_program_local_era(
        &mut stale_lifecycle,
        20,
        first.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
        121,
        true,
    );
    let stale = installation
        .seal_epoch_cohort(
            &stale_lifecycle,
            [ProgramLocalRootCohortMember::new(
                prebinding,
                &first,
                stale_lease,
            )],
        )
        .expect_err("a lease from a now-closing era cannot establish new authority");
    assert!(stale.diagnostic().0.contains("current epoch ledger"));
    let stale_lease = sole_rejected_cohort_lease(*stale);
    stale_lifecycle
        .release_program_local_root_epoch_lease(stale_lease)
        .expect("stale establishment rejection returns the lifecycle hold");

    let exact = join_program_local(
        &mut installation,
        prebinding,
        &first,
        &mut lifecycle,
        813,
        10,
        "TestRoot::entry",
    )
    .expect("failed joins did not burn the exact occurrence");
    installation
        .retire(exact, &mut lifecycle)
        .expect("exact occurrence remains retireable");
}

#[test]
fn program_local_root_failed_retirement_returns_the_complete_occurrence() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let code_identity = code.identity().normalized_identity();
    let module = program_local_root_module();
    let catalog = program_local_root_catalog(&module);
    let terminal = program_local_terminal_object(&module);
    let (mut root_ledger, root, _open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let mut installation = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("sole program-local cohort verifier");
    let [prebinding] = installation
        .prebind(&catalog, &terminal, &root)
        .expect("verified installed prebinding")
        .try_into()
        .expect("one program-local prebinding");
    let mut rightful = program_local_lifecycle(
        720,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let mut substituted = program_local_lifecycle(
        721,
        10,
        root.installed_artifact_occurrence_digest(),
        code_identity,
        "TestRoot::entry",
    );
    let occurrence = join_program_local(
        &mut installation,
        prebinding.identity(),
        &root,
        &mut rightful,
        820,
        10,
        "TestRoot::entry",
    )
    .expect("exact lifecycle join");

    let error = installation
        .retire(occurrence, &mut substituted)
        .expect_err("another lifecycle ledger cannot release the occurrence");
    assert!(error.diagnostic().0.contains("lifecycle lease"));
    let occurrence = (*error).into_occurrence();
    assert_eq!(rightful.program_local_root_authority_holds(10), Some(1));
    installation
        .retire(occurrence, &mut rightful)
        .expect("returned occurrence retires through the rightful ledger");
    assert_eq!(rightful.program_local_root_authority_holds(10), Some(0));
}

fn interrupt_boundary() -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary x86 plan");
    let mut call = ordinary.plan().call.clone();
    call.ordinary_clobbers = RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ]);
    call.entry_control = EntryControl::InterruptReturn;
    let interrupted_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::VectorRegisters,
    ]);
    let saved_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime: MachineRegime::X86Long64,
                interrupted_state,
                saved_state,
                restored_state: saved_state,
                permitted_transitive_use: MachineStateSet::new([
                    MachineState::GeneralRegisters,
                    MachineState::Flags,
                ]),
                stack: EntryStack::Dedicated { class: 1 },
                preemption: Preemption::Masked,
            },
        },
        &signature,
    )
    .expect("interrupt boundary")
}

fn interrupt_candidate(entry: EntryStubId) -> ExternalRootCandidate {
    interrupt_candidate_for_code(entry, &installed_code(1, entry))
}

fn interrupt_candidate_for_code(entry: EntryStubId, code: &InstalledCode) -> ExternalRootCandidate {
    let selected_completion = selected_interrupt_completion();
    interrupt_candidate_for_code_with_completion(entry, code, &selected_completion)
}

fn interrupt_candidate_for_code_with_completion(
    entry: EntryStubId,
    code: &InstalledCode,
    selected_completion: &SelectedProviderPlanFacts,
) -> ExternalRootCandidate {
    let mut candidate = candidate_for_code(entry, code);
    candidate.requirement_identity = "TimerRoot::tick".into();
    candidate.entry_claims = vec![ExternalRootEntryClaim {
        parameter_index: 0,
        domain: "InterruptAcknowledgement::Pending".into(),
        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
    }];
    candidate.acknowledgement_parameter_index = Some(0);
    candidate.interrupt_mask_guard_claim = Some(ExternalRootResultClaim {
        provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "InterruptMaskControl::save_and_mask".into(),
        domain: "InterruptMaskGuard::Active".into(),
        effective_carry: psi_language_semantics::CarryPolicy::STRICT,
    });
    candidate.service_reach = ResolvedRootServiceReach::from_selected_provider_closure(
        Vec::new(),
        vec!["InterruptCompletion::complete".into()],
        selected_completion,
    )
    .expect("selected completion closes the installed interrupt reach");
    let boundary = interrupt_boundary();
    candidate.stack.realization = stack_demand(
        candidate.identity,
        candidate.provider,
        candidate.nesting_relation,
        &boundary,
        code,
        entry,
        EntryStack::Dedicated { class: 1 },
        2048,
    );
    candidate
}

fn interrupt_entry_receipt(
    root: &InstalledExternalRoot<'_>,
    invocation: u64,
    acknowledgement_policy: Option<u64>,
    acknowledgement: Option<u64>,
) -> InterruptEntryReceipt {
    InterruptEntryReceipt::from_provider(
        root_id(
            60 + invocation,
            InterruptEntryReceiptId::from_normalized_identity,
        ),
        root,
        root_id(invocation, InterruptInvocationId::from_normalized_identity),
        root_id(
            70 + invocation,
            InterruptMaskControlId::from_normalized_identity,
        ),
        root_id(80, InterruptMaskStateId::from_normalized_identity),
        acknowledgement_policy
            .map(|identity| root_id(identity, AcknowledgementPolicyId::from_normalized_identity)),
        acknowledgement.map(|identity| {
            root_id(
                identity,
                InterruptAcknowledgementId::from_normalized_identity,
            )
        }),
    )
}

#[test]
fn interrupt_entry_mints_exact_linear_obligations_and_requires_settlement() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let boundary = interrupt_boundary();
    let validated =
        validate_external_root(interrupt_candidate(entry), &boundary).expect("interrupt root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed interrupt root");

    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 90, Some(7), Some(91)),
        )
        .expect("admitted interrupt entry");
    let (pending, mut control, acknowledgement) = obligations.into_parts();
    let masked = root_id(81, InterruptMaskStateId::from_normalized_identity);
    let nested_masked = root_id(82, InterruptMaskStateId::from_normalized_identity);
    let first_guard_id = root_id(92, InterruptMaskGuardId::from_normalized_identity);
    let second_guard_id = root_id(93, InterruptMaskGuardId::from_normalized_identity);
    let first = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                94,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            first_guard_id,
            masked,
            true,
        ))
        .expect("first exact mask save");
    assert_eq!(
        first.qualification(),
        &AdmittedResultQualification {
            provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
            requirement_identity: "InterruptMaskControl::save_and_mask".into(),
            domain: "InterruptMaskGuard::Active".into(),
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            transition_receipt: root_id(
                94,
                InterruptMaskTransitionReceiptId::from_normalized_identity
            ),
            invocation: root_id(90, InterruptInvocationId::from_normalized_identity),
            subject: AdmittedResultSubject::InterruptMaskGuard(first_guard_id),
        }
    );
    let second = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                95,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            second_guard_id,
            nested_masked,
            true,
        ))
        .expect("nested exact mask save");

    let out_of_order_receipt = InterruptMaskRestoreReceipt::from_provider(
        root_id(
            96,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &first,
        true,
    );
    let out_of_order = first
        .restore(&mut control, out_of_order_receipt)
        .expect_err("nested masks must restore in LIFO order");
    assert!(
        out_of_order
            .diagnostic()
            .0
            .contains("newest exact saved state")
    );
    let (first, _) = out_of_order.into_parts();
    let second_receipt = InterruptMaskRestoreReceipt::from_provider(
        root_id(
            97,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &second,
        true,
    );
    second
        .restore(&mut control, second_receipt)
        .expect("nested restore");
    let first_receipt = InterruptMaskRestoreReceipt::from_provider(
        root_id(
            98,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &first,
        true,
    );
    first
        .restore(&mut control, first_receipt)
        .expect("outer restore");
    let replayed_guard = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                105,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            first_guard_id,
            masked,
            true,
        ))
        .expect_err("a settled guard identity cannot be minted again");
    assert!(replayed_guard.diagnostic().0.contains("fresh guard"));

    let acknowledgement = acknowledgement.expect("policy-bearing interrupt mints acknowledgement");
    let [pending_qualification] = acknowledgement.qualifications() else {
        panic!("acknowledgement must retain its exact Pending entry qualification");
    };
    assert_eq!(
        pending_qualification.provider_plan,
        root_id(55, ProviderPlanId::from_normalized_identity)
    );
    assert_eq!(
        pending_qualification.requirement_identity,
        "TimerRoot::tick"
    );
    assert_eq!(pending_qualification.parameter_index, 0);
    assert_eq!(
        pending_qualification.abi_placement(),
        &interrupt_boundary().plan().call.parameters[0],
        "the live admitted occurrence must retain the exact ABI placement for its semantic parameter"
    );
    assert!(
        pending_qualification
            .matches_parameter_placement(0, &interrupt_boundary().plan().call.parameters[0])
    );
    assert!(
        !pending_qualification
            .matches_parameter_placement(1, &interrupt_boundary().plan().call.parameters[0])
    );
    let mut drifted_placement = interrupt_boundary().plan().call.parameters[0].clone();
    drifted_placement.locations.clear();
    assert!(!pending_qualification.matches_parameter_placement(0, &drifted_placement));
    assert_eq!(
        pending_qualification.domain,
        "InterruptAcknowledgement::Pending"
    );
    assert_eq!(
        pending_qualification.effective_carry,
        psi_language_semantics::CarryPolicy::STRICT
    );
    assert_eq!(
        pending_qualification.entry_receipt,
        root_id(150, InterruptEntryReceiptId::from_normalized_identity)
    );
    assert_eq!(
        pending_qualification.subject,
        AdmittedEntrySubject::InterruptAcknowledgement(root_id(
            91,
            InterruptAcknowledgementId::from_normalized_identity
        ))
    );
    assert!(pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(56, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "LookalikeRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        1,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Forged",
        psi_language_semantics::CarryPolicy::STRICT,
    ));
    assert!(!pending_qualification.matches_contract(
        root_id(55, ProviderPlanId::from_normalized_identity),
        "TimerRoot::tick",
        0,
        "InterruptAcknowledgement::Pending",
        psi_language_semantics::CarryPolicy::PERMISSIVE,
    ));
    assert_eq!(
        acknowledgement
            .qualification_for_contract(
                root_id(55, ProviderPlanId::from_normalized_identity),
                "TimerRoot::tick",
                0,
                "InterruptAcknowledgement::Pending",
                psi_language_semantics::CarryPolicy::STRICT,
            )
            .expect("linear acknowledgement must resolve its exact accepted contract"),
        pending_qualification
    );
    assert!(
        acknowledgement
            .qualification_for_contract(
                root_id(56, ProviderPlanId::from_normalized_identity),
                "TimerRoot::tick",
                0,
                "InterruptAcknowledgement::Pending",
                psi_language_semantics::CarryPolicy::STRICT,
            )
            .expect_err("a different provider plan cannot reuse the occurrence")
            .0
            .contains("maps to 0 qualifications")
    );
    let acknowledgement_receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            99,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        "InterruptCompletion::complete",
    )
    .expect("exact installed completion route");
    let completed_acknowledgement = acknowledgement
        .complete(acknowledgement_receipt)
        .expect("exact acknowledgement completion");
    let completed = ledger
        .finish_interrupt_entry(pending, control, Some(completed_acknowledgement))
        .expect("settled interrupt exit");
    assert_eq!(completed.root, installed.root());
    assert_eq!(
        completed.entry_receipt,
        root_id(150, InterruptEntryReceiptId::from_normalized_identity)
    );
    assert_eq!(
        completed.acknowledgement_receipt,
        Some(root_id(
            99,
            InterruptAcknowledgementReceiptId::from_normalized_identity
        ))
    );
}

#[test]
fn interrupt_completion_route_retains_pic_and_lapic_selected_rows() {
    let providers = [
        (
            0x4100,
            selected_interrupt_completion_for(
                "LegacyPic",
                "LegacyPicController",
                "LegacyPicController::complete",
                &["PortIo"],
            ),
            vec!["PortIo".to_owned()],
        ),
        (
            0x4200,
            selected_interrupt_completion_for(
                "LocalApic",
                "LocalApicController",
                "LocalApicController::complete",
                &["MachineControl"],
            ),
            vec!["MachineControl".to_owned()],
        ),
    ];

    for (seed, selected, expected_row) in providers {
        let entry = entry_id(seed + 1);
        let mut code = installed_code(seed + 2, entry);
        let boundary = interrupt_boundary();
        let candidate = interrupt_candidate_for_code_with_completion(entry, &code, &selected);
        let expected_resolution = candidate.service_reach.resolutions()[0].clone();
        let validated = validate_external_root(candidate, &boundary)
            .expect("provider-shaped interrupt root plan");
        let authority = slot();
        let execution = provider_execution(&validated);
        let expected_entry_plan = execution.provider_plan();
        let expected_execution = execution.identity();
        let admission = RootAdmission::from_admitted_provider(
            root_id(seed + 3, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
        let installed = ledger
            .install(&code, validated, authority, admission)
            .expect("installed interrupt root");
        let invocation = seed + 4;
        let acknowledgement_identity = seed + 5;
        let obligations = ledger
            .begin_interrupt_entry(
                &installed,
                interrupt_entry_receipt(
                    &installed,
                    invocation,
                    Some(7),
                    Some(acknowledgement_identity),
                ),
            )
            .expect("admitted interrupt entry");
        let (pending, control, acknowledgement) = obligations.into_parts();
        let acknowledgement = acknowledgement.expect("policy mints acknowledgement");
        let receipt = InterruptAcknowledgementReceipt::from_provider(
            root_id(
                seed + 6,
                InterruptAcknowledgementReceiptId::from_normalized_identity,
            ),
            &acknowledgement,
            "InterruptCompletion::complete",
        )
        .expect("provider completion binds exact installed route");

        assert_eq!(receipt.route().entry_provider_plan(), expected_entry_plan);
        assert_eq!(receipt.route().provider_execution(), expected_execution);
        assert_eq!(
            receipt.route().completion_requirement_identity(),
            "InterruptCompletion::complete"
        );
        assert_eq!(receipt.route().resolution(), &expected_resolution);
        assert_eq!(receipt.route().resolution().resolved_row, expected_row);
        assert_eq!(
            receipt.route().invocation(),
            root_id(invocation, InterruptInvocationId::from_normalized_identity)
        );
        assert_eq!(
            receipt.route().policy(),
            root_id(7, AcknowledgementPolicyId::from_normalized_identity)
        );
        assert_eq!(
            receipt.route().acknowledgement(),
            root_id(
                acknowledgement_identity,
                InterruptAcknowledgementId::from_normalized_identity
            )
        );

        let completed = acknowledgement
            .complete(receipt)
            .expect("exact provider-shaped completion route settles token");
        ledger
            .finish_interrupt_entry(pending, control, Some(completed))
            .expect("exact provider-shaped completion permits interrupt exit");
    }
}

#[test]
fn interrupt_completion_route_rejects_every_coordinate_drift_and_returns_retry_custody() {
    let entry = entry_id(0x4301);
    let mut code = installed_code(0x4302, entry);
    let boundary = interrupt_boundary();
    let validated = validate_external_root(interrupt_candidate_for_code(entry, &code), &boundary)
        .expect("interrupt root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(0x4303, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed interrupt root");
    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 0x4304, Some(7), Some(0x4305)),
        )
        .expect("admitted interrupt entry");
    let (pending, control, acknowledgement) = obligations.into_parts();
    let acknowledgement = acknowledgement.expect("policy mints acknowledgement");
    let missing = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            0x4306,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        "LookalikeCompletion::complete",
    )
    .expect_err("a lookalike requirement has no exact installed resolution");
    assert!(missing.0.contains("absent from the exact installed reach"));

    let mut receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            0x4307,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        "InterruptCompletion::complete",
    )
    .expect("exact installed completion route");
    let exact_route = receipt.route.clone();
    let acknowledgement_identity = acknowledgement.identity();

    receipt.route.completion_requirement_identity = "LookalikeCompletion::complete".into();
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion requirement drift must reject");
    assert!(error.diagnostic().0.contains("exact invocation"));
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.resolution.provider_plan_report_identity ^= 1;
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion provider-plan drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.provider_execution =
        root_id(0x4308, ProviderExecutionId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("provider-execution drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.policy = root_id(8, AcknowledgementPolicyId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("acknowledgement-policy drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.invocation = root_id(0x4309, InterruptInvocationId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("invocation-lineage drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.acknowledgement =
        root_id(0x430a, InterruptAcknowledgementId::from_normalized_identity);
    let error = acknowledgement
        .complete(receipt)
        .expect_err("token-lineage drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.resolution.upper_bound = vec!["PortIo".into()];
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion bound drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route.clone();

    receipt.route.resolution.resolved_row = vec!["MachineControl".into()];
    let error = acknowledgement
        .complete(receipt)
        .expect_err("completion row drift must reject");
    let (acknowledgement, mut receipt) = (*error).into_parts();
    assert_eq!(acknowledgement.identity(), acknowledgement_identity);
    receipt.route = exact_route;

    let completed = acknowledgement
        .complete(receipt)
        .expect("returned acknowledgement and receipt retry on the exact route");
    ledger
        .finish_interrupt_entry(pending, control, Some(completed))
        .expect("retried exact completion permits interrupt exit");
}

#[test]
fn interrupt_entry_rejects_policy_drift_replay_and_unsettled_exit() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let boundary = interrupt_boundary();
    let validated =
        validate_external_root(interrupt_candidate(entry), &boundary).expect("interrupt root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed interrupt root");

    let drifted = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(8), Some(101)),
        )
        .expect_err("a different acknowledgement policy cannot mint a token");
    assert!(drifted.diagnostic().0.contains("acknowledgement policy"));

    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(7), Some(101)),
        )
        .expect("admitted interrupt entry");
    let replay = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(7), Some(102)),
        )
        .expect_err("an admitted invocation cannot be replayed");
    assert!(replay.diagnostic().0.contains("replays an invocation"));
    let removal_receipt = RootRemovalReceipt::from_provider(
        root_id(104, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    let removal = ledger
        .remove(installed, removal_receipt)
        .expect_err("an active interrupt pins root retirement");
    assert!(removal.diagnostic().0.contains("quiescence"));
    let (installed, _) = removal.into_parts();

    let (pending, control, acknowledgement) = obligations.into_parts();
    let unsettled = ledger
        .finish_interrupt_entry(pending, control, None)
        .expect_err("policy-bearing interrupt must return its completed acknowledgement");
    assert!(
        unsettled
            .diagnostic()
            .0
            .contains("completed acknowledgement")
    );
    let (pending, control, _) = unsettled.into_parts();
    let acknowledgement = acknowledgement.expect("minted acknowledgement");
    let acknowledgement_receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            103,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        "InterruptCompletion::complete",
    )
    .expect("exact installed completion route");
    let completed = acknowledgement
        .complete(acknowledgement_receipt)
        .expect("exact acknowledgement");
    ledger
        .finish_interrupt_entry(pending, control, Some(completed))
        .expect("settled retry");
    let completed_replay = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 100, Some(7), Some(104)),
        )
        .expect_err("a completed invocation cannot be replayed");
    assert!(
        completed_replay
            .diagnostic()
            .0
            .contains("replays an invocation")
    );
    let removal_receipt = RootRemovalReceipt::from_provider(
        root_id(105, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    ledger
        .remove(installed, removal_receipt)
        .expect("settled interrupt permits exact root retirement");
}

#[test]
fn interrupt_entry_without_acknowledgement_policy_mints_no_acknowledgement() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let boundary = interrupt_boundary();
    let mut candidate = interrupt_candidate(entry);
    candidate.acknowledgement_policy = None;
    candidate.interrupt_mask_guard_claim = None;
    let validated = validate_external_root(candidate, &boundary).expect("exception root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed exception root");

    let obligations = ledger
        .begin_interrupt_entry(
            &installed,
            interrupt_entry_receipt(&installed, 110, None, None),
        )
        .expect("entry without an acknowledgement protocol");
    let (pending, mut control, acknowledgement) = obligations.into_parts();
    assert!(acknowledgement.is_none());
    let rejected_mask = control
        .save_and_mask(InterruptMaskSaveReceipt::from_provider(
            root_id(
                112,
                InterruptMaskTransitionReceiptId::from_normalized_identity,
            ),
            &control,
            root_id(113, InterruptMaskGuardId::from_normalized_identity),
            root_id(114, InterruptMaskStateId::from_normalized_identity),
            true,
        ))
        .expect_err("a mask transition without a routed result contract must reject");
    assert!(
        rejected_mask
            .diagnostic()
            .0
            .contains("no admitted routed result contract")
    );
    ledger
        .finish_interrupt_entry(pending, control, None)
        .expect("exception exit with restored mask and no acknowledgement debt");
}

#[test]
fn interrupt_entry_receipt_cannot_substitute_colliding_installed_root() {
    let entry = entry_id(1001);
    let mut first_code = installed_code_with_fill(1, entry, 0x90);
    let mut second_code = installed_code_with_fill(1, entry, 0xcc);
    let boundary = interrupt_boundary();
    let first_root =
        validate_external_root(interrupt_candidate_for_code(entry, &first_code), &boundary)
            .expect("first interrupt root");
    let second_root =
        validate_external_root(interrupt_candidate_for_code(entry, &second_code), &boundary)
            .expect("second interrupt root");
    let first_execution = provider_execution(&first_root);
    let second_execution = provider_execution(&second_root);
    let first_slot = slot();
    let second_slot = slot();
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first_root,
        &first_execution,
        &first_code,
        &first_slot,
        first_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("first admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second_root,
        &second_execution,
        &second_code,
        &second_slot,
        second_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second admission");

    let mut first_ledger =
        InstalledRootLedger::claim(&mut first_code).expect("first canonical root ledger");
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed interrupt root");
    let mut second_ledger =
        InstalledRootLedger::claim(&mut second_code).expect("second canonical root ledger");
    let second_installed = second_ledger
        .install(&second_code, second_root, second_slot, second_admission)
        .expect("second installed interrupt root");
    let substituted_receipt = interrupt_entry_receipt(&second_installed, 120, Some(7), Some(121));

    let error = first_ledger
        .begin_interrupt_entry(&first_installed, substituted_receipt)
        .expect_err("entry receipt must bind exact installed-root evidence");
    assert!(error.diagnostic().0.contains("exact installed"));
}

#[test]
fn interrupt_obligation_receipts_retain_exact_invocation_evidence() {
    let entry = entry_id(1001);
    let mut first_code = installed_code_with_fill(1, entry, 0x90);
    let mut second_code = installed_code_with_fill(1, entry, 0xcc);
    let boundary = interrupt_boundary();
    let first_root =
        validate_external_root(interrupt_candidate_for_code(entry, &first_code), &boundary)
            .expect("first interrupt root");
    let second_root =
        validate_external_root(interrupt_candidate_for_code(entry, &second_code), &boundary)
            .expect("second interrupt root");
    let first_execution = provider_execution(&first_root);
    let second_execution = provider_execution(&second_root);
    let first_slot = slot();
    let second_slot = slot();
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first_root,
        &first_execution,
        &first_code,
        &first_slot,
        first_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("first admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second_root,
        &second_execution,
        &second_code,
        &second_slot,
        second_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second admission");
    let mut first_ledger =
        InstalledRootLedger::claim(&mut first_code).expect("first canonical root ledger");
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed root");
    let mut second_ledger =
        InstalledRootLedger::claim(&mut second_code).expect("second canonical root ledger");
    let second_installed = second_ledger
        .install(&second_code, second_root, second_slot, second_admission)
        .expect("second installed root");

    let first_obligations = first_ledger
        .begin_interrupt_entry(
            &first_installed,
            interrupt_entry_receipt(&first_installed, 130, Some(7), Some(131)),
        )
        .expect("first invocation");
    let second_obligations = second_ledger
        .begin_interrupt_entry(
            &second_installed,
            interrupt_entry_receipt(&second_installed, 130, Some(7), Some(131)),
        )
        .expect("second invocation");
    let (_, mut first_control, first_acknowledgement) = first_obligations.into_parts();
    let (_, second_control, second_acknowledgement) = second_obligations.into_parts();

    let substituted_mask_receipt = InterruptMaskSaveReceipt::from_provider(
        root_id(
            132,
            InterruptMaskTransitionReceiptId::from_normalized_identity,
        ),
        &second_control,
        root_id(133, InterruptMaskGuardId::from_normalized_identity),
        root_id(134, InterruptMaskStateId::from_normalized_identity),
        true,
    );
    let mask_error = first_control
        .save_and_mask(substituted_mask_receipt)
        .expect_err("mask receipt cannot cross exact invocation evidence");
    assert!(mask_error.diagnostic().0.contains("exact control"));

    let first_acknowledgement = first_acknowledgement.expect("first acknowledgement");
    let second_acknowledgement = second_acknowledgement.expect("second acknowledgement");
    let substituted_ack_receipt = InterruptAcknowledgementReceipt::from_provider(
        root_id(
            135,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &second_acknowledgement,
        "InterruptCompletion::complete",
    )
    .expect("second exact installed completion route");
    let acknowledgement_error = first_acknowledgement
        .complete(substituted_ack_receipt)
        .expect_err("acknowledgement receipt cannot cross exact invocation evidence");
    assert!(
        acknowledgement_error
            .diagnostic()
            .0
            .contains("exact invocation")
    );
}

#[test]
fn opaque_provider_exit_admission_fails_closed_and_rejects_plan_drift() {
    let validated =
        validate_external_root(candidate(entry_id(1001)), &boundary()).expect("root plan");
    let identity = root_id(54, ProviderExecutionId::from_normalized_identity);

    let missing = ProviderExecution::from_admitted_provider(identity, &validated, None)
        .expect_err("opaque provider without exit evidence must reject");
    assert!(
        missing
            .0
            .contains("accepted exit claim or adequate hardware isolation")
    );

    let unreported_isolation = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::HardwareIsolation {
            validation_receipt: root_id(99, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect_err("unreported isolation cannot serve as adequate evidence");
    assert!(unreported_isolation.0.contains("admitted trust receipts"));

    let wrong_control = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: omega_calling_conventions::EntryControl::InterruptReturn,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect_err("provider exit that violates the CallPlan must reject");
    assert!(wrong_control.0.contains("exit control"));

    let wrong_restore = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: MachineStateSet::new([MachineState::Flags]),
            },
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect_err("provider exit that violates the StatePlan must reject");
    assert!(wrong_restore.0.contains("restored-state set"));

    let isolated = ProviderExecution::from_admitted_provider(
        identity,
        &validated,
        Some(OpaqueProviderExitAssurance::HardwareIsolation {
            validation_receipt: root_id(4, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("adequate hardware isolation is the explicit alternative");
    assert!(matches!(
        isolated.exit_assurance(),
        OpaqueProviderExitAssurance::HardwareIsolation { .. }
    ));
}

#[test]
fn provider_execution_prepares_only_its_selected_entry_writer_and_exact_placement() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated =
        validate_external_root(candidate_for_code(entry, &code), &boundary()).expect("root plan");
    let mut execution = provider_execution(&validated);
    let writer = entry_writer(entry);
    let selected_plan = execution.provider_plan();

    let wrong_plan = root_id(56, ProviderPlanId::from_normalized_identity);
    let error = execution
        .prepare_post_handoff_entry_writer(wrong_plan, &code, &writer, 16, writer_site(0x8000))
        .expect_err("a different selected provider closure must reject");
    assert!(error.0.contains("selected provider plan"));

    execution.normalized_report_identity ^= 1;
    let error = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8000))
        .expect_err("execution fingerprint drift must reject before source resolution");
    assert!(error.0.contains("identity fails exact structural replay"));
    execution.normalized_report_identity ^= 1;
    execution
        .validate_for_writer_preparation()
        .expect("repaired execution evidence supports exact preparation retry");

    let wrong_writer = entry_writer(entry_id(1002));
    let error = execution
        .prepare_post_handoff_entry_writer(
            selected_plan,
            &code,
            &wrong_writer,
            16,
            writer_site(0x8000),
        )
        .expect_err("an admitted artifact sibling is not the selected root entry");
    assert!(
        error
            .0
            .contains("does not contain the admitted external-root entry")
    );

    let mut pre_resolved_writer = writer.clone();
    pre_resolved_writer.steps[0].source =
        psi_layout_plans::PostHandoffWriterSource::Resolved(0x1010);
    let error = execution
        .prepare_post_handoff_entry_writer(
            selected_plan,
            &code,
            &pre_resolved_writer,
            16,
            writer_site(0x8000),
        )
        .expect_err("a copied numeric entry cannot replace provider resolution");
    assert!(error.0.contains("sealed provider context"));

    let error = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8001))
        .expect_err("misaligned destination placement must reject");
    assert!(
        error.0.contains("align"),
        "unexpected diagnostic: {error:?}"
    );

    let prepared = execution
        .prepare_post_handoff_entry_writer(selected_plan, &code, &writer, 16, writer_site(0x8000))
        .expect("exact selected execution, entry writer, resolver, and placement");
    assert_eq!(prepared.provider_execution(), execution.binding());
    assert_eq!(prepared.selected_entry(), entry);
    assert_eq!(prepared.selected_entry_source_slot(), 0);
    assert_eq!(prepared.selected_requirement_identity(), "TestRoot::entry");
    assert_eq!(prepared.architecture(), code.architecture());
    assert!(prepared.context().binds_invocation(prepared.invocation()));
}

#[test]
fn prepared_writer_execution_replays_structure_before_destination_consumption() {
    let entry = entry_id(1001);
    let code = installed_code(1, entry);
    let validated =
        validate_external_root(candidate_for_code(entry, &code), &boundary()).expect("root plan");
    let execution = provider_execution(&validated);
    let writer = entry_writer(entry);
    let mut prepared = execution
        .prepare_post_handoff_entry_writer(
            execution.provider_plan(),
            &code,
            &writer,
            16,
            writer_site(0x8000),
        )
        .expect("exact writer preparation");
    prepared.invocation = entry_writer(entry_id(1002))
        .lower_reusable_fragment()
        .expect("structurally valid sibling invocation");
    let error = prepared
        .validate_execution(&code)
        .expect_err("retained writer/invocation drift must reject before destination use");
    assert!(
        error
            .0
            .contains("no longer matches its retained invocation")
    );

    prepared.invocation = prepared
        .writer
        .lower_reusable_fragment()
        .expect("restore exact retained invocation");
    let exact_root_evidence = prepared.root_evidence.clone();
    let mut drifted_candidate = exact_root_evidence.candidate.clone();
    drifted_candidate.requirement_identity = "SiblingRoot::entry".into();
    prepared.root_evidence = validate_external_root(drifted_candidate, &boundary())
        .expect("independently valid sibling root evidence");
    let error = prepared
        .validate_execution(&code)
        .expect_err("source requirement drift must reject");
    assert!(
        error
            .0
            .contains("exact validated external-root requirement")
    );
    prepared.root_evidence = exact_root_evidence;
    prepared.selected_entry_source_slot = 1;
    let error = prepared
        .validate_execution(&code)
        .expect_err("selected-entry source-slot drift must reject");
    assert!(error.0.contains("source-slot correspondence"));
    prepared.selected_entry_source_slot = 0;
    prepared
        .validate_execution(&code)
        .expect("corrected retained invocation supports retry");

    let colliding_code = installed_code(2, entry);
    let diagnostic = prepared
        .context
        .validate_for_destination(&colliding_code, writer_site(0x8000), 16)
        .expect_err("outward consumer must replay the exact installed realization");
    assert!(diagnostic.0.contains("exact installed context"));
    prepared
        .context
        .validate_for_destination(&code, writer_site(0x8000), 16)
        .expect("repaired opaque context supports outward replay");
}

#[test]
fn installation_records_the_complete_external_root_and_pins_code_liveness() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let selected = selected_interrupt_completion();
    let mut candidate = candidate(entry);
    candidate.service_reach = ResolvedRootServiceReach::from_selected_provider_closure(
        vec!["Timer".into()],
        vec!["InterruptCompletion::complete".into()],
        &selected,
    )
    .expect("selected provider closes root reach");
    let validated = validate_external_root(candidate, &boundary()).expect("root plan");
    let validated_identity = validated.normalized_report_identity();
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed external root");

    let record = ledger.record(installed.root()).expect("root record");
    assert_eq!(record.entry, entry);
    assert_eq!(record.normalized_root_report_identity, validated_identity);
    assert_eq!(record.installed_code, code.identity());
    assert_eq!(record.provider_execution, execution.identity());
    assert_eq!(record.provider_plan, execution.provider_plan());
    assert_eq!(record.requirement_identity, "TestRoot::entry");
    assert!(record.entry_claims.is_empty());
    assert_eq!(record.acknowledgement_parameter_index, None);
    assert!(record.interrupt_mask_guard_claim.is_none());
    assert_eq!(record.service_reach, ["PortIo", "Timer"]);
    assert_eq!(
        record.selected_provider_closure_report_fingerprint,
        selected.report_fingerprint()
    );
    assert_eq!(
        record.selected_provider_closure_digest,
        selected.identity_digest()
    );
    assert_eq!(record.installation_reach_resolutions.len(), 1);
    assert_eq!(
        record.provider_execution_report_fingerprint,
        execution.normalized_report_identity()
    );
    assert_eq!(record.effects.len(), 1);
    assert_eq!(record.trust_receipts.len(), 1);
    assert_eq!(
        record
            .stack
            .realization
            .demand(record.root)
            .expect("installed root stack demand")
            .domain(StackDomain::Interrupted)
            .expect("resolved interrupted stack domain")
            .bytes,
        2048
    );
    assert_eq!(record.logical_fuel.realization.units(), 7);
    assert_eq!(
        record.machine_state.realization.registers().as_slice(),
        &[MachineRegister::X86Rax]
    );
    assert_eq!(record.component_pins.len(), 1);
    assert_eq!(
        record.boundary_contract_report_fingerprint,
        boundary().contract_report_fingerprint()
    );
    let installed_report_fingerprint = ledger.report_fingerprint();
    assert_ne!(installed_report_fingerprint, 0);

    let root_identity = installed.root();
    let root_slot = installed.slot();
    let receipt = RootRemovalReceipt::from_provider(
        root_id(23, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    let returned = ledger.remove(installed, receipt).expect("root removal");
    assert_eq!(returned.slot(), root_slot);
    assert!(ledger.record(root_identity).is_none());
    assert_ne!(ledger.report_fingerprint(), installed_report_fingerprint);
}

#[test]
fn opaque_callback_gateway_must_be_exact_current_dispatch_and_process_lifetime() {
    let entry = entry_id(1001);
    let admitted_code = installed_code_with_fill(1, entry, 0x90);
    let substituted_code = installed_code_with_fill(1, entry, 0xcc);
    let receipt = ProcessLifetimeGatewayAdmissionReceipt::from_provider(
        root_id(70, GatewayAdmissionReceiptId::from_normalized_identity),
        root_id(71, OpaqueCallbackRegistrationId::from_normalized_identity),
        root_id(72, OpaqueCallbackProviderId::from_normalized_identity),
        root_id(73, ProcessLifetimeGatewayId::from_normalized_identity),
        root_id(74, GatewayDispatchContractId::from_normalized_identity),
        &admitted_code,
        entry,
        true,
        true,
        true,
    );
    let error = admit_process_lifetime_opaque_callback(&substituted_code, receipt)
        .expect_err("compact installed identities cannot substitute gateway code");
    assert!(error.diagnostic().0.contains("exact installed code"));
    let receipt = (*error).into_receipt();
    let gateway = admit_process_lifetime_opaque_callback(&admitted_code, receipt)
        .expect("exact process-lifetime gateway");
    assert_eq!(gateway.entry(), entry);
    assert_eq!(gateway.installed_code(), admitted_code.identity());

    let incomplete = ProcessLifetimeGatewayAdmissionReceipt::from_provider(
        root_id(75, GatewayAdmissionReceiptId::from_normalized_identity),
        root_id(76, OpaqueCallbackRegistrationId::from_normalized_identity),
        root_id(72, OpaqueCallbackProviderId::from_normalized_identity),
        root_id(73, ProcessLifetimeGatewayId::from_normalized_identity),
        root_id(74, GatewayDispatchContractId::from_normalized_identity),
        &admitted_code,
        entry,
        true,
        false,
        true,
    );
    assert!(
        admit_process_lifetime_opaque_callback(&admitted_code, incomplete)
            .expect_err("replaceable gateway cannot be advertised as process lifetime")
            .diagnostic()
            .0
            .contains("not retained for process lifetime")
    );
}

#[test]
fn installed_root_entry_replay_requires_exact_code_occurrence_and_entry() {
    let entry = entry_id(1001);
    let other_entry = entry_id(1002);
    let mut code = installed_code_with_fill(1, entry, 0x90);
    let substituted_code = installed_code_with_fill(1, entry, 0xcc);
    let context = code.receipt_context();
    let substituted_context = substituted_code.receipt_context();
    let (_ledger, installed) = install_test_root(&mut code, entry);

    assert!(installed.binds_installed_entry(&context, entry));
    assert!(!installed.binds_installed_entry(&context, other_entry));
    assert!(!installed.binds_installed_entry(&substituted_context, entry));
}

#[test]
fn reclaimable_opaque_callback_requires_unregister_and_root_quiescence() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let (mut ledger, installed) = install_test_root(&mut code, entry);
    let root_identity = installed.root();
    let not_quiesced = RootRemovalReceipt::from_provider(
        root_id(80, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        false,
    );
    let quiesced = RootRemovalReceipt::from_provider(
        root_id(81, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        true,
    );
    let provider = root_id(84, OpaqueCallbackProviderId::from_normalized_identity);
    let capacity_identity = root_id(
        89,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let registration_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            82,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(83, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            85,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &installed,
        &capacity,
        true,
    );
    let collision_equal_capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let error = admit_reclaimable_opaque_callback(
        installed,
        registration_receipt,
        collision_equal_capacity,
    )
    .expect_err("compact capacity identity cannot substitute exact provider occurrence");
    assert!(error.diagnostic().0.contains("capacity occurrence"));
    let (installed, registration_receipt, collision_equal_capacity) = (*error).into_parts();
    assert_eq!(collision_equal_capacity.identity(), capacity_identity);
    assert_eq!(collision_equal_capacity.provider(), provider);

    let substituted_capacity_identity = root_id(
        90,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let substituted_capacity = OpaqueCallbackRegistrationCapacityOccurrence::from_provider(
        substituted_capacity_identity,
        provider,
    );
    let error =
        admit_reclaimable_opaque_callback(installed, registration_receipt, substituted_capacity)
            .expect_err("a distinct live-registration capacity occurrence must reject");
    assert!(error.diagnostic().0.contains("capacity occurrence"));
    let (installed, registration_receipt, substituted_capacity) = (*error).into_parts();
    assert_eq!(
        substituted_capacity.identity(),
        substituted_capacity_identity
    );
    assert_eq!(substituted_capacity.provider(), provider);

    let substituted_provider = root_id(91, OpaqueCallbackProviderId::from_normalized_identity);
    let provider_drift_capacity = OpaqueCallbackRegistrationCapacityOccurrence::from_provider(
        capacity_identity,
        substituted_provider,
    );
    let error =
        admit_reclaimable_opaque_callback(installed, registration_receipt, provider_drift_capacity)
            .expect_err("live-registration capacity from another provider must reject");
    assert!(error.diagnostic().0.contains("capacity occurrence"));
    let (installed, registration_receipt, provider_drift_capacity) = (*error).into_parts();
    assert_eq!(provider_drift_capacity.identity(), capacity_identity);
    assert_eq!(provider_drift_capacity.provider(), substituted_provider);

    let registration = admit_reclaimable_opaque_callback(installed, registration_receipt, capacity)
        .expect("accepted unregister contract");
    assert_eq!(registration.capacity().identity(), capacity_identity);

    let mut collision_equal_unregistration = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            92,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        true,
    );
    collision_equal_unregistration.substitute_capacity_evidence_for_test(&collision_equal_capacity);
    let error = registration
        .unregister_and_quiesce(&mut ledger, collision_equal_unregistration, not_quiesced)
        .expect_err("compact capacity identity cannot substitute unregistration provenance");
    assert!(error.diagnostic().0.contains("exact registered"));
    let (registration, _, not_quiesced) = (*error).into_parts();
    assert_eq!(registration.capacity().identity(), capacity_identity);
    assert!(ledger.record(root_identity).is_some());

    let provider_incomplete = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            86,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        false,
    );
    let error = registration
        .unregister_and_quiesce(&mut ledger, provider_incomplete, not_quiesced)
        .expect_err("provider did not unregister the callback");
    assert!(error.diagnostic().0.contains("does not remove"));
    let (registration, _, not_quiesced) = (*error).into_parts();
    assert_eq!(registration.capacity().identity(), capacity_identity);
    assert!(ledger.record(root_identity).is_some());

    let provider_complete = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            87,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        true,
    );
    let error = registration
        .unregister_and_quiesce(&mut ledger, provider_complete, not_quiesced)
        .expect_err("unregistration alone cannot stand in for quiescence");
    assert!(
        error
            .diagnostic()
            .0
            .contains("quiescence is not established")
    );
    let (registration, _, _) = (*error).into_parts();
    assert_eq!(registration.capacity().identity(), capacity_identity);
    assert!(ledger.record(root_identity).is_some());

    let provider_complete = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            88,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        &registration,
        true,
    );
    let completion = registration
        .unregister_and_quiesce(&mut ledger, provider_complete, quiesced)
        .expect("foreign callback unreachable and external root quiesced");
    assert_eq!(
        completion.registration(),
        root_id(83, OpaqueCallbackRegistrationId::from_normalized_identity)
    );
    assert!(ledger.record(root_identity).is_none());
    let (slot, capacity) = completion.into_parts();
    assert_eq!(
        slot.slot(),
        root_id(20, RootSlotId::from_normalized_identity)
    );
    assert_eq!(capacity.identity(), capacity_identity);
    assert_eq!(capacity.provider(), provider);
}

#[test]
fn external_root_identity_binds_canonical_entry_claims() {
    let entry = entry_id(1001);
    let boundary = interrupt_boundary();
    let baseline = validate_external_root(interrupt_candidate(entry), &boundary)
        .expect("canonical interrupt entry contract");

    let mut drifted = interrupt_candidate(entry);
    drifted.entry_claims[0].domain = "InterruptAcknowledgement::Forged".into();
    let drifted = validate_external_root(drifted, &boundary)
        .expect("a different admitted domain remains a structurally valid root");
    assert_ne!(
        baseline.normalized_report_identity(),
        drifted.normalized_report_identity()
    );

    let mut duplicate = interrupt_candidate(entry);
    duplicate
        .entry_claims
        .push(duplicate.entry_claims[0].clone());
    let duplicate = validate_external_root(duplicate, &boundary)
        .expect_err("duplicate accepted claims must fail closed");
    assert!(duplicate.0.contains("uniquely sorted"));

    let mut missing = interrupt_candidate(entry);
    missing.entry_claims.clear();
    let missing = validate_external_root(missing, &boundary)
        .expect_err("the acknowledgement parameter must name an admitted claim");
    assert!(missing.0.contains("acknowledgement parameter"));
}

#[test]
fn external_root_entry_claim_requires_an_exact_abi_parameter() {
    let boundary = interrupt_boundary();
    let mut candidate = interrupt_candidate(entry_id(162));
    candidate.entry_claims[0].parameter_index = 1;
    candidate.acknowledgement_parameter_index = Some(1);

    let diagnostic = validate_external_root(candidate, &boundary)
        .expect_err("a semantic entry parameter outside the boundary signature must reject");
    assert!(diagnostic.0.contains("has no exact ABI placement"));
}

#[test]
fn root_admission_cannot_substitute_colliding_installed_code() {
    let entry = entry_id(1001);
    let mut admitted_code = installed_code_with_fill(1, entry, 0x90);
    let substituted_code = installed_code_with_fill(1, entry, 0xcc);
    assert_eq!(admitted_code.identity(), substituted_code.identity());
    assert_eq!(admitted_code.artifact(), substituted_code.artifact());

    let validated = validate_external_root(candidate_for_code(entry, &admitted_code), &boundary())
        .expect("root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &admitted_code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");

    let mut ledger =
        InstalledRootLedger::claim(&mut admitted_code).expect("canonical admitted-code ledger");
    let error = ledger
        .install(&substituted_code, validated, authority, admission)
        .expect_err("compact installed/artifact IDs cannot substitute exact code");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact installed-code occurrence and installation scope")
    );
}

#[test]
fn root_removal_receipt_cannot_substitute_colliding_installed_code() {
    let entry = entry_id(1001);
    let mut first_code = installed_code_with_fill(1, entry, 0x90);
    let mut second_code = installed_code_with_fill(1, entry, 0xcc);
    let first_root = validate_external_root(candidate_for_code(entry, &first_code), &boundary())
        .expect("first root plan");
    let second_root = validate_external_root(candidate_for_code(entry, &second_code), &boundary())
        .expect("second root plan");
    let first_execution = provider_execution(&first_root);
    let second_execution = provider_execution(&second_root);
    let first_slot = slot();
    let second_slot = slot();
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first_root,
        &first_execution,
        &first_code,
        &first_slot,
        first_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("first admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second_root,
        &second_execution,
        &second_code,
        &second_slot,
        second_root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second admission");

    let mut first_ledger =
        InstalledRootLedger::claim(&mut first_code).expect("first canonical root ledger");
    let first_installed = first_ledger
        .install(&first_code, first_root, first_slot, first_admission)
        .expect("first installed root");
    let mut second_ledger =
        InstalledRootLedger::claim(&mut second_code).expect("second canonical root ledger");
    let second_installed = second_ledger
        .install(&second_code, second_root, second_slot, second_admission)
        .expect("second installed root");
    let substituted_receipt = RootRemovalReceipt::from_provider(
        root_id(23, RootRemovalReceiptId::from_normalized_identity),
        &second_installed,
        true,
        true,
    );

    let error = first_ledger
        .remove(first_installed, substituted_receipt)
        .expect_err("root removal must bind exact installed code");
    assert!(error.diagnostic().0.contains("exact-slot"));
}

#[test]
fn install_rejects_foreign_entries_and_returns_every_consumed_authority() {
    let admitted_entry = entry_id(1001);
    let mut code = installed_code(1, admitted_entry);
    let foreign_entry = entry_id(1002);
    let validated =
        validate_external_root(candidate(foreign_entry), &boundary()).expect("root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let error = ledger
        .install(&code, validated, authority, admission)
        .expect_err("foreign entry must reject");

    assert!(error.diagnostic().0.contains("not in the admitted"));
    let (root, slot, admission) = error.into_parts();
    assert_eq!(root.candidate().entry, foreign_entry);
    assert_eq!(
        slot.slot(),
        root_id(20, RootSlotId::from_normalized_identity)
    );
    assert_eq!(
        admission.identity(),
        root_id(22, RootAdmissionId::from_normalized_identity)
    );
    assert_eq!(ledger.records().count(), 0);
}

#[test]
fn installation_registry_claim_is_one_shot_and_rejects_another_installation() {
    let entry = entry_id(1001);
    let mut first_code = installed_code(1, entry);
    let mut ledger =
        InstalledRootLedger::claim(&mut first_code).expect("first canonical root ledger");
    let replay = InstalledRootLedger::claim(&mut first_code)
        .expect_err("one installed-code occurrence cannot issue a second registry");
    assert!(replay.0.contains("already issued"));

    let second_code = installed_code(2, entry);
    let root = validate_external_root(candidate_for_code(entry, &second_code), &boundary())
        .expect("second-code root plan");
    let authority = slot();
    let execution = provider_execution(&root);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &root,
        &execution,
        &second_code,
        &authority,
        root.candidate().trust_receipts.iter().copied(),
    )
    .expect("second-code root admission");
    let error = ledger
        .install(&second_code, root, authority, admission)
        .expect_err("one installation registry cannot accept another installed-code occurrence");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact installed-code occurrence and installation scope")
    );
}

#[test]
fn root_admission_rejects_provider_execution_from_another_realization() {
    let first = validate_external_root(candidate(entry_id(1001)), &boundary())
        .expect("first root realization");
    let execution = provider_execution(&first);
    let second = validate_external_root(candidate(entry_id(1002)), &boundary())
        .expect("second root realization");
    let code = installed_code(2, entry_id(1002));
    let authority = slot();
    let error = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second,
        &execution,
        &code,
        &authority,
        second.candidate().trust_receipts.iter().copied(),
    )
    .expect_err("provider execution cannot be replayed for changed entry/resources");

    assert!(error.0.contains("exact validated root realization"));
}

#[test]
fn root_admission_rejects_execution_after_selected_plan_drift() {
    let entry = entry_id(1001);
    let first = validate_external_root(candidate(entry), &boundary())
        .expect("first selected provider plan");
    let execution = provider_execution(&first);
    let mut drifted = candidate(entry);
    drifted.provider_plan = root_id(56, ProviderPlanId::from_normalized_identity);
    let second =
        validate_external_root(drifted, &boundary()).expect("second selected provider plan");
    assert_ne!(
        first.normalized_report_identity(),
        second.normalized_report_identity()
    );

    let code = installed_code(2, entry);
    let authority = slot();
    let error = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second,
        &execution,
        &code,
        &authority,
        second.candidate().trust_receipts.iter().copied(),
    )
    .expect_err("provider execution cannot cross selected-plan drift");

    assert!(error.0.contains("exact validated root realization"));
}

#[test]
fn provider_execution_retains_exact_root_facts_beyond_the_compact_identity() {
    let entry = entry_id(1001);
    let first =
        validate_external_root(candidate(entry), &boundary()).expect("first root realization");
    let execution = provider_execution(&first);
    let mut drifted = candidate(entry);
    drifted
        .trust_receipts
        .insert(root_id(44, TrustReceiptId::from_normalized_identity));
    let mut second = validate_external_root(drifted, &boundary()).expect("second root realization");
    second.normalized_report_identity = first.normalized_report_identity;

    let code = installed_code(2, entry);
    let authority = slot();
    let error = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &second,
        &execution,
        &code,
        &authority,
        second.candidate().trust_receipts.iter().copied(),
    )
    .expect_err("equal compact identity cannot replay execution across exact-root drift");

    assert!(error.0.contains("exact validated root realization"));
}

#[test]
fn terminal_settlement_inherits_the_admitted_provider_execution() {
    let validated = validate_external_root(candidate(entry_id(1001)), &boundary()).expect("root");
    let execution = provider_execution(&validated);
    let binding = execution.binding();
    assert_eq!(
        binding.provider_plan_report_identity(),
        execution.provider_plan().normalized_identity()
    );
    assert_eq!(
        binding.provider_execution_report_identity(),
        execution.identity().normalized_identity()
    );
    assert_eq!(
        binding.provider_execution_report_fingerprint(),
        execution.normalized_report_identity()
    );
    assert_eq!(
        binding.normalized_root_report_identity(),
        validated.normalized_report_identity()
    );
    assert_eq!(
        binding.boundary_contract_report_fingerprint(),
        validated.boundary_contract_report_fingerprint()
    );
}

#[test]
fn slot_admission_retains_the_exact_validated_root() {
    let entry = entry_id(1001);
    let first =
        validate_external_root(candidate(entry), &boundary()).expect("first root realization");
    let mut code = installed_code(1, entry);
    let authority = slot();
    let execution = provider_execution(&first);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &first,
        &execution,
        &code,
        &authority,
        first.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");

    let mut drifted = candidate(entry);
    drifted.acknowledgement_policy = None;
    let mut second = validate_external_root(drifted, &boundary()).expect("second root realization");
    second.normalized_report_identity = first.normalized_report_identity;
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let error = ledger
        .install(&code, second, authority, admission)
        .expect_err("equal compact identity cannot replay admission across root-policy drift");

    assert!(
        error
            .diagnostic()
            .0
            .contains("does not bind the exact root")
    );
}

#[test]
fn removal_requires_both_unreachability_and_execution_quiescence() {
    let entry = entry_id(1001);
    let mut code = installed_code(1, entry);
    let validated = validate_external_root(candidate(entry), &boundary()).expect("root plan");
    let authority = slot();
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(22, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        &code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let installed = ledger
        .install(&code, validated, authority, admission)
        .expect("installed external root");
    let receipt = RootRemovalReceipt::from_provider(
        root_id(23, RootRemovalReceiptId::from_normalized_identity),
        &installed,
        true,
        false,
    );
    let error = ledger
        .remove(installed, receipt)
        .expect_err("live executions prevent slot reuse");
    assert!(error.diagnostic().0.contains("quiescence"));
    assert_eq!(ledger.records().count(), 1);
    let (installed, _) = error.into_parts();
    assert_eq!(installed.installed_code(), code.identity());
}

#[test]
fn independent_resource_columns_are_validated_before_ledger_entry() {
    let invalid_summary = ProviderStackSummary::from_admitted_provider(
        root_id(1, ExternalRootId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        EntryStack::ProviderSelected,
        2048,
        3,
        root_id(49, StackValidationReceiptId::from_normalized_identity),
    );
    let error = compose_artifact_stacks(
        &StackNestingRelation {
            identity: root_id(6, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&invalid_summary],
    )
    .expect_err("bad WCSU alignment");
    assert!(error.0.contains("power of two"));

    let mut over_stack = candidate(entry_id(1001));
    over_stack.stack.ceiling_bytes = 2047;
    let error = validate_external_root(over_stack, &boundary()).expect_err("stack ceiling");
    assert!(error.0.contains("stack ceiling"));

    let mut wrong_root = candidate(entry_id(1001));
    wrong_root.stack.realization = stack_demand(
        root_id(99, ExternalRootId::from_normalized_identity),
        root_id(2, RootProviderId::from_normalized_identity),
        root_id(6, NestingRelationId::from_normalized_identity),
        &boundary(),
        &installed_code(1, entry_id(1001)),
        entry_id(1001),
        EntryStack::Interrupted,
        2048,
    );
    let error = validate_external_root(wrong_root, &boundary()).expect_err("wrong stack root");
    assert!(error.0.contains("candidate root"));

    let mut over_work = candidate(entry_id(1001));
    over_work.logical_fuel.ceiling_units = 6;
    let error = validate_external_root(over_work, &boundary()).expect_err("logical-fuel ceiling");
    assert!(error.0.contains("logical fuel"));

    let mut wrong_fuel_schedule = candidate(entry_id(1001));
    wrong_fuel_schedule.logical_fuel.schedule =
        FuelScheduleIdentity::new(2).expect("different fuel schedule");
    let error = validate_external_root(wrong_fuel_schedule, &boundary())
        .expect_err("fuel provision cannot reinterpret another schedule's units");
    assert!(error.0.contains("different schedule versions"));

    let mut wrong_state = candidate(entry_id(1001));
    wrong_state.machine_state.realization = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::Aarch64X(0)]),
        MachineStateSet::empty(),
    );
    let error = validate_external_root(wrong_state, &boundary()).expect_err("state ceiling");
    assert!(error.0.contains("machine-state"));

    let mut conflicting = candidate(entry_id(1001));
    conflicting.component_pins.insert(ComponentVersionPin {
        contract: root_id(8, ComponentContractId::from_normalized_identity),
        artifact: root_id(90, ComponentArtifactId::from_normalized_identity),
        provider: root_id(91, ComponentProviderId::from_normalized_identity),
        version: root_id(92, ComponentVersionPinId::from_normalized_identity),
    });
    let error = validate_external_root(conflicting, &boundary())
        .expect_err("one contract cannot pin two component realizations");
    assert!(error.0.contains("more than one realization"));
}

#[test]
fn cathedral_irq_stack_is_maximum_root_plus_current_stack_fault() {
    let timer = root_id(100, ExternalRootId::from_normalized_identity);
    let keyboard = root_id(101, ExternalRootId::from_normalized_identity);
    let fatal_fault = root_id(102, ExternalRootId::from_normalized_identity);
    let double_fault = root_id(103, ExternalRootId::from_normalized_identity);
    let relation_identity = root_id(110, NestingRelationId::from_normalized_identity);
    let irq_provider = root_id(120, RootProviderId::from_normalized_identity);
    let fault_provider = root_id(121, RootProviderId::from_normalized_identity);
    let receipt = |identity| root_id(identity, StackValidationReceiptId::from_normalized_identity);
    let timer_summary = ProviderStackSummary::from_admitted_provider(
        timer,
        irq_provider,
        EntryStack::Dedicated { class: 4 },
        2048,
        16,
        receipt(130),
    );
    let keyboard_summary = ProviderStackSummary::from_admitted_provider(
        keyboard,
        irq_provider,
        EntryStack::Dedicated { class: 4 },
        1536,
        16,
        receipt(131),
    );
    let fatal_fault_summary = ProviderStackSummary::from_admitted_provider(
        fatal_fault,
        fault_provider,
        EntryStack::Interrupted,
        1024,
        16,
        receipt(132),
    );
    let double_fault_summary = ProviderStackSummary::from_admitted_provider(
        double_fault,
        fault_provider,
        EntryStack::Dedicated { class: 1 },
        4096,
        64,
        receipt(133),
    );
    let relation = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([
            StackNestingEdge {
                interrupted: timer,
                preemptor: fatal_fault,
            },
            StackNestingEdge {
                interrupted: timer,
                preemptor: double_fault,
            },
            StackNestingEdge {
                interrupted: keyboard,
                preemptor: fatal_fault,
            },
        ]),
    };

    let forward = compose_artifact_stacks(
        &relation,
        [
            &timer_summary,
            &keyboard_summary,
            &fatal_fault_summary,
            &double_fault_summary,
        ],
    )
    .expect("Cathedral stack composition");
    let reverse = compose_artifact_stacks(
        &relation,
        [
            &double_fault_summary,
            &fatal_fault_summary,
            &keyboard_summary,
            &timer_summary,
        ],
    )
    .expect("order-independent Cathedral stack composition");

    assert_eq!(forward, reverse);
    assert_eq!(
        forward
            .demand(timer)
            .expect("timer WCSU")
            .composed_wcsu_bytes(),
        3072
    );
    assert_eq!(
        forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 4 }),
        Some(3072)
    );
    assert_eq!(
        forward.domain_wcsu_bytes(StackDomain::Dedicated { class: 1 }),
        Some(4096)
    );
    assert_eq!(
        forward
            .demand(timer)
            .expect("timer WCSU")
            .contributing_roots(),
        &BTreeSet::from([timer, fatal_fault])
    );

    let nested_maskable = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([StackNestingEdge {
            interrupted: timer,
            preemptor: keyboard,
        }]),
    };
    let error = compose_artifact_stacks(&nested_maskable, [&timer_summary, &keyboard_summary])
        .expect_err("shared dedicated IRQ stack cannot be re-entered");
    assert!(error.0.contains("re-enters active dedicated class 4"));

    let missing = compose_artifact_stacks(&relation, [&timer_summary])
        .expect_err("every nesting endpoint needs a provider stack summary");
    assert!(missing.0.contains("missing"));

    let cyclic = StackNestingRelation {
        identity: relation_identity,
        edges: BTreeSet::from([
            StackNestingEdge {
                interrupted: timer,
                preemptor: fatal_fault,
            },
            StackNestingEdge {
                interrupted: fatal_fault,
                preemptor: timer,
            },
        ]),
    };
    let error = compose_artifact_stacks(&cyclic, [&timer_summary, &fatal_fault_summary])
        .expect_err("recursive nesting is not a finite WCSU");
    assert!(error.0.contains("cycle"));
}

#[test]
fn stack_composition_retains_exact_inputs_beyond_compact_fingerprints() {
    let root = root_id(140, ExternalRootId::from_normalized_identity);
    let nested = root_id(141, ExternalRootId::from_normalized_identity);
    let relation_identity = root_id(142, NestingRelationId::from_normalized_identity);
    let root_summary = ProviderStackSummary::from_admitted_provider(
        root,
        root_id(143, RootProviderId::from_normalized_identity),
        EntryStack::Dedicated { class: 4 },
        1024,
        16,
        root_id(144, StackValidationReceiptId::from_normalized_identity),
    );
    let nested_summary = ProviderStackSummary::from_admitted_provider(
        nested,
        root_id(145, RootProviderId::from_normalized_identity),
        EntryStack::Dedicated { class: 1 },
        2048,
        16,
        root_id(146, StackValidationReceiptId::from_normalized_identity),
    );
    let without_edge = compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::new(),
        },
        [&root_summary, &nested_summary],
    )
    .expect("independent roots");
    let with_edge = compose_artifact_stacks(
        &StackNestingRelation {
            identity: relation_identity,
            edges: BTreeSet::from([StackNestingEdge {
                interrupted: root,
                preemptor: nested,
            }]),
        },
        [&root_summary, &nested_summary],
    )
    .expect("dedicated nested root");

    let exact = without_edge.demand(root).expect("root demand");
    let mut collided = with_edge.demand(root).expect("root demand").clone();
    collided.non_authoritative_artifact_composition_report_fingerprint =
        exact.non_authoritative_artifact_composition_report_fingerprint;
    collided.non_authoritative_composition_report_fingerprint =
        exact.non_authoritative_composition_report_fingerprint;

    assert_eq!(exact.composed_wcsu_bytes, collided.composed_wcsu_bytes);
    assert_eq!(exact.contributing_roots, collided.contributing_roots);
    assert_ne!(
        exact, &collided,
        "compact fingerprint collision cannot erase exact nesting evidence"
    );
}

#[test]
fn fixed_fuel_composition_is_transitive_canonical_and_fails_closed() {
    assert_eq!(FuelScheduleIdentity::new(0), None);

    let leaf_identity = root_id(61, ProviderFuelSummaryId::from_normalized_identity);
    let root_identity = root_id(60, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(62, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            63,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(2, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 2,
        }]),
        root_id(
            64,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );

    let forward = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("composition");
    let reverse = compose_fixed_fuel(root_identity, [&leaf, &root]).expect("composition");
    assert_eq!(forward.units(), 11);
    assert_eq!(forward.schedule(), fuel_schedule());
    assert_eq!(forward, reverse);
    assert_eq!(forward.summaries().len(), 2);
    assert_eq!(forward.provider_receipts().len(), 2);

    let error = compose_fixed_fuel(root_identity, [&root]).expect_err("missing callee");
    assert!(error.0.contains("missing"));

    let mismatched_leaf = FixedFuelProviderSummary {
        local_evidence: FixedFuelLocalEvidence::AdmittedProvider {
            schedule: FuelScheduleIdentity::new(2).expect("different fuel schedule"),
            units: 4,
            validation_receipt: root_id(
                63,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        },
        ..leaf.clone()
    };
    let error = compose_fixed_fuel(root_identity, [&root, &mismatched_leaf])
        .expect_err("mixed fuel schedules must not compose");
    assert!(error.0.contains("schedule version"));

    let cyclic_leaf = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: root_identity,
            maximum_invocations: 1,
        }]),
        ..leaf
    };
    let error =
        compose_fixed_fuel(root_identity, [&root, &cyclic_leaf]).expect_err("cyclic fuel graph");
    assert!(error.0.contains("cycle"));
}

#[test]
fn fixed_fuel_composition_retains_exact_graph_beyond_compact_fingerprint() {
    let leaf_identity = root_id(71, ProviderFuelSummaryId::from_normalized_identity);
    let root_identity = root_id(70, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(72, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            73,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(74, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 2,
        }]),
        root_id(
            75,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let exact = compose_fixed_fuel(root_identity, [&root, &leaf]).expect("original fuel graph");

    let drifted_leaf = FixedFuelProviderSummary {
        local_evidence: FixedFuelLocalEvidence::AdmittedProvider {
            schedule: fuel_schedule(),
            units: 2,
            validation_receipt: root_id(
                73,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        },
        ..leaf
    };
    let drifted_root = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 4,
        }]),
        ..root
    };
    let mut collided = compose_fixed_fuel(root_identity, [&drifted_root, &drifted_leaf])
        .expect("equal-total drifted fuel graph");
    collided.non_authoritative_composition_report_fingerprint =
        exact.non_authoritative_composition_report_fingerprint;

    assert_eq!(exact.units, collided.units);
    assert_eq!(exact.summaries, collided.summaries);
    assert_eq!(exact.provider_receipts, collided.provider_receipts);
    assert_ne!(
        exact, collided,
        "compact fingerprint collision cannot erase exact fuel-graph evidence"
    );
}

#[test]
fn cathedral_first_timer_profile_is_five_fixed_one_shot_nodes() {
    // Cathedral's first hard timer root does exactly four provider-facing
    // operations before its deriver-owned return: acknowledge the source,
    // capture the clock, set one preallocated coalescing wake state, and
    // return. Every edge is one-shot; application timer draining remains
    // outside this hard-root graph.
    let root_identity = root_id(100, ProviderFuelSummaryId::from_normalized_identity);
    let acknowledge_identity = root_id(101, ProviderFuelSummaryId::from_normalized_identity);
    let clock_identity = root_id(102, ProviderFuelSummaryId::from_normalized_identity);
    let wake_identity = root_id(103, ProviderFuelSummaryId::from_normalized_identity);
    let return_identity = root_id(104, ProviderFuelSummaryId::from_normalized_identity);

    let leaf = |identity, provider_identity, receipt_identity| {
        FixedFuelProviderSummary::from_admitted_provider(
            identity,
            root_id(provider_identity, RootProviderId::from_normalized_identity),
            fuel_schedule(),
            1,
            BTreeSet::new(),
            root_id(
                receipt_identity,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        )
    };
    let acknowledge = leaf(acknowledge_identity, 201, 301);
    let clock = leaf(clock_identity, 202, 302);
    let wake = leaf(wake_identity, 203, 303);
    let return_path = leaf(return_identity, 204, 304);
    let timer = FixedFuelProviderSummary::from_admitted_provider(
        root_identity,
        root_id(200, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        1,
        BTreeSet::from([
            FixedFuelCall {
                callee: acknowledge_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: clock_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: wake_identity,
                maximum_invocations: 1,
            },
            FixedFuelCall {
                callee: return_identity,
                maximum_invocations: 1,
            },
        ]),
        root_id(
            300,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );

    let forward = compose_fixed_fuel(
        root_identity,
        [&timer, &acknowledge, &clock, &wake, &return_path],
    )
    .expect("the first Cathedral timer profile is finite fixed work");
    let reverse = compose_fixed_fuel(
        root_identity,
        [&return_path, &wake, &clock, &acknowledge, &timer],
    )
    .expect("presentation order cannot change the timer profile");
    assert_eq!(forward, reverse);
    assert_eq!(forward.units(), 5);
    assert_eq!(
        forward.summaries(),
        &BTreeSet::from([
            root_identity,
            acknowledge_identity,
            clock_identity,
            wake_identity,
            return_identity,
        ])
    );
    assert_eq!(forward.provider_receipts().len(), 5);

    let recursive_acknowledge = FixedFuelProviderSummary {
        calls: BTreeSet::from([FixedFuelCall {
            callee: root_identity,
            maximum_invocations: 1,
        }]),
        ..acknowledge.clone()
    };
    let error = compose_fixed_fuel(
        root_identity,
        [&timer, &recursive_acknowledge, &clock, &wake, &return_path],
    )
    .expect_err("a recursive acknowledgement provider cannot hide behind the timer root");
    assert!(error.0.contains("cycle"));

    let error = compose_fixed_fuel(root_identity, [&timer, &acknowledge, &clock, &return_path])
        .expect_err("a timer provider cannot omit its wake summary");
    assert!(error.0.contains("missing"));
}

fn progress_installation_fixture() -> (
    SelectedProviderPlanFacts,
    ComponentProgressManifest,
    u64,
    u64,
    ServiceProgressEstablishmentRoute,
) {
    let route = ServiceProgressEstablishmentRoute {
        kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
        requirement_identity: "SchedulerAdmission::grant_weak_fair#exact".into(),
    };
    let scheduler = ProviderPlan {
        name: "scheduler-plan".into(),
        provider_type: "SchedulerProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "Scheduler".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "wait".into(),
                requirement_owner: "Scheduler".into(),
                requirement_identity: "Scheduler::wait#exact".into(),
                service_reach: vec!["Scheduler".into()],
                may_suspend: true,
                terminates_guarantee: true,
                termination_premises: vec![ServiceProgressPremise {
                    profile: "SchedulerHandle::WeakFair".into(),
                    subject: ServiceProgressSubject::ProviderReceiver,
                    subject_projections: vec!["queue".into()],
                    establishment_routes: vec![route.clone()],
                }],
                ..ServiceMethod::default()
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "wait".into(),
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CompilerIntrinsic {
                machine: "TestScheduler::wait".into(),
            },
        }],
        origin_package_identity: None,
        origin_package: "omega::test".into(),
    };
    let admission = ProviderPlan {
        name: "scheduler-admission-plan".into(),
        provider_type: "SchedulerAdmissionProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "SchedulerAdmission".into(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "grant_weak_fair".into(),
                requirement_owner: "SchedulerAdmission".into(),
                requirement_identity: route.requirement_identity.clone(),
                parameter_count: 1,
                parameter_type_identities: vec!["SchedulerHandle".into()],
                has_result: true,
                result_type_identity: Some("SchedulerHandle in WeakFair".into()),
                service_reach: vec!["SchedulerAdmission".into()],
                terminates_guarantee: true,
                ..ServiceMethod::default()
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "grant_weak_fair".into(),
            requirement_identity: route.requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CompilerIntrinsic {
                machine: "TestSchedulerAdmission::grant_weak_fair".into(),
            },
        }],
        origin_package_identity: None,
        origin_package: "omega::test".into(),
    };
    let scheduler_identity = scheduler.report_fingerprint();
    let admission_identity = admission.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selection(
        &[scheduler, admission],
        &["scheduler-plan".into(), "scheduler-admission-plan".into()],
    )
    .expect("exact selected provider closure");
    let manifest = ComponentProgressManifest::bind(
        "Application::start".into(),
        &selected,
        vec![CheckedComponentProgressDemand {
            provider_service_identity: "Scheduler".into(),
            provider_service_package_identity: None,
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_owner_package_identity: None,
            profile_identity: "SchedulerHandle::WeakFair".into(),
            subject_projections: vec!["queue".into()],
            origin_callable_identity: "Application::start".into(),
            origin_state_identity: "Application::start::entry".into(),
            statement_ordinal: 4,
            call_ordinal: 1,
        }],
    )
    .expect("component progress manifest");
    (
        selected,
        manifest,
        scheduler_identity,
        admission_identity,
        route,
    )
}

fn provider_occurrence_binding(
    code: &InstalledCode,
    selected: &SelectedProviderPlanFacts,
    plan: u64,
    receipt: u64,
    occurrence: u64,
    provider: &str,
) -> ProviderOccurrencePlanBinding {
    ProviderOccurrencePlanBinding::new(
        plan,
        selected
            .plan_by_report_fingerprint(plan)
            .expect("fixture plan belongs to the selected closure")
            .clone(),
        ProviderOccurrenceInstallationReceipt::from_provider(
            root_id(
                receipt,
                ProviderOccurrenceInstallationReceiptId::from_normalized_identity,
            ),
            code,
            root_id(
                occurrence,
                InstalledProviderOccurrenceId::from_normalized_identity,
            ),
            provider,
        ),
    )
}

fn admitted_progress_receipt(
    ledger: &mut InstalledRootLedger,
    code: &InstalledCode,
    subject: u64,
    issuer: u64,
    issuer_plan: u64,
    seed: u64,
    route: ServiceProgressEstablishmentRoute,
) -> AdmittedProgressProfileEstablishment {
    ledger
        .admit_progress_profile_establishment(
            ProgressProfileEstablishmentAttestation::from_provider(
                root_id(
                    seed,
                    ProgressProfileEstablishmentReceiptId::from_normalized_identity,
                ),
                code,
                root_id(
                    subject,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                root_id(
                    issuer,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                issuer_plan,
                root_id(
                    seed + 1,
                    ProgressProfileGrantInvocationId::from_normalized_identity,
                ),
                "SchedulerHandle::WeakFair",
                vec!["queue".into()],
                route,
            ),
        )
        .expect("admitted establishment receipt")
}

#[test]
fn component_progress_seals_against_distinct_exact_subject_and_issuer_occurrences() {
    let (selected, manifest, scheduler_plan, admission_plan, route) =
        progress_installation_fixture();
    let mut code = installed_code(54_000, entry_id(54_001));
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    54_010,
                    54_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    54_011,
                    54_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("installed provider closure");
    let receipt = admitted_progress_receipt(
        &mut ledger,
        &code,
        54_020,
        54_021,
        admission_plan,
        54_030,
        route,
    );
    assert_ne!(receipt.subject(), receipt.issuer());

    let demand = ComponentProgressDemandIdentity::from_demand(&manifest.pending()[0]);
    let acceptance = ledger
        .seal_component_progress(
            manifest.clone(),
            [ComponentProgressReceiptBinding::new(
                demand,
                receipt.clone(),
            )],
        )
        .expect("exact component progress closure");
    assert_eq!(acceptance.manifest(), &manifest);
    assert_eq!(acceptance.receipts().collect::<Vec<_>>(), vec![&receipt]);
    assert!(acceptance.binds_installed_code(&code));
    let mut expected_provider_plans = vec![scheduler_plan, admission_plan];
    expected_provider_plans.sort_unstable();
    assert_eq!(
        acceptance.selected_provider_plan_report_identities(),
        expected_provider_plans.as_slice()
    );
    let colliding_code = installed_code_with_fill(54_000, entry_id(54_001), 1);
    assert!(!acceptance.binds_installed_code(&colliding_code));
    assert_ne!(acceptance.non_authoritative_report_fingerprint(), 0);
}

#[test]
fn component_progress_sealing_is_transactional_and_receipt_facts_are_reusable() {
    let (selected, _, scheduler_plan, admission_plan, route) = progress_installation_fixture();
    let demands = [(4, 1), (9, 2)]
        .into_iter()
        .map(
            |(statement_ordinal, call_ordinal)| CheckedComponentProgressDemand {
                provider_service_identity: "Scheduler".into(),
                provider_service_package_identity: None,
                requirement_identity: "Scheduler::wait#exact".into(),
                requirement_owner_package_identity: None,
                profile_identity: "SchedulerHandle::WeakFair".into(),
                subject_projections: vec!["queue".into()],
                origin_callable_identity: "Application::start".into(),
                origin_state_identity: "Application::start::entry".into(),
                statement_ordinal,
                call_ordinal,
            },
        )
        .collect::<Vec<_>>();
    let manifest = ComponentProgressManifest::bind("Application::start".into(), &selected, demands)
        .expect("two-demand manifest");
    let mut code = installed_code(55_000, entry_id(55_001));
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    55_010,
                    55_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    55_011,
                    55_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("installed provider closure");
    let receipt = admitted_progress_receipt(
        &mut ledger,
        &code,
        55_020,
        55_021,
        admission_plan,
        55_030,
        route,
    );

    let missing = ledger
        .seal_component_progress(
            manifest.clone(),
            [ComponentProgressReceiptBinding::new(
                ComponentProgressDemandIdentity::from_demand(&manifest.pending()[0]),
                receipt.clone(),
            )],
        )
        .expect_err("partial closure must reject");
    assert!(missing.diagnostic().0.contains("exactly cover"));

    let bindings = manifest
        .pending()
        .iter()
        .map(|demand| {
            ComponentProgressReceiptBinding::new(
                ComponentProgressDemandIdentity::from_demand(demand),
                receipt.clone(),
            )
        })
        .collect::<Vec<_>>();
    let acceptance = ledger
        .seal_component_progress(manifest, bindings)
        .expect("corrected retry succeeds without burning acceptance");
    assert_eq!(acceptance.receipts().count(), 2);
}

#[test]
fn provider_occurrence_and_progress_route_admission_fail_closed() {
    let (selected, _, scheduler_plan, admission_plan, route) = progress_installation_fixture();
    let mut code = installed_code(56_000, entry_id(56_001));
    let other_code = installed_code_with_fill(56_000, entry_id(56_001), 1);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    let error = ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &other_code,
                    &selected,
                    scheduler_plan,
                    56_010,
                    56_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    56_011,
                    56_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect_err("colliding compact IDs cannot substitute exact installed evidence");
    assert!(error.0.contains("different installed-code"));

    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    56_010,
                    56_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    56_011,
                    56_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("failed closure did not mutate the ledger");
    let wrong_route = ServiceProgressEstablishmentRoute {
        requirement_identity: "SchedulerAdmission::grant_other#exact".into(),
        ..route
    };
    let error = ledger
        .admit_progress_profile_establishment(
            ProgressProfileEstablishmentAttestation::from_provider(
                root_id(
                    56_030,
                    ProgressProfileEstablishmentReceiptId::from_normalized_identity,
                ),
                &code,
                root_id(
                    56_020,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                root_id(
                    56_021,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                admission_plan,
                root_id(
                    56_031,
                    ProgressProfileGrantInvocationId::from_normalized_identity,
                ),
                "SchedulerHandle::WeakFair",
                vec!["queue".into()],
                wrong_route,
            ),
        )
        .expect_err("issuer plan must realize the exact route");
    assert!(error.diagnostic().0.contains("exact requirement"));
}

#[test]
fn progress_receipt_identity_and_grant_invocation_cannot_be_rebound() {
    let (selected, _, scheduler_plan, admission_plan, route) = progress_installation_fixture();
    let mut code = installed_code(57_000, entry_id(57_001));
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    57_010,
                    57_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    57_011,
                    57_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("installed provider closure");
    let receipt_identity = root_id(
        57_030,
        ProgressProfileEstablishmentReceiptId::from_normalized_identity,
    );
    let invocation = root_id(
        57_031,
        ProgressProfileGrantInvocationId::from_normalized_identity,
    );
    let attestation = |receipt, invocation, profile: &str| {
        ProgressProfileEstablishmentAttestation::from_provider(
            receipt,
            &code,
            root_id(
                57_020,
                InstalledProviderOccurrenceId::from_normalized_identity,
            ),
            root_id(
                57_021,
                InstalledProviderOccurrenceId::from_normalized_identity,
            ),
            admission_plan,
            invocation,
            profile,
            vec!["queue".into()],
            route.clone(),
        )
    };
    let admitted = ledger
        .admit_progress_profile_establishment(attestation(
            receipt_identity,
            invocation,
            "SchedulerHandle::WeakFair",
        ))
        .expect("first receipt admission");
    assert_eq!(
        ledger
            .admit_progress_profile_establishment(attestation(
                receipt_identity,
                invocation,
                "SchedulerHandle::WeakFair",
            ))
            .expect("exact replay is idempotent"),
        admitted
    );
    let divergent = ledger
        .admit_progress_profile_establishment(attestation(
            receipt_identity,
            invocation,
            "SchedulerHandle::StrongFair",
        ))
        .expect_err("one receipt identity cannot name another profile");
    assert!(divergent.diagnostic().0.contains("divergent evidence"));
    let second_receipt = root_id(
        57_032,
        ProgressProfileEstablishmentReceiptId::from_normalized_identity,
    );
    let duplicate_invocation = ledger
        .admit_progress_profile_establishment(attestation(
            second_receipt,
            invocation,
            "SchedulerHandle::WeakFair",
        ))
        .expect_err("one grant invocation cannot mint another receipt");
    assert!(
        duplicate_invocation
            .diagnostic()
            .0
            .contains("grant invocation")
    );
}

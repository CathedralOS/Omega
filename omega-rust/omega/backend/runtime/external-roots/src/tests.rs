//! Fixtures shared by the external-root tests: fuel schedules, installed
//! code, boundary plans, candidates and slots with fixed seeds.

mod entry_stack_epochs;
mod interrupt_entries;
mod program_local_epochs;
mod program_local_extents;
mod program_local_root_joins;
mod progress_profiles;
mod provider_execution;
mod required_root_closures;
mod root_installation;
mod stack_and_fuel_composition;

use crate::{
    AcknowledgementPolicyId, AdmittedOpaqueArrivalContextSet, AdmittedProgressProfileEstablishment,
    ArtifactId, BoundEpochStackComposition, BoundEpochStackCompositionInput, ComponentArtifactId,
    ComponentContractId, ComponentProviderId, ComponentVersionPin, ComponentVersionPinId,
    ComposedFuelDemand, ExternalRootCandidate, ExternalRootDiagnostic, ExternalRootEntryClaim,
    ExternalRootId, ExternalRootResultClaim, FixedFuelCall, FixedFuelProviderSummary,
    FuelProvisionId, FuelScheduleIdentity, FuelValidationReceiptId,
    GeneratedProgramStorageAdapterLiveFrameDemand, InstalledCodeId, InstalledExternalRoot,
    InstalledProgramLocalRootOccurrence, InstalledProgramLocalRootSubject,
    InstalledProviderOccurrenceId, InstalledRootLedger, InterruptAcknowledgementId,
    InterruptEntryReceipt, InterruptEntryReceiptId, InterruptInvocationId, InterruptMaskControlId,
    InterruptMaskStateId, LogicalFuelResourceColumn, MachineStateResourceColumn, NestingRelationId,
    OpaqueProviderExitAssurance, ProgramLocalRootCohortMember, ProgramLocalRootCohortSealError,
    ProgramLocalRootEntryInvocationId, ProgramLocalRootInstallationLedger,
    ProgramLocalRootPrebindingId, ProgramLocalRootScalarBinding, ProgramLocalRootSubjectPlaceId,
    ProgressProfileEstablishmentAttestation, ProgressProfileEstablishmentReceiptId,
    ProgressProfileGrantInvocationId, ProviderExecution, ProviderExecutionId,
    ProviderFuelSummaryId, ProviderFuelValidationReceiptId, ProviderOccurrenceInstallationReceipt,
    ProviderOccurrenceInstallationReceiptId, ProviderOccurrencePlanBinding, ProviderPlanId,
    ProviderStackSummary, ResolvedRootServiceReach, RootAdmission, RootAdmissionId, RootEffectId,
    RootProviderId, RootSlotAuthority, RootSlotId, RootSlotOwnerId, StackNestingRelation,
    StackResourceColumn, StackValidationReceiptId, StateValidationReceiptId,
    TargetRequiredRootSlotSelection, TrustReceiptId, ValidatedExternalRoot,
    VerifiedRequiredRootSlotClosure, X86_64GeneratedProgramStorageAdapterEmission,
    admit_opaque_arrival_context_set, bind_installed_entry_stack,
    bind_opaque_adapter_stack_realization,
    bind_x86_64_generated_program_storage_adapter_stack_realization,
    compose_bound_entry_stack_epochs, compose_fixed_fuel,
    derive_generated_program_storage_adapter_live_frame_demand, validate_external_root,
    verify_target_required_root_slot_closure,
};
use calling_conventions::BoundaryEntryPlan;
use calling_conventions::EntryControl;
use calling_conventions::EntryStack;
use calling_conventions::MachineRegister;
use calling_conventions::ProviderExitRealization;
use calling_conventions::StateFootprintEvidence;
use calling_conventions::ValidatedBoundaryEntryPlan;
use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, CallSignature,
    CallingPolicy, EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineRegime,
    MachineState, MachineStateSet, Preemption, RegisterSet, StackDomainRef, StatePlan,
    ValidatedEntryStackDomainClosure, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan, validate_entry_stack_domain_closure,
    validate_entry_stack_realization,
};
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceSchema,
};
use effects::{
    CheckedComponentProgressDemand, ComponentEraCandidate, ComponentEraEntryLedger,
    ComponentEraLedgerId, ComponentEraPublicationReceipt, ComponentProgressManifest,
    ExecutableTcbManifest, ExecutableTcbProfile, ExecutableTcbProfileAcceptance, ExecutionScope,
    IncompleteScopePolicy, ProgramLocalRootEpochLeaseId, ScopeCompleteness,
    SelectedProviderPlanFacts, evaluate_executable_tcb_profile,
};
use executable_installation::InstalledCode;
use executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, CodePlacementAuthority,
    CodePlacementId, EntrySetId, FinalValidationCertificate, FinalValidationId, InstallAuthority,
    InstallationAudience, InstallationReceipt, InstallationScopeId, MachineContractSetId,
    MachineFootprintId, MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement,
    admit_executable, install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use extents::{
    AddressSpaceId, Extent, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use installation_evidence::{ObjectEvidence, StackDemandEvidence};
use isa_x86_64::{
    canonical_x86_64_semantic_unit_wrapper_encoding_request,
    encode_x86_64_semantic_unit_wrapper_template,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
};
use layout_plans::EntryStubId;
use layout_plans::{
    ArtifactInstallationScopeId, ByteOrder, MaterializationWrite, PlacementAddressRange,
    PlacementConstraints, PlacementPhase, PlacementSite, PostHandoffWriterPlan,
    PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
};
use proof_admission::AdmissionProfile;
use std::collections::BTreeSet;
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalModule, TerminalRootServiceReach, VocabularyMarker,
    program_local_root_introduction_compatibility_report_identity,
};

pub(crate) fn root_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>,
) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
}

fn install_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(identity).expect("normalized installation identity")
}

pub(crate) fn extent_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, ExtentDiagnostic>,
) -> T {
    constructor(identity).expect("normalized extent identity")
}

pub(crate) fn extent_provider_issuance(seed: u64) -> extents::ExtentProviderIssuance {
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

pub(crate) fn installed_code(artifact_identity: u64, entry: EntryStubId) -> InstalledCode {
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
    installed_code_in_placement(
        artifact_identity,
        entry,
        bytes,
        installed_code_identity,
        target::Architecture::X86_64,
        constraints(),
        0x1000,
        4096,
    )
}

/// Installed-code fixture with explicit retained placement constraints and a
/// chosen realized extent. The default helper above pins the shared
/// unconstrained-regime site; secondary-processor startup tests need a
/// declared machine regime and a low-memory window instead. The fixture's
/// placement authority still cites installation scope 61, so a constrained
/// `installation_scope` must normalize to that identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn installed_code_in_placement(
    artifact_identity: u64,
    entry: EntryStubId,
    bytes: Vec<u8>,
    installed_code_identity: u64,
    architecture: target::Architecture,
    placement_constraints: PlacementConstraints,
    extent_base: u64,
    extent_length: u64,
) -> InstalledCode {
    installed_code_in_placement_with_entries(
        artifact_identity,
        bytes,
        installed_code_identity,
        architecture,
        placement_constraints,
        extent_base,
        extent_length,
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
    )
}

/// Installed-code fixture whose artifact admits several entries — an
/// interrupt table's members each occupy their own entry offset.
#[allow(clippy::too_many_arguments)]
pub(crate) fn installed_code_in_placement_with_entries(
    artifact_identity: u64,
    bytes: Vec<u8>,
    installed_code_identity: u64,
    architecture: target::Architecture,
    placement_constraints: PlacementConstraints,
    extent_base: u64,
    extent_length: u64,
    entries: Vec<ArtifactEntry>,
) -> InstalledCode {
    let artifact_constraints = placement_constraints;
    let contracts = install_id(30, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(31, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(artifact_identity, ArtifactId::from_normalized_identity),
        architecture,
        bytes,
        contracts,
        footprint,
        install_id(32, PlacementPlanId::from_normalized_identity),
        artifact_constraints,
        install_id(33, EntrySetId::from_normalized_identity),
        entries,
        install_id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            artifact_constraints
                .machine_regime()
                .map(|regime| (regime, b"test-machine-regime-v1".as_slice())),
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
    .mint(extent_base, extent_length)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(100, CodePlacementId::from_normalized_identity),
        install_id(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::FutureFetcher,
        &extent,
        rights,
        placement_constraints,
        PlacementSite {
            base_address: extent_base,
            phase: placement_constraints.phase(),
            machine_regime: placement_constraints.machine_regime(),
            installation_scope: placement_constraints.installation_scope(),
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

/// Separately provisioned per-processor state extent for the
/// secondary-processor startup tests: each mint stays in the shared fixture
/// address space (50) under a distinct lineage so overlap checks compare real
/// custody geometry.
pub(crate) fn minted_secondary_processor_state(
    identity_seed: u64,
    base: u64,
    length: u64,
) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(identity_seed),
        extent_id(
            identity_seed + 1000,
            ExtentLineageId::from_normalized_identity,
        ),
        extent_id(50, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([extent_id(
            51,
            ExtentRightId::from_normalized_identity,
        )]),
        extent_id(
            identity_seed + 2000,
            ExtentProvenanceId::from_normalized_identity,
        ),
        extent_id(identity_seed + 3000, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("minted secondary-processor state extent")
}

/// AP entry boundary fixture: begins in `initial_regime` under `policy` and
/// arrives on `stack`, matching the secondary-processor startup ledger's
/// admission contract.
pub(crate) fn secondary_processor_boundary(
    policy: CallingPolicy,
    initial_regime: MachineRegime,
    stack: EntryStack,
) -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary =
        evaluate_ordinary_boundary_entry_plan(policy, &signature).expect("ordinary boundary plan");
    let mut plan = ordinary.plan().clone();
    plan.state.initial_regime = initial_regime;
    plan.state.stack = stack;
    validate_boundary_entry_plan(plan, &signature).expect("secondary-processor entry boundary")
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

fn two_parameter_boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("validated two-parameter boundary")
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
    let request =
        canonical_x86_64_semantic_unit_wrapper_encoding_request(target::NativeTarget::uefi_x64());
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
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x8f; 32]),
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
    identity: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    bytes: Vec<u8>,
}

impl ObjectEvidence for TestObject {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }

    fn target(&self) -> target::NativeTarget {
        target::NativeTarget::linux_x64()
    }

    fn text_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn function_text_offset(&self, machine: semantic_vocabulary::MachineId) -> Option<usize> {
        (machine == self.entry).then_some(16)
    }
}

struct TestStackDemand {
    identity: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    contributing: BTreeSet<semantic_vocabulary::MachineId>,
    admitted_reports: BTreeSet<u64>,
    admitted_commitments: BTreeSet<[u8; 32]>,
}

impl StackDemandEvidence for TestStackDemand {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }

    fn architecture(&self) -> target::Architecture {
        target::Architecture::X86_64
    }

    fn entry(&self) -> semantic_vocabulary::MachineId {
        self.entry
    }

    fn ceiling_bytes(&self) -> u64 {
        64
    }

    fn stack_alignment(&self) -> u32 {
        16
    }

    fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId> {
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

/// One member's bound epoch input. Interrupt-table tests compose several of
/// these into the single artifact-wide composition every installed root of
/// one artifact must carry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_demand_input(
    root: ExternalRootId,
    provider: RootProviderId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    resolved_stack: EntryStack,
    local_wcsu_bytes: u64,
) -> BoundEpochStackCompositionInput {
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
    bind_opaque_adapter_stack_realization(&summary, boundary, code, entry, realization, contexts)
        .expect("test epoch evidence binding")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_demand(
    root: ExternalRootId,
    provider: RootProviderId,
    relation: NestingRelationId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    resolved_stack: EntryStack,
    local_wcsu_bytes: u64,
) -> BoundEpochStackComposition {
    let bound = stack_demand_input(
        root,
        provider,
        boundary,
        code,
        entry,
        resolved_stack,
        local_wcsu_bytes,
    );
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
    candidate_for_code_with_root_on_boundary(entry, code, root_identity, &boundary())
}

pub(crate) fn candidate_for_code_with_root_on_boundary(
    entry: EntryStubId,
    code: &InstalledCode,
    root_identity: u64,
    boundary: &ValidatedBoundaryEntryPlan,
) -> ExternalRootCandidate {
    let root = root_id(root_identity, ExternalRootId::from_normalized_identity);
    let provider = root_id(2, RootProviderId::from_normalized_identity);
    let nesting_relation = root_id(6, NestingRelationId::from_normalized_identity);
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
            &effects::SelectedProviderPlanFacts::default(),
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
                boundary,
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
) -> effects::SelectedProviderPlanFacts {
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
    effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("selected interrupt completion provider")
    .with_installation_reach_resolutions(vec![effects::InstallationReachResolution {
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

fn selected_interrupt_completion() -> effects::SelectedProviderPlanFacts {
    selected_interrupt_completion_for(
        "LegacyPic",
        "LegacyPicController",
        "LegacyPicController::complete",
        &["PortIo"],
    )
}

fn slot() -> RootSlotAuthority {
    RootSlotAuthority::from_admitted_owner(
        root_id(20, RootSlotId::from_normalized_identity),
        root_id(21, RootSlotOwnerId::from_normalized_identity),
    )
}

pub(crate) fn provider_execution(root: &ValidatedExternalRoot) -> ProviderExecution {
    provider_execution_for(root, 54)
}

/// Provider execution fixture with a caller-chosen identity so one ledger can
/// hold several roots whose executions stay distinct under the
/// invocation/acknowledgement replay keys.
pub(crate) fn provider_execution_for(
    root: &ValidatedExternalRoot,
    identity: u64,
) -> ProviderExecution {
    ProviderExecution::from_admitted_provider(
        root_id(identity, ProviderExecutionId::from_normalized_identity),
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
    install_test_root_pair_with_ids_unsealed_on_boundary(code, first, second, entry, &boundary())
}

fn install_test_root_pair_with_ids_unsealed_on_boundary<'code>(
    code: &'code mut InstalledCode,
    first: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    second: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    entry: EntryStubId,
    boundary: &ValidatedBoundaryEntryPlan,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    let mut first_candidate =
        candidate_for_code_with_root_on_boundary(entry, code, first.0, boundary);
    first_candidate.entry_claims = first.4;
    let mut second_candidate =
        candidate_for_code_with_root_on_boundary(entry, code, second.0, boundary);
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

    let first_validated =
        validate_external_root(first_candidate, boundary).expect("first root plan");
    let second_validated =
        validate_external_root(second_candidate, boundary).expect("second root plan");
    let target_profile = target::TargetProfile::UefiX64;
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
    let target_profile = target::TargetProfile::UefiX64;
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

fn install_program_local_two_parameter_roots<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    let (mut ledger, first, second) = install_test_root_pair_with_ids_unsealed_on_boundary(
        code,
        (1, 20, 21, 22, entry_claims.clone()),
        (101, 120, 121, 122, entry_claims),
        entry,
        &two_parameter_boundary(),
    );
    ledger
        .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
        .expect("installed required root-slot closure");
    (ledger, first, second)
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
    let entry = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let carrier = semantic_vocabulary::StructuralTypeId::new(1).expect("carrier identity");
    let qualification = semantic_vocabulary::StructuralDomainId::new(1).expect("domain identity");
    let algebra = semantic_vocabulary::ContentAlgebra {
        kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
        parameter: "ByteUnit".into(),
    };
    let capacity = semantic_vocabulary::ContentProjectionExpression::CountedQuantity(
        semantic_vocabulary::ContentProjectionScalar::Add(
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["length".into()],
            )),
            Box::new(semantic_vocabulary::ContentProjectionScalar::Natural(
                "1".into(),
            )),
        ),
    );
    let mut schema = terminal_psi::ProgramLocalRootIntroductionSchema {
        argument_index: 0,
        source_parameter_position: 0,
        qualification,
        carrier,
        projection: semantic_vocabulary::ContentProjectionIdentity {
            domain: semantic_vocabulary::ContentDomainId::new(1).expect("content domain identity"),
            projection_report_fingerprint:
                language_semantics::content::terminal_projection_report_fingerprint(
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
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry,
        structural_types: vec![StructuralTypeDeclaration {
            id: carrier,
            identity: "Region".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).expect("field identity"),
                    identity: "length".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(
                        semantic_vocabulary::ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                64,
                            )
                            .expect("u64 type"),
                        ),
                    ),
                }],
            },
        }],
        structural_domains: vec![StructuralDomainDeclaration {
            id: qualification,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1)
                .expect("semantic domain identity"),
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
            fixed_service_reach: Vec::new(),
            id: semantic_vocabulary::BoundaryMachineId::new(1).expect("boundary identity"),
            identity: "TestRoot::entry".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: semantic_vocabulary::PlaceId::new(1).expect("place identity"),
                position: 0,
                is_self: false,
                structural_type: carrier,
                multiplicity: StructuralMultiplicity::Linear,
                access: terminal_psi::StructuralAccess::Owned,
                qualifications: vec![qualification],
                projected_qualifications: Vec::new(),
            }],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain: qualification,
            }],
            program_local_root_introductions: vec![schema],
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
            crash_routes: Vec::new(),
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
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![terminal_psi::TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: entry,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: terminal_psi::TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: semantic_vocabulary::BlockId::new(1).expect("block identity"),
            blocks: vec![terminal_psi::Block {
                structural_parameters: Vec::new(),
                id: semantic_vocabulary::BlockId::new(1).expect("block identity"),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: terminal_psi::Terminator::ReturnUnit {
                    edge: semantic_vocabulary::EdgeId::new(1).expect("edge identity"),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: terminal_psi::MachineContract {
                id: semantic_vocabulary::ContractId::new(1).expect("contract identity"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn program_local_two_schema_module() -> TerminalModule {
    let mut module = program_local_root_module();
    let machine = &mut module.boundary_machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(2).expect("place identity"),
            position: 1,
            is_self: false,
            structural_type: machine.structural_parameters[0].structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: machine.structural_parameters[0].qualifications.clone(),
            projected_qualifications: Vec::new(),
        });
    machine.requires.push(StructuralDomainRequirement {
        argument_index: 1,
        domain: machine.structural_parameters[0].qualifications[0],
    });
    let mut second = machine.program_local_root_introductions[0].clone();
    second.argument_index = 1;
    second.source_parameter_position = 1;
    second.compatibility_report_identity =
        program_local_root_introduction_compatibility_report_identity(
            "TestRoot::entry",
            "Region::Owned",
            "Region",
            &second,
        );
    machine.program_local_root_introductions.push(second);
    module
}

fn program_local_extent_module() -> TerminalModule {
    let mut module = program_local_root_module();
    let algebra = semantic_vocabulary::ContentAlgebra {
        kind: semantic_vocabulary::ContentAlgebraKind::IntervalSet,
        parameter: "Nat".into(),
    };
    let capacity = semantic_vocabulary::ContentProjectionExpression::IntervalSet(vec![(
        semantic_vocabulary::ContentProjectionScalar::SubjectField(vec!["base".into()]),
        semantic_vocabulary::ContentProjectionScalar::Add(
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["base".into()],
            )),
            Box::new(semantic_vocabulary::ContentProjectionScalar::SubjectField(
                vec!["length".into()],
            )),
        ),
    )]);
    let schema = &mut module.boundary_machines[0].program_local_root_introductions[0];
    schema.algebra = algebra;
    schema.capacity = capacity;
    schema.projection.projection_report_fingerprint =
        language_semantics::content::terminal_projection_report_fingerprint(
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
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut module.structural_types[0].shape
    else {
        panic!("program-local test carrier is a record")
    };
    fields.push(StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(2).expect("base field identity"),
        identity: "base".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 type"),
        )),
    });
    module
}

fn program_local_terminal_object(module: &TerminalModule) -> TestObject {
    TestObject {
        identity: terminal_codec::terminal_psi_identity(module).expect("terminal program identity"),
        entry: module.entry,
        bytes: vec![0; 64],
    }
}

fn program_local_root_catalog(
    module: &TerminalModule,
) -> terminal_codec::VerifiedProgramLocalRootProducerCatalog {
    let proof = terminal_verifier::ProofBundle::default();
    let verified = terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default())
        .expect("program-local terminal module verifies");
    terminal_codec::VerifiedProgramLocalRootProducerCatalog::from_verified(&verified)
        .expect("verified program-local producer catalog")
}

fn program_local_claim() -> ExternalRootEntryClaim {
    program_local_claim_at(0)
}

fn program_local_claim_at(parameter_index: usize) -> ExternalRootEntryClaim {
    ExternalRootEntryClaim {
        parameter_index,
        domain: "Region::Owned".into(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
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
    artifact_occurrence_digest: installation_evidence::InstalledArtifactOccurrenceDigest,
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
    artifact_occurrence_digest: installation_evidence::InstalledArtifactOccurrenceDigest,
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
) -> effects::ProgramLocalRootEpochLease {
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
    program_local_subject_at(root, invocation, subject_place, 0, 0, length)
}

fn program_local_subject_at<'root, 'code>(
    root: &'root InstalledExternalRoot<'code>,
    invocation: u64,
    subject_place: u64,
    argument_index: u32,
    source_parameter_position: u32,
    length: Option<u64>,
) -> InstalledProgramLocalRootSubject<'root, 'code> {
    let scalars = length
        .into_iter()
        .map(|length| {
            ProgramLocalRootScalarBinding::subject_field(
                ["length"],
                numerics::bignum::BigInt::from_u64(length),
            )
            .expect("natural subject field")
        })
        .collect::<Vec<_>>();
    InstalledProgramLocalRootSubject::from_generated_entry(
        root,
        ProgramLocalRootEntryInvocationId::from_normalized_identity(invocation)
            .expect("entry invocation identity"),
        argument_index,
        source_parameter_position,
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
                numerics::bignum::BigInt::from_u64(base),
            )
            .expect("natural base field"),
            ProgramLocalRootScalarBinding::subject_field(
                ["length"],
                numerics::bignum::BigInt::from_u64(length),
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
) -> effects::ProgramLocalRootEpochLease {
    let [member]: [ProgramLocalRootCohortMember<'_, '_>; 1] = error
        .into_members()
        .try_into()
        .expect("single-member rejected cohort");
    member.into_parts().2
}

/// One actual installed backing extent: provider-issued authority over the
/// exact range a program-local root's interval capacity occupies. Materialize
/// consumes it into the held account and retirement returns it.
fn installed_backing_extent(seed: u64, base: u64, length: u64, mapping_era: u64) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(seed),
        extent_id(10 + seed, ExtentLineageId::from_normalized_identity),
        extent_id(10, AddressSpaceId::from_normalized_identity),
        ExtentRights::from_normalized_identities([
            extent_id(100, ExtentRightId::from_normalized_identity),
            extent_id(101, ExtentRightId::from_normalized_identity),
        ]),
        extent_id(20, ExtentProvenanceId::from_normalized_identity),
        extent_id(mapping_era, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("actual installed backing extent")
}

fn interrupt_boundary() -> ValidatedBoundaryEntryPlan {
    interrupt_boundary_on(EntryStack::Dedicated { class: 1 })
}

/// Interrupt-return boundary fixture arriving on `stack`. Table members use
/// distinct dedicated classes; the ordinary interrupt tests keep class 1.
pub(crate) fn interrupt_boundary_on(stack: EntryStack) -> ValidatedBoundaryEntryPlan {
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
                stack,
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
        effective_carry: language_semantics::CarryPolicy::STRICT,
    }];
    candidate.acknowledgement_parameter_index = Some(0);
    candidate.interrupt_mask_guard_claim = Some(ExternalRootResultClaim {
        provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "InterruptMaskControl::save_and_mask".into(),
        domain: "InterruptMaskGuard::Active".into(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
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

/// Interrupt-root candidate shaped for one interrupt-table member: `stack` is
/// the declared arrival disposition and `acknowledged` selects whether the
/// root mints a settle-able `Pending` acknowledgement. The stack-realization
/// column is assigned by the caller so several members can share the one
/// artifact-wide composition the ledger requires.
pub(crate) fn interrupt_candidate_shaped(
    entry: EntryStubId,
    code: &InstalledCode,
    root_identity: u64,
    acknowledged: bool,
) -> ExternalRootCandidate {
    let mut candidate = candidate_for_code_with_root(entry, code, root_identity);
    candidate.requirement_identity = if acknowledged {
        "TimerRoot::tick".into()
    } else {
        "FatalExceptionRoot::enter".into()
    };
    candidate.entry_claims = if acknowledged {
        vec![ExternalRootEntryClaim {
            parameter_index: 0,
            domain: "InterruptAcknowledgement::Pending".into(),
            effective_carry: language_semantics::CarryPolicy::STRICT,
        }]
    } else {
        Vec::new()
    };
    candidate.acknowledgement_parameter_index = acknowledged.then_some(0);
    candidate.acknowledgement_policy =
        acknowledged.then(|| root_id(7, AcknowledgementPolicyId::from_normalized_identity));
    candidate.interrupt_mask_guard_claim = Some(ExternalRootResultClaim {
        provider_plan: root_id(56, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "InterruptMaskControl::save_and_mask".into(),
        domain: "InterruptMaskGuard::Active".into(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    });
    candidate.service_reach = ResolvedRootServiceReach::from_selected_provider_closure(
        Vec::new(),
        vec!["InterruptCompletion::complete".into()],
        &selected_interrupt_completion(),
    )
    .expect("selected completion closes the installed interrupt reach");
    candidate
}

/// One declared interrupt-table member fixture: its root identity seed,
/// selected entry, declared dedicated stack class, and acknowledgement
/// obligation.
pub(crate) struct InterruptTableMemberFixture {
    pub root_identity: u64,
    pub entry: EntryStubId,
    pub stack_class: u16,
    pub acknowledged: bool,
}

/// Build validated interrupt roots sharing the one artifact-wide stack
/// composition `InstalledRootLedger::install` requires. Each member's own
/// bound input enters the single composition; assigning any per-member
/// realization would be rejected as a foreign nesting aggregate.
pub(crate) fn interrupt_table_candidates(
    code: &InstalledCode,
    members: &[InterruptTableMemberFixture],
) -> Vec<(ValidatedExternalRoot, ValidatedBoundaryEntryPlan)> {
    let mut inputs = Vec::with_capacity(members.len());
    let mut shaped = Vec::with_capacity(members.len());
    for member in members {
        let stack = EntryStack::Dedicated {
            class: member.stack_class,
        };
        let boundary = interrupt_boundary_on(stack);
        let candidate = interrupt_candidate_shaped(
            member.entry,
            code,
            member.root_identity,
            member.acknowledged,
        );
        let input = stack_demand_input(
            candidate.identity,
            candidate.provider,
            &boundary,
            code,
            member.entry,
            stack,
            2048,
        );
        inputs.push(input);
        shaped.push((candidate, boundary));
    }
    let relation = StackNestingRelation {
        identity: shaped
            .first()
            .expect("at least one table member")
            .0
            .nesting_relation,
        edges: BTreeSet::new(),
    };
    let composition =
        compose_bound_entry_stack_epochs(&relation, inputs.iter()).expect("shared composition");
    shaped
        .into_iter()
        .map(|(mut candidate, boundary)| {
            candidate.stack.realization = composition.clone();
            (
                validate_external_root(candidate, &boundary).expect("table member root plan"),
                boundary,
            )
        })
        .collect()
}

pub(crate) fn interrupt_entry_receipt(
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

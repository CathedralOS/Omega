use super::*;

use std::collections::BTreeSet;

use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStackEpoch,
    EntryStackRealization, EntryStackStage, MachineRegister, MachineState, MachineStateSet,
    ProviderExitRealization, RegisterSet, StackDomainRef, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, ValueShape, evaluate_call_plan,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_realization,
};
use effects::provider_plan::ProviderPlan;
use effects::{
    ComponentEraLedgerId, ExecutableTcbManifest, ExecutableTcbProfile,
    ExecutableTcbProfileAcceptance, ExecutionScope, IncompleteScopePolicy, ScopeCompleteness,
    SelectedProviderPlanFacts, evaluate_executable_tcb_profile,
};
use executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, CodePlacementAuthority,
    CodePlacementId, EntrySetId, FinalValidationCertificate, FinalValidationId, InstallAuthority,
    InstallationAudience, InstallationReceipt, InstallationScopeId, InstalledCode,
    MachineContractSetId, MachineFootprintId, MaterializationReceipt, PlacementPlanId,
    RelocationSetId, WxEnforcement, admit_executable, install_validated,
    materialize_admitted_artifact, materialize_and_freeze, validate_final_placement,
};
use extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use external_roots::{
    AcknowledgementPolicyId, AdmittedOpaqueArrivalContextSet, BoundEpochStackComposition,
    ComponentArtifactId, ComponentContractId, ComponentProviderId, ComponentVersionPin,
    ComponentVersionPinId, ExternalRootCandidate, ExternalRootDiagnostic, ExternalRootId,
    FixedFuelCall, FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity,
    FuelValidationReceiptId, LogicalFuelResourceColumn, MachineStateResourceColumn,
    NestingRelationId, OpaqueCallbackProviderId, OpaqueCallbackRegistrationCapacityOccurrence,
    OpaqueCallbackRegistrationCapacityOccurrenceId, OpaqueCallbackRegistrationId,
    OpaqueCallbackRegistrationReceipt, OpaqueCallbackRegistrationReceiptId,
    OpaqueCallbackUnregistrationContractId, OpaqueCallbackUnregistrationReceipt,
    OpaqueCallbackUnregistrationReceiptId, OpaqueProviderExitAssurance, ProviderExecution,
    ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId, ProviderPlanId,
    ProviderStackSummary, ResolvedRootServiceReach, RootAdmission, RootAdmissionId, RootEffectId,
    RootProviderId, RootRemovalReceipt, RootRemovalReceiptId, RootSlotAuthority, RootSlotId,
    RootSlotOwnerId, StackDomain, StackNestingRelation, StackResourceColumn,
    StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
    admit_opaque_arrival_context_set, bind_opaque_adapter_stack_realization,
    compose_bound_entry_stack_epochs, compose_fixed_fuel, validate_external_root,
};
use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    bind_installed_artifact, bind_installed_compiler_private_function_entry,
    build_installation_record, build_object_artifact_with_private_functions, emit_executable_image,
};
use layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use machine_code::{
    CompilerPrivateMachineCodeFunction, MachineCodeFunction, MachineCodePlan,
    MachineCodePlanWithPrivateFunctions,
};
use semantic_vocabulary::{
    EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ProfileDecisionId, ValueId,
};
use symbols::SymbolHandle;
use target_operations::{ScalarAbiValue, ScalarFunctionAbi, TerminalPsiProvenance};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn install_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine")
}

fn psi_identity(seed: u8) -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([seed; 32]),
    }
}

fn callback_private_function_identity() -> MachineFunctionIdentity {
    MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: SymbolHandle::from_parts(91, 2),
            state: SymbolHandle::from_parts(93, 3),
            segment_index: 5,
        },
        0,
    )
    .expect("private callback identity")
}

fn program_function() -> MachineCodeFunction {
    MachineCodeFunction {
        machine: machine_id(1),
        attachment: None,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        structural_call_scalar_return: None,
        parameter_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![OperationId::new(1).expect("entry operation")],
            edges: vec![EdgeId::new(1).expect("entry edge")],
        },
        bytes: vec![0xb8, 3, 0, 0, 0, 0xc3],
        x86_scalar_fma: Vec::new(),
        x86_scalar_fma_occurrences: Vec::new(),
        x86_floating_control: None,
        unit_stack: None,
        unit_parameter_homes: Vec::new(),
        unit_parameters: Vec::new(),
        scalar_stack: None,
        internal_calls: Vec::new(),
        foreign_calls: Vec::new(),
        internal_unit_calls: Vec::new(),
        internal_unit_scalar_calls: Vec::new(),
        installed_provider_unit_scalar_calls: Vec::new(),
        dynamic_calls: Vec::new(),
        stored_dynamic_calls: Vec::new(),
        dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_descriptor_calls: Vec::new(),
        unit_scalar_homes: Vec::new(),
        unit_integer_constants: Vec::new(),
        unit_affine_scalar_records: Vec::new(),
        unit_structural_scalar_field_stores: Vec::new(),
        unit_write_only_primitive_stores: Vec::new(),
        scalar_structural_scalar_field_stores: Vec::new(),
        unit_affine_cleanup: None,
        unit_continuations: Vec::new(),
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: Vec::new(),
        scalar_structural_parameter_homes: Vec::new(),
        semantic_code_attribution: Vec::new(),
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    }
}

/// Hand-retained compiler-private callback: `mov rax, rdi; ret` realizes the
/// one-u64 System V scalar ABI declared beside it.
fn callback_private_function() -> CompilerPrivateMachineCodeFunction {
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("one-u64 callback ABI");
    let scalar_type = semantic_vocabulary::ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
    );
    let mut function = program_function();
    function.machine = machine_id(97);
    function.scalar_abi = Some(ScalarFunctionAbi {
        parameters: vec![ScalarAbiValue {
            value: ValueId::new(19).expect("parameter value"),
            scalar_type,
            placement: call_plan.parameters[0].clone(),
        }],
        result: ScalarAbiValue {
            value: ValueId::new(23).expect("result value"),
            scalar_type,
            placement: call_plan.result.clone().expect("result placement"),
        },
        call_plan,
    });
    function.bytes = vec![0x48, 0x89, 0xf8, 0xc3];
    CompilerPrivateMachineCodeFunction {
        identity: callback_private_function_identity(),
        private_symbol: "__omega_component_callback".into(),
        source_psi: psi_identity(0x52),
        function,
    }
}

fn terminal_image() -> (
    image_emission::ObjectArtifact,
    image_emission::ExecutableImage,
) {
    let object =
        build_object_artifact_with_private_functions(&MachineCodePlanWithPrivateFunctions {
            plan: MachineCodePlan {
                psi: psi_identity(7),
                target: target::NativeTarget::linux_x64(),
                entry: machine_id(1),
                functions: vec![program_function()],
            },
            private_functions: vec![callback_private_function()],
        })
        .expect("terminal object with private callback");
    let image = emit_executable_image(&object, 3).expect("terminal image");
    (object, image)
}

fn install_terminal_text(
    object: &image_emission::ObjectArtifact,
    artifact_identity: u64,
    installed_identity: u64,
    placement_base: u64,
) -> InstalledCode {
    let scope = ArtifactInstallationScopeId::from_normalized_identity(1).expect("scope");
    let constraints = PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
        .expect("constraints");
    let entry = EntryStubId::from_normalized_identity(1).expect("entry");
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let [private_function] = object.private_functions() else {
        panic!("one private callback expected")
    };
    let contracts = install_id(2, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(3, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(artifact_identity, ArtifactId::from_normalized_identity),
        object.target().architecture,
        object.text_bytes().to_vec(),
        contracts,
        footprint,
        install_id(4, PlacementPlanId::from_normalized_identity),
        constraints,
        install_id(5, EntrySetId::from_normalized_identity),
        vec![
            ArtifactEntry::from_canonical_decode(entry, 0),
            ArtifactEntry::from_canonical_decode(
                private_entry,
                u64::try_from(private_function.function.text_offset)
                    .expect("private callback offset"),
            ),
        ],
        install_id(6, RelocationSetId::from_normalized_identity),
        Vec::new(),
        executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            None,
            Some((scope, b"test-installation-scope-v1")),
        ),
    )
    .expect("artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(7, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        1,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        extents::ExtentProviderIssuance::from_normalized_identities([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
        ])
        .expect("extent issuance"),
        extent_id(2, ExtentLineageId::from_normalized_identity),
        extent_id(3, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(4, ExtentProvenanceId::from_normalized_identity),
        extent_id(5, MappingEraId::from_normalized_identity),
    )
    .mint(placement_base, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(8, CodePlacementId::from_normalized_identity),
        install_id(1, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: placement_base,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("materialized artifact");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(9, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen artifact");
    let validation = FinalValidationCertificate::from_validator(
        install_id(10, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &validation).expect("validated artifact");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(
            installed_identity,
            InstalledCodeId::from_normalized_identity,
        ),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).expect("installed code")
}

fn tcb_acceptance(name: &str, closure: u64) -> ExecutableTcbProfileAcceptance {
    evaluate_executable_tcb_profile(
        &ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_report_identity: closure,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        },
        &ExecutableTcbProfile {
            name: name.into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        },
    )
    .expect("TCB acceptance")
}

fn lifecycle() -> RunnableComponentEraLedger {
    RunnableComponentEraLedger::new(
        ComponentEraEntryLedger::new(
            ComponentEraLedgerId::from_normalized_identity(1).expect("ledger"),
            "CodecBinding/v1".into(),
            "CodecEntry/v1".into(),
            2,
            tcb_acceptance("platform", 1),
        )
        .expect("component lifecycle"),
    )
}

fn candidate(era: u64, runnable: &InstalledRunnableComponent) -> ComponentEraCandidate {
    ComponentEraCandidate {
        era_identity: era,
        artifact_occurrence_digest: runnable.installed().occurrence_digest(),
        artifact_instance_compatibility_report_identity: runnable
            .installed_code()
            .normalized_identity(),
        binding_contract_identity: "CodecBinding/v1".into(),
        entry_contract_identity: "CodecEntry/v1".into(),
        entry_plan_identity: format!("entry-plan:{era}"),
        entry_plan_admission_receipt_identity: format!("entry-plan-receipt:{era}"),
        executable_tcb_acceptance: tcb_acceptance(format!("era-{era}").as_str(), era),
    }
}

struct RunnableFixture {
    installed_code: InstalledCodeId,
    runnable: InstalledRunnableComponent,
}

fn runnable_fixture(seed: u64) -> RunnableFixture {
    runnable_fixture_at(seed, 0x1000)
}

/// One installed runnable component that retains a compiler-private callback
/// entry. The provider-occurrence closure is sealed empty and the terminal
/// installation record commits no component progress: the callback lifetime
/// chain under test joins exact installed-occurrence evidence, not provider
/// plan content.
fn runnable_fixture_at(seed: u64, placement_base: u64) -> RunnableFixture {
    let mut fixture = unprovisioned_runnable_fixture_at(seed, placement_base);
    let provision = callback_stack_provision(fixture.runnable.installed());
    fixture
        .runnable
        .admit_external_stack_provision(provision)
        .expect("admitted callback stack provision");
    fixture
}

/// One installed runnable component with no admitted external stack
/// provision. Installs through its runtime custody reject until provider
/// supply is admitted and sealed against this exact occurrence.
fn unprovisioned_runnable_fixture_at(seed: u64, placement_base: u64) -> RunnableFixture {
    let (object, image) = terminal_image();
    let mut installed = install_terminal_text(&object, seed + 20, seed + 21, placement_base);
    let installed_code = installed.identity();
    let mut root_ledger =
        InstalledRootLedger::claim(&mut installed).expect("installation registry");
    root_ledger
        .seal_provider_occurrence_closure(&SelectedProviderPlanFacts::default(), [])
        .expect("empty provider occurrence closure");
    let installation = build_installation_record(
        &image,
        ProfileDecisionId::new(seed + 40).expect("profile decision"),
    )
    .expect("terminal installation record");
    let artifact = bind_installed_artifact(object, image, installation, installed)
        .expect("installed terminal artifact");
    let runnable = bind_installed_runnable_component(artifact, root_ledger, None)
        .expect("installed runnable component");
    RunnableFixture {
        installed_code,
        runnable,
    }
}

/// Provider-owned stack supply covering the interrupted domain the callback
/// root's bound epoch composition demands (2048 bytes at 16-byte alignment).
fn callback_stack_provision(installed: &InstalledCode) -> ProvisionedExternalStackSet {
    seal_external_stack_provision(
        installed,
        [admit_external_stack_domain_lease(
            installed,
            StackDomain::Interrupted,
            8192,
            16,
            root_id(760, RootProviderId::from_normalized_identity),
            root_id(761, StackValidationReceiptId::from_normalized_identity),
        )
        .expect("interrupted-domain stack lease")],
    )
    .expect("sealed callback stack provision")
}

fn callback_boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("callback boundary")
}

fn callback_fuel(_root: ExternalRootId) -> external_roots::ComposedFuelDemand {
    let leaf_identity = root_id(731, ProviderFuelSummaryId::from_normalized_identity);
    let owner_identity = root_id(730, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(712, RootProviderId::from_normalized_identity),
        FuelScheduleIdentity::new(1).expect("fuel schedule"),
        5,
        BTreeSet::new(),
        root_id(
            741,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let owner = FixedFuelProviderSummary::from_admitted_provider(
        owner_identity,
        root_id(702, RootProviderId::from_normalized_identity),
        FuelScheduleIdentity::new(1).expect("fuel schedule"),
        2,
        BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 1,
        }]),
        root_id(
            740,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    compose_fixed_fuel(owner_identity, [&owner, &leaf]).expect("callback fuel")
}

fn callback_stack(
    root: ExternalRootId,
    provider: RootProviderId,
    relation: NestingRelationId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
) -> BoundEpochStackComposition {
    let active_domain = StackDomainRef::Interrupted;
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
    .expect("callback stack realization");
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        2048,
        16,
        root_id(749, StackValidationReceiptId::from_normalized_identity),
    );
    let contexts: AdmittedOpaqueArrivalContextSet = admit_opaque_arrival_context_set(
        &summary,
        boundary,
        code,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(748, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("callback arrival context");
    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        boundary,
        code,
        entry,
        realization,
        contexts,
    )
    .expect("callback stack binding");
    compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation,
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("callback stack composition")
}

fn callback_root_candidate(code: &InstalledCode, entry: EntryStubId) -> ExternalRootCandidate {
    let root = root_id(701, ExternalRootId::from_normalized_identity);
    let provider = root_id(702, RootProviderId::from_normalized_identity);
    let relation = root_id(706, NestingRelationId::from_normalized_identity);
    let boundary = callback_boundary();
    ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(755, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "Callback::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &SelectedProviderPlanFacts::default(),
        )
        .expect("empty callback service reach"),
        effects: [root_id(703, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [root_id(704, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation: relation,
        acknowledgement_policy: Some(root_id(
            707,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: callback_stack(root, provider, relation, &boundary, code, entry),
            validation_receipt: root_id(750, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: FuelScheduleIdentity::new(1).expect("fuel schedule"),
            provision: root_id(753, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: callback_fuel(root),
            validation_receipt: root_id(751, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(752, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: [ComponentVersionPin {
            contract: root_id(708, ComponentContractId::from_normalized_identity),
            artifact: root_id(709, ComponentArtifactId::from_normalized_identity),
            provider: root_id(710, ComponentProviderId::from_normalized_identity),
            version: root_id(711, ComponentVersionPinId::from_normalized_identity),
        }]
        .into_iter()
        .collect(),
    }
}

/// A successful registration is a linear external root owned by the exact
/// retained component-era lease that issued it. Rejection, provider retry,
/// unregister, and quiescence each preserve or release that linear custody:
/// capacity bounds live registrations, code/component leases stay held until
/// the root ends, and teardown returns the exact root slot and capacity
/// occurrence minted at registration. Those returned authorities then back a
/// replacement registration in the same era.
#[test]
fn package_registration_owns_exact_component_era_lease_through_replacement() {
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let process_entry = EntryStubId::from_normalized_identity(1).expect("process entry");
    let private_function = callback_private_function_identity();
    let fixture = runnable_fixture(800);
    let other = runnable_fixture_at(800, 0x9000);
    assert_eq!(fixture.runnable.installed_code(), fixture.installed_code);
    assert_eq!(other.runnable.installed_code(), other.installed_code);
    assert_eq!(
        fixture.installed_code, other.installed_code,
        "fixture occurrences collide only in compact report identity"
    );
    assert_ne!(
        fixture.runnable.installed().receipt_context(),
        other.runnable.installed().receipt_context(),
        "different exact placements retain distinct installed occurrence evidence"
    );
    let attribution = bind_installed_compiler_private_function_entry(
        fixture.runnable.installed_artifact(),
        private_function,
        private_entry,
    )
    .expect("private callback attribution");
    let other_attribution = bind_installed_compiler_private_function_entry(
        other.runnable.installed_artifact(),
        private_function,
        private_entry,
    )
    .expect("other installed occurrence attribution");
    let mut other_lifecycle = lifecycle();
    let other_candidate = candidate(10, &other.runnable);
    let other_receipt = ComponentEraPublicationReceipt::from_runtime(
        797,
        other_lifecycle.lifecycle(),
        &other_candidate,
        true,
        false,
    );
    other_lifecycle
        .publish(other_candidate, other_receipt, other.runnable)
        .expect("other callback component era");
    let foreign_lease_identity = ProgramLocalRootEpochLeaseId::from_normalized_identity(797)
        .expect("foreign callback lease");
    let foreign_lease = other_lifecycle
        .acquire_program_local_root_epoch_lease(foreign_lease_identity, 10, "CodecEntry/v1")
        .expect("foreign component-era lease");
    assert!(
        bind_installed_compiler_private_function_entry(
            fixture.runnable.installed_artifact(),
            private_function,
            process_entry,
        )
        .expect_err("process entry cannot substitute for private callback")
        .diagnostic()
        .contains("text offset")
    );

    let root_candidate = callback_root_candidate(fixture.runnable.installed(), private_entry);
    let boundary = callback_boundary();
    let validated = validate_external_root(root_candidate, &boundary).expect("callback root");
    let slot = RootSlotAuthority::from_admitted_owner(
        root_id(720, RootSlotId::from_normalized_identity),
        root_id(721, RootSlotOwnerId::from_normalized_identity),
    );
    let execution = ProviderExecution::from_admitted_provider(
        root_id(754, ProviderExecutionId::from_normalized_identity),
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(704, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("callback provider execution");
    let admission = RootAdmission::from_admitted_provider(
        root_id(722, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        fixture.runnable.installed(),
        &slot,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("callback root admission");
    let mut lifecycle = lifecycle();
    let era_candidate = candidate(10, &fixture.runnable);
    let era_receipt = ComponentEraPublicationReceipt::from_runtime(
        799,
        lifecycle.lifecycle(),
        &era_candidate,
        true,
        false,
    );
    lifecycle
        .publish(era_candidate, era_receipt, fixture.runnable)
        .expect("callback component era");
    let mut runtime = lifecycle
        .callback_registration_runtime(10)
        .expect("retained callback runtime");
    let root = runtime
        .install(validated, slot, admission)
        .expect("installed callback root");
    let root_identity = root.root();
    let not_quiesced = RootRemovalReceipt::from_provider(
        root_id(780, RootRemovalReceiptId::from_normalized_identity),
        &root,
        true,
        false,
    );
    let quiesced = RootRemovalReceipt::from_provider(
        root_id(781, RootRemovalReceiptId::from_normalized_identity),
        &root,
        true,
        true,
    );
    let provider = root_id(784, OpaqueCallbackProviderId::from_normalized_identity);
    let capacity_identity = root_id(
        789,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let failed_provider_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            782,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(783, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            785,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &root,
        &capacity,
        false,
    );
    let error = runtime
        .admit_compiler_private_callback(other_attribution, root, failed_provider_receipt, capacity)
        .expect_err("cross-occurrence attribution rejects before provider success");
    let (other_attribution, root, _failed_provider_receipt, capacity) = (*error).into_parts();
    assert_eq!(other_attribution.entry(), private_entry);
    assert_eq!(capacity.identity(), capacity_identity);
    assert_eq!(root.root(), root_identity);

    let failed_provider_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            786,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(783, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            785,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &root,
        &capacity,
        false,
    );
    let error = runtime
        .admit_compiler_private_callback(attribution, root, failed_provider_receipt, capacity)
        .expect_err("provider rejection establishes no registration");
    let (attribution, root, _failed_provider_receipt, capacity) = (*error).into_parts();
    assert_eq!(attribution.entry(), private_entry);
    assert_eq!(capacity.identity(), capacity_identity);

    let accepted_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            787,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(783, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            785,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &root,
        &capacity,
        true,
    );
    let collision_capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let error = runtime
        .admit_compiler_private_callback(attribution, root, accepted_receipt, collision_capacity)
        .expect_err("collision-equal capacity occurrence rejects");
    let (attribution, root, accepted_receipt, collision_capacity) = (*error).into_parts();
    assert_eq!(collision_capacity.identity(), capacity_identity);
    let registered = runtime
        .admit_compiler_private_callback(attribution, root, accepted_receipt, capacity)
        .expect("exact provider registration");
    let lease_identity = ProgramLocalRootEpochLeaseId::from_normalized_identity(798)
        .expect("callback component-era lease");
    let lease = runtime
        .acquire_registration_lease(lease_identity)
        .expect("exact callback component-era lease");
    let error = runtime
        .lower_registration(registered, foreign_lease)
        .expect_err("different installed occurrence lease cannot lower registration");
    let (registered, foreign_lease) = (*error).into_parts();
    assert_eq!(foreign_lease.identity(), foreign_lease_identity);
    other_lifecycle
        .release_program_local_root_epoch_lease(foreign_lease)
        .expect("foreign lease remains returnable to its exact ledger");
    let registration = runtime
        .lower_registration(registered, lease)
        .expect("package-visible linear registration");
    assert_eq!(registration.attribution().entry(), private_entry);
    assert_eq!(registration.component_era_identity(), 10);
    assert_eq!(registration.component_era_lease_identity(), lease_identity);
    assert_eq!(runtime.component_era_lease_holds(), Some(1));
    assert_eq!(
        registration.registration().capacity().identity(),
        capacity_identity
    );

    let unsuccessful = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            788,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        registration.registration(),
        false,
    );
    let error = registration
        .unregister_and_quiesce(&mut runtime, unsuccessful, not_quiesced)
        .expect_err("unsuccessful unregister retains callback custody");
    let (registration, _, not_quiesced) = (*error).into_parts();
    assert_eq!(registration.attribution().entry(), private_entry);
    assert_eq!(runtime.component_era_lease_holds(), Some(1));
    assert_eq!(
        registration.registration().capacity().identity(),
        capacity_identity
    );

    let successful = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            790,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        registration.registration(),
        true,
    );
    let error = registration
        .unregister_and_quiesce(&mut runtime, successful, not_quiesced)
        .expect_err("provider success without quiescence retains callback custody");
    let (registration, _, _) = (*error).into_parts();
    assert_eq!(registration.attribution().entry(), private_entry);
    assert_eq!(runtime.component_era_lease_holds(), Some(1));

    let successful = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            791,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        registration.registration(),
        true,
    );
    let unregistered = registration
        .unregister_and_quiesce(&mut runtime, successful, quiesced)
        .expect("provider unregister plus root quiescence");
    assert_eq!(unregistered.component_era_identity(), 10);
    assert_eq!(unregistered.component_era_lease_identity(), lease_identity);
    let mut other_runtime = other_lifecycle
        .callback_registration_runtime(10)
        .expect("other retained callback runtime");
    let error = unregistered
        .release_component_era(&mut other_runtime)
        .expect_err("different retained component cannot release callback lease");
    let unregistered = (*error).into_registration();
    assert_eq!(unregistered.component_era_lease_identity(), lease_identity);
    let completed = unregistered
        .release_component_era(&mut runtime)
        .expect("exact component-era release");
    let (attribution, completion) = completed.into_parts();
    assert_eq!(attribution.entry(), private_entry);
    let (returned_slot, returned_capacity) = completion.into_parts();
    assert_eq!(
        returned_slot.slot(),
        root_id(720, RootSlotId::from_normalized_identity)
    );
    assert_eq!(returned_capacity.identity(), capacity_identity);

    // Replacement: the returned root-slot authority and live-registration
    // capacity occurrence back a fresh registration in the same retained era.
    // The capacity occurrence is linear, so the provider can re-issue it only
    // because the first registration ended and returned it.
    let replacement_candidate = callback_root_candidate(runtime.installed(), private_entry);
    let replacement_validated = validate_external_root(replacement_candidate, &boundary)
        .expect("replacement callback root");
    let replacement_execution = ProviderExecution::from_admitted_provider(
        root_id(756, ProviderExecutionId::from_normalized_identity),
        &replacement_validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: replacement_validated.boundary().call.entry_control,
                restored_state: replacement_validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(704, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("replacement provider execution");
    let replacement_admission = RootAdmission::from_admitted_provider(
        root_id(723, RootAdmissionId::from_normalized_identity),
        &replacement_validated,
        &replacement_execution,
        runtime.installed(),
        &returned_slot,
        replacement_validated
            .candidate()
            .trust_receipts
            .iter()
            .copied(),
    )
    .expect("replacement root admission");
    let replacement_root = runtime
        .install(replacement_validated, returned_slot, replacement_admission)
        .expect("returned slot authority admits the replacement callback root");
    let replacement_quiesced = RootRemovalReceipt::from_provider(
        root_id(795, RootRemovalReceiptId::from_normalized_identity),
        &replacement_root,
        true,
        true,
    );
    let replacement_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            792,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(793, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            785,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &replacement_root,
        &returned_capacity,
        true,
    );
    let replacement_registered = runtime
        .admit_compiler_private_callback(
            attribution,
            replacement_root,
            replacement_receipt,
            returned_capacity,
        )
        .expect("returned capacity occurrence re-issues a replacement registration");
    let replacement_lease_identity = ProgramLocalRootEpochLeaseId::from_normalized_identity(801)
        .expect("replacement component-era lease");
    let replacement_lease = runtime
        .acquire_registration_lease(replacement_lease_identity)
        .expect("replacement component-era lease");
    let replacement = runtime
        .lower_registration(replacement_registered, replacement_lease)
        .expect("replacement linear registration");
    assert_eq!(replacement.component_era_identity(), 10);
    assert_eq!(
        replacement.component_era_lease_identity(),
        replacement_lease_identity
    );
    assert_eq!(runtime.component_era_lease_holds(), Some(1));
    let replacement_unregister = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            794,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        replacement.registration(),
        true,
    );
    let replacement_unregistered = replacement
        .unregister_and_quiesce(&mut runtime, replacement_unregister, replacement_quiesced)
        .expect("replacement unregister plus root quiescence");
    let replacement_completed = replacement_unregistered
        .release_component_era(&mut runtime)
        .expect("replacement exact component-era release");
    let (replacement_attribution, replacement_completion) = replacement_completed.into_parts();
    assert_eq!(replacement_attribution.entry(), private_entry);
    let (replaced_slot, replaced_capacity) = replacement_completion.into_parts();
    assert_eq!(
        replaced_slot.slot(),
        root_id(720, RootSlotId::from_normalized_identity)
    );
    assert_eq!(replaced_capacity.identity(), capacity_identity);
    drop(runtime);
    assert_eq!(lifecycle.program_local_root_authority_holds(10), Some(0));
}

/// The retained stack-lease set is an independent lane beside WCSU evidence,
/// artifact entry, and ledger custody: an external root can install only
/// while provider-owned supply binds the exact installed occurrence and
/// covers the root's composed domain demand.
///
/// An unresolved provider-selected disposition can never be leased — it is a
/// pending choice, and leasing it would admit the root without deciding which
/// concrete domain runs. A lease sealed to another installed occurrence is a
/// cross-context disposition and rejects both at seal and at admission. An
/// install with no retained set, with no lease for a demanded domain, or
/// below the composed demand all reject with the validated root, slot, and
/// admission returned for correction and retry.
#[test]
fn external_root_install_rejoins_exact_admitted_stack_provision() {
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let mut fixture = unprovisioned_runnable_fixture_at(600, 0x1000);
    let other = runnable_fixture_at(600, 0x9000);
    assert_ne!(
        fixture.runnable.installed().receipt_context(),
        other.runnable.installed().receipt_context(),
        "different exact placements retain distinct installed occurrence evidence"
    );

    let lease_authority = || {
        (
            root_id(760, RootProviderId::from_normalized_identity),
            root_id(761, StackValidationReceiptId::from_normalized_identity),
        )
    };
    let (provisioner, lease_receipt) = lease_authority();

    // An unresolved provider-selected disposition is a pending choice, not a
    // provisionable domain.
    admit_external_stack_domain_lease(
        fixture.runnable.installed(),
        StackDomain::ProviderSelected,
        8192,
        16,
        provisioner,
        lease_receipt,
    )
    .expect_err("provider-selected disposition cannot be leased");

    // Zero capacity and non-power-of-two alignment cannot describe real
    // stack supply.
    admit_external_stack_domain_lease(
        fixture.runnable.installed(),
        StackDomain::Interrupted,
        0,
        16,
        provisioner,
        lease_receipt,
    )
    .expect_err("zero-capacity lease cannot provision a domain");
    admit_external_stack_domain_lease(
        fixture.runnable.installed(),
        StackDomain::Interrupted,
        8192,
        24,
        provisioner,
        lease_receipt,
    )
    .expect_err("non-power-of-two alignment cannot provision a domain");

    // A lease bound to another installed occurrence is a cross-context
    // disposition: it cannot seal into this occurrence's set, and the foreign
    // set cannot be admitted over this component.
    let foreign_lease = admit_external_stack_domain_lease(
        other.runnable.installed(),
        StackDomain::Interrupted,
        8192,
        16,
        provisioner,
        lease_receipt,
    )
    .expect("foreign-occurrence stack lease");
    let error = seal_external_stack_provision(fixture.runnable.installed(), [foreign_lease])
        .expect_err("a lease bound to another occurrence cannot seal here");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("different installed-code occurrence")
    );
    let foreign_set =
        seal_external_stack_provision(other.runnable.installed(), error.into_leases())
            .expect("the returned lease reseals against its own occurrence");
    let error = fixture
        .runnable
        .admit_external_stack_provision(foreign_set)
        .expect_err("cross-context provision set cannot be admitted");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("different installed-code occurrence")
    );
    drop(error);

    // Build the exact root, slot, and admission once; every rejected install
    // returns them so the same inputs retry.
    let boundary = callback_boundary();
    let root_candidate = callback_root_candidate(fixture.runnable.installed(), private_entry);
    let validated = validate_external_root(root_candidate, &boundary).expect("callback root");
    let slot = RootSlotAuthority::from_admitted_owner(
        root_id(720, RootSlotId::from_normalized_identity),
        root_id(721, RootSlotOwnerId::from_normalized_identity),
    );
    let execution = ProviderExecution::from_admitted_provider(
        root_id(754, ProviderExecutionId::from_normalized_identity),
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(704, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("callback provider execution");
    let admission = RootAdmission::from_admitted_provider(
        root_id(722, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        fixture.runnable.installed(),
        &slot,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("callback root admission");

    // No retained provision: install rejects before the ledger sees the root.
    let mut runtime = fixture.runnable.external_root_runtime();
    let error = runtime
        .install(validated, slot, admission)
        .expect_err("install requires admitted external stack provision");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("no admitted external stack provision")
    );
    let (validated, slot, admission) = (*error).into_parts();
    drop(runtime);

    // A set leasing only an unrelated dedicated domain leaves the demanded
    // interrupted domain uncovered.
    let dedicated_only = seal_external_stack_provision(
        fixture.runnable.installed(),
        [admit_external_stack_domain_lease(
            fixture.runnable.installed(),
            StackDomain::Dedicated { class: 7 },
            8192,
            16,
            provisioner,
            lease_receipt,
        )
        .expect("dedicated-domain stack lease")],
    )
    .expect("dedicated-only provision set");
    fixture
        .runnable
        .admit_external_stack_provision(dedicated_only)
        .expect("dedicated-only provision admitted while no roots are live");
    let mut runtime = fixture.runnable.external_root_runtime();
    let error = runtime
        .install(validated, slot, admission)
        .expect_err("a demanded domain without a lease rejects");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("no admitted stack lease"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (validated, slot, admission) = (*error).into_parts();
    drop(runtime);

    // A lease below the composed domain demand rejects — demand evidence, not
    // the lease's own claim, sets the floor.
    let undersized = seal_external_stack_provision(
        fixture.runnable.installed(),
        [admit_external_stack_domain_lease(
            fixture.runnable.installed(),
            StackDomain::Interrupted,
            1024,
            16,
            provisioner,
            lease_receipt,
        )
        .expect("undersized stack lease")],
    )
    .expect("undersized provision set");
    fixture
        .runnable
        .admit_external_stack_provision(undersized)
        .expect("undersized provision replaces while no roots are live");
    let mut runtime = fixture.runnable.external_root_runtime();
    let error = runtime
        .install(validated, slot, admission)
        .expect_err("a lease below the composed domain demand rejects");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("below the composed"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (validated, slot, admission) = (*error).into_parts();
    drop(runtime);

    // A covering set admits and the same retained inputs install.
    let covering = callback_stack_provision(fixture.runnable.installed());
    fixture
        .runnable
        .admit_external_stack_provision(covering)
        .expect("covering provision admitted while no roots are live");
    let mut runtime = fixture.runnable.external_root_runtime();
    let root = runtime
        .install(validated, slot, admission)
        .expect("a covering lease set admits the install");
    assert_eq!(
        root.root(),
        root_id(701, ExternalRootId::from_normalized_identity)
    );

    // While the root is live the provision is pinned: replacing the supply
    // under it would revoke the storage its demand was admitted against.
    drop(runtime);
    let replacement = callback_stack_provision(fixture.runnable.installed());
    fixture
        .runnable
        .admit_external_stack_provision(replacement)
        .expect_err("external stack provision is pinned while a root is live");
}

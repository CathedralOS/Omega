use super::{
    AdmittedExternalStackDomainLease, ArtifactId, CompletedRegistration, ComponentEraCandidate,
    ComponentEraEntryLedger, ComponentEraPublicationReceipt, InstalledCodeId, InstalledRootLedger,
    InstalledRunnableComponent, ProgramLocalRootEpochLeaseId, ProvisionedExternalStackSet,
    ProvisionedRootInstallError, Registration, RunnableComponentCallbackRegistrationRuntime,
    RunnableComponentEraLedger, admit_external_stack_domain_lease,
    bind_installed_runnable_component, seal_external_stack_provision,
};
use std::collections::BTreeSet;

use checked_trees_to_lowered_psi::lower_machine;

use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStackEpoch,
    EntryStackRealization, EntryStackStage, MachineRegister, MachineState, MachineStateSet,
    ProviderExitRealization, RegisterSet, StackDomainRef, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, ValueShape, evaluate_call_plan,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_realization,
};
use effects::provider_plan::{ProviderPlan, ServiceSchema};
use effects::{
    ComponentEraLedgerId, ComponentEraQuiescenceReceipt, ComponentEraRetirementReceipt,
    ComponentProgressManifest, ExecutableTcbManifest, ExecutableTcbProfile,
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
    FuelValidationReceiptId, InstalledComponentProgressClosure, InstalledProviderOccurrenceId,
    LogicalFuelResourceColumn, MachineStateResourceColumn, NestingRelationId,
    OpaqueCallbackProviderId, OpaqueCallbackRegistrationCapacityOccurrence,
    OpaqueCallbackRegistrationCapacityOccurrenceId, OpaqueCallbackRegistrationId,
    OpaqueCallbackRegistrationReceipt, OpaqueCallbackRegistrationReceiptId,
    OpaqueCallbackUnregistrationContractId, OpaqueCallbackUnregistrationReceipt,
    OpaqueCallbackUnregistrationReceiptId, OpaqueProviderExitAssurance, ProviderExecution,
    ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId,
    ProviderOccurrenceInstallationReceipt, ProviderOccurrenceInstallationReceiptId,
    ProviderOccurrencePlanBinding, ProviderPlanId, ProviderStackSummary, ResolvedRootServiceReach,
    RootAdmission, RootAdmissionId, RootEffectId, RootProviderId, RootRemovalReceipt,
    RootRemovalReceiptId, RootSlotAuthority, RootSlotId, RootSlotOwnerId, StackDomain,
    StackNestingRelation, StackResourceColumn, StackValidationReceiptId, StateValidationReceiptId,
    TrustReceiptId, ValidatedExternalRoot, admit_opaque_arrival_context_set,
    bind_opaque_adapter_stack_realization, compose_bound_entry_stack_epochs, compose_fixed_fuel,
    validate_external_root,
};
use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    InstalledArtifact, bind_installed_artifact, bind_installed_compiler_private_function_entry,
    build_installation_record, build_installation_record_with_evidence,
    build_object_artifact_with_private_functions, emit_executable_image,
};
use layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use machine_code::{
    CompilerPrivateMachineCodeFunction, MachineCodeFunction, MachineCodePlan,
    MachineCodePlanWithPrivateFunctions,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BoundaryMachineId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ProfileDecisionId, ValueId,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use target_operations::{ScalarAbiValue, ScalarFunctionAbi, TerminalPsiProvenance};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecutionResult, TerminalScalarValue, TerminalStructuralInputs,
    TerminalStructuralScalarFieldValue, TerminalStructuralValue,
    interpret_terminal_artifact_measured,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

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
    seal_external_stack_provision(installed, [stack_lease(installed)])
        .expect("sealed callback stack provision")
}

/// One provider-admitted lease over the interrupted domain: 8192 bytes at
/// 16-byte alignment, covering the 2048-byte demand the callback root's bound
/// epoch composition carries.
fn stack_lease(installed: &InstalledCode) -> AdmittedExternalStackDomainLease {
    admit_external_stack_domain_lease(
        installed,
        StackDomain::Interrupted,
        8192,
        16,
        root_id(760, RootProviderId::from_normalized_identity),
        root_id(761, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("interrupted-domain stack lease")
}

/// The validated callback root, its slot authority, and its exact admission
/// for `installed` — the inputs `external_root_runtime().install` consumes
/// and hands back on every provision-lane rejection.
fn callback_install_inputs(
    installed: &InstalledCode,
    entry: EntryStubId,
) -> (ValidatedExternalRoot, RootSlotAuthority, RootAdmission) {
    let boundary = callback_boundary();
    let validated = validate_external_root(callback_root_candidate(installed, entry), &boundary)
        .expect("callback root");
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
        installed,
        &slot,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("callback root admission");
    (validated, slot, admission)
}

/// Drive one install against `runnable`'s retained provision, asserting the
/// rejection comes from the provision lane — not the ledger — with
/// `fragment`, then return the consumed inputs the error hands back for
/// retry.
fn expect_provision_rejection(
    runnable: &mut InstalledRunnableComponent,
    validated: ValidatedExternalRoot,
    slot: RootSlotAuthority,
    admission: RootAdmission,
    field: &str,
    fragment: &str,
) -> (ValidatedExternalRoot, RootSlotAuthority, RootAdmission) {
    let mut runtime = runnable.external_root_runtime();
    let error = runtime
        .install(validated, slot, admission)
        .expect_err(field);
    assert!(
        matches!(*error, ProvisionedRootInstallError::Provision { .. }),
        "{field}: rejection must come from the provision lane before ledger custody: {}",
        error.diagnostic()
    );
    assert!(
        error.diagnostic().to_string().contains(fragment),
        "{field}: unexpected diagnostic: {}",
        error.diagnostic()
    );
    drop(runtime);
    (*error).into_parts()
}

/// An installed terminal artifact plus the root registry claimed on its exact
/// occurrence, before runnable binding joins them — the two pieces a
/// one-field substitution can misalign.
struct UnboundInstallation {
    artifact: InstalledArtifact,
    roots: InstalledRootLedger,
}

fn unbound_installation(seed: u64, placement_base: u64) -> UnboundInstallation {
    let (object, image) = terminal_image();
    let mut installed = install_terminal_text(&object, seed + 20, seed + 21, placement_base);
    let roots = InstalledRootLedger::claim(&mut installed).expect("installation registry");
    let installation = build_installation_record(
        &image,
        ProfileDecisionId::new(seed + 40).expect("profile decision"),
    )
    .expect("terminal installation record");
    let artifact = bind_installed_artifact(object, image, installation, installed)
        .expect("installed terminal artifact");
    UnboundInstallation { artifact, roots }
}

/// An installed terminal artifact, its claimed registry, and one sealed
/// component-progress acceptance, before runnable binding joins them. `commit`
/// selects whether the artifact's canonical installation record carries the
/// acceptance identities.
struct UnboundProgressInstallation {
    artifact: InstalledArtifact,
    roots: InstalledRootLedger,
    progress: InstalledComponentProgressClosure,
}

fn unbound_progress_installation(
    seed: u64,
    placement_base: u64,
    entry_callable: &str,
    commit: bool,
) -> UnboundProgressInstallation {
    let (object, image) = terminal_image();
    let mut installed = install_terminal_text(&object, seed + 20, seed + 21, placement_base);
    let mut roots = InstalledRootLedger::claim(&mut installed).expect("installation registry");
    roots
        .seal_provider_occurrence_closure(&SelectedProviderPlanFacts::default(), [])
        .expect("empty provider occurrence closure");
    let manifest = ComponentProgressManifest::bind(
        entry_callable.into(),
        &SelectedProviderPlanFacts::default(),
        Vec::new(),
    )
    .expect("component progress manifest");
    let progress = roots
        .seal_component_progress(manifest, [])
        .expect("sealed component progress");
    let profile = ProfileDecisionId::new(seed + 40).expect("profile decision");
    let installation = if commit {
        build_installation_record_with_evidence(
            &image,
            profile,
            std::iter::empty::<&ProviderExecution>(),
            Some(&progress),
        )
        .expect("committed installation record")
    } else {
        build_installation_record(&image, profile).expect("terminal installation record")
    };
    let artifact = bind_installed_artifact(object, image, installation, installed)
        .expect("installed terminal artifact");
    UnboundProgressInstallation {
        artifact,
        roots,
        progress,
    }
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
    callback_root_candidate_on_stack(
        entry,
        callback_stack(root, provider, relation, &boundary, code, entry),
    )
}

/// The callback candidate shape with an explicit bound stack composition, so
/// tests can admit compositions whose bound inputs span more than one root.
fn callback_root_candidate_on_stack(
    entry: EntryStubId,
    realization: BoundEpochStackComposition,
) -> ExternalRootCandidate {
    let root = root_id(701, ExternalRootId::from_normalized_identity);
    let provider = root_id(702, RootProviderId::from_normalized_identity);
    let relation = root_id(706, NestingRelationId::from_normalized_identity);
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
            realization,
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
/// concrete domain runs. An empty seal or a second lease for one domain
/// cannot describe one exact supply either. A lease sealed to another
/// installed occurrence is a cross-context disposition and rejects both at
/// seal and at admission. An install with no retained set, with no lease for
/// a demanded domain, or with capacity or alignment below the composed
/// demand all reject with the validated root, slot, and admission returned
/// for correction and retry.
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

    // An empty seal names no supply at all, and a second lease for one domain
    // would leave the provisioned supply ambiguous.
    let error = seal_external_stack_provision(fixture.runnable.installed(), [])
        .expect_err("an empty lease set cannot seal a provision");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("at least one admitted domain lease")
    );
    let error = seal_external_stack_provision(
        fixture.runnable.installed(),
        [
            admit_external_stack_domain_lease(
                fixture.runnable.installed(),
                StackDomain::Interrupted,
                8192,
                16,
                provisioner,
                lease_receipt,
            )
            .expect("first interrupted-domain stack lease"),
            admit_external_stack_domain_lease(
                fixture.runnable.installed(),
                StackDomain::Interrupted,
                4096,
                16,
                provisioner,
                lease_receipt,
            )
            .expect("second interrupted-domain stack lease"),
        ],
    )
    .expect_err("two leases for one domain cannot seal a provision");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("two external stack leases provision domain")
    );

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

    // A lease aligned below the composed domain demand rejects for the same
    // reason: supply, not its own claim, must meet the bound evidence.
    let under_aligned = seal_external_stack_provision(
        fixture.runnable.installed(),
        [admit_external_stack_domain_lease(
            fixture.runnable.installed(),
            StackDomain::Interrupted,
            8192,
            8,
            provisioner,
            lease_receipt,
        )
        .expect("under-aligned stack lease")],
    )
    .expect("under-aligned provision set");
    fixture
        .runnable
        .admit_external_stack_provision(under_aligned)
        .expect("under-aligned provision replaces while no roots are live");
    let mut runtime = fixture.runnable.external_root_runtime();
    let error = runtime
        .install(validated, slot, admission)
        .expect_err("a lease aligned below the composed domain demand rejects");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("below the composed alignment"),
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

/// The provision lane rejoins every bound epoch input in the artifact-wide
/// composition against the retained installed occurrence — the complete
/// installed-code context, not the compact installed-code and artifact
/// identities. Two placements issued the same compact identities cannot be
/// told apart by the old join, so bound stack evidence bound to one
/// occurrence could ride on stack supply retained for the other.
///
/// A cohort input bound to the foreign placement rejects at the provision
/// gate even though the installed root's own entry binding — the only input
/// the ledger's install check visits — is exact. Rejection returns the
/// validated root, slot, and admission so the corrected composition retries.
#[test]
fn external_stack_provision_rejects_cohort_evidence_for_another_occurrence() {
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let mut fixture = unprovisioned_runnable_fixture_at(600, 0x1000);
    let other = unprovisioned_runnable_fixture_at(600, 0x9000);
    assert_eq!(
        fixture.installed_code, other.installed_code,
        "occurrences collide on compact installed-code identity"
    );
    assert_eq!(
        fixture.runnable.installed().artifact(),
        other.runnable.installed().artifact(),
        "occurrences collide on compact artifact identity"
    );
    assert_ne!(
        fixture.runnable.installed().receipt_context(),
        other.runnable.installed().receipt_context(),
        "different exact placements retain distinct installed occurrence evidence"
    );

    // The artifact-wide bound composition retains the installed root's input
    // bound to this occurrence beside a cohort root's input bound to the
    // other placement under colliding compact identities.
    let boundary = callback_boundary();
    let relation = root_id(706, NestingRelationId::from_normalized_identity);
    let own_stack = callback_stack(
        root_id(701, ExternalRootId::from_normalized_identity),
        root_id(702, RootProviderId::from_normalized_identity),
        relation,
        &boundary,
        fixture.runnable.installed(),
        private_entry,
    );
    let foreign_stack = callback_stack(
        root_id(771, ExternalRootId::from_normalized_identity),
        root_id(772, RootProviderId::from_normalized_identity),
        relation,
        &boundary,
        other.runnable.installed(),
        private_entry,
    );
    let relation_evidence = StackNestingRelation {
        identity: relation,
        edges: BTreeSet::new(),
    };
    let merged = compose_bound_entry_stack_epochs(
        &relation_evidence,
        own_stack
            .inputs()
            .chain(foreign_stack.inputs())
            .map(|(_, input)| input),
    )
    .expect("cross-occurrence cohort composition");
    let cohort_validated = validate_external_root(
        callback_root_candidate_on_stack(private_entry, merged),
        &boundary,
    )
    .expect("cross-occurrence cohort root");
    let slot = RootSlotAuthority::from_admitted_owner(
        root_id(720, RootSlotId::from_normalized_identity),
        root_id(721, RootSlotOwnerId::from_normalized_identity),
    );
    let execution = ProviderExecution::from_admitted_provider(
        root_id(754, ProviderExecutionId::from_normalized_identity),
        &cohort_validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: cohort_validated.boundary().call.entry_control,
                restored_state: cohort_validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(704, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("cohort provider execution");
    let admission = RootAdmission::from_admitted_provider(
        root_id(722, RootAdmissionId::from_normalized_identity),
        &cohort_validated,
        &execution,
        fixture.runnable.installed(),
        &slot,
        cohort_validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("cohort root admission");

    // The retained provision binds this occurrence and covers the composed
    // demand; only the cohort input's foreign occurrence context rejects.
    let covering = callback_stack_provision(fixture.runnable.installed());
    fixture
        .runnable
        .admit_external_stack_provision(covering)
        .expect("covering provision admitted while no roots are live");
    let mut runtime = fixture.runnable.external_root_runtime();
    let error = runtime
        .install(cohort_validated, slot, admission)
        .expect_err("cohort bound epoch evidence naming another occurrence rejects");
    assert!(
        matches!(*error, ProvisionedRootInstallError::Provision { .. }),
        "rejection must come from the provision lane before ledger custody"
    );
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("different installed-code occurrence than the retained stack provision"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (_, slot, _) = (*error).into_parts();
    drop(runtime);

    // The rejected inputs return for correction: the same composition with
    // every bound input on the retained occurrence admits and installs.
    let cohort_own = callback_stack(
        root_id(771, ExternalRootId::from_normalized_identity),
        root_id(772, RootProviderId::from_normalized_identity),
        relation,
        &boundary,
        fixture.runnable.installed(),
        private_entry,
    );
    let clean = compose_bound_entry_stack_epochs(
        &relation_evidence,
        own_stack
            .inputs()
            .chain(cohort_own.inputs())
            .map(|(_, input)| input),
    )
    .expect("same-occurrence cohort composition");
    let clean_validated = validate_external_root(
        callback_root_candidate_on_stack(private_entry, clean),
        &boundary,
    )
    .expect("same-occurrence cohort root");
    let clean_execution = ProviderExecution::from_admitted_provider(
        root_id(754, ProviderExecutionId::from_normalized_identity),
        &clean_validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: clean_validated.boundary().call.entry_control,
                restored_state: clean_validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(704, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("same-occurrence provider execution");
    let clean_admission = RootAdmission::from_admitted_provider(
        root_id(722, RootAdmissionId::from_normalized_identity),
        &clean_validated,
        &clean_execution,
        fixture.runnable.installed(),
        &slot,
        clean_validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("same-occurrence root admission");
    let mut runtime = fixture.runnable.external_root_runtime();
    let root = runtime
        .install(clean_validated, slot, clean_admission)
        .expect("same-occurrence cohort composition installs under the retained provision");
    assert_eq!(
        root.root(),
        root_id(701, ExternalRootId::from_normalized_identity)
    );
}

/// `AdmittedExternalStackDomainLease` and `ProvisionedExternalStackSet` are
/// in-memory custody records: every representable field mutates
/// independently. A substitution either fails before a record exists — the
/// lease admission gates — or is rejected by the exact later seam that owns
/// its join: the seal binding each lease's occurrence triple and keying the
/// domain map, the component-admission gate retaining the set for this exact
/// occurrence, and the install-time rejoin re-authenticating the retained
/// set's occurrence triple and each demanded domain's supply before the
/// ledger sees the root. Provider-minted provenance on a lease (provisioner,
/// validation receipt), a lease's binding triple restated inside an
/// already-sealed set, a domain restated under its sealed key, and supply
/// over the composed demand are carried verbatim: sealing is the
/// lease-binding seam and the coverage joins are supply inequalities.
#[test]
fn external_stack_provision_rejects_every_one_field_substitution() {
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let mut fixture = unprovisioned_runnable_fixture_at(910, 0x1000);
    // The same seed at a second placement collides on the compact
    // installed-code and artifact identities; only the exact receipt context
    // differs. A different seed diverges every occurrence axis.
    let colliding = unprovisioned_runnable_fixture_at(910, 0x9000);
    let distinct = unprovisioned_runnable_fixture_at(920, 0x1000);
    assert_eq!(fixture.installed_code, colliding.installed_code);
    assert_eq!(
        fixture.runnable.installed().artifact(),
        colliding.runnable.installed().artifact()
    );
    assert_ne!(
        fixture.runnable.installed().receipt_context(),
        colliding.runnable.installed().receipt_context()
    );
    assert_ne!(fixture.installed_code, distinct.installed_code);
    assert_ne!(
        fixture.runnable.installed().artifact(),
        distinct.runnable.installed().artifact()
    );

    let foreign_code = distinct.installed_code;
    let foreign_context = colliding.runnable.installed().receipt_context();
    let foreign_artifact = distinct.runnable.installed().artifact();
    let foreign_provider = root_id(762, RootProviderId::from_normalized_identity);
    let foreign_validation = root_id(763, StackValidationReceiptId::from_normalized_identity);
    let provisioner = root_id(760, RootProviderId::from_normalized_identity);
    let lease_receipt = root_id(761, StackValidationReceiptId::from_normalized_identity);

    // Lease admission: fields that cannot describe real supply reject before
    // a record exists — an unresolved provider-selected disposition, zero
    // capacity, and a zero or non-power-of-two alignment.
    let admission_rejected: [(&str, StackDomain, u64, u64, &str); 4] = [
        (
            "provider-selected domain",
            StackDomain::ProviderSelected,
            8192,
            16,
            "cannot be provisioned",
        ),
        (
            "zero capacity",
            StackDomain::Interrupted,
            0,
            16,
            "requires nonzero capacity",
        ),
        (
            "zero alignment",
            StackDomain::Interrupted,
            8192,
            0,
            "power-of-two alignment",
        ),
        (
            "non-power-of-two alignment",
            StackDomain::Interrupted,
            8192,
            24,
            "power-of-two alignment",
        ),
    ];
    for (field, domain, capacity_bytes, alignment, fragment) in admission_rejected {
        let error = admit_external_stack_domain_lease(
            fixture.runnable.installed(),
            domain,
            capacity_bytes,
            alignment,
            provisioner,
            lease_receipt,
        )
        .expect_err(field);
        assert!(
            error.to_string().contains(fragment),
            "{field}: unexpected diagnostic: {error}"
        );
    }

    // Seal: the lease's retained occurrence triple is authenticated against
    // the set being sealed, and the roster must be nonempty with unique
    // domains. Rejection hands every supplied lease back for correction.
    let seal_rejected: [(&str, Box<dyn Fn(&mut AdmittedExternalStackDomainLease)>); 3] = [
        (
            "installed-code identity",
            Box::new(move |lease| *lease.installed_code_mut_for_test() = foreign_code),
        ),
        (
            "installed-code context",
            Box::new(move |lease| {
                *lease.installed_code_context_mut_for_test() = foreign_context.clone();
            }),
        ),
        (
            "artifact identity",
            Box::new(move |lease| *lease.artifact_mut_for_test() = foreign_artifact),
        ),
    ];
    for (field, mutate) in seal_rejected {
        let authentic = stack_lease(fixture.runnable.installed());
        let mut changed = authentic.clone();
        mutate(&mut changed);
        assert_ne!(
            changed, authentic,
            "{field}: substitution changes the lease"
        );
        let error = seal_external_stack_provision(fixture.runnable.installed(), [changed])
            .expect_err(field);
        assert!(
            error.diagnostic().to_string().contains(
                "different installed-code occurrence than the provision set being sealed"
            ),
            "{field}: unexpected diagnostic: {}",
            error.diagnostic()
        );
        assert_eq!(
            error.into_leases().len(),
            1,
            "{field}: supplied leases return for correction"
        );
    }
    let error = seal_external_stack_provision(fixture.runnable.installed(), [])
        .expect_err("an empty lease set cannot seal a provision");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("at least one admitted domain lease")
    );
    let error = seal_external_stack_provision(
        fixture.runnable.installed(),
        [
            stack_lease(fixture.runnable.installed()),
            stack_lease(fixture.runnable.installed()),
        ],
    )
    .expect_err("two leases for one domain cannot seal a provision");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("two external stack leases provision domain")
    );

    // Component admission: the set's retained occurrence triple is
    // authenticated before it can become this component's provision.
    let foreign_code = distinct.installed_code;
    let foreign_context = colliding.runnable.installed().receipt_context();
    let admit_rejected: [(&str, Box<dyn Fn(&mut ProvisionedExternalStackSet)>); 3] = [
        (
            "installed-code identity",
            Box::new(move |set| *set.installed_code_mut_for_test() = foreign_code),
        ),
        (
            "installed-code context",
            Box::new(move |set| {
                *set.installed_code_context_mut_for_test() = foreign_context.clone();
            }),
        ),
        (
            "artifact identity",
            Box::new(move |set| *set.artifact_mut_for_test() = foreign_artifact),
        ),
    ];
    for (field, mutate) in admit_rejected {
        let authentic = callback_stack_provision(fixture.runnable.installed());
        let mut changed = authentic.clone();
        mutate(&mut changed);
        assert_ne!(changed, authentic, "{field}: substitution changes the set");
        let error = fixture
            .runnable
            .admit_external_stack_provision(changed)
            .expect_err(field);
        assert!(
            error.diagnostic().to_string().contains(
                "different installed-code occurrence than the retained runnable component"
            ),
            "{field}: unexpected diagnostic: {}",
            error.diagnostic()
        );
        let recovered = (*error).into_provision();
        assert!(
            !recovered.binds_installed_code(fixture.runnable.installed()),
            "{field}: the rejected set still names its foreign occurrence"
        );
        assert!(
            fixture.runnable.external_stack_provision().is_none(),
            "{field}: rejection leaves the absent field untouched"
        );
    }

    // Install: with no provision retained at all the field's absent state
    // rejects before the ledger sees the root.
    let (validated, slot, admission) =
        callback_install_inputs(fixture.runnable.installed(), private_entry);
    let (mut validated, mut slot, mut admission) = expect_provision_rejection(
        &mut fixture.runnable,
        validated,
        slot,
        admission,
        "absent provision",
        "no admitted external stack provision",
    );

    // A lease admitted for the demanded domain but restated before the seal
    // lands under its restated key: the demanded domain has no provisioned
    // lease at the coverage join.
    let mut restated = stack_lease(fixture.runnable.installed());
    *restated.domain_mut_for_test() = StackDomain::Dedicated { class: 7 };
    let dedicated_only = seal_external_stack_provision(fixture.runnable.installed(), [restated])
        .expect("the restated domain keys the retained map");
    fixture
        .runnable
        .admit_external_stack_provision(dedicated_only)
        .expect("a set keyed on an undemanded domain still binds this occurrence");
    (validated, slot, admission) = expect_provision_rejection(
        &mut fixture.runnable,
        validated,
        slot,
        admission,
        "restated lease domain",
        "no admitted stack lease provisions domain",
    );

    // Corrupting the RETAINED record in place is the substitution an
    // in-memory custody family must survive: the install-time rejoin
    // re-authenticates the set's occurrence triple and each demanded
    // domain's supply before the ledger sees the root, and every rejection
    // returns the inputs for retry.
    let foreign_context = colliding.runnable.installed().receipt_context();
    let retained_rejected: [(&str, &str, Box<dyn Fn(&mut ProvisionedExternalStackSet)>); 8] = [
        (
            "set installed-code identity",
            "different installed-code occurrence",
            Box::new(move |set| *set.installed_code_mut_for_test() = foreign_code),
        ),
        (
            "set installed-code context",
            "different installed-code occurrence",
            Box::new(move |set| {
                *set.installed_code_context_mut_for_test() = foreign_context.clone();
            }),
        ),
        (
            "set artifact identity",
            "different installed-code occurrence",
            Box::new(move |set| *set.artifact_mut_for_test() = foreign_artifact),
        ),
        (
            "dropped demanded lease",
            "no admitted stack lease provisions domain",
            Box::new(|set| {
                set.leases_mut_for_test().remove(&StackDomain::Interrupted);
            }),
        ),
        (
            "lease capacity below demand",
            "below the composed 2048-byte demand",
            Box::new(|set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .capacity_bytes_mut_for_test() = 1024;
            }),
        ),
        (
            "lease zero capacity",
            "below the composed 2048-byte demand",
            Box::new(|set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .capacity_bytes_mut_for_test() = 0;
            }),
        ),
        (
            "lease alignment below demand",
            "below the composed alignment",
            Box::new(|set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .alignment_mut_for_test() = 8;
            }),
        ),
        (
            "lease zero alignment",
            "below the composed alignment",
            Box::new(|set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .alignment_mut_for_test() = 0;
            }),
        ),
    ];
    for (field, fragment, mutate) in retained_rejected {
        *fixture.runnable.external_stack_provision_mut_for_test() =
            Some(callback_stack_provision(fixture.runnable.installed()));
        mutate(
            fixture
                .runnable
                .external_stack_provision_mut_for_test()
                .as_mut()
                .expect("retained provision"),
        );
        (validated, slot, admission) = expect_provision_rejection(
            &mut fixture.runnable,
            validated,
            slot,
            admission,
            field,
            fragment,
        );
    }

    // Substitutions the later seams deliberately do not authenticate stay
    // carried: lease provenance is provider-minted at admission and the seal
    // is the lease-binding seam, while the coverage join is a supply
    // inequality that over-satisfying, desynced, or undemanded leases still
    // meet.
    let foreign_context = colliding.runnable.installed().receipt_context();
    let carried: [(&str, Box<dyn Fn(&mut ProvisionedExternalStackSet)>); 8] = [
        (
            "lease provisioner",
            Box::new(move |set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .provisioner_mut_for_test() = foreign_provider;
            }),
        ),
        (
            "lease validation receipt",
            Box::new(move |set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .validation_receipt_mut_for_test() = foreign_validation;
            }),
        ),
        (
            "lease installed-code identity",
            Box::new(move |set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .installed_code_mut_for_test() = foreign_code;
            }),
        ),
        (
            "lease installed-code context",
            Box::new(move |set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .installed_code_context_mut_for_test() = foreign_context.clone();
            }),
        ),
        (
            "lease artifact identity",
            Box::new(move |set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .artifact_mut_for_test() = foreign_artifact;
            }),
        ),
        (
            "lease domain restated under its sealed key",
            Box::new(|set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .domain_mut_for_test() = StackDomain::Dedicated { class: 7 };
            }),
        ),
        (
            "over-provisioned lease capacity",
            Box::new(|set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .capacity_bytes_mut_for_test() = 16_384;
            }),
        ),
        (
            "over-aligned lease supply",
            Box::new(|set| {
                *set.leases_mut_for_test()
                    .get_mut(&StackDomain::Interrupted)
                    .expect("interrupted lease")
                    .alignment_mut_for_test() = 32;
            }),
        ),
    ];
    for (field, mutate) in carried {
        let authentic = callback_stack_provision(fixture.runnable.installed());
        let mut changed = authentic.clone();
        mutate(&mut changed);
        assert_ne!(changed, authentic, "{field}: substitution changes the set");
        assert!(
            changed.binds_installed_code(fixture.runnable.installed()),
            "{field}: a lease-row substitution leaves the set binding intact"
        );
        assert!(
            changed.covers_root_demand(&validated).is_ok(),
            "{field}: a post-seal lease substitution is carried, not re-authenticated"
        );
    }

    // Undemanded supply and a lease bound to another occurrence inserted
    // post-seal are likewise roster content the coverage join does not visit
    // — sealing is the only lease-binding seam.
    let mut changed = callback_stack_provision(fixture.runnable.installed());
    changed.leases_mut_for_test().insert(
        StackDomain::Dedicated { class: 9 },
        admit_external_stack_domain_lease(
            fixture.runnable.installed(),
            StackDomain::Dedicated { class: 9 },
            8192,
            16,
            provisioner,
            lease_receipt,
        )
        .expect("undemanded lease"),
    );
    changed.leases_mut_for_test().insert(
        StackDomain::Dedicated { class: 11 },
        admit_external_stack_domain_lease(
            distinct.runnable.installed(),
            StackDomain::Dedicated { class: 11 },
            8192,
            16,
            provisioner,
            lease_receipt,
        )
        .expect("foreign-bound lease"),
    );
    assert!(
        changed.covers_root_demand(&validated).is_ok(),
        "undemanded or post-seal foreign supply does not break coverage"
    );

    // Carried provenance still rides the retained record through the complete
    // install seam: a substituted lease provisioner installs, and the live
    // root then pins the provision field against replacement.
    let mut changed = callback_stack_provision(fixture.runnable.installed());
    *changed
        .leases_mut_for_test()
        .get_mut(&StackDomain::Interrupted)
        .expect("interrupted lease")
        .provisioner_mut_for_test() = foreign_provider;
    *fixture.runnable.external_stack_provision_mut_for_test() = Some(changed);
    let mut runtime = fixture.runnable.external_root_runtime();
    let root = runtime
        .install(validated, slot, admission)
        .expect("provider-minted provenance is carried through install");
    assert_eq!(
        root.root(),
        root_id(701, ExternalRootId::from_normalized_identity)
    );
    drop(root);
    drop(runtime);
    let error = fixture
        .runnable
        .admit_external_stack_provision(callback_stack_provision(fixture.runnable.installed()))
        .expect_err("the retained provision is pinned while a root is live");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("pinned while external roots are live")
    );
}

/// The retained `InstalledRunnableComponent` is the crate's installation
/// record: the bound artifact, the claimed root registry, the optional
/// committed progress acceptance, and the retained stack provision. Its
/// binding gate joins each field to the exact installed occurrence, the era
/// ledger's publish and retire gates bind the retained record to its
/// candidate axes and era, and the provision field's admit and pin seams
/// guard substitution. Every representable substitution below rejects at the
/// exact seam that owns its join. The progress acceptance is opaque custody
/// sealed inside external-roots — its own field surface is mutation-covered
/// there — so the substitutions this record can express are absent, foreign,
/// or divergent acceptances, each rejected at binding.
#[test]
fn installed_runnable_component_rejects_every_one_field_substitution() {
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");

    // roots: a registry claimed on another occurrence cannot bind this
    // artifact — the ledger's retained context, not the compact identity,
    // names the occurrence. Compact identities collide between these two
    // fixtures; the join still rejects, in both substitution directions.
    let own = unbound_installation(930, 0x1000);
    let foreign = unbound_installation(930, 0x9000);
    assert_eq!(
        own.artifact.installed().identity(),
        foreign.artifact.installed().identity(),
        "compact installed-code identities collide"
    );
    assert_ne!(
        own.artifact.installed().receipt_context(),
        foreign.artifact.installed().receipt_context(),
        "exact installed occurrence contexts differ"
    );
    let error = bind_installed_runnable_component(own.artifact, foreign.roots, None)
        .expect_err("a registry claimed on another occurrence cannot bind this artifact");
    assert!(
        error
            .diagnostic()
            .contains("installation registry names a different installed-code occurrence"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let own = unbound_installation(931, 0x1000);
    let foreign = unbound_installation(931, 0x9000);
    let error = bind_installed_runnable_component(foreign.artifact, own.roots, None)
        .expect_err("an artifact bound to another occurrence cannot bind this registry");
    assert!(
        error
            .diagnostic()
            .contains("installation registry names a different installed-code occurrence"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );

    // roots: the registry must already be sealed with the exact
    // provider-occurrence closure the artifact's record names; an unsealed
    // registry cannot bind, and a closure sealed over a divergent selected
    // plan set cannot either. Rejection returns the pieces for correction.
    let pieces = unbound_installation(932, 0x1000);
    let error = bind_installed_runnable_component(pieces.artifact, pieces.roots, None)
        .expect_err("an unsealed registry cannot bind");
    assert!(
        error
            .diagnostic()
            .contains("requires a sealed provider-occurrence closure"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (artifact, mut roots, _) = (*error).into_parts();
    let divergent = SelectedProviderPlanFacts::from_selected_plans(vec![ProviderPlan {
        name: "scheduler".into(),
        schema: ServiceSchema {
            trait_name: "Scheduler".into(),
            ..ServiceSchema::default()
        },
        ..ProviderPlan::default()
    }])
    .expect("one-plan selected closure");
    let plan = divergent.plans()[0].clone();
    roots
        .seal_provider_occurrence_closure(
            &divergent,
            [ProviderOccurrencePlanBinding::new(
                plan.report_fingerprint(),
                plan,
                ProviderOccurrenceInstallationReceipt::from_provider(
                    root_id(
                        970,
                        ProviderOccurrenceInstallationReceiptId::from_normalized_identity,
                    ),
                    artifact.installed(),
                    root_id(980, InstalledProviderOccurrenceId::from_normalized_identity),
                    "Provider",
                ),
            )],
        )
        .expect("divergent provider-occurrence closure seals");
    let error = bind_installed_runnable_component(artifact, roots, None)
        .expect_err("a divergent provider-occurrence closure cannot bind");
    assert!(
        error
            .diagnostic()
            .contains("different selected provider-plan closures"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );

    // roots: a registry already holding a live external root belongs to that
    // root's slot owner, not to this binding.
    let (object, image) = terminal_image();
    let mut installed = install_terminal_text(&object, 953, 954, 0x1000);
    let mut roots = InstalledRootLedger::claim(&mut installed).expect("installation registry");
    roots
        .seal_provider_occurrence_closure(&SelectedProviderPlanFacts::default(), [])
        .expect("empty provider occurrence closure");
    let (validated, slot, admission) = callback_install_inputs(&installed, private_entry);
    let live = roots
        .install(&installed, validated, slot, admission)
        .expect("root installs into the claimed registry");
    drop(live);
    let installation = build_installation_record(
        &image,
        ProfileDecisionId::new(990).expect("profile decision"),
    )
    .expect("terminal installation record");
    let artifact = bind_installed_artifact(object, image, installation, installed)
        .expect("installed terminal artifact");
    let error = bind_installed_runnable_component(artifact, roots, None)
        .expect_err("a registry with a live root is owned by that root's slot");
    assert!(
        error
            .diagnostic()
            .contains("installed external roots require their own live owner"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );

    // progress: committed by the record but withheld at binding rejects, as
    // does an acceptance the record never committed. A divergent acceptance
    // bound to this occurrence and an acceptance bound to a foreign
    // occurrence each reject on their own identity join.
    let pieces = unbound_progress_installation(940, 0x1000, "Codec::start", true);
    let error = bind_installed_runnable_component(pieces.artifact, pieces.roots, None)
        .expect_err("committed progress cannot be dropped at binding");
    assert!(
        error
            .diagnostic()
            .contains("commits component progress but the opaque acceptance was not supplied"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let uncommitted = unbound_progress_installation(941, 0x1000, "Codec::start", false);
    let error = bind_installed_runnable_component(
        uncommitted.artifact,
        uncommitted.roots,
        Some(uncommitted.progress),
    )
    .expect_err("uncommitted progress cannot be supplied at binding");
    assert!(
        error
            .diagnostic()
            .contains("omits the supplied component-progress acceptance"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let mut pieces = unbound_progress_installation(942, 0x1000, "Codec::start", true);
    let divergent_manifest = ComponentProgressManifest::bind(
        "Codec::other".into(),
        &SelectedProviderPlanFacts::default(),
        Vec::new(),
    )
    .expect("divergent component progress manifest");
    let divergent_progress = pieces
        .roots
        .seal_component_progress(divergent_manifest, [])
        .expect("a second manifest seals its own acceptance");
    let error =
        bind_installed_runnable_component(pieces.artifact, pieces.roots, Some(divergent_progress))
            .expect_err("a divergent acceptance cannot bind this record");
    assert!(
        error
            .diagnostic()
            .contains("commits different component-progress identities"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let foreign = unbound_progress_installation(943, 0x9000, "Codec::start", false);
    let committed = unbound_progress_installation(944, 0x1000, "Codec::start", true);
    let error = bind_installed_runnable_component(
        committed.artifact,
        committed.roots,
        Some(foreign.progress),
    )
    .expect_err("a foreign-occurrence acceptance cannot bind this record");
    assert!(
        error
            .diagnostic()
            .contains("component-progress acceptance names a different installed-code occurrence"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    // The authentic committed pair binds and retains the acceptance.
    let pair = unbound_progress_installation(945, 0x1000, "Codec::start", true);
    let runnable =
        bind_installed_runnable_component(pair.artifact, pair.roots, Some(pair.progress))
            .expect("the committed progress acceptance binds");
    assert!(
        runnable.progress().is_some(),
        "the retained record carries the committed acceptance"
    );

    // external_stack_provision: the field's representable states — absent,
    // foreign, admitted, corrected while quiescent, and pinned while a root
    // is live — each hold at their own seam. The retained record's own field
    // surface is covered by
    // `external_stack_provision_rejects_every_one_field_substitution`.
    let mut fixture = unprovisioned_runnable_fixture_at(946, 0x1000);
    let foreign = unprovisioned_runnable_fixture_at(946, 0x9000);
    assert!(fixture.runnable.external_stack_provision().is_none());
    let error = fixture
        .runnable
        .admit_external_stack_provision(callback_stack_provision(foreign.runnable.installed()))
        .expect_err("a foreign-bound set cannot substitute the provision field");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("different installed-code occurrence than the retained runnable component"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let returned = (*error).into_provision();
    assert!(
        returned.binds_installed_code(foreign.runnable.installed()),
        "the returned set still binds its own occurrence"
    );
    assert!(fixture.runnable.external_stack_provision().is_none());
    fixture
        .runnable
        .admit_external_stack_provision(callback_stack_provision(fixture.runnable.installed()))
        .expect("authentic provision admitted while quiescent");
    fixture
        .runnable
        .admit_external_stack_provision(callback_stack_provision(fixture.runnable.installed()))
        .expect("the provision field may still be corrected while quiescent");
    let (validated, slot, admission) =
        callback_install_inputs(fixture.runnable.installed(), private_entry);
    let mut runtime = fixture.runnable.external_root_runtime();
    let root = runtime
        .install(validated, slot, admission)
        .expect("covering provision installs");
    drop(root);
    drop(runtime);
    let error = fixture
        .runnable
        .admit_external_stack_provision(callback_stack_provision(fixture.runnable.installed()))
        .expect_err("the provision field is pinned while a root is live");
    assert!(
        error
            .diagnostic()
            .to_string()
            .contains("pinned while external roots are live")
    );

    // Era publication joins the retained record to the candidate's exact
    // occurrence axes before the lifecycle sees it; a receipt minted for
    // another candidate rejects at the lifecycle seam with every piece
    // returned, and one era retains exactly one record. Three live-era slots
    // keep every leg exercisable in one ledger.
    let mut ledger = RunnableComponentEraLedger::new(
        ComponentEraEntryLedger::new(
            ComponentEraLedgerId::from_normalized_identity(1).expect("ledger"),
            "CodecBinding/v1".into(),
            "CodecEntry/v1".into(),
            3,
            tcb_acceptance("platform", 1),
        )
        .expect("component lifecycle"),
    );
    let era_a = runnable_fixture_at(960, 0x1000);
    let era_b = runnable_fixture_at(970, 0x9000);
    let era_c = unprovisioned_runnable_fixture_at(980, 0x1_1000);

    // A publication receipt minted for another candidate is rejected by the
    // lifecycle and forwarded with every piece intact for correction.
    let candidate_c = candidate(30, &era_c.runnable);
    let other_candidate = candidate(31, &era_c.runnable);
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        41,
        ledger.lifecycle(),
        &other_candidate,
        true,
        false,
    );
    let error = ledger
        .publish(candidate_c, receipt, era_c.runnable)
        .expect_err("a receipt minted for another candidate cannot publish this record");
    assert!(
        error
            .diagnostic()
            .contains("does not bind and expose the exact candidate"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (candidate_c, _receipt, runnable_c) = (*error).into_parts();
    let honest_receipt = ComponentEraPublicationReceipt::from_runtime(
        42,
        ledger.lifecycle(),
        &candidate_c,
        true,
        false,
    );
    ledger
        .publish(candidate_c, honest_receipt, runnable_c)
        .expect("the corrected pair publishes era 30");

    // The candidate's occurrence axes bind the retained record — not its own
    // claims — at the crate gate.
    let mut forged = candidate(10, &era_a.runnable);
    forged.artifact_occurrence_digest = era_b.runnable.installed().occurrence_digest();
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(43, ledger.lifecycle(), &forged, true, true);
    let error = ledger
        .publish(forged, receipt, era_a.runnable)
        .expect_err("a foreign occurrence digest cannot publish this record");
    assert!(
        error
            .diagnostic()
            .contains("different installed artifact occurrence"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (mut candidate_a, _receipt, runnable_a) = (*error).into_parts();
    // Restore the digest axis and substitute the compact report identity
    // alone: the same join still rejects on the retained record's authority.
    candidate_a.artifact_occurrence_digest = runnable_a.installed().occurrence_digest();
    candidate_a.artifact_instance_compatibility_report_identity =
        era_b.runnable.installed_code().normalized_identity();
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        44,
        ledger.lifecycle(),
        &candidate_a,
        true,
        true,
    );
    let error = ledger
        .publish(candidate_a, receipt, runnable_a)
        .expect_err("a foreign compatibility report cannot publish this record");
    assert!(
        error
            .diagnostic()
            .contains("different installed artifact occurrence"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (_candidate_a, _receipt, runnable_a) = (*error).into_parts();

    let authentic_a = candidate(10, &runnable_a);
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        45,
        ledger.lifecycle(),
        &authentic_a,
        true,
        true,
    );
    ledger
        .publish(authentic_a, receipt, runnable_a)
        .expect("authentic era 10 publishes");
    let duplicate = candidate(10, &era_b.runnable);
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        46,
        ledger.lifecycle(),
        &duplicate,
        true,
        true,
    );
    let error = ledger
        .publish(duplicate, receipt, era_b.runnable)
        .expect_err("one era retains exactly one runnable record");
    assert!(
        error
            .diagnostic()
            .contains("already retains runnable installation evidence"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let (_candidate, _receipt, runnable_b) = (*error).into_parts();
    let authentic_b = candidate(20, &runnable_b);
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        47,
        ledger.lifecycle(),
        &authentic_b,
        true,
        true,
    );
    ledger
        .publish(authentic_b, receipt, runnable_b)
        .expect("authentic era 20 publishes");

    // Era retirement releases the retained record only through the exact
    // seam: an era with no retained record rejects at the crate gate, a live
    // non-quiescent era rejects at the lifecycle with its receipt returned,
    // and a quiescent noncurrent era hands the complete installation record —
    // artifact, registry, progress, and retained provision — back intact.
    let error = ledger
        .retire(ComponentEraRetirementReceipt::from_runtime(
            7,
            ledger.lifecycle(),
            99,
            true,
        ))
        .expect_err("an era with no retained record cannot retire");
    assert!(
        error
            .diagnostic()
            .contains("has no retained runnable installation evidence"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let error = ledger
        .retire(ComponentEraRetirementReceipt::from_runtime(
            8,
            ledger.lifecycle(),
            20,
            true,
        ))
        .expect_err("the current era cannot retire");
    assert!(
        error.diagnostic().contains("noncurrent quiescent"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let _receipt = error.into_receipt();
    ledger
        .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
            ledger.lifecycle(),
            10,
            0,
            true,
        ))
        .expect("closed era 10 establishes quiescence");
    let retired = ledger
        .retire(ComponentEraRetirementReceipt::from_runtime(
            9,
            ledger.lifecycle(),
            10,
            true,
        ))
        .expect("quiescent era 10 retires");
    assert_eq!(retired.installed().identity(), era_a.installed_code);
    let provision = retired
        .external_stack_provision()
        .expect("the admitted provision survives retirement");
    assert!(
        provision.binds_installed_code(retired.installed()),
        "the retained provision still binds the retired occurrence"
    );
    assert!(retired.progress().is_none());
    let (_artifact, _roots, _progress, provision) = retired.into_parts();
    assert!(
        provision.is_some(),
        "custody decomposition hands the provision back"
    );
}

/// The authored customer REGISTERED-CALLBACK-LIFETIME pins: `register` returns
/// the claimed linear `Registration` under `Registration::Live`, and
/// `unregister` consumes that exact qualification. This is the same program
/// the pipeline contract test `registered_callback_lifetime.rs` lowers and
/// The authored customer REGISTERED-CALLBACK-LIFETIME pins for teardown: the
/// program receives the live registration as an entry claim and ends it
/// through `unregister` — the pinned contract's dispatch leg, which
/// `registered_callback_lifetime.rs` interprets end to end. The register leg
/// of that program (`register -> Registration in Registration::Live`) is
/// still gated by the check-layer call-result qualification seam, so this
/// witness drives the legs the authored program can already express:
/// unregistration, root quiescence, and component-era lease release.
const CALLBACK_REGISTRATION_PROGRAM: &str = r#"
    data RegistrationSlot {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Registration [linear] { slot: u64; }

    domain Registration::Live
    established by Registrar::register, Registrar::unregister;

    machine Live::content(registration: &Registration) -> CountedQuantity<RegistrationSlot>
    satisfies Content<CountedQuantity<RegistrationSlot>>::project
    {
        CountedQuantity { magnitude: 1 }
    }

    boundary trait Registrar {
        machine register(registration: Registration) -> Registration in Registration::Live;

        machine unregister(registration: Registration in Live);
    }

    data Customer {}
    machine Customer::run(&mut self, registered: Registration in Live)
    reaches Registrar invokes Registrar;
    {
        Registrar::unregister(registered);
    }
"#;

/// Joins one interpreted program's `unregister` boundary call to the real
/// registration custody chain. Construction supplies the already-lowered
/// linear `Registration` (root admission, provider registration, and lease
/// acquisition ran during harness setup); the program's `unregister` call
/// then performs provider unregistration, exact root quiescence, and
/// component-era lease release in the program's own order — a real caller
/// ending the root, not a Rust test's sequenced calls.
///
/// The runtime carries 'static custody pieces because the ledger is leaked
/// for post-execution inspection; `Registration`'s `code` lifetime is the
/// runtime's own, so the live registration rests beside it until the boundary
/// call consumes it.
struct ProgramDrivenUnregister {
    unregister: BoundaryMachineId,
    calls: Vec<(BoundaryMachineId, Vec<TerminalStructuralValue>)>,
    runtime: RunnableComponentCallbackRegistrationRuntime<'static>,
    unregistration_receipt_identity: OpaqueCallbackUnregistrationReceiptId,
    removal: Option<RootRemovalReceipt>,
    live: Option<Registration<'static>>,
    completed: Option<CompletedRegistration>,
}

impl TerminalEffectHandler for ProgramDrivenUnregister {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            boundary,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new(
                "only boundary effects are expected",
            ));
        };
        self.calls.push((*boundary, structural_arguments.clone()));
        Ok(())
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { boundary, .. } = effect else {
            return Err(TerminalEffectRejection::new(
                "only boundary effects are expected",
            ));
        };
        if *boundary != self.unregister {
            return Err(TerminalEffectRejection::new("unexpected boundary call"));
        }
        self.handle_effect(effect)?;
        let live = self.live.take().expect("the live registration is present");
        let provider_receipt = OpaqueCallbackUnregistrationReceipt::from_provider(
            self.unregistration_receipt_identity,
            live.registration(),
            true,
        );
        let unregistered = live
            .unregister_and_quiesce(
                &mut self.runtime,
                provider_receipt,
                self.removal.take().expect("removal evidence retained"),
            )
            .expect("program-driven unregister and quiescence");
        self.completed = Some(
            unregistered
                .release_component_era(&mut self.runtime)
                .expect("program-driven component-era release"),
        );
        Ok(TerminalEffectResult::Unit)
    }
}

/// The authored customer's teardown leg driving the ledger, witnessed end to
/// end: the interpreted program's `unregister` boundary call quiesces the
/// root and releases the exact component-era lease the lowered registration
/// held — program operations, not a Rust caller's sequence.
#[test]
fn interpreted_unregister_drives_registration_ledger_teardown() {
    let tokens = Lexer::new(CALLBACK_REGISTRATION_PROGRAM)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = lower_machine(&checked, "Customer::run").expect("lower registration program");
    let module = &lowered.semantic_module;
    let unregister_boundary = module
        .boundary_machines
        .iter()
        .find(|boundary| boundary.identity.contains("Registrar::unregister"))
        .expect("unregister boundary retained");
    let registration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.identity.contains("Registration"))
        .expect("Registration declaration");
    let domain = module
        .structural_domains
        .iter()
        .find(|domain| domain.identity.contains("Registration::Live"))
        .expect("Live domain");
    let terminal_psi::StructuralTypeShape::Record { fields } = &registration.shape else {
        panic!("Registration is a record")
    };
    let slot = fields
        .iter()
        .find(|field| field.identity.contains("slot"))
        .expect("slot field");

    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("registration program verifies");
    let module_bytes = encode_module(module).expect("encode module");
    let proof_bytes = encode_proof_section(module, &lowered.proof_bundle).expect("encode proof");

    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let private_function = callback_private_function_identity();
    let fixture = runnable_fixture(800);
    let attribution = bind_installed_compiler_private_function_entry(
        fixture.runnable.installed_artifact(),
        private_function,
        private_entry,
    )
    .expect("private callback attribution");
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
    let retained: &'static mut RunnableComponentEraLedger = Box::leak(Box::new(lifecycle));
    let mut runtime: RunnableComponentCallbackRegistrationRuntime<'static> = retained
        .callback_registration_runtime(10)
        .expect("retained callback runtime");

    // Registration occurred before the program ran: root admission, provider
    // registration, lease acquisition, and lowering are the chain's register
    // leg, which the authored program cannot yet express — the check layer
    // still gates a claimed linear boundary result.
    let (validated, slot_authority, admission) =
        callback_install_inputs(runtime.installed(), private_entry);
    let root = runtime
        .install(validated, slot_authority, admission)
        .expect("callback root admission");
    let removal = RootRemovalReceipt::from_provider(
        root_id(781, RootRemovalReceiptId::from_normalized_identity),
        &root,
        true,
        true,
    );
    let capacity_identity = root_id(
        789,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let provider = root_id(784, OpaqueCallbackProviderId::from_normalized_identity);
    let capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let receipt = OpaqueCallbackRegistrationReceipt::from_provider(
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
    let registered = runtime
        .admit_compiler_private_callback(attribution, root, receipt, capacity)
        .expect("provider registration");
    let lease = runtime
        .acquire_registration_lease(
            ProgramLocalRootEpochLeaseId::from_normalized_identity(798)
                .expect("callback component-era lease"),
        )
        .expect("component-era lease");
    let live = runtime
        .lower_registration(registered, lease)
        .expect("linear registration");

    // The program's entry claim is the live registration the chain lowered.
    let registered_input = TerminalStructuralValue {
        opaque_identity: 41,
        structural_type: registration.id,
        qualifications: vec![domain.id],
        path: Vec::new(),
    };
    let inputs = TerminalStructuralInputs {
        arguments: std::slice::from_ref(&registered_input),
        scalar_fields: &[TerminalStructuralScalarFieldValue {
            argument_index: 0,
            path: Vec::new(),
            field: slot.id,
            value: TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(3),
            },
        }],
        ..Default::default()
    };
    let mut driver = ProgramDrivenUnregister {
        unregister: unregister_boundary.id,
        calls: Vec::new(),
        runtime,
        unregistration_receipt_identity: root_id(
            791,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        removal: Some(removal),
        live: Some(live),
        completed: None,
    };

    let execution = interpret_terminal_artifact_measured(
        &module_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        &[],
        inputs,
        &mut driver,
    )
    .expect("registration teardown program interprets");

    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    let [(call, arguments)] = driver.calls.as_slice() else {
        panic!("one boundary call observed")
    };
    assert_eq!(*call, unregister_boundary.id);
    assert_eq!(arguments.as_slice(), &[registered_input]);
    let completed = driver
        .completed
        .expect("the program's unregister released the component-era lease");
    let (completed_attribution, completion) = completed.into_parts();
    assert_eq!(completed_attribution.entry(), private_entry);
    let (returned_slot, returned_capacity) = completion.into_parts();
    assert_eq!(
        returned_slot.slot(),
        root_id(720, RootSlotId::from_normalized_identity)
    );
    assert_eq!(returned_capacity.identity(), capacity_identity);
    assert_eq!(
        driver.runtime.component_era_lease_holds(),
        Some(0),
        "the program's unregister released the exact component-era hold"
    );
}

//! Real-source proof that Psi emits a self-contained terminal module: frontend
//! trees are dropped before canonical decoding, verification, interpretation,
//! and Omega lowering.

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractOperation, AbstractOperationPlan,
    AbstractParameter, ValueBinding,
};
use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_artifacts::external_root_manifest_json;
use omega_assigned_target_operations::{
    AssignedBooleanControl, AssignedIntegerControl, AssignedOperation,
};
use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, CallSignature,
    CallingPolicy, EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineRegister,
    MachineState, MachineStateSet, RegisterSet, StackDomainRef, StateFootprintEvidence,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_domain_closure,
    validate_entry_stack_realization,
};
use omega_compiler::{
    ArtifactEmissionPolicy, CheckedCompilation, CompileOptions, CompileRequest,
    RequestedCompileProduct, compile_to_checked,
};
use omega_component_candidate::{ComponentCandidate, ComponentCandidateParts};
use omega_component_deployment::{
    ComponentProgressAttestationBinding, DynamicNativeFuelRootDeployment,
    begin_component_deployment, begin_component_deployment_with_claimed_registry,
    publish_component_flat_output,
};
use omega_component_publication::{
    InstalledRunnableComponent, RunnableComponentEraLedger, bind_installed_runnable_component,
};
use omega_effects::{
    ComponentEraCandidate, ComponentEraEntryLedger, ComponentEraLedgerId,
    ComponentEraPublicationReceipt, ExecutableTcbManifest, ExecutableTcbProfile, ExecutionScope,
    IncompleteScopePolicy, ScopeCompleteness, evaluate_executable_tcb_profile,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
    ArtifactId, CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_external_roots::{
    AdapterStackRealizationOrigin, AdmittedOpaqueFuelSuspensionFree, ArrivalStackRealizationOrigin,
    ComponentProgressDemandIdentity, DynamicNativeFuelMeterPlan, ExternalRootCandidate,
    ExternalRootId, FixedFuelProviderSummary, FuelProvisionId, FuelSuspensionValidationReceiptId,
    FuelValidationReceiptId, InstalledProviderOccurrenceId, InstalledRootLedger,
    LogicalFuelResourceColumn, MachineStateResourceColumn, NativeFuelActivationStateSlot,
    NativeFuelContextLayout, NativeFuelExecutionEnvironment, NativeFuelMeterPlanId,
    NativeFuelRuntimeEntryIdentity, NativeFuelSavedValue, NativeFuelSponsorStackPlan,
    NativeFuelTargetPlanProjection, NativeFuelTransferRuntimePlanProjection, NestingRelationId,
    OpaqueProviderExitAssurance, ProgressProfileEstablishmentAttestation,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId, ProviderExecution,
    ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId,
    ProviderOccurrenceInstallationReceipt, ProviderOccurrenceInstallationReceiptId,
    ProviderOccurrencePlanBinding, ProviderPlanId, ProviderStackSummary, RootAdmission,
    RootAdmissionId, RootProviderId, RootRemovalReceipt, RootRemovalReceiptId, RootSlotAuthority,
    RootSlotId, RootSlotOwnerId, SponsorContextTransport, StackNestingRelation,
    StackResourceColumn, StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
    admit_fixed_native_fuel, admit_native_fuel_target_policy, admit_native_fuel_transfer_plan,
    admit_opaque_arrival_context_set, bind_direct_generated_entry_stack_realization,
    bind_installed_dynamic_fuel_attribution, bind_installed_entry_fuel, bind_installed_entry_stack,
    bind_installed_native_fuel_sponsor_route, bind_installed_native_fuel_transfer_code,
    bind_installed_native_fuel_transfer_runtime, bind_opaque_adapter_stack_realization,
    bind_suspension_free_fixed_fuel, compose_bound_entry_stack_epochs, compose_fixed_fuel,
    derive_fuel_suspension_free, validate_dynamic_fuel_attribution_basis, validate_external_root,
    validate_installed_entry_fuel, validate_installed_entry_stack,
};
use omega_image_emission::{
    ObjectArtifact, bind_installed_artifact, build_installation_record, build_object_artifact,
    decode_installation_record, derive_installation_stack_demand, derive_stack_demand,
    emit_executable_image, emit_object_container, encode_installation_record,
    installation_fingerprint, validate_installation_record,
};
use omega_installation_evidence::{
    FuelAttributionEvidence, FuelAttributionSite, NativeFuelChargeEvidence,
    NativeFuelImageEvidence, NativeFuelRuntimeTextEvidence, NativeFuelRuntimeTextSpan,
    NativeFuelTransferRuntimeEvidence, NativeFuelTransferRuntimeImageEvidence, ObjectEvidence,
};
use omega_machine_emission::emit_machine_code;
use omega_native_artifact::{NativeArtifact, NativeArtifactParts};
use omega_native_differential_test::{
    admit_native_provider, admit_native_provider_for_selected_plan,
};
use omega_psi_to_abstract_operations::{ArtifactLoweringError, lower_artifact_sections};
use omega_target::{NativeTarget, TargetProfile};
use omega_target_operations::{
    LinuxExitGroupI32Realization, TargetBooleanControl, TargetBooleanExpression,
    TargetIntegerControl, TargetIntegerExpression, TargetOperation,
};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use omega_terminal_psi_to_native_artifact::{
    NativeProviderSettlement as ComponentProviderSettlement, realize_native_artifact,
};
use psi_checked_trees_to_terminal::{LoweringError, lower_machine};
use psi_core::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ProfileDecisionId, ScalarType, ValueId,
};
use psi_extents::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRightId, ExtentRights,
    ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    CrashCause, CrashRouteGuard, OperationKind, TerminalModule, Terminator, VocabularyMarker,
};
use psi_terminal_codec::{
    DebugSubject, build_artifact_manifest, decode_debug_map, decode_module, decode_proof_bundle,
    encode_debug_map, encode_module, encode_proof_bundle, terminal_psi_identity,
    validate_artifact_manifest,
};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    MeasuredTerminalExecution, TerminalArtifactInterpretError, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    interpret_terminal_artifact_measured,
};
use psi_terminal_verifier::{VerifiedTerminalModule, verify_module};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

#[cfg(unix)]
static NEXT_SCRATCH_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn terminal_source_canary(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .join("tests/omega/pass/terminal_psi")
        .join(name)
        .join("main.omg")
}

fn source_canary() -> PathBuf {
    terminal_source_canary("integer_control_contract")
}

fn progress_source_canary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .join("tests/omega/pass/progress/provider_receiver_progress_installation/main.omg")
}

fn progress_free_selected_source_canary() -> PathBuf {
    terminal_source_canary("selected_empty_component")
}

fn selected_optimizer_source_canary() -> PathBuf {
    terminal_source_canary("selected_optimizer_component")
}

fn selected_lowering_optimizer_source_canary() -> PathBuf {
    terminal_source_canary("selected_lowering_optimizer_component")
}

fn stage_terminal_component(
    checked: &CheckedCompilation,
    target: NativeTarget,
    subsystem: u16,
    profile: &AdmissionProfile,
    settlements: &[ComponentProviderSettlement<'_>],
) -> Result<ComponentCandidate, Vec<psi_diagnostics::Diagnostic>> {
    let selected_target = checked.selected_native_target().ok_or_else(|| {
        vec![psi_diagnostics::Diagnostic::error(
            "terminal component staging requires one exact selected native target",
        )]
    })?;
    if selected_target != target {
        return Err(vec![psi_diagnostics::Diagnostic::error(format!(
            "terminal component staging target {target:?} does not match checked target {selected_target:?}"
        ))]);
    }
    let selected_program_entry = checked.selected_program_entry().ok_or_else(|| {
        vec![psi_diagnostics::Diagnostic::error(
            "terminal component staging requires one exact selected program entry",
        )]
    })?;
    let entry_machine = selected_program_entry.machine_name();
    let artifact = psi_checked_trees_to_terminal::produce_terminal_artifact(checked, entry_machine)
        .map_err(|error| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "terminal component artifact production failed: {error}"
            ))]
        })?;
    let native_artifact = realize_native_artifact(
        artifact,
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target,
            subsystem,
            profile,
            program_entry: omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
                selected_program_entry.source_signature(),
                selected_program_entry
                    .calling_plans()
                    .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry)),
            ),
            optimization_selections: checked.optimization_selections(),
            selected_provider_plans: checked.selected_provider_plans(),
            settlements,
        },
    )?;
    let stack_demand = omega_image_emission::derive_stack_demand(
        native_artifact.object(),
        native_artifact.object().entry(),
    )
    .map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "Terminal component stack-demand derivation failed: {error}"
        ))]
    })?;
    ComponentCandidate::checked(ComponentCandidateParts {
        native_artifact,
        entry_machine: entry_machine.to_owned(),
        selected_provider_plans: checked.selected_provider_plans().clone(),
        component_progress: checked
            .component_progress()
            .filter(|manifest| !manifest.pending().is_empty())
            .cloned(),
        stack_demand,
    })
    .map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "Terminal component policy replay failed: {error}"
        ))]
    })
}

fn write_finalized_terminal_component_output(
    options: &CompileOptions,
    runnable: InstalledRunnableComponent,
) -> Result<
    omega_component_deployment::PublishedComponentFlatOutput,
    Box<omega_component_deployment::ComponentFlatOutputPublicationError>,
> {
    let output_path = options
        .build_dir()
        .join(&runnable.installed_artifact().image().output().file_name);
    publish_component_flat_output(runnable, output_path)
}

fn unsupported_optimizer_source_canary() -> PathBuf {
    terminal_source_canary("unsupported_optimizer_component")
}

fn artifact_sections(verified: &VerifiedTerminalModule<'_>) -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(verified.module()).expect("verified terminal semantics encode"),
        encode_proof_bundle(verified.proof_bundle()).expect("verified proof bundle encodes"),
    )
}

fn interpret_verified_artifact(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let (semantic_bytes, proof_bytes) = artifact_sections(verified);
    interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        arguments,
    )
}

fn lower_verified_artifact(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
    let (semantic_bytes, proof_bytes) = artifact_sections(verified);
    lower_artifact_sections(&semantic_bytes, &proof_bytes, &AdmissionProfile::default())
}

fn start_verified_artifact(
    verified: &VerifiedTerminalModule<'_>,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecution, TerminalArtifactInterpretError> {
    let (semantic_bytes, proof_bytes) = artifact_sections(verified);
    TerminalExecution::start_artifact(
        &semantic_bytes,
        &proof_bytes,
        &AdmissionProfile::default(),
        arguments,
    )
}

fn expected_crash(module: &TerminalModule) -> TerminalExecutionStatus {
    let crash = module.machines[0]
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Crash {
                edge,
                cause,
                site_guard,
                frontier_lower_bound,
            } => Some(psi_terminal_interpreter::TerminalCrash {
                edge: *edge,
                cause: *cause,
                site_guard: site_guard.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
            }),
            _ => None,
        })
        .expect("test module should contain a crash terminator");
    TerminalExecutionStatus::Crashed(crash)
}

fn collect_integer_crash_leaves(
    control: &TargetIntegerControl,
    output: &mut Vec<(EdgeId, CrashCause)>,
) {
    match control {
        TargetIntegerControl::Crash {
            psi_crash_edge,
            cause,
            ..
        } => output.push((*psi_crash_edge, *cause)),
        TargetIntegerControl::Conditional {
            when_true,
            when_false,
            ..
        }
        | TargetIntegerControl::ConditionalExpression {
            when_true,
            when_false,
            ..
        } => {
            collect_integer_crash_leaves(&when_true.control, output);
            collect_integer_crash_leaves(&when_false.control, output);
        }
        TargetIntegerControl::Return { .. } => {}
    }
}

fn target_integer_crash_leaves(operation: &TargetOperation) -> Vec<(EdgeId, CrashCause)> {
    let mut output = Vec::new();
    match operation {
        TargetOperation::Crash {
            psi_edge, cause, ..
        } => output.push((*psi_edge, *cause)),
        TargetOperation::ReturnIntegerConditionalControl {
            when_true,
            when_false,
            ..
        }
        | TargetOperation::ReturnIntegerExpressionConditionalControl {
            when_true,
            when_false,
            ..
        } => {
            collect_integer_crash_leaves(&when_true.control, &mut output);
            collect_integer_crash_leaves(&when_false.control, &mut output);
        }
        _ => {}
    }
    output.sort_unstable();
    output
}

fn assert_guarded_crash_emits(verified: &VerifiedTerminalModule<'_>) {
    let abstract_operations = lower_verified_artifact(verified)
        .expect("guarded crash should cross the source-independent Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("guarded crash should select as recursive terminal control");
        let assigned =
            assign_registers(&target_operations).expect("guarded crash control should assign");
        let emitted = emit_machine_code(&assigned).expect("guarded crash control should emit");
        let fault = match target.architecture {
            omega_target::Architecture::X86_64 => &[0x0f, 0x0b][..],
            omega_target::Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4][..],
        };
        assert!(
            emitted.functions[0]
                .bytes
                .windows(fault.len())
                .any(|window| window == fault),
            "guarded crash machine code must retain its selected fault leaf"
        );
    }
}

#[path = "terminal_psi_source/admission_crashes_and_native.rs"]
mod admission_crashes_and_native;
#[path = "terminal_psi_source/comparisons_bitwise_and_casts.rs"]
mod comparisons_bitwise_and_casts;
#[path = "terminal_psi_source/contracts_and_frontend_drop.rs"]
mod contracts_and_frontend_drop;
#[path = "terminal_psi_source/control_graphs.rs"]
mod control_graphs;
#[path = "terminal_psi_source/divide_remainder_policies.rs"]
mod divide_remainder_policies;
#[path = "terminal_psi_source/exact_add_subtract_bounds.rs"]
mod exact_add_subtract_bounds;
#[path = "terminal_psi_source/exact_multiply_bounds.rs"]
mod exact_multiply_bounds;
#[path = "terminal_psi_source/exact_shift_bounds.rs"]
mod exact_shift_bounds;
#[path = "terminal_psi_source/locals_calls_and_short_circuit.rs"]
mod locals_calls_and_short_circuit;
#[path = "terminal_psi_source/runtime_policy_and_narrowing.rs"]
mod runtime_policy_and_narrowing;
fn install_terminal_object(
    terminal: &ObjectArtifact,
    code: Vec<u8>,
    entry_offset: u64,
) -> (InstalledCode, EntryStubId) {
    fn installation_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
    ) -> T {
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

    let entry = EntryStubId::from_normalized_identity(0x5300).expect("entry stub");
    let scope =
        ArtifactInstallationScopeId::from_normalized_identity(0x5301).expect("artifact scope");
    let constraints = PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
        .expect("terminal placement constraints");
    let artifact = Artifact::from_canonical_decode(
        installation_id(0x5310, ArtifactId::from_normalized_identity),
        installation_id(0x5311, ArtifactContentId::from_normalized_identity),
        terminal.target().architecture,
        code,
        installation_id(0x5312, MachineContractSetId::from_normalized_identity),
        installation_id(0x5313, MachineFootprintId::from_normalized_identity),
        installation_id(0x5314, PlacementPlanId::from_normalized_identity),
        constraints,
        installation_id(0x5315, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, entry_offset)],
        installation_id(0x5316, RelocationSetId::from_normalized_identity),
        Vec::new(),
    )
    .expect("terminal text should decode as an executable artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            installation_id(0x5320, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("terminal artifact admission");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        0x5330,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(0x5331),
        extent_id(0x5331, ExtentLineageId::from_normalized_identity),
        extent_id(0x5332, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(0x5333, ExtentProvenanceId::from_normalized_identity),
        extent_id(0x5334, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .expect("terminal placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        installation_id(0x5340, CodePlacementId::from_normalized_identity),
        installation_id(0x5301, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("terminal code placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("relocation-free terminal text should materialize exactly");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            installation_id(0x5341, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("terminal placement freeze");
    let validation = FinalValidationCertificate::from_validator(
        installation_id(0x5342, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated =
        validate_final_placement(frozen, &validation).expect("terminal final validation");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        installation_id(0x5343, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    (
        install_validated(validated, authority, receipt).expect("terminal code installation"),
        entry,
    )
}

#[test]
fn source_terminal_installation_publishes_only_with_retained_code_custody() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("terminal-Psi source canary should compile");
    assert!(
        checked.component_progress().is_none(),
        "targetless source canary publishes no build-bound progress manifest"
    );
    let lowered = lower_machine(&checked, "terminal_constant")
        .expect("accepted source slice should lower to terminal Psi");
    let verified = verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced terminal Psi should verify");
    let abstract_operations = lower_artifact_sections(
        &encode_module(verified.module()).expect("terminal semantics encode"),
        &encode_proof_bundle(verified.proof_bundle()).expect("terminal proof encodes"),
        &AdmissionProfile::default(),
    )
    .expect("verified source artifact should lower without frontend state");
    let target_operations = lower_to_target_operations(&abstract_operations, NativeTarget::host())
        .expect("source terminal operations should select for the host");
    let assigned = assign_registers(&target_operations).expect("source terminal homes assign");
    let machine_code = emit_machine_code(&assigned).expect("source terminal machine code emits");
    let object =
        build_object_artifact(&machine_code).expect("source terminal machine code forms an object");
    let image = emit_executable_image(&object, 3)
        .expect("source terminal object emits an executable image");
    let entry_offset = u64::try_from(object.entry_function().text_offset)
        .expect("terminal entry offset fits installation geometry");
    let (mut installed, _) =
        install_terminal_object(&object, object.text_bytes().to_vec(), entry_offset);
    let installed_identity = installed.identity();
    let artifact_identity = installed.artifact();
    let installation = build_installation_record(
        &image,
        ProfileDecisionId::new(0x53a0).expect("source publication profile decision"),
    )
    .expect("source terminal image has canonical installation metadata");
    let installation = decode_installation_record(
        &encode_installation_record(&installation).expect("source installation record encodes"),
    )
    .expect("source installation record decodes");
    let mut roots =
        InstalledRootLedger::claim(&mut installed).expect("source terminal installation registry");
    roots
        .seal_provider_occurrence_closure(checked.selected_provider_plans(), [])
        .expect("empty selected-provider closure");
    let artifact = bind_installed_artifact(object, image, installation, installed)
        .expect("source terminal join consumes exact installed-code custody");
    let runnable = bind_installed_runnable_component(artifact, roots, None)
        .expect("progress-free source terminal artifact is runnable");

    let tcb_acceptance = |identity: u64| {
        evaluate_executable_tcb_profile(
            &ExecutableTcbManifest {
                known_entries: Vec::new(),
                completeness: ScopeCompleteness::Complete {
                    scope: ExecutionScope::CallerAddressSpace,
                    selected_provider_closure_identity: identity,
                    opaque_closure_evidence: Vec::new(),
                    runtime_closure_evidence: Vec::new(),
                },
            },
            &ExecutableTcbProfile {
                name: format!("source-terminal-publication-{identity}"),
                scope: ExecutionScope::CallerAddressSpace,
                allow_static_current_artifact_checked_bodies: true,
                exact_allowances: Vec::new(),
                incomplete_scope: IncompleteScopePolicy::Reject,
            },
        )
        .expect("source terminal TCB acceptance")
    };
    let mut lifecycle = RunnableComponentEraLedger::new(
        ComponentEraEntryLedger::new(
            ComponentEraLedgerId::from_normalized_identity(0x53a1)
                .expect("source terminal lifecycle ledger"),
            "SourceTerminalBinding/v1".into(),
            "terminal_constant".into(),
            1,
            tcb_acceptance(0x53a2),
        )
        .expect("source terminal lifecycle"),
    );
    let candidate = ComponentEraCandidate {
        era_identity: 1,
        artifact_instance_identity: installed_identity.normalized_identity(),
        binding_contract_identity: "SourceTerminalBinding/v1".into(),
        entry_contract_identity: "terminal_constant".into(),
        entry_plan_identity: "source-terminal-entry-plan".into(),
        entry_plan_admission_receipt_identity: "source-terminal-entry-plan-receipt".into(),
        executable_tcb_acceptance: tcb_acceptance(0x53a3),
    };
    let publication = ComponentEraPublicationReceipt::from_runtime(
        0x53a4,
        lifecycle.lifecycle(),
        &candidate,
        true,
        false,
    );
    lifecycle
        .publish(candidate, publication, runnable)
        .expect("source terminal artifact publishes one runnable era");
    let retained = lifecycle
        .retained_component(1)
        .expect("published source terminal era retains installation custody");
    assert_eq!(retained.installed_code(), installed_identity);
    assert_eq!(retained.artifact(), artifact_identity);
    assert!(retained.progress().is_none());
}

#[test]
fn selected_progress_free_source_stages_non_visible_terminal_candidate() {
    let checked = compile_to_checked(&progress_free_selected_source_canary(), Some("linux_x64"))
        .expect("selected progress-free source entry should compile");
    let candidate = stage_terminal_component(
        &checked,
        NativeTarget::linux_x64(),
        3,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("selected progress-free source should stage one terminal candidate");
    assert_eq!(candidate.target(), NativeTarget::linux_x64());
    assert_eq!(candidate.entry_machine(), "Main::main");
    assert!(candidate.component_progress().is_none());
    assert!(candidate.selected_provider_plans().is_empty());
    assert!(candidate.provider_executions().is_empty());
    assert!(!candidate.semantic_bytes().is_empty());
    assert!(!candidate.proof_bytes().is_empty());
    assert!(!candidate.object().text_bytes().is_empty());
    assert!(!candidate.image().output().bytes.is_empty());

    let direct_report = omega_compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: progress_free_selected_source_canary(),
            build_dir: None,
            target_name: Some("linux_x64".into()),
            write_output: false,
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect("ordinary NativeArtifact compilation shares component realization");
    let direct = direct_report
        .retained_native_artifact()
        .expect("ordinary compilation retains one native artifact");
    assert_eq!(
        direct.psi_artifact().manifest(),
        candidate.artifact().manifest()
    );
    assert_eq!(direct.object(), candidate.object());
    assert_eq!(
        direct.object().relocations(),
        candidate.object().relocations()
    );
    assert_eq!(
        direct.object().text_bytes(),
        candidate.object().text_bytes()
    );
    assert_eq!(direct.image(), candidate.image());

    let target_mismatch = stage_terminal_component(
        &checked,
        NativeTarget::windows_x64(),
        3,
        &AdmissionProfile::default(),
        &[],
    )
    .expect_err("staging must reject a target different from checked selection");
    assert!(
        target_mismatch
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not match checked target")),
        "{target_mismatch:#?}"
    );

    let entry_offset = u64::try_from(candidate.object().entry_function().text_offset)
        .expect("progress-free terminal entry offset fits installation geometry");
    let mut wrong_text = candidate.object().text_bytes().to_vec();
    wrong_text[0] ^= 0x01;
    let (wrong_installed, _) =
        install_terminal_object(candidate.object(), wrong_text, entry_offset);
    let begin_error = begin_component_deployment(candidate, wrong_installed)
        .expect_err("deployment must reject installed bytes that differ from the candidate");
    assert!(begin_error.diagnostic().contains("exact unrelocated"));
    let (candidate, _) = begin_error.into_parts();

    let (installed, _) = install_terminal_object(
        candidate.object(),
        candidate.object().text_bytes().to_vec(),
        entry_offset,
    );
    let mut other_text = candidate.object().text_bytes().to_vec();
    other_text[0] ^= 0x01;
    let (mut other_installed, _) =
        install_terminal_object(candidate.object(), other_text, entry_offset);
    let other_roots = InstalledRootLedger::claim(&mut other_installed)
        .expect("different installation claims its registry");
    let claimed_error =
        begin_component_deployment_with_claimed_registry(candidate, installed, other_roots)
            .expect_err("claimed deployment rejects a different full installation context");
    assert!(
        claimed_error
            .diagnostic()
            .contains("different installed-code")
    );
    let (candidate, mut installed, other_roots) = claimed_error.into_parts();
    assert!(other_roots.binds_installed_code(&other_installed));
    assert!(!other_roots.binds_installed_code(&installed));
    drop(other_roots);

    let roots = InstalledRootLedger::claim(&mut installed)
        .expect("exact installation claims its registry before deployment");
    let stray_binding = ProviderOccurrencePlanBinding::new(
        0x55ff,
        ProviderOccurrenceInstallationReceipt::from_provider(
            ProviderOccurrenceInstallationReceiptId::from_normalized_identity(0x55f0)
                .expect("stray provider receipt"),
            &installed,
            InstalledProviderOccurrenceId::from_normalized_identity(0x55f1)
                .expect("stray provider occurrence"),
            "StrayProvider",
        ),
    );
    let session = begin_component_deployment_with_claimed_registry(candidate, installed, roots)
        .expect("exact progress-free deployment reuses the existing registry claim");
    let provider_error = session
        .seal_provider_occurrences(vec![stray_binding])
        .expect_err("extra provider occurrence must reject against an empty selected closure");
    assert!(provider_error.diagnostic().contains("exactly cover"));
    let (session, returned) = provider_error.into_parts();
    assert_eq!(returned.len(), 1);
    assert!(session.roots().provider_occurrence_closure().is_none());
    let provider_closed = session
        .seal_provider_occurrences(Vec::new())
        .expect("returned claimed session retries with the exact empty closure");
    let progress_closed = provider_closed
        .close_progress(Vec::new())
        .expect("progress-free candidate closes without attestations");
    let runnable = progress_closed
        .finalize(ProfileDecisionId::new(0x55f2).expect("progress-free deployment profile"))
        .expect("progress-free production deployment finalizes");
    assert!(runnable.progress().is_none());
    assert!(runnable.roots().live_external_roots_are_empty());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let scratch = ScratchDirectory(fresh_scratch_directory(
            "omega-native-component-flat-output",
        ));
        let file_name = runnable
            .installed_artifact()
            .image()
            .output()
            .file_name
            .clone();
        let expected_bytes = runnable.installed_artifact().image().output().bytes.clone();
        let installed_identity = runnable.installed_code();

        let wrong_name = scratch.0.join("not-the-sealed-name");
        let error = publish_component_flat_output(runnable, wrong_name.clone())
            .expect_err("flat publication must reject a substituted executable filename");
        assert!(error.diagnostic().contains("sealed executable filename"));
        let (runnable, returned_path) = error.into_parts();
        assert_eq!(returned_path, wrong_name);
        assert_eq!(runnable.installed_code(), installed_identity);

        let blocked_parent = scratch.0.join("not-a-directory");
        std::fs::write(&blocked_parent, b"occupied").expect("create blocked output parent");
        let blocked_path = blocked_parent.join(&file_name);
        let blocked_options = CompileOptions {
            root_path: progress_free_selected_source_canary(),
            build_dir: Some(blocked_parent),
            target_name: Some("linux_x64".into()),
            write_output: true,
        };
        let error = write_finalized_terminal_component_output(&blocked_options, runnable)
            .expect_err("filesystem rejection must preserve runnable deployment custody");
        assert!(error.diagnostic().contains("output directory"));
        let (runnable, returned_path) = error.into_parts();
        assert_eq!(returned_path, blocked_path);
        assert_eq!(runnable.installed_code(), installed_identity);

        let output_options = CompileOptions {
            root_path: progress_free_selected_source_canary(),
            build_dir: Some(scratch.0.join("published")),
            target_name: Some("linux_x64".into()),
            write_output: true,
        };
        let output_path = output_options.build_dir().join(&file_name);
        let published = write_finalized_terminal_component_output(&output_options, runnable)
            .expect("returned runnable custody should publish on an exact retry");
        assert_eq!(published.receipt().output_path(), output_path);
        assert_eq!(published.receipt().byte_count(), expected_bytes.len());
        assert_eq!(
            published.receipt().installation_fingerprint(),
            installation_fingerprint(published.runnable().installed_artifact().installation())
                .expect("terminal installation fingerprint")
        );
        assert_eq!(
            published.receipt().image_fingerprint(),
            published
                .runnable()
                .installed_artifact()
                .installation()
                .image()
        );
        assert_eq!(
            std::fs::read(&output_path).expect("read deployed flat output"),
            expected_bytes
        );
        assert_eq!(
            std::fs::metadata(&output_path)
                .expect("read deployed flat output mode")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        published
            .validate()
            .expect("flat output receipt should replay against retained runnable custody");

        std::fs::write(&output_path, b"drifted").expect("drift published output");
        let validation = published
            .validate()
            .expect_err("post-publication byte drift must invalidate the receipt");
        assert!(validation.diagnostic().contains("bytes differ"));
        let (runnable, _drifted_receipt) = published.into_parts();
        assert_eq!(runnable.installed_code(), installed_identity);
        let repaired = write_finalized_terminal_component_output(&output_options, runnable)
            .expect("retained runnable custody should repair a drifted flat output");
        repaired
            .validate()
            .expect("repaired flat output should replay exactly");
    }
}

#[test]
fn selected_optimizer_source_cannot_create_a_second_native_pipeline() {
    let checked = compile_to_checked(&selected_optimizer_source_canary(), Some("linux_x64"))
        .expect("selected optimizer source should reach checked compilation");
    let diagnostics = stage_terminal_component(
        &checked,
        NativeTarget::linux_x64(),
        3,
        &AdmissionProfile::default(),
        &[],
    )
    .expect_err("selected physical work is not a second production backend");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`SparseConditionalConstantPropagation`, `CopyPropagation`"),
        "{}",
        diagnostics[0].message
    );
    assert!(
        diagnostics[0]
            .message
            .contains("cannot yet enter native production"),
        "{}",
        diagnostics[0].message
    );
    assert!(
        diagnostics[0]
            .message
            .contains(
                "continuation does not cover baseline frame/exit, executable-image, and publication validation"
            ),
        "{}",
        diagnostics[0].message
    );
    assert!(!diagnostics[0].message.contains("UnsupportedSourceShape"));
    assert!(diagnostics[0].message.contains("no output was installed"));
}

#[test]
fn lower_only_optimizer_source_cannot_create_a_second_native_pipeline() {
    let checked = compile_to_checked(
        &selected_lowering_optimizer_source_canary(),
        Some("linux_x64"),
    )
    .expect("lower-only optimizer source should reach checked compilation");
    let diagnostics = stage_terminal_component(
        &checked,
        NativeTarget::linux_x64(),
        3,
        &AdmissionProfile::default(),
        &[],
    )
    .expect_err("selected physical work is not a second production backend");
    assert_eq!(diagnostics.len(), 1);
    let message = &diagnostics[0].message;
    assert!(message.contains("`SelectedIncomingU12ExactAddImmediate`"));
    assert!(
        message.contains("cannot yet enter native production"),
        "{message}"
    );
    assert!(
        message.contains(
            "continuation does not cover baseline frame/exit, executable-image, and publication validation"
        ),
        "{message}"
    );
    assert!(message.contains("no output was installed"), "{message}");
}

#[test]
fn control_flow_cleanup_source_reaches_the_publication_gate() {
    let checked = compile_to_checked(&unsupported_optimizer_source_canary(), Some("linux_x64"))
        .expect("explicit optimizer selection is retained through checking");
    let diagnostics = stage_terminal_component(
        &checked,
        NativeTarget::linux_x64(),
        3,
        &AdmissionProfile::default(),
        &[],
    )
    .expect_err("optimized publication remains unavailable");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("ControlFlowCleanup"));
    assert!(diagnostics[0].message.contains("no output was installed"));
}

#[test]
fn selected_source_entry_retains_build_bound_progress_for_terminal_publication() {
    let target = NativeTarget::linux_x64();
    let checked = compile_to_checked(&progress_source_canary(), Some("linux_x64"))
        .expect("selected progress-bearing source entry should compile");
    assert_eq!(checked.selected_program_entry_machine(), Some("Main::main"));
    let manifest = checked
        .component_progress()
        .expect("selected source entry should retain its build-bound progress manifest");
    assert_eq!(manifest.pending().len(), 1);
    assert_eq!(checked.selected_provider_plans().plans().len(), 1);
    let demand = &manifest.pending()[0];
    assert_eq!(demand.provider_service_identity, "Scheduler");
    assert_eq!(demand.profile_identity, "Scheduler::WeakFair");
    assert_eq!(demand.establishment_routes.len(), 1);

    let direct_native = omega_compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: progress_source_canary(),
            build_dir: None,
            target_name: Some("linux_x64".into()),
            write_output: false,
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
    .expect_err("a bare native artifact cannot discard build-bound progress");
    assert!(direct_native.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot discard pending build-bound component progress")
    }));

    let provider = admit_native_provider_for_selected_plan(
        target,
        &demand.requirement_identity,
        checked.selected_provider_plans(),
        "Scheduler",
        0x5400,
        CallSignature {
            parameters: vec![omega_calling_conventions::ValueShape::integer(4, 4)],
            result: None,
        },
    );
    assert_eq!(
        provider.provider_plan().normalized_identity(),
        demand.provider_plan_identity
    );

    let missing_provider =
        stage_terminal_component(&checked, target, 3, &AdmissionProfile::default(), &[])
            .expect_err("staging must reject an unresolved selected boundary");
    assert!(
        missing_provider
            .iter()
            .any(|diagnostic| diagnostic.message.contains("MissingBoundarySettlement")),
        "{missing_provider:#?}"
    );

    let unselected = admit_native_provider(
        target,
        &demand.requirement_identity,
        0x5410,
        CallSignature {
            parameters: vec![omega_calling_conventions::ValueShape::integer(4, 4)],
            result: None,
        },
    );
    let unselected_error = stage_terminal_component(
        &checked,
        target,
        3,
        &AdmissionProfile::default(),
        &[ComponentProviderSettlement {
            provider_execution: &unselected,
            realization: LinuxExitGroupI32Realization.into(),
        }],
    )
    .expect_err("staging must reject a provider execution outside the selected closure");
    assert!(
        unselected_error
            .iter()
            .any(|diagnostic| diagnostic.message.contains("names unselected plan")),
        "{unselected_error:#?}"
    );

    let candidate = stage_terminal_component(
        &checked,
        target,
        3,
        &AdmissionProfile::default(),
        &[ComponentProviderSettlement {
            provider_execution: &provider,
            realization: LinuxExitGroupI32Realization.into(),
        }],
    )
    .expect("production staging should retain the progress-bearing terminal candidate");
    assert_eq!(candidate.entry_machine(), "Main::main");
    assert!(!candidate.semantic_bytes().is_empty());
    assert!(!candidate.proof_bytes().is_empty());
    assert_eq!(
        candidate.selected_provider_plans().normalized_identity(),
        checked.selected_provider_plans().normalized_identity()
    );
    assert_eq!(candidate.provider_executions().len(), 1);
    assert_eq!(
        candidate
            .component_progress()
            .expect("staged progress manifest")
            .normalized_identity(),
        manifest.normalized_identity()
    );
    let entry_offset = u64::try_from(candidate.object().entry_function().text_offset)
        .expect("progress terminal entry offset fits installation geometry");
    let (installed, _) = install_terminal_object(
        candidate.object(),
        candidate.object().text_bytes().to_vec(),
        entry_offset,
    );
    let installed_identity = installed.identity();
    let artifact_identity = installed.artifact();

    let selected_plan = checked
        .selected_provider_plans()
        .plan_by_identity(demand.provider_plan_identity)
        .expect("progress demand retains its exact selected plan");
    let occurrence = InstalledProviderOccurrenceId::from_normalized_identity(0x5420)
        .expect("installed provider occurrence");
    let provider_bindings = vec![ProviderOccurrencePlanBinding::new(
        demand.provider_plan_identity,
        ProviderOccurrenceInstallationReceipt::from_provider(
            ProviderOccurrenceInstallationReceiptId::from_normalized_identity(0x5421)
                .expect("provider occurrence receipt"),
            &installed,
            occurrence,
            selected_plan.provider_type.clone(),
        ),
    )];
    let progress_bindings = vec![ComponentProgressAttestationBinding::new(
        ComponentProgressDemandIdentity::from_demand(demand),
        ProgressProfileEstablishmentAttestation::from_provider(
            ProgressProfileEstablishmentReceiptId::from_normalized_identity(0x5422)
                .expect("progress establishment receipt"),
            &installed,
            occurrence,
            occurrence,
            demand.provider_plan_identity,
            ProgressProfileGrantInvocationId::from_normalized_identity(0x5423)
                .expect("progress grant invocation"),
            demand.profile_identity.clone(),
            demand.subject_projections.clone(),
            demand.establishment_routes[0].clone(),
        ),
    )];
    let session = begin_component_deployment(candidate, installed)
        .expect("deployment claims the exact installed candidate");
    let provider_closed = session
        .seal_provider_occurrences(provider_bindings)
        .expect("source-selected provider occurrence closure seals");
    let wrong_progress = vec![ComponentProgressAttestationBinding::new(
        ComponentProgressDemandIdentity::from_demand(demand),
        ProgressProfileEstablishmentAttestation::from_provider(
            ProgressProfileEstablishmentReceiptId::from_normalized_identity(0x5425)
                .expect("wrong progress establishment receipt"),
            provider_closed.installed(),
            occurrence,
            occurrence,
            demand.provider_plan_identity,
            ProgressProfileGrantInvocationId::from_normalized_identity(0x5426)
                .expect("wrong progress grant invocation"),
            "Scheduler::WrongProfile",
            demand.subject_projections.clone(),
            demand.establishment_routes[0].clone(),
        ),
    )];
    let progress_error = provider_closed
        .close_progress(wrong_progress)
        .expect_err("wrong profile attestation cannot close component progress");
    assert!(progress_error.diagnostic().contains("profile"));
    let (provider_closed, returned_wrong_progress) = progress_error.into_parts();
    assert_eq!(returned_wrong_progress.len(), 1);
    let progress_closed = provider_closed
        .close_progress(progress_bindings)
        .expect("returned deployment retries with source-derived component progress");
    let progress_fingerprint = progress_closed
        .progress()
        .expect("progress-bearing deployment retains its opaque closure")
        .fingerprint();
    let runnable = progress_closed
        .finalize(ProfileDecisionId::new(0x5424).expect("progress publication profile decision"))
        .expect("production deployment binds the runnable component");

    #[cfg(unix)]
    let runnable = {
        let scratch =
            ScratchDirectory(fresh_scratch_directory("omega-native-progress-flat-output"));
        let output_options = CompileOptions {
            root_path: progress_source_canary(),
            build_dir: Some(scratch.0.clone()),
            target_name: Some("linux_x64".into()),
            write_output: true,
        };
        let published = write_finalized_terminal_component_output(&output_options, runnable)
            .expect("progress-bearing deployment should authorize exact flat output");
        assert!(
            published
                .runnable()
                .installed_artifact()
                .installation()
                .component_progress()
                .is_some(),
            "flat publication must retain the installation's accepted progress manifest"
        );
        assert_eq!(
            published.receipt().installation_fingerprint(),
            installation_fingerprint(published.runnable().installed_artifact().installation())
                .expect("progress installation fingerprint")
        );
        published
            .validate()
            .expect("progress-bearing flat output should replay exactly");
        let (runnable, receipt) = published.into_parts();
        drop(receipt);
        runnable
    };

    let tcb_acceptance = |identity: u64| {
        evaluate_executable_tcb_profile(
            &ExecutableTcbManifest {
                known_entries: Vec::new(),
                completeness: ScopeCompleteness::Complete {
                    scope: ExecutionScope::CallerAddressSpace,
                    selected_provider_closure_identity: identity,
                    opaque_closure_evidence: Vec::new(),
                    runtime_closure_evidence: Vec::new(),
                },
            },
            &ExecutableTcbProfile {
                name: format!("source-progress-publication-{identity}"),
                scope: ExecutionScope::CallerAddressSpace,
                allow_static_current_artifact_checked_bodies: true,
                exact_allowances: Vec::new(),
                incomplete_scope: IncompleteScopePolicy::Reject,
            },
        )
        .expect("progress source terminal TCB acceptance")
    };
    let mut lifecycle = RunnableComponentEraLedger::new(
        ComponentEraEntryLedger::new(
            ComponentEraLedgerId::from_normalized_identity(0x5425)
                .expect("progress source lifecycle ledger"),
            "SourceProgressBinding/v1".into(),
            "Main::main".into(),
            1,
            tcb_acceptance(0x5426),
        )
        .expect("progress source lifecycle"),
    );
    let candidate = ComponentEraCandidate {
        era_identity: 1,
        artifact_instance_identity: installed_identity.normalized_identity(),
        binding_contract_identity: "SourceProgressBinding/v1".into(),
        entry_contract_identity: "Main::main".into(),
        entry_plan_identity: "source-progress-entry-plan".into(),
        entry_plan_admission_receipt_identity: "source-progress-entry-plan-receipt".into(),
        executable_tcb_acceptance: tcb_acceptance(0x5427),
    };
    let publication = ComponentEraPublicationReceipt::from_runtime(
        0x5428,
        lifecycle.lifecycle(),
        &candidate,
        true,
        false,
    );
    lifecycle
        .publish(candidate, publication, runnable)
        .expect("progress source terminal artifact publishes one runnable era");
    let retained = lifecycle
        .retained_component(1)
        .expect("published progress era retains installation and progress custody");
    assert_eq!(retained.installed_code(), installed_identity);
    assert_eq!(retained.artifact(), artifact_identity);
    assert_eq!(
        retained
            .progress()
            .expect("published era retains progress")
            .fingerprint(),
        progress_fingerprint
    );

    #[cfg(unix)]
    {
        let candidate = stage_terminal_component(
            &checked,
            target,
            3,
            &AdmissionProfile::default(),
            &[ComponentProviderSettlement {
                provider_execution: &provider,
                realization: LinuxExitGroupI32Realization.into(),
            }],
        )
        .expect("compiler transaction should restage the progress-bearing candidate");
        let entry_offset = u64::try_from(candidate.object().entry_function().text_offset)
            .expect("progress transaction entry offset fits installation geometry");
        let (installed, _) = install_terminal_object(
            candidate.object(),
            candidate.object().text_bytes().to_vec(),
            entry_offset,
        );
        let installed_identity = installed.identity();
        let occurrence = InstalledProviderOccurrenceId::from_normalized_identity(0x5430)
            .expect("compiler transaction provider occurrence");
        let provider_bindings = vec![ProviderOccurrencePlanBinding::new(
            demand.provider_plan_identity,
            ProviderOccurrenceInstallationReceipt::from_provider(
                ProviderOccurrenceInstallationReceiptId::from_normalized_identity(0x5431)
                    .expect("compiler transaction provider receipt"),
                &installed,
                occurrence,
                selected_plan.provider_type.clone(),
            ),
        )];
        let progress_bindings = vec![ComponentProgressAttestationBinding::new(
            ComponentProgressDemandIdentity::from_demand(demand),
            ProgressProfileEstablishmentAttestation::from_provider(
                ProgressProfileEstablishmentReceiptId::from_normalized_identity(0x5432)
                    .expect("compiler transaction progress receipt"),
                &installed,
                occurrence,
                occurrence,
                demand.provider_plan_identity,
                ProgressProfileGrantInvocationId::from_normalized_identity(0x5433)
                    .expect("compiler transaction grant invocation"),
                demand.profile_identity.clone(),
                demand.subject_projections.clone(),
                demand.establishment_routes[0].clone(),
            ),
        )];
        let scratch = ScratchDirectory(fresh_scratch_directory(
            "omega-compiler-progress-deployment-transaction",
        ));
        let options = CompileOptions {
            root_path: progress_source_canary(),
            build_dir: Some(scratch.0.clone()),
            target_name: Some("linux_x64".into()),
            write_output: true,
        };
        let expected_path = options
            .build_dir()
            .join(&candidate.image().output().file_name);
        let session = begin_component_deployment(candidate, installed)
            .expect("accepted component and installation should begin deployment");
        let provider_closed = session
            .seal_provider_occurrences(provider_bindings)
            .expect("selected provider occurrences should close exactly");
        let progress_closed = provider_closed
            .close_progress(progress_bindings)
            .expect("component progress evidence should close exactly");
        let runnable = progress_closed
            .finalize(
                ProfileDecisionId::new(0x5434).expect("component deployment profile decision"),
            )
            .expect("closed deployment should become runnable");
        let published = write_finalized_terminal_component_output(&options, runnable)
            .expect("accepted progress deployment should publish");
        assert_eq!(published.receipt().output_path(), expected_path);
        assert_eq!(published.runnable().installed_code(), installed_identity);
        assert!(
            published
                .runnable()
                .installed_artifact()
                .installation()
                .component_progress()
                .is_some()
        );
        published
            .validate()
            .expect("compiler progress transaction should retain replayable custody");
    }
}

#[cfg(target_os = "macos")]
fn run_host_executable_image(bytes: &[u8]) -> i32 {
    use std::os::unix::fs::PermissionsExt;

    let directory = fresh_scratch_directory("omega-native-source-image");
    let _cleanup = ScratchDirectory(directory.clone());
    let executable_path = directory.join("omega-program");
    std::fs::write(&executable_path, bytes).expect("write direct source terminal image");
    let mut permissions = std::fs::metadata(&executable_path)
        .expect("source terminal image metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable_path, permissions)
        .expect("mark source terminal image executable");
    Command::new(&executable_path)
        .status()
        .expect("execute direct source terminal image")
        .code()
        .expect("direct source terminal image exited normally")
}

#[cfg(unix)]
fn run_host_machine_code(bytes: &[u8]) -> i32 {
    let directory = fresh_scratch_directory("omega-native");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _main\n.p2align 2\n_main:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl main\n.type main,@function\nmain:\n.byte {bytes}\n.size main, .-main\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    std::fs::write(&assembly_path, assembly).expect("write native linker harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal native canary")
        .code()
        .expect("terminal native canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_nine_u8(bytes: &[u8], first: u8, second: u8, ninth: u8) -> i32 {
    let directory = fresh_scratch_directory("omega-native-nine-parameter");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdint.h>\n\
extern uint8_t entry(uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t);\n\
int main(void) {{ return entry({first}, {second}, 3, 4, 5, 6, 7, 8, {ninth}); }}\n"
    );
    std::fs::write(&assembly_path, assembly).expect("write parameter assembly harness");
    std::fs::write(&driver_path, driver).expect("write parameter C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected parameter terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal parameter canary")
        .code()
        .expect("terminal parameter canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_two_u64(bytes: &[u8], left: u64, right: u64) -> i32 {
    let directory = fresh_scratch_directory("omega-native-integer-equality");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdint.h>\n\
extern uint8_t entry(uint64_t, uint64_t);\n\
int main(void) {{ return entry({left}ULL, {right}ULL); }}\n"
    );
    std::fs::write(&assembly_path, assembly).expect("write integer-equality assembly harness");
    std::fs::write(&driver_path, driver).expect("write integer-equality C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected integer-equality terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal integer-equality canary")
        .code()
        .expect("terminal integer-equality canary exited normally")
}

#[cfg(unix)]
fn host_machine_code_with_two_u64_matches(
    bytes: &[u8],
    left: u64,
    right: u64,
    expected: u64,
) -> bool {
    let directory = fresh_scratch_directory("omega-native-integer-result");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdint.h>\n\
extern uint64_t entry(uint64_t, uint64_t);\n\
int main(void) {{ return entry({left}ULL, {right}ULL) == {expected}ULL ? 0 : 1; }}\n"
    );
    std::fs::write(&assembly_path, assembly).expect("write integer-result assembly harness");
    std::fs::write(&driver_path, driver).expect("write integer-result C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected integer-result terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal integer-result canary")
        .success()
}

#[cfg(unix)]
fn run_host_machine_code_with_nine_bool(bytes: &[u8]) -> i32 {
    let directory = fresh_scratch_directory("omega-native-nine-boolean");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = "#include <stdbool.h>\n\
extern bool entry(bool, bool, bool, bool, bool, bool, bool, bool, bool);\n\
int main(void) { return entry(false, false, false, false, false, false, false, false, true); }\n";
    std::fs::write(&assembly_path, assembly).expect("write Boolean assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean canary")
        .code()
        .expect("terminal Boolean canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_bool(bytes: &[u8], value: bool) -> i32 {
    let directory = fresh_scratch_directory("omega-native-boolean-not");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdbool.h>\nextern bool entry(bool);\nint main(void) {{ return entry({}); }}\n",
        if value { "true" } else { "false" }
    );
    std::fs::write(&assembly_path, assembly).expect("write Boolean-not assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean-not C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean-not terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean-not canary")
        .code()
        .expect("terminal Boolean-not canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_two_bools(bytes: &[u8], left: bool, right: bool) -> i32 {
    let directory = fresh_scratch_directory("omega-native-boolean-equality");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdbool.h>\nextern bool entry(bool, bool);\nint main(void) {{ return entry({}, {}); }}\n",
        if left { "true" } else { "false" },
        if right { "true" } else { "false" },
    );
    std::fs::write(&assembly_path, assembly).expect("write Boolean-equality assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean-equality C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean-equality machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean-equality canary")
        .code()
        .expect("terminal Boolean-equality canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_three_bools(
    bytes: &[u8],
    first: bool,
    second: bool,
    third: bool,
) -> i32 {
    let directory = fresh_scratch_directory("omega-native-boolean-control-expression");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdbool.h>\nextern bool entry(bool, bool, bool);\nint main(void) {{ return entry({}, {}, {}); }}\n",
        if first { "true" } else { "false" },
        if second { "true" } else { "false" },
        if third { "true" } else { "false" },
    );
    std::fs::write(&assembly_path, assembly)
        .expect("write Boolean-control-expression assembly harness");
    std::fs::write(&driver_path, driver).expect("write Boolean-control-expression C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected Boolean-control-expression machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal Boolean-control-expression canary")
        .code()
        .expect("terminal Boolean-control-expression canary exited normally")
}

#[cfg(unix)]
fn run_host_machine_code_with_conditional_u8(
    bytes: &[u8],
    condition: bool,
    when_true: u8,
    when_false: u8,
) -> i32 {
    let directory = fresh_scratch_directory("omega-native-conditional");
    let _cleanup = ScratchDirectory(directory.clone());
    let assembly_path = directory.join("entry.s");
    let driver_path = directory.join("driver.c");
    let executable_path = directory.join("entry");
    let bytes = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    let assembly = if cfg!(target_os = "macos") {
        format!(".text\n.globl _entry\n.p2align 2\n_entry:\n.byte {bytes}\n")
    } else {
        format!(
            ".text\n.globl entry\n.type entry,@function\nentry:\n.byte {bytes}\n.size entry, .-entry\n.section .note.GNU-stack,\"\",@progbits\n"
        )
    };
    let driver = format!(
        "#include <stdbool.h>\n#include <stdint.h>\n\
extern uint8_t entry(bool, uint8_t, uint8_t);\n\
int main(void) {{ return entry({}, {when_true}, {when_false}); }}\n",
        if condition { "true" } else { "false" }
    );
    std::fs::write(&assembly_path, assembly).expect("write conditional assembly harness");
    std::fs::write(&driver_path, driver).expect("write conditional C harness");
    let link = Command::new("cc")
        .arg(&assembly_path)
        .arg(&driver_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .expect("invoke host C linker driver");
    assert!(
        link.status.success(),
        "host linker rejected conditional terminal machine code:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    Command::new(&executable_path)
        .status()
        .expect("execute terminal conditional canary")
        .code()
        .expect("terminal conditional canary exited normally")
}

#[cfg(unix)]
fn fresh_scratch_directory(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_nanos();
    let sequence = NEXT_SCRATCH_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "{prefix}-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).expect("create unique terminal test directory");
    directory
}

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

use abstract_operations::AbstractOperation;
use abstract_operations_to_abstract_operations::WrappingIntegerAddConstantsRule;
use abstract_operations_to_target_operations::*;
use machine_code::{NonAuthoritativeCalleeSaveStoragePolicy, TargetFrameLayoutPolicy};
use machine_emission::frame_layout::{
    stage_non_authoritative_callee_save_storage, stage_target_frame_layout,
};
use native_realization::*;
use optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use optimization_unit::PsiRewritePatch;
use post_allocation_machine_to_selected_form_encoding::*;
use proof_admission::AdmissionProfile;
use register_environment::*;
use register_homes_to_post_allocation_machine::*;
use selected_form_encoding_to_resolved_layout::*;
use selected_instructions_to_register_homes::*;
use semantic_vocabulary::IntegerValue;
use target::NativeTarget;

use super::generator::CorpusCase;
use super::psi::{CorpusArtifact, CorpusExpected};

pub(super) fn exercise_x86(case: &CorpusCase, artifact: &CorpusArtifact) {
    let first_psi = run_psi(case, artifact);
    let second_psi = run_psi(case, artifact);
    assert_eq!(
        first_psi, second_psi,
        "x86 Psi corpus case drifted: {case:?}",
    );
    let machine_artifact =
        super::psi::immediate_artifact(case.ordinal, expected_unsigned(artifact), 30_000);
    let first = run_machine(case.ordinal, &machine_artifact, NativeTarget::linux_x64());
    let second = run_machine(case.ordinal, &machine_artifact, NativeTarget::linux_x64());
    assert_eq!(first, second, "x86 corpus case drifted: {case:?}");

    let sign_extended_expected = i64::from(expected_unsigned(artifact) as u32 as i32) as u64;
    let sign_extended_artifact =
        super::psi::immediate_artifact(case.ordinal, sign_extended_expected, 35_000);
    let first = run_machine(
        case.ordinal,
        &sign_extended_artifact,
        NativeTarget::linux_x64(),
    );
    let second = run_machine(
        case.ordinal,
        &sign_extended_artifact,
        NativeTarget::linux_x64(),
    );
    assert_eq!(
        first, second,
        "x86 sign-extended corpus case drifted: {case:?}"
    );
}

pub(super) fn exercise_aarch64(case: &CorpusCase, artifact: &CorpusArtifact) {
    let first_psi = run_psi(case, artifact);
    let second_psi = run_psi(case, artifact);
    assert_eq!(
        first_psi, second_psi,
        "AArch64 Psi corpus case drifted: {case:?}",
    );
    let machine_artifact =
        super::psi::immediate_artifact(case.ordinal, expected_unsigned(artifact), 40_000);
    let first = run_machine(case.ordinal, &machine_artifact, NativeTarget::linux_arm64());
    let second = run_machine(case.ordinal, &machine_artifact, NativeTarget::linux_arm64());
    assert_eq!(first, second, "AArch64 corpus case drifted: {case:?}",);
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
pub(super) fn exercise_host_native(case: &CorpusCase, artifact: &CorpusArtifact) {
    let target = NativeTarget::host();
    let first = run_machine(case.ordinal, artifact, target);
    let second = run_machine(case.ordinal, artifact, target);
    assert_eq!(first, second, "host-native corpus case drifted: {case:?}");
    super::native::assert_u64_result(&first.layout, expected_unsigned(artifact));
}

pub(super) fn exercise_ieee_compare(
    case: &super::ieee_compare::CompareCase,
    artifact: &CorpusArtifact,
) {
    let first_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    let second_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    assert_eq!(
        first_x86, second_x86,
        "IEEE x86 corpus case drifted: {case:?}"
    );
    let first_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    let second_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    assert_eq!(
        first_aarch64, second_aarch64,
        "IEEE AArch64 corpus case drifted: {case:?}"
    );
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
pub(super) fn exercise_host_native_ieee_compare(
    case: &super::ieee_compare::CompareCase,
    artifact: &CorpusArtifact,
) {
    let target = NativeTarget::host();
    let first = run_machine(case.ordinal, artifact, target);
    let second = run_machine(case.ordinal, artifact, target);
    assert_eq!(
        first, second,
        "host-native IEEE corpus case drifted: {case:?}"
    );
    super::native::assert_bool_result(&first.layout, expected_boolean(artifact));
}

pub(super) fn exercise_exact_traps(case: &super::exact_traps::TrapCase, artifact: &CorpusArtifact) {
    let first_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    let second_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    assert_eq!(
        first_x86, second_x86,
        "exact-trap x86 corpus case drifted: {case:?}"
    );
    let first_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    let second_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    assert_eq!(
        first_aarch64, second_aarch64,
        "exact-trap AArch64 corpus case drifted: {case:?}"
    );
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
pub(super) fn exercise_host_native_exact_traps(
    case: &super::exact_traps::TrapCase,
    artifact: &CorpusArtifact,
) {
    let target = NativeTarget::host();
    let first = run_machine(case.ordinal, artifact, target);
    let second = run_machine(case.ordinal, artifact, target);
    assert_eq!(
        first, second,
        "host-native exact-trap corpus case drifted: {case:?}"
    );
    super::native::assert_u64_result(&first.layout, expected_unsigned(artifact));
}

pub(super) fn exercise_affine_cleanup(
    case: &super::affine_cleanup::CleanupCase,
    artifact: &CorpusArtifact,
) {
    let first_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    let second_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    assert_eq!(
        first_x86, second_x86,
        "affine-cleanup x86 corpus case drifted: {case:?}"
    );
    let first_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    let second_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    assert_eq!(
        first_aarch64, second_aarch64,
        "affine-cleanup AArch64 corpus case drifted: {case:?}"
    );
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
pub(super) fn exercise_host_native_affine_cleanup(
    case: &super::affine_cleanup::CleanupCase,
    artifact: &CorpusArtifact,
) {
    let target = NativeTarget::host();
    let first = run_machine(case.ordinal, artifact, target);
    let second = run_machine(case.ordinal, artifact, target);
    assert_eq!(
        first, second,
        "host-native affine-cleanup corpus case drifted: {case:?}"
    );
    super::native::assert_u64_result(&first.layout, expected_unsigned(artifact));
}

pub(super) fn exercise_atomic_establishment(
    case: &super::atomic_establishment::AtomicCase,
    artifact: &CorpusArtifact,
) {
    let first_x86 = run_atomic_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    let second_x86 = run_atomic_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    assert_eq!(
        first_x86, second_x86,
        "atomic-establishment x86 corpus case drifted: {case:?}"
    );
    let first_aarch64 = run_atomic_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    let second_aarch64 = run_atomic_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    assert_eq!(
        first_aarch64, second_aarch64,
        "atomic-establishment AArch64 corpus case drifted: {case:?}"
    );
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
pub(super) fn exercise_host_native_atomic_establishment(
    case: &super::atomic_establishment::AtomicCase,
    artifact: &CorpusArtifact,
) {
    let target = NativeTarget::host();
    let first = run_atomic_machine(case.ordinal, artifact, target);
    let second = run_atomic_machine(case.ordinal, artifact, target);
    assert_eq!(
        first, second,
        "host-native atomic-establishment corpus case drifted: {case:?}"
    );
    let (when_false, when_true) = expected_boolean_arms(artifact);
    super::native::assert_bool_result_arms_atomic(artifact, when_false, when_true);
}

pub(super) fn exercise_placed_memory(
    case: &super::placed_memory::PlacedMemoryCase,
    artifact: &CorpusArtifact,
) {
    let first_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    let second_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    assert_eq!(
        first_x86, second_x86,
        "placed-memory x86 corpus case drifted: {case:?}"
    );
    let first_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    let second_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    assert_eq!(
        first_aarch64, second_aarch64,
        "placed-memory AArch64 corpus case drifted: {case:?}"
    );
}

pub(super) fn exercise_transition(
    case: &super::transition::TransitionCase,
    artifact: &CorpusArtifact,
) {
    let first_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    let second_x86 = run_machine(case.ordinal, artifact, NativeTarget::linux_x64());
    assert_eq!(
        first_x86, second_x86,
        "transition x86 corpus case drifted: {case:?}"
    );
    let first_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    let second_aarch64 = run_machine(case.ordinal, artifact, NativeTarget::linux_arm64());
    assert_eq!(
        first_aarch64, second_aarch64,
        "transition AArch64 corpus case drifted: {case:?}"
    );
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
pub(super) fn exercise_host_native_placed_memory(
    case: &super::placed_memory::PlacedMemoryCase,
    artifact: &CorpusArtifact,
) {
    let target = NativeTarget::host();
    let first = run_machine(case.ordinal, artifact, target);
    let second = run_machine(case.ordinal, artifact, target);
    assert_eq!(
        first, second,
        "host-native placed-memory corpus case drifted: {case:?}"
    );
    super::native::assert_placed_memory_u64_result(artifact, expected_unsigned(artifact));
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
pub(super) fn exercise_host_native_transition(
    case: &super::transition::TransitionCase,
    artifact: &CorpusArtifact,
) {
    let target = NativeTarget::host();
    let first = run_machine(case.ordinal, artifact, target);
    let second = run_machine(case.ordinal, artifact, target);
    assert_eq!(
        first, second,
        "host-native transition corpus case drifted: {case:?}"
    );
    let (when_false, when_true) = expected_unsigned_arms(artifact);
    super::native::assert_u64_result_arms(&first.layout, when_false, when_true);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PsiEvidence {
    unit: optimization_core::OptimizationUnitIdentity,
    identity_bundle: optimization_core::OptimizationIdentityBundle,
    pass_manifests: Vec<optimization_core::OptimizationPassManifestRecord>,
    commits: Vec<abstract_operations_to_abstract_operations::PsiOptimizationCommit>,
    ledger: optimization_unit::PsiTransformationLedger,
    pre_manifest: optimization_unit::PrePhysicalOptimizationManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineEvidence {
    unit: optimization_core::OptimizationUnitIdentity,
    identity_bundle: optimization_core::OptimizationIdentityBundle,
    pass_manifests: Vec<optimization_core::OptimizationPassManifestRecord>,
    commits: Vec<abstract_operations_to_abstract_operations::PsiOptimizationCommit>,
    ledger: optimization_unit::PsiTransformationLedger,
    pre_manifest: optimization_unit::PrePhysicalOptimizationManifest,
    post_manifest: selected_instructions_to_register_homes::PostAllocationOptimizationManifest,
    home_custody: StagedOptimizedRegisterHomeCustodyReceipt,
    machine_custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
    encoding: StagedOptimizedSelectedFormEncoding,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    physical: register_model::ValidatedPhysicalRegisterModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AtomicMachineEvidence {
    unit: optimization_core::OptimizationUnitIdentity,
    identity_bundle: optimization_core::OptimizationIdentityBundle,
    pass_manifests: Vec<optimization_core::OptimizationPassManifestRecord>,
    commits: Vec<abstract_operations_to_abstract_operations::PsiOptimizationCommit>,
    ledger: optimization_unit::PsiTransformationLedger,
    pre_manifest: optimization_unit::PrePhysicalOptimizationManifest,
    post_manifest: selected_instructions_to_register_homes::PostAllocationOptimizationManifest,
    machine_custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
    frame: machine_code::TargetFrameLayoutPlan,
    encoding: StagedOptimizedSelectedFormEncoding,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    physical: register_model::ValidatedPhysicalRegisterModel,
}

/// The atomic lane's `EstablishScalarCase`/`EstablishScalarArray` results own
/// frame-local aggregate storage, so this runner composes the production
/// selected-instruction optimization, register allocation, callee-save
/// requirement/storage, and fixed-frame layout stages exactly as
/// `stage_optimized_verified_physical_pipeline` does, then encodes against the
/// resolved frame. The scalar-only lanes above keep the granular no-frame
/// stages because their machine plans declare no local storage.
fn run_atomic_machine(
    ordinal: usize,
    artifact: &CorpusArtifact,
    target: NativeTarget,
) -> AtomicMachineEvidence {
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        &artifact.semantic,
        &artifact.proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap_or_else(|error| panic!("case {ordinal} failed Psi optimization: {error}"));
    assert!(optimized.commits().is_empty());

    let unit = optimized.unit().identity;
    let identity_bundle = optimized.identity_bundle();
    let pass_manifests = optimized.pass_manifests().to_vec();
    let commits = optimized.commits().to_vec();
    let ledger = optimized.transformation_ledger().clone();
    let pre_manifest = optimized.pre_physical_manifest().record().clone();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    let register_environment = baseline_target_register_environment(target.target()).unwrap();
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            target,
            register_environment,
        )
        .unwrap();
    let selected = optimize_selected_instructions(selected).unwrap();
    let allocation = stage_register_allocation(selected).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&allocation.current()).unwrap();
    let machine_custody = machine.custody().clone();
    let current = allocation.current();
    let budget = current.budget_per_pass();
    let environment = current.register_environment();
    let requirements = stage_allocated_callee_saved_requirements(
        &allocation,
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        budget,
    )
    .unwrap();
    let storage = machine_emission::frame_layout::stage_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        machine_code::NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
    )
    .unwrap();
    let frame = machine_emission::frame_layout::stage_target_frame_layout(
        &machine,
        &requirements,
        &storage,
        environment,
        machine_code::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )
    .unwrap();
    let selected_stage = current.selected();
    let physical = environment.physical().clone();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected_stage,
        &machine,
        &physical,
        Some(frame.plan()),
    )
    .unwrap();
    let layout = stage_optimized_resolved_selected_form_layout(
        selected_stage,
        &machine,
        &physical,
        &encoding,
    )
    .unwrap();

    validate_optimized_layout_independent_selected_form_encoding(
        selected_stage,
        &machine,
        &physical,
        Some(frame.plan()),
        &encoding,
    )
    .unwrap();
    AtomicMachineEvidence {
        unit,
        identity_bundle,
        pass_manifests,
        commits,
        ledger,
        pre_manifest,
        post_manifest: current.post_allocation_manifest().record().clone(),
        machine_custody,
        frame: frame.plan().clone(),
        encoding,
        layout,
        physical,
    }
}

fn run_psi(case: &CorpusCase, artifact: &CorpusArtifact) -> PsiEvidence {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let budget = OptimizationWorkBudget::new(10_000, 10_000, 100_000, 10_000, 64).unwrap();
    let optimized = optimize_artifact_sections(
        &artifact.semantic,
        &artifact.proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap_or_else(|error| panic!("case {} failed Psi optimization: {error}", case.ordinal));
    assert_sccp(artifact, &optimized);
    PsiEvidence {
        unit: optimized.unit().identity,
        identity_bundle: optimized.identity_bundle(),
        pass_manifests: optimized.pass_manifests().to_vec(),
        commits: optimized.commits().to_vec(),
        ledger: optimized.transformation_ledger().clone(),
        pre_manifest: optimized.pre_physical_manifest().record().clone(),
    }
}

fn run_machine(ordinal: usize, artifact: &CorpusArtifact, target: NativeTarget) -> MachineEvidence {
    assert!(artifact.add_operations.is_empty());
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        &artifact.semantic,
        &artifact.proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap_or_else(|error| panic!("case {ordinal} failed Psi optimization: {error}"));
    assert!(optimized.commits().is_empty());

    let unit = optimized.unit().identity;
    let identity_bundle = optimized.identity_bundle();
    let pass_manifests = optimized.pass_manifests().to_vec();
    let commits = optimized.commits().to_vec();
    let ledger = optimized.transformation_ledger().clone();
    let pre_manifest = optimized.pre_physical_manifest().record().clone();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    // The pipeline crate's one-argument shorthand is #[cfg(test)] pub(crate) on
    // purpose, so production keeps the register environment as its own stage.
    // Build it the way stage_non_allocation_recovery_physical_pipeline does.
    let register_environment = baseline_target_register_environment(target.target()).unwrap();
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            target,
            register_environment,
        )
        .unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let post_manifest = homes.post_allocation_manifest().record().clone();
    let home_custody = homes.custody();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let machine_custody = machine.custody().clone();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical().clone();
    // Placed-memory lanes carry real local storage slots; the production
    // frame stage owns their layout, so the encoding needs a real frame plan
    // rather than the slot-free `None` shortcut used by scalar-only lanes.
    let frame = machine
        .machine()
        .plan()
        .functions
        .iter()
        .any(|function| {
            !function.local_storage_slots.is_empty() || !function.outgoing_arguments.is_empty()
        })
        .then(|| {
            let environment = selected_stage.register_environment().clone();
            let budget =
                OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000)
                    .unwrap();
            let requirements = stage_allocated_callee_saved_requirements(
                &homes,
                AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
                budget,
            )
            .unwrap();
            let storage = stage_non_authoritative_callee_save_storage(
                &requirements,
                &environment,
                NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
                budget,
            )
            .unwrap();
            stage_target_frame_layout(
                &machine,
                &requirements,
                &storage,
                &environment,
                TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            )
            .unwrap()
        });
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(),
        &machine,
        &physical,
        frame.as_ref().map(|frame| frame.plan()),
    )
    .unwrap();
    let layout = stage_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        &physical,
        &encoding,
    )
    .unwrap();

    validate_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(),
        &machine,
        &physical,
        frame.as_ref().map(|frame| frame.plan()),
        &encoding,
    )
    .unwrap();
    MachineEvidence {
        unit,
        identity_bundle,
        pass_manifests,
        commits,
        ledger,
        pre_manifest,
        post_manifest,
        home_custody,
        machine_custody,
        encoding,
        layout,
        physical,
    }
}

fn assert_sccp(
    artifact: &CorpusArtifact,
    optimized: &abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan,
) {
    let expected_rule = WrappingIntegerAddConstantsRule::contract().identity();
    let commits = optimized
        .commits()
        .iter()
        .filter(|commit| commit.rule == expected_rule)
        .collect::<Vec<_>>();
    assert_eq!(commits.len(), 2);
    let mut rewritten = Vec::new();
    for commit in commits {
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(rewrite) =
            commit.declaration().patch_ref()
        else {
            panic!("SCCP commit must replace the wrapping add with a constant")
        };
        assert_eq!(
            rewrite.constant,
            IntegerValue::Unsigned(expected_unsigned(artifact).into())
        );
        rewritten.push(rewrite.source_operation);
    }
    rewritten.sort_unstable();
    assert_eq!(rewritten, artifact.add_operations);
    for &add_operation in &artifact.add_operations {
        assert!(
            optimized.plan().functions[0]
                .operations
                .iter()
                .any(|operation| {
                    matches!(
                        operation,
                        AbstractOperation::IntegerConstant {
                            psi_operation,
                            value: IntegerValue::Unsigned(value),
                            ..
                        } if *psi_operation == add_operation
                            && *value == expected_unsigned(artifact).into()
                    )
                })
        );
    }
}

fn expected_unsigned(artifact: &CorpusArtifact) -> u64 {
    match artifact.expected {
        CorpusExpected::Unsigned(expected) => expected,
        CorpusExpected::Boolean(_)
        | CorpusExpected::BooleanPerArm { .. }
        | CorpusExpected::UnsignedPerArm { .. } => {
            panic!("expected an unsigned corpus artifact")
        }
    }
}

// Only the host-native exercisers read the Boolean and per-arm expectations,
// and those are gated to the hosts that can execute the produced text.
#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
fn expected_boolean(artifact: &CorpusArtifact) -> bool {
    match artifact.expected {
        CorpusExpected::Boolean(expected) => expected,
        CorpusExpected::Unsigned(_)
        | CorpusExpected::BooleanPerArm { .. }
        | CorpusExpected::UnsignedPerArm { .. } => {
            panic!("expected a Boolean corpus artifact")
        }
    }
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
fn expected_boolean_arms(artifact: &CorpusArtifact) -> (bool, bool) {
    match artifact.expected {
        CorpusExpected::BooleanPerArm {
            when_false,
            when_true,
        } => (when_false, when_true),
        CorpusExpected::Unsigned(_)
        | CorpusExpected::Boolean(_)
        | CorpusExpected::UnsignedPerArm { .. } => {
            panic!("expected a per-arm Boolean corpus artifact")
        }
    }
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
fn expected_unsigned_arms(artifact: &CorpusArtifact) -> (u64, u64) {
    match artifact.expected {
        CorpusExpected::UnsignedPerArm {
            when_false,
            when_true,
        } => (when_false, when_true),
        CorpusExpected::Unsigned(_)
        | CorpusExpected::Boolean(_)
        | CorpusExpected::BooleanPerArm { .. } => {
            panic!("expected a per-arm unsigned corpus artifact")
        }
    }
}

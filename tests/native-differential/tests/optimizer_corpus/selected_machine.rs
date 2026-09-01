use omega_abstract_operations::AbstractOperation;
use omega_isa_aarch64::validate_aarch64_shortest_movn_materialization;
use omega_isa_x86_64::{
    decode_x86_64_mov_r32_imm32_i64_materialization,
    decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization,
};
use omega_optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use omega_optimization_pipeline::*;
use omega_optimization_unit::PsiRewritePatch;
use omega_psi_optimizer::WrappingIntegerAddConstantsRule;
use omega_target::NativeTarget;
use psi_core::IntegerValue;
use psi_proof_admission::AdmissionProfile;

use super::generator::CorpusCase;
use super::psi::CorpusArtifact;

pub(super) fn exercise_x86(case: &CorpusCase, artifact: &CorpusArtifact) {
    let first_psi = run_psi(case, artifact);
    let second_psi = run_psi(case, artifact);
    assert_eq!(
        first_psi, second_psi,
        "x86 Psi corpus case drifted: {case:?}",
    );
    let machine_artifact = super::psi::immediate_artifact(case.ordinal, artifact.expected, 30_000);
    let first = run_machine(
        case,
        &machine_artifact,
        NativeTarget::linux_x64(),
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
    );
    let second = run_machine(
        case,
        &machine_artifact,
        NativeTarget::linux_x64(),
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
    );
    assert_eq!(first, second, "x86 corpus case drifted: {case:?}");
    assert_x86_oracle(&first, artifact.expected);

    let sign_extended_expected = i64::from(artifact.expected as u32 as i32) as u64;
    let sign_extended_artifact =
        super::psi::immediate_artifact(case.ordinal, sign_extended_expected, 35_000);
    let first = run_machine(
        case,
        &sign_extended_artifact,
        NativeTarget::linux_x64(),
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
    );
    let second = run_machine(
        case,
        &sign_extended_artifact,
        NativeTarget::linux_x64(),
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
    );
    assert_eq!(
        first, second,
        "x86 sign-extended corpus case drifted: {case:?}"
    );
    assert_x86_sign_extended_oracle(&first, sign_extended_expected);
}

pub(super) fn exercise_aarch64(case: &CorpusCase, artifact: &CorpusArtifact) {
    let first_psi = run_psi(case, artifact);
    let second_psi = run_psi(case, artifact);
    assert_eq!(
        first_psi, second_psi,
        "AArch64 Psi corpus case drifted: {case:?}",
    );
    let machine_artifact = super::psi::immediate_artifact(case.ordinal, artifact.expected, 40_000);
    let first = run_machine(
        case,
        &machine_artifact,
        NativeTarget::linux_arm64(),
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    );
    let second = run_machine(
        case,
        &machine_artifact,
        NativeTarget::linux_arm64(),
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    );
    assert_eq!(first, second, "AArch64 corpus case drifted: {case:?}",);
    assert_aarch64_oracle(&first, artifact.expected);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PsiEvidence {
    unit: omega_optimization_core::OptimizationUnitIdentity,
    identity_bundle: omega_optimization_core::OptimizationIdentityBundle,
    pass_manifests: Vec<omega_optimization_core::OptimizationPassManifestRecord>,
    commits: Vec<omega_psi_optimizer::PsiOptimizationCommit>,
    ledger: omega_optimization_unit::PsiTransformationLedger,
    pre_manifest: omega_optimization_validation::PrePhysicalOptimizationManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineEvidence {
    unit: omega_optimization_core::OptimizationUnitIdentity,
    identity_bundle: omega_optimization_core::OptimizationIdentityBundle,
    pass_manifests: Vec<omega_optimization_core::OptimizationPassManifestRecord>,
    commits: Vec<omega_psi_optimizer::PsiOptimizationCommit>,
    ledger: omega_optimization_unit::PsiTransformationLedger,
    pre_manifest: omega_optimization_validation::PrePhysicalOptimizationManifest,
    post_manifest: omega_regalloc::PostAllocationOptimizationManifest,
    home_custody: StagedOptimizedRegisterHomeCustodyReceipt,
    machine_custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
    optimization: StagedOptimizedPostAllocationMachineOptimization,
    encoding: StagedOptimizedSelectedFormEncoding,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    physical: omega_register_model::ValidatedPhysicalRegisterModel,
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

fn run_machine(
    case: &CorpusCase,
    artifact: &CorpusArtifact,
    target: NativeTarget,
    machine_rule: Optimization,
) -> MachineEvidence {
    assert!(artifact.add_operations.is_empty());
    let selections = OptimizationSelections::new([machine_rule]).unwrap();
    let budget = OptimizationWorkBudget::new(10_000, 10_000, 100_000, 10_000, 64).unwrap();
    let optimized = optimize_artifact_sections(
        &artifact.semantic,
        &artifact.proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap_or_else(|error| panic!("case {} failed Psi optimization: {error}", case.ordinal));
    assert!(optimized.commits().is_empty());

    let unit = optimized.unit().identity;
    let identity_bundle = optimized.identity_bundle();
    let pass_manifests = optimized.pass_manifests().to_vec();
    let commits = optimized.commits().to_vec();
    let ledger = optimized.transformation_ledger().clone();
    let pre_manifest = optimized.pre_physical_manifest().record().clone();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let post_manifest = homes.post_allocation_manifest().record().clone();
    let home_custody = homes.custody();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let machine_custody = machine.custody().clone();
    let optimization =
        stage_optimized_post_allocation_machine_optimization(&homes, &machine).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical().clone();
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            &physical,
            Some(&optimization),
        )
        .unwrap();
    let layout =
        stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            &physical,
            &encoding,
            Some(&optimization),
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
        optimization,
        encoding,
        layout,
        physical,
    }
}

fn assert_sccp(
    artifact: &CorpusArtifact,
    optimized: &omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan,
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
            IntegerValue::Unsigned(artifact.expected.into())
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
                        } if *psi_operation == add_operation && *value == artifact.expected.into()
                    )
                })
        );
    }
}

fn assert_x86_oracle(run: &MachineEvidence, expected: u64) {
    let StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(materialization) =
        &run.optimization
    else {
        panic!("x86 lane did not select MOV-r32-imm32")
    };
    assert_eq!(materialization.materialization().plan().actions.len(), 2);
    for action in &materialization.materialization().plan().actions {
        let row = run
            .encoding
            .rows()
            .iter()
            .find(|row| row.instruction == action.instruction)
            .unwrap();
        let SelectedFormEncodingState::Encoded { bytes, .. } = &row.state else {
            panic!("x86 materialization must own encoded bytes")
        };
        let decoded =
            decode_x86_64_mov_r32_imm32_i64_materialization(&run.physical, bytes).unwrap();
        assert_eq!(decoded.value_bits(), expected);
        assert_eq!(action.literal_bits, expected);
    }
}

fn assert_x86_sign_extended_oracle(run: &MachineEvidence, expected: u64) {
    let StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended(
        materialization,
    ) = &run.optimization
    else {
        panic!("x86 sign-extended lane did not select MOV-r64-imm32")
    };
    assert_eq!(materialization.materialization().plan().actions.len(), 2);
    for action in &materialization.materialization().plan().actions {
        let row = run
            .encoding
            .rows()
            .iter()
            .find(|row| row.instruction == action.instruction)
            .unwrap();
        let SelectedFormEncodingState::Encoded { bytes, .. } = &row.state else {
            panic!("x86 sign-extended materialization must own encoded bytes")
        };
        let decoded =
            decode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(&run.physical, bytes)
                .unwrap();
        assert_eq!(decoded.value_bits(), expected);
        assert_eq!(action.literal_bits, expected);
    }
}

fn assert_aarch64_oracle(run: &MachineEvidence, expected: u64) {
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) =
        &run.optimization
    else {
        panic!("AArch64 lane did not select MOVN")
    };
    assert_eq!(materialization.materialization().plan().actions.len(), 2);
    for action in &materialization.materialization().plan().actions {
        let row = run
            .encoding
            .rows()
            .iter()
            .find(|row| row.instruction == action.instruction)
            .unwrap();
        let SelectedFormEncodingState::Encoded { bytes, .. } = &row.state else {
            panic!("AArch64 materialization must own encoded bytes")
        };
        validate_aarch64_shortest_movn_materialization(
            &run.physical,
            action.destination.view,
            IntegerValue::Unsigned(expected.into()),
            bytes,
        )
        .unwrap();
        assert_eq!(action.literal_bits, expected);
    }
}

//! Independent assignment rejection for target roots, function shape, and provenance corruption.

use crate::tests::*;

#[test]
fn independent_assignment_custody_rejects_each_root_and_provenance_corruption() {
    let (semantic, proof) = artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(
            OptimizationSelections::new([
                Optimization::SparseConditionalConstantPropagation,
                Optimization::CopyPropagation,
            ])
            .unwrap(),
        ),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    let staged = stage_optimized_assignment(target).unwrap();

    let wrong_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            &wrong_environment,
            staged.assigned(),
        ),
        Err(OptimizedAssignmentCustodyError::RegisterEnvironmentTargetMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.psi.program_fingerprint = psi_terminal::SemanticFingerprint::from_bytes([0x44; 32]);
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::TerminalPsiMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.target = NativeTarget::windows_x64();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::NativeTargetMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.entry = MachineId::new(9_001).unwrap();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::EntryMismatch)
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions.push(corrupted.functions[0].clone());
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionCountMismatch {
            expected: 1,
            actual: 2,
        })
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions[0].machine = MachineId::new(9_002).unwrap();
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionMachineMismatch { position: 0 })
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions[0].attachment = Some(psi_core::StructuralTypeId::new(9_003).unwrap());
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionAttachmentMismatch { position: 0 })
    );

    let mut corrupted = staged.assigned().clone();
    corrupted.functions[0]
        .provenance
        .operations
        .push(OperationId::new(9_004).unwrap());
    assert_eq!(
        validate_optimized_assignment_custody(
            staged.optimized_target(),
            staged.register_environment(),
            &corrupted,
        ),
        Err(OptimizedAssignmentCustodyError::FunctionProvenanceMismatch { position: 0 })
    );
}

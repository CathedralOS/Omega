use super::super::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationSelections, SelectionDecodeError,
};
use std::collections::BTreeSet;

#[test]
fn authoritative_vocabulary_has_contiguous_tags_and_unique_nonempty_names() {
    // The descriptor macro generates both the enum and `ALL`; this test
    // protects the stable tag sequence and the two generated build views.
    let mut case_names = BTreeSet::new();
    let mut counter_fields = BTreeSet::new();
    for (index, optimization) in Optimization::ALL.into_iter().enumerate() {
        assert_eq!(usize::from(optimization as u8), index + 1);
        assert!(!optimization.build_case_name().is_empty());
        assert!(!optimization.build_counter_field().is_empty());
        assert!(case_names.insert(optimization.build_case_name()));
        assert!(counter_fields.insert(optimization.build_counter_field()));
        assert_eq!(
            Optimization::from_build_case_name(optimization.build_case_name()),
            Some(optimization)
        );
    }
    assert_eq!(case_names.len(), Optimization::ALL.len());
    assert_eq!(counter_fields.len(), Optimization::ALL.len());
    assert_eq!(
        Optimization::from_build_case_name("CopyPropagationV2"),
        None
    );
}

#[test]
fn generic_catalog_descriptor_retains_exact_name_and_typed_payload() {
    let descriptor = OptimizationCatalogDescriptor::new(
        Optimization::X86SelectXorZeroI64MaterializationV1,
        ("x86-64", 7_u16),
    );
    assert_eq!(
        descriptor.optimization(),
        Optimization::X86SelectXorZeroI64MaterializationV1
    );
    assert_eq!(descriptor.payload(), &("x86-64", 7));
}

#[test]
fn selections_are_sorted_and_round_trip_canonically() {
    let selections = OptimizationSelections::new([
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
        Optimization::ProofCheckElision,
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        Optimization::X86SelectXorZeroI64MaterializationV1,
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
    ])
    .expect("unique selections");
    assert_eq!(
        selections.as_slice(),
        &[
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
            Optimization::ProofCheckElision,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
            Optimization::X86SelectXorZeroI64MaterializationV1,
            Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
            Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        ]
    );
    let encoded = selections.encode();
    assert_eq!(
        OptimizationSelections::decode(&encoded).expect("canonical decode"),
        selections
    );
    assert_eq!(
        OptimizationSelections::decode(&encoded)
            .expect("repeat decode")
            .encode(),
        encoded
    );
}

#[test]
fn duplicates_reject_before_identity() {
    let error = OptimizationSelections::new([
        Optimization::GlobalValueNumbering,
        Optimization::GlobalValueNumbering,
    ])
    .expect_err("duplicate selection must reject");
    assert_eq!(error.0, Optimization::GlobalValueNumbering);
}

#[test]
fn decoder_rejects_noncanonical_and_trailing_encodings() {
    let selections = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
    ])
    .expect("unique selections");
    let mut reversed = selections.encode();
    reversed[16..].reverse();
    assert_eq!(
        OptimizationSelections::decode(&reversed),
        Err(SelectionDecodeError::NonCanonicalOrder)
    );

    let mut trailing = selections.encode();
    trailing.push(0);
    assert_eq!(
        OptimizationSelections::decode(&trailing),
        Err(SelectionDecodeError::TrailingBytes)
    );

    let mut old_version = selections.encode();
    old_version[8..12].copy_from_slice(&10_u32.to_le_bytes());
    assert_eq!(
        OptimizationSelections::decode(&old_version),
        Err(SelectionDecodeError::UnsupportedVersion(10))
    );
}

#[test]
fn phase_projection_is_canonical_without_replacing_the_full_identity() {
    let selections = OptimizationSelections::new([
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        Optimization::X86SelectXorZeroI64MaterializationV1,
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        Optimization::CopyPropagation,
        Optimization::SparseConditionalConstantPropagation,
    ])
    .unwrap();
    let full_identity = selections.identity();
    assert_eq!(
        selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .as_slice(),
        &[
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ]
    );
    assert_eq!(
        selections
            .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
            .as_slice(),
        &[
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
            Optimization::X86SelectXorZeroI64MaterializationV1,
            Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
            Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        ]
    );
    assert_eq!(
        selections
            .for_phase(OptimizationExecutionPhase::Psi)
            .as_slice(),
        &[
            Optimization::SparseConditionalConstantPropagation,
            Optimization::CopyPropagation,
        ]
    );
    assert_eq!(
        selections
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .as_slice(),
        &[
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ]
    );
    assert_eq!(
        selections
            .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
            .as_slice(),
        &[Optimization::X86RelaxConditionalBranchesToRel8V1]
    );
    assert_eq!(full_identity, selections.identity());
    assert_ne!(
        full_identity,
        selections
            .for_phase(OptimizationExecutionPhase::Psi)
            .identity()
    );
}

#[test]
fn phase_projection_retains_complete_policy_and_represents_identity_phases() {
    let selections = OptimizationSelections::new([
        Optimization::CopyPropagation,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
    ])
    .unwrap();
    let complete = selections.identity();

    let psi = selections.project_phase(OptimizationExecutionPhase::Psi);
    assert_eq!(psi.phase(), OptimizationExecutionPhase::Psi);
    assert_eq!(psi.complete_selection(), complete);
    assert_eq!(
        psi.selections().as_slice(),
        &[Optimization::CopyPropagation]
    );

    let checked = selections.project_phase(OptimizationExecutionPhase::CheckedTrees);
    assert_eq!(checked.phase(), OptimizationExecutionPhase::CheckedTrees);
    assert_eq!(checked.complete_selection(), complete);
    assert!(checked.is_empty());

    for phase in OptimizationExecutionPhase::ALL {
        let projection = selections.project_phase(phase);
        assert_eq!(projection.phase(), phase);
        assert_eq!(projection.complete_selection(), complete);
        assert!(
            projection
                .selections()
                .as_slice()
                .iter()
                .all(|optimization| optimization.execution_phase() == phase)
        );
    }
}

#[test]
fn psi_projection_is_exhaustive_target_neutral_and_bound_to_complete_policy() {
    let selections = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
        Optimization::X86SelectXorZeroI64MaterializationV1,
    ])
    .unwrap();
    let projection = selections.project_psi();

    assert_eq!(projection.complete_selection(), selections.identity());
    assert_eq!(
        projection.selections().as_slice(),
        &[
            psi_optimization::PsiOptimization::ControlFlowCleanup,
            psi_optimization::PsiOptimization::CopyPropagation,
        ]
    );
    assert!(
        !projection
            .selections()
            .contains(psi_optimization::PsiOptimization::ProofCheckElision)
    );

    for optimization in Optimization::ALL {
        assert_eq!(
            optimization.psi_optimization().is_some(),
            optimization.execution_phase() == OptimizationExecutionPhase::Psi
        );
    }
}

#[test]
fn post_terminal_projection_excludes_earlier_phases_and_retains_complete_identity() {
    let selections = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let projection = selections.project_post_terminal();

    assert_eq!(projection.complete_selection(), selections.identity());
    assert_eq!(
        projection.selections().as_slice(),
        &[Optimization::SelectedIncomingU12ExactAddImmediate]
    );

    let psi_only = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let projection = psi_only.project_post_terminal();
    assert_eq!(projection.complete_selection(), psi_only.identity());
    assert!(projection.selections().is_empty());
}

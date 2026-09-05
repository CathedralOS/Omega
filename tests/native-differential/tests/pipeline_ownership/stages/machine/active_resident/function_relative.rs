//! Function-relative exit realization and custody rejection.

use std::collections::BTreeSet;

use crate::tests::*;

#[test]
fn active_resident_rematerialization_reaches_function_relative_exit_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_active_resident_allocation_recovery_realization(target);
        let source = &staged;
        let current = staged.allocation().current();
        let rematerialization = staged
            .allocation()
            .rematerialization_proof_for_test()
            .unwrap();
        let AllocationEvidence::ActiveResidentRematerialization(recovery_receipt) =
            current.evidence()
        else {
            panic!("fixture must retain rematerialization evidence")
        };
        let physical = current.register_environment().physical();
        let admitted_names = match target.architecture {
            target::Architecture::X86_64 => ["rax", "rcx"],
            target::Architecture::Aarch64 => ["x0", "x1"],
        };
        let admitted_views = admitted_names
            .into_iter()
            .map(|name| physical.model().view_named(name).unwrap().id)
            .collect::<BTreeSet<_>>();
        let AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } = &staged
            .allocation()
            .rematerialization_availability_for_test()
            .unwrap()
            .plan()
            .policy
        else {
            panic!("pressure fixture must retain an explicit caller-saved allowlist")
        };
        assert_eq!(
            views.iter().copied().collect::<BTreeSet<_>>(),
            admitted_views
        );
        let action = rematerialization.plan().functions[0]
            .action
            .as_ref()
            .expect("the explicit active-resident staging route must rematerialize");
        let fresh = action.fresh_materialize;
        let transformed_selected = rematerialization.receipt().transformed_selected();
        let fresh_layout_row = source
            .layout()
            .functions()
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|instruction| instruction.instruction == fresh)
            .expect("fresh rematerialization must survive function-relative layout");
        assert_eq!(
            fresh_layout_row.alternative.family,
            selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(!fresh_layout_row.bytes.is_empty());

        let manifest = staged.manifest().record();
        let empty = OptimizationSelections::default().identity();
        assert_eq!(
            manifest.selections,
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap()
            .identity()
        );
        assert_eq!(manifest.selected_lowering_selections, empty);
        assert_eq!(manifest.selected_lowering_completion, None);
        assert_eq!(manifest.allocation_recovery_selections, manifest.selections);
        assert_eq!(manifest.post_allocation_machine_selections, empty);
        assert_eq!(manifest.function_relative_layout_selections, empty);
        assert_eq!(
            manifest.pre_physical_manifest,
            (*recovery_receipt).source().manifest()
        );
        assert_eq!(
            manifest.post_allocation_manifest,
            current.post_allocation_manifest().record().identity
        );
        assert_eq!(manifest.selected, transformed_selected);
        assert_eq!(manifest.baseline_pre_layout, manifest.pre_layout);
        assert_eq!(manifest.baseline_resolved_layout, manifest.resolved_layout);
        assert_eq!(manifest.x86_branch_relaxation, None);
        assert_eq!(manifest.post_allocation_machine_optimization, None);
        assert_eq!(manifest.target, target);
        assert_eq!(
            current
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization.receipt().identity(),
                )
            ]
        );
        assert_eq!(
            staged.exit_contract().contract().selected,
            transformed_selected
        );
        assert_eq!(
            staged.exit_contract().contract().resolved_layout,
            source.layout().identity()
        );
        assert!(matches!(
            staged.exit_contract().contract().layout_custody,
            WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        ));
        assert!(
            staged
                .exit_contract()
                .contract()
                .functions
                .iter()
                .all(|function| function.modified_callee_saved_units.is_empty())
        );
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&manifest.encode()),
            Ok(manifest.clone())
        );
        assert_eq!(
            validate_allocation_recovery_function_relative_realization(&staged,).unwrap(),
            staged.custody().clone()
        );
        assert_eq!(
            staged.custody().source(),
            &AllocationEvidence::ActiveResidentRematerialization(*recovery_receipt)
        );
        assert_eq!(
            staged.custody().exit_contract(),
            staged.exit_contract().identity()
        );
        assert_eq!(staged.custody().realization(), manifest.identity);
    }
}

#[test]
fn active_resident_function_relative_realization_rejects_corrupt_or_detached_custody() {
    let target = NativeTarget::linux_x64();

    let mut source_corruption = staged_active_resident_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_layout_for_test(&mut source_corruption);
    assert!(matches!(
        validate_allocation_recovery_function_relative_realization(&source_corruption,),
        Err(AllocationRecoveryFunctionRelativeRealizationError::Layout(
            OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch
        ),)
    ));

    let mut exit_corruption = staged_active_resident_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_exit_for_test(&mut exit_corruption);
    assert_eq!(
        validate_allocation_recovery_function_relative_realization(&exit_corruption,),
        Err(
            AllocationRecoveryFunctionRelativeRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            ),
        )
    );

    let mut manifest_corruption = staged_active_resident_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_manifest_for_test(&mut manifest_corruption);
    assert_eq!(
        validate_allocation_recovery_function_relative_realization(&manifest_corruption,),
        Err(AllocationRecoveryFunctionRelativeRealizationError::RootMismatch,)
    );

    let mut receipt_corruption = staged_active_resident_allocation_recovery_realization(target);
    corrupt_allocation_recovery_realization_custody_for_test(&mut receipt_corruption);
    assert_eq!(
        validate_allocation_recovery_function_relative_realization(&receipt_corruption,),
        Err(AllocationRecoveryFunctionRelativeRealizationError::ReceiptMismatch,)
    );

    let mut detached = staged_active_resident_allocation_recovery_realization(target);
    let foreign =
        staged_active_resident_allocation_recovery_realization(NativeTarget::linux_arm64());
    replace_allocation_recovery_realization_exit_for_test(&mut detached, &foreign);
    assert_eq!(
        validate_allocation_recovery_function_relative_realization(&detached,),
        Err(
            AllocationRecoveryFunctionRelativeRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            ),
        )
    );
}

#[test]
fn active_resident_function_relative_realization_rejects_unexecuted_later_phase_selections() {
    for later in [
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
    ] {
        let selections = OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            later,
        ])
        .unwrap();
        let source = stage_optimized_active_resident_rematerialization(
            staged_active_resident_two_view_legality_with_selections(NativeTarget::linux_x64(), selections),
            SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            selected_lowering_budget(),
        ).unwrap();
        let machine = stage_optimized_post_allocation_machine_plan(&source).unwrap();
        assert!(matches!(
            stage_allocation_recovery_function_relative_realization(source, machine,),
            Err(AllocationRecoveryFunctionRelativeRealizationError::UnsupportedSelections)
        ));
    }
}

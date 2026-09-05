//! Reauthenticated function-relative V9 manifest and nested-custody mutations.

use crate::tests::*;
use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_physical_instructions::PostAllocationMachineIdentity;
use omega_selected_instructions::PreAllocationMachineEffectIdentity;
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_target::{Architecture, ObjectFormat};

use super::fixture::{direct_rel8_realization, post_allocation_realization};

type ManifestMutation = fn(&mut FunctionRelativeOptimizationRealizationManifest);

#[test]
fn every_representable_direct_manifest_field_rejects_after_reauthentication() {
    let mut staged = direct_rel8_realization();
    let baseline = staged.manifest().record().clone();
    // Stage and scope are singleton in memory; closed tags are covered by the
    // wire matrix. Post-allocation custody has its own positive-shape matrix.
    let mutations: [(&str, ManifestMutation); 32] = [
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0xb1; 32])
        }),
        ("selected_lowering_selections", |record| {
            record.selected_lowering_selections =
                OptimizationSelectionIdentity::from_bytes([0xb2; 32])
        }),
        ("selected_lowering_completion", |record| {
            record.selected_lowering_completion = Some(
                SelectedLoweringOptimizationCompletionIdentity::from_bytes([0xb3; 32]),
            )
        }),
        ("allocation_recovery_selections", |record| {
            record.allocation_recovery_selections =
                OptimizationSelectionIdentity::from_bytes([0xb4; 32])
        }),
        ("post_allocation_machine_selections", |record| {
            record.post_allocation_machine_selections =
                OptimizationSelectionIdentity::from_bytes([0xb5; 32])
        }),
        ("function_relative_layout_selections", |record| {
            record.function_relative_layout_selections =
                OptimizationSelectionIdentity::from_bytes([0xb6; 32])
        }),
        ("pre_physical_manifest", |record| {
            record.pre_physical_manifest =
                PrePhysicalOptimizationManifestIdentity::from_bytes([0xb7; 32])
        }),
        ("post_allocation_manifest", |record| {
            record.post_allocation_manifest =
                PostAllocationOptimizationManifestIdentity::from_bytes([0xb8; 32])
        }),
        ("selected", |record| {
            record.selected = SelectedInstructionPlanIdentity::from_bytes([0xb9; 32])
        }),
        ("pre_allocation_machine_effects", |record| {
            record.pre_allocation_machine_effects =
                PreAllocationMachineEffectIdentity::from_bytes([0xba; 32])
        }),
        ("post_allocation_machine", |record| {
            record.post_allocation_machine = PostAllocationMachineIdentity::from_bytes([0xbb; 32])
        }),
        ("baseline_pre_layout", |record| {
            record.baseline_pre_layout = SelectedFormEncodingIdentity::from_bytes([0xbc; 32])
        }),
        ("pre_layout", |record| {
            record.pre_layout = SelectedFormEncodingIdentity::from_bytes([0xbd; 32])
        }),
        ("baseline_resolved_layout", |record| {
            record.baseline_resolved_layout =
                ResolvedSelectedFormLayoutIdentity::from_bytes([0xbe; 32])
        }),
        ("resolved_layout", |record| {
            record.resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes([0xbf; 32])
        }),
        ("x86_branch_relaxation", |record| {
            record.x86_branch_relaxation = Some(X86BranchRelaxationIdentity::from_bytes([0xc0; 32]))
        }),
        ("whole_function_exit_contract", |record| {
            record.whole_function_exit_contract =
                WholeFunctionExitContractIdentity::from_bytes([0xc1; 32])
        }),
        ("target.architecture", |record| {
            record.target.architecture = Architecture::Aarch64
        }),
        ("target.object_format", |record| {
            record.target.object_format = ObjectFormat::Coff
        }),
        ("target.pointer_size", |record| {
            record.target.pointer_size += 1
        }),
        ("target.pointer_alignment", |record| {
            record.target.pointer_alignment += 1
        }),
        ("layout_policy", |record| {
            record.layout_policy = SelectedFunctionLayoutPolicy::SingleEntryBlockV1
        }),
        ("statistics.functions", |record| {
            record.statistics.functions += 1
        }),
        ("statistics.blocks", |record| record.statistics.blocks += 1),
        ("statistics.instructions", |record| {
            record.statistics.instructions += 1
        }),
        ("statistics.bytes", |record| record.statistics.bytes += 1),
        ("statistics.resolved_conditional_branches", |record| {
            record.statistics.resolved_conditional_branches += 1
        }),
        ("statistics.structural_unit_functions", |record| {
            record.statistics.structural_unit_functions += 1
        }),
        ("statistics.structural_unit_blocks", |record| {
            record.statistics.structural_unit_blocks += 1
        }),
        ("statistics.structural_unit_instructions", |record| {
            record.statistics.structural_unit_instructions += 1
        }),
        ("statistics.structural_unit_bytes", |record| {
            record.statistics.structural_unit_bytes += 1
        }),
        ("statistics.unresolved_internal_machine_fixups", |record| {
            record.statistics.unresolved_internal_machine_fixups += 1
        }),
    ];

    for (field, mutate) in mutations {
        *staged.manifest_mut().record_mut() = baseline.clone();
        let record = staged.manifest_mut().record_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&record.encode()),
            Ok(record.clone()),
            "reauthenticated {field} mutation must retain a valid V9 envelope",
        );
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(&staged),
            Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
            "independent replay must reject reauthenticated {field}",
        );
    }

    *staged.manifest_mut().record_mut() = baseline;
    staged.manifest_mut().record_mut().identity =
        FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes([0xc2; 32]);
    assert_eq!(
        validate_function_relative_layout_optimization_realization_custody(&staged),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    );
}

fn replace_custody(
    record: &mut FunctionRelativeOptimizationRealizationManifest,
    mutate: impl FnOnce(
        PostAllocationMachineOptimizationCustody,
    ) -> PostAllocationMachineOptimizationCustody,
) {
    record.post_allocation_machine_optimization = Some(mutate(
        record
            .post_allocation_machine_optimization
            .expect("post-allocation fixture custody"),
    ));
}

#[test]
fn every_post_allocation_custody_subfield_rejects_after_reauthentication() {
    let mut staged = post_allocation_realization();
    let baseline = staged.manifest().record().clone();
    let mutations: [(&str, ManifestMutation); 9] = [
        ("post_allocation_machine_optimization.presence", |record| {
            record.post_allocation_machine_optimization = None
        }),
        (
            "post_allocation_machine_optimization.optimization",
            |record| {
                replace_custody(record, |custody| {
                    PostAllocationMachineOptimizationCustody::from_parts(
                        Optimization::X86SelectXorZeroI64MaterializationV1,
                        custody.artifact_identity(),
                        custody.selections(),
                        custody.post_allocation_machine_selections(),
                        custody.source(),
                        custody.action_count(),
                        custody.baseline_bytes(),
                        custody.selected_bytes(),
                    )
                })
            },
        ),
        (
            "post_allocation_machine_optimization.artifact_identity",
            |record| {
                replace_custody(record, |custody| {
                    PostAllocationMachineOptimizationCustody::from_parts(
                        custody.optimization(),
                        [0xd1; 32],
                        custody.selections(),
                        custody.post_allocation_machine_selections(),
                        custody.source(),
                        custody.action_count(),
                        custody.baseline_bytes(),
                        custody.selected_bytes(),
                    )
                })
            },
        ),
        (
            "post_allocation_machine_optimization.selections",
            |record| {
                replace_custody(record, |custody| {
                    PostAllocationMachineOptimizationCustody::from_parts(
                        custody.optimization(),
                        custody.artifact_identity(),
                        OptimizationSelectionIdentity::from_bytes([0xd2; 32]),
                        custody.post_allocation_machine_selections(),
                        custody.source(),
                        custody.action_count(),
                        custody.baseline_bytes(),
                        custody.selected_bytes(),
                    )
                })
            },
        ),
        (
            "post_allocation_machine_optimization.phase_selections",
            |record| {
                replace_custody(record, |custody| {
                    PostAllocationMachineOptimizationCustody::from_parts(
                        custody.optimization(),
                        custody.artifact_identity(),
                        custody.selections(),
                        OptimizationSelectionIdentity::from_bytes([0xd3; 32]),
                        custody.source(),
                        custody.action_count(),
                        custody.baseline_bytes(),
                        custody.selected_bytes(),
                    )
                })
            },
        ),
        ("post_allocation_machine_optimization.source", |record| {
            replace_custody(record, |custody| {
                PostAllocationMachineOptimizationCustody::from_parts(
                    custody.optimization(),
                    custody.artifact_identity(),
                    custody.selections(),
                    custody.post_allocation_machine_selections(),
                    PostAllocationMachineIdentity::from_bytes([0xd4; 32]),
                    custody.action_count(),
                    custody.baseline_bytes(),
                    custody.selected_bytes(),
                )
            })
        }),
        (
            "post_allocation_machine_optimization.action_count",
            |record| {
                replace_custody(record, |custody| {
                    PostAllocationMachineOptimizationCustody::from_parts(
                        custody.optimization(),
                        custody.artifact_identity(),
                        custody.selections(),
                        custody.post_allocation_machine_selections(),
                        custody.source(),
                        custody.action_count() + 1,
                        custody.baseline_bytes(),
                        custody.selected_bytes(),
                    )
                })
            },
        ),
        (
            "post_allocation_machine_optimization.baseline_bytes",
            |record| {
                replace_custody(record, |custody| {
                    PostAllocationMachineOptimizationCustody::from_parts(
                        custody.optimization(),
                        custody.artifact_identity(),
                        custody.selections(),
                        custody.post_allocation_machine_selections(),
                        custody.source(),
                        custody.action_count(),
                        custody.baseline_bytes() + 1,
                        custody.selected_bytes(),
                    )
                })
            },
        ),
        (
            "post_allocation_machine_optimization.selected_bytes",
            |record| {
                replace_custody(record, |custody| {
                    PostAllocationMachineOptimizationCustody::from_parts(
                        custody.optimization(),
                        custody.artifact_identity(),
                        custody.selections(),
                        custody.post_allocation_machine_selections(),
                        custody.source(),
                        custody.action_count(),
                        custody.baseline_bytes(),
                        custody.selected_bytes() + 1,
                    )
                })
            },
        ),
    ];

    for (field, mutate) in mutations {
        *staged.manifest_mut().record_mut() = baseline.clone();
        let record = staged.manifest_mut().record_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&record.encode()),
            Ok(record.clone()),
            "reauthenticated {field} mutation must retain a valid V9 envelope",
        );
        assert_eq!(
            validate_post_allocation_machine_function_relative_realization_custody(&staged),
            Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
            "independent replay must reject reauthenticated {field}",
        );
    }
}

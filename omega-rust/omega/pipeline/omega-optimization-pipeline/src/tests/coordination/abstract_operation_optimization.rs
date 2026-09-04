use crate::tests::*;

#[test]
fn legacy_explicit_request_still_rejects_an_empty_selection() {
    assert_eq!(
        ExplicitOptimizationRequest::new(OptimizationSelections::default(), budget()),
        Err(EmptyOptimizationSelections)
    );
}

#[test]
fn canonical_empty_selection_executes_the_identity_phase() {
    let (semantic, proof) = artifact();
    let selections = OptimizationSelections::default();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        OptimizationPipelineRequest::new(selections.clone(), budget()),
    )
    .expect("the canonical empty selection must execute as an identity phase");

    assert_eq!(optimized.selections(), &selections);
    assert!(optimized.psi_selections().is_empty());
    assert!(optimized.commits().is_empty());
    assert!(optimized.pass_manifests().is_empty());
    assert!(optimized.transformation_ledger().records().is_empty());
    assert_eq!(
        optimized.transformation_ledger().input(),
        optimized.transformation_ledger().output()
    );
}

#[test]
fn compiler_baseline_request_retains_the_selection_and_canonical_budget() {
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let request = compiler_baseline_request_v1(&selections);
    assert_eq!(request.selections(), &selections);
    assert_eq!(
        request.psi_projection().complete_selection(),
        selections.identity()
    );
    assert_eq!(
        request.budget_per_pass(),
        OptimizationWorkBudget::new(1_000_000, 100_000, 100_000, 100_000, 10_000).unwrap()
    );

    let (semantic, proof) = artifact();
    let optimized =
        optimize_artifact_sections(&semantic, &proof, &AdmissionProfile::default(), request)
            .expect("the compiler baseline request must run the selected optimization");
    assert_eq!(optimized.selections(), &selections);
}

#[test]
fn canonical_three_pass_suite_retains_each_manifest_and_one_ledger() {
    let (semantic, proof) = artifact();
    let selections = OptimizationSelections::new([
        Optimization::CopyPropagation,
        Optimization::SparseConditionalConstantPropagation,
        Optimization::ControlFlowCleanup,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(selections.clone()),
    )
    .unwrap();

    assert_eq!(optimized.selections(), &selections);
    assert_eq!(optimized.commits().len(), 2);
    assert_eq!(optimized.pass_manifests().len(), 3);
    assert_eq!(optimized.transformation_ledger().records().len(), 2);
    assert_eq!(
        optimized
            .pass_manifests()
            .iter()
            .map(|manifest| manifest.work_usage().commits)
            .collect::<Vec<_>>(),
        [1, 1, 0]
    );
    assert_eq!(
        optimized.pass_manifests()[0].output(),
        optimized.pass_manifests()[1].input()
    );
    assert_eq!(
        optimized.pass_manifests()[1].output(),
        optimized.pass_manifests()[2].input()
    );
    assert!(matches!(
        optimized.plan().functions[0].operations[2],
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(15),
            ..
        }
    ));
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
    assert_eq!(optimized.plan().functions[0].operations.len(), 4);
    assert!(matches!(
        &optimized.plan().functions[0].operations[3],
        AbstractOperation::Return { value, .. } if *value == ValueId::new(2_006).unwrap()
    ));
}

#[test]
fn three_pass_artifact_orchestration_is_deterministic() {
    let (semantic, proof) = artifact();
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let run = || {
        optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap()
    };
    let first = run();
    let second = run();

    assert_eq!(first.plan(), second.plan());
    assert_eq!(first.pass_manifests(), second.pass_manifests());
    assert_eq!(
        first.transformation_ledger(),
        second.transformation_ledger()
    );
    assert_eq!(first.identity_bundle(), second.identity_bundle());
    assert_eq!(first.validation(), second.validation());
}

#[test]
fn control_flow_cleanup_selection_runs_as_its_own_exact_pass() {
    let (semantic, proof) = artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
    )
    .unwrap();
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(
        optimized.selections().as_slice(),
        [Optimization::ControlFlowCleanup]
    );
}

#[test]
fn control_flow_cleanup_projects_an_atomically_pruned_block_roster() {
    let (semantic, proof) = constant_conditional_prune_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
    )
    .unwrap();

    assert_eq!(optimized.commits().len(), 2);
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
    assert_eq!(
        optimized.plan().functions[0].block_entries[0].operation_offset,
        0
    );
    assert_eq!(optimized.plan().functions[0].operations.len(), 2);
    assert_eq!(optimized.transformation_ledger().records().len(), 2);
    assert_eq!(
        optimized.transformation_ledger().records()[0]
            .provenance
            .iter()
            .filter(|row| !row.disposition.is_realized())
            .count(),
        2
    );
    let report = optimized.pre_physical_manifest().record().render_text();
    assert!(report.contains("source structure: functions=1, blocks=3, nodes=4"));
    assert!(report.contains("optimized structure: functions=1, blocks=1, nodes=2"));
    assert!(report.contains("proven-unreachable=2"));
    assert!(report.contains("runtime-charge=none reason=proven-unreachable"));
}

#[test]
fn control_flow_cleanup_projects_linear_threading_with_both_fuel_sources() {
    let (semantic, proof) = linear_empty_block_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
    )
    .unwrap();

    assert_eq!(optimized.commits().len(), 2);
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
    assert_eq!(optimized.plan().functions[0].operations.len(), 1);
    assert_eq!(
        optimized.unit().functions[0].blocks[0].nodes[0]
            .provenance
            .len(),
        3
    );
    assert_eq!(
        optimized.unit().functions[0].blocks[0].nodes[0].fuel.len(),
        3
    );
    assert_eq!(optimized.transformation_ledger().records().len(), 2);
    assert!(
        optimized
            .transformation_ledger()
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .all(|row| row.disposition.is_realized())
    );
    assert!(
        optimized
            .pre_physical_manifest()
            .record()
            .render_text()
            .contains("optimized structure: functions=1, blocks=1, nodes=1")
    );
}

#[test]
fn control_flow_cleanup_projects_adjacent_block_merge_occurrences() {
    let (semantic, proof) = adjacent_block_merge_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
    )
    .unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 1);
    assert_eq!(optimized.plan().functions[0].operations.len(), 2);
    let first = &optimized.unit().functions[0].blocks[0].nodes[0];
    assert!(matches!(
        first.operation,
        omega_abstract_operations::AbstractOperation::BooleanNot {
            operand,
            ..
        } if operand == ValueId::new(4_254).unwrap()
    ));
    assert_eq!(
        first.provenance,
        [
            omega_optimization_unit::PsiProvenance::Operation(OperationId::new(4_258).unwrap()),
            omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(4_257).unwrap()),
        ]
    );
    assert_eq!(first.fuel.len(), 2);
    assert_eq!(optimized.transformation_ledger().records().len(), 1);
    assert_eq!(
        optimized.transformation_ledger().records()[0]
            .provenance
            .len(),
        3
    );
    let report = optimized.pre_physical_manifest().record().render_text();
    assert!(report.contains("source structure: functions=1, blocks=2, nodes=3"));
    assert!(report.contains("optimized structure: functions=1, blocks=1, nodes=2"));
}

#[test]
fn control_flow_cleanup_projects_adjacent_conditional_fanout() {
    let (semantic, proof) = adjacent_conditional_merge_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
    )
    .unwrap();

    assert_eq!(optimized.commits().len(), 1);
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 3);
    assert_eq!(optimized.plan().functions[0].operations.len(), 3);
    let node = &optimized.unit().functions[0].blocks[0].nodes[0];
    assert!(matches!(
        node.operation,
        omega_abstract_operations::AbstractOperation::Conditional {
            condition,
            ..
        } if condition == ValueId::new(4_276).unwrap()
    ));
    let inherited = omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(4_278).unwrap());
    assert!(
        node.successors
            .iter()
            .all(|edge| edge.provenance.last() == Some(&inherited))
    );
    let input = omega_optimization_unit::PsiRealizationSite::Edge {
        machine: MachineId::new(4_271).unwrap(),
        edge: EdgeId::new(4_278).unwrap(),
    };
    assert_eq!(
        optimized.transformation_ledger().records()[0]
            .provenance
            .iter()
            .filter(|row| row.input == input)
            .count(),
        2
    );
    let report = optimized.pre_physical_manifest().record().render_text();
    assert!(report.contains("source structure: functions=1, blocks=4, nodes=4"));
    assert!(report.contains("optimized structure: functions=1, blocks=3, nodes=3"));
}

#[test]
fn control_flow_cleanup_projects_path_qualified_fanout_custody() {
    let (semantic, proof) = path_qualified_empty_block_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap()),
    )
    .unwrap();

    assert_eq!(optimized.commits().len(), 3);
    assert_eq!(optimized.plan().functions[0].block_entries.len(), 2);
    assert_eq!(optimized.plan().functions[0].operations.len(), 2);
    let outgoing = omega_optimization_unit::PsiProvenance::Edge(EdgeId::new(4_315).unwrap());
    let occurrences = optimized.unit().functions[0]
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter())
        .flat_map(|node| node.successors.iter())
        .filter(|edge| edge.provenance.contains(&outgoing))
        .collect::<Vec<_>>();
    assert_eq!(occurrences.len(), 2);
    assert_eq!(occurrences[0].target, BlockId::new(4_306).unwrap());
    assert_eq!(occurrences[1].target, BlockId::new(4_306).unwrap());
    let ledger = optimized.transformation_ledger();
    let outgoing_site = omega_optimization_unit::PsiRealizationSite::Edge {
        machine: MachineId::new(4_301).unwrap(),
        edge: EdgeId::new(4_315).unwrap(),
    };
    assert_eq!(
        ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .filter(|row| row.input == outgoing_site)
            .count(),
        2
    );
    assert!(
        optimized
            .pre_physical_manifest()
            .record()
            .render_text()
            .contains("optimized structure: functions=1, blocks=2, nodes=2")
    );
}

#[test]
fn mixed_phase_suite_retains_the_full_request_and_exact_psi_projection() {
    let (semantic, proof) = artifact();
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(selections.clone()),
    )
    .unwrap();

    assert_eq!(optimized.selections(), &selections);
    assert_eq!(
        optimized.psi_selections().as_slice(),
        &[Optimization::SparseConditionalConstantPropagation]
    );
    assert_eq!(optimized.pass_manifests().len(), 1);
    assert_eq!(
        optimized.pre_physical_manifest().record().selections,
        selections
    );
}

#[test]
fn lower_only_suite_reaches_prephysical_custody_without_claiming_psi_work() {
    let (semantic, proof) = artifact();
    let selections =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(selections.clone()),
    )
    .unwrap();

    assert_eq!(optimized.selections(), &selections);
    assert!(optimized.psi_selections().is_empty());
    assert!(optimized.commits().is_empty());
    assert!(optimized.pass_manifests().is_empty());
    assert!(
        optimized
            .pre_physical_manifest()
            .record()
            .psi_selections
            .is_empty()
    );
}

#[test]
fn unsupported_target_shape_fails_at_legalization_boundary() {
    let (semantic, proof) = artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        stage_optimized_instruction_selection(target),
        Err(OptimizedSelectionPipelineError::Legalization(
            LegalizationError::UnsupportedSourceShape { function: 0 }
        ))
    ));
}

#[test]
fn non_u64_conditional_fails_at_named_integer_legalization_boundary() {
    let (semantic, proof) = conditional_immediate_artifact_with_type(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
    );
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    assert!(matches!(
        stage_optimized_instruction_selection(target),
        Err(OptimizedSelectionPipelineError::Legalization(
            LegalizationError::UnsupportedIntegerShape { function: 0 }
        ))
    ));
}

#[test]
fn overflowing_u8_add_is_rejected_before_the_widen_commutation_recipe() {
    let (semantic, proof) =
        conditional_widened_u8_exact_add_artifact_with_values([255, 1], [254, 1]);
    let error = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        OptimizationPipelineError::ArtifactLowering(
            omega_psi_to_abstract_operations::ArtifactLoweringError::Verification(
                psi_terminal_verifier::VerificationError::RejectedEvidence {
                    obligation,
                    ..
                }
            )
        ) if obligation == ObligationId::new(5_131).unwrap()
    ));
}

#[test]
fn underflowing_u8_subtract_is_rejected_before_the_widen_commutation_recipe() {
    let (semantic, proof) =
        conditional_widened_u8_exact_subtract_artifact_with_values([0, 1], [200, 55]);
    let error = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        OptimizationPipelineError::ArtifactLowering(
            omega_psi_to_abstract_operations::ArtifactLoweringError::Verification(
                psi_terminal_verifier::VerificationError::RejectedEvidence {
                    obligation,
                    ..
                }
            )
        ) if obligation == ObligationId::new(5_131).unwrap()
    ));
}

#[test]
fn per_pass_budget_exhaustion_publishes_no_carrier() {
    let (semantic, proof) = artifact();
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let constrained = OptimizationWorkBudget::new(128, 128, 128, 1, 16).unwrap();
    let error = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, constrained).unwrap(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        OptimizationPipelineError::Run(OptimizationRunError::WorkBudgetExhausted("commits"))
    ));
}

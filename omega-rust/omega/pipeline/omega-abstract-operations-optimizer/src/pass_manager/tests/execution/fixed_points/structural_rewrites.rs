//! Control-flow cleanup and copy-propagation structural fixed points.

use super::super::super::*;

#[test]
fn named_control_flow_cleanup_reaches_edge_count_fixed_point() {
    let unit = constant_conditional_same_target_unit(true);
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit.clone(), &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(usage.commits, 2);
    assert_eq!(usage.iterations, 3);
    assert_eq!(output.functions[0].blocks.len(), 1);
    assert_eq!(ledger.records().len(), 2);
    assert_eq!(ledger.records()[0].provenance.len(), 2);
    assert!(matches!(
        ledger.records()[0].provenance[0].disposition,
        omega_optimization_unit::ProvenanceDisposition::RealizedAt(_)
    ));
    assert!(matches!(
        ledger.records()[0].provenance[1].disposition,
        omega_optimization_unit::ProvenanceDisposition::ProvenUnreachableAt(_)
    ));
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 7);
    assert_eq!(manifest.decisions().len(), 2);
    assert_eq!(manifest.decisions()[0].consumed_facts().len(), 1);

    let (second, second_commits, _, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_control_flow_cleanup_atomically_prunes_and_accounts_for_a_dead_arm() {
    let unit = propagated_block_parameter_unit(true);
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit.clone(), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 3);
    assert_eq!(usage.commits, 3);
    assert_eq!(usage.iterations, 4);
    assert_eq!(unit.functions[0].blocks.len(), 4);
    assert_eq!(output.functions[0].blocks.len(), 1);
    assert_eq!(ledger.records().len(), 3);
    assert_eq!(ledger.records()[0].provenance.len(), 6);
    assert_eq!(
        ledger.records()[0]
            .provenance
            .iter()
            .filter(|row| row.disposition.is_realized())
            .count(),
        3
    );
    assert_eq!(
        ledger.records()[0]
            .provenance
            .iter()
            .filter(|row| !row.disposition.is_realized())
            .count(),
        3
    );
    assert_eq!(output.functions[0].facts.len(), 2);
    assert_eq!(output.functions[0].blocks[0].nodes[0].effect.input, 0);
    assert_eq!(output.functions[0].blocks[0].nodes[3].effect.output, 4);
    assert_eq!(manifest.unwrap().decisions().len(), 4);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_control_flow_cleanup_threads_a_linear_empty_block_to_fixed_point() {
    let unit = linear_empty_block_unit();
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit.clone(), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(usage.commits, 2);
    assert_eq!(usage.iterations, 3);
    assert_eq!(usage.rule_evaluations, 13);
    assert_eq!(output.functions[0].blocks.len(), 1);
    assert_eq!(ledger.records().len(), 2);
    assert_eq!(ledger.records()[0].provenance.len(), 3);
    assert!(
        ledger.records()[0]
            .provenance
            .iter()
            .all(|row| row.disposition.is_realized())
    );
    assert_eq!(manifest.unwrap().ordered_rules().len(), 7);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_control_flow_cleanup_merges_non_adjacent_blocks_to_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    for target_before_predecessor in [false, true] {
        let unit = non_adjacent_merge_unit(target_before_predecessor);
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(usage.commits, 2);
        assert_eq!(usage.iterations, 3);
        assert_eq!(
            usage.rule_evaluations,
            if target_before_predecessor { 21 } else { 18 }
        );
        assert_eq!(output.functions[0].blocks.len(), 3);
        assert_eq!(ledger.records().len(), 2);
        assert!(ledger.records().iter().all(|record| {
            record
                .provenance
                .iter()
                .all(|row| row.disposition.is_realized())
        }));
        assert_eq!(manifest.unwrap().ordered_rules().len(), 7);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second, output);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert_eq!(second_usage.rule_evaluations, 7);
        assert!(second_ledger.records().is_empty());
    }
}

#[test]
fn named_copy_propagation_reaches_its_block_parameter_fixed_point() {
    let unit = redundant_block_parameter_unit(true);
    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit.clone(), &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.commits, 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 2);
    assert!(output.functions[0].blocks[1].parameters.is_empty());
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.decisions().len(), 1);
    assert!(manifest.decisions()[0].consumed_facts().is_empty());
    assert_eq!(manifest.decisions()[0].input(), unit.identity);
    assert_eq!(manifest.output(), output.identity);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second, output);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn copy_propagation_is_disabled_by_default_deterministic_and_budgeted() {
    let unit = redundant_block_parameter_unit(true);
    let disabled = built_in_psi_registry(&OptimizationSelections::default()).unwrap();
    let (unchanged, commits, usage, decisions, manifest, ledger) =
        run_unit(unit.clone(), &disabled, budget(8)).unwrap();
    assert_eq!(unchanged, unit);
    assert!(commits.is_empty());
    assert_eq!(usage.iterations, 1);
    assert_eq!(usage.rule_evaluations, 0);
    assert!(decisions.records.is_empty());
    assert!(manifest.is_none());
    assert!(ledger.records().is_empty());

    let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let first = run_unit(unit.clone(), &registry, budget(8)).unwrap();
    let second = run_unit(unit.clone(), &registry, budget(8)).unwrap();
    assert_eq!(first.0, second.0);
    assert_eq!(first.1, second.1);
    assert_eq!(first.2, second.2);
    assert_eq!(first.3, second.3);
    assert_eq!(first.4, second.4);
    assert_eq!(first.5, second.5);

    let first_error = run_unit(unit.clone(), &registry, budget(1)).unwrap_err();
    let second_error = run_unit(unit, &registry, budget(1)).unwrap_err();
    assert_eq!(first_error, second_error);
    assert_eq!(
        first_error,
        OptimizationRunError::WorkBudgetExhausted("iterations")
    );
}

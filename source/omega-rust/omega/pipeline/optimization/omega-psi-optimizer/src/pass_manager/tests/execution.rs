use super::*;

#[test]
fn fixed_point_dispatch_validates_then_commits_with_stable_usage() {
    let unit = exact_add_unit();
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, decisions, pass_manifest, ledger) =
        run_unit(unit.clone(), &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].input, unit.identity);
    assert_eq!(commits[0].output, output.identity);
    assert_eq!(usage.commits, 1);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 40);
    assert_eq!(decisions.records.len(), 1);
    assert_eq!(
        decisions.records[0].outcome,
        BaselineDecisionOutcome::Choose(commits[0].candidate)
    );
    let pass_manifest = pass_manifest.expect("selected pass emits a manifest row");
    assert_eq!(pass_manifest.ordered_rules().len(), 39);
    assert_eq!(pass_manifest.input(), unit.identity);
    assert_eq!(pass_manifest.output(), output.identity);
    assert_eq!(pass_manifest.decisions().len(), 1);
    assert_eq!(pass_manifest.decisions()[0].input(), unit.identity);
    assert_eq!(pass_manifest.decisions()[0].consumed_facts().len(), 3);
    assert_eq!(
        pass_manifest.decisions()[0].verdict(),
        OptimizationCandidateVerdict::Applied
    );
    assert_eq!(
        OptimizationPassManifestRecord::decode(&pass_manifest.encode()),
        Ok(pass_manifest)
    );
    assert_eq!(ledger.input(), unit.identity);
    assert_eq!(ledger.output(), output.identity);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(ledger.records()[0].provenance, commits[0].provenance);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant { .. }
    ));
}

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
fn ordered_multi_rule_group_reaches_a_dependent_exact_fixed_point() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, pass_manifest, ledger) =
        run_unit(dependent_exact_chain_unit(), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(usage.iterations, 3);
    assert_eq!(usage.rule_evaluations, 43);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(15),
            ..
        }
    ));
    assert!(matches!(
        output.functions[0].blocks[0].nodes[3].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(120),
            ..
        }
    ));
    let manifest = pass_manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 39);
    assert_eq!(manifest.decisions().len(), 2);
    assert_eq!(ledger.records().len(), 2);
}

#[test]
fn named_dead_scalar_suite_reaches_a_custody_preserving_fixed_point() {
    let selections =
        OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let unit = dead_wrapping_add_unit();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 3);
    assert_eq!(usage.iterations, 4);
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 1);
    assert_eq!(output.functions[0].blocks[0].nodes[0].provenance.len(), 4);
    assert_eq!(ledger.records().len(), 3);
    assert!(
        ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .all(|row| matches!(
                row.disposition,
                omega_optimization_unit::ProvenanceDisposition::RealizedAt(_)
            ))
    );
    assert_eq!(manifest.unwrap().ordered_rules().len(), 2);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_proof_check_elision_reaches_an_evidence_preserving_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let unit = dead_exact_add_unit();
    let accepted_fact = unit.accepted_obligation_facts[0].identity;
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 3);
    assert_eq!(output.accepted_obligation_facts.len(), 1);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 11);
    assert_eq!(
        manifest.decisions()[0].consumed_facts(),
        [OptimizationFactReference::AcceptedObligation(accepted_fact)]
    );

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_proof_check_elision_materializes_self_subtract_zero_at_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
    let unit = live_exact_self_subtract_unit(integer);
    let accepted_fact = unit.accepted_obligation_facts[0].identity;
    let original_provenance = unit.functions[0].blocks[0].nodes[0].provenance.clone();
    let original_fuel = unit.functions[0].blocks[0].nodes[0].fuel.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 11);
    assert_eq!(
        commits[0].declaration.consumed_facts(),
        [OptimizationFactReference::AcceptedObligation(accepted_fact),]
    );
    assert!(matches!(
        output.functions[0].blocks[0].nodes[0].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(0),
            ..
        }
    ));
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].provenance,
        original_provenance
    );
    assert_eq!(output.functions[0].blocks[0].nodes[0].fuel, original_fuel);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_proof_check_elision_materializes_self_remainder_zero_at_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
    let unit = live_self_remainder_unit(integer, SelfRemainderPolicy::Exact);
    let accepted_fact = unit.accepted_obligation_facts[0].identity;
    let original_provenance = unit.functions[0].blocks[0].nodes[0].provenance.clone();
    let original_fuel = unit.functions[0].blocks[0].nodes[0].fuel.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 11);
    assert_eq!(
        manifest.decisions()[0].rule(),
        crate::LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract().identity()
    );
    assert_eq!(
        commits[0].declaration.consumed_facts(),
        [OptimizationFactReference::AcceptedObligation(accepted_fact)]
    );
    assert!(matches!(
        output.functions[0].blocks[0].nodes[0].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(0),
            ..
        }
    ));
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].provenance,
        original_provenance
    );
    assert_eq!(output.functions[0].blocks[0].nodes[0].fuel, original_fuel);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_proof_check_elision_materializes_self_divide_one_at_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
    let unit = live_self_divide_unit(integer, SelfDividePolicy::Exact);
    let accepted_fact = unit.accepted_obligation_facts[0].identity;
    let original_provenance = unit.functions[0].blocks[0].nodes[0].provenance.clone();
    let original_fuel = unit.functions[0].blocks[0].nodes[0].fuel.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 11);
    assert_eq!(
        manifest.decisions()[0].rule(),
        crate::LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity()
    );
    assert_eq!(
        commits[0].declaration.consumed_facts(),
        [OptimizationFactReference::AcceptedObligation(accepted_fact)]
    );
    assert!(matches!(
        output.functions[0].blocks[0].nodes[0].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(1),
            ..
        }
    ));
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].provenance,
        original_provenance
    );
    assert_eq!(output.functions[0].blocks[0].nodes[0].fuel, original_fuel);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_proof_check_elision_materializes_remainder_by_one_zero_at_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
    let unit = live_remainder_by_one_unit(integer, SelfRemainderPolicy::Exact);
    let original_provenance = unit.functions[0].blocks[0].nodes[1].provenance.clone();
    let original_fuel = unit.functions[0].blocks[0].nodes[1].fuel.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 11);
    assert_eq!(
        manifest.decisions()[0].rule(),
        crate::LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract().identity()
    );
    assert_eq!(commits[0].declaration.consumed_facts().len(), 2);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(0),
            ..
        }
    ));
    assert_eq!(
        output.functions[0].blocks[0].nodes[1].provenance,
        original_provenance
    );
    assert_eq!(output.functions[0].blocks[0].nodes[1].fuel, original_fuel);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_proof_check_elision_materializes_signed_remainder_by_negative_one_zero() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 8).unwrap();
    let unit = live_signed_remainder_by_negative_one_unit(integer, SelfRemainderPolicy::Exact);
    let original_provenance = unit.functions[0].blocks[0].nodes[1].provenance.clone();
    let original_fuel = unit.functions[0].blocks[0].nodes[1].fuel.clone();
    let accepted_catalog = unit.accepted_obligation_facts.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 11);
    assert_eq!(
        manifest.decisions()[0].rule(),
        crate::LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule::contract()
            .identity()
    );
    assert_eq!(commits[0].declaration.consumed_facts().len(), 2);
    assert_eq!(output.accepted_obligation_facts, accepted_catalog);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Signed(0),
            ..
        }
    ));
    assert_eq!(
        output.functions[0].blocks[0].nodes[1].provenance,
        original_provenance
    );
    assert_eq!(output.functions[0].blocks[0].nodes[1].fuel, original_fuel);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_a_cross_block_ledger_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(diamond_dominator_gvn_unit(), &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(usage.iterations, 3);
    assert_eq!(usage.rule_evaluations, 15);
    assert_eq!(usage.candidates, 2);
    assert_eq!(usage.validation_steps, 2);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
    assert_eq!(ledger.records().len(), 2);
    assert_eq!(ledger.records()[0].provenance.len(), 5);
    assert_eq!(ledger.records()[1].provenance.len(), 4);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 9);
    assert!(second_ledger.records().is_empty());
    assert_eq!(second_ledger.input(), second_ledger.output());
}

#[test]
fn named_global_value_numbering_reaches_a_phi_translated_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(phi_translated_gvn_unit(), &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 14);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
    assert_eq!(ledger.records().len(), 1);
    let join = &output.functions[0].blocks[0];
    assert_eq!(join.parameters.len(), 2);
    assert_eq!(join.nodes.len(), 1);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 9);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_records_proof_phi_fact_consumption() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let unit = proof_certified_phi_translated_gvn_unit();
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == psi_core::OperationId::new(1_713).unwrap())
        .unwrap()
        .identity;
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 15);
    assert_eq!(usage.candidates, 1);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 9);
    assert_eq!(
        manifest.decisions()[0].consumed_facts(),
        [OptimizationFactReference::AcceptedObligation(
            redundant_fact
        )]
    );
    assert_eq!(output.accepted_obligation_facts.len(), 3);
    assert_eq!(output.functions[0].blocks[0].parameters.len(), 2);

    let (_, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output, &registry, budget(8)).unwrap();
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.rule_evaluations, 9);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_records_proof_certified_fact_consumption() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let unit = proof_certified_local_cse_unit();
    let redundant_fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| fact.operation == psi_core::OperationId::new(1_309).unwrap())
        .unwrap()
        .identity;

    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 3);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 9);
    assert_eq!(
        manifest.decisions()[0].consumed_facts(),
        [OptimizationFactReference::AcceptedObligation(
            redundant_fact
        )]
    );

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_compatible_policy_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let unit = compatible_policy_local_cse_unit();
    let accepted_catalog = unit.accepted_obligation_facts.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(output.accepted_obligation_facts, accepted_catalog);
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 3);

    let (second, commits, usage, _, _, ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second, output);
    assert!(commits.is_empty());
    assert_eq!(usage.rule_evaluations, 9);
    assert!(ledger.records().is_empty());
}

#[test]
fn named_global_value_numbering_reaches_compatible_policy_phi_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let unit = compatible_policy_phi_translated_gvn_unit();
    let accepted_catalog = unit.accepted_obligation_facts.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(usage.rule_evaluations, 18);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(output.accepted_obligation_facts, accepted_catalog);
    assert_eq!(output.functions[0].blocks[0].parameters.len(), 2);

    let (second, commits, usage, _, _, ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second, output);
    assert!(commits.is_empty());
    assert_eq!(usage.rule_evaluations, 9);
    assert!(ledger.records().is_empty());
}

#[test]
fn shuffled_builtin_registration_constructs_identical_multi_rule_runs() {
    for (optimization, unit) in [
        (
            Optimization::SparseConditionalConstantPropagation,
            dependent_exact_chain_unit(),
        ),
        (
            Optimization::ControlFlowCleanup,
            propagated_block_parameter_unit(true),
        ),
        (
            Optimization::GlobalValueNumbering,
            compatible_policy_local_cse_unit(),
        ),
        (
            Optimization::ProofCheckElision,
            live_self_divide_unit(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                SelfDividePolicy::Exact,
            ),
        ),
        (
            Optimization::DeadPureScalarElimination,
            dead_wrapping_add_unit(),
        ),
    ] {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let expected_registry = built_in_psi_registry(&selections).unwrap();
        let expected = run_unit(unit.clone(), &expected_registry, budget(8)).unwrap();

        for (seed, registry) in randomized_built_in_registries(optimization)
            .into_iter()
            .enumerate()
        {
            let actual = run_unit(unit.clone(), &registry, budget(8)).unwrap();
            let context = format!("{optimization:?} registration seed {}", seed + 1);
            assert_eq!(actual.0, expected.0, "final unit differs for {context}");
            assert_eq!(actual.1, expected.1, "commits differ for {context}");
            assert_eq!(actual.2, expected.2, "usage differs for {context}");
            assert_eq!(actual.3, expected.3, "decisions differ for {context}");
            assert_eq!(actual.4, expected.4, "manifest differs for {context}");
            assert_eq!(actual.5, expected.5, "ledger differs for {context}");
        }
    }
}

#[test]
fn full_sccp_cfg_copy_gvn_proof_dead_scalar_second_sweep_is_a_composed_ledger_fixed_point() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::ControlFlowCleanup,
        Optimization::CopyPropagation,
        Optimization::GlobalValueNumbering,
        Optimization::ProofCheckElision,
        Optimization::DeadPureScalarElimination,
    ])
    .unwrap();
    let registries = built_in_psi_registries(&selections).unwrap();

    for initial in [
        dependent_exact_chain_unit(),
        redundant_block_parameter_unit(true),
        dead_wrapping_add_unit(),
        dead_exact_add_unit(),
        local_cse_unit(),
        dominator_gvn_unit(),
        proof_certified_local_cse_unit(),
        proof_certified_dominator_gvn_unit(),
        live_divide_by_one_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            |psi_operation, obligation, result, scalar_type, left, right| {
                AbstractOperation::ExactIntegerDivide {
                    psi_operation,
                    obligation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
            },
        ),
        live_exact_multiply_by_zero_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            false,
        ),
        live_exact_zero_value_shift_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            true,
        ),
        live_self_remainder_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            SelfRemainderPolicy::Exact,
        ),
        live_self_divide_unit(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
            SelfDividePolicy::Exact,
        ),
    ] {
        let (first_output, first_manifests, first_ledger) = run_test_pipeline(initial, &registries);
        assert_eq!(first_manifests.len(), 6);
        assert_eq!(first_manifests[0].input(), first_ledger.input());
        assert_eq!(first_manifests[0].output(), first_manifests[1].input());
        assert_eq!(first_manifests[1].output(), first_manifests[2].input());
        assert_eq!(first_manifests[2].output(), first_manifests[3].input());
        assert_eq!(first_manifests[3].output(), first_manifests[4].input());
        assert_eq!(first_manifests[4].output(), first_manifests[5].input());
        assert_eq!(first_manifests[5].output(), first_ledger.output());
        assert!(!first_ledger.records().is_empty());

        let (second_output, second_manifests, second_delta) =
            run_test_pipeline(first_output.clone(), &registries);
        assert_eq!(second_output, first_output);
        assert_eq!(second_manifests.len(), 6);
        assert!(second_delta.records().is_empty());
        assert_eq!(second_delta.input(), second_delta.output());
        assert!(second_manifests.iter().all(|manifest| {
            manifest.input() == manifest.output()
                && manifest
                    .decisions()
                    .iter()
                    .all(|decision| decision.verdict() != OptimizationCandidateVerdict::Applied)
        }));

        let mut composed_records = first_ledger.records().to_vec();
        composed_records.extend_from_slice(second_delta.records());
        let composed = PsiTransformationLedger::new(
            first_ledger.psi(),
            first_ledger.fuel_schedule(),
            first_ledger.input(),
            second_delta.output(),
            composed_records,
        )
        .unwrap();
        assert_eq!(composed, first_ledger);
    }
}

#[test]
fn manifest_retains_propagated_block_parameter_fact_identity() {
    let unit = propagated_block_parameter_unit(true);
    let AnalysisProduct::ScalarConstants(constants) = crate::compute_analysis(
        &unit,
        omega_optimization_core::AnalysisKind::ScalarConstants,
    )
    .unwrap() else {
        unreachable!()
    };
    let derived = constants
        .facts
        .iter()
        .find(|fact| !fact.support.edges.is_empty())
        .and_then(|fact| fact.identity)
        .expect("fixture has one proof-bearing propagated parameter fact");
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (_, commits, _, _, manifest, _) = run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(
        manifest.unwrap().decisions()[0].consumed_facts(),
        &[OptimizationFactReference::ScalarConstant(derived)]
    );
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
    assert!(output.functions[0].blocks[1].parameters.is_empty());
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.decisions().len(), 1);
    assert!(manifest.decisions()[0].consumed_facts().is_empty());
    assert_eq!(manifest.decisions()[0].input(), unit.identity);
    assert_eq!(manifest.output(), output.identity);
}

#[test]
fn pass_convergence_measure_includes_wrapping_and_saturating_rules() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, _, _) =
        run_unit(wrapping_add_unit(), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: psi_core::IntegerValue::Unsigned(4),
            ..
        }
    ));
}

#[test]
fn pass_dispatches_typed_boolean_validation_to_fixed_point() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, _, _) =
        run_unit(boolean_unit(true), &registry, budget(8)).unwrap();

    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::BooleanConstant { value: false, .. }
    ));
}

#[test]
fn exhausted_iteration_budget_fails_deterministically_without_output() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let first = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
    let second = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
    assert_eq!(first, second);
    assert_eq!(
        first,
        OptimizationRunError::WorkBudgetExhausted("iterations")
    );
}

#[test]
fn synthetic_a_to_b_to_a_revision_cycle_fails_before_repeated_commit() {
    let a = OptimizationUnitIdentity::from_canonical_bytes(b"synthetic-state-a");
    let b = OptimizationUnitIdentity::from_canonical_bytes(b"synthetic-state-b");

    let run = || {
        let mut seen = BTreeMap::from([(a, 0)]);
        let mut committed = Vec::new();
        register_revision(&mut seen, b, 1)?;
        committed.push(b);
        let error = register_revision(&mut seen, a, 2).unwrap_err();
        Ok::<_, OptimizationRunError>((committed, error, seen))
    };

    let first = run().unwrap();
    let second = run().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.0, vec![b]);
    assert_eq!(first.2, BTreeMap::from([(a, 0), (b, 1)]));
    assert_eq!(
        first.1,
        OptimizationRunError::OscillatingRevision {
            identity: a,
            first_seen_iteration: 0,
            repeated_at_iteration: 2,
        }
    );
}

#[test]
fn nonprofitable_validated_candidate_is_recorded_as_a_skip() {
    let registry =
        OrderedRuleRegistry::new(
            [Arc::new(NonProfitableExactRule) as Arc<dyn PsiOptimizationRule>],
        )
        .unwrap();
    let (unit, commits, _, decisions, pass_manifest, ledger) =
        run_unit(exact_add_unit(), &registry, budget(2)).unwrap();

    assert!(commits.is_empty());
    assert_eq!(decisions.records.len(), 1);
    assert!(matches!(
        unit.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::ExactIntegerAdd { .. }
    ));
    let manifest = pass_manifest.unwrap();
    assert_eq!(manifest.decisions().len(), 1);
    assert_eq!(
        manifest.decisions()[0].verdict(),
        OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::NotProfitable)
    );
    assert!(manifest.decisions()[0].validator().is_some());
    assert_eq!(manifest.decisions()[0].consumed_facts().len(), 3);
    assert!(ledger.records().is_empty());
    assert_eq!(ledger.input(), ledger.output());
}

#[test]
fn duplicate_candidate_identity_fails_closed_without_a_manifest() {
    let registry =
        OrderedRuleRegistry::new([Arc::new(DuplicateExactRule) as Arc<dyn PsiOptimizationRule>])
            .unwrap();
    assert!(matches!(
        run_unit(exact_add_unit(), &registry, budget(2)),
        Err(OptimizationRunError::DuplicateCandidate(_))
    ));
}

#[test]
fn public_run_requires_and_retains_verified_optimizer_context() {
    let selections = OptimizationSelections::default();
    let registry = OrderedRuleRegistry::new(Vec::new()).unwrap();
    let run = run_psi_registry(verified_empty_unit(), &selections, &registry, budget(2)).unwrap();
    assert!(run.commits.is_empty());
    assert!(run.pass_manifests.is_empty());
    assert!(run.external_decisions().points().is_empty());
    assert_eq!(
        ExternalDecisionLog::decode(&run.external_decisions().encode()),
        Ok(run.external_decisions().clone())
    );
    assert_eq!(
        run.external_decisions().context().source(),
        run.transformation_ledger.input()
    );
    assert!(run.transformation_ledger.records().is_empty());
    assert_eq!(run.identity_bundle.selections(), selections.identity());
    assert_eq!(
        run.identity_bundle.transformation_ledger(),
        run.transformation_ledger.identity()
    );
    assert_eq!(run.usage.iterations, 0);
    assert_eq!(run.session.unit().psi, run.session.input().plan().psi);
}

#[test]
fn lower_only_suite_retains_the_request_but_executes_no_psi_pass() {
    let selections =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    let run = run_psi_pipeline(verified_empty_unit(), &selections, budget(2)).unwrap();

    assert_eq!(run.selections(), &selections);
    assert!(run.psi_selections().is_empty());
    assert_eq!(run.identity_bundle.selections(), selections.identity());
    assert_eq!(
        run.identity_bundle.rule_set(),
        OptimizationRuleSetIdentity::from_ordered_rules(&[]).unwrap()
    );
    assert!(run.commits.is_empty());
    assert!(run.pass_manifests.is_empty());
    assert!(run.decisions.records.is_empty());
    assert!(run.external_decisions().points().is_empty());
    assert_eq!(
        run.external_decisions().context().selections(),
        selections.identity()
    );
    assert_eq!(
        run.external_decisions().context().phase_selections(),
        run.psi_selections().identity()
    );
    assert!(run.transformation_ledger.records().is_empty());
    assert_eq!(run.usage, OptimizationRunUsage::default());
    assert_eq!(
        run.transformation_ledger.input(),
        run.transformation_ledger.output()
    );
}

#[test]
fn mixed_suite_executes_only_its_psi_projection() {
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let psi_selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let run = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
    let registry = built_in_psi_registry(&psi_selections).unwrap();

    assert_eq!(run.selections(), &selections);
    assert_eq!(run.psi_selections(), &psi_selections);
    assert_eq!(run.identity_bundle.selections(), selections.identity());
    assert_eq!(run.identity_bundle.rule_set(), registry.identity());
    assert_eq!(run.pass_manifests.len(), 1);
    assert_eq!(run.commits.len(), 1);
    assert_eq!(run.external_decisions().points().len(), 1);
    let external = &run.external_decisions().points()[0];
    let baseline = &run.decisions().records[0];
    assert_eq!(external.input(), baseline.input);
    assert_eq!(external.action(), baseline.outcome.into());
    assert_eq!(external.legal_candidates().len(), baseline.considered.len());
    assert_eq!(external.rule(), run.pass_manifests[0].decisions()[0].rule());
    assert_eq!(
        run.identity_bundle.decision_log(),
        Some(run.decisions().identity)
    );
    assert_ne!(
        run.external_decisions().identity(),
        run.decisions().identity
    );
    assert_eq!(
        ExternalDecisionLog::decode(&run.external_decisions().encode()),
        Ok(run.external_decisions().clone())
    );
}

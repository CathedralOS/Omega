//! Cross-block, phi-translated, and proof-certified value-numbering fixed points.

use super::super::super::*;

#[test]
fn named_global_value_numbering_reaches_a_cross_block_ledger_fixed_point() {
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(diamond_dominator_gvn_unit(), &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(usage.iterations, 3);
    assert_eq!(usage.rule_evaluations, 22);
    assert_eq!(usage.candidates, 2);
    assert_eq!(usage.validation_steps, 2);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 16);
    assert_eq!(ledger.records().len(), 2);
    assert_eq!(ledger.records()[0].provenance.len(), 5);
    assert_eq!(ledger.records()[1].provenance.len(), 4);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
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
    assert_eq!(usage.rule_evaluations, 21);
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 16);
    assert_eq!(ledger.records().len(), 1);
    let join = &output.functions[0].blocks[0];
    assert_eq!(join.parameters.len(), 2);
    assert_eq!(join.nodes.len(), 1);

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert_eq!(second_usage.rule_evaluations, 16);
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
    assert_eq!(usage.rule_evaluations, 22);
    assert_eq!(usage.candidates, 1);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 16);
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
    assert_eq!(second_usage.rule_evaluations, 16);
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
    assert_eq!(manifest.ordered_rules().len(), 16);
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
    assert_eq!(manifest.unwrap().ordered_rules().len(), 16);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(output.accepted_obligation_facts, accepted_catalog);
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 3);

    let (second, commits, usage, _, _, ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second, output);
    assert!(commits.is_empty());
    assert_eq!(usage.rule_evaluations, 16);
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
    assert_eq!(usage.rule_evaluations, 25);
    assert_eq!(usage.validation_steps, 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 16);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(output.accepted_obligation_facts, accepted_catalog);
    assert_eq!(output.functions[0].blocks[0].parameters.len(), 2);

    let (second, commits, usage, _, _, ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second, output);
    assert!(commits.is_empty());
    assert_eq!(usage.rule_evaluations, 16);
    assert!(ledger.records().is_empty());
}

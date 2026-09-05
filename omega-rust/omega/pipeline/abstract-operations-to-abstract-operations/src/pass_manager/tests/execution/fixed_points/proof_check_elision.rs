//! Evidence-preserving proof-check elision fixed points.

use super::super::super::*;

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
    assert_eq!(manifest.ordered_rules().len(), 12);
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
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap();
    let unit = live_exact_self_subtract_unit(integer);
    let accepted_fact = unit.accepted_obligation_facts[0].identity;
    let original_provenance = unit.functions[0].blocks[0].nodes[0].provenance.clone();
    let original_fuel = unit.functions[0].blocks[0].nodes[0].fuel.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(manifest.unwrap().ordered_rules().len(), 12);
    assert_eq!(
        commits[0].declaration.consumed_facts(),
        [OptimizationFactReference::AcceptedObligation(accepted_fact),]
    );
    assert!(matches!(
        output.functions[0].blocks[0].nodes[0].operation,
        AbstractOperation::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
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
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap();
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
    assert_eq!(manifest.ordered_rules().len(), 12);
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
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
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
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap();
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
    assert_eq!(manifest.ordered_rules().len(), 12);
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
            value: semantic_vocabulary::IntegerValue::Unsigned(1),
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
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap();
    let unit = live_remainder_by_one_unit(integer, SelfRemainderPolicy::Exact);
    let original_provenance = unit.functions[0].blocks[0].nodes[1].provenance.clone();
    let original_fuel = unit.functions[0].blocks[0].nodes[1].fuel.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 12);
    assert_eq!(
        manifest.decisions()[0].rule(),
        crate::LiveProofCertifiedIntegerRemainderByOneEliminationRule::contract().identity()
    );
    assert_eq!(commits[0].declaration.consumed_facts().len(), 2);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(0),
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
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 8).unwrap();
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
    assert_eq!(manifest.ordered_rules().len(), 12);
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
            value: semantic_vocabulary::IntegerValue::Signed(0),
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
fn named_proof_check_elision_elides_exact_signed_negative_one_shift_right() {
    let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 8).unwrap();
    let unit = live_exact_signed_negative_one_shift_right_unit(integer);
    let accepted_catalog = unit.accepted_obligation_facts.clone();
    let (output, commits, usage, _, manifest, ledger) =
        run_unit(unit, &registry, budget(8)).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(usage.iterations, 2);
    assert_eq!(ledger.records().len(), 1);
    let manifest = manifest.unwrap();
    assert_eq!(manifest.ordered_rules().len(), 12);
    assert_eq!(
        manifest.decisions()[0].rule(),
        crate::LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule::contract()
            .identity()
    );
    assert_eq!(commits[0].declaration.consumed_facts().len(), 2);
    assert_eq!(output.accepted_obligation_facts, accepted_catalog);
    assert_eq!(output.functions[0].blocks[0].nodes.len(), 2);
    assert!(matches!(
        output.functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::Return { value, .. } if value == semantic_vocabulary::ValueId::new(324).unwrap()
    ));

    let (second, second_commits, second_usage, _, _, second_ledger) =
        run_unit(output.clone(), &registry, budget(8)).unwrap();
    assert_eq!(second.identity, output.identity);
    assert!(second_commits.is_empty());
    assert_eq!(second_usage.iterations, 1);
    assert!(second_ledger.records().is_empty());
}

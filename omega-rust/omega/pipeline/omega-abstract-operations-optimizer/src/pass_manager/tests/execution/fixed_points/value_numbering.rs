//! Cross-block, phi-translated, and proof-certified value-numbering fixed points.

use omega_optimization_core::OptimizationValidatorIdentity;

use super::super::super::*;
use crate::rules::tests::compatible_policy_dominator_gvn_unit;
use crate::rules::{
    DominatorProofCertifiedCompatiblePolicyScalarGvnRule, DominatorProofCertifiedScalarGvnRule,
    DominatorTotalScalarGvnRule, PhiTranslatedObligationFreeScalarGvnRule,
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    PhiTranslatedProofCertifiedScalarGvnRule, SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
    SameBlockProofCertifiedScalarCseRule, SameBlockTotalScalarCseRule,
};

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

#[test]
fn scalar_cse_rules_are_runtime_disabled_deterministic_budgeted_and_idempotent() {
    let cases = vec![
        (
            local_cse_unit(),
            SameBlockTotalScalarCseRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.same-block-obligation-free-total-scalar-cse.v1",
            ),
            17,
            None,
        ),
        (
            proof_certified_local_cse_unit(),
            SameBlockProofCertifiedScalarCseRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.same-block-proof-certified-total-scalar-cse.v1",
            ),
            18,
            Some(1_309),
        ),
        (
            dominator_gvn_unit(),
            DominatorTotalScalarGvnRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.dominator-total-scalar-cse.v1",
            ),
            19,
            None,
        ),
        (
            proof_certified_dominator_gvn_unit(),
            DominatorProofCertifiedScalarGvnRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.dominator-proof-certified-total-scalar-gvn.v1",
            ),
            20,
            Some(1_351),
        ),
        (
            phi_translated_gvn_unit(),
            PhiTranslatedObligationFreeScalarGvnRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.phi-translated-obligation-free-total-scalar-gvn.v1",
            ),
            21,
            None,
        ),
        (
            proof_certified_phi_translated_gvn_unit(),
            PhiTranslatedProofCertifiedScalarGvnRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1",
            ),
            22,
            Some(1_713),
        ),
        (
            compatible_policy_local_cse_unit(),
            SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.same-block-proof-certified-compatible-policy-scalar-cse.v1",
            ),
            23,
            Some(1_309),
        ),
        (
            compatible_policy_dominator_gvn_unit(),
            DominatorProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
            ),
            24,
            Some(1_351),
        ),
        (
            compatible_policy_phi_translated_gvn_unit(),
            PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
            ),
            25,
            Some(1_713),
        ),
    ];
    let disabled = built_in_psi_registry(&OptimizationSelections::default()).unwrap();
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();

    for (unit, rule, validator, expected_evaluations, redundant_operation) in cases {
        let (unchanged, commits, usage, decisions, manifest, ledger) =
            run_unit(unit.clone(), &disabled, budget(8)).unwrap();
        assert_eq!(unchanged, unit);
        assert!(commits.is_empty());
        assert_eq!(usage.iterations, 1);
        assert_eq!(usage.rule_evaluations, 0);
        assert_eq!(usage.candidates, 0);
        assert_eq!(usage.validation_steps, 0);
        assert_eq!(usage.commits, 0);
        assert!(decisions.records.is_empty());
        assert!(manifest.is_none());
        assert!(ledger.records().is_empty());

        let first = run_unit(unit.clone(), &registry, budget(8)).unwrap();
        let second = run_unit(unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, second.1);
        assert_eq!(first.2, second.2);
        assert_eq!(first.3, second.3);
        assert_eq!(first.4, second.4);
        assert_eq!(first.5, second.5);

        let (output, commits, usage, _, manifest, ledger) = first;
        assert_eq!(usage.iterations, 2);
        assert_eq!(usage.rule_evaluations, expected_evaluations);
        assert_eq!(usage.candidates, 1);
        assert_eq!(usage.validation_steps, 1);
        assert_eq!(usage.commits, 1);
        let [commit] = commits.as_slice() else {
            panic!("one-rule fixture commits exactly once")
        };
        assert_eq!(commit.rule, rule);
        assert_eq!(commit.validator, validator);
        assert_eq!(commit.input, unit.identity);
        assert_eq!(commit.output, output.identity);
        assert_eq!(commit.predicted_cost_delta, -1);

        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 16);
        let [decision] = manifest.decisions() else {
            panic!("one-rule fixture records exactly one decision")
        };
        assert_eq!(decision.rule(), rule);
        assert_eq!(decision.validator(), Some(validator));
        let expected_facts = redundant_operation
            .map(|operation| {
                let operation = psi_core::OperationId::new(operation).unwrap();
                let fact = unit
                    .accepted_obligation_facts
                    .iter()
                    .find(|fact| fact.operation == operation)
                    .unwrap()
                    .identity;
                vec![OptimizationFactReference::AcceptedObligation(fact)]
            })
            .unwrap_or_default();
        assert_eq!(decision.consumed_facts(), expected_facts);

        let [record] = ledger.records() else {
            panic!("one-rule fixture publishes exactly one ledger record")
        };
        assert_eq!(record.rule, commit.rule);
        assert_eq!(record.candidate, commit.candidate);
        assert_eq!(record.validator, commit.validator);
        assert_eq!(record.input, commit.input);
        assert_eq!(record.output, commit.output);
        assert_eq!(record.provenance, commit.provenance);
        assert_eq!(ledger.input(), unit.identity);
        assert_eq!(ledger.output(), output.identity);
        assert_eq!(
            output.accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        if let Some(operation) = redundant_operation {
            let operation = psi_core::OperationId::new(operation).unwrap();
            assert!(output.functions[0].facts.iter().all(|fact| {
                !matches!(fact, omega_optimization_unit::OptimizationFact::OperationObligationReference { support, .. }
                    if *support == operation)
            }));
        }

        let (fixed, commits, usage, decisions, manifest, ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(fixed, output);
        assert!(commits.is_empty());
        assert_eq!(usage.iterations, 1);
        assert_eq!(usage.rule_evaluations, 16);
        assert_eq!(usage.candidates, 0);
        assert_eq!(usage.validation_steps, 0);
        assert_eq!(usage.commits, 0);
        assert!(decisions.records.is_empty());
        assert_eq!(manifest.unwrap().decisions(), []);
        assert!(ledger.records().is_empty());

        let first_error = run_unit(unit.clone(), &registry, budget(1)).unwrap_err();
        let second_error = run_unit(unit, &registry, budget(1)).unwrap_err();
        assert_eq!(first_error, second_error);
        assert_eq!(
            first_error,
            OptimizationRunError::WorkBudgetExhausted("iterations")
        );
    }
}

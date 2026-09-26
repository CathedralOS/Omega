//! Matched machine law guarantees.

use crate::validation::proof_contracts::contract_entailment::law_conformance::check_law_conformance_with_matches;
use crate::validation::proof_contracts::contract_entailment::law_conformance::operator_contracts::is_equality_conjunction;
use crate::validation::proof_contracts::contract_entailment::law_conformance::proposition_laws::collect_equality_conjuncts;
use crate::validation::proof_contracts::contract_entailment::{
    ExpressionHandle, ProofFact, SignatureContractKind, SymbolHandle, TypedTrees,
};

/// LAW-CONFORMANCE (rearrange rung B, settle 2026-07-18): a trait requirement
/// carrying `ensures` is a LAW -- an obligation every satisfier proves. The
/// satisfier machine must carry a PROVEN ensures conjunct matching the
/// declared law forall-to-forall: the requirement's parameters are pattern
/// variables that must bind to DISTINCT parameters of the satisfier (a weaker
/// instance -- `add(x, x) == add(x, x)` against `add(a, b) == add(b, a)` --
/// does not license the law), and the law's op-slot applications (`add`,
/// `mul` -- the trait's own requirement names) resolve to the CARRIER's bound
/// machines first. This is the N3 shape-match machinery promoted from
/// suggestion-only to load-bearing.
/// Exact correspondence, not a proof: every listed authored source expression
/// must independently have a positive entailment result before its matching
/// inherited law expression can be consumed.
pub struct MatchedLawGuarantee {
    pub machine: SymbolHandle,
    pub requirement: SymbolHandle,
    pub expression: ExpressionHandle,
    pub source_expressions: Vec<ExpressionHandle>,
}

pub fn matched_machine_law_guarantees(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> Vec<MatchedLawGuarantee> {
    let Some(machine) = program.machines().iter().find(|machine| {
        machine.symbol == machine_symbol
            && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
    }) else {
        return Vec::new();
    };
    let mut outcomes = Vec::new();
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == conformance.symbol)
        else {
            continue;
        };
        let Some(requirement) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|requirement| {
                requirement.symbol.is_valid()
                    && requirement.symbol == conformance.requirement_symbol
                    && conformance.requirement.as_ref() == Some(&requirement.name)
            })
        else {
            continue;
        };
        let mut diagnostics = Vec::new();
        let mut matches = Vec::new();
        check_law_conformance_with_matches(
            program,
            machine,
            conformance.alias.as_ref().map(|alias| alias.as_str()),
            trait_definition,
            requirement,
            program
                .type_reference_table
                .type_reference_handles(conformance.arguments),
            &mut diagnostics,
            &mut matches,
        );
        if !diagnostics.is_empty() {
            continue;
        }
        for contract in program
            .state_signature_contracts(requirement)
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(expression) = fact else {
                    continue;
                };
                if !is_equality_conjunction(program, *expression) {
                    continue;
                }
                let mut conjuncts = Vec::new();
                collect_equality_conjuncts(program, *expression, &mut conjuncts);
                let Some(source_expressions) = conjuncts
                    .iter()
                    .map(|conjunct| {
                        matches
                            .iter()
                            .find_map(|(law, source)| (law == conjunct).then_some(*source))
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                if !source_expressions.is_empty() {
                    outcomes.push(MatchedLawGuarantee {
                        machine: machine_symbol,
                        requirement: requirement.symbol,
                        expression: *expression,
                        source_expressions,
                    });
                }
            }
        }
    }
    outcomes
}

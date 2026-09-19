//! Rejection of refuted value-call requirements.

use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::structural_term;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

/// Reject a value-position call when one of the callee's equality-style
/// `requires` facts is structurally FALSE at the concrete operands. This is
/// the fail-safe first rung of general value-call precondition discharge: a
/// proven contradiction is an error, while an unproved/unknown fact remains for
/// the later fail-closed obligation rung instead of becoming a false positive.
pub(crate) fn reject_refuted_value_call_requires(
    program: &TypedTrees,
    caller_machine: &Machine,
    caller_state: &typed_trees::state::State,
    callee_machine: &Machine,
    callee_state: &typed_trees::state::State,
    arguments: &[ExpressionHandle],
    self_is_argument: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program
        .state_parameters(callee_state)
        .iter()
        .filter(|parameter| self_is_argument || !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return;
    }

    let mut map = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let Some(term) = structural_term(program, *argument) else {
            return;
        };
        map.push((parameter.name.as_str().to_owned(), term));
    }

    let mut caller_requires = Vec::new();
    for contract in program.machine_contracts(caller_machine) {
        if contract.kind == SignatureContractKind::Requires {
            caller_requires.extend(
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .filter_map(|fact| match fact {
                        ProofFact::Expression(expression) => Some(*expression),
                        ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
                    }),
            );
        }
    }
    for contract in program.state_contracts(caller_state) {
        if contract.kind == SignatureContractKind::Requires {
            caller_requires.extend(
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .filter_map(|fact| match fact {
                        ProofFact::Expression(expression) => Some(*expression),
                        ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
                    }),
            );
        }
    }
    let judge = StructuralJudge::from_requires(program, caller_machine, &caller_requires);
    if judge.hypotheses_contradictory {
        return;
    }

    for contract in program
        .machine_contracts(callee_machine)
        .iter()
        .chain(program.state_contracts(callee_state))
    {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if matches!(
                instantiated_fact_judgment(program, &judge, *expression, &map),
                StructuralJudgment::Refuted
            ) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` value call to `{}` violates required fact `{}`: the instantiated fact is structurally false",
                    caller_machine.name,
                    caller_state.name,
                    callee_machine.name,
                    program.expression_table.display_name(*expression),
                )));
            }
        }
    }
}

pub(super) fn instantiated_fact_judgment(
    program: &TypedTrees,
    judge: &StructuralJudge,
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
) -> StructuralJudgment {
    if super::structural_terms::is_case_observation(program, fact) {
        return StructuralJudgment::Unknown;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return StructuralJudgment::Unknown;
    };
    match binary.operator {
        BinaryOperator::And => {
            match (
                instantiated_fact_judgment(program, judge, binary.left, map),
                instantiated_fact_judgment(program, judge, binary.right, map),
            ) {
                (StructuralJudgment::Refuted, _) | (_, StructuralJudgment::Refuted) => {
                    StructuralJudgment::Refuted
                }
                (StructuralJudgment::Proven, StructuralJudgment::Proven) => {
                    StructuralJudgment::Proven
                }
                _ => StructuralJudgment::Unknown,
            }
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            let (Some(left), Some(right)) = (
                structural_term(program, binary.left),
                structural_term(program, binary.right),
            ) else {
                return StructuralJudgment::Unknown;
            };
            let equality = judge.judge_equation(
                judge.resolve(StructuralJudge::substitute_term(&left, map)),
                judge.resolve(StructuralJudge::substitute_term(&right, map)),
                0,
            );
            if binary.operator == BinaryOperator::Equal {
                equality
            } else {
                match equality {
                    StructuralJudgment::Proven => StructuralJudgment::Refuted,
                    StructuralJudgment::Refuted => StructuralJudgment::Proven,
                    StructuralJudgment::Unknown => StructuralJudgment::Unknown,
                }
            }
        }
        _ => StructuralJudgment::Unknown,
    }
}

/// Walk an ensures fact's `&&`-conjuncts; each `==` conjunct whose sides
/// term-ify yields one instantiated equation under the citation's
/// parameter map.
pub(crate) fn collect_instantiated_conjuncts(
    program: &TypedTrees,
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
    equations: &mut Vec<(StructuralTerm, StructuralTerm)>,
) {
    if super::structural_terms::is_case_observation(program, fact) {
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return;
    };
    match binary.operator {
        BinaryOperator::And => {
            collect_instantiated_conjuncts(program, binary.left, map, equations);
            collect_instantiated_conjuncts(program, binary.right, map, equations);
        }
        BinaryOperator::Equal => {
            let (Some(left), Some(right)) = (
                structural_term(program, binary.left),
                structural_term(program, binary.right),
            ) else {
                return;
            };
            equations.push((
                StructuralJudge::substitute_term(&left, map),
                StructuralJudge::substitute_term(&right, map),
            ));
        }
        _ => {}
    }
}

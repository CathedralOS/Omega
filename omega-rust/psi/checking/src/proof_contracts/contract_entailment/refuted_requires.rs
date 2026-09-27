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

    let callee_name = authored_callee_name(program, callee_machine);
    let authored = authored_requires_spellings(program, callee_machine, callee_state);
    for (position, expression) in requires_expressions(program, callee_machine, callee_state)
        .into_iter()
        .enumerate()
    {
        if !matches!(
            instantiated_fact_judgment(
                program,
                &judge,
                program.state_parameters(callee_state),
                expression,
                &map,
            ),
            StructuralJudgment::Refuted
        ) {
            continue;
        }
        let instantiated = program.expression_table.display_name(expression);
        let clause = authored
            .as_ref()
            .and_then(|spellings| spellings.get(position))
            .filter(|authored| **authored != instantiated)
            .map(|authored| {
                format!(
                    "`{authored}`: the fact instantiates here as `{instantiated}`, which is \
                     structurally false"
                )
            })
            .unwrap_or_else(|| {
                format!("`{instantiated}`: the instantiated fact is structurally false")
            });
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` value call to `{callee_name}` violates required fact {clause}",
            caller_machine.name, caller_state.name,
        )));
    }
}

/// The callee's `requires` expression facts in declaration order: machine
/// contracts before state contracts, each contract's facts in place. Order is
/// the only thing that pairs a specialized copy's clause with the one its
/// author wrote, so every reader of that pairing walks the chain this way.
fn requires_expressions(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
) -> Vec<ExpressionHandle> {
    program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
        })
        .collect()
}

/// The name the callee's author wrote. Specialization mints a machine whose
/// name carries its selection fingerprint -- `Indexing::index$specialized$…`
/// -- and that spelling is the compiler's, not a reader's.
fn authored_callee_name(program: &TypedTrees, callee_machine: &Machine) -> String {
    let authored = program.symbols.authored_origin(callee_machine.symbol);
    if authored == callee_machine.symbol {
        return callee_machine.name.as_str().to_owned();
    }
    program.symbols.name(authored).to_owned()
}

/// The authored spelling of each `requires` fact the callee carries, or
/// `None` when the callee is not a specialization or its source declaration
/// is no longer reachable. Specialization substitutes const arguments into
/// the cloned contract, so `Count == 0` reaches a reader as `4 == 0` and
/// names no clause they can find. The clone preserves contract and fact
/// order; a differing count means that assumption does not hold here, and the
/// instantiated fact stands alone rather than risk naming the wrong clause.
fn authored_requires_spellings(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &typed_trees::state::State,
) -> Option<Vec<String>> {
    let machine_symbol = program.symbols.authored_origin(callee_machine.symbol);
    if machine_symbol == callee_machine.symbol {
        return None;
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    // A cloned state's derivation origin is the machine that owns it, not the
    // state it copies, so the state pairs by the name the clone preserves.
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name == callee_state.name)?;
    let spellings: Vec<String> = requires_expressions(program, machine, state)
        .into_iter()
        .map(|expression| program.expression_table.display_name(expression))
        .collect();
    (spellings.len() == requires_expressions(program, callee_machine, callee_state).len())
        .then_some(spellings)
}

pub(super) fn instantiated_fact_judgment(
    program: &TypedTrees,
    judge: &StructuralJudge,
    callee_parameters: &[typed_trees::signature::StateParameter],
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
) -> StructuralJudgment {
    if super::structural_terms::is_case_observation(program, fact) {
        return judge.instantiated_case_judgment(callee_parameters, fact, map);
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
        return StructuralJudgment::Unknown;
    };
    match binary.operator {
        BinaryOperator::And => {
            match (
                instantiated_fact_judgment(program, judge, callee_parameters, binary.left, map),
                instantiated_fact_judgment(program, judge, callee_parameters, binary.right, map),
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

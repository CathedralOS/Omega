//! Rejection of refuted value-call requirements.

use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::{
    structural_term, structural_term_in_scope,
};
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
    let Some(map) = argument_term_map(
        program,
        caller_machine,
        caller_state,
        program.state_parameters(callee_state),
        arguments,
        self_is_argument,
    ) else {
        return;
    };

    let judge = caller_requires_judge(program, caller_machine, caller_state);
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
                Some((caller_machine, caller_state)),
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

/// Whether one callee `requires` expression is structurally TRUE at the
/// concrete call operands — the positive read of the same instantiated
/// judgment the refutation gate uses. An untermifiable argument, symbolic
/// operand, or contradictory caller hypothesis leaves the fact for the
/// bound, guard, and entailment channels; this only discharges what the
/// terms themselves decide.
pub fn instantiated_value_call_requires_proven(
    program: &TypedTrees,
    caller_machine: &Machine,
    caller_state: &typed_trees::state::State,
    callee_parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
    self_is_argument: bool,
) -> bool {
    let Some(map) = argument_term_map(
        program,
        caller_machine,
        caller_state,
        callee_parameters,
        arguments,
        self_is_argument,
    ) else {
        return false;
    };
    let judge = caller_requires_judge(program, caller_machine, caller_state);
    if judge.hypotheses_contradictory {
        return false;
    }
    matches!(
        instantiated_fact_judgment(
            program,
            &judge,
            callee_parameters,
            expression,
            &map,
            Some((caller_machine, caller_state)),
        ),
        StructuralJudgment::Proven
    )
}

/// One term per substituted parameter, each read from the call's argument in
/// the caller's scope. A single untermifiable argument leaves no map to judge
/// against, so the whole call declines rather than judge a partial picture.
fn argument_term_map(
    program: &TypedTrees,
    caller_machine: &Machine,
    caller_state: &typed_trees::state::State,
    callee_parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    self_is_argument: bool,
) -> Option<Vec<(String, StructuralTerm)>> {
    let parameters = callee_parameters
        .iter()
        .filter(|parameter| self_is_argument || !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }

    let mut map = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let scope = Some((caller_machine, caller_state, parameter.type_reference));
        let term = structural_term_in_scope(program, scope, *argument)?;
        map.push((parameter.name.as_str().to_owned(), term));
    }
    Some(map)
}

/// The hypothesis judge built from the caller's own machine- and state-level
/// `requires` facts — the same premise environment both sides of the
/// instantiated judgment read.
fn caller_requires_judge<'program>(
    program: &'program TypedTrees,
    caller_machine: &'program Machine,
    caller_state: &'program typed_trees::state::State,
) -> StructuralJudge<'program> {
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
    StructuralJudge::from_requires(program, caller_machine, &caller_requires)
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
    caller_scope: Option<(&Machine, &typed_trees::state::State)>,
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
                instantiated_fact_judgment(
                    program,
                    judge,
                    callee_parameters,
                    binary.left,
                    map,
                    caller_scope,
                ),
                instantiated_fact_judgment(
                    program,
                    judge,
                    callee_parameters,
                    binary.right,
                    map,
                    caller_scope,
                ),
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
            let Some((left, right)) =
                instantiated_operands(program, judge, binary, map, caller_scope)
            else {
                return StructuralJudgment::Unknown;
            };
            let equality = judge.judge_equation(left, right, 0);
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
        BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            // Ordering decides only over closed values: an operand that stays
            // symbolic leaves the fact for the positive prover's bound, guard,
            // and hypothesis channels.
            let Some((left, right)) =
                instantiated_operands(program, judge, binary, map, caller_scope)
            else {
                return StructuralJudgment::Unknown;
            };
            let verdict = match (&left, &right) {
                (StructuralTerm::Integer(left), StructuralTerm::Integer(right)) => {
                    match binary.operator {
                        BinaryOperator::Less => left < right,
                        BinaryOperator::LessOrEqual => left <= right,
                        BinaryOperator::Greater => left > right,
                        BinaryOperator::GreaterOrEqual => left >= right,
                        _ => unreachable!(),
                    }
                }
                // A float literal's exact decimal value widens losslessly to
                // binary64, so ordering is IEEE-exact at f64 regardless of the
                // authored format. UndeterminedNaN never termifies; a
                // synthesized NaN spelling declines rather than decide.
                (StructuralTerm::Float(left), StructuralTerm::Float(right)) => {
                    let (left, right) = (left.value_f64(), right.value_f64());
                    if left.is_nan() || right.is_nan() {
                        return StructuralJudgment::Unknown;
                    }
                    match binary.operator {
                        BinaryOperator::Less => left < right,
                        BinaryOperator::LessOrEqual => left <= right,
                        BinaryOperator::Greater => left > right,
                        BinaryOperator::GreaterOrEqual => left >= right,
                        _ => unreachable!(),
                    }
                }
                _ => return StructuralJudgment::Unknown,
            };
            if verdict {
                StructuralJudgment::Proven
            } else {
                StructuralJudgment::Refuted
            }
        }
        _ => StructuralJudgment::Unknown,
    }
}

/// Both sides of a binary fact, substituted with the call's argument terms and
/// normalized through the judge: applications resolve under the judge's
/// machine bodies, constant applications unfold to their closed constructor.
fn instantiated_operands(
    program: &TypedTrees,
    judge: &StructuralJudge,
    binary: &typed_trees::expression::TableBinaryExpression,
    map: &[(String, StructuralTerm)],
    caller_scope: Option<(&Machine, &typed_trees::state::State)>,
) -> Option<(StructuralTerm, StructuralTerm)> {
    let left = structural_term(program, binary.left)?;
    let right = structural_term(program, binary.right)?;
    Some((
        declared_len_extent_term(
            program,
            caller_scope,
            super::structural_terms::unfold_constant_applications(
                program,
                judge.resolve(StructuralJudge::substitute_term(&left, map)),
            ),
        ),
        declared_len_extent_term(
            program,
            caller_scope,
            super::structural_terms::unfold_constant_applications(
                program,
                judge.resolve(StructuralJudge::substitute_term(&right, map)),
            ),
        ),
    ))
}

/// A `.len` projection over a rooted member path reads a closed value when
/// the root's declared storage is statically countable: after substitution a
/// callee's `buffer.len` may name the caller's `[u8; 64]` field, and the same
/// holds for a `main.nodes.len` spelled on a self receiver the judgment sees
/// directly. Anything else — a runtime-length slice, a non-`.len` tail, or an
/// unrooted subject — keeps its symbolic form.
fn declared_len_extent_term(
    program: &TypedTrees,
    caller_scope: Option<(&Machine, &typed_trees::state::State)>,
    term: StructuralTerm,
) -> StructuralTerm {
    let Some((machine, state)) = caller_scope else {
        return term;
    };
    let mut names = Vec::new();
    match &term {
        StructuralTerm::Projection { subject, path } => {
            if language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
                path.as_str(),
            ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
            {
                return term;
            }
            let mut current = subject.as_ref();
            loop {
                match current {
                    StructuralTerm::Variable(name) => {
                        names.insert(0, name.clone());
                        break;
                    }
                    // An Opaque carries a display spelling: a caller-side
                    // multi-member place (`main.control`) joins its path with
                    // `.`, so split it back into members to recover the place.
                    StructuralTerm::Opaque(name) => {
                        for (index, member) in name.split('.').enumerate() {
                            names.insert(index, member.to_string());
                        }
                        break;
                    }
                    StructuralTerm::Projection { subject, path } => {
                        names.insert(0, path.clone());
                        current = subject.as_ref();
                    }
                    _ => return term,
                }
            }
        }
        // A `.len`-tailed opaque spelling is the same place the projection
        // arm names: `main.nodes.len` arrives whole when ordinary field
        // members keep the citation vocabulary's opaque term.
        StructuralTerm::Opaque(name) => {
            let mut members: Vec<String> =
                name.split('.').map(|member| member.to_string()).collect();
            if members
                .last()
                .is_none_or(|last| {
                    language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
                        last.as_str(),
                    ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
                })
            {
                return term;
            }
            members.pop();
            names = members;
        }
        _ => return term,
    }
    let Some(declared) = crate::value_custody::places::declared_member_path_type(
        program,
        machine,
        Some(state),
        &names,
    )
    .and_then(|declared| crate::value_custody::places::unwrapped_type_reference(program, declared)) else {
        return term;
    };
    let typed_trees::types::TypeReferenceNode::FixedArray {
        length: typed_trees::types::FixedArrayLength::Literal(extent),
        ..
    } = program.type_reference_table.type_reference(declared)
    else {
        return term;
    };
    StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(*extent as u64))
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

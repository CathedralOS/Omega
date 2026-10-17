//! Rejection of refuted value-call requirements.

use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::structural_term;
use diagnostics::Diagnostic;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

/// Reject a value-position call when one of the callee's equality-style
/// `requires` facts is structurally FALSE at the concrete operands. This is
/// the fail-safe first rung of general value-call precondition discharge: a
/// proven contradiction is an error, while an unproved/unknown fact remains for
/// the later fail-closed obligation rung instead of becoming a false positive.
pub(crate) fn reject_refuted_value_call_requires(
    program: &dyn crate::CheckingInput,
    caller_machine: &Machine,
    caller_state: &typed_trees::state::State,
    callee_machine: &Machine,
    callee_state: &typed_trees::state::State,
    arguments: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(map) = argument_term_map(
        program,
        caller_machine,
        caller_state,
        program.state_parameters(callee_state),
        arguments,
    ) else {
        return;
    };

    // This judgment has no executed-prefix custody. Entry preconditions may
    // already have been invalidated by a store or an earlier call. Only the
    // prefix-aware route may use them; this route decides closed operands.
    let judge = StructuralJudge::from_runtime_requires(program, caller_machine, &[]);
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
                callee_state.symbol,
                program.state_parameters(callee_state),
                expression,
                &map,
                Some((caller_machine, caller_state)),
            ),
            StructuralJudgment::Refuted
        ) {
            continue;
        }
        // The authored clause names what the reader can find in the callee;
        // the instantiated spelling names the values this call compared.
        let authored = authored
            .as_ref()
            .and_then(|spellings| spellings.get(position).cloned())
            .unwrap_or_else(|| program.expression_table.display_name(expression));
        let instantiated = instantiated_fact_spelling(
            program,
            &judge,
            callee_state.symbol,
            program.state_parameters(callee_state),
            expression,
            &map,
            Some((caller_machine, caller_state)),
        )
        .filter(|instantiated| *instantiated != authored)
        .map(|instantiated| format!(" (`{instantiated}`)"))
        .unwrap_or_default();
        let clause =
            format!("`{authored}`: the instantiated fact is structurally false{instantiated}");
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
    program: &dyn crate::CheckingInput,
    caller_machine: &Machine,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    site: crate::machine_calls::calls::CallSite<'_>,
    callee_owner: symbols::SymbolHandle,
    callee_parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    if crate::guarded_call_requires_proven(
        program,
        caller_machine,
        caller_state,
        statement_index,
        site,
        callee_owner,
        callee_parameters,
        arguments,
        expression,
    ) {
        return true;
    }
    if super::call_requirements::selected_value_call_requirement_entailed(
        program,
        caller_machine,
        caller_state,
        statement_index,
        site,
        expression,
    ) {
        return true;
    }
    let Some(map) = argument_term_map(
        program,
        caller_machine,
        caller_state,
        callee_parameters,
        arguments,
    ) else {
        return false;
    };
    // This judgment has no executed-prefix custody. Entry preconditions may
    // already have been invalidated by a store or an earlier call. Only the
    // prefix-aware route may use them; this route decides closed operands.
    let judge = StructuralJudge::from_runtime_requires(program, caller_machine, &[]);
    if judge.hypotheses_contradictory {
        return false;
    }
    matches!(
        instantiated_fact_judgment(
            program,
            &judge,
            callee_owner,
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
///
/// An argument term describes a concrete operand, so it must identify that
/// operand's value. A call that is not a repeatable value-only normal return
/// produces a fresh observation per invocation, while its term is built from
/// the callee and the argument terms alone: two identical spellings would
/// compare equal and the judge would decide an equality between two distinct
/// runtime values. Such an argument is not termifiable for instantiation and
/// declines the whole map, in both the refutation and the proof direction.
fn argument_term_map(
    program: &dyn crate::CheckingInput,
    caller_machine: &Machine,
    caller_state: &typed_trees::state::State,
    callee_parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
) -> Option<Vec<(String, StructuralTerm)>> {
    let parameters = callee_parameters.iter().collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }

    let plans = crate::machine_calls::calls::operand_call_plans(program, None);
    let mut map = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if !crate::machine_calls::denotational_calls::expression_values_have_call_identity(
            program,
            *argument,
            &plans.0,
            &plans.1,
            &|_| false,
        ) {
            return None;
        }
        let scope = Some((caller_machine, caller_state, parameter.type_reference));
        let term = structural_term(program, scope, None, *argument, None)?;
        map.push((parameter.name.as_str().to_owned(), term));
    }
    Some(map)
}

/// The callee's `requires` expression facts in declaration order: machine
/// contracts before state contracts, each contract's facts in place. Order is
/// the only thing that pairs a specialized copy's clause with the one its
/// author wrote, so every reader of that pairing walks the chain this way.
fn requires_expressions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
) -> Vec<ExpressionHandle> {
    crate::proof_contracts::proof_facts::contract_expression_facts(
        program,
        program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state)),
        SignatureContractKind::Requires,
    )
    .collect()
}

/// The name the callee's author wrote. Specialization mints a machine whose
/// name carries its selection fingerprint -- `Indexing::index$specialized$…`
/// -- and that spelling is the compiler's, not a reader's.
fn authored_callee_name(program: &dyn crate::CheckingInput, callee_machine: &Machine) -> String {
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
    program: &dyn crate::CheckingInput,
    callee_machine: &Machine,
    callee_state: &typed_trees::state::State,
) -> Option<Vec<String>> {
    let machine_symbol = program.symbols.authored_origin(callee_machine.symbol);
    if machine_symbol == callee_machine.symbol {
        return None;
    }
    let machine = program.machine(machine_symbol)?;
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
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge,
    callee_owner: symbols::SymbolHandle,
    callee_parameters: &[typed_trees::signature::StateParameter],
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
    caller_scope: Option<(&Machine, &typed_trees::state::State)>,
) -> StructuralJudgment {
    instantiated_selected_fact_judgment(
        program,
        judge,
        callee_owner,
        callee_parameters,
        fact,
        map,
        &[],
        caller_scope,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn instantiated_selected_fact_judgment(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge,
    callee_owner: symbols::SymbolHandle,
    callee_parameters: &[typed_trees::signature::StateParameter],
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
    machine_map: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
        typed_trees::expression::StaticMachineArgument,
    )],
    caller_scope: Option<(&Machine, &typed_trees::state::State)>,
) -> StructuralJudgment {
    if program
        .expression_table
        .call_node(fact)
        .is_some_and(|call| {
            crate::mathematical_applications::selected_definition(program, call).is_some()
        })
    {
        return judge.instantiated_mathematical_predicate(
            callee_owner,
            callee_parameters,
            fact,
            map,
            machine_map,
        );
    }
    if super::structural_terms::is_case_observation(program, fact) {
        return judge.instantiated_selected_case_judgment(
            callee_owner,
            callee_parameters,
            fact,
            map,
            machine_map,
        );
    }
    // A preserved runtime prefix can recognize the very same selected
    // Boolean predicate after captured-actual substitution. This is truth
    // transport, not an interval law or an equality on floating values.
    // Term construction owns selected builtin meaning. Reclassifying this as
    // fixed-width bound syntax would discard mathematical Int predicates over
    // those same captured actuals.
    if judge.has_runtime_body_values()
        && let Some(term) = super::structural_terms::structural_term(
            program,
            None,
            Some((callee_owner, callee_parameters)),
            fact,
            None,
        )
    {
        let term = StructuralJudge::substitute_call_term(&term, map, machine_map);
        match judge.judge_observed_predicate(term, 0) {
            StructuralJudgment::Proven => return StructuralJudgment::Proven,
            StructuralJudgment::Refuted => return StructuralJudgment::Refuted,
            StructuralJudgment::Unknown => {}
        }
    }
    let Some(binary) = program.expression_table.binary_node(fact) else {
        return StructuralJudgment::Unknown;
    };
    match binary.operator {
        BinaryOperator::And | BinaryOperator::Or => {
            // Conjunction has no authored operator selection. Each leaf
            // still establishes its own scalar or nominal predicate meaning.
            let operands = (
                instantiated_selected_fact_judgment(
                    program,
                    judge,
                    callee_owner,
                    callee_parameters,
                    binary.left,
                    map,
                    machine_map,
                    caller_scope,
                ),
                instantiated_selected_fact_judgment(
                    program,
                    judge,
                    callee_owner,
                    callee_parameters,
                    binary.right,
                    map,
                    machine_map,
                    caller_scope,
                ),
            );
            if binary.operator == BinaryOperator::Or {
                return match operands {
                    (StructuralJudgment::Proven, _) | (_, StructuralJudgment::Proven) => {
                        StructuralJudgment::Proven
                    }
                    (StructuralJudgment::Refuted, StructuralJudgment::Refuted) => {
                        StructuralJudgment::Refuted
                    }
                    _ => StructuralJudgment::Unknown,
                };
            }
            match operands {
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
            if !super::structural_terms::has_identity_equality_meaning(
                program,
                callee_owner,
                callee_parameters,
                fact,
            ) {
                return StructuralJudgment::Unknown;
            }
            let Some((left, right)) = instantiated_operands(
                program,
                judge,
                callee_owner,
                callee_parameters,
                binary,
                map,
                machine_map,
                caller_scope,
            ) else {
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
            let Some((left, right)) = instantiated_operands(
                program,
                judge,
                callee_owner,
                callee_parameters,
                binary,
                map,
                machine_map,
                caller_scope,
            ) else {
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
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge,
    callee_owner: symbols::SymbolHandle,
    callee_parameters: &[typed_trees::signature::StateParameter],
    binary: &typed_trees::expression::TableBinaryExpression,
    map: &[(String, StructuralTerm)],
    machine_map: &[(
        symbols::SymbolHandle,
        typed_trees::types::TypeReferenceHandle,
        typed_trees::expression::StaticMachineArgument,
    )],
    caller_scope: Option<(&Machine, &typed_trees::state::State)>,
) -> Option<(StructuralTerm, StructuralTerm)> {
    let (left, right) = super::structural_terms::structural_term_pair_for_parameters(
        program,
        callee_owner,
        callee_parameters,
        binary,
    )?;
    Some((
        declared_len_extent_term(
            program,
            caller_scope,
            super::structural_terms::unfold_constant_applications(
                program,
                judge.resolve(StructuralJudge::substitute_call_term(
                    &left,
                    map,
                    machine_map,
                )),
            ),
        ),
        declared_len_extent_term(
            program,
            caller_scope,
            super::structural_terms::unfold_constant_applications(
                program,
                judge.resolve(StructuralJudge::substitute_call_term(
                    &right,
                    map,
                    machine_map,
                )),
            ),
        ),
    ))
}

/// The instantiated fact as the values it compared, for the refutation
/// diagnostic: each comparison spells its two instantiated operands, and a
/// conjunction or disjunction joins its parts. Facts the judgment decides
/// another way (case observations) keep their authored spelling.
fn instantiated_fact_spelling(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge,
    callee_owner: symbols::SymbolHandle,
    callee_parameters: &[typed_trees::signature::StateParameter],
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
    caller_scope: Option<(&Machine, &typed_trees::state::State)>,
) -> Option<String> {
    let binary = program.expression_table.binary_node(fact)?;
    match binary.operator {
        BinaryOperator::And | BinaryOperator::Or => {
            let left = instantiated_fact_spelling(
                program,
                judge,
                callee_owner,
                callee_parameters,
                binary.left,
                map,
                caller_scope,
            )?;
            let right = instantiated_fact_spelling(
                program,
                judge,
                callee_owner,
                callee_parameters,
                binary.right,
                map,
                caller_scope,
            )?;
            Some(format!("{left} {} {right}", binary.operator.display_name()))
        }
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            let (left, right) = instantiated_operands(
                program,
                judge,
                callee_owner,
                callee_parameters,
                binary,
                map,
                &[],
                caller_scope,
            )?;
            Some(format!(
                "{} {} {}",
                super::structural_terms::term_spelling(program, &left),
                binary.operator.display_name(),
                super::structural_terms::term_spelling(program, &right)
            ))
        }
        _ => None,
    }
}

/// A `.len` projection over a rooted member path reads a closed value when
/// the root's declared storage is statically countable: after substitution a
/// callee's `buffer.len` may name the caller's `[u8; 64]` field, and the same
/// holds for a `main.nodes.len` spelled on a self receiver the judgment sees
/// directly. Anything else — a runtime-length slice, a non-`.len` tail, or an
/// unrooted subject — keeps its symbolic form.
fn declared_len_extent_term(
    program: &dyn crate::CheckingInput,
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
    let Some(extent) = crate::value_custody::places::declared_member_path_type(
        program,
        machine,
        Some(state),
        &names,
    )
    .and_then(|declared| crate::fixed_array_type_length(program, declared)) else {
        return term;
    };
    StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(extent as u64))
}

/// Walk an ensures fact's `&&`-conjuncts; each `==` conjunct whose sides
/// term-ify yields one instantiated equation under the citation's
/// parameter map.
pub(crate) fn collect_instantiated_conjuncts(
    program: &dyn crate::CheckingInput,
    scope: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    fact: ExpressionHandle,
    map: &[(String, StructuralTerm)],
    equations: &mut Vec<(StructuralTerm, StructuralTerm)>,
) {
    if super::structural_terms::is_case_observation(program, fact) {
        return;
    }
    let Some(binary) = program.expression_table.binary_node(fact) else {
        return;
    };
    match binary.operator {
        BinaryOperator::And => {
            if !crate::has_builtin_parameter_bound_expression_meaning(
                program, scope, parameters, fact,
            ) {
                return;
            }
            collect_instantiated_conjuncts(program, scope, parameters, binary.left, map, equations);
            collect_instantiated_conjuncts(
                program,
                scope,
                parameters,
                binary.right,
                map,
                equations,
            );
        }
        BinaryOperator::Equal => {
            let Some((left, right)) =
                super::structural_terms::identity_equality_terms(program, scope, parameters, fact)
            else {
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

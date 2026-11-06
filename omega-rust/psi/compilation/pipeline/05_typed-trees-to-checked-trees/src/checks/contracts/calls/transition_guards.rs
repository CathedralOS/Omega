//! Transition-guard requirement proofs: the selected transition call,
//! guard conjunct matching, and guarded-value preservation a call
//! requirement may read through.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::FactPayload;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

use crate::published::labels::semantic_fact_requirement_label;

/// Recover the premise of this selected edge once, for both complete facts
/// and their Boolean children. A computed transition subject is a captured
/// predicate, not merely its generated truth-test binding. Its reads must
/// survive both evaluation of the predicate and the arm's operands before any
/// consumer can use that premise; splitting a conjunction changes no epoch.
pub(in crate::checks::contracts) fn prepare_selected_guard(
    queries: &super::super::direct::GuardDenotations<'_>,
) -> Option<super::super::direct::SelectedGuardEvidence> {
    let program = queries.program;
    let state_flow = queries.state;
    let call_flow = queries.call;
    let call_frames = queries.call_frames;
    let (machine, caller_state, call_site, _target_parameters) =
        super::call_coordinates(program, state_flow, call_flow)?;
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statement_at(caller_state, call_flow.statement_index)
    else {
        return None;
    };
    let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        return None;
    };
    // The arm this guard selects, in either spelling. A bare named target is
    // one; so is `-> (callee(..))`, which `ac52bc4114` requires of an attached
    // machine and which a receiver-qualified call always uses. Only the
    // SELECTED arm's own target qualifies: a call inside the guard, nested
    // inside the target expression, or on the continuation arm -- which holds
    // the guard's negation -- is not this edge.
    let selects_this_call = match &call_site {
        validation::CallSite::TransitionNamed { .. } => {
            validation::transition_call_target(
                program,
                machine,
                caller_state,
                call_flow.statement_index,
                call_flow.call_ordinal,
            ) == Some(transition.target)
        }
        validation::CallSite::Expression { expression, .. } => {
            matches!(
                program.statement_table.transition_target(transition.target),
                typed_trees::statement::TransitionTargetNode::Value(value) if value == expression
            )
        }
        validation::CallSite::Statement(_) => false,
    };
    if !selects_this_call || !guard_preserves_values(machine, guard, call_frames) {
        return None;
    }
    // Both direct and saved predicates use the flow owner's exact backing-read
    // interval. Argument effects precede invocation; the callee's own future
    // writes cannot erase the evidence it needs at entry.
    let frames = call_frames?;
    let writes = queries.operand_writes()?;
    let prepared = crate::flow::prepare_guard_observations_after_writes(
        program,
        machine,
        caller_state,
        call_flow.statement_index,
        guard,
        true,
        writes,
        frames,
    )?;
    Some(super::super::direct::SelectedGuardEvidence {
        current: prepared
            .current
            .into_iter()
            .filter_map(|(expression, value)| value.then_some(expression))
            .collect(),
        captures: prepared.captures,
    })
}

fn selected_transition_premise<'program, 'query>(
    queries: &'query super::super::direct::GuardDenotations<'program>,
) -> Option<(
    validation::CallSite<'program>,
    &'program [typed_trees::signature::StateParameter],
    &'query [ExpressionHandle],
)> {
    let (_, _, call_site, parameters) =
        super::call_coordinates(queries.program, queries.state, queries.call)?;
    let prepared = queries.selected_guard()?;
    Some((call_site, parameters, &prepared.current))
}

/// Bind the actual selected edge for a conditional proof goal. Its reads
/// must survive the guard and every arm operand. Recovered subpredicates do
/// not replace this guard: their truth is conditional on this exact selection.
pub(in crate::checks::contracts) fn selected_guard_predicate(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    guard_queries: Option<&super::super::direct::GuardDenotations<'_>>,
) -> Option<crate::published::facts::boolean_predicates::BoundPredicate> {
    let owned_queries = if guard_queries.is_none() {
        super::super::direct::GuardDenotations::for_call(
            program,
            facts,
            state_flow,
            call_flow,
            call_frames,
        )
    } else {
        None
    };
    let queries = guard_queries.or(owned_queries.as_ref())?;
    let (_, _, guards) = selected_transition_premise(queries)?;
    let guard = *guards.first()?;
    let (machine, _, _, _) = super::call_coordinates(program, state_flow, call_flow)?;
    if !guard_preserves_values(machine, guard, call_frames)
        || !super::super::guard_operands::premise_reads_survive_arm_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            guard,
            call_frames,
        )
    {
        return None;
    }
    // This conditional goal names surviving current storage, not a new
    // completed read or an assertion of the guard. Bind the shared checked
    // algebra under the same declaration coordinates as the required formula.
    let expression = crate::published::facts::boolean_predicates::lower_current_guard(
        program,
        &facts.operators,
        state_flow.state_symbol,
        call_flow.statement_index,
        guard,
    )?;
    crate::published::facts::boolean_predicates::bind_current_expression(
        program,
        state_flow.state_symbol,
        expression,
        true,
        |operand| {
            crate::published::facts::boolean_predicates::declared_observation(
                program,
                state_flow.state_symbol,
                call_flow.statement_index,
                operand,
            )
        },
    )
}

/// Query one original Boolean conjunct through the same selected-arm and
/// operand-currentness gates as the complete contract fact.
pub(in crate::checks::contracts) fn transition_guard_proves_boolean_requirement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    queries: Option<&super::super::direct::GuardDenotations<'_>>,
) -> bool {
    let Some(queries) = queries else {
        return false;
    };
    let Some((call_site, target_parameters, guards)) = selected_transition_premise(queries) else {
        return false;
    };
    let required_label = super::super::labels::instantiate_call_contract_expression_label(
        program,
        state_flow.state_symbol,
        call_flow.statement_index,
        &call_site,
        target_parameters,
        expression,
    );
    (guards.iter().any(|guard| {
        guard_conjunct_matches(program, *guard, &required_label)
            || super::guard_observations::conjunct_matches(queries, *guard, expression)
            || super::super::call_bounds::proves(
                program,
                facts,
                state_flow,
                call_flow,
                &[],
                expression,
                Some(*guard),
            )
            || decomposed_guard_premise_proves(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                *guard,
                expression,
                &required_label,
                call_frames,
                queries,
            )
    }) || case_requirement_matches(program, state_flow, guards, &required_label))
        && super::super::guard_operands::requirement_reads_survive_earlier_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            validation::call_site_argument_expressions(program, &call_site),
            target_parameters,
            &super::super::guard_operands::boolean_requirement_mentions(program, expression),
            call_frames,
        )
}

/// A named transition is itself an incoming edge. Its taken-arm guard may
/// establish the target state's arrival requirement after positional
/// substitution (`value > 0` becomes `self.value > 0`). Ordinary call entry
/// contexts are statement-entry facts and therefore deliberately do not assume
/// that guard; discharge it explicitly for the transition target only, and
/// only while the arm's own operands, evaluated after the guard and before
/// the jump, leave the guard's storage unwritten (`guard_operands`).
pub(super) fn transition_guard_proves_requires(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fact: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    queries: Option<&super::super::direct::GuardDenotations<'_>>,
) -> bool {
    let Some(queries) = queries else {
        return false;
    };
    let Some((call_site, target_parameters, matching_guards)) =
        selected_transition_premise(queries)
    else {
        return false;
    };
    let matches_guard = |label: &str| {
        matching_guards
            .iter()
            .any(|guard| guard_conjunct_matches(program, *guard, label))
    };
    let required_label = match fact.payload {
        FactPayload::ContractBooleanExpression { expression, .. } => {
            super::super::labels::instantiate_call_contract_expression_label(
                program,
                state_flow.state_symbol,
                call_flow.statement_index,
                &call_site,
                target_parameters,
                expression,
            )
        }
        FactPayload::ContractDomainMembership { .. } | FactPayload::DomainMembership { .. } => {
            semantic_fact_requirement_label(program, &facts.semantic, fact)
        }
        _ => return false,
    };
    let copied_guard_establishes = match fact.payload {
        FactPayload::ContractBooleanExpression { expression, .. } => {
            matching_guards.iter().any(|matching_guard| {
                super::guard_observations::conjunct_matches(queries, *matching_guard, expression)
            })
        }
        _ => false,
    };
    let guard_establishes = matches_guard(&required_label)
        || copied_guard_establishes
        || case_requirement_matches(program, state_flow, matching_guards, &required_label)
        || super::predicate_domains::predicate_only_domain_labels(program, facts, fact).is_some_and(|labels| {
            labels
                .iter()
                .all(|label| matches_guard(label))
        })
        // The spellings need not agree when both state a closed interval:
        // `index < 16` establishes `requires self <= 15`. Comparing the
        // intervals rather than the text is what lets a bound move to a domain
        // without the guard being rewritten to match the predicate.
        || super::predicate_domains::predicate_only_domain_interval(program, facts, fact).is_some_and(
            |(subject_label, required_low, required_high)| {
                matching_guards.iter().any(|guard| {
                    super::super::intervals::guard_interval_for_label(
                        program,
                        *guard,
                        &subject_label,
                    )
                    .is_some_and(|interval| {
                        super::super::intervals::establishes_closed_requirement(
                            &interval,
                            &required_low,
                            &required_high,
                        )
                    })
                })
            },
        );
    if !guard_establishes {
        return false;
    }
    // The guard was read before the arm's operands ran. An operand evaluated
    // ahead of the requirement's read may have written the storage that read
    // names; the label match alone cannot see that, so the operand write
    // frames decide whether the quoted premise still describes the delivered
    // value.
    super::super::guard_operands::requirement_reads_survive_earlier_operand_writes(
        program,
        facts,
        state_flow,
        call_flow,
        validation::call_site_argument_expressions(program, &call_site),
        target_parameters,
        &super::super::guard_operands::fact_requirement_mentions(program, facts, fact),
        call_frames,
    )
}

/// A selected `subject in Data::Case` guard establishes that case's data
/// `requires` conjuncts. Membership is the guard; the scalar obligations are
/// the premises a callee `requires` may quote. Projection renames the
/// predicate subject onto the dispatched value and the data's value
/// parameters onto that value's type arguments, which is the vocabulary the
/// instantiated callee contract already uses.
fn case_requirement_matches(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    guards: &[typed_trees::expression::ExpressionHandle],
    required_label: &str,
) -> bool {
    let Some(machine) = program.machine(state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = program.state_in_machine(machine, state_flow.state_symbol) else {
        return false;
    };
    guards.iter().any(|guard| {
        projected_case_requirement_labels(program, machine, state, *guard)
            .iter()
            .any(|label| {
                label == required_label || label.split(" && ").any(|part| part == required_label)
            })
    })
}

fn projected_case_requirement_labels(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    guard: typed_trees::expression::ExpressionHandle,
) -> Vec<String> {
    let Some(comparison) = program.expression_table.binary_node(guard) else {
        return Vec::new();
    };
    let Some(case) = validation::applied_case_membership_variant(
        program,
        machine,
        Some(state),
        guard,
        comparison,
    ) else {
        return Vec::new();
    };
    let mut subject = comparison.left;
    for _ in 0..8 {
        match program.expression_table.expression(subject) {
            ExpressionNode::Borrow(borrow) => subject = borrow.target,
            ExpressionNode::Name(path) => {
                let initial = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .find_map(|statement| match statement {
                        StatementNode::LocalData(local) if local.symbol == path.symbol => {
                            Some(local.initial_value)
                        }
                        _ => None,
                    });
                let Some(initial) = initial else {
                    break;
                };
                subject = initial;
            }
            _ => break,
        }
    }
    let Some(reference) =
        validation::expression_result_type_reference(program, machine, state, subject)
    else {
        return Vec::new();
    };
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return Vec::new();
    };
    let Some(definition) = program.data_definition_by_symbol(*base_symbol) else {
        return Vec::new();
    };
    let parameters = program.declaration_type_parameters(*base_symbol);
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    let mut parameter_names = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
        if !matches!(
            parameter.kind,
            typed_trees::data::TypeParameterKind::Value { .. }
                | typed_trees::data::TypeParameterKind::Const { .. }
        ) {
            continue;
        }
        let name = match program.type_reference_table.type_reference(*argument) {
            typed_trees::types::TypeReferenceNode::ConstExpression(expression) => {
                program.expression_table.display_name(*expression)
            }
            _ => continue,
        };
        parameter_names.push((parameter.symbol, name));
    }
    let subject_name = program.expression_table.display_name(subject);
    let requirements = validation::case_predicate_expressions(program, definition, case);
    let rendered = requirements
        .iter()
        .map(|requirement| {
            render_projected_requirement(
                program,
                definition.predicate_subject_symbol,
                &subject_name,
                &parameter_names,
                *requirement,
            )
        })
        .collect::<Vec<_>>();
    if rendered.len() <= 1 {
        return rendered;
    }
    let mut labels = vec![rendered.join(" && ")];
    labels.extend(rendered);
    labels
}

fn render_projected_requirement(
    program: &dyn ::validation::CheckingInput,
    predicate_subject: symbols::SymbolHandle,
    subject_name: &str,
    parameters: &[(symbols::SymbolHandle, String)],
    expression: typed_trees::expression::ExpressionHandle,
) -> String {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            render_projected_requirement(
                program,
                predicate_subject,
                subject_name,
                parameters,
                binary.left,
            ),
            binary.operator.display_name(),
            render_projected_requirement(
                program,
                predicate_subject,
                subject_name,
                parameters,
                binary.right,
            ),
        ),
        ExpressionNode::Call(call) if !call.receiver.is_valid() => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    render_projected_requirement(
                        program,
                        predicate_subject,
                        subject_name,
                        parameters,
                        *argument,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({arguments})", call.target.as_str())
        }
        ExpressionNode::Name(path) => {
            if path.symbol == predicate_subject {
                return subject_name.to_string();
            }
            if let Some((_, name)) = parameters.iter().find(|(symbol, _)| *symbol == path.symbol) {
                return name.clone();
            }
            program.expression_table.display_name(expression)
        }
        ExpressionNode::Member(member) => {
            let receiver = match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path) if path.symbol == predicate_subject => {
                    subject_name.to_string()
                }
                _ => render_projected_requirement(
                    program,
                    predicate_subject,
                    subject_name,
                    parameters,
                    member.receiver,
                ),
            };
            format!("{receiver}.{}", member.member.as_str())
        }
        _ => program.expression_table.display_name(expression),
    }
}

// A later guard conjunct can overwrite an earlier premise. Operand write
// checks begin after the complete guard, so guard-derived proofs additionally
// require this complete, known-empty expression frame.
pub(super) fn guard_preserves_values<'program>(
    machine: &'program typed_trees::machine::Machine,
    guard: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
) -> bool {
    call_frames.is_some_and(|frames| {
        frames
            .expression_write_frame(machine, guard)
            .into_complete_paths()
            .is_some_and(|writes| writes.is_empty())
    })
}

pub(super) fn guard_conjunct_matches(
    program: &dyn ::validation::CheckingInput,
    guard: typed_trees::expression::ExpressionHandle,
    required_label: &str,
) -> bool {
    if program.expression_table.display_name(guard) == required_label {
        return true;
    }
    let Some(binary) = program.expression_table.binary_node(guard) else {
        return false;
    };
    match binary.operator {
        typed_trees::expression::BinaryOperator::And => {
            guard_conjunct_matches(program, binary.left, required_label)
                || guard_conjunct_matches(program, binary.right, required_label)
        }
        typed_trees::expression::BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            guard_conjunct_matches(program, binary.left, required_label)
        }
        _ => false,
    }
}

/// The negated-arm read of `guard_conjunct_matches`: whether `!guard` -- the
/// evidence a continuation edge carries for a comparison or `||` guard that
/// evaluated false -- spells `required_label`. `!(a || b)` keeps each refuted
/// operand as its own conjunct of evidence; `!(a && b)` refutes an
/// unspecified operand and contributes nothing. Comparisons match through the
/// label their complement operator spells (`!(v >= n)` reads `v < n`). The
/// caller owes the guard's builtin-operator gate separately.
pub(super) fn negated_guard_conjunct_matches(
    program: &dyn ::validation::CheckingInput,
    guard: typed_trees::expression::ExpressionHandle,
    required_label: &str,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(guard) else {
        return false;
    };
    match binary.operator {
        typed_trees::expression::BinaryOperator::Or => {
            negated_guard_conjunct_matches(program, binary.left, required_label)
                || negated_guard_conjunct_matches(program, binary.right, required_label)
        }
        typed_trees::expression::BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            negated_guard_conjunct_matches(program, binary.left, required_label)
        }
        _ => binary.operator.complement().is_some_and(|complement| {
            format!(
                "{} {} {}",
                program.expression_table.display_name(binary.left),
                complement.display_name(),
                program.expression_table.display_name(binary.right)
            ) == required_label
        }),
    }
}

/// A premise conjunct may be a defined Boolean name rather than a spelled
/// predicate. Bound-Boolean transport publishes `premise <-> X` as the clause
/// pair `premise || !X` and `!premise || X` in the state's semantic contexts;
/// under the selected edge's premise the refuted disjunct dies and the
/// survivor stands as premise evidence. The assembly `zf` scratch recovers
/// `destination == 0` this way once normalization threads the flag through a
/// generated parameter, where no statement-level capture exists.
#[allow(clippy::too_many_arguments)]
fn decomposed_guard_premise_proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    guard: ExpressionHandle,
    required: ExpressionHandle,
    required_label: &str,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    queries: &super::super::direct::GuardDenotations<'_>,
) -> bool {
    let Some((machine, state, _, _)) = super::call_coordinates(program, state_flow, call_flow)
    else {
        return false;
    };
    let Some(frames) = call_frames else {
        return false;
    };
    let Some(writes) = queries.operand_writes() else {
        return false;
    };
    let establishes = |premise: ExpressionHandle| {
        crate::flow::guard_observations_after_writes(
            program,
            machine,
            state,
            call_flow.statement_index,
            premise,
            true,
            writes,
            frames,
        )
        .is_some_and(|observations| observations.contains(&(premise, true)))
            && (guard_conjunct_matches(program, premise, required_label)
                || super::guard_observations::conjunct_matches(queries, premise, required))
    };
    let mut conjuncts = Vec::new();
    premise_conjuncts(program, guard, &mut conjuncts);
    // Only a bare name can carry a transported definition: the refuted
    // disjunct below must spell `!name` or `name == false`, so comparisons
    // and composite conjuncts never match and skip the context scan.
    conjuncts
        .iter()
        .filter(|conjunct| premise_names_defined_boolean(program, **conjunct))
        .any(|conjunct| {
            contexts.iter().any(|context| {
                facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(*context))
                    .facts()
                    .any(|fact| {
                        let clause = match fact.payload {
                            FactPayload::BooleanExpression(expression)
                            | FactPayload::ContractBooleanExpression { expression, .. } => {
                                expression
                            }
                            _ => return false,
                        };
                        let Some(binary) = program.expression_table.binary_node(clause) else {
                            return false;
                        };
                        binary.operator == typed_trees::expression::BinaryOperator::Or
                            && [(binary.left, binary.right), (binary.right, binary.left)]
                                .into_iter()
                                .any(|(refuted, surviving)| {
                                    expressions_are_logical_negations(program, refuted, *conjunct)
                                        && establishes(surviving)
                                })
                    })
            })
        })
}

/// Whether a premise conjunct names a carried Boolean binding — a bare name,
/// `!name`, or `name == <bool>` — the only spellings a transported
/// definition clause can refute.
fn premise_names_defined_boolean(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Unary(unary) => {
            unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
                && matches!(
                    program.expression_table.expression(unary.operand),
                    ExpressionNode::Name(_)
                )
        }
        ExpressionNode::Binary(binary) => {
            binary.operator == typed_trees::expression::BinaryOperator::Equal
                && matches!(
                    program.expression_table.expression(binary.left),
                    ExpressionNode::Name(_)
                )
                && matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(_)
                )
        }
        _ => false,
    }
}

/// The same conjunct decomposition `guard_conjunct_matches` applies: `&&`
/// splits and `x == true` unwraps to `x`.
fn premise_conjuncts(
    program: &dyn ::validation::CheckingInput,
    guard: ExpressionHandle,
    conjuncts: &mut Vec<ExpressionHandle>,
) {
    let Some(binary) = program.expression_table.binary_node(guard) else {
        conjuncts.push(guard);
        return;
    };
    match binary.operator {
        typed_trees::expression::BinaryOperator::And => {
            premise_conjuncts(program, binary.left, conjuncts);
            premise_conjuncts(program, binary.right, conjuncts);
        }
        typed_trees::expression::BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            premise_conjuncts(program, binary.left, conjuncts);
        }
        _ => conjuncts.push(guard),
    }
}

/// Whether `refuted` spells the logical negation of `guard` in either checked
/// shape — `!guard`, `guard == false`, or `false == guard` — and in either
/// direction. Spelling identity is the same comparison the conjunct matchers
/// already apply to premise labels.
fn expressions_are_logical_negations(
    program: &dyn ::validation::CheckingInput,
    refuted: ExpressionHandle,
    guard: ExpressionHandle,
) -> bool {
    let negated = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            Some(unary.operand)
        }
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Equal =>
        {
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(false)
            ) {
                Some(binary.left)
            } else {
                matches!(
                    program.expression_table.expression(binary.left),
                    ExpressionNode::Boolean(false)
                )
                .then_some(binary.right)
            }
        }
        _ => None,
    };
    let same = |left: ExpressionHandle, right: ExpressionHandle| {
        program.expression_table.display_name(left) == program.expression_table.display_name(right)
    };
    negated(refuted).is_some_and(|inner| same(inner, guard))
        || negated(guard).is_some_and(|inner| same(inner, refuted))
}

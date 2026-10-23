//! Transition-guard requirement proofs: the selected transition call,
//! guard conjunct matching, and guarded-value preservation a call
//! requirement may read through.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::FactPayload;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

use crate::labels::semantic_fact_requirement_label;

/// Recover the premise of this selected edge once, for both complete facts
/// and their Boolean children. A computed transition subject is a captured
/// predicate, not merely its generated truth-test binding. Its reads must
/// survive both evaluation of the predicate and the arm's operands before any
/// consumer can use that premise; splitting a conjunction changes no epoch.
fn selected_transition_premise<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
) -> Option<(
    validation::CallSite<'program>,
    Vec<typed_trees::expression::ExpressionHandle>,
)> {
    let call_site = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    )?;
    let (machine, caller_state) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    )?;
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
        validation::CallSite::TransitionNamed { .. } => true,
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
    // The flow owner authenticates exact reaching captures, polarity, and
    // effects through dispatch. Whole requirements and Boolean children must
    // consume that same observation set, not recover a generated-name capture
    // through an independent matcher. Arm operands execute after observation.
    let captured = crate::flow::saved_guard_observations(
        program,
        machine,
        caller_state,
        call_flow.statement_index,
        guard,
        true,
        &[],
        call_frames,
    );
    let premises = std::iter::once(guard)
        .chain(captured.into_iter().filter_map(|(premise, holds)| {
            (holds
                && super::super::guard_operands::premise_reads_survive_arm_operand_writes(
                    program,
                    facts,
                    state_flow,
                    call_flow,
                    premise,
                    call_frames,
                ))
            .then_some(premise)
        }))
        .collect();
    Some((call_site, premises))
}

/// Query one original Boolean conjunct through the same selected-arm and
/// operand-currentness gates as the complete contract fact.
pub(in crate::checks::contracts) fn transition_guard_proves_boolean_requirement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((call_site, guards)) =
        selected_transition_premise(program, facts, state_flow, call_flow, call_frames)
    else {
        return false;
    };
    let Some(target_parameters) =
        validation::call_target_parameters(program, call_flow.target_symbol)
    else {
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
    guards.iter().any(|guard| {
        guard_conjunct_matches(program, *guard, &required_label)
            || super::guard_observations::conjunct_matches(
                program, facts, state_flow, call_flow, *guard, expression,
            )
            || super::super::call_bounds::proves(
                program,
                facts,
                state_flow,
                call_flow,
                &[],
                expression,
                Some(*guard),
            )
    }) && super::super::guard_operands::requirement_reads_survive_earlier_operand_writes(
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
) -> bool {
    let Some((call_site, matching_guards)) =
        selected_transition_premise(program, facts, state_flow, call_flow, call_frames)
    else {
        return false;
    };
    let matches_guard = |label: &str| {
        matching_guards
            .iter()
            .any(|guard| guard_conjunct_matches(program, *guard, label))
    };
    let Some(target_parameters) =
        validation::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
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
                super::guard_observations::conjunct_matches(
                    program,
                    facts,
                    state_flow,
                    call_flow,
                    *matching_guard,
                    expression,
                )
            })
        }
        _ => false,
    };
    let guard_establishes = matches_guard(&required_label) || copied_guard_establishes
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
                let contains = |(low, high): (numerics::bignum::BigInt, numerics::bignum::BigInt)| {
                    low >= required_low && high <= required_high
                };
                matching_guards.iter().any(|guard| {
                    super::super::intervals::guard_interval_for_label(program, *guard, &subject_label)
                        .is_some_and(contains)
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

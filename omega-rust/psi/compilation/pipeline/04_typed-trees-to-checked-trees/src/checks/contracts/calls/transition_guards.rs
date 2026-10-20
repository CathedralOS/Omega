//! Transition-guard requirement proofs: the selected transition call,
//! guard conjunct matching, and guarded-value preservation a call
//! requirement may read through.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::FactPayload;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

use crate::labels::semantic_fact_requirement_label;

fn selected_transition_call<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<(
    validation::CallSite<'program>,
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
    typed_trees::expression::ExpressionHandle,
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
    if !selects_this_call {
        return None;
    }
    Some((call_site, machine, caller_state, guard))
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
    let Some((call_site, machine, _, guard)) =
        selected_transition_call(program, state_flow, call_flow)
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
    if !guard_preserves_values(machine, guard, call_frames) {
        return false;
    }
    (guard_conjunct_matches(program, guard, &required_label)
        || super::guard_observations::conjunct_matches(
            program, facts, state_flow, call_flow, guard, expression,
        )
        || super::super::call_bounds::proves(
            program,
            facts,
            state_flow,
            call_flow,
            &[],
            expression,
            Some(guard),
        ))
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

/// The predicate a COMPUTED transition subject was captured from, when the
/// guard this arm reads is that captured subject's truth test.
///
/// `parse_transition` evaluates a computed subject once, before arm dispatch,
/// and captures it into a generated local (`__transition_subject#N`); the arms
/// then read `__transition_subject#N == true`. A requirement proof that matches
/// a guard against the requirement's own predicate needs the PREDICATE, not the
/// lowered truth test: a predicate-only domain reduces to exactly the predicate
/// the capture was built from, and matching the truth test against it finds
/// nothing. Recovery is refused unless the capture is the statement immediately
/// before the transition, so no statement can run between the subject's
/// evaluation and the jump.
fn captured_subject_predicate(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    transition_index: usize,
    guard: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let subject = subject_truth_test_symbol(program, guard)?;
    let capture = program
        .statement_table
        .statement_at(state, transition_index.checked_sub(1)?)?;
    let local = capture.as_local_data()?;
    if local.symbol != subject || !local.initial_value.is_valid() {
        return None;
    }
    Some(local.initial_value)
}

/// The symbol a `<subject> == true` guard tests, or `None` for any other guard.
/// Only the captured-subject truth test has this shape; an authored
/// `flag == true` comparison is a different expression and is left alone.
fn subject_truth_test_symbol(
    program: &dyn ::validation::CheckingInput,
    guard: ExpressionHandle,
) -> Option<SymbolHandle> {
    let binary = program.expression_table.binary_node(guard)?;
    if binary.operator != typed_trees::expression::BinaryOperator::Equal
        || !matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        )
    {
        return None;
    }
    match program.expression_table.expression(binary.left) {
        ExpressionNode::Name(path) => Some(path.symbol),
        _ => None,
    }
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
    let Some((call_site, machine, caller_state, guard)) =
        selected_transition_call(program, state_flow, call_flow)
    else {
        return false;
    };
    if !guard_preserves_values(machine, guard, call_frames) {
        return false;
    }
    // A computed subject reaches the arms as its captured truth test, whose text
    // carries none of the predicate the requirement must match. Match against
    // the captured predicate instead, and below require that predicate's
    // operands to survive the arm's own operand calls: the truth test says
    // nothing about them.
    let captured_premise =
        captured_subject_predicate(program, caller_state, call_flow.statement_index, guard);
    let matching_guard = captured_premise.unwrap_or(guard);
    // Short-circuiting leaves a later conjunct's calls inside the capture.
    // The truth test's empty frame cannot establish that the recovered
    // predicate kept its earlier reads current through those calls.
    if captured_premise.is_some() && !guard_preserves_values(machine, matching_guard, call_frames) {
        return false;
    }
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
            super::guard_observations::conjunct_matches(
                program, facts, state_flow, call_flow, guard, expression,
            )
        }
        _ => false,
    };
    let guard_establishes = guard_conjunct_matches(program, matching_guard, &required_label) || copied_guard_establishes
        || super::predicate_domains::predicate_only_domain_labels(program, facts, fact).is_some_and(|labels| {
            labels
                .iter()
                .all(|label| guard_conjunct_matches(program, matching_guard, label))
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
                super::super::intervals::guard_interval_for_label(
                    program,
                    matching_guard,
                    &subject_label,
                )
                .is_some_and(contains)
            },
        );
    if !guard_establishes {
        return false;
    }
    // A recovered predicate was read at the subject's evaluation, before every
    // arm operand. An operand call that writes one of its operands would leave
    // the requirement discharged from a value the jump no longer carries, and
    // the truth test cannot see that.
    if let Some(premise) = captured_premise
        && !super::super::guard_operands::premise_reads_survive_arm_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            premise,
            call_frames,
        )
    {
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

//! Transition-guard requirement proofs: the selected transition call,
//! guard conjunct matching, and guarded-value preservation a call
//! requirement may read through.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::FactPayload;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

use crate::labels::semantic_fact_requirement_label;

fn selected_transition_call<'program>(
    program: &'program typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<(
    crate::semantic::calls::CallSite<'program>,
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
    typed_trees::expression::ExpressionHandle,
)> {
    let call_site = crate::semantic::calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    )?;
    let machine = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol)?;
    let caller_state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)?;
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statements(caller_state.statement_nodes)
        .get(call_flow.statement_index)
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
        crate::semantic::calls::CallSite::TransitionNamed { .. } => true,
        crate::semantic::calls::CallSite::Expression { expression, .. } => {
            matches!(
                program.statement_table.transition_target(transition.target),
                typed_trees::statement::TransitionTargetNode::Value(value) if value == expression
            )
        }
        crate::semantic::calls::CallSite::Statement(_) => false,
    };
    if !selects_this_call {
        return None;
    }
    Some((call_site, machine, caller_state, guard))
}

/// Query one original Boolean conjunct through the same selected-arm and
/// operand-currentness gates as the complete contract fact.
pub(in crate::checks::contracts) fn transition_guard_proves_boolean_requirement(
    program: &typed_trees::TypedTrees,
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
        crate::semantic::calls::call_target_parameters(program, call_flow.target_symbol)
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
            crate::semantic::calls::call_site_argument_expressions(program, &call_site),
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
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fact: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((call_site, machine, _, guard)) =
        selected_transition_call(program, state_flow, call_flow)
    else {
        return false;
    };
    if !guard_preserves_values(machine, guard, call_frames) {
        return false;
    }
    let Some(target_parameters) =
        crate::semantic::calls::call_target_parameters(program, call_flow.target_symbol)
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
    let guard_establishes = guard_conjunct_matches(program, guard, &required_label)
        || super::predicate_domains::predicate_only_domain_labels(program, facts, fact).is_some_and(|labels| {
            labels
                .iter()
                .all(|label| guard_conjunct_matches(program, guard, label))
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
                super::super::intervals::guard_interval_for_label(program, guard, &subject_label)
                    .is_some_and(contains)
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
        crate::semantic::calls::call_site_argument_expressions(program, &call_site),
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
    program: &typed_trees::TypedTrees,
    guard: typed_trees::expression::ExpressionHandle,
    required_label: &str,
) -> bool {
    if program.expression_table.display_name(guard) == required_label {
        return true;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
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
    program: &typed_trees::TypedTrees,
    guard: typed_trees::expression::ExpressionHandle,
    required_label: &str,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
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
        _ => super::super::intervals::complement_comparison(binary.operator).is_some_and(
            |complement| {
                format!(
                    "{} {} {}",
                    program.expression_table.display_name(binary.left),
                    complement.display_name(),
                    program.expression_table.display_name(binary.right)
                ) == required_label
            },
        ),
    }
}

use typed_trees::statement::{StatementNode, TransitionTargetNode};

use super::calls::collect_state_argument_facts_for_call;
use super::expressions::collect_state_argument_facts_from_expression;
use super::{StateArgumentContext, StateArgumentFacts};
use crate::checks::ranges::arrays::fixed_array_type_length;
use crate::checks::ranges::expressions::{
    expression_indexable_length, expression_integer_value, expression_name,
};
use crate::checks::ranges::facts::RangeFacts;
use crate::checks::ranges::guards;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn collect_state_argument_facts_from_statement(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    collected: &mut Vec<StateArgumentFacts>,
) {
    let program = context.program;
    let machine = context.machine;
    match statement {
        StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            collect_state_argument_facts_from_expression(
                context,
                facts,
                assignment.target,
                collected,
            );
            collect_state_argument_facts_from_expression(
                context,
                facts,
                assignment.value,
                collected,
            );
            // RHS effects and values are evaluated before replacing the target.
            let mut next_length = crate::checks::ranges::assignment_lengths::replacement_length(
                program,
                machine,
                context.state,
                facts,
                assignment.target,
                assignment.value,
            );
            // Keep the checking pass's rebound-reference extent lane in this
            // collection replay: `view = borrow_rows(other)` re-lends
            // `other.rooms`, so the same referent supplies the slice's length.
            let mut referent_floor = None;
            if next_length.is_none()
                && let Some((symbol, _)) = expression_name(program, assignment.target)
                && let Some(declared) =
                    crate::checks::ranges::statements::assigned_local_declared_type(
                        program,
                        context.state,
                        facts.statement_index,
                        symbol,
                    )
                && let Some(referent) =
                    crate::checks::ranges::statements::bound_reference_referent_extent(
                        program,
                        machine,
                        context.state,
                        context.call_frames,
                        facts,
                        symbol,
                        declared,
                    )
            {
                next_length = referent.exact;
                referent_floor = referent.minimum;
            }
            let next_integer = expression_integer_value(program, facts, assignment.value);
            let extent = crate::checks::ranges::assignment_lengths::assigned_extent(
                program,
                machine,
                context.state,
                facts,
                assignment.target,
                assignment.value,
            );
            facts.invalidate_assignment_bounds(program, machine, context.state, statement);
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                facts.assign_local(symbol, name, next_length, next_integer);
                if let Some(floor) = referent_floor {
                    facts.prove_minimum_length(
                        program.expression_table.display_name(assignment.target),
                        floor,
                    );
                }
                seed_boolean_guard_local(context, facts, symbol, name, assignment.value);
                // The checking pass seeds a bound name's ensured result
                // bounds on its label; mirror that here or a later
                // transition cannot transport `i = pick()`'s exit proof.
                crate::checks::ranges::statements::aliases::seed_ensured_call_result_bounds(
                    program,
                    facts,
                    name.unwrap_or_default(),
                    assignment.value,
                );
            } else if matches!(
                program.expression_table.expression(assignment.target),
                ExpressionNode::Member(_)
            ) {
                // A field store carries the same exit-proven bound on its
                // display label, matching the member-target seeding in the
                // checking pass (`self.slot = pick()` then `-> load(self.slot)`).
                crate::checks::ranges::statements::aliases::seed_ensured_call_result_bounds(
                    program,
                    facts,
                    &program.expression_table.display_name(assignment.target),
                    assignment.value,
                );
            }
            crate::checks::ranges::assignment_lengths::seed_assigned_extent(
                program,
                machine,
                context.state,
                facts,
                extent,
            );
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_state_argument_facts_from_expression(context, facts, *argument, collected);
            }
            collect_state_argument_facts_for_call(
                program,
                machine,
                context.state,
                facts,
                call.target_symbol,
                program.statement_table.expression_handles(call.arguments),
                false,
                collected,
            );
            let paths = context
                .call_frames
                .and_then(|frames| frames.may_write_frame(machine, call).into_complete_paths());
            facts.invalidate_call_writes(
                program,
                machine,
                context.state,
                paths.as_deref(),
                Some(&crate::semantic_calls::CallSite::Statement(call)),
            );
            // R4 witness mint in the COLLECTION pass too: boundary ensures
            // bound the &mut argument places, so a later transition can
            // transport the fact into its target's params.
            crate::checks::ranges::statements::seed_boundary_call_ensures_facts(
                program, machine, call, facts,
            );
        }
        StatementNode::Expression(expression) => {
            collect_state_argument_facts_from_expression(context, facts, *expression, collected);
        }
        StatementNode::LocalData(local) => {
            collect_state_argument_facts_from_expression(
                context,
                facts,
                local.initial_value,
                collected,
            );
            let mut length = fixed_array_type_length(program, local.type_reference).or_else(|| {
                expression_indexable_length(
                    program,
                    machine,
                    context.state,
                    facts,
                    local.initial_value,
                )
            });
            // Mirror the checking pass's returned-reference extent lane so the
            // argument facts a transition carries see the same slice lengths.
            if length.is_none()
                && let Some(referent) =
                    crate::checks::ranges::statements::bound_reference_referent_extent(
                        program,
                        machine,
                        context.state,
                        context.call_frames,
                        facts,
                        local.symbol,
                        local.type_reference,
                    )
            {
                length = referent.exact;
                if let Some(minimum) = referent.minimum {
                    facts.prove_minimum_length(local.name.to_string(), minimum);
                }
            }
            let integer = expression_integer_value(program, facts, local.initial_value);
            facts.define_local(local.symbol, local.name.to_string(), length, integer);
            seed_boolean_guard_local(
                context,
                facts,
                local.symbol,
                Some(local.name.as_str()),
                local.initial_value,
            );
            // Mirror the checking pass's ensured-result seeding on the bound
            // name so a later transition transports `let i = pick()`'s exit
            // proof through the local's label (`i < K`, `i >= 0`).
            crate::checks::ranges::statements::aliases::seed_ensured_call_result_bounds(
                program,
                facts,
                local.name.as_str(),
                local.initial_value,
            );
        }
        StatementNode::Transition(transition) => {
            // A guard established before a recursive / cyclic transition refines
            // the facts that flow into the callee's arguments. The guard's
            // positive form constrains the branch that is actually taken
            // (`transition.target`), so narrow a working copy of the facts with
            // it before deriving the target's argument facts.
            let guarded_facts = match transition.guard {
                typed_trees::statement::TransitionGuardNode::When(guard) if guard.is_valid() => {
                    collect_state_argument_facts_from_expression(context, facts, guard, collected);
                    let mut narrowed = facts.clone();
                    if context.call_frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, guard)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    }) {
                        guards::seed_guard_facts(
                            program,
                            machine,
                            context.state,
                            &mut narrowed,
                            guard,
                        );
                    }
                    Some(narrowed)
                }
                _ => None,
            };
            let mut target_facts = guarded_facts.unwrap_or_else(|| facts.clone());

            collect_state_argument_facts_from_target(
                context,
                &mut target_facts,
                transition.target,
                collected,
            );
            // The continuation branch is taken when the guard does not hold, so
            // it is analysed with the unrefined facts.
            let mut continuation_facts = facts.clone();
            collect_state_argument_facts_from_target(
                context,
                &mut continuation_facts,
                transition.continuation,
                collected,
            );
        }
    }
}

fn collect_state_argument_facts_from_target(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    target: typed_trees::statement::TransitionTargetHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    let program = context.program;
    let machine = context.machine;
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_state_argument_facts_from_expression(context, facts, *argument, collected);
            }
            let Some(target_state) = program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == path.symbol)
            else {
                return;
            };

            collect_state_argument_facts_for_call(
                program,
                machine,
                context.state,
                facts,
                target_state.symbol,
                program.statement_table.expression_handles(*arguments),
                true,
                collected,
            );
        }
        TransitionTargetNode::Value(value) => {
            collect_state_argument_facts_from_expression(context, facts, *value, collected);
        }
        TransitionTargetNode::SelfTarget => {
            collect_state_argument_facts_for_call(
                program,
                machine,
                context.state,
                facts,
                context.state.symbol,
                &[],
                true,
                collected,
            );
        }
        TransitionTargetNode::Terminal => {}
    }
}

fn seed_boolean_guard_local(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    symbol: symbols::SymbolHandle,
    name: Option<&str>,
    expression: ExpressionHandle,
) {
    let program = context.program;
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) && context.call_frames.is_some_and(|frames| {
        frames
            .expression_write_frame(context.machine, expression)
            .into_complete_paths()
            .is_some_and(|paths| paths.is_empty())
    }) {
        facts.define_boolean_guard_local(symbol, name.unwrap_or_default().to_owned(), expression);
    }
}

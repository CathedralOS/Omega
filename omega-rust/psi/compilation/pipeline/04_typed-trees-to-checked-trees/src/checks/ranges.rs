//! Index and range checks: every indexed access and subslice range in a
//! machine body must be proved within its collection's length from the facts
//! known at that statement.
//!
//! `check_indexed_accesses` is the entry;
//! `checks::check_checked_facts_recording` calls it after
//! the operator check and passes the incoming guard index it built from
//! `incoming_guards`. It reads the fixed-array field lengths once (`arrays`),
//! then for each machine prepares one `facts::RangeCallContext` per state and
//! collects the facts that edge arguments carry into each state
//! (`state_arguments`), the machine's incoming guards, and its loop
//! invariants (`loop_invariants`). For each state it starts a new
//! `facts::RangeFacts`, defines the state parameters as locals, and seeds, in
//! order: the state's `requires` (`requirements`), the state argument facts,
//! dependent parameter orderings (`dependent_params`), the incoming guard
//! facts and the loop invariant facts. It then runs
//! `statements::check_statement` on each statement.
//! Branch-local facts share the state's collection query context. Exact bounds
//! consume the normal-return observation owner used by call requirements and
//! casts, including ordered children and path-local guarantees.
//!
//! `check_statement` drives `statement_transfer::transfer_statement_facts`,
//! the single statement fact transfer that `state_arguments` also replays,
//! and checks each expression it reads with `indexes` and each transition
//! target under its edge's facts. `assignment_lengths` decides how an
//! assignment changes a collection's live length.
//!
//! The remaining children are shared vocabulary: `facts` is the per-state
//! fact store, `guards` seeds facts from guard expressions, `expressions`,
//! `types` and `proofs` answer integer, length, type and bound questions, and
//! `diagnostics` words the failures. `incoming_guards`, `requirements` and
//! `types` are also used by other checks, and `indexes` exports
//! `ranges_seam_owns` for the operator `requires` check.

// What a state starts with before its statements run: its `requires`, the
// facts its incoming edges' arguments carry, dependent parameter orderings,
// incoming guards and loop invariants.
mod dependent_params;
pub(in crate::checks) mod incoming_guards;
mod loop_invariants;
pub(in crate::checks) mod requirements;
mod state_arguments;

// Statement checking: the shared statement transfer, the statement check
// that drives it, and the index and assignment-length checks it runs.
mod assignment_lengths;
mod expression_transfer;
mod indexes;
pub(in crate::checks) use indexes::ranges_seam_owns;
mod statement_transfer;
mod statements;

// Shared vocabulary: the per-state fact store, guard seeding, integer,
// length, type and bound queries, and failure diagnostics.
mod arrays;
mod diagnostics;
mod expressions;
mod facts;
mod guards;
mod proofs;
pub(crate) mod types;

// Tests.

use ::diagnostics::Diagnostic;
use arrays::fixed_array_field_lengths;
pub(in crate::checks) use arrays::fixed_array_type_length;
use dependent_params::seed_dependent_param_orderings;
pub(in crate::checks) use expressions::folded_integer_binary;
use facts::RangeFacts;
use incoming_guards::{IncomingGuardIndex, seed_incoming_guard_facts};
use loop_invariants::{collect_loop_invariant_facts, seed_loop_invariant_facts};
use requirements::seed_state_requires;
use state_arguments::{collect_state_argument_facts, seed_state_argument_facts};
use statements::check_statement;
pub(in crate::checks) use types::expression_enforced_declared_range;

pub(crate) fn check_indexed_accesses(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    borrows: &checked_trees::BorrowFacts,
    flow: &checked_trees::FlowFacts,
    semantic: &::facts::FactPlan,
    expression_intervals: &[validation::ExpressionInterval],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming_guards: &IncomingGuardIndex,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<
    (
        Vec<typed_trees::expression::ExpressionHandle>,
        Vec<(typed_trees::expression::ExpressionHandle, u64)>,
        Vec<checked_trees::ConstantRecipeObligation>,
    ),
    Vec<Diagnostic>,
> {
    let field_lengths = fixed_array_field_lengths(program);
    let bound_lookup = validation::ImmutableBoundLookup::new(program);
    let mut diagnostics = Vec::new();
    let mut verdicts = indexes::IndexVerdicts::default();
    // All states and their branch snapshots query the same immutable program
    // and borrow facts through the pass's shared summary table.

    for machine in program.machines() {
        let machine_states = program.machine_states(machine);
        let state_has_indexed: Vec<bool> = machine_states
            .iter()
            .map(|state| state_contains_indexed_expression(program, state))
            .collect();
        if !state_has_indexed
            .iter()
            .copied()
            .any(std::convert::identity)
        {
            continue;
        }
        let non_entry_has_indexed = state_has_indexed
            .get(1..)
            .is_some_and(|rest| rest.iter().copied().any(std::convert::identity));
        let calls: Vec<_> = phase_profile::scope("05.d.7.9.a.range_call_context", || {
            let states = if non_entry_has_indexed {
                machine_states
            } else {
                &machine_states[..1]
            };
            states
                .iter()
                .map(|state| {
                    facts::RangeCallContext::new(
                        machine,
                        state,
                        borrows,
                        flow,
                        semantic,
                        call_frames,
                    )
                })
                .collect()
        });
        let state_argument_facts = if non_entry_has_indexed {
            phase_profile::scope("05.d.7.9.b.state_argument_facts", || {
                collect_state_argument_facts(
                    program,
                    &field_lengths,
                    &bound_lookup,
                    machine,
                    call_frames,
                    &calls,
                    operators,
                    mutation_summaries,
                )
            })
        } else {
            Vec::new()
        };
        let incoming_guard_facts = incoming_guards.for_machine(machine.symbol);
        let loop_invariant_facts = phase_profile::scope("05.d.7.9.c.loop_invariant_facts", || {
            collect_loop_invariant_facts(program, machine, call_frames)
        });
        for ((state, calls), has_indexed) in
            machine_states.iter().zip(&calls).zip(state_has_indexed)
        {
            if !has_indexed {
                continue;
            }
            let collection_bounds =
                validation::CollectionBoundsObservations::new(program, machine, state);
            let mut facts = RangeFacts::new(machine.symbol, &field_lengths, &bound_lookup);
            facts.collection_bounds = Some(&collection_bounds);
            facts.mutation_summaries = std::borrow::Cow::Borrowed(mutation_summaries);
            facts.checked_operators = Some(operators);
            facts.expression_intervals = expression_intervals;
            facts.checked_calls = Some(calls);
            // State parameters are stable named places for the duration of
            // the state, just like locals introduced by `let`. Retain a
            // literal fixed-array referee's length even through a reference
            // access mode so ordinary index checking—not an access-mode-
            // specific gate—owns every dynamic bounds obligation.
            for parameter in program.state_parameters(state) {
                facts.define_local(
                    parameter.symbol,
                    parameter.name.to_string(),
                    fixed_array_type_length(program, parameter.type_reference),
                    None,
                );
            }
            phase_profile::scope("05.d.7.9.d.seed_state", || {
                seed_state_requires(program, &mut facts, machine, state);
                seed_state_argument_facts(&mut facts, state, &state_argument_facts);
                seed_dependent_param_orderings(program, &mut facts, machine, state);
                seed_incoming_guard_facts(
                    program,
                    machine,
                    &mut facts,
                    state,
                    incoming_guard_facts,
                );
                seed_loop_invariant_facts(
                    program,
                    machine,
                    &mut facts,
                    state,
                    &loop_invariant_facts,
                );
            });
            let _s = phase_profile::guard("05.d.7.9.e.check_statements");
            for (statement_index, statement) in program
                .statement_table
                .admitted_execution_statements(state.statement_nodes)
            {
                facts.statement_index = statement_index;
                check_statement(
                    program,
                    machine,
                    state,
                    call_frames,
                    &mut facts,
                    statement,
                    &mut diagnostics,
                    &mut verdicts,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(verdicts.into_parts())
    } else {
        Err(diagnostics)
    }
}

fn state_contains_indexed_expression(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> bool {
    for (_, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        if statement_contains_indexed_expression(program, statement) {
            return true;
        }
    }
    false
}

fn statement_contains_indexed_expression(
    program: &dyn ::validation::CheckingInput,
    statement: &typed_trees::statement::StatementNode,
) -> bool {
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => {
            expression_contains_indexed(program, assignment.target, 0)
                || expression_contains_indexed(program, assignment.value, 0)
        }
        StatementNode::Call(call) => program
            .statement_table
            .expression_handles(call.arguments)
            .iter()
            .any(|argument| expression_contains_indexed(program, *argument, 0)),
        StatementNode::Expression(expression) => {
            expression_contains_indexed(program, *expression, 0)
        }
        StatementNode::LocalData(local) => {
            expression_contains_indexed(program, local.initial_value, 0)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard
                && expression_contains_indexed(program, guard, 0)
            {
                return true;
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        if program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .any(|argument| expression_contains_indexed(program, *argument, 0))
                        {
                            return true;
                        }
                    }
                    TransitionTargetNode::Value(value) => {
                        if expression_contains_indexed(program, *value, 0) {
                            return true;
                        }
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
            false
        }
    }
}

fn expression_contains_indexed(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    depth: usize,
) -> bool {
    use typed_trees::expression::{ExpressionNode, MatchPattern};
    let Some(node) = program
        .expression_table
        .expression_at_depth(expression, depth)
    else {
        return false;
    };
    let next = depth + 1;
    match node {
        ExpressionNode::SavedSubject(_) => false,
        ExpressionNode::Indexed(_) => true,
        ExpressionNode::Match(dispatch) => {
            expression_contains_indexed(program, dispatch.subject, next)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        (match arm.pattern {
                            MatchPattern::Destructure(_) => program
                                .expression_table
                                .match_pattern_operands(arm.pattern)
                                .any(|operand| expression_contains_indexed(program, operand, next)),
                            MatchPattern::Value(pattern) => {
                                expression_contains_indexed(program, pattern, next)
                            }
                            MatchPattern::Wildcard => false,
                        }) || expression_contains_indexed(program, arm.value, next)
                    })
        }
        ExpressionNode::Binary(binary) => {
            expression_contains_indexed(program, binary.left, next)
                || expression_contains_indexed(program, binary.right, next)
        }
        ExpressionNode::Call(call) => {
            expression_contains_indexed(program, call.receiver, next)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_contains_indexed(program, *argument, next))
        }
        ExpressionNode::Cast(cast) => expression_contains_indexed(program, cast.value, next),
        ExpressionNode::Atomic(atomic) => {
            if let Some(carrier) = validation::atomic_once_carrier(program, expression) {
                expression_contains_indexed(program, carrier.place, next)
                    || carrier
                        .operands
                        .into_iter()
                        .any(|operand| expression_contains_indexed(program, operand, next))
            } else {
                expression_contains_indexed(program, atomic.value, next)
            }
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_contains_indexed(program, *value, next)),
        ExpressionNode::Member(member) => {
            expression_contains_indexed(program, member.receiver, next)
        }
        ExpressionNode::Borrow(borrow) => expression_contains_indexed(program, borrow.target, next),
        ExpressionNode::Unary(unary) => expression_contains_indexed(program, unary.operand, next),
        ExpressionNode::Range(range) => {
            expression_contains_indexed(program, range.start, next)
                || expression_contains_indexed(program, range.end, next)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains_indexed(program, field.value, next)),
        typed_trees::literal_expression_pattern!() => false,
    }
}

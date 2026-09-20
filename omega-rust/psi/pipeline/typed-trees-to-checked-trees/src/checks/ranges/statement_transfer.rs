//! The single statement transfer both range passes replay.
//!
//! The checking pass (`statements::check_statement`) and the
//! state-argument collection replay (`state_arguments::statements`)
//! must leave identical fact state after every statement: a seed the
//! replay misses silently weakens every fact a transition argument can
//! carry, and ensured call results, name aliases, member stores and
//! subslice windows each arrived as a separate divergence repair.
//! Both walkers now drive this one transfer and differ only in the
//! per-edge sink: the checking pass emits diagnostics, the collection
//! pass merges outgoing-edge argument facts.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableAssignment, TableCall, TransitionGuardNode, TransitionTargetHandle,
};
use validation::CallFrameResolver;

use super::arrays::fixed_array_type_length;
use super::assignment_lengths::{assigned_extent, replacement_length, seed_assigned_extent};
use super::expressions::{expression_indexable_length, expression_integer_value, expression_name};
use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts, seed_value_vs_value_endpoints};
use super::statements::aliases::{
    seed_ensured_call_result_bounds, seed_local_alias_facts, seed_subslice_window_facts,
};
use super::statements::{
    assigned_local_declared_type, bound_reference_referent_extent, expression_member_name,
    seed_boundary_call_ensures_facts, seed_offset_index_bound,
};

/// The pass-specific work a statement performs around the shared fact
/// transfer. Each hook lands at the point the owning pass needs it; a
/// pass with no work at a hook leaves the default no-op.
pub(super) trait StatementTransferSink<'program> {
    /// Visit a read expression — an operand, initializer, call argument
    /// or guard — in evaluation order, before the statement's fact
    /// mutation. The checking pass validates indexes; the collection
    /// pass harvests expression-position call edges.
    fn visit_expression(&mut self, facts: &mut RangeFacts<'_>, expression: ExpressionHandle);

    /// Between an assignment's target visit and its value visit: the
    /// checking pass decides here whether the RHS survives the captured
    /// extent window. The collection pass does no work between operands.
    fn check_assignment_extent_window(
        &mut self,
        _facts: &mut RangeFacts<'_>,
        _assignment: &'program TableAssignment,
    ) {
    }

    /// A plain call statement's invocation edge, after its argument
    /// expressions and before the callee's write frame retires affected
    /// facts. The collection pass merges the callee's argument facts;
    /// the checking pass has no call-edge work of its own.
    fn visit_call(&mut self, _facts: &mut RangeFacts<'_>, _call: &'program TableCall) {}

    /// A transition target under the facts narrowed for its edge —
    /// guarded edges see the arm guard's positive seeds, continuations
    /// see the refuted seeds.
    fn visit_transition_target(
        &mut self,
        facts: &mut RangeFacts<'_>,
        target: TransitionTargetHandle,
    );
}

/// Replay one statement's complete fact transfer into `facts`, driving
/// `sink` at the points the passes diverge. This is the ONLY definition
/// of the order: expression visits, extent-window decision, invalidation,
/// local/alias/window/member-store seeds, call-edge visit, write-frame
/// invalidation, ensured-result seeds, and per-edge guard narrowing.
pub(super) fn transfer_statement_facts<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    call_frames: Option<&CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    statement: &'program StatementNode,
    sink: &mut dyn StatementTransferSink<'program>,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            for expression in [binding.receiver, binding.implementation_operand] {
                if expression.is_valid() {
                    sink.visit_expression(facts, expression);
                }
            }
        }
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            sink.visit_expression(facts, assignment.target);
            sink.check_assignment_extent_window(facts, assignment);
            sink.visit_expression(facts, assignment.value);
            // RHS effects and values are evaluated before replacing the target.
            let mut next_length = replacement_length(
                program,
                machine,
                state,
                facts,
                assignment.target,
                assignment.value,
            );
            // A rebound reference takes its NEW referent's extent the way the
            // `let` binding does: `view = borrow_rows(other)` re-lends
            // `other.rooms`, so the write-origins prefix proves the
            // replacement slice's length evidence the same way.
            let mut referent_floor = None;
            if next_length.is_none()
                && let Some((symbol, _)) = expression_name(program, assignment.target)
                && let Some(declared) =
                    assigned_local_declared_type(program, state, facts.statement_index, symbol)
                && let Some(referent) = bound_reference_referent_extent(
                    program,
                    machine,
                    state,
                    call_frames,
                    facts,
                    symbol,
                    declared,
                )
            {
                next_length = referent.exact;
                referent_floor = referent.minimum;
            }
            let next_integer = expression_integer_value(program, facts, assignment.value);
            let extent = assigned_extent(
                program,
                machine,
                state,
                facts,
                assignment.target,
                assignment.value,
            );
            facts.invalidate_assignment_bounds(program, machine, state, statement);
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                facts.assign_local(symbol, name, next_length, next_integer);
                // The referent's floor describes the NEW binding, so it
                // seeds only after the rebound label sheds the old
                // value's facts.
                if let Some(floor) = referent_floor {
                    facts.prove_minimum_length(
                        program.expression_table.display_name(assignment.target),
                        floor,
                    );
                }
                seed_boolean_guard_local(
                    program,
                    machine,
                    call_frames,
                    facts,
                    symbol,
                    name,
                    assignment.value,
                );
                seed_local_alias_facts(
                    program,
                    machine,
                    state,
                    facts,
                    assignment.value,
                    symbol,
                    name,
                );
                seed_subslice_window_facts(program, facts, assignment.value, name);
            } else if let Some((symbol, name)) = expression_member_name(program, assignment.target)
            {
                // A member store carries the folded field integer
                // (`self.slot = 2`), the offset index bound (`self.jp =
                // self.i + 1`), and the ensured call result bound on its
                // display label — `-> load(self.slot)` transports all
                // three into the destination parameter's merged facts.
                facts.assign_field_integer(symbol, name, next_integer);
                seed_offset_index_bound(program, facts, assignment.target, assignment.value);
                seed_ensured_call_result_bounds(
                    program,
                    facts,
                    &program.expression_table.display_name(assignment.target),
                    assignment.value,
                );
            }
            seed_assigned_extent(program, machine, state, facts, extent);
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                sink.visit_expression(facts, *argument);
            }
            // The callee's argument facts see the caller's seeded facts
            // (transport through ensured call results, aliases and
            // windows above) but not the call's own write retirement.
            sink.visit_call(facts, call);
            let paths = call_frames
                .and_then(|frames| frames.may_write_frame(machine, call).into_complete_paths());
            facts.invalidate_call_writes(
                program,
                machine,
                state,
                paths.as_deref(),
                Some(&crate::semantic_calls::CallSite::Statement(call)),
            );
            // R4 witness mint: a BOUNDARY callee's `ensures <param> <= K`
            // bounds the `&mut` out-argument's place the moment the call
            // returns (the boundary model's citable fact). Any prior
            // upper-bound fact for a written place is dropped first; the
            // ensures then re-proves what it states.
            seed_boundary_call_ensures_facts(program, machine, call, facts);
        }
        StatementNode::Expression(expression) => {
            sink.visit_expression(facts, *expression);
        }
        StatementNode::LocalData(local) => {
            sink.visit_expression(facts, local.initial_value);
            let mut length = fixed_array_type_length(program, local.type_reference).or_else(|| {
                expression_indexable_length(program, machine, state, facts, local.initial_value)
            });
            // A returned `&mut [T]` binding has no indexable initializer the
            // expression lane can measure: the callee chose the referent. The
            // write-origins prefix recovers that referent, whose declared
            // extent (or recorded live length) is the slice's length.
            if length.is_none()
                && let Some(referent) = bound_reference_referent_extent(
                    program,
                    machine,
                    state,
                    call_frames,
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
                program,
                machine,
                call_frames,
                facts,
                local.symbol,
                Some(local.name.as_str()),
                local.initial_value,
            );
            seed_local_alias_facts(
                program,
                machine,
                state,
                facts,
                local.initial_value,
                local.symbol,
                Some(local.name.as_str()),
            );
            seed_subslice_window_facts(
                program,
                facts,
                local.initial_value,
                Some(local.name.as_str()),
            );
        }
        StatementNode::Transition(transition) => {
            // A guard established before a recursive / cyclic transition
            // refines the facts that flow into the callee's arguments. The
            // guard's positive form constrains the branch that is actually
            // taken (`transition.target`), so narrow a working copy of the
            // facts with it before deriving the target's argument facts.
            let readonly_guard = match transition.guard {
                TransitionGuardNode::When(guard) => {
                    sink.visit_expression(facts, guard);
                    call_frames
                        .is_some_and(|frames| {
                            frames
                                .expression_write_frame(machine, guard)
                                .into_complete_paths()
                                .is_some_and(|paths| paths.is_empty())
                        })
                        .then_some(guard)
                }
                TransitionGuardNode::Always => None,
            };
            if transition.target.is_valid() {
                let mut target_facts = facts.clone();
                if let Some(guard) = readonly_guard {
                    seed_guard_facts(program, machine, state, &mut target_facts, guard);
                    seed_value_vs_value_endpoints(
                        program,
                        machine,
                        state,
                        &mut target_facts,
                        guard,
                    );
                }
                sink.visit_transition_target(&mut target_facts, transition.target);
            }
            if transition.continuation.is_valid() {
                // The continuation branch is taken when the guard does not
                // hold, so it is analysed with the refuted guard's facts.
                let mut continuation_facts = facts.clone();
                if let Some(guard) = readonly_guard {
                    seed_negated_guard_facts(
                        program,
                        machine,
                        state,
                        &mut continuation_facts,
                        guard,
                    );
                }
                sink.visit_transition_target(&mut continuation_facts, transition.continuation);
            }
            // Reaching the next statement refutes a prior exit arm. Guard
            // evaluation has already retired its write-affected facts, and
            // only the existing read-only frame gate seeds a new complement.
            // Target effects belong to the selected exit, not fall-through.
            // An absent continuation already owns `facts`; cloning it only to
            // replace the original would copy every name and dependency twice.
            if transition.target.is_valid()
                && !transition.continuation.is_valid()
                && let Some(guard) = readonly_guard
            {
                seed_negated_guard_facts(program, machine, state, facts, guard);
            }
        }
    }
}

/// A freshly bound boolean local remembers its comparison when the
/// comparison is read-only, so a later `has_next ->` arm seed resolves
/// the name back to the comparison under both passes.
fn seed_boolean_guard_local(
    program: &TypedTrees,
    machine: &Machine,
    call_frames: Option<&CallFrameResolver<'_>>,
    facts: &mut RangeFacts<'_>,
    symbol: SymbolHandle,
    name: Option<&str>,
    expression: ExpressionHandle,
) {
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) && call_frames.is_some_and(|frames| {
        frames
            .expression_write_frame(machine, expression)
            .into_complete_paths()
            .is_some_and(|paths| paths.is_empty())
    }) {
        facts.define_boolean_guard_local(symbol, name.unwrap_or_default().to_owned(), expression);
    }
}

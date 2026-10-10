//! Exact scalar entry-rank preservation through direct disjoint stores and
//! statement calls whose complete write frames stay disjoint. The shared
//! write-frame owner closes aliases; an opaque frame is not evidence.

use crate::machine_calls::calls::{CallFrameResolver, frame_paths_overlap};
use facts::NormalizedWriteFrame;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;

use super::projection::{RankOrder, RankProjection};

/// `premises` names the member's entry premise carriers (`None` when its
/// witness resolves none, which rejects every store); `entry_parameters` is
/// this state's source-admitted coordinate readings;
/// `arrival_premises` names the current-state carriers read by its own requires;
/// `conserved_endpoints` says the component mixes ranged and unranged
/// members, so every caller entry input may be read as a conserved endpoint.
pub(super) fn preserves_rank(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    rank: &RankProjection,
    premises: Option<&[SymbolHandle]>,
    entry_parameters: &crate::RankStateBindings,
    arrival_premises: &[SymbolHandle],
    conserved_endpoints: bool,
    statement: &StatementNode,
    frames: Option<&CallFrameResolver<'_>>,
) -> bool {
    if !matches!(
        rank.order,
        RankOrder::Natural(_)
            | RankOrder::IncreasingTo(_)
            | RankOrder::BoundedDistance(_)
            | RankOrder::SliceLength
            | RankOrder::DeclaredIdentity { .. }
            | RankOrder::DeclaredComputation { .. }
            | RankOrder::CustomStructView { .. }
    ) {
        return false;
    }
    let Some(premises) = premises else {
        return false;
    };
    // Source facts name exact selected coordinates. Field paths stay precise
    // at the shared frame adapter; collection indices retain its documented
    // coarse containing-home policy. No equal-copy premise is reconstructed.
    let mut protected = Vec::new();
    for binding in &entry_parameters.coordinates {
        if !conserved_endpoints && !premises.contains(&binding.entry.parameter) {
            continue;
        }
        if program
            .state_parameter_by_symbol(state, binding.destination.parameter)
            .is_none()
        {
            return false;
        }
        let Some(path) = crate::frame_coordinate_path(
            program,
            binding.destination.parameter,
            &binding.destination.path,
        ) else {
            return false;
        };
        if !protected.contains(&path) {
            protected.push(path);
        }
    }
    // Authored state Requires are independently discharged at each arrival.
    for parameter in program.state_parameters(state) {
        if arrival_premises.contains(&parameter.symbol) {
            let Some(path) = crate::frame_coordinate_path(program, parameter.symbol, &[]) else {
                return false;
            };
            if !protected.contains(&path) {
                protected.push(path);
            }
        }
    }
    // The shared arithmetic judgment also consumes this exact state's
    // declared constraints. Their subject and dependent bounds are arrival
    // observations, not live facts renewed after a prefix write.
    let Some(constraints) =
        crate::ranking_range_state_constraint_coordinates(program, machine, state)
    else {
        return false;
    };
    for coordinate in constraints {
        let Some(path) =
            crate::frame_coordinate_path(program, coordinate.parameter, &coordinate.path)
        else {
            return false;
        };
        if !protected.contains(&path) {
            protected.push(path);
        }
    }
    let disjoint = |frame: NormalizedWriteFrame| {
        frame.into_complete_paths().is_some_and(|paths| {
            protected
                .iter()
                .all(|input| paths.iter().all(|path| !frame_paths_overlap(path, input)))
        })
    };
    match statement {
        StatementNode::Assignment(assignment) => {
            if !super::expression_is_inert(program, machine, state, assignment.target)
                || !super::expression_is_inert(program, machine, state, assignment.value)
            {
                return false;
            }
            frames.is_some_and(|frames| disjoint(frames.assignment_write_frame(machine, statement)))
        }
        StatementNode::Call(call) => {
            // A statement-position call keeps the transition-actuals bar:
            // inert arguments, then complete write-frame evidence disjoint
            // from every protected carrier -- the call's own frame plus the
            // aggregated frame of any value calls nested in its arguments.
            // An authored operator or index selection inside an argument
            // could hide a write no frame sees, so the inertness check is
            // structural rather than delegated to the frames. The frame
            // query itself selects the callee's contract: a checked body is
            // summarized, a boundary, requirement, or admitted declaration is
            // bounded by its signature's exclusive reach, and anything
            // unresolved stays opaque rather than admitted.
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .all(|argument| super::expression_is_inert(program, machine, state, *argument))
                && frames.is_some_and(|frames| {
                    disjoint(frames.may_write_frame(machine, call))
                        && disjoint(frames.statement_value_write_frame(machine, statement))
                })
        }
        StatementNode::LocalData(local) => {
            // A `let` binds a fresh local no premise carrier can name, so
            // only its initializer's calls can disturb protected state. The
            // statement-call bar covers the whole initializer tree: every
            // value-position call keeps its subterms inert the way
            // transition actuals are -- boundary, requirement, and admitted
            // callees are covered by their selected signature frame rather
            // than an empty body summary -- while every non-call subterm
            // stays inert, so nested call arguments and composed
            // initializers are admitted alike and a call-free initializer
            // vacuously clears the bar. The aggregate write frame is
            // conservative over every nested call and must be complete and
            // disjoint from every protected carrier.
            let resolved_callee =
                |call: &typed_trees::expression::TableCallExpression| call.target_symbol.is_valid();
            super::expression_is_inert_or_calls(
                program,
                machine,
                state,
                local.initial_value,
                &resolved_callee,
            ) && frames.is_some_and(|frames| {
                disjoint(frames.expression_write_frame(machine, local.initial_value))
            })
        }
        _ => false,
    }
}

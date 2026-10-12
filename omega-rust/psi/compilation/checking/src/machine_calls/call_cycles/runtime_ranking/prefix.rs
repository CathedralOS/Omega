//! Exact scalar entry-rank preservation through direct disjoint stores and
//! statement calls whose complete write frames stay disjoint. The shared
//! write-frame owner closes aliases; an opaque frame is not evidence.

use crate::machine_calls::calls::{CallFrameResolver, frame_paths_overlap};
use facts::NormalizedWriteFrame;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;

/// `premises` names the member's entry premise carriers (`None` conservatively
/// protects all formals); `entry_parameters` is
/// this state's source-admitted coordinate readings;
/// `conserved_endpoints` says the component mixes ranged and unranged
/// members, so every caller entry input may be read as a conserved endpoint.
pub(super) fn preserves_rank(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    premises: Option<&[SymbolHandle]>,
    entry_parameters: &crate::RankStateBindings,
    conserved_endpoints: bool,
    statement: &StatementNode,
    frames: Option<&CallFrameResolver<'_>>,
) -> bool {
    // Source facts name exact selected coordinates. Field paths stay precise
    // at the shared frame adapter; collection indices retain its documented
    // coarse containing-home policy. No equal-copy premise is reconstructed.
    let mut protected = Vec::new();
    // An order without a scalar dependency roster preserves the whole entry
    // frame conservatively. Missing arithmetic projection is not evidence of
    // mutation, and an empty complete operand frame remains meaningful.
    if premises.is_none() {
        for parameter in program.state_parameters(state) {
            let Some(path) = crate::frame_coordinate_path(program, parameter.symbol, &[]) else {
                return false;
            };
            protected.push(path);
        }
    }
    for binding in &entry_parameters.coordinates {
        if !conserved_endpoints
            && premises.is_some_and(|premises| !premises.contains(&binding.entry.parameter))
        {
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
    // Named-state and call-component proofs consume the same exact arrival
    // observations. Preserve selected field paths, not a second whole-root
    // approximation of authored Requires. Writes never renew an arrival fact.
    let Some(premises) = crate::ranking_range_state_premise_coordinates(program, machine, state)
    else {
        return false;
    };
    for coordinate in premises {
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
            if !operand_has_admitted_meaning(program, machine, state, assignment.target)
                || !operand_has_admitted_meaning(program, machine, state, assignment.value)
            {
                return false;
            }
            frames.is_some_and(|frames| disjoint(frames.assignment_write_frame(machine, statement)))
        }
        StatementNode::Call(call) => {
            // Selected call operands retain their own meaning; the complete
            // frame, not call syntax, decides whether they preserve inputs.
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .all(|argument| operand_has_admitted_meaning(program, machine, state, *argument))
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
            operand_has_admitted_meaning(program, machine, state, local.initial_value)
                && frames.is_some_and(|frames| {
                    disjoint(frames.expression_write_frame(machine, local.initial_value))
                })
        }
        StatementNode::Transition(_) => frames
            .is_some_and(|frames| disjoint(frames.statement_value_write_frame(machine, statement))),
        StatementNode::Expression(expression) => {
            operand_has_admitted_meaning(program, machine, state, *expression)
                && frames.is_some_and(|frames| {
                    disjoint(frames.statement_value_write_frame(machine, statement))
                })
        }
        _ => false,
    }
}

/// Calls and intrinsic operations have separate authorities. This proves only
/// operation identity; the caller must also supply complete frame preservation.
pub(super) fn operand_has_admitted_meaning(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    super::expression_is_inert_or_calls(program, machine, state, expression, &|call| {
        call.target_symbol.is_valid()
    })
}

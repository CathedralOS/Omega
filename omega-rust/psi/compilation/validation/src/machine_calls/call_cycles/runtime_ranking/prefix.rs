//! Exact scalar entry-rank preservation through direct disjoint stores and
//! statement calls whose complete write frames stay disjoint. The shared
//! write-frame owner closes aliases; an opaque frame is not evidence.

use crate::machine_calls::calls::CallFrameResolver;
use crate::{
    RankPremiseInput, RankPremiseObservation, frame_preserves_rank_premises, protect_rank_premise,
    rank_protected_path,
};
use facts::NormalizedWriteFrame;
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;

/// `premises` names the member's entry premise carriers and how much of each
/// the judgment reads (`None` conservatively protects all formals whole);
/// `entry_parameters` is this state's source-admitted coordinate readings.
/// Mixed-component endpoint conservation checks its own complete prefix
/// frames independently; it does not turn every unrelated formal into a
/// whole-value ranking premise here.
pub(super) fn preserves_rank(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    premises: Option<&[RankPremiseInput]>,
    entry_parameters: &crate::RankStateBindings,
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
            let Some(carrier) = rank_protected_path(
                program,
                parameter.symbol,
                &[],
                RankPremiseObservation::Value,
            ) else {
                return false;
            };
            protect_rank_premise(&mut protected, carrier);
        }
    }
    for binding in &entry_parameters.coordinates {
        // Transport each observation through the selected coordinate, keeping
        // its suffix and observation kind. Projecting a collection into a
        // field does not turn a length read into a whole-value read.
        let whole = || {
            vec![(
                binding.destination.path.clone(),
                RankPremiseObservation::Value,
            )]
        };
        let readings: Vec<(Vec<facts::PlaceSegment>, RankPremiseObservation)> = match premises {
            None => whole(),
            Some(premises) => {
                premises
                    .iter()
                    .filter(|input| input.parameter == binding.entry.parameter)
                    .map(|input| {
                        if let Some(suffix) = input.path.strip_prefix(binding.entry.path.as_slice())
                        {
                            let mut path = binding.destination.path.clone();
                            path.extend_from_slice(suffix);
                            (path, input.observation)
                        } else {
                            // An unmatched path supplies no exact suffix.
                            // In particular, unequal indexed spellings do not
                            // prove disjoint storage. Keep the supplied place
                            // protected whole until correspondence is known.
                            (
                                binding.destination.path.clone(),
                                RankPremiseObservation::Value,
                            )
                        }
                    })
                    .collect()
            }
        };
        if program
            .state_parameter_by_symbol(state, binding.destination.parameter)
            .is_none()
        {
            return false;
        }
        for (path, observation) in readings {
            let Some(carrier) =
                rank_protected_path(program, binding.destination.parameter, &path, observation)
            else {
                return false;
            };
            protect_rank_premise(&mut protected, carrier);
        }
    }
    // Named-state and call-component proofs consume the same exact arrival
    // observations. Preserve selected field paths, not a second whole-root
    // approximation of authored Requires. Writes never renew an arrival fact.
    let Some(premises) = crate::ranking_range_state_premise_coordinates(program, machine, state)
    else {
        return false;
    };
    for premise in premises {
        let Some(carrier) = rank_protected_path(
            program,
            premise.coordinate.parameter,
            &premise.coordinate.path,
            premise.observation,
        ) else {
            return false;
        };
        protect_rank_premise(&mut protected, carrier);
    }
    let preserves = |frame: NormalizedWriteFrame| frame_preserves_rank_premises(frame, &protected);
    match statement {
        StatementNode::Assignment(assignment) => {
            if !operand_has_admitted_meaning(program, machine, state, assignment.target)
                || !operand_has_admitted_meaning(program, machine, state, assignment.value)
            {
                return false;
            }
            frames
                .is_some_and(|frames| preserves(frames.assignment_write_frame(machine, statement)))
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
                    preserves(frames.may_write_frame(machine, call))
                        && preserves(frames.statement_value_write_frame(machine, statement))
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
                    preserves(frames.expression_write_frame(machine, local.initial_value))
                })
        }
        StatementNode::Transition(_) => frames.is_some_and(|frames| {
            preserves(frames.transition_operand_write_frame(machine, statement))
        }),
        StatementNode::Expression(expression) => {
            operand_has_admitted_meaning(program, machine, state, *expression)
                && frames.is_some_and(|frames| {
                    preserves(frames.statement_value_write_frame(machine, statement))
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

//! Select and seed entry requirements under one state-scope and write-lifetime law.
//! Numeric comparisons and membership consequences are assumptions for checking
//! the body; incoming calls independently establish their admission obligations.
//! Arrivals reuse these seeds while iterating, never reintroducing a stale fact.

use super::{
    ExpressionHandle, ExpressionNode, Interval, Machine, SignatureContractKind, State,
    ValueEnvironment, literal_i64, narrow_environment_by_condition, ordered_values, place_path,
};
use crate::CallFrameResolver;

pub(super) fn seed_state_requirements(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    frames: Option<&CallFrameResolver>,
    environment: &mut ValueEnvironment,
) {
    let is_entry = program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let machine_written = (!is_entry)
        .then(|| machine_written_paths(program, machine, frames))
        .flatten();
    RequirementSeeds::new(
        program,
        machine,
        state,
        frames,
        is_entry,
        machine_written.as_deref(),
    )
    .apply(program, machine, state, environment);
}

/// Every place the machine's complete state write frames can write, or
/// `None` when a frame is opaque. Only a machine-level `requires` condition
/// reads it, so a machine without one never infers its frames here.
pub(super) fn machine_written_paths(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    frames: Option<&CallFrameResolver>,
) -> Option<Vec<String>> {
    let has_machine_requires = program
        .machine_contracts(machine)
        .iter()
        .any(|contract| contract.kind == SignatureContractKind::Requires);
    if !has_machine_requires {
        return None;
    }
    frames.and_then(|frames| {
        frames
            .inferred_machine_state_write_frames(machine)
            .into_iter()
            .map(|frame| frame.into_complete_paths())
            .collect::<Option<Vec<_>>>()
            .map(|paths| paths.concat())
    })
}

/// The `requires` conditions that seed one state's arrival, and the
/// environment they establish on their own. Neither depends on the arriving
/// environment, so a fixed-point walk selects them once per state and only
/// re-narrows each round's arrival by them.
pub(super) struct RequirementSeeds {
    conditions: Vec<ExpressionHandle>,
    memberships: Vec<(String, Interval)>,
}

impl RequirementSeeds {
    pub(super) fn new(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        frames: Option<&CallFrameResolver>,
        is_entry: bool,
        machine_written: Option<&[String]>,
    ) -> Self {
        let mut conditions = Vec::new();
        // A machine-level `requires` fact is a precondition on entry. It still
        // holds at a later state's entry exactly when every place the recorded
        // facts name survives the whole machine's write frame; otherwise a
        // preheader or loop-body write could leave the contract stale before
        // this arrival (the same machine-preservation law the loop-invariant
        // index discharge applies to authored bound chains).
        for condition in program.contract_expressions(
            program.machine_contracts(machine).iter(),
            SignatureContractKind::Requires,
        ) {
            if condition_belongs_to_state(program, machine, state, condition, frames)
                && (is_entry
                    || machine_requires_survives(
                        program,
                        machine,
                        state,
                        machine_written,
                        condition,
                    ))
            {
                conditions.push(condition);
            }
        }
        for condition in program.contract_expressions(
            program.state_contracts(state).iter(),
            SignatureContractKind::Requires,
        ) {
            if condition_belongs_to_state(program, machine, state, condition, frames) {
                conditions.push(condition);
            }
        }
        let mut memberships = Vec::new();
        // These are assumptions of body checking, not proofs of admission.
        // Membership implies every declared predicate even when it also owes
        // an issuer route. Retain the same state scope and write lifetime as
        // an authored comparison; do not turn the range back into membership.
        for (contract, machine_level) in program
            .machine_contracts(machine)
            .iter()
            .map(|contract| (contract, true))
            .chain(
                program
                    .state_contracts(state)
                    .iter()
                    .map(|contract| (contract, false)),
            )
            .filter(|(contract, _)| {
                contract.kind == SignatureContractKind::Requires && contract.binding.is_none()
            })
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if !condition_belongs_to_state(program, machine, state, membership.value, frames) {
                    continue;
                }
                let Some((low, high)) = super::super::domain_intervals::membership_predicate_bounds(
                    program, membership,
                ) else {
                    continue;
                };
                let interval = Interval {
                    low: low.and_then(|value| value.to_i64()),
                    high: high.and_then(|value| value.to_i64()),
                };
                if interval.low.is_none() && interval.high.is_none() {
                    continue;
                }
                let Some(path) = place_path(program, membership.value) else {
                    continue;
                };
                if machine_level && !is_entry {
                    let Some(written) = machine_written else {
                        continue;
                    };
                    let mut seeded = ValueEnvironment::new();
                    seeded.narrow(path.clone(), interval);
                    let mut retained = seeded.clone();
                    retained.invalidate_written_paths(written);
                    if retained != seeded {
                        continue;
                    }
                }
                memberships.push((path, interval));
            }
        }
        Self {
            conditions,
            memberships,
        }
    }

    pub(super) fn apply(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        environment: &mut ValueEnvironment,
    ) {
        for condition in &self.conditions {
            narrow_environment_by_condition(
                program,
                machine,
                Some(state),
                environment,
                *condition,
                true,
            );
        }
        for (path, interval) in &self.memberships {
            environment.narrow(path.clone(), *interval);
        }
        if environment.intervals.values().any(|interval| {
            interval
                .low
                .zip(interval.high)
                .is_some_and(|(low, high)| low > high)
        }) {
            // An arrival violating requires is rejected by the call-contract
            // checker. Check this body's return under its authored
            // assumptions, without using an empty interval as evidence for a
            // produced value.
            let mut required = ValueEnvironment::new();
            for condition in &self.conditions {
                narrow_environment_by_condition(
                    program,
                    machine,
                    Some(state),
                    &mut required,
                    *condition,
                    true,
                );
            }
            for (path, interval) in &self.memberships {
                required.narrow(path.clone(), *interval);
            }
            *environment = required;
        }
    }
}

/// A machine-level `requires` condition carries into a non-entry state only
/// when the machine can never overwrite a place its recorded facts mention.
/// Re-recording the condition and applying the ordinary write-invalidation
/// law keeps the stability test on exactly the operands the environment
/// would use, `.len` operands included. An opaque write frame or any dropped
/// fact fails closed: the fact stays entry-only, as before.
fn machine_requires_survives(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    machine_written: Option<&[String]>,
    condition: ExpressionHandle,
) -> bool {
    let Some(written) = machine_written else {
        return false;
    };
    let mut seeded = ValueEnvironment::new();
    narrow_environment_by_condition(program, machine, Some(state), &mut seeded, condition, true);
    let mut retained = seeded.clone();
    retained.invalidate_written_paths(written);
    retained == seeded
}

fn condition_belongs_to_state(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    frames: Option<&CallFrameResolver>,
) -> bool {
    if literal_i64(program, expression).is_some() {
        return true;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            // Compiler-owned carrier members have no state-local binder.
            // Recognize their resolved identity before the lexical-owner test;
            // rejecting one would discard unrelated conjuncts over this state's
            // parameters. This grants scope only, not a concrete target value.
            if program.symbols.builtin_type_member(path.symbol).is_some() {
                return true;
            }
            path.symbol.is_valid()
                && path.head_symbol.is_valid()
                && ((program
                    .state_parameter_by_symbol(state, path.head_symbol)
                    .is_some())
                    || (path.symbol == path.head_symbol
                        && program
                            .machine_type_parameters(machine)
                            .iter()
                            .any(|parameter| {
                                parameter.symbol == path.symbol
                                    && matches!(
                                        parameter.kind,
                                        typed_trees::data::TypeParameterKind::Value { .. }
                                            | typed_trees::data::TypeParameterKind::Const { .. }
                                    )
                            })))
        }
        ExpressionNode::Member(member) => {
            // The builtin `.len` selector carries no declared field symbol;
            // admit it through the same structural receiver check
            // `ordered_values::build_operand` uses for its CollectionLength
            // operand.
            (member.member_symbol.is_valid()
                || crate::value_custody::places::collection_length_receiver(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                .is_some())
                && condition_belongs_to_state(program, machine, state, member.receiver, frames)
        }
        ExpressionNode::Indexed(indexed) => {
            // A requirement belongs to this arrival only when both the
            // collection and selector belong to its exact parameter scope.
            // Its live identity and write lifetime are owned by ordered_values.
            crate::place_has_builtin_coordinates(program, machine, Some(state), expression)
                && condition_belongs_to_state(program, machine, state, indexed.collection, frames)
                && condition_belongs_to_state(program, machine, state, indexed.index, frames)
        }
        ExpressionNode::Binary(binary) => {
            condition_belongs_to_state(program, machine, state, binary.left, frames)
                && condition_belongs_to_state(program, machine, state, binary.right, frames)
        }
        // A carrier conversion does not change the operand's lexical owner.
        // Its meaning and formation remain obligations of the bound reader;
        // scope admission alone grants no converted value or qualification.
        ExpressionNode::Cast(cast) => {
            condition_belongs_to_state(program, machine, state, cast.value, frames)
        }
        ExpressionNode::Unary(unary) => {
            condition_belongs_to_state(program, machine, state, unary.operand, frames)
        }
        ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_) => true,
        ExpressionNode::Call(_) => {
            if let Some(application) =
                crate::proof_contracts::collection_predicates::predicate_application(
                    program,
                    machine.symbol,
                    expression,
                )
            {
                // Scope belongs to this exact actual telescope. Predicate
                // consequence checking remains with the existing body explorer;
                // this lexical admission supplies no truth or call equality.
                return application.arguments.iter().all(|argument| {
                    condition_belongs_to_state(program, machine, state, *argument, frames)
                });
            }
            // The proof-only projection has no executable operand, but its
            // lexical scope is exactly its source's scope. Rejecting it here
            // discarded unrelated conjuncts of a state invariant before the
            // narrowing owner could retain their useful ordinary bounds.
            if let Some((_, argument)) = crate::integer_embedding_argument(program, expression) {
                condition_belongs_to_state(program, machine, state, argument, frames)
            } else {
                ordered_values::operand(program, machine, state, expression, frames).is_some()
            }
        }
        _ => false,
    }
}

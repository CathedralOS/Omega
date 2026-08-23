use omega_state_graph::{PlannedTransitionTarget, StateKey};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::name::Identifier;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::{TableCall, TransitionTargetHandle, TransitionTargetNode};
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

use crate::segments::StateSegment;

pub(super) fn plan_transition_target(
    source_key: StateKey,
    segments: &[StateSegment],
    target: TransitionTargetHandle,
    program: &CheckedTrees,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    let source_machine = exact_source_context(program, source_key)?;
    if !target.is_valid() {
        return Ok(PlannedTransitionTarget::Terminal);
    }

    if let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    {
        let members = program.statement_table.name_path_members(path.members);
        if exact_self_sibling_transition(
            source_key,
            members,
            path.head_symbol,
            path.symbol,
            source_machine,
            program,
        ) {
            return Ok(PlannedTransitionTarget::Nested {
                receiver_symbol: path.head_symbol,
                state_symbol: path.symbol,
                receiver: members[0].clone(),
                state: members[1].clone(),
            });
        }
    }

    let local_transition = if let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    {
        is_local_transition_path(
            source_key,
            program.statement_table.name_path_members(path.members),
            path.head_symbol,
            path.symbol,
            source_machine,
            program,
        )?
    } else {
        false
    };

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments: _, ..
        } if local_transition => {
            let members = program.statement_table.name_path_members(path.members);
            let name = members
                .last()
                .expect("named transition has a state")
                .clone();
            let target = resolve_local_target(
                source_key,
                segments,
                path.symbol,
                &name,
                source_machine,
                program,
            )?;
            let Some(target) = target else {
                if members.len() == 2 {
                    return Ok(PlannedTransitionTarget::Nested {
                        receiver_symbol: path.head_symbol,
                        state_symbol: path.symbol,
                        receiver: members[0].clone(),
                        state: members[1].clone(),
                    });
                }

                return Err(Diagnostic::error(format!(
                    "unknown state transition target `{name}`"
                )));
            };

            // Measured recursion MR1 (2026-07-18 ruling, landed 2026-07-11):
            // `-> self.X(..)` resolving to a LOCAL segment. When X is the
            // machine's OWN ENTRY and the machine is MEASURED (`terminates
            // by ...;`), this is the sanctioned TAIL spelling -- it
            // resolves to the SAME loop-back edge as the bare `-> X(..)`
            // (a jump with re-bound arguments; the termination pass already
            // proves the strict decrease across this edge by symbol).
            // Unmeasured stays refused: recursive CALL spellings are legal
            // iff measured; unmeasured repetition is the bare loop, which
            // may legally diverge on constant stack.
            if members.len() == 2 && members[0].as_str() == "self" {
                if !machine_targets_own_entry(source_machine, &name) {
                    return Err(Diagnostic::error(format!(
                        "`self.{name}(..)` in a transition arm targets a sub-state \
                         through a call spelling. Write the state transition bare -- \
                         `-> {name}(..)` -- which is a self-transition LOOP (a jump \
                         with re-bound arguments), not a call.",
                    )));
                }
                if !machine_is_measured(source_machine) {
                    return Err(Diagnostic::error(unmeasured_recursion_message(&name)));
                }
            }

            Ok(PlannedTransitionTarget::State {
                index: target.0,
                key: target.1.key,
                name,
            })
        }
        TransitionTargetNode::Named {
            path, arguments: _, ..
        } => {
            let members = program.statement_table.name_path_members(path.members);
            if members.len() == 2 {
                // A non-local head stays an opaque nested target. In
                // particular, neither a valid foreign symbol nor an invalid
                // head may be repaired into a local state from the `self`
                // spelling alone. Exact local `self.X` paths enter the arm
                // above because their head symbol is the source machine.
                return Ok(PlannedTransitionTarget::Nested {
                    receiver_symbol: path.head_symbol,
                    state_symbol: path.symbol,
                    receiver: members[0].clone(),
                    state: members[1].clone(),
                });
            }

            Err(Diagnostic::error(format!(
                "unsupported transition target `{}`",
                display_transition_path(members)
            )))
        }
        TransitionTargetNode::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => {
            Ok(PlannedTransitionTarget::Terminal)
        }
    }
}

pub(super) fn plan_call_target(
    source_key: StateKey,
    segments: &[StateSegment],
    call: &TableCall,
    program: &CheckedTrees,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    let source_machine = exact_source_context(program, source_key)?;
    let receiver = program.statement_table.name_path_members(call.receiver);

    if receiver.is_empty() || call.receiver_symbol == source_key.machine {
        let name = call.target.clone();
        let target = resolve_local_target(
            source_key,
            segments,
            call.target_symbol,
            &name,
            source_machine,
            program,
        )?;
        let Some(target) = target else {
            if receiver.len() == 1 && receiver[0].as_str() == "self" {
                return Ok(PlannedTransitionTarget::Nested {
                    receiver_symbol: call.receiver_symbol,
                    state_symbol: call.target_symbol,
                    receiver: receiver[0].clone(),
                    state: call.target.clone(),
                });
            }

            return Err(Diagnostic::error(format!(
                "unknown state call target `{name}`"
            )));
        };

        return Ok(PlannedTransitionTarget::State {
            index: target.0,
            key: target.1.key,
            name,
        });
    }

    let receiver = receiver.last().cloned().unwrap_or_default();
    Ok(PlannedTransitionTarget::Nested {
        receiver_symbol: call.receiver_symbol,
        state_symbol: call.target_symbol,
        receiver,
        state: call.target.clone(),
    })
}

pub(super) fn next_segment_target(
    source_key: StateKey,
    segments: &[StateSegment],
) -> Result<PlannedTransitionTarget, Diagnostic> {
    let next_key = StateKey {
        segment_index: source_key.segment_index + 1,
        ..source_key
    };
    let mut targets = segments
        .iter()
        .enumerate()
        .filter(|(_, segment)| segment.key == next_key);
    let target = targets.next().ok_or_else(|| {
        Diagnostic::error("internal state-call continuation segment was not indexed")
    })?;
    if targets.next().is_some() {
        return Err(Diagnostic::error(
            "internal state-call continuation segment was indexed more than once",
        ));
    }

    Ok(PlannedTransitionTarget::State {
        index: target.0,
        key: target.1.key,
        name: target.1.name.clone(),
    })
}

/// MR1: a machine is MEASURED when it declares `terminates` with a
/// `decreases` clause -- the gate that legalizes call-spelled TAIL
/// self-recursion (the termination pass separately PROVES the decrease).
fn machine_is_measured(machine: &Machine) -> bool {
    // TPR3 slice 1: "measured" = carries a ranking witness, read from
    // the normalized plan (decision 23).
    machine.termination_plan.implementation_witness.is_some()
}

/// Does `name` spell the CURRENT machine's own entry (its simple method
/// name)?
fn machine_targets_own_entry(
    machine: &Machine,
    name: &psi_checked_trees::name::Identifier,
) -> bool {
    machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str())
        == name.as_str()
}

fn unmeasured_recursion_message(name: &str) -> String {
    format!(
        "`self.{name}(..)` in a transition arm is call-spelled self-recursion \
         WITHOUT a measure. Recursive call spellings are legal only on a \
         measured machine (`terminates by ...;`; the decrease is \
         proven across the loop edge). Measure the machine, or spell \
         unmeasured repetition as the bare loop `-> {name}(..)` -- a jump \
         with re-bound arguments (constant stack, may diverge)."
    )
}

fn is_local_transition_path(
    source_key: StateKey,
    path: &[psi_checked_trees::name::Identifier],
    head_symbol: psi_symbols::SymbolHandle,
    target_symbol: SymbolHandle,
    source_machine: &Machine,
    program: &CheckedTrees,
) -> Result<bool, Diagnostic> {
    if path.len() == 1 {
        return Ok(true);
    }
    if path.len() != 2 || path[0].as_str() != "self" {
        return Ok(false);
    }
    if head_symbol == source_key.machine {
        return Ok(true);
    }

    // The symbol resolver's exact local-state fallback stamps `self.X` with
    // the target state as both its head and final symbol. Only the current
    // machine's own entry uses that carrier as call-spelled recursion; local
    // substates retain their existing nested routing. Validate ownership
    // before the entry-name discriminator so duplicate or cross-owned local
    // symbols still fail closed here.
    if head_symbol == target_symbol && target_symbol.is_valid() {
        let target = exact_owned_state(program, source_machine, target_symbol, "local target")?;
        return Ok(machine_targets_own_entry(source_machine, &target.name));
    }

    Ok(false)
}

fn exact_self_sibling_transition(
    source_key: StateKey,
    path: &[Identifier],
    head_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    source_machine: &Machine,
    program: &CheckedTrees,
) -> bool {
    // Symbol resolution stamps `self.sibling(..)` with the current machine as
    // its head and the attached sibling's exact state as its final symbol.
    // Only that unique foreign-final shape is nested; every invalid, missing,
    // duplicate, or local coordinate stays on the exact local validator.
    if path.len() != 2
        || path[0].as_str() != "self"
        || head_symbol != source_key.machine
        || !target_symbol.is_valid()
        || program
            .machine_states(source_machine)
            .iter()
            .any(|state| state.symbol == target_symbol)
    {
        return false;
    }

    let mut foreign_states = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol != source_machine.symbol)
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.symbol == target_symbol);
    foreign_states.next().is_some() && foreign_states.next().is_none()
}

fn display_transition_path(path: &[psi_checked_trees::name::Identifier]) -> String {
    let mut display = String::new();

    for member in path {
        if !display.is_empty() {
            display.push('.');
        }
        display.push_str(member.as_str());
    }

    display
}

/// A FREE machine's self-recursive transition (`-> count(s[1..], acc + 1)`
/// inside top-level `machine count`) names the MACHINE, but the machine's
/// implicit body state is the generated `entry` (attached machines name it
/// after the method), so the by-symbol/by-name segment lookups miss. When the
/// single-member target names the source machine itself (no attached data),
/// resolve to its `entry` segment -- a real self-loop back-edge.
fn free_machine_self_entry_segment<'segments>(
    source_key: StateKey,
    segments: &'segments [StateSegment],
    symbol: SymbolHandle,
    name: &Identifier,
    machine: &Machine,
    program: &CheckedTrees,
) -> Result<Option<(usize, &'segments StateSegment)>, Diagnostic> {
    if machine.attached_data.is_some()
        || machine.name.as_str() != name.as_str()
        || symbol.is_valid() && symbol != machine.symbol
    {
        return Ok(None);
    }

    let mut matches = segments.iter().enumerate().filter(|(_, segment)| {
        segment.key.machine == source_key.machine
            && segment.key.segment_index == 0
            && segment.name.as_str() == "entry"
    });
    let target = matches
        .next()
        .ok_or_else(|| Diagnostic::error("free-machine self entry segment was not indexed"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "free-machine self entry segment was indexed more than once",
        ));
    }
    let state = exact_owned_state(program, machine, target.1.key.state, "free-machine entry")?;
    if state.name.as_str() != "entry" {
        return Err(Diagnostic::error(
            "free-machine self entry segment disagrees with its typed state name",
        ));
    }
    Ok(Some(target))
}

fn resolve_local_target<'segments>(
    source_key: StateKey,
    segments: &'segments [StateSegment],
    symbol: SymbolHandle,
    name: &Identifier,
    machine: &Machine,
    program: &CheckedTrees,
) -> Result<Option<(usize, &'segments StateSegment)>, Diagnostic> {
    if let Some(target) =
        free_machine_self_entry_segment(source_key, segments, symbol, name, machine, program)?
    {
        return Ok(Some(target));
    }

    let state = if symbol.is_valid() {
        let state = exact_owned_state(program, machine, symbol, "local target")?;
        if state.name != *name {
            return Err(Diagnostic::error(
                "local target symbol and spelled state name disagree",
            ));
        }
        Some(state)
    } else {
        let mut matches = program
            .machine_states(machine)
            .iter()
            .filter(|state| state.name == *name);
        let state = matches.next();
        if matches.next().is_some() {
            return Err(Diagnostic::error(
                "invalid-symbol local target name is ambiguous",
            ));
        }
        state
    };
    let Some(state) = state else {
        return Ok(None);
    };
    exact_initial_segment(source_key, segments, state).map(Some)
}

fn exact_initial_segment<'segments>(
    source_key: StateKey,
    segments: &'segments [StateSegment],
    state: &State,
) -> Result<(usize, &'segments StateSegment), Diagnostic> {
    let mut matches = segments.iter().enumerate().filter(|(_, segment)| {
        segment.key.machine == source_key.machine
            && segment.key.state == state.symbol
            && segment.key.segment_index == 0
    });
    let target = matches
        .next()
        .ok_or_else(|| Diagnostic::error("exact local target segment was not indexed"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "exact local target segment was indexed more than once",
        ));
    }
    if target.1.name != state.name {
        return Err(Diagnostic::error(
            "exact local target segment name disagrees with its typed state",
        ));
    }
    Ok(target)
}

fn exact_source_context(
    program: &CheckedTrees,
    source_key: StateKey,
) -> Result<&Machine, Diagnostic> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == source_key.machine);
    let machine = machines
        .next()
        .ok_or_else(|| Diagnostic::error("exact transition source machine is missing"))?;
    if machines.next().is_some() {
        return Err(Diagnostic::error(
            "exact transition source machine is duplicated",
        ));
    }
    exact_owned_state(program, machine, source_key.state, "transition source")?;
    Ok(machine)
}

fn exact_owned_state<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    symbol: SymbolHandle,
    role: &str,
) -> Result<&'program State, Diagnostic> {
    let mut matches = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == symbol);
    let state = matches.next();
    if matches.next().is_some() {
        return Err(Diagnostic::error(format!(
            "exact {role} state is duplicated within its machine"
        )));
    }
    if let Some(state) = state {
        if program.machines().iter().any(|candidate| {
            candidate.symbol != machine.symbol
                && program
                    .machine_states(candidate)
                    .iter()
                    .any(|candidate_state| candidate_state.symbol == symbol)
        }) {
            return Err(Diagnostic::error(format!(
                "exact {role} state belongs to more than one machine"
            )));
        }
        return Ok(state);
    }

    let cross_owned = program.machines().iter().any(|candidate| {
        candidate.symbol != machine.symbol
            && program
                .machine_states(candidate)
                .iter()
                .any(|state| state.symbol == symbol)
    });
    Err(Diagnostic::error(if cross_owned {
        format!("exact {role} state belongs to another machine")
    } else {
        format!("exact {role} state is missing")
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_state_graph::{Operation, StateParameterNode};
    use psi_arena::HandleSpan;
    use psi_checked_trees::statement::{TableNamePath, TransitionTargetNode};

    const SOURCE_MACHINE: usize = 1;
    const SOURCE_STATE: usize = 2;
    const TARGET_STATE: usize = 3;
    const FOREIGN_MACHINE: usize = 4;
    const FOREIGN_STATE: usize = 5;

    struct TargetFixture {
        program: CheckedTrees,
        segments: Vec<StateSegment>,
        source_key: StateKey,
        target_state: SymbolHandle,
        foreign_machine: SymbolHandle,
        foreign_state: SymbolHandle,
    }

    fn symbol(index: usize) -> SymbolHandle {
        SymbolHandle::from_arena_index(u32::try_from(index).expect("fixture symbol index overflow"))
    }

    fn push_machine(
        program: &mut CheckedTrees,
        machine_symbol: SymbolHandle,
        machine_name: &str,
        attached_data: Option<&str>,
        states: &[(SymbolHandle, &str)],
    ) {
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated(machine_name),
            attached_data: attached_data.map(Identifier::generated),
            ..Default::default()
        };
        for (state_symbol, state_name) in states {
            program.typed.push_machine_state(
                &mut machine,
                State {
                    symbol: *state_symbol,
                    name: Identifier::generated(*state_name),
                    ..Default::default()
                },
            );
        }
        program.typed.push_machine(machine);
    }

    fn segment(machine: usize, state: usize, segment_index: usize, name: &str) -> StateSegment {
        StateSegment {
            key: StateKey {
                machine: symbol(machine),
                state: symbol(state),
                segment_index,
            },
            name: Identifier::generated(name),
            parameters: HandleSpan::<StateParameterNode>::empty(),
            operations: HandleSpan::<Operation>::empty(),
            transitions: HandleSpan::empty(),
            next_segment_key: StateKey::default(),
        }
    }

    fn target_fixture() -> TargetFixture {
        let mut program = CheckedTrees::default();
        push_machine(
            &mut program,
            symbol(FOREIGN_MACHINE),
            "Other::next",
            Some("Other"),
            &[(symbol(FOREIGN_STATE), "next")],
        );
        push_machine(
            &mut program,
            symbol(SOURCE_MACHINE),
            "Root::run",
            Some("Root"),
            &[
                (symbol(SOURCE_STATE), "run"),
                (symbol(TARGET_STATE), "next"),
            ],
        );
        TargetFixture {
            program,
            segments: vec![
                segment(SOURCE_MACHINE, SOURCE_STATE, 0, "run"),
                segment(SOURCE_MACHINE, TARGET_STATE, 0, "next"),
            ],
            source_key: StateKey {
                machine: symbol(SOURCE_MACHINE),
                state: symbol(SOURCE_STATE),
                segment_index: 0,
            },
            target_state: symbol(TARGET_STATE),
            foreign_machine: symbol(FOREIGN_MACHINE),
            foreign_state: symbol(FOREIGN_STATE),
        }
    }

    fn named_target(
        program: &mut CheckedTrees,
        members: &[&str],
        head_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
    ) -> TransitionTargetHandle {
        let mut path_members = HandleSpan::empty();
        for member in members {
            program
                .typed
                .statement_table
                .push_name_path_member(&mut path_members, Identifier::generated(*member));
        }
        program
            .typed
            .statement_table
            .insert_transition_target(TransitionTargetNode::Named {
                path: TableNamePath {
                    members: path_members,
                    head_symbol,
                    symbol: target_symbol,
                },
                arguments: HandleSpan::empty(),
                evidence_arguments: Box::default(),
            })
    }

    fn state_target_key(target: PlannedTransitionTarget) -> StateKey {
        let PlannedTransitionTarget::State { key, .. } = target else {
            panic!("expected exact state target")
        };
        key
    }

    fn error_message<T: std::fmt::Debug>(result: Result<T, Diagnostic>) -> String {
        result
            .expect_err("invalid target coordinates must fail closed")
            .message
    }

    #[test]
    fn exact_local_transition_and_call_resolve_one_owned_initial_segment() {
        let mut transition = target_fixture();
        let target = named_target(
            &mut transition.program,
            &["next"],
            transition.target_state,
            transition.target_state,
        );
        assert_eq!(
            state_target_key(
                plan_transition_target(
                    transition.source_key,
                    &transition.segments,
                    target,
                    &transition.program,
                )
                .expect("exact local transition"),
            ),
            transition.segments[1].key
        );

        let call = TableCall {
            receiver_symbol: transition.source_key.machine,
            target_symbol: transition.target_state,
            target: Identifier::generated("next"),
            ..Default::default()
        };
        assert_eq!(
            state_target_key(
                plan_call_target(
                    transition.source_key,
                    &transition.segments,
                    &call,
                    &transition.program,
                )
                .expect("exact local call"),
            ),
            transition.segments[1].key
        );
    }

    #[test]
    fn foreign_transition_and_call_targets_remain_opaque_nested_rows() {
        let mut fixture = target_fixture();
        let target = named_target(
            &mut fixture.program,
            &["other", "next"],
            fixture.foreign_machine,
            fixture.foreign_state,
        );
        assert!(matches!(
            plan_transition_target(
                fixture.source_key,
                &fixture.segments,
                target,
                &fixture.program,
            )
            .expect("foreign transition stays nested"),
            PlannedTransitionTarget::Nested {
                receiver_symbol,
                state_symbol,
                ..
            } if receiver_symbol == fixture.foreign_machine
                && state_symbol == fixture.foreign_state
        ));

        let mut receiver = HandleSpan::empty();
        fixture
            .program
            .typed
            .statement_table
            .push_name_path_member(&mut receiver, Identifier::generated("other"));
        let call = TableCall {
            receiver_symbol: fixture.foreign_machine,
            target_symbol: fixture.foreign_state,
            receiver,
            target: Identifier::generated("next"),
            ..Default::default()
        };
        assert!(matches!(
            plan_call_target(
                fixture.source_key,
                &fixture.segments,
                &call,
                &fixture.program,
            )
            .expect("foreign call stays nested"),
            PlannedTransitionTarget::Nested {
                receiver_symbol,
                state_symbol,
                ..
            } if receiver_symbol == fixture.foreign_machine
                && state_symbol == fixture.foreign_state
        ));
    }

    #[test]
    fn self_head_with_exact_foreign_final_remains_an_opaque_nested_row() {
        let mut fixture = target_fixture();
        let target = named_target(
            &mut fixture.program,
            &["self", "next"],
            fixture.source_key.machine,
            fixture.foreign_state,
        );
        assert!(matches!(
            plan_transition_target(
                fixture.source_key,
                &fixture.segments,
                target,
                &fixture.program,
            )
            .expect("self-qualified sibling transition stays nested"),
            PlannedTransitionTarget::Nested {
                receiver_symbol,
                state_symbol,
                ..
            } if receiver_symbol == fixture.source_key.machine
                && state_symbol == fixture.foreign_state
        ));
    }

    #[test]
    fn valid_local_symbol_rejects_cross_owner_and_name_drift_without_fallback() {
        let fixture = target_fixture();
        let cross_owned = TableCall {
            receiver_symbol: fixture.source_key.machine,
            target_symbol: fixture.foreign_state,
            target: Identifier::generated("next"),
            ..Default::default()
        };
        assert!(
            error_message(plan_call_target(
                fixture.source_key,
                &fixture.segments,
                &cross_owned,
                &fixture.program,
            ))
            .contains("belongs to another machine")
        );

        let drifted = TableCall {
            receiver_symbol: fixture.source_key.machine,
            target_symbol: fixture.target_state,
            target: Identifier::generated("run"),
            ..Default::default()
        };
        assert!(
            error_message(plan_call_target(
                fixture.source_key,
                &fixture.segments,
                &drifted,
                &fixture.program,
            ))
            .contains("symbol and spelled state name disagree")
        );
    }

    #[test]
    fn source_and_target_typed_coordinates_must_be_unique() {
        let mut duplicate_machine = target_fixture();
        let source = duplicate_machine
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == duplicate_machine.source_key.machine)
            .expect("source machine")
            .clone();
        duplicate_machine.program.typed.push_machine(source);
        let call = TableCall {
            receiver_symbol: duplicate_machine.source_key.machine,
            target_symbol: duplicate_machine.target_state,
            target: Identifier::generated("next"),
            ..Default::default()
        };
        assert!(
            error_message(plan_call_target(
                duplicate_machine.source_key,
                &duplicate_machine.segments,
                &call,
                &duplicate_machine.program,
            ))
            .contains("source machine is duplicated")
        );

        let mut duplicate_state = target_fixture();
        let machine_index = duplicate_state
            .program
            .typed
            .machines_mut()
            .iter()
            .position(|machine| machine.symbol == duplicate_state.source_key.machine)
            .expect("source machine index");
        let mut machine = duplicate_state.program.machines()[machine_index].clone();
        duplicate_state.program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: duplicate_state.target_state,
                name: Identifier::generated("next"),
                ..Default::default()
            },
        );
        duplicate_state.program.typed.machines_mut()[machine_index] = machine;
        let call = TableCall {
            receiver_symbol: duplicate_state.source_key.machine,
            target_symbol: duplicate_state.target_state,
            target: Identifier::generated("next"),
            ..Default::default()
        };
        assert!(
            error_message(plan_call_target(
                duplicate_state.source_key,
                &duplicate_state.segments,
                &call,
                &duplicate_state.program,
            ))
            .contains("local target state is duplicated")
        );
    }

    #[test]
    fn initial_segment_must_be_exact_unique_and_name_coherent() {
        let fixture = target_fixture();
        let call = TableCall {
            receiver_symbol: fixture.source_key.machine,
            target_symbol: fixture.target_state,
            target: Identifier::generated("next"),
            ..Default::default()
        };
        assert!(
            error_message(plan_call_target(
                fixture.source_key,
                &fixture.segments[..1],
                &call,
                &fixture.program,
            ))
            .contains("segment was not indexed")
        );

        let mut duplicated = fixture.segments.clone();
        duplicated.push(fixture.segments[1].clone());
        assert!(
            error_message(plan_call_target(
                fixture.source_key,
                &duplicated,
                &call,
                &fixture.program,
            ))
            .contains("segment was indexed more than once")
        );

        let mut drifted = fixture.segments.clone();
        drifted[1].name = Identifier::generated("other");
        assert!(
            error_message(plan_call_target(
                fixture.source_key,
                &drifted,
                &call,
                &fixture.program,
            ))
            .contains("segment name disagrees")
        );
    }

    #[test]
    fn invalid_local_name_requires_one_typed_state() {
        let mut fixture = target_fixture();
        let source_machine = fixture
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == fixture.source_key.machine)
            .expect("source machine")
            .clone();
        fixture.program.typed.machine_states_mut(&source_machine)[0].name =
            Identifier::generated("next");
        let call = TableCall {
            receiver_symbol: fixture.source_key.machine,
            target_symbol: SymbolHandle::invalid(),
            target: Identifier::generated("next"),
            ..Default::default()
        };
        assert!(
            error_message(plan_call_target(
                fixture.source_key,
                &fixture.segments,
                &call,
                &fixture.program,
            ))
            .contains("local target name is ambiguous")
        );
    }

    #[test]
    fn continuation_segment_requires_one_exact_next_key() {
        let source_key = StateKey {
            machine: symbol(SOURCE_MACHINE),
            state: symbol(SOURCE_STATE),
            segment_index: 0,
        };
        let next = segment(SOURCE_MACHINE, SOURCE_STATE, 1, "run");
        assert_eq!(
            state_target_key(
                next_segment_target(source_key, std::slice::from_ref(&next))
                    .expect("one next segment"),
            ),
            next.key
        );
        assert!(error_message(next_segment_target(source_key, &[])).contains("was not indexed"));
        assert!(
            error_message(next_segment_target(source_key, &[next.clone(), next]))
                .contains("indexed more than once")
        );
    }

    #[test]
    fn free_entry_and_measured_self_entry_preserve_existing_recursion_rules() {
        let mut free = CheckedTrees::default();
        push_machine(
            &mut free,
            symbol(SOURCE_MACHINE),
            "count",
            None,
            &[(symbol(SOURCE_STATE), "entry")],
        );
        let free_segments = vec![segment(SOURCE_MACHINE, SOURCE_STATE, 0, "entry")];
        let free_key = free_segments[0].key;
        let free_target = named_target(
            &mut free,
            &["count"],
            symbol(SOURCE_MACHINE),
            symbol(SOURCE_MACHINE),
        );
        assert_eq!(
            state_target_key(
                plan_transition_target(free_key, &free_segments, free_target, &free)
                    .expect("free-machine own entry"),
            ),
            free_key
        );

        let mut measured = target_fixture();
        let target = named_target(
            &mut measured.program,
            &["self", "run"],
            measured.source_key.machine,
            measured.source_key.state,
        );
        assert!(
            error_message(plan_transition_target(
                measured.source_key,
                &measured.segments,
                target,
                &measured.program,
            ))
            .contains("WITHOUT a measure")
        );
        measured
            .program
            .typed
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.symbol == measured.source_key.machine)
            .expect("source machine")
            .termination_plan
            .implementation_witness = Some(psi_language_semantics::RankingWitness::default());
        assert_eq!(
            state_target_key(
                plan_transition_target(
                    measured.source_key,
                    &measured.segments,
                    target,
                    &measured.program,
                )
                .expect("measured self entry"),
            ),
            measured.source_key
        );
    }

    #[test]
    fn resolver_local_state_head_preserves_existing_recursion_rules() {
        let mut fixture = target_fixture();
        let target = named_target(
            &mut fixture.program,
            &["self", "run"],
            fixture.source_key.state,
            fixture.source_key.state,
        );
        assert!(
            error_message(plan_transition_target(
                fixture.source_key,
                &fixture.segments,
                target,
                &fixture.program,
            ))
            .contains("WITHOUT a measure")
        );
        fixture
            .program
            .typed
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.symbol == fixture.source_key.machine)
            .expect("source machine")
            .termination_plan
            .implementation_witness = Some(psi_language_semantics::RankingWitness::default());
        assert_eq!(
            state_target_key(
                plan_transition_target(
                    fixture.source_key,
                    &fixture.segments,
                    target,
                    &fixture.program,
                )
                .expect("measured resolver-carrier self entry"),
            ),
            fixture.source_key
        );
    }
}

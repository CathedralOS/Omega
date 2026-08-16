//! Omega-owned boundary-provider admission after Psi semantic checking.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(super) fn check_boundary_provider_approval(
    checked: &psi_checked_trees::CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let program = &checked.typed;
    let calls = checked_boundary_call_coordinates(checked)?;
    let registry = omega_effects::build_boundary_provider_approval_registry(program);
    let unapproved = omega_effects::audit_boundary_provider_calls(program, calls, &registry);

    if unapproved.is_empty() {
        return Ok(());
    }

    Err(unapproved
        .into_iter()
        .map(|call| {
            Diagnostic::error(format!(
                "unapproved boundary call: {} in {} exercises a boundary capability with no approved provider for that exact capability",
                symbol_name(program, call.boundary_trait_symbol),
                symbol_name(program, call.state_symbol),
            ))
        })
        .collect())
}

fn checked_boundary_call_coordinates(
    checked: &psi_checked_trees::CheckedTrees,
) -> Result<Vec<omega_effects::BoundaryCallCoordinate>, Vec<Diagnostic>> {
    let program = &checked.typed;
    let flow = &checked.facts.flow.control;
    let mut diagnostics = Vec::new();
    let mut state_coordinates = Vec::new();
    let mut coordinates = Vec::new();

    let machines = program.machines();
    if machines.len() != program.roots.machines.count() as usize {
        diagnostics.push(Diagnostic::error(
            "provider approval checked flow has an invalid typed machine span",
        ));
    }
    for machine in machines {
        if program.machine_states(machine).len() != machine.states.count() as usize {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow has an invalid typed state span for machine {:?}",
                machine.symbol,
            )));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for (_, flow_state) in flow.states.iter() {
        let state_coordinate = (flow_state.machine_symbol, flow_state.state_symbol);
        if state_coordinates.contains(&state_coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow contains duplicate state coordinate ({:?}, {:?})",
                flow_state.machine_symbol, flow_state.state_symbol,
            )));
            continue;
        }
        state_coordinates.push(state_coordinate);

        let machines = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == flow_state.machine_symbol)
            .collect::<Vec<_>>();
        let machine = match machines.as_slice() {
            [machine] => *machine,
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has no exact typed machine owner {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has duplicate exact typed machine owners {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                continue;
            }
        };

        let state_owners = program
            .machines()
            .iter()
            .flat_map(|candidate_machine| {
                program
                    .machine_states(candidate_machine)
                    .iter()
                    .filter(move |state| state.symbol == flow_state.state_symbol)
                    .map(move |state| (candidate_machine, state))
            })
            .collect::<Vec<_>>();
        let typed_state = match state_owners.as_slice() {
            [(owner, state)] if owner.symbol == machine.symbol => *state,
            [(owner, _)] => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} belongs to typed machine {:?}, not {:?}",
                    flow_state.state_symbol, owner.symbol, flow_state.machine_symbol,
                )));
                continue;
            }
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} is missing from its exact typed machine {:?}",
                    flow_state.state_symbol, flow_state.machine_symbol,
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} has duplicate typed state owners",
                    flow_state.state_symbol,
                )));
                continue;
            }
        };

        let Some(calls) = flow.calls.span(flow_state.calls) else {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked-flow state {:?} has an invalid call span",
                flow_state.state_symbol,
            )));
            continue;
        };
        let statements = program
            .statement_table
            .statements(typed_state.statement_nodes);
        if statements.len() != typed_state.statement_nodes.count() as usize {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked-flow state {:?} has an invalid typed statement span",
                flow_state.state_symbol,
            )));
            continue;
        }
        let statement_count = statements.len();
        let mut call_coordinates = Vec::new();
        for call in calls {
            let call_coordinate = (call.statement_index, call.call_ordinal);
            if call_coordinates.contains(&call_coordinate) {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow state {:?} contains duplicate call coordinate (statement {}, call {})",
                    flow_state.state_symbol, call.statement_index, call.call_ordinal,
                )));
                continue;
            }
            call_coordinates.push(call_coordinate);
            if call.statement_index >= statement_count {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has out-of-range statement index {} for {} typed statements",
                    flow_state.state_symbol, call.statement_index, statement_count,
                )));
                continue;
            }
            let Some(boundary_edges) = checked
                .facts
                .flow
                .boundaries
                .edges
                .span(call.boundary_edges)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has an invalid boundary-edge span",
                    flow_state.state_symbol,
                )));
                continue;
            };
            if !call.target_symbol.is_valid() {
                if boundary_edges.is_empty() {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "provider approval checked-flow call in state {:?} has no valid target symbol",
                    flow_state.state_symbol,
                )));
                continue;
            }
            coordinates.push(omega_effects::BoundaryCallCoordinate {
                machine_symbol: flow_state.machine_symbol,
                state_symbol: flow_state.state_symbol,
                target_state_symbol: call.target_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            });
        }
    }

    if diagnostics.is_empty() {
        Ok(coordinates)
    } else {
        Err(diagnostics)
    }
}

fn symbol_name(program: &psi_typed_trees::TypedTrees, symbol: SymbolHandle) -> String {
    if !symbol.is_valid() {
        return "unknown".to_owned();
    }

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        return machine.name.as_str().to_owned();
    }

    if let Some(state) = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.symbol == symbol)
    {
        return state.name.as_str().to_owned();
    }

    program.symbols.name(symbol).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CoordinateFixture {
        checked: psi_checked_trees::CheckedTrees,
        flow_state: psi_arena::Handle<psi_checked_trees::FlowStateFact>,
        first_call: psi_arena::Handle<psi_checked_trees::FlowCallFact>,
        second_call: psi_arena::Handle<psi_checked_trees::FlowCallFact>,
        machine: SymbolHandle,
        state: SymbolHandle,
        other_machine: SymbolHandle,
        first_target: SymbolHandle,
        second_target: SymbolHandle,
    }

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn coordinate_fixture() -> CoordinateFixture {
        let machine = symbol(1);
        let state = symbol(2);
        let other_machine = symbol(3);
        let other_state = symbol(4);
        let first_target = symbol(5);
        let second_target = symbol(6);
        let mut checked = psi_checked_trees::CheckedTrees::default();

        for (machine_symbol, state_symbol, machine_name) in [
            (machine, state, "Main"),
            (other_machine, other_state, "Other"),
        ] {
            let mut machine_definition = psi_checked_trees::machine::Machine {
                symbol: machine_symbol,
                name: psi_checked_trees::name::Identifier::generated(machine_name),
                ..Default::default()
            };
            let mut state_definition = psi_checked_trees::state::State {
                symbol: state_symbol,
                name: psi_checked_trees::name::Identifier::generated("run"),
                ..Default::default()
            };
            checked
                .typed
                .statement_table
                .push_statement(&mut state_definition.statement_nodes, Default::default());
            checked
                .typed
                .push_machine_state(&mut machine_definition, state_definition);
            checked.typed.push_machine(machine_definition);
        }

        let mut calls = psi_arena::HandleSpan::empty();
        let first_call = checked.facts.flow.control.calls.append_to_span(
            &mut calls,
            psi_checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: first_target,
                ..Default::default()
            },
        );
        let second_call = checked.facts.flow.control.calls.append_to_span(
            &mut calls,
            psi_checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 1,
                target_symbol: second_target,
                ..Default::default()
            },
        );
        let flow_state =
            checked
                .facts
                .flow
                .control
                .states
                .append(psi_checked_trees::FlowStateFact {
                    machine_symbol: machine,
                    state_symbol: state,
                    calls,
                    ..Default::default()
                });

        CoordinateFixture {
            checked,
            flow_state,
            first_call,
            second_call,
            machine,
            state,
            other_machine,
            first_target,
            second_target,
        }
    }

    #[test]
    fn checked_coordinates_preserve_exact_order_and_identity() {
        let mut fixture = coordinate_fixture();

        assert_eq!(
            checked_boundary_call_coordinates(&fixture.checked).expect("valid checked flow"),
            vec![
                omega_effects::BoundaryCallCoordinate {
                    machine_symbol: fixture.machine,
                    state_symbol: fixture.state,
                    target_state_symbol: fixture.first_target,
                    statement_index: 0,
                    call_ordinal: 0,
                },
                omega_effects::BoundaryCallCoordinate {
                    machine_symbol: fixture.machine,
                    state_symbol: fixture.state,
                    target_state_symbol: fixture.second_target,
                    statement_index: 0,
                    call_ordinal: 1,
                },
            ]
        );
        assert_eq!(check_boundary_provider_approval(&fixture.checked), Ok(()));

        fixture
            .checked
            .facts
            .flow
            .control
            .calls
            .get_mut(fixture.first_call)
            .target_symbol = SymbolHandle::invalid();
        assert_eq!(
            checked_boundary_call_coordinates(&fixture.checked)
                .expect("non-boundary call without a resolved target remains ignorable"),
            vec![omega_effects::BoundaryCallCoordinate {
                machine_symbol: fixture.machine,
                state_symbol: fixture.state,
                target_state_symbol: fixture.second_target,
                statement_index: 0,
                call_ordinal: 1,
            }]
        );
    }

    #[derive(Clone, Copy)]
    enum CoordinateCorruption {
        InvalidCallSpan,
        InvalidBoundaryEdgeSpan,
        InvalidMachineSpan,
        InvalidStateSpan,
        InvalidStatementSpan,
        MissingMachine,
        DuplicateMachine,
        MissingState,
        CrossOwnerState,
        DuplicateState,
        DuplicateFlowState,
        OutOfRangeStatement,
        DuplicateCallCoordinate,
        InvalidTarget,
    }

    #[test]
    fn checked_coordinates_fail_closed_on_invalid_custody() {
        let cases = [
            (CoordinateCorruption::InvalidCallSpan, "invalid call span"),
            (
                CoordinateCorruption::InvalidBoundaryEdgeSpan,
                "invalid boundary-edge span",
            ),
            (
                CoordinateCorruption::InvalidMachineSpan,
                "invalid typed machine span",
            ),
            (
                CoordinateCorruption::InvalidStateSpan,
                "invalid typed state span",
            ),
            (
                CoordinateCorruption::InvalidStatementSpan,
                "invalid typed statement span",
            ),
            (
                CoordinateCorruption::MissingMachine,
                "no exact typed machine owner",
            ),
            (
                CoordinateCorruption::DuplicateMachine,
                "duplicate exact typed machine owners",
            ),
            (
                CoordinateCorruption::MissingState,
                "missing from its exact typed machine",
            ),
            (
                CoordinateCorruption::CrossOwnerState,
                "belongs to typed machine",
            ),
            (
                CoordinateCorruption::DuplicateState,
                "duplicate typed state owners",
            ),
            (
                CoordinateCorruption::DuplicateFlowState,
                "duplicate state coordinate",
            ),
            (
                CoordinateCorruption::OutOfRangeStatement,
                "out-of-range statement index",
            ),
            (
                CoordinateCorruption::DuplicateCallCoordinate,
                "duplicate call coordinate",
            ),
            (
                CoordinateCorruption::InvalidTarget,
                "no valid target symbol",
            ),
        ];

        for (corruption, expected) in cases {
            let mut fixture = coordinate_fixture();
            match corruption {
                CoordinateCorruption::InvalidCallSpan => {
                    fixture.checked.facts.flow.control.calls.clear();
                }
                CoordinateCorruption::InvalidBoundaryEdgeSpan => {
                    attach_boundary_edge(&mut fixture);
                    fixture.checked.facts.flow.boundaries.edges.clear();
                }
                CoordinateCorruption::InvalidMachineSpan => {
                    fixture.checked.typed.machines.clear();
                }
                CoordinateCorruption::InvalidStateSpan => {
                    fixture.checked.typed.machine_states.clear();
                }
                CoordinateCorruption::InvalidStatementSpan => {
                    fixture.checked.typed.statement_table = Default::default();
                }
                CoordinateCorruption::MissingMachine => {
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .states
                        .get_mut(fixture.flow_state)
                        .machine_symbol = symbol(90);
                }
                CoordinateCorruption::DuplicateMachine => {
                    fixture.checked.typed.machines.for_each_mut(|_, machine| {
                        if machine.symbol == fixture.other_machine {
                            machine.symbol = fixture.machine;
                        }
                    });
                }
                CoordinateCorruption::MissingState => {
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .states
                        .get_mut(fixture.flow_state)
                        .state_symbol = symbol(91);
                }
                CoordinateCorruption::CrossOwnerState => {
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .states
                        .get_mut(fixture.flow_state)
                        .machine_symbol = fixture.other_machine;
                }
                CoordinateCorruption::DuplicateState => {
                    let other = fixture
                        .checked
                        .typed
                        .machines()
                        .iter()
                        .find(|machine| machine.symbol == fixture.other_machine)
                        .expect("other typed machine")
                        .clone();
                    fixture.checked.typed.machine_states_mut(&other)[0].symbol = fixture.state;
                }
                CoordinateCorruption::DuplicateFlowState => {
                    let duplicate = fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .states
                        .get(fixture.flow_state)
                        .clone();
                    fixture.checked.facts.flow.control.states.append(duplicate);
                }
                CoordinateCorruption::OutOfRangeStatement => {
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(fixture.first_call)
                        .statement_index = 1;
                }
                CoordinateCorruption::DuplicateCallCoordinate => {
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(fixture.second_call)
                        .call_ordinal = 0;
                }
                CoordinateCorruption::InvalidTarget => {
                    attach_boundary_edge(&mut fixture);
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(fixture.first_call)
                        .target_symbol = SymbolHandle::invalid();
                }
            }

            let diagnostics = checked_boundary_call_coordinates(&fixture.checked)
                .expect_err("invalid coordinate custody must fail closed");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "expected diagnostic containing {expected:?}, got {diagnostics:?}",
            );
        }
    }

    fn attach_boundary_edge(fixture: &mut CoordinateFixture) {
        let mut boundary_edges = psi_arena::HandleSpan::empty();
        fixture.checked.facts.flow.boundaries.edges.append_to_span(
            &mut boundary_edges,
            psi_checked_trees::FlowBoundaryEdgeFact {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: fixture.first_target,
                boundary_trait_symbol: symbol(7),
                boundary_signature_symbol: symbol(8),
                ..Default::default()
            },
        );
        fixture
            .checked
            .facts
            .flow
            .control
            .calls
            .get_mut(fixture.first_call)
            .boundary_edges = boundary_edges;
    }
}

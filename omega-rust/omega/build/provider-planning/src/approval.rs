//! Omega-owned boundary-provider admission after Psi semantic checking.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub fn check_boundary_provider_approval(
    checked: &checked_trees::CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let program = &checked.typed;
    let calls = checked_boundary_call_coordinates(checked)?;
    let registry = effects::build_boundary_provider_approval_registry(program);
    let unapproved = effects::audit_boundary_provider_calls(program, calls, &registry);

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
    checked: &checked_trees::CheckedTrees,
) -> Result<Vec<effects::BoundaryCallCoordinate>, Vec<Diagnostic>> {
    let program = &checked.typed;
    let flow = &checked.facts.flow.control;
    let mut diagnostics = Vec::new();
    let mut state_coordinates = Vec::new();
    let mut coordinates = Vec::new();

    let traits = program.traits();
    if traits.len() != program.roots.traits.count() as usize {
        diagnostics.push(Diagnostic::error(
            "provider approval checked flow has an invalid typed trait span",
        ));
    }
    for definition in traits {
        if program.trait_machine_signatures(definition).len()
            != definition.machines.count() as usize
        {
            diagnostics.push(Diagnostic::error(format!(
                "provider approval checked flow has an invalid typed signature span for trait {:?}",
                definition.symbol,
            )));
        }
    }
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
            if boundary_edges.is_empty() {
                match exact_direct_boundary_requirement(program, call.target_symbol) {
                    Ok(Some((boundary_trait_symbol, boundary_signature_symbol))) => {
                        coordinates.push(effects::BoundaryCallCoordinate {
                            machine_symbol: flow_state.machine_symbol,
                            state_symbol: flow_state.state_symbol,
                            target_state_symbol: call.target_symbol,
                            boundary_trait_symbol,
                            boundary_signature_symbol,
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                        });
                    }
                    Ok(None) => {}
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
                continue;
            }

            let mut exact_edges = Vec::new();
            for edge in boundary_edges {
                if edge.statement_index != call.statement_index
                    || edge.call_ordinal != call.call_ordinal
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked boundary edge in state {:?} has coordinate (statement {}, call {}), not its owning call coordinate (statement {}, call {})",
                        flow_state.state_symbol,
                        edge.statement_index,
                        edge.call_ordinal,
                        call.statement_index,
                        call.call_ordinal,
                    )));
                    continue;
                }
                if edge.target_symbol != call.target_symbol {
                    diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked boundary edge in state {:?} targets {:?}, not its owning call target {:?}",
                        flow_state.state_symbol, edge.target_symbol, call.target_symbol,
                    )));
                    continue;
                }
                let edge_identity = (edge.boundary_trait_symbol, edge.boundary_signature_symbol);
                if exact_edges.contains(&edge_identity) {
                    diagnostics.push(Diagnostic::error(format!(
                        "provider approval checked call in state {:?} contains duplicate exact boundary edge ({:?}, {:?})",
                        flow_state.state_symbol,
                        edge.boundary_trait_symbol,
                        edge.boundary_signature_symbol,
                    )));
                    continue;
                }
                exact_edges.push(edge_identity);
                if let Err(diagnostic) = validate_exact_boundary_requirement(
                    program,
                    edge.boundary_trait_symbol,
                    edge.boundary_signature_symbol,
                    "provider approval checked boundary edge",
                ) {
                    diagnostics.push(diagnostic);
                    continue;
                }
                coordinates.push(effects::BoundaryCallCoordinate {
                    machine_symbol: flow_state.machine_symbol,
                    state_symbol: flow_state.state_symbol,
                    target_state_symbol: call.target_symbol,
                    boundary_trait_symbol: edge.boundary_trait_symbol,
                    boundary_signature_symbol: edge.boundary_signature_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                });
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(coordinates)
    } else {
        Err(diagnostics)
    }
}

fn exact_direct_boundary_requirement(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> Result<Option<(SymbolHandle, SymbolHandle)>, Diagnostic> {
    let matches = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == target_symbol)
                .map(move |signature| (definition.symbol, signature.symbol))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [(boundary_trait_symbol, boundary_signature_symbol)] => {
            validate_exact_boundary_requirement(
                program,
                *boundary_trait_symbol,
                *boundary_signature_symbol,
                "provider approval direct abstract boundary call",
            )?;
            Ok(Some((*boundary_trait_symbol, *boundary_signature_symbol)))
        }
        _ => Err(Diagnostic::error(format!(
            "provider approval call target {:?} resolves to {} exact direct boundary requirements",
            target_symbol,
            matches.len(),
        ))),
    }
}

fn validate_exact_boundary_requirement(
    program: &typed_trees::TypedTrees,
    boundary_trait_symbol: SymbolHandle,
    boundary_signature_symbol: SymbolHandle,
    context: &str,
) -> Result<(), Diagnostic> {
    if !boundary_trait_symbol.is_valid() || !boundary_signature_symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "{context} has an invalid exact boundary trait or signature symbol",
        )));
    }
    let owners = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == boundary_trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "{context} trait {:?} resolves to {} exact typed owners",
            boundary_trait_symbol,
            owners.len(),
        )));
    };
    if !owner.is_boundary {
        return Err(Diagnostic::error(format!(
            "{context} trait {:?} is not a boundary trait",
            boundary_trait_symbol,
        )));
    }
    let owned_signatures = program
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| signature.symbol == boundary_signature_symbol)
        .count();
    let global_signatures = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .filter(|signature| signature.symbol == boundary_signature_symbol)
        .count();
    let state_matches = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.symbol == boundary_signature_symbol)
        .count();
    if owned_signatures != 1 || global_signatures != 1 || state_matches != 0 {
        return Err(Diagnostic::error(format!(
            "{context} signature {:?} is missing, duplicated, cross-owned, or collides with a typed state",
            boundary_signature_symbol,
        )));
    }
    Ok(())
}

fn symbol_name(program: &typed_trees::TypedTrees, symbol: SymbolHandle) -> String {
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
        checked: checked_trees::CheckedTrees,
        flow_state: arena::Handle<checked_trees::FlowStateFact>,
        first_call: arena::Handle<checked_trees::FlowCallFact>,
        second_call: arena::Handle<checked_trees::FlowCallFact>,
        machine: SymbolHandle,
        state: SymbolHandle,
        other_machine: SymbolHandle,
        first_target: SymbolHandle,
        second_target: SymbolHandle,
        boundary_trait: SymbolHandle,
        boundary_signature: SymbolHandle,
        other_boundary_trait: SymbolHandle,
        other_boundary_signature: SymbolHandle,
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
        let boundary_trait = symbol(7);
        let boundary_signature = symbol(8);
        let other_boundary_trait = symbol(9);
        let other_boundary_signature = symbol(10);
        let mut checked = checked_trees::CheckedTrees::default();

        for (trait_symbol, signature_symbol, trait_name) in [
            (boundary_trait, boundary_signature, "Console"),
            (other_boundary_trait, other_boundary_signature, "Clock"),
        ] {
            let mut definition = checked_trees::trait_definition::TraitDefinition {
                symbol: trait_symbol,
                is_boundary: true,
                name: checked_trees::name::Identifier::generated(trait_name),
                ..Default::default()
            };
            checked.typed.push_trait_machine_signature(
                &mut definition,
                checked_trees::signature::StateSignature {
                    symbol: signature_symbol,
                    name: checked_trees::name::Identifier::generated("invoke"),
                    ..Default::default()
                },
            );
            checked.typed.push_trait_definition(definition);
        }

        for (machine_symbol, state_symbol, machine_name) in [
            (machine, state, "Main"),
            (other_machine, other_state, "Other"),
        ] {
            let mut machine_definition = checked_trees::machine::Machine {
                symbol: machine_symbol,
                name: checked_trees::name::Identifier::generated(machine_name),
                ..Default::default()
            };
            let mut state_definition = checked_trees::state::State {
                symbol: state_symbol,
                name: checked_trees::name::Identifier::generated("run"),
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

        let mut calls = arena::HandleSpan::empty();
        let first_call = checked.facts.flow.control.calls.append_to_span(
            &mut calls,
            checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: first_target,
                ..Default::default()
            },
        );
        let second_call = checked.facts.flow.control.calls.append_to_span(
            &mut calls,
            checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 1,
                target_symbol: second_target,
                ..Default::default()
            },
        );
        let flow_state = checked
            .facts
            .flow
            .control
            .states
            .append(checked_trees::FlowStateFact {
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
            boundary_trait,
            boundary_signature,
            other_boundary_trait,
            other_boundary_signature,
        }
    }

    #[test]
    fn checked_coordinates_preserve_exact_order_and_identity() {
        let mut fixture = coordinate_fixture();
        let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
        attach_boundary_edge(&mut fixture, owner, requirement);
        let direct_requirement = fixture.other_boundary_signature;
        fixture
            .checked
            .facts
            .flow
            .control
            .calls
            .get_mut(fixture.second_call)
            .target_symbol = direct_requirement;

        assert_eq!(
            checked_boundary_call_coordinates(&fixture.checked).expect("valid checked flow"),
            vec![
                effects::BoundaryCallCoordinate {
                    machine_symbol: fixture.machine,
                    state_symbol: fixture.state,
                    target_state_symbol: fixture.first_target,
                    boundary_trait_symbol: fixture.boundary_trait,
                    boundary_signature_symbol: fixture.boundary_signature,
                    statement_index: 0,
                    call_ordinal: 0,
                },
                effects::BoundaryCallCoordinate {
                    machine_symbol: fixture.machine,
                    state_symbol: fixture.state,
                    target_state_symbol: fixture.other_boundary_signature,
                    boundary_trait_symbol: fixture.other_boundary_trait,
                    boundary_signature_symbol: fixture.other_boundary_signature,
                    statement_index: 0,
                    call_ordinal: 1,
                },
            ]
        );
        assert_eq!(check_boundary_provider_approval(&fixture.checked), Ok(()));
    }

    #[test]
    fn checked_coordinates_ignore_genuine_non_boundary_calls() {
        let mut fixture = coordinate_fixture();

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
            Vec::<effects::BoundaryCallCoordinate>::new(),
        );
    }

    #[test]
    fn checked_coordinates_retain_every_distinct_exact_boundary_edge() {
        let mut fixture = coordinate_fixture();
        let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
        attach_boundary_edge(&mut fixture, owner, requirement);
        let (owner, requirement) = (
            fixture.other_boundary_trait,
            fixture.other_boundary_signature,
        );
        attach_boundary_edge(&mut fixture, owner, requirement);

        let coordinates =
            checked_boundary_call_coordinates(&fixture.checked).expect("two exact boundary edges");
        assert_eq!(coordinates.len(), 2);
        assert_eq!(
            coordinates
                .iter()
                .map(|coordinate| (
                    coordinate.boundary_trait_symbol,
                    coordinate.boundary_signature_symbol,
                ))
                .collect::<Vec<_>>(),
            vec![
                (fixture.boundary_trait, fixture.boundary_signature),
                (
                    fixture.other_boundary_trait,
                    fixture.other_boundary_signature,
                ),
            ],
        );
    }

    #[derive(Clone, Copy)]
    enum CoordinateCorruption {
        InvalidCallSpan,
        InvalidBoundaryEdgeSpan,
        InvalidTraitSpan,
        InvalidSignatureSpan,
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
        DriftedEdgeStatement,
        DriftedEdgeOrdinal,
        DriftedEdgeTarget,
        InvalidBoundaryTrait,
        MissingBoundaryTrait,
        NonBoundaryTrait,
        InvalidBoundarySignature,
        MissingBoundarySignature,
        CrossOwnedBoundarySignature,
        DuplicateExactBoundaryEdge,
        DuplicateDirectBoundaryRequirement,
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
                CoordinateCorruption::InvalidTraitSpan,
                "invalid typed trait span",
            ),
            (
                CoordinateCorruption::InvalidSignatureSpan,
                "invalid typed signature span",
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
            (
                CoordinateCorruption::DriftedEdgeStatement,
                "not its owning call coordinate",
            ),
            (
                CoordinateCorruption::DriftedEdgeOrdinal,
                "not its owning call coordinate",
            ),
            (
                CoordinateCorruption::DriftedEdgeTarget,
                "not its owning call target",
            ),
            (
                CoordinateCorruption::InvalidBoundaryTrait,
                "invalid exact boundary trait or signature symbol",
            ),
            (
                CoordinateCorruption::MissingBoundaryTrait,
                "resolves to 0 exact typed owners",
            ),
            (
                CoordinateCorruption::NonBoundaryTrait,
                "is not a boundary trait",
            ),
            (
                CoordinateCorruption::InvalidBoundarySignature,
                "invalid exact boundary trait or signature symbol",
            ),
            (
                CoordinateCorruption::MissingBoundarySignature,
                "missing, duplicated, cross-owned, or collides",
            ),
            (
                CoordinateCorruption::CrossOwnedBoundarySignature,
                "missing, duplicated, cross-owned, or collides",
            ),
            (
                CoordinateCorruption::DuplicateExactBoundaryEdge,
                "duplicate exact boundary edge",
            ),
            (
                CoordinateCorruption::DuplicateDirectBoundaryRequirement,
                "resolves to 2 exact direct boundary requirements",
            ),
        ];

        for (corruption, expected) in cases {
            let mut fixture = coordinate_fixture();
            match corruption {
                CoordinateCorruption::InvalidCallSpan => {
                    fixture.checked.facts.flow.control.calls.clear();
                }
                CoordinateCorruption::InvalidBoundaryEdgeSpan => {
                    let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
                    attach_boundary_edge(&mut fixture, owner, requirement);
                    fixture.checked.facts.flow.boundaries.edges.clear();
                }
                CoordinateCorruption::InvalidTraitSpan => {
                    fixture.checked.typed.traits.clear();
                }
                CoordinateCorruption::InvalidSignatureSpan => {
                    fixture.checked.typed.trait_machine_signatures.clear();
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
                    let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
                    attach_boundary_edge(&mut fixture, owner, requirement);
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(fixture.first_call)
                        .target_symbol = SymbolHandle::invalid();
                }
                CoordinateCorruption::DriftedEdgeStatement => {
                    let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
                    let edge = attach_boundary_edge(&mut fixture, owner, requirement);
                    fixture
                        .checked
                        .facts
                        .flow
                        .boundaries
                        .edges
                        .get_mut(edge)
                        .statement_index = 1;
                }
                CoordinateCorruption::DriftedEdgeOrdinal => {
                    let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
                    let edge = attach_boundary_edge(&mut fixture, owner, requirement);
                    fixture
                        .checked
                        .facts
                        .flow
                        .boundaries
                        .edges
                        .get_mut(edge)
                        .call_ordinal = 1;
                }
                CoordinateCorruption::DriftedEdgeTarget => {
                    let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
                    let edge = attach_boundary_edge(&mut fixture, owner, requirement);
                    let drifted_target = fixture.second_target;
                    fixture
                        .checked
                        .facts
                        .flow
                        .boundaries
                        .edges
                        .get_mut(edge)
                        .target_symbol = drifted_target;
                }
                CoordinateCorruption::InvalidBoundaryTrait => {
                    let requirement = fixture.boundary_signature;
                    attach_boundary_edge(&mut fixture, SymbolHandle::invalid(), requirement);
                }
                CoordinateCorruption::MissingBoundaryTrait => {
                    let requirement = fixture.boundary_signature;
                    attach_boundary_edge(&mut fixture, symbol(90), requirement);
                }
                CoordinateCorruption::NonBoundaryTrait => {
                    let boundary_trait = fixture.boundary_trait;
                    fixture.checked.typed.traits.for_each_mut(|_, definition| {
                        if definition.symbol == boundary_trait {
                            definition.is_boundary = false;
                        }
                    });
                    let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
                    attach_boundary_edge(&mut fixture, owner, requirement);
                }
                CoordinateCorruption::InvalidBoundarySignature => {
                    let owner = fixture.boundary_trait;
                    attach_boundary_edge(&mut fixture, owner, SymbolHandle::invalid());
                }
                CoordinateCorruption::MissingBoundarySignature => {
                    let owner = fixture.boundary_trait;
                    attach_boundary_edge(&mut fixture, owner, symbol(90));
                }
                CoordinateCorruption::CrossOwnedBoundarySignature => {
                    let (owner, requirement) =
                        (fixture.boundary_trait, fixture.other_boundary_signature);
                    attach_boundary_edge(&mut fixture, owner, requirement);
                }
                CoordinateCorruption::DuplicateExactBoundaryEdge => {
                    let (owner, requirement) = (fixture.boundary_trait, fixture.boundary_signature);
                    attach_boundary_edge(&mut fixture, owner, requirement);
                    attach_boundary_edge(&mut fixture, owner, requirement);
                }
                CoordinateCorruption::DuplicateDirectBoundaryRequirement => {
                    let boundary_signature = fixture.boundary_signature;
                    let other_boundary_signature = fixture.other_boundary_signature;
                    fixture
                        .checked
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(fixture.first_call)
                        .target_symbol = boundary_signature;
                    fixture
                        .checked
                        .typed
                        .trait_machine_signatures
                        .for_each_mut(|_, signature| {
                            if signature.symbol == other_boundary_signature {
                                signature.symbol = boundary_signature;
                            }
                        });
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

    fn attach_boundary_edge(
        fixture: &mut CoordinateFixture,
        boundary_trait_symbol: SymbolHandle,
        boundary_signature_symbol: SymbolHandle,
    ) -> arena::Handle<checked_trees::FlowBoundaryEdgeFact> {
        let mut boundary_edges = fixture
            .checked
            .facts
            .flow
            .control
            .calls
            .get(fixture.first_call)
            .boundary_edges;
        let edge = fixture.checked.facts.flow.boundaries.edges.append_to_span(
            &mut boundary_edges,
            checked_trees::FlowBoundaryEdgeFact {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: fixture.first_target,
                boundary_trait_symbol,
                boundary_signature_symbol,
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
        edge
    }
}

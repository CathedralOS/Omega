//! Provider approval tests.

use super::{SymbolHandle, check_boundary_provider_approval, checked_boundary_call_coordinates};

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

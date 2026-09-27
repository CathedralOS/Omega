use super::{AdmissionProfile, encode_module, encode_proof_section};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{BlockId, StructuralPlaceKind, StructuralTypeId};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_psi::{
    Block, OperationKind, StructuralAccess, StructuralArgument, SuccessorEdge, TerminalMachine,
    TerminalModule, Terminator,
};
use terminal_verifier::{ModuleError, validate_module};

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    machine relay(first: &[u8], left: u8, second: &[u8], right: u8, selected: bool) reaches Output {
        Output::write(first, left);
        transition selected {
            true -> joined(second, left, first, right)
            false -> joined(first, right, second, left)
        }
        state joined(first: &[u8], left: u8, second: &[u8], right: u8) {
            Output::write(first, left);
            Output::write(second, right);
            transition { _ -> finished(second, right, first, left) }
        }
        state finished(first: &[u8], left: u8, second: &[u8], right: u8) {}
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80A", 1u8, "different", 2u8, true);
        relay("", 3u8, "\x80B", 4u8, false);
        Output::write("last", 5u8);
    }
"#;

fn lowered(source: &str) -> lowered_psi::LoweredPsi {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("source view bindings lower");
    validate_module(&lowered.semantic_module).expect("mutation baseline validates");
    lowered
}

fn is_graph(machine: &TerminalMachine) -> bool {
    machine.structural_parameters.len() == 2
        && machine.blocks.iter().any(|block| {
            block.id == machine.entry && matches!(block.terminator, Terminator::Conditional { .. })
        })
}

fn graph_mut(module: &mut TerminalModule) -> &mut TerminalMachine {
    module
        .machines
        .iter_mut()
        .find(|machine| is_graph(machine))
        .expect("relay graph")
}

fn entry_edges(machine: &TerminalMachine) -> (&SuccessorEdge, &SuccessorEdge) {
    let block = machine
        .blocks
        .iter()
        .find(|block| block.id == machine.entry)
        .unwrap();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &block.terminator
    else {
        panic!("direct conditional entry");
    };
    (when_true, when_false)
}

fn first_edge_mut(machine: &mut TerminalMachine) -> &mut SuccessorEdge {
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| block.id == machine.entry)
        .unwrap();
    let Terminator::Conditional { when_true, .. } = &mut block.terminator else {
        panic!("direct conditional entry");
    };
    when_true
}

fn joined_mut(machine: &mut TerminalMachine) -> &mut Block {
    let target = entry_edges(machine).0.target;
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == target)
        .unwrap()
}

fn reject(module: &TerminalModule, expected: ModuleError) {
    let Err(actual) = validate_module(module) else {
        panic!("malformed bindings unexpectedly validated");
    };
    assert_eq!(actual, expected);
}

#[test]
fn block_parameter_declarations_reject_wrong_position_self_type_and_kind() {
    let base = lowered(SOURCE).semantic_module;
    for mutation in ["position", "self", "type", "kind", "owner"] {
        let mut changed = base.clone();
        let machine = graph_mut(&mut changed);
        let joined = joined_mut(machine);
        let block = joined.id;
        let declaration = &mut joined.structural_parameters[0];
        let place = declaration.place;
        match mutation {
            "position" => declaration.position = 2,
            "self" => declaration.is_self = true,
            "type" => declaration.structural_type = StructuralTypeId::new(u64::MAX).unwrap(),
            "kind" | "owner" => {
                let declaration = machine
                    .structural_places
                    .iter_mut()
                    .find(|declaration| declaration.id == place)
                    .unwrap();
                declaration.kind = if mutation == "kind" {
                    StructuralPlaceKind::Parameter {
                        position: 99,
                        is_self: false,
                    }
                } else {
                    StructuralPlaceKind::BlockParameter {
                        block: BlockId::new(u64::MAX).unwrap(),
                        position: 0,
                    }
                };
            }
            _ => panic!("unknown mutation"),
        }
        reject(
            &changed,
            ModuleError::InvalidBlockStructuralParameter { block, place },
        );
    }
}

#[test]
fn orphan_block_parameter_place_rejects() {
    let mut changed = lowered(SOURCE).semantic_module;
    let joined = joined_mut(graph_mut(&mut changed));
    let block = joined.id;
    let place = joined.structural_parameters.pop().unwrap().place;
    reject(
        &changed,
        ModuleError::InvalidBlockStructuralParameter { block, place },
    );
}

#[test]
fn structural_successor_rejects_unknown_target() {
    let mut changed = lowered(SOURCE).semantic_module;
    let target = BlockId::new(u64::MAX).unwrap();
    first_edge_mut(graph_mut(&mut changed)).target = target;
    reject(&changed, ModuleError::UnknownTargetBlock(target));
}

#[test]
fn later_block_view_is_not_available_in_entry_operations_or_edges() {
    let base = lowered(SOURCE).semantic_module;
    for operation_use in [true, false] {
        let mut changed = base.clone();
        let machine = graph_mut(&mut changed);
        let place = joined_mut(machine).structural_parameters[0].place;
        let expected = if operation_use {
            let entry = machine
                .blocks
                .iter_mut()
                .find(|block| block.id == machine.entry)
                .unwrap();
            let operation = entry
                .operations
                .iter_mut()
                .find(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
                .unwrap();
            let OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } = &mut operation.kind
            else {
                panic!("output call");
            };
            structural_arguments[0].place = place;
            ModuleError::ByteSequenceViewNotEstablished {
                operation: operation.id,
                place,
            }
        } else {
            let successor = first_edge_mut(machine);
            successor.structural_arguments[0].place = place;
            ModuleError::InvalidStructuralSuccessorArgument {
                edge: successor.edge,
                place,
            }
        };
        reject(&changed, expected);
    }
}

#[test]
fn unranked_self_bindings_validate_without_claiming_finite_fuel() {
    let lowered = lowered(SOURCE);
    let mut changed = lowered.semantic_module;
    let machine = graph_mut(&mut changed);
    let finished = machine
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator, Terminator::ReturnUnit { .. }))
        .unwrap();
    let Terminator::ReturnUnit { edge, .. } = finished.terminator else {
        panic!("finished returns");
    };
    let block = finished.id;
    finished.terminator = Terminator::Jump {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge,
        target: block,
        arguments: finished
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect(),
        structural_arguments: finished
            .structural_parameters
            .iter()
            .map(|parameter| StructuralArgument {
                place: parameter.place,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            })
            .collect(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    // The productive unranked self-loop verifies, but fixed-fuel derivation
    // refuses rather than invent a ceiling: the directed report names the
    // verifier-derived component identity and the Unranked cause — not
    // whichever block the traversal happened to revisit.
    let expected_component = terminal_verifier::cyclic_component_identity(machine, &[block]);
    let verified = terminal_verifier::verify_module(
        &changed,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("an unchanged immutable view may be carried by a productive loop");
    assert!(matches!(
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, changed.entry),
        Err(terminal_fixed_fuel::FixedFuelError::UnboundedCycleComponent {
            component,
            cause: terminal_fixed_fuel::UnboundedCycleCause::Unranked,
        }) if component == expected_component
    ));
    let mut execution = TerminalExecution::start_artifact(
        &encode_module(&changed).unwrap(),
        &encode_proof_section(&changed, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(100);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    let prefix = execution.effects().to_vec();
    assert_eq!(
        prefix.len(),
        3,
        "only the first relay runs before its endless final state"
    );
    meter.replenish(100).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.effects(), prefix);
}

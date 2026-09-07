use super::*;
use semantic_vocabulary::{BlockId, IntegerValue, StructuralPlaceKind, StructuralTypeId};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus, TerminalScalarValue};
use terminal_psi::{
    Block, ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralArgument,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalMachine, TerminalModule, Terminator,
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
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked(source), "Root::enter")
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

fn graph(module: &TerminalModule) -> &TerminalMachine {
    module
        .machines
        .iter()
        .find(|machine| is_graph(machine))
        .expect("relay graph")
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
fn direct_conditional_join_selects_two_views_and_independent_scalar_permutations() {
    let lowered = lowered(SOURCE);
    let machine = graph(&lowered.semantic_module);
    let (when_true, when_false) = entry_edges(machine);
    assert_eq!(
        when_true.target, when_false.target,
        "both arms directly enter the same join"
    );
    let joined = machine
        .blocks
        .iter()
        .find(|block| block.id == when_true.target)
        .unwrap();
    let first = machine.structural_parameters[0].place;
    let second = machine.structural_parameters[1].place;
    assert_eq!(
        when_true
            .structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect::<Vec<_>>(),
        [second, first]
    );
    assert_eq!(
        when_false
            .structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect::<Vec<_>>(),
        [first, second]
    );
    assert_eq!(
        when_true.arguments,
        [machine.parameters[0].id, machine.parameters[1].id]
    );
    assert_eq!(
        when_false.arguments,
        [machine.parameters[1].id, machine.parameters[0].id]
    );
    assert_eq!(joined.structural_parameters.len(), 2);
    for (position, parameter) in joined.structural_parameters.iter().enumerate() {
        assert_eq!(
            parameter.position as usize, position,
            "public positions are dense despite interleaved source scalars"
        );
        assert!(!parameter.is_self);
        assert_eq!(
            parameter.structural_type,
            machine.structural_parameters[position].structural_type
        );
        assert_eq!(
            machine
                .structural_places
                .iter()
                .find(|place| place.id == parameter.place)
                .unwrap()
                .kind,
            StructuralPlaceKind::BlockParameter {
                block: joined.id,
                position: position as u32
            }
        );
    }
    assert!(
        machine
            .blocks
            .iter()
            .find(|block| block.id == machine.entry)
            .unwrap()
            .structural_parameters
            .is_empty()
    );

    let execution = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
    )
    .expect("serialized source module and proof execute");
    assert_eq!(execution.value(), TerminalExecutionResult::Unit);
    let output = execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall {
                byte_sequence_arguments,
                arguments,
                ..
            } = effect
            else {
                panic!("one output boundary effect");
            };
            let [Some(bytes)] = byte_sequence_arguments.as_slice() else {
                panic!("one whole byte view");
            };
            let [
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(marker),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned marker");
            };
            (bytes.clone(), *marker)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        output,
        vec![
            (b"\x80A".to_vec(), 1),
            (b"different".to_vec(), 1),
            (b"\x80A".to_vec(), 2),
            (Vec::new(), 3),
            (Vec::new(), 4),
            (b"\x80B".to_vec(), 3),
            (b"last".to_vec(), 5),
        ]
    );
}

#[test]
fn one_unit_resume_preserves_selected_views_and_emits_each_call_once() {
    let lowered = lowered(SOURCE);
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let profile = AdmissionProfile::default();
    let decoded_module = terminal_codec::decode_module(&semantic).unwrap();
    let decoded_proof = terminal_codec::decode_proof_bundle(&proof).unwrap();
    let verified = terminal_verifier::verify_module(&decoded_module, &decoded_proof, &profile)
        .expect("serialized acyclic bindings verify under ordinary execution");
    let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, decoded_module.entry)
        .expect("acyclic structural transfers retain the existing logical fuel schedule");
    let unlimited = interpret_terminal_artifact_measured(&semantic, &proof, &profile, &[]).unwrap();
    assert!(unlimited.usage().total_units() <= certificate.ceiling_units());
    assert_eq!(
        unlimited.usage().total_units(),
        certificate.ceiling_units(),
        "both direct arms enter the same body and attain the maximal route bound"
    );
    let mut execution =
        TerminalExecution::start_artifact(&semantic, &proof, &profile, &[]).unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(1);
    let mut completed = false;
    for _ in 0..=unlimited.usage().total_units() {
        let status = execution.resume(&mut meter).unwrap();
        assert!(
            unlimited.effects().starts_with(execution.effects()),
            "every pause preserves the exact effect prefix"
        );
        match status {
            TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                assert_eq!(exhaustion.required_units, 1);
                assert_eq!(exhaustion.remaining_units, 0);
                meter.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(value) => {
                assert_eq!(value, TerminalExecutionResult::Unit);
                completed = true;
                break;
            }
            TerminalExecutionStatus::Crashed(crash) => panic!("unexpected crash: {crash:?}"),
        }
    }
    assert!(
        completed,
        "one-unit resumes complete within the unlimited charge count"
    );
    assert_eq!(execution.effects(), unlimited.effects());
    assert_eq!(meter.usage().total_units(), unlimited.usage().total_units());
    assert_eq!(meter.usage().total_units(), certificate.ceiling_units());
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        execution.effects(),
        unlimited.effects(),
        "resuming completion never repeats a boundary"
    );
    assert_eq!(meter.usage().total_units(), unlimited.usage().total_units());
}

#[test]
fn structural_successors_reject_missing_and_surplus_arguments() {
    let base = lowered(SOURCE).semantic_module;
    for conditional in [true, false] {
        for surplus in [false, true] {
            let mut changed = base.clone();
            let machine = graph_mut(&mut changed);
            let (edge, arguments) = if conditional {
                let successor = first_edge_mut(machine);
                (successor.edge, &mut successor.structural_arguments)
            } else {
                let Terminator::Jump {
                    edge,
                    structural_arguments,
                    ..
                } = &mut joined_mut(machine).terminator
                else {
                    panic!("joined state jumps");
                };
                (*edge, structural_arguments)
            };
            if surplus {
                arguments.push(arguments[0].clone());
            } else {
                arguments.pop();
            }
            let actual = arguments.len();
            reject(
                &changed,
                ModuleError::StructuralJumpArityMismatch {
                    edge,
                    expected: 2,
                    actual,
                },
            );
        }
    }
}

#[test]
fn structural_successors_reject_nonshared_access_and_projection() {
    let base = lowered(SOURCE).semantic_module;
    for access in [
        StructuralAccess::Owned,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut changed = base.clone();
        let successor = first_edge_mut(graph_mut(&mut changed));
        successor.structural_arguments[0].access = access;
        let expected = ModuleError::InvalidStructuralSuccessorArgument {
            edge: successor.edge,
            place: successor.structural_arguments[0].place,
        };
        reject(&changed, expected);
    }
    let mut changed = base;
    let successor = first_edge_mut(graph_mut(&mut changed));
    successor.structural_arguments[0]
        .path
        .push(StructuralPathSegment::FixedIndex(0));
    let expected = ModuleError::InvalidStructuralSuccessorArgument {
        edge: successor.edge,
        place: successor.structural_arguments[0].place,
    };
    reject(&changed, expected);
}

#[test]
fn structural_successor_requires_exact_type_identity() {
    let mut changed = lowered(SOURCE).semantic_module;
    let different_type = StructuralTypeId::new(u64::MAX).unwrap();
    changed.structural_types.push(StructuralTypeDeclaration {
        id: different_type,
        identity: "test::different_byte_view".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    });
    let machine = graph_mut(&mut changed);
    let Terminator::Jump {
        edge,
        target,
        structural_arguments,
        ..
    } = &joined_mut(machine).terminator
    else {
        panic!("joined state jumps");
    };
    let expected = ModuleError::InvalidStructuralSuccessorArgument {
        edge: *edge,
        place: structural_arguments[0].place,
    };
    let target = *target;
    // Only the target declaration changes: the supplied argument still has the original type.
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == target)
        .unwrap()
        .structural_parameters[0]
        .structural_type = different_type;
    reject(&changed, expected);
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

const SIBLING_SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    machine relay(first: &[u8], second: &[u8], selected: bool) reaches Output {
        transition selected { true -> left(first) false -> right(second) }
        state left(bytes: &[u8]) {
            Output::write(bytes, 1u8);
            transition { _ -> finished(bytes) }
        }
        state right(bytes: &[u8]) {
            Output::write(bytes, 2u8);
            transition { _ -> finished(bytes) }
        }
        state finished(bytes: &[u8]) {}
    }
    data Root {}
    machine Root::enter() reaches Output { relay("\x80", "", true); }
"#;

#[test]
fn sibling_block_view_is_not_available_in_operations_or_edges() {
    let base = lowered(SIBLING_SOURCE).semantic_module;
    for operation_use in [true, false] {
        let mut changed = base.clone();
        let machine = graph_mut(&mut changed);
        let (when_true, when_false) = entry_edges(machine);
        let left = when_true.target;
        let right = when_false.target;
        let place = machine
            .blocks
            .iter()
            .find(|block| block.id == right)
            .unwrap()
            .structural_parameters[0]
            .place;
        let left = machine
            .blocks
            .iter_mut()
            .find(|block| block.id == left)
            .unwrap();
        let expected = if operation_use {
            let operation = left
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
            let Terminator::Jump {
                edge,
                structural_arguments,
                ..
            } = &mut left.terminator
            else {
                panic!("left jumps to finish");
            };
            structural_arguments[0].place = place;
            ModuleError::InvalidStructuralSuccessorArgument { edge: *edge, place }
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
    let verified = terminal_verifier::verify_module(
        &changed,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("an unchanged immutable view may be carried by a productive loop");
    assert!(
        matches!(terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, changed.entry),
        Err(terminal_fixed_fuel::FixedFuelError::ControlCycle(actual)) if actual == block)
    );
    let mut execution = TerminalExecution::start_artifact(
        &encode_module(&changed).unwrap(),
        &encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(), &[],
    ).unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(100);
    assert!(matches!(execution.resume(&mut meter).unwrap(), TerminalExecutionStatus::SponsorExhausted(_)));
    let prefix = execution.effects().to_vec();
    assert_eq!(prefix.len(), 3, "only the first relay runs before its endless final state");
    meter.replenish(100).unwrap();
    assert!(matches!(execution.resume(&mut meter).unwrap(), TerminalExecutionStatus::SponsorExhausted(_)));
    assert_eq!(execution.effects(), prefix);
}

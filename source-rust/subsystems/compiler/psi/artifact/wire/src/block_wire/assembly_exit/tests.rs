use super::*;
use crate::module_wire::encode_raw;
use crate::semantic_module::entry_tests::unit_machine;
use crate::{decode_module, encode_module};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, RegisterTargetApplicability, SuccessorEdge,
    TerminalModule, ValueDeclaration,
};

fn register(slot: usize) -> AsmRegisterView {
    AsmRegisterView::from_scratch_slot(slot).unwrap()
}

fn declaration(id: u64, ty: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type: ty,
        qualifications: Default::default(),
    }
}

fn unsigned(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap())
}

fn edge_module(conditional: bool) -> TerminalModule {
    let machine_id = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine_id);
    let mut machine = unit_machine(machine_id);
    machine.assembly_target_requirement = RegisterTargetApplicability::X86_64;
    let target = BlockId::new(2).unwrap();
    let entry = &mut machine.blocks[0];
    let first = declaration(1, unsigned(32));
    let second = declaration(2, unsigned(32));
    let arguments = vec![first.id, second.id];
    for (index, value) in [first, second].into_iter().enumerate() {
        entry.operations.push(Operation::unsuspended(
            OperationId::new(index as u64 + 1).unwrap(),
            OperationResult::Scalar(value),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(index as u128),
            },
        ));
    }
    let edge = EdgeId::new(1).unwrap();
    let taken_target = BlockId::new(3).unwrap();
    entry.terminator = if conditional {
        let condition = declaration(5, ScalarType::Boolean);
        let id = condition.id;
        entry.operations.push(Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Scalar(condition),
            OperationKind::BooleanConstant { value: true },
        ));
        let fallthrough = SuccessorEdge {
            edge,
            target,
            arguments,
            erased_arguments: vec![],
            erased_proof_arguments: vec![],
            structural_arguments: vec![],
            cleanup_actions: vec![],
        };
        Terminator::Conditional {
            condition: id,
            when_true: SuccessorEdge {
                edge: EdgeId::new(3).unwrap(),
                target: taken_target,
                arguments: vec![],
                ..fallthrough.clone()
            },
            when_false: fallthrough,
        }
    } else {
        Terminator::Jump {
            edge,
            target,
            arguments,
            erased_arguments: vec![],
            erased_proof_arguments: vec![],
            structural_arguments: vec![],
            cleanup_actions: vec![],
        }
    };
    entry.assembly_exit = Some(AssemblyExitConstraint::Adjacent {
        target,
        registers: vec![register(2), register(0)],
    });
    machine.blocks.push(Block::plain(
        target,
        vec![declaration(3, unsigned(32)), declaration(4, unsigned(32))],
        Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: vec![],
        },
    ));
    if conditional {
        machine.blocks.push(Block::plain(
            taken_target,
            vec![],
            Terminator::ReturnUnit {
                edge: EdgeId::new(4).unwrap(),
                cleanup_actions: vec![],
            },
        ));
    }
    module.machines.push(machine);
    module
}

#[test]
fn received_jump_and_conditional_preserve_register_order_on_the_adjacent_edge() {
    for conditional in [false, true] {
        let module = edge_module(conditional);
        let bytes = encode_module(&module).unwrap();
        let received = decode_module(&bytes).unwrap();
        assert_eq!(received, module);
        assert_eq!(encode_module(&received).unwrap(), bytes);
    }
}

#[test]
fn malformed_register_edges_fail_at_receipt() {
    for (slots, target, conditional) in [
        (vec![0], 2, false),
        (vec![0, 0], 2, true),
        (vec![0, 22], 2, false),
        (vec![0, 2], 3, true),
        (vec![0, 2], 17, false),
    ] {
        let mut module = edge_module(conditional);
        module.machines[0].blocks[0].assembly_exit = Some(AssemblyExitConstraint::Adjacent {
            target: BlockId::new(target).unwrap(),
            registers: slots.into_iter().map(register).collect(),
        });
        assert!(matches!(
            decode_module(&encode_raw(&module).unwrap()),
            Err(CodecError::MalformedStructuralFoundation(_))
        ));
    }
}

#[test]
fn both_ends_of_register_bindings_require_available_exact_width_values() {
    let mut module = edge_module(false);
    let blocks = &mut module.machines[0].blocks;
    // A successor formal is unavailable in its predecessor.
    let successor_value = blocks[1].parameters[0].id;
    let Terminator::Jump { arguments, .. } = &mut blocks[0].terminator else {
        unreachable!()
    };
    arguments[0] = successor_value;
    assert!(decode_module(&encode_raw(&module).unwrap()).is_err());

    let mut module = edge_module(false);
    let blocks = &mut module.machines[0].blocks;
    let OperationResult::Scalar(value) = &mut blocks[0].operations[0].result else {
        unreachable!()
    };
    value.scalar_type = unsigned(16);
    blocks[1].parameters[0].scalar_type = unsigned(16);
    assert!(matches!(
        decode_module(&encode_raw(&module).unwrap()),
        Err(CodecError::MalformedStructuralFoundation(_))
    ));
}

#[test]
fn unused_register_slots_reject_at_wire_ingress() {
    for slot in [14, 21, 51, 255] {
        assert!(matches!(AssemblyExit::read(&mut Reader::new(&[2, slot])),
            Err(CodecError::InvalidTag("AsmRegisterView", invalid)) if invalid == slot));
    }
}

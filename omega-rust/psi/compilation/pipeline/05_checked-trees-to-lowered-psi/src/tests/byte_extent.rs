//! The writer's strict-decrease obligation, exercised as an actual byte read.

use super::lower_machine;
use crate::TerminalMachineSelection;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::terminal_identities::{obligation_id, operation_id, value_id};
use checked_trees::types::PrimitiveType;
use lowered_psi::LoweredPsi;
use semantic_vocabulary::IntegerValue;
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};
mod rejections;

const SOURCE: &str = r#"
    boundary trait Output { machine write(value: u8) reaches Output; }
    machine relay(bytes: &[u8]) reaches Output {
        transition bytes.len > 0 {
            true -> tail(bytes[1..])
            false -> empty()
        }
        state tail(bytes: &[u8]) { Output::write(1u8); }
        state empty() {}
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80AB"); relay(""); relay("Z"); Output::write(7u8);
    }
"#;

/// Use the source-produced acyclic call graph as fixture scaffolding. The
/// added Terminal read tests a proof obligation, not source correspondence.
fn fixture() -> LoweredPsi {
    let mut lowered = lower_machine(
        &crate::front_end::checked_program(SOURCE),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .unwrap();
    let machine = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| {
            machine.blocks.iter().any(|block| {
                block.operations.iter().any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                })
            })
        })
        .unwrap();
    let slice = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
        .unwrap()
        .clone();
    let OperationKind::ByteSequenceSubslice { source, length, .. } = slice.kind else {
        panic!("subslice fixture");
    };
    let tail = slice.result.structural().unwrap().place;
    let next_operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .map(|operation| operation.id.get())
        .max()
        .unwrap()
        + 1;
    let next_value = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.scalar().map(|value| value.id.get()))
        .max()
        .unwrap()
        + 1;
    let extent = value_id(next_value);
    let byte = value_id(next_value + 1);
    let obligation = obligation_id(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .map(|evidence| evidence.obligation.get())
            .max()
            .unwrap()
            + 1,
    );
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| operation.id == slice.id)
        })
        .unwrap();
    block.operations.extend([
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(next_operation),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: extent,
                scalar_type: terminal_scalar_type(PrimitiveType::U64).unwrap(),
            }),
            kind: OperationKind::ByteSequenceLength { source: tail },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id(next_operation + 1),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: byte,
                scalar_type: terminal_scalar_type(PrimitiveType::U8).unwrap(),
            }),
            // Its canonical bounds goal is tail.len < original.len: exactly
            // strict descent, not a fresh guard asserting the desired result.
            kind: OperationKind::ByteSequenceRead {
                source,
                index: extent,
                length,
                obligation,
            },
        },
    ]);
    for operation in machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::BoundaryCall { arguments, .. } = &mut operation.kind {
            *arguments = vec![byte];
        }
    }
    lowered.proof_bundle.evidence.clear();
    lowered
}

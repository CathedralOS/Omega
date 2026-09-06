//! Immutable typed indexes used while reconstructing one terminal machine.

use std::collections::BTreeMap;

use semantic_vocabulary::{BlockId, MachineId, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{Block, OperationKind, TerminalMachine, TerminalModule, Terminator};

pub(super) struct MachineReconstructionContext<'a> {
    pub(super) reconstruct_path_facts: bool,
    pub(super) value_types: BTreeMap<ValueId, ScalarType>,
    pub(super) blocks: BTreeMap<BlockId, &'a Block>,
    pub(super) machines: BTreeMap<MachineId, &'a TerminalMachine>,
}

impl<'a> MachineReconstructionContext<'a> {
    pub(super) fn new(
        module: &'a TerminalModule,
        machine: &'a TerminalMachine,
        crash_facts: bool,
    ) -> Self {
        let reconstruct_path_facts = machine.blocks.iter().any(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    &operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
                        | OperationKind::CallDynamicScalar { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::IntegerExactCast { .. }
                        | OperationKind::ExactIntegerShiftLeft { .. }
                        | OperationKind::ExactIntegerShiftRight { .. }
                        | OperationKind::ExactIntegerAdd { .. }
                        | OperationKind::ExactIntegerSubtract { .. }
                        | OperationKind::ExactIntegerMultiply { .. }
                        | OperationKind::ExactIntegerDivide { .. }
                        | OperationKind::ExactIntegerRemainder { .. }
                        | OperationKind::WrappingIntegerDivide { .. }
                        | OperationKind::WrappingIntegerRemainder { .. }
                        | OperationKind::SaturatingIntegerDivide { .. }
                        | OperationKind::SaturatingIntegerRemainder { .. }
                )
            })
        }) || (crash_facts && machine.blocks.iter().any(|block| {
            matches!(&block.terminator, Terminator::Crash { site_guard, .. } if !site_guard.is_empty())
        }));
        let value_types = machine
            .parameters
            .iter()
            .chain(machine.result.scalar_ref())
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.parameters.iter()),
            )
            .chain(machine.blocks.iter().flat_map(|block| {
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar_ref())
            }))
            .map(|declaration| (declaration.id, declaration.scalar_type))
            .collect::<BTreeMap<_, _>>();
        let blocks = machine
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect::<BTreeMap<_, _>>();
        let machines = module
            .machines
            .iter()
            .map(|machine| (machine.id, machine))
            .collect::<BTreeMap<_, _>>();
        Self {
            reconstruct_path_facts,
            value_types,
            blocks,
            machines,
        }
    }

    pub(super) fn value_term(&self, id: ValueId) -> ScalarTerm {
        ScalarTerm::value(
            id,
            *self
                .value_types
                .get(&id)
                .expect("validated module contains every referenced value"),
        )
    }
}

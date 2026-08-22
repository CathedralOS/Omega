//! Immutable typed indexes used while reconstructing one terminal machine.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{BlockId, MachineId, PropositionContext, ScalarTerm, ScalarType, ValueId};
use psi_terminal::{Block, OperationKind, TerminalMachine, TerminalModule};

use crate::ModuleError;

pub(super) struct MachineReconstructionContext<'a> {
    pub(super) reconstruct_path_facts: bool,
    pub(super) value_types: BTreeMap<ValueId, ScalarType>,
    pub(super) proposition_context: PropositionContext,
    pub(super) machine_parameter_values: BTreeSet<ValueId>,
    pub(super) blocks: BTreeMap<BlockId, &'a Block>,
    pub(super) machines: BTreeMap<MachineId, &'a TerminalMachine>,
}

impl<'a> MachineReconstructionContext<'a> {
    pub(super) fn new(
        module: &'a TerminalModule,
        machine: &'a TerminalMachine,
    ) -> Result<Self, ModuleError> {
        let reconstruct_path_facts = machine.blocks.iter().any(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    &operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::CallUnit { .. }
                        | OperationKind::CallStructuralScalar { .. }
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
        });
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
        let proposition_context = PropositionContext::from_value_types(
            value_types
                .iter()
                .map(|(&id, &scalar_type)| (id, scalar_type)),
        )
        .map_err(ModuleError::MalformedProposition)?;
        let machine_parameter_values = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<BTreeSet<_>>();
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
        Ok(Self {
            reconstruct_path_facts,
            value_types,
            proposition_context,
            machine_parameter_values,
            blocks,
            machines,
        })
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

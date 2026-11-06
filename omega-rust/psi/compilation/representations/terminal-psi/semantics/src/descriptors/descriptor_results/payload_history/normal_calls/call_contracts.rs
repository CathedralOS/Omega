//! Calls with the same module can share their declared requests and names.
//!
//! These are immutable lookup rows, not call-entry facts or completed proofs.
//! Sharing them must not share another invocation's mutable payload summaries.

use super::*;
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use terminal_psi::{
    BoundaryLenderPreservationDemand, BoundaryMachineDeclaration, BoundaryReferenceForwardingDemand,
};

pub(super) struct CallContracts<'module> {
    pub(super) module: &'module TerminalModule,
    pub(super) machines: Vec<(MachineId, usize)>,
    declarations: Vec<&'module BoundaryMachineDeclaration>,
    lenders: Vec<&'module BoundaryLenderPreservationDemand>,
    forwarding: Vec<&'module BoundaryReferenceForwardingDemand>,
    blocks: Vec<(
        usize,
        semantic_vocabulary::BlockId,
        &'module terminal_psi::Block,
    )>,
}

impl<'module> CallContracts<'module> {
    pub(super) fn new(module: &'module TerminalModule) -> Self {
        let mut machines = module
            .machines
            .iter()
            .enumerate()
            .map(|(ordinal, body)| (body.id, ordinal))
            .collect::<Vec<_>>();
        machines.sort_unstable();
        let mut declarations = module.boundary_machines.iter().collect::<Vec<_>>();
        declarations.sort_by_key(|declaration| declaration.id);
        let mut lenders = module
            .boundary_lender_preservation
            .iter()
            .collect::<Vec<_>>();
        lenders.sort_by_key(|demand| (demand.machine, demand.operation));
        let mut forwarding = module
            .boundary_reference_forwarding
            .iter()
            .collect::<Vec<_>>();
        forwarding.sort_by_key(|demand| (demand.machine, demand.operation));
        let mut blocks = module
            .machines
            .iter()
            .enumerate()
            .flat_map(|(ordinal, body)| {
                body.blocks
                    .iter()
                    .map(move |block| (ordinal, block.id, block))
            })
            .collect::<Vec<_>>();
        blocks.sort_by_key(|(ordinal, identity, _)| (*ordinal, *identity));
        Self {
            module,
            machines,
            declarations,
            lenders,
            forwarding,
            blocks,
        }
    }

    pub(super) fn operation(
        &self,
        machine: MachineId,
        position: DescriptorPosition,
    ) -> Option<&'module Operation> {
        let (_, machine_ordinal) = self.machines(machine).first()?;
        let key = (*machine_ordinal, position.0);
        let index = self
            .blocks
            .partition_point(|(ordinal, block, _)| (*ordinal, *block) < key);
        let (ordinal, block, body) = self.blocks.get(index)?;
        if (*ordinal, *block) != key {
            return None;
        }
        // Duplicate block identities keep the original first-block lookup.
        body.operations.get(position.1 as usize)
    }

    pub(super) fn machines(&self, identity: MachineId) -> &[(MachineId, usize)] {
        let start = self
            .machines
            .partition_point(|(found, _)| *found < identity);
        let end = self
            .machines
            .partition_point(|(found, _)| *found <= identity);
        &self.machines[start..end]
    }

    pub(super) fn declarations(
        &self,
        identity: BoundaryMachineId,
    ) -> &[&'module BoundaryMachineDeclaration] {
        let start = self
            .declarations
            .partition_point(|declaration| declaration.id < identity);
        let end = self
            .declarations
            .partition_point(|declaration| declaration.id <= identity);
        &self.declarations[start..end]
    }

    pub(super) fn first_declaration(
        &self,
        identity: BoundaryMachineId,
    ) -> Option<&'module BoundaryMachineDeclaration> {
        self.declarations(identity).first().copied()
    }

    pub(super) fn lenders(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> impl Iterator<Item = &'module BoundaryLenderPreservationDemand> {
        let key = (machine, operation);
        let start = self
            .lenders
            .partition_point(|demand| (demand.machine, demand.operation) < key);
        let end = self
            .lenders
            .partition_point(|demand| (demand.machine, demand.operation) <= key);
        self.lenders[start..end].iter().copied()
    }

    pub(super) fn forwarding(
        &self,
        machine: MachineId,
        operation: OperationId,
    ) -> impl Iterator<Item = &'module BoundaryReferenceForwardingDemand> {
        let key = (machine, operation);
        let start = self
            .forwarding
            .partition_point(|demand| (demand.machine, demand.operation) < key);
        let end = self
            .forwarding
            .partition_point(|demand| (demand.machine, demand.operation) <= key);
        self.forwarding[start..end].iter().copied()
    }
}

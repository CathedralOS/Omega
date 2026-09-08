//! Functions, source and implementation blocks, and explicitly identified successor edges.
use super::{SelectedBlockId, SelectedInstruction, VirtualRegister, VirtualRegisterId};
use abstract_operations::ValueBinding;
use optimization_unit::FuelSettlement;
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub ranked: Option<abstract_operations::RankedU32CountdownCustody>,
    pub structural: Option<legalized_operations::LegalizedStructuralContract>,
    pub local_storage_slots: Vec<super::SelectedLocalStorageSlot>,
    pub outgoing_arguments: Vec<super::SelectedOutgoingArgumentSlot>,
    pub calls: Vec<super::SelectedCallContract>,
    pub memory_accesses: Vec<super::SelectedMemoryAccess>,
    pub boundary_settlements: Vec<super::SelectedBoundarySettlement>,
    pub entry_block: SelectedBlockId,
    pub virtual_registers: Vec<VirtualRegister>,
    pub blocks: Vec<SelectedBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBlock {
    pub id: SelectedBlockId,
    pub origin: SelectedBlockOrigin,
    pub instructions: Vec<SelectedInstruction>,
    pub terminator: SelectedTerminator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedBlockOrigin {
    Source(BlockId),
    /// Copies for one authored edge; the target is semantic lineage, not a
    /// claim that this implementation block exists in Terminal Psi.
    EdgeTransfer {
        edge: EdgeId,
        target: BlockId,
    },
}

impl SelectedBlock {
    /// Semantic anchor; an implementation block anchors its destination.
    pub const fn source_block(&self) -> BlockId {
        match self.origin {
            SelectedBlockOrigin::Source(block) => block,
            SelectedBlockOrigin::EdgeTransfer { target, .. } => target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedSuccessorRole {
    Semantic,
    /// Completes the physical transfer after its semantic edge was selected.
    /// It retains the edge's identity but carries no second fuel charge.
    EdgeTransferContinuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedTerminator {
    /// Unconditional control, with semantic versus implementation role on the
    /// successor. Argument transfers are not inferred from physical fallthrough.
    Jump {
        instruction: SelectedInstruction,
        successor: SelectedSuccessor,
    },
    ConditionalBranch {
        instruction: SelectedInstruction,
        when_nonzero: SelectedSuccessor,
        when_zero: SelectedSuccessor,
    },
    /// Predicate-aware unsigned-U64 control. Keeping the semantic successors
    /// distinct prevents later persistence and allocation stages from having
    /// to infer ordering meaning from a generic nonzero/zero branch.
    ConditionalBranchU64LessThan {
        instruction: SelectedInstruction,
        when_less: SelectedSuccessor,
        when_not_less: SelectedSuccessor,
    },
    /// Predicate-aware signed-I64 strict-less-than control.
    ConditionalBranchI64LessThan {
        instruction: SelectedInstruction,
        when_less: SelectedSuccessor,
        when_not_less: SelectedSuccessor,
    },
    Return {
        instruction: SelectedInstruction,
        psi_return_edge: EdgeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSuccessor {
    pub role: SelectedSuccessorRole,
    pub psi_edge: EdgeId,
    pub block: SelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<SelectedValueBinding>,
    /// Path-specific logical fuel for this exact semantic edge.
    pub fuel: Vec<FuelSettlement>,
}

/// A source transfer and its selected realization. Source identity alone cannot
/// choose between a durable value and ABI temporaries carrying the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedValueBinding {
    pub semantic: ValueBinding,
    pub transport: SelectedValueTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedValueTransport {
    /// The destination retains the semantic parameter but never reads it.
    Unused,
    Registers {
        argument: VirtualRegisterId,
        parameter: VirtualRegisterId,
    },
}

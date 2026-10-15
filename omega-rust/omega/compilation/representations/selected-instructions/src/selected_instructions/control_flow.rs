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
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<semantic_vocabulary::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub structural: Option<legalized_operations::LegalizedStructuralContract>,
    pub local_storage_slots: Vec<super::SelectedLocalStorageSlot>,
    pub outgoing_arguments: Vec<super::SelectedOutgoingArgumentSlot>,
    pub calls: Vec<super::SelectedCallContract>,
    pub normalized_foreign_calls: Vec<super::SelectedNormalizedForeignCall>,
    pub memory_accesses: Vec<super::SelectedMemoryAccess>,
    pub boundary_settlements: Vec<super::SelectedBoundarySettlement>,
    pub entry_block: SelectedBlockId,
    pub virtual_registers: Vec<VirtualRegister>,
    pub blocks: Vec<SelectedBlock>,
}

impl SelectedFunction {
    /// The block keyed `block`, when the function declares it.
    pub fn block(&self, block: SelectedBlockId) -> Option<&SelectedBlock> {
        self.blocks.iter().find(|row| row.id == block)
    }

    /// The position of the block keyed `id`, when the function declares it.
    pub fn block_index(&self, id: SelectedBlockId) -> Option<usize> {
        self.blocks.iter().position(|block| block.id == id)
    }

    /// The virtual register keyed `register`, when the function declares it.
    pub fn virtual_register(&self, register: VirtualRegisterId) -> Option<&VirtualRegister> {
        self.virtual_registers.iter().find(|row| row.id == register)
    }

    /// The call contract covering `instruction`, when the function carries one.
    pub fn call(
        &self,
        instruction: super::SelectedInstructionId,
    ) -> Option<&super::SelectedCallContract> {
        self.calls
            .iter()
            .find(|call| call.instruction == instruction)
    }

    /// The memory-access row covering `instruction`, when carried.
    pub fn memory_access(
        &self,
        instruction: super::SelectedInstructionId,
    ) -> Option<&super::SelectedMemoryAccess> {
        self.memory_accesses
            .iter()
            .find(|access| access.instruction == instruction)
    }

    /// Every block's instructions, in block order.
    pub fn instructions(&self) -> impl Iterator<Item = &super::SelectedInstruction> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }

    /// The block keyed `block`, mutably, when the function declares it.
    pub fn block_mut(&mut self, block: SelectedBlockId) -> Option<&mut SelectedBlock> {
        self.blocks.iter_mut().find(|row| row.id == block)
    }

    /// Physical entry liveness, after independent source/ABI replay. A hidden
    /// result destination is an ABI input but never a semantic parameter.
    pub fn is_entry_register(&self, register: &VirtualRegister) -> bool {
        use super::VirtualRegisterOrigin;
        match register.origin {
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. } => true,
            VirtualRegisterOrigin::AbiTransport {
                instruction: super::SelectedInstructionId(0),
                place,
                byte_offset: 0,
            } => {
                register.entry_fixed_view.is_some()
                    && register.definition_site.is_none()
                    && self
                        .structural
                        .as_ref()
                        .and_then(|signature| signature.result.as_ref())
                        .is_some_and(|result| result.place == place)
            }
            _ => false,
        }
    }
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
    /// Continues selection of a declared sum case; no semantic edge is taken yet.
    CaseDispatch {
        source: BlockId,
        case_ordinal: u32,
    },
    /// Copies for one authored edge; the target is semantic lineage, not a
    /// claim that this implementation block exists in Terminal Psi.
    EdgeTransfer {
        edge: EdgeId,
        target: BlockId,
    },
}

impl SelectedBlock {
    /// The instruction keyed `instruction`, when the block carries it.
    pub fn instruction(
        &self,
        instruction: super::SelectedInstructionId,
    ) -> Option<&SelectedInstruction> {
        self.instructions.iter().find(|row| row.id == instruction)
    }

    /// The position of the instruction keyed `instruction`.
    pub fn instruction_index(&self, instruction: super::SelectedInstructionId) -> Option<usize> {
        self.instructions
            .iter()
            .position(|row| row.id == instruction)
    }

    /// Semantic anchor; an implementation block anchors its destination.
    pub const fn source_block(&self) -> BlockId {
        match self.origin {
            SelectedBlockOrigin::Source(block) => block,
            SelectedBlockOrigin::CaseDispatch { source, .. } => source,
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
    /// Continues case tests. The edge names the next candidate's lineage only;
    /// this transfer performs no source edge, payload binding, cleanup, or fuel.
    CaseDispatchContinuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedTerminator {
    /// Explicit semantic abandonment, with no normal successor or cleanup.
    Crash {
        instruction: SelectedInstruction,
        psi_edge: EdgeId,
        cause: terminal_psi::CrashCause,
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
    },
    /// Physical termination or trap, never an executable return successor.
    HostedExitProcess {
        instruction: SelectedInstruction,
        completion_edge: EdgeId,
    },
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
    pub structural_bindings: Vec<SelectedStructuralBinding>,
    /// Case custody is retained on both physical legs; only the semantic leg
    /// selects a case and carries its cleanup/fuel; continuation cleanup is empty.
    pub structural_case: Option<super::SelectedStructuralCaseEdge>,
    /// Path-specific logical fuel for this exact semantic edge.
    pub fuel: Vec<FuelSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralBinding {
    pub semantic: abstract_operations::AbstractStructuralBinding,
    pub transport: SelectedStructuralTransport,
    /// Selectors in the final contiguous source region. Earlier pointer
    /// crossings carry their evidence on their own preparation loads.
    pub source_indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
}

impl SelectedStructuralBinding {
    /// Footprints for a source snapshot, never for an address-only loan.
    pub fn source_read_roles(&self) -> impl Iterator<Item = super::SelectedMemoryAccessRole> + '_ {
        self.source_indices
            .iter()
            .map(
                |index| super::SelectedMemoryAccessRole::ReadIndexedPrimitive {
                    index: index.operand.value,
                    stride: index.stride,
                    extent: index.extent,
                    obligation: index.obligation,
                    accepted_fact: index.accepted_fact,
                },
            )
            .chain(
                self.source_indices
                    .is_empty()
                    .then_some(super::SelectedMemoryAccessRole::ReadPlace),
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedStructuralTransport {
    Unused,
    /// Bind an internal scalar loan's address and interpretation as one pair.
    /// A descriptor source supplies both words; canonical storage lends its
    /// address and supplies an admitted codec without copying backing bytes.
    ScalarReference {
        base: SelectedAddressBase,
        byte_offset: u32,
        source: SelectedScalarReferenceSource,
        destination: super::LocalStorageSlotId,
    },
    /// Snapshot the complete selected value before replacing its destination.
    /// An empty semantic path has offset zero; a projection selects storage
    /// inside the same root pointer without changing that root's identity.
    WholeValue {
        argument: VirtualRegisterId,
        source_byte_offset: u32,
        destination: super::LocalStorageSlotId,
        byte_size: u32,
        alignment: u16,
    },
    /// Store one borrowed referent's exact address in the destination slot.
    /// The edge lends that place: it neither reads nor copies the referent's
    /// `byte_count` bytes at `byte_offset` beneath `base`, and the root keeps
    /// its owner and storage.
    Address {
        base: SelectedAddressBase,
        byte_offset: u32,
        byte_count: u32,
        destination: super::LocalStorageSlotId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedScalarReferenceSource {
    Canonical { codec: u64 },
    Descriptor,
}

/// The storage root an address transport offsets from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedAddressBase {
    /// A pointer already live where the edge runs: an incoming borrowed
    /// parameter or an earlier address join.
    Register(VirtualRegisterId),
    /// A frame-resident home, addressed on the edge itself.
    Local(super::LocalStorageSlotId),
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

macro_rules! terminator_instruction_match {
    ($terminator:expr) => {
        match $terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::Crash { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
        }
    };
}

/// The instruction record every terminator carries.
pub fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    terminator_instruction_match!(terminator)
}

/// The mutable instruction record every terminator carries.
pub fn terminator_instruction_mut(terminator: &mut SelectedTerminator) -> &mut SelectedInstruction {
    terminator_instruction_match!(terminator)
}

/// The Psi edge a `Return` terminator maps back to, when it is one.
pub fn terminator_return_edge(terminator: &SelectedTerminator) -> Option<EdgeId> {
    match terminator {
        SelectedTerminator::Return {
            psi_return_edge, ..
        } => Some(*psi_return_edge),
        _ => None,
    }
}

macro_rules! terminator_successors_match {
    ($terminator:expr) => {
        match $terminator {
            SelectedTerminator::Jump { successor, .. } => vec![successor],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => vec![when_nonzero, when_zero],
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            } => vec![when_less, when_not_less],
            SelectedTerminator::Return { .. }
            | SelectedTerminator::Crash { .. }
            | SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
        }
    };
}

/// Every successor edge a terminator carries, in its physical record
/// order; `HostedExitProcess` and `Return` carry none.
pub fn terminator_successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    terminator_successors_match!(terminator)
}

/// Every successor edge a terminator carries, mutably.
pub fn terminator_successors_mut(
    terminator: &mut SelectedTerminator,
) -> Vec<&mut SelectedSuccessor> {
    terminator_successors_match!(terminator)
}

#[macro_export]
macro_rules! terminator_exit_triple_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedTerminator::Crash { $($fields)* }
            | $crate::SelectedTerminator::HostedExitProcess { $($fields)* }
            | $crate::SelectedTerminator::Return { $($fields)* }
    };
}

#[macro_export]
macro_rules! conditional_branch_less_pair_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedTerminator::ConditionalBranchI64LessThan { $($fields)* }
            | $crate::SelectedTerminator::ConditionalBranchU64LessThan { $($fields)* }
    };
}

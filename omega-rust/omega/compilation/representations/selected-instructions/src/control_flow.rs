/*
  Selected functions, source and implementation blocks, terminators, and
  explicitly identified successor edges.

  `SelectedFunction` owns a machine's selected basic-block graph (`SelectedBlocks`),
  virtual-register table, frame storage slots, call and memory-access contracts,
  and boundary settlements. `SelectedBlock` pairs a dense `SelectedBlockId` and
  `SelectedBlockOrigin` with its linear instruction sequence and `SelectedTerminator`,
  while `SelectedSuccessor` records the semantic or implementation continuation
  role, scalar/structural edge transports, case dispatch bindings, and logical
  work settlements.
*/

use super::SelectedInstructionId;
use super::SelectedMemoryAccessRole::{ReadIndexedPrimitive, ReadPlace};
use super::{SelectedBlockId, SelectedInstruction, VirtualRegister, VirtualRegisterId};
use abstract_operations::ValueBinding;
use optimization_unit::WorkSettlement;
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    pub nominal_cleanup_subject: Option<semantic_vocabulary::StructuralTypeId>, // Exact owner-designated cleanup subject, independent of attachment or call sites.
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
    pub blocks: super::SelectedBlocks,
}

impl SelectedFunction {
    pub fn block(&self, block: SelectedBlockId) -> Option<&SelectedBlock> {
        self.block_index(block).map(|index| &self.blocks[index])
    }

    pub fn block_index(&self, id: SelectedBlockId) -> Option<usize> {
        let index = id.0 as usize;
        self.blocks
            .get(index)
            .is_some_and(|block| block.id == id)
            .then_some(index)
    }

    pub fn virtual_register(&self, register: VirtualRegisterId) -> Option<&VirtualRegister> {
        self.virtual_registers
            .get(register.0 as usize)
            .filter(|row| row.id == register)
    }

    pub fn ids_are_positions(&self) -> bool {
        self.blocks
            .iter()
            .enumerate()
            .all(|(index, block)| block.id.0 as usize == index)
            && self
                .virtual_registers
                .iter()
                .enumerate()
                .all(|(index, register)| register.id.0 as usize == index)
    }

    pub fn call(&self, instruction: SelectedInstructionId) -> Option<&super::SelectedCallContract> {
        self.calls
            .iter()
            .find(|call| call.instruction == instruction)
    }

    pub fn memory_access(
        &self,
        instruction: SelectedInstructionId,
    ) -> Option<&super::SelectedMemoryAccess> {
        self.memory_accesses
            .iter()
            .find(|access| access.instruction == instruction)
    }

    pub fn instructions(&self) -> impl Iterator<Item = &super::SelectedInstruction> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }

    pub fn block_mut(&mut self, block: SelectedBlockId) -> Option<&mut SelectedBlock> {
        let index = self.block_index(block)?;
        self.blocks.get_mut(index)
    }

    pub fn is_entry_register(&self, register: &VirtualRegister) -> bool {
        use super::VirtualRegisterOrigin;
        match register.origin {
            VirtualRegisterOrigin::EntryParameter { .. }
            | VirtualRegisterOrigin::StructuralParameter { .. } => true,
            VirtualRegisterOrigin::AbiTransport {
                instruction: SelectedInstructionId(0),
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
    Operation {
        source: BlockId,
        operation: OperationId,
    },
    CaseDispatch {
        source: BlockId,
        case_ordinal: u32,
    },
    EdgeTransfer {
        edge: EdgeId,
        target: BlockId,
    },
}

impl SelectedBlock {
    pub fn instruction(&self, instruction: SelectedInstructionId) -> Option<&SelectedInstruction> {
        self.instructions.iter().find(|row| row.id == instruction)
    }

    pub fn instruction_index(&self, instruction: SelectedInstructionId) -> Option<usize> {
        self.instructions
            .iter()
            .position(|row| row.id == instruction)
    }

    pub const fn source_block(&self) -> BlockId {
        match self.origin {
            SelectedBlockOrigin::Source(block) => block,
            SelectedBlockOrigin::Operation { source, .. } => source,
            SelectedBlockOrigin::CaseDispatch { source, .. } => source,
            SelectedBlockOrigin::EdgeTransfer { target, .. } => target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedSuccessorRole {
    Semantic,
    OperationContinuation { operation: OperationId },
    EdgeTransferContinuation,
    CaseDispatchContinuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedTerminator {
    InvokeNoNormal {
        instruction: SelectedInstruction,
        psi_edge: EdgeId,
    },
    OperationInvokeNoNormal {
        instruction: SelectedInstruction,
        operation: OperationId,
    },
    OperationTrap {
        instruction: SelectedInstruction,
        operation: OperationId,
    },
    Crash {
        instruction: SelectedInstruction,
        psi_edge: EdgeId,
        cause: terminal_psi::CrashCause,
        site_guard: Vec<terminal_psi::CrashPredicateTerm>,
        frontier_lower_bound: Vec<semantic_vocabulary::ClaimId>,
    },
    HostedExitProcess {
        instruction: SelectedInstruction,
        completion_edge: EdgeId,
    },
    Jump {
        instruction: SelectedInstruction,
        successor: SelectedSuccessor,
    },
    ConditionalBranch {
        instruction: SelectedInstruction,
        when_nonzero: SelectedSuccessor,
        when_zero: SelectedSuccessor,
    },
    ConditionalBranchU64LessThan {
        instruction: SelectedInstruction,
        when_less: SelectedSuccessor,
        when_not_less: SelectedSuccessor,
    },
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
    pub psi_edge: Option<EdgeId>, // Absent only for operation-internal control.
    pub block: SelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<SelectedValueBinding>,
    pub structural_bindings: Vec<SelectedStructuralBinding>,
    pub structural_case: Option<super::SelectedStructuralCaseEdge>,
    pub work: Vec<WorkSettlement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedStructuralBinding {
    pub semantic: abstract_operations::AbstractStructuralBinding,
    pub transport: SelectedStructuralTransport,
    pub source_indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
}

impl SelectedStructuralBinding {
    pub fn source_read_roles(&self) -> impl Iterator<Item = super::SelectedMemoryAccessRole> + '_ {
        self.source_indices
            .iter()
            .map(|index| ReadIndexedPrimitive {
                index: index.operand.value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            })
            .chain(self.source_indices.is_empty().then_some(ReadPlace))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedStructuralTransport {
    Unused,
    ScalarReference {
        base: SelectedAddressBase,
        byte_offset: u32,
        source: SelectedScalarReferenceSource,
        destination: super::LocalStorageSlotId,
    },
    WholeValue {
        argument: VirtualRegisterId,
        source_byte_offset: u32,
        destination: super::LocalStorageSlotId,
        byte_size: u32,
        alignment: u16,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedAddressBase {
    Register(VirtualRegisterId),
    Local(super::LocalStorageSlotId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedValueBinding {
    pub semantic: ValueBinding,
    pub transport: SelectedValueTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedValueTransport {
    Unused,
    Registers {
        argument: VirtualRegisterId,
        parameter: VirtualRegisterId,
    },
}

macro_rules! terminator_instruction_match {
    ($terminator:expr) => {
        match $terminator {
            SelectedTerminator::InvokeNoNormal { instruction, .. }
            | SelectedTerminator::OperationInvokeNoNormal { instruction, .. }
            | SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::Crash { instruction, .. }
            | SelectedTerminator::OperationTrap { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
        }
    };
}

pub fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    terminator_instruction_match!(terminator)
}

pub fn terminator_instruction_mut(terminator: &mut SelectedTerminator) -> &mut SelectedInstruction {
    terminator_instruction_match!(terminator)
}

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
            SelectedTerminator::InvokeNoNormal { .. }
            | SelectedTerminator::OperationInvokeNoNormal { .. }
            | SelectedTerminator::Return { .. }
            | SelectedTerminator::Crash { .. }
            | SelectedTerminator::OperationTrap { .. }
            | SelectedTerminator::HostedExitProcess { .. } => Vec::new(),
        }
    };
}

pub fn terminator_successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    terminator_successors_match!(terminator)
}

pub fn terminator_successors_mut(
    terminator: &mut SelectedTerminator,
) -> Vec<&mut SelectedSuccessor> {
    terminator_successors_match!(terminator)
}

#[macro_export]
macro_rules! terminator_no_successor_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedTerminator::InvokeNoNormal { $($fields)* }
            | $crate::SelectedTerminator::OperationInvokeNoNormal { $($fields)* }
            | $crate::SelectedTerminator::Crash { $($fields)* }
            | $crate::SelectedTerminator::OperationTrap { $($fields)* }
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

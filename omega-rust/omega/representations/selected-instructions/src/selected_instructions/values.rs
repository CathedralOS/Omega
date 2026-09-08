//! Virtual values and instruction operand constraints; no assigned storage.
use super::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterViewId};
use semantic_vocabulary::{PlaceId, ScalarType, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualRegister {
    pub id: VirtualRegisterId,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: VirtualRegisterOrigin,
    pub definition_site: Option<ValueDefinitionSite>,
    /// An ABI live-in constraint. This is not an assigned physical home.
    pub entry_fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualRegisterOrigin {
    /// Compiler-owned ABI stack address associated with a scalar source, not its payload.
    ScalarAbiAddress {
        instruction: SelectedInstructionId,
        source_value: ValueId,
    },
    /// Compiler-owned frame address for one original virtual value's spill slot.
    SpillAddress {
        instruction: SelectedInstructionId,
        register: VirtualRegisterId,
    },
    StructuralParameter {
        place: PlaceId,
        parameter_index: usize,
    },
    AbiTransport {
        instruction: SelectedInstructionId,
        place: PlaceId,
        byte_offset: u32,
    },
    EntryParameter {
        source_value: ValueId,
        parameter_index: usize,
    },
    /// Defined by the selected block's incoming edge bindings, not by any
    /// instruction in a predecessor. Each edge retains its distinct argument.
    BlockParameter {
        source_value: ValueId,
        block: SelectedBlockId,
        parameter_index: usize,
    },
    InstructionResult {
        instruction: SelectedInstructionId,
        source_value: ValueId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedOperand {
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub access: RegisterOperandAccess,
    pub class: RegisterClassId,
    /// A fixed instruction-use/def constraint, not an assigned home.
    pub fixed_view: Option<RegisterViewId>,
    /// Canonical one-way allocation tie to an earlier operand.
    pub tied_to: Option<u16>,
    /// This definition may clobber before unrelated inputs are all read.
    pub early_clobber: bool,
}

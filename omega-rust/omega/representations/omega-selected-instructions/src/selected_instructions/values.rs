//! Virtual values and instruction operand constraints; no assigned storage.
use super::{SelectedInstructionId, VirtualRegisterId};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{RegisterClassId, RegisterOperandAccess, RegisterViewId};
use psi_core::{ScalarType, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualRegister {
    pub id: VirtualRegisterId,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: VirtualRegisterOrigin,
    pub definition_site: ValueDefinitionSite,
    /// An ABI live-in constraint. This is not an assigned physical home.
    pub entry_fixed_view: Option<RegisterViewId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualRegisterOrigin {
    EntryParameter {
        source_value: ValueId,
        parameter_index: usize,
    },
    InstructionResult {
        instruction: SelectedInstructionId,
        source_value: ValueId,
    },
    /// A value introduced by mandatory target legalization. `source_value`
    /// retains Psi lineage without claiming that the source value itself has
    /// the legalized register type.
    LegalizationTemporary {
        instruction: SelectedInstructionId,
        temporary: omega_legalized_operations::LegalizedTemporaryId,
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

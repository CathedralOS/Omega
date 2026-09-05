//! Semantic input declarations and their physical ABI homes.

use omega_calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use omega_target_operations::MachineRegister;
use psi_core::{PlaceId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitParameterHomeRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub access: psi_terminal::StructuralAccess,
    pub shape: ValueShape,
    pub source: ValuePlacement,
    pub byte_offset: u32,
    pub indirect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitParameterRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: psi_terminal::StructuralMultiplicity,
    pub access: psi_terminal::StructuralAccess,
    pub shape: ValueShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScalarFunctionAbiRecord {
    pub call_plan: CallPlan,
    pub parameters: Vec<omega_target_operations::UnitScalarAbiValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitScalarParameterLocationRecord {
    Register(MachineRegister),
    IncomingStack { byte_offset: u32 },
}

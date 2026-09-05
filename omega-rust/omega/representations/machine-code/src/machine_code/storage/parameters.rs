//! Semantic input declarations and their physical ABI homes.

use calling_conventions::{CallPlan, ValuePlacement, ValueShape};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use target_operations::MachineRegister;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitParameterHomeRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: terminal_psi::StructuralAccess,
    pub shape: ValueShape,
    pub source: ValuePlacement,
    pub byte_offset: u32,
    pub indirect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitParameterRecord {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
    pub multiplicity: terminal_psi::StructuralMultiplicity,
    pub access: terminal_psi::StructuralAccess,
    pub shape: ValueShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScalarFunctionAbiRecord {
    pub call_plan: CallPlan,
    pub parameters: Vec<target_operations::UnitScalarAbiValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitScalarParameterLocationRecord {
    Register(MachineRegister),
    IncomingStack { byte_offset: u32 },
}

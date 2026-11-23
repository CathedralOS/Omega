use crate::{BlockId, OperationId, TargetIdentity, TypeId, ValueId};
pub use omega_target_contract::{InstructionExit, InstructionInput, InstructionSemantics, InstructionValue, InstructionWrite, LocationLifetime, LocationRole};
use std::sync::Arc;

/// A catalog location with its declared role and lifetime. A write to a register
/// location is a changed register whatever its lifetime. A persistent location
/// keeps its value after its region ends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyLocation {
    pub name: String,
    pub ty: TypeId,
    pub role: LocationRole,
    pub lifetime: LocationLifetime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssemblyOperand {
    /// A register named in source.
    Register(AssemblyLocation),
    Value(ValueId),
    LocalBlock(BlockId),
}

/// All initial values are evaluated before any bound register is written.
/// Each execution of this operation starts a fresh region: region locations
/// start unsupplied, and persistent locations keep their incoming values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyEntry {
    pub target: TargetIdentity,
    pub bindings: Vec<(AssemblyLocation, ValueId)>,
    pub clobbers: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyInstruction {
    pub entry: OperationId,
    pub target: TargetIdentity,
    pub mnemonic: String,
    pub operands: Vec<AssemblyOperand>,
    /// Locations the selected semantics names directly, in name order.
    pub implicit_locations: Vec<AssemblyLocation>,
    /// The selected executable state transition. A conditional exit produces
    /// its pre-instruction Boolean condition as this operation's scalar result.
    pub semantics: Arc<InstructionSemantics>,
}

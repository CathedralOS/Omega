use crate::{AssemblyLocation, Site, ValueType};
use omega_resolved::{DeclarationId, TreeNodeId};
use omega_target_contract::{InstructionSemantics, LocationLifetime, TargetIdentity};
use std::sync::Arc;

/// Inputs evaluated simultaneously on entry to a selected target's working register region.
#[derive(Debug, Eq, PartialEq)]
pub struct TypedAssemblyRegion {
    pub target: TargetIdentity,
    /// Each initializer, the register it binds and that register's catalog declaration.
    pub bindings: Vec<(TreeNodeId, String, AssemblyLocation)>,
    pub clobbers: Option<Vec<String>>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TypedInstructionOperand {
    Value {
        node: TreeNodeId,
        ty: ValueType,
    },
    /// A register named in source. Its role is always register; its lifetime is the catalog's.
    Register {
        node: TreeNodeId,
        name: String,
        ty: ValueType,
        lifetime: LocationLifetime,
    },
    Block {
        node: TreeNodeId,
        declaration: DeclarationId,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub struct TypedInstruction {
    pub node: TreeNodeId,
    pub site: Site,
    pub target: TargetIdentity,
    pub mnemonic: String,
    pub operands: Vec<TypedInstructionOperand>,
    /// Catalog declarations of the locations the selected semantics names directly,
    /// rather than through an operand, in name order.
    pub implicit_locations: Vec<(String, AssemblyLocation)>,
    pub semantics: Arc<InstructionSemantics>,
}

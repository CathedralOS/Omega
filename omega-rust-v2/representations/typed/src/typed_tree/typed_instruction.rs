use crate::{Site, ValueType};
use omega_resolved::{DeclarationId, TreeNodeId};
use omega_target_contract::{InstructionSemantics, TargetIdentity};
use std::sync::Arc;

/// Inputs evaluated simultaneously on entry to a selected target's working register region.
#[derive(Debug, Eq, PartialEq)]
pub struct TypedAssemblyRegion {
    pub target: TargetIdentity,
    pub bindings: Vec<(TreeNodeId, String, ValueType)>,
    pub clobbers: Option<Vec<String>>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TypedInstructionOperand {
    Value { node: TreeNodeId, ty: ValueType },
    Register { node: TreeNodeId, name: String, ty: ValueType },
    Block { node: TreeNodeId, declaration: DeclarationId },
}

#[derive(Debug, Eq, PartialEq)]
pub struct TypedInstruction {
    pub node: TreeNodeId,
    pub site: Site,
    pub target: TargetIdentity,
    pub mnemonic: String,
    pub operands: Vec<TypedInstructionOperand>,
    /// Types of the implicitly referenced registers in the selected semantics.
    pub implicit_registers: Vec<(String, ValueType)>,
    pub semantics: Arc<InstructionSemantics>,
}

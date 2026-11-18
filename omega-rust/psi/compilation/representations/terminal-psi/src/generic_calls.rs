//! Source-checking correspondence for an open nominal call. These rows are
//! adjunct checking inputs, never members of a published Terminal module.

use crate::TerminalIdentityIndex;
use semantic_vocabulary::{MachineId, OperationId, ScalarTerm, StructuralTypeId};

#[derive(Debug, Clone)]
pub struct NominalConstArgument {
    pub parameter: u32,
    pub actual: ScalarTerm,
}

#[derive(Debug, Clone)]
pub struct NominalCallArgument {
    pub caller: MachineId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub argument: u32,
    pub actual: StructuralTypeId,
    pub expected: StructuralTypeId,
    pub actual_constructor: StructuralTypeId,
    pub expected_constructor: StructuralTypeId,
    pub declaration: TerminalIdentityIndex,
    pub actual_application: TerminalIdentityIndex,
    pub expected_application: TerminalIdentityIndex,
    pub const_arguments: Vec<NominalConstArgument>,
}

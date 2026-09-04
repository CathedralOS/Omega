//! Functions, entry blocks and canonical operation order.

use crate::{AbstractFunctionResult, AbstractOperation, AbstractParameter};
use psi_core::{BlockId, MachineId, ServiceId, StructuralTypeId};
use psi_terminal::{EntryClaim, StructuralParameterDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub entry: BlockId,
    /// Runtime values supplied by the caller, in declared terminal-Psi order.
    pub parameters: Vec<AbstractParameter>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub result: AbstractFunctionResult,
    /// Generic live claims supplied by the caller/root installation.
    pub entry_claims: Vec<EntryClaim>,
    /// Exact verified service ceiling retained for realization and audit.
    pub published_service_ceiling: Vec<ServiceId>,
    /// Canonical block starts in `operations`. This keeps conditional targets
    /// source-independent without flattening away control-flow identity.
    pub block_entries: Vec<AbstractBlockEntry>,
    /// Operations in canonical block order. Straight-line functions retain
    /// their historical executable order.
    pub operations: Vec<AbstractOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractBlockEntry {
    pub block: BlockId,
    /// Scalar parameters in canonical Terminal-Psi declaration order. This is
    /// retained independently of incoming bindings so entry and otherwise
    /// unreferenced declarations cannot disappear during lowering.
    pub parameters: Vec<AbstractParameter>,
    pub operation_offset: usize,
}

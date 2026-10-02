//! Functions, entry blocks and canonical operation order.

use crate::{AbstractFunctionResult, AbstractOperation, AbstractParameter};
use semantic_vocabulary::{BlockId, MachineId, PlaceId, ServiceId, StructuralTypeId};
use terminal_psi::{EntryClaim, StructuralParameterDeclaration};

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

impl AbstractFunction {
    /// The parameter declaration a structural place arrives as: this
    /// function's own parameter, or the parameter of the block that declares
    /// it. Both are storage an operation addresses in place, unlike an
    /// operation result or a literal, which have no caller-owned home.
    ///
    /// A receiver collapses every state onto the one function parameter, while
    /// a named data parameter gives each state its own block parameter, so
    /// consulting only `structural_parameters` admits the receiver spelling of
    /// a program and refuses the identical program spelled statically. Place
    /// identities are unique within a function, so matching the place is
    /// exact. Callers still read the returned access, multiplicity and
    /// qualifications themselves; this answers only which storage it is.
    pub fn arrival_parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        self.structural_parameters
            .iter()
            .chain(
                self.block_entries
                    .iter()
                    .flat_map(|entry| &entry.structural_parameters),
            )
            .find(|parameter| parameter.place == place)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractBlockEntry {
    pub block: BlockId,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    /// Scalar parameters in canonical Terminal-Psi declaration order. This is
    /// retained independently of incoming bindings so entry and otherwise
    /// unreferenced declarations cannot disappear during lowering.
    pub parameters: Vec<AbstractParameter>,
    pub operation_offset: usize,
}

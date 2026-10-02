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
    /// The one exact declaration whose storage arrives at `place` through the
    /// function entrance or a block edge. Terminal-to-abstract lowering has
    /// already authenticated each place kind; this representation retains the
    /// entry roster and every block's ordered `(block, position)` coordinates.
    /// A duplicate spelling fails closed rather than selecting one by type.
    ///
    /// A receiver may reuse its entrance place across states while a named
    /// state parameter receives a new place. Storage operations must resolve
    /// both through this query, then check access, multiplicity and qualifications;
    /// consulting only the entrance roster makes equivalent programs disagree.
    pub fn arrival_parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        let mut found = None;
        for parameter in self
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.place == place)
        {
            if found.replace(parameter).is_some() {
                return None;
            }
        }
        for block in &self.block_entries {
            for (position, parameter) in block.structural_parameters.iter().enumerate() {
                if parameter.place != place {
                    continue;
                }
                // Entry block parameters are forbidden by abstract-program
                // validation: function formals live only in the signature.
                // Block positions are dense and never carry receiver identity.
                if block.block == self.entry
                    || parameter.is_self
                    || parameter.position as usize != position
                    || found.replace(parameter).is_some()
                {
                    return None;
                }
            }
        }
        found
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

#[cfg(test)]
mod tests;

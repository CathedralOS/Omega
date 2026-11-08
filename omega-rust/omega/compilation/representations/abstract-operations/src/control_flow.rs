/*
We define abstract functions, block entries, and control-flow successor edges,
along with their subordinate address-join and bounded-owner loan classifiers.

A receiver may reuse its entrance place across states while a named state
parameter receives a fresh place. `AbstractFunction::arrival_parameter` resolves
both across the function entrance roster and every non-entry block's ordered
`(block, position)` coordinates, failing closed on any duplicate place spelling
or entry-block parameter collision.
*/

pub mod address_joins;
pub mod owner_loans;

use crate::{AbstractFunctionResult, AbstractOperation, AbstractParameter, ValueBinding};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, PlaceId, ScalarType, ServiceId, StructuralCaseId, StructuralTypeId,
    ValueId,
};
use terminal_psi::{EntryClaim, StructuralParameterDeclaration as ParameterDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractFunction {
    pub machine: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub nominal_cleanup_subject: Option<StructuralTypeId>, // Owner-designated cleanup subject, independent of attachment or call sites.
    pub entry: BlockId,
    pub parameters: Vec<AbstractParameter>, // Runtime values supplied by the caller in declared Terminal Psi order.
    pub structural_parameters: Vec<ParameterDeclaration>,
    pub result: AbstractFunctionResult,
    pub entry_claims: Vec<EntryClaim>, // Generic live claims supplied by the caller/root installation.
    pub block_entry_claims: Vec<EntryClaim>, // Block-local names that continue owned incoming claims by typed path.
    pub published_service_ceiling: Vec<ServiceId>, // Verified service ceiling retained for realization and audit.
    pub block_entries: Vec<AbstractBlockEntry>,    // Canonical block starts in `operations`.
    pub operations: Vec<AbstractOperation>,        // Operations in canonical block order.
}

macro_rules! structural_parameter_accessor {
    ($name:ty) => {
        impl $name {
            pub fn structural_parameter(&self, place: PlaceId) -> Option<&ParameterDeclaration> {
                ParameterDeclaration::find_by_place(&self.structural_parameters, place)
            }
        }
    };
}

structural_parameter_accessor!(AbstractFunction);

impl AbstractFunction {
    pub fn block_entry(&self, block: BlockId) -> Option<&AbstractBlockEntry> {
        self.block_entries
            .binary_search_by_key(&block, |entry| entry.block)
            .ok()
            .map(|index| &self.block_entries[index])
    }

    pub fn block_structural_parameters(&self) -> impl Iterator<Item = &ParameterDeclaration> + '_ {
        self.block_entries
            .iter()
            .flat_map(|entry| &entry.structural_parameters)
    }

    pub fn block_structural_parameter(&self, place: PlaceId) -> Option<&ParameterDeclaration> {
        self.block_structural_parameters()
            .find(|parameter| parameter.place == place)
    }

    pub fn arrival_parameter(&self, place: PlaceId) -> Option<&ParameterDeclaration> {
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
                if block.block == self.entry
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

structural_parameter_accessor!(AbstractBlockEntry);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractBlockEntry {
    pub block: BlockId,
    pub structural_parameters: Vec<ParameterDeclaration>,
    pub parameters: Vec<AbstractParameter>, // Scalar parameters in canonical Terminal Psi declaration order.
    pub operation_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<AbstractStructuralBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>, // Exact Terminal Psi cleanup order for this conditional edge.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStructuralBinding {
    pub parameter: PlaceId,
    pub argument: terminal_psi::StructuralArgument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStructuralCaseSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub payloads: Vec<AbstractStructuralCasePayloadBinding>,
    pub structural_bindings: Vec<AbstractStructuralBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractStructuralCasePayloadBinding {
    pub parameter: ValueId,
    pub field: semantic_vocabulary::StructuralFieldId,
    pub scalar_type: ScalarType,
}

impl AbstractSuccessor {
    pub fn structural_binding(&self, parameter: PlaceId) -> Option<&AbstractStructuralBinding> {
        self.structural_bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
    }
}

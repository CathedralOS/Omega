//! Current layout-independent instruction bytes, call templates and evidence.

mod calls;
mod custody;
mod identity;
mod rows;
#[cfg(test)]
mod tests;
pub use calls::*;
pub use custody::*;
use optimization_core::Optimization;
use physical_instructions::{Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity};
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};
pub use rows::*;

/// Current encoded data and its exact content joins. Construction, cloning and
/// identity recomputation grant no admission, layout or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFormEncoding {
    pub selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub machine: PostAllocationMachineIdentity,
    pub post_allocation_machine_optimization: Option<PostAllocationMachineOptimizationCustody>,
    pub identity: SelectedFormEncodingIdentity,
    pub rows: Vec<SelectedFormEncodingRow>,
    pub structural_unit_functions: Vec<SelectedStructuralUnitFunctionEncoding>,
    pub counts: SelectedFormEncodingCounts,
}

impl SelectedFormEncoding {
    pub const fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.post_allocation_machine_optimization
    }

    /// Compatibility projection for layout routes that still name CBNZ.
    pub const fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        match self.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                ) =>
            {
                Some(SelectedFormMachineOptimizationCustody {
                    selections: custody.selections(),
                    post_allocation_machine_selections: custody
                        .post_allocation_machine_selections(),
                    fusion: Aarch64CbnzFusionIdentity::from_bytes(custody.artifact_identity()),
                })
            }
            _ => None,
        }
    }

    /// Compatibility projection for layout routes that still name MOVN.
    pub const fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        match self.post_allocation_machine_optimization {
            Some(custody)
                if matches!(
                    custody.optimization(),
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                ) =>
            {
                Some(SelectedFormMovnOptimizationCustody {
                    selections: custody.selections(),
                    post_allocation_machine_selections: custody
                        .post_allocation_machine_selections(),
                    materialization: Aarch64MovnMaterializationIdentity::from_bytes(
                        custody.artifact_identity(),
                    ),
                })
            }
            _ => None,
        }
    }

    pub const fn identity(&self) -> SelectedFormEncodingIdentity {
        self.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.rows
    }

    pub fn structural_unit_functions(&self) -> &[SelectedStructuralUnitFunctionEncoding] {
        &self.structural_unit_functions
    }

    pub const fn counts(&self) -> SelectedFormEncodingCounts {
        self.counts
    }

    /// Recompute content identity without granting encoding or publication authority.
    pub fn recomputed_identity(&self) -> SelectedFormEncodingIdentity {
        identity::encoding_identity(
            self.selected,
            self.machine,
            self.post_allocation_machine_optimization,
            &self.rows,
            &self.structural_unit_functions,
            self.counts,
        )
    }
}

/// An identity names retained bytes and rows. It does not grant encoding,
/// optimization, layout, or publication authority.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormEncodingIdentity([u8; 32]);

impl SelectedFormEncodingIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

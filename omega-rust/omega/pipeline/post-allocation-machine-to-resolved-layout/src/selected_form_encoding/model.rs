pub use machine_code::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncoding,
    SelectedFormEncodingCounts, SelectedFormEncodingIdentity, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition, SelectedFormMachineOptimizationCustody,
    SelectedFormMovnOptimizationCustody, SelectedStructuralUnitCallEncodingRow,
    SelectedStructuralUnitFunctionEncoding,
};
use optimization_core::Optimization;
use physical_instructions::{
    Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity, PostAllocationMachineIdentity,
    PostAllocationMachineOptimizationCustody,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedSelectedFormEncoding {
    pub(super) program: std::sync::Arc<SelectedFormEncoding>,
}
impl StagedOptimizedSelectedFormEncoding {
    pub fn program(&self) -> &SelectedFormEncoding {
        &self.program
    }

    pub fn shared_program(&self) -> std::sync::Arc<SelectedFormEncoding> {
        std::sync::Arc::clone(&self.program)
    }
    pub fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.program.selected
    }

    pub fn machine(&self) -> PostAllocationMachineIdentity {
        self.program.machine
    }

    pub fn post_allocation_machine_optimization(
        &self,
    ) -> Option<PostAllocationMachineOptimizationCustody> {
        self.program.post_allocation_machine_optimization
    }

    /// Compatibility projection for layout routes that still name CBNZ.
    pub fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        match self.program.post_allocation_machine_optimization {
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
    pub fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        match self.program.post_allocation_machine_optimization {
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

    pub fn identity(&self) -> SelectedFormEncodingIdentity {
        self.program.identity
    }

    pub fn rows(&self) -> &[SelectedFormEncodingRow] {
        &self.program.rows
    }

    pub fn structural_unit_functions(&self) -> &[SelectedStructuralUnitFunctionEncoding] {
        &self.program.structural_unit_functions
    }

    pub fn counts(&self) -> SelectedFormEncodingCounts {
        self.program.counts
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn rows_mut(&mut self) -> &mut [SelectedFormEncodingRow] {
        std::sync::Arc::make_mut(&mut std::sync::Arc::make_mut(&mut self.program).rows)
            .as_mut_slice()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn structural_unit_functions_mut(
        &mut self,
    ) -> &mut [SelectedStructuralUnitFunctionEncoding] {
        std::sync::Arc::make_mut(
            &mut std::sync::Arc::make_mut(&mut self.program).structural_unit_functions,
        )
        .as_mut_slice()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn counts_mut(&mut self) -> &mut SelectedFormEncodingCounts {
        &mut std::sync::Arc::make_mut(&mut self.program).counts
    }
}

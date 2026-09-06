use machine_code::{
    SelectedFormEncoding, SelectedFormEncodingCounts, SelectedFormEncodingIdentity,
    SelectedFormEncodingRow, SelectedFormMachineOptimizationCustody,
    SelectedFormMovnOptimizationCustody, SelectedStructuralUnitFunctionEncoding,
};
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachineOptimizationCustody,
};

/// Independently admitted encoding data. Raw program data cannot construct this token.
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

    pub fn machine_optimization(&self) -> Option<SelectedFormMachineOptimizationCustody> {
        self.program.machine_optimization()
    }

    pub fn movn_optimization(&self) -> Option<SelectedFormMovnOptimizationCustody> {
        self.program.movn_optimization()
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
        std::sync::Arc::make_mut(&mut self.program)
            .rows
            .as_mut_slice()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn structural_unit_functions_mut(
        &mut self,
    ) -> &mut [SelectedStructuralUnitFunctionEncoding] {
        std::sync::Arc::make_mut(&mut self.program)
            .structural_unit_functions
            .as_mut_slice()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn counts_mut(&mut self) -> &mut SelectedFormEncodingCounts {
        &mut std::sync::Arc::make_mut(&mut self.program).counts
    }
}

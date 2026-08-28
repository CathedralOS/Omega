use crate::{
    OptimizedActiveResidentRematerializationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedSelectedFormEncodingError, SelectedFormEncodingIdentity, SelectedFormEncodingState,
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding, stage_optimized_layout_independent_selected_form_encoding,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
};

/// Owning pre-layout custody for one pressure-rematerialized selected form, its
/// source-specific post-allocation machine plan, and canonical scalar bytes.
/// Deferred control rows remain unresolved and this grants no layout, frame,
/// emission, section, object, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedActiveResidentRematerializationSelectedFormEncoding {
    source: StagedOptimizedActiveResidentRematerialization,
    machine: StagedOptimizedPostAllocationMachinePlan,
    encoding: StagedOptimizedSelectedFormEncoding,
    custody: StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerializationSelectedFormEncoding {
    pub const fn source(&self) -> &StagedOptimizedActiveResidentRematerialization {
        &self.source
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
    rematerialization: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    transformed_selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    encoding: SelectedFormEncodingIdentity,
    row_count: usize,
    encoded_count: usize,
    deferred_count: usize,
}

impl StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
    pub const fn rematerialization(
        &self,
    ) -> StagedOptimizedActiveResidentRematerializationCustodyReceipt {
        self.rematerialization
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }

    pub const fn transformed_selected(
        &self,
    ) -> omega_selected_instructions::SelectedInstructionPlanIdentity {
        self.transformed_selected
    }

    pub const fn encoding(&self) -> SelectedFormEncodingIdentity {
        self.encoding
    }

    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    pub const fn encoded_count(&self) -> usize {
        self.encoded_count
    }

    pub const fn deferred_count(&self) -> usize {
        self.deferred_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationSelectedFormEncodingError {
    Rematerialization(OptimizedActiveResidentRematerializationError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Encoding(OptimizedSelectedFormEncodingError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedActiveResidentRematerializationSelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized active-resident rematerialization selected-form encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedActiveResidentRematerializationSelectedFormEncodingError {}

pub fn stage_optimized_active_resident_rematerialization_selected_form_encoding(
    source: StagedOptimizedActiveResidentRematerialization,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
> {
    let rematerialization = validate_optimized_active_resident_rematerialization(&source).map_err(
        OptimizedActiveResidentRematerializationSelectedFormEncodingError::Rematerialization,
    )?;
    let machine_custody =
        validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
            &source,
            &machine,
        )
        .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Machine)?;
    let environment = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        source.rematerialization(),
        &machine,
        environment.physical(),
    )
    .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding)?;
    let custody = custody_receipt(rematerialization, machine_custody, &encoding);
    let staged = StagedOptimizedActiveResidentRematerializationSelectedFormEncoding {
        source,
        machine,
        encoding,
        custody,
    };
    validate_optimized_active_resident_rematerialization_selected_form_encoding(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_active_resident_rematerialization_selected_form_encoding(
    staged: &StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
) -> Result<
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
> {
    let rematerialization = validate_optimized_active_resident_rematerialization(&staged.source)
        .map_err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::Rematerialization,
        )?;
    let machine =
        validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
            &staged.source,
            &staged.machine,
        )
        .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Machine)?;
    let environment = staged
        .source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    validate_optimized_layout_independent_selected_form_encoding(
        staged.source.rematerialization(),
        &staged.machine,
        environment.physical(),
        &staged.encoding,
    )
    .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding)?;
    let custody = custody_receipt(rematerialization, machine, &staged.encoding);
    if custody != staged.custody {
        return Err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::ReceiptMismatch,
        );
    }
    Ok(custody)
}

fn custody_receipt(
    rematerialization: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    encoding: &StagedOptimizedSelectedFormEncoding,
) -> StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
    let encoded_count = encoding
        .rows()
        .iter()
        .filter(|row| matches!(row.state, SelectedFormEncodingState::Encoded { .. }))
        .count();
    let deferred_count = encoding
        .rows()
        .iter()
        .filter(|row| matches!(row.state, SelectedFormEncodingState::DeferredControl { .. }))
        .count();
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt {
        rematerialization,
        machine,
        transformed_selected: encoding.selected(),
        encoding: encoding.identity(),
        row_count: encoding.rows().len(),
        encoded_count,
        deferred_count,
    }
}

#[cfg(test)]
pub(crate) fn corrupt_active_resident_selected_form_encoding_byte_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
) {
    let bytes = staged
        .encoding
        .rows_mut()
        .iter_mut()
        .find_map(|row| match &mut row.state {
            SelectedFormEncodingState::Encoded { bytes, .. } => Some(bytes),
            SelectedFormEncodingState::DeferredControl { .. } => None,
        })
        .expect("active-resident fixture must retain one scalar encoding");
    bytes[0] ^= 1;
}

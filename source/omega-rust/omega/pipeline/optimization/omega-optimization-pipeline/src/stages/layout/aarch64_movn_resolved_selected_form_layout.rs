use omega_regalloc::ValidatedSelectedAnalysis;

use crate::{
    OptimizedPostAllocationMachineOptimizationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    OptimizedResolvedSelectedFormLayoutError, OptimizedSelectedFormEncodingError,
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
    StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization,
    stage_optimized_resolved_selected_form_layout,
    stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
    validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody,
    validate_optimized_aarch64_movn_materialization_custody,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody, validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
};

/// Owning resolved-layout carrier for the exact direct-homes shortest-MOVN
/// transformation. Baseline and transformed artifacts remain together so the
/// byte reduction is replayable without granting exit, emission, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedAarch64MovnResolvedSelectedFormLayout {
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    materialization: StagedOptimizedAarch64MovnMaterialization,
    baseline_encoding: StagedOptimizedSelectedFormEncoding,
    encoding: StagedOptimizedSelectedFormEncoding,
    baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    custody: StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
}

impl StagedOptimizedAarch64MovnResolvedSelectedFormLayout {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomes {
        &self.homes
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn materialization(&self) -> &StagedOptimizedAarch64MovnMaterialization {
        &self.materialization
    }

    pub const fn baseline_encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.baseline_encoding
    }

    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }

    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }

    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }

    pub const fn custody(
        &self,
    ) -> &StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
        &self.custody
    }
}

/// The same owning MOVN layout boundary after an exact named selected-lowering
/// run. The selected-lowering completion remains nested in `homes`.
#[derive(Debug)]
pub struct StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout {
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: StagedOptimizedPostAllocationMachinePlan,
    materialization: StagedOptimizedAarch64MovnMaterialization,
    baseline_encoding: StagedOptimizedSelectedFormEncoding,
    encoding: StagedOptimizedSelectedFormEncoding,
    baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    layout: StagedOptimizedResolvedSelectedFormLayout,
    custody: StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
}

impl StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout {
    pub const fn homes(&self) -> &StagedOptimizedRegisterHomesAfterSelectedLowering {
        &self.homes
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn materialization(&self) -> &StagedOptimizedAarch64MovnMaterialization {
        &self.materialization
    }

    pub const fn baseline_encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.baseline_encoding
    }

    pub const fn encoding(&self) -> &StagedOptimizedSelectedFormEncoding {
        &self.encoding
    }

    pub const fn baseline_layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.baseline_layout
    }

    pub const fn layout(&self) -> &StagedOptimizedResolvedSelectedFormLayout {
        &self.layout
    }

    pub const fn custody(
        &self,
    ) -> &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    materialization: StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    baseline_encoding: SelectedFormEncodingIdentity,
    encoding: SelectedFormEncodingIdentity,
    baseline_layout: ResolvedSelectedFormLayoutIdentity,
    layout: ResolvedSelectedFormLayoutIdentity,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
    pub const fn source(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.source
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }

    pub const fn materialization(&self) -> StagedOptimizedAarch64MovnMaterializationCustodyReceipt {
        self.materialization
    }

    pub const fn physical(&self) -> omega_register_model::PhysicalRegisterModelIdentity {
        self.physical
    }

    pub const fn baseline_encoding(&self) -> SelectedFormEncodingIdentity {
        self.baseline_encoding
    }

    pub const fn encoding(&self) -> SelectedFormEncodingIdentity {
        self.encoding
    }

    pub const fn baseline_layout(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.baseline_layout
    }

    pub const fn layout(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.layout
    }

    pub const fn baseline_bytes(&self) -> u64 {
        self.baseline_bytes
    }

    pub const fn selected_bytes(&self) -> u64 {
        self.selected_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
    source: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    materialization: StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    baseline_encoding: SelectedFormEncodingIdentity,
    encoding: SelectedFormEncodingIdentity,
    baseline_layout: ResolvedSelectedFormLayoutIdentity,
    layout: ResolvedSelectedFormLayoutIdentity,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedPostSelectedLoweringHomeCustodyReceipt {
        &self.source
    }

    pub const fn machine(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.machine
    }

    pub const fn materialization(&self) -> StagedOptimizedAarch64MovnMaterializationCustodyReceipt {
        self.materialization
    }

    pub const fn physical(&self) -> omega_register_model::PhysicalRegisterModelIdentity {
        self.physical
    }

    pub const fn baseline_encoding(&self) -> SelectedFormEncodingIdentity {
        self.baseline_encoding
    }

    pub const fn encoding(&self) -> SelectedFormEncodingIdentity {
        self.encoding
    }

    pub const fn baseline_layout(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.baseline_layout
    }

    pub const fn layout(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.layout
    }

    pub const fn baseline_bytes(&self) -> u64 {
        self.baseline_bytes
    }

    pub const fn selected_bytes(&self) -> u64 {
        self.selected_bytes
    }
}

#[derive(Debug)]
pub enum OptimizedAarch64MovnResolvedSelectedFormLayoutError {
    DirectHomes(OptimizedRegisterHomeCustodyError),
    SelectedLoweringHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    Machine(OptimizedPostAllocationMachinePipelineError),
    Materialization(OptimizedPostAllocationMachineOptimizationError),
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedAarch64MovnResolvedSelectedFormLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized AArch64 MOVN resolved selected-form layout failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAarch64MovnResolvedSelectedFormLayoutError {}

pub fn stage_optimized_aarch64_movn_resolved_selected_form_layout(
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    materialization: StagedOptimizedAarch64MovnMaterialization,
) -> Result<
    StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
> {
    let source = validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::DirectHomes)?;
    if source != homes.custody() {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    let machine_receipt = validate_optimized_post_allocation_machine_plan_custody(&homes, &machine)
        .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Machine)?;
    if &machine_receipt != machine.custody() {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    let materialization_receipt =
        validate_optimized_aarch64_movn_materialization_custody(&homes, &machine, &materialization)
            .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Materialization)?;
    if materialization_receipt != materialization.custody() {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let (baseline_encoding, encoding, baseline_layout, layout) = build_artifacts(
        selected_stage.selected(),
        &machine,
        physical,
        &materialization,
    )?;
    let custody = direct_custody(
        source,
        machine_receipt,
        materialization_receipt,
        physical.identity(),
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
    )?;
    let staged = StagedOptimizedAarch64MovnResolvedSelectedFormLayout {
        homes,
        machine,
        materialization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        custody,
    };
    validate_optimized_aarch64_movn_resolved_selected_form_layout(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_aarch64_movn_resolved_selected_form_layout(
    staged: &StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
> {
    let source = validate_optimized_register_home_custody(
        staged.homes.legality_stage(),
        staged.homes.homes(),
        staged.homes.post_allocation_manifest(),
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::DirectHomes)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Machine)?;
    let materialization = validate_optimized_aarch64_movn_materialization_custody(
        &staged.homes,
        &staged.machine,
        &staged.materialization,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Materialization)?;
    let selected_stage = staged
        .homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    validate_artifacts(
        selected_stage.selected(),
        &staged.machine,
        selected_stage.register_environment().physical(),
        &staged.materialization,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
    )?;
    let custody = direct_custody(
        source,
        machine,
        materialization,
        selected_stage.register_environment().physical().identity(),
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
    )?;
    if custody != staged.custody {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn stage_selected_lowering_aarch64_movn_resolved_selected_form_layout(
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: StagedOptimizedPostAllocationMachinePlan,
    materialization: StagedOptimizedAarch64MovnMaterialization,
) -> Result<
    StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
> {
    let source = validate_optimized_register_home_after_selected_lowering_custody(&homes)
        .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::SelectedLoweringHomes)?;
    if &source != homes.custody() {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    let machine_receipt =
        validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
            &homes, &machine,
        )
        .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Machine)?;
    if &machine_receipt != machine.custody() {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    let materialization_receipt =
        validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody(
            &homes,
            &machine,
            &materialization,
        )
        .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Materialization)?;
    if materialization_receipt != materialization.custody() {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    let run = homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let artifacts = match run.steps().last() {
        Some(step) => build_artifacts(step.fold(), &machine, physical, &materialization),
        None => build_artifacts(
            selected_stage.selected(),
            &machine,
            physical,
            &materialization,
        ),
    }?;
    let (baseline_encoding, encoding, baseline_layout, layout) = artifacts;
    let custody = selected_lowering_custody(
        source,
        machine_receipt,
        materialization_receipt,
        physical.identity(),
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
    )?;
    let staged = StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout {
        homes,
        machine,
        materialization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        custody,
    };
    validate_selected_lowering_aarch64_movn_resolved_selected_form_layout(&staged)?;
    Ok(staged)
}

pub fn validate_selected_lowering_aarch64_movn_resolved_selected_form_layout(
    staged: &StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayout,
) -> Result<
    StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
> {
    let source = validate_optimized_register_home_after_selected_lowering_custody(&staged.homes)
        .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::SelectedLoweringHomes)?;
    let machine = validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        &staged.homes,
        &staged.machine,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Machine)?;
    let materialization =
        validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody(
            &staged.homes,
            &staged.machine,
            &staged.materialization,
        )
        .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Materialization)?;
    let run = staged.homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    match run.steps().last() {
        Some(step) => validate_artifacts(
            step.fold(),
            &staged.machine,
            physical,
            &staged.materialization,
            &staged.baseline_encoding,
            &staged.encoding,
            &staged.baseline_layout,
            &staged.layout,
        ),
        None => validate_artifacts(
            selected_stage.selected(),
            &staged.machine,
            physical,
            &staged.materialization,
            &staged.baseline_encoding,
            &staged.encoding,
            &staged.baseline_layout,
            &staged.layout,
        ),
    }?;
    let custody = selected_lowering_custody(
        source,
        machine,
        materialization,
        physical.identity(),
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
    )?;
    if custody != staged.custody {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    Ok(custody)
}

fn build_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        StagedOptimizedResolvedSelectedFormLayout,
    ),
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
> {
    let baseline_encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Encoding)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization(
            selected,
            machine,
            physical,
            materialization,
        )
        .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Encoding)?;
    let baseline_layout = stage_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        &baseline_encoding,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Layout)?;
    let layout = stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization(
        selected,
        machine,
        physical,
        &encoding,
        materialization,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Layout)?;
    validate_layout_pair(&baseline_layout, &layout, materialization.custody())?;
    Ok((baseline_encoding, encoding, baseline_layout, layout))
}

#[allow(clippy::too_many_arguments)]
fn validate_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedAarch64MovnResolvedSelectedFormLayoutError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        machine,
        physical,
        baseline_encoding,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Encoding)?;
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization(
        selected,
        machine,
        physical,
        materialization,
        encoding,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        baseline_encoding,
        baseline_layout,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Layout)?;
    validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization(
        selected,
        machine,
        physical,
        encoding,
        materialization,
        layout,
    )
    .map_err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::Layout)?;
    validate_layout_pair(baseline_layout, layout, materialization.custody())
}

fn validate_layout_pair(
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    selected: &StagedOptimizedResolvedSelectedFormLayout,
    materialization: StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
) -> Result<(), OptimizedAarch64MovnResolvedSelectedFormLayoutError> {
    let baseline_bytes = layout_byte_count(baseline)?;
    let selected_bytes = layout_byte_count(selected)?;
    let expected_savings = materialization
        .baseline_words()
        .checked_sub(materialization.selected_words())
        .and_then(|words| words.checked_mul(4))
        .ok_or(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch)?;
    if baseline.selected() != selected.selected()
        || baseline.machine() != selected.machine()
        || baseline.target() != selected.target()
        || baseline.policy() != selected.policy()
        || baseline.functions().len() != selected.functions().len()
        || baseline.structural_unit_functions() != selected.structural_unit_functions()
        || baseline_bytes.checked_sub(selected_bytes) != Some(expected_savings)
    {
        return Err(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch);
    }
    Ok(())
}

fn layout_byte_count(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<u64, OptimizedAarch64MovnResolvedSelectedFormLayoutError> {
    layout
        .functions()
        .iter()
        .try_fold(0_u64, |total, function| {
            total
                .checked_add(function.byte_count)
                .ok_or(OptimizedAarch64MovnResolvedSelectedFormLayoutError::ReceiptMismatch)
        })
}

#[allow(clippy::too_many_arguments)]
fn direct_custody(
    source: StagedOptimizedRegisterHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    materialization: StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
> {
    Ok(
        StagedOptimizedAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
            source,
            machine,
            materialization,
            physical,
            baseline_encoding: baseline_encoding.identity(),
            encoding: encoding.identity(),
            baseline_layout: baseline_layout.identity(),
            layout: layout.identity(),
            baseline_bytes: layout_byte_count(baseline_layout)?,
            selected_bytes: layout_byte_count(layout)?,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn selected_lowering_custody(
    source: StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    machine: StagedOptimizedPostAllocationMachineCustodyReceipt,
    materialization: StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    physical: omega_register_model::PhysicalRegisterModelIdentity,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt,
    OptimizedAarch64MovnResolvedSelectedFormLayoutError,
> {
    Ok(
        StagedSelectedLoweringAarch64MovnResolvedSelectedFormLayoutCustodyReceipt {
            source,
            machine,
            materialization,
            physical,
            baseline_encoding: baseline_encoding.identity(),
            encoding: encoding.identity(),
            baseline_layout: baseline_layout.identity(),
            layout: layout.identity(),
            baseline_bytes: layout_byte_count(baseline_layout)?,
            selected_bytes: layout_byte_count(layout)?,
        },
    )
}

#[cfg(test)]
pub(crate) fn corrupt_aarch64_movn_resolved_layout_byte_for_test(
    staged: &mut StagedOptimizedAarch64MovnResolvedSelectedFormLayout,
) {
    let byte = staged
        .layout
        .functions_mut()
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| instruction.bytes.first_mut())
        .expect("MOVN resolved-layout fixture must contain encoded bytes");
    *byte ^= 1;
}

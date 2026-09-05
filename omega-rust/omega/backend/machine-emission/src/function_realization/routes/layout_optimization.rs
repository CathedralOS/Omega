use super::super::{assembly::*, carriers::*, error::*, prelude::*};
use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

pub fn stage_function_relative_layout_optimization_realization(
    allocation: RetainedAllocation,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedFunctionRelativeLayoutOptimizationRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    let current = allocation
        .replay_allocation()
        .map_err(FunctionRelativeOptimizationRealizationError::Allocation)?;
    let source = baseline_allocation_source(&current)?;
    validate_optimized_post_allocation_machine_plan_custody(&current, &machine)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let selected = current.selected();
    let physical = current.register_environment().physical();
    let selections = current.selections();
    if !rel8_selected(
        selections,
        current.register_environment().target().architecture,
    )? {
        return Err(
            FunctionRelativeOptimizationRealizationError::MissingFunctionRelativeLayoutOptimization,
        );
    }
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let relaxation = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        current.budget_per_pass(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)?;
    let exit_contract = stage_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        &relaxation,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_direct_manifest(
        &current,
        &machine,
        &encoding,
        &baseline_layout,
        &relaxation,
        &exit_contract,
    )?;
    let custody = direct_custody_receipt(source, &machine, &relaxation, &exit_contract, &manifest);
    Ok(StagedFunctionRelativeLayoutOptimizationRealization {
        allocation,
        machine,
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_function_relative_layout_optimization_realization_custody(
    staged: &StagedFunctionRelativeLayoutOptimizationRealization,
) -> Result<
    StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let current = staged
        .allocation
        .replay_allocation()
        .map_err(FunctionRelativeOptimizationRealizationError::Allocation)?;
    let source = baseline_allocation_source(&current)?;
    if source != staged.custody.source() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&current, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let selected = current.selected();
    let physical = current.register_environment().physical();
    let selections = current.selections();
    if !rel8_selected(
        selections,
        current.register_environment().target().architecture,
    )? {
        return Err(
            FunctionRelativeOptimizationRealizationError::MissingFunctionRelativeLayoutOptimization,
        );
    }
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_optimized_x86_branch_relaxation(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)?;
    validate_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_direct_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
        &staged.exit_contract,
    )?;
    if manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = direct_custody_receipt(
        source,
        &staged.machine,
        &staged.relaxation,
        &staged.exit_contract,
        &manifest,
    );
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

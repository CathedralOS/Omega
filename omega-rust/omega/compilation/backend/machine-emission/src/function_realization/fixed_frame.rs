//! Join produced encoding/layout artifacts with their exact fixed-frame custody.
//!
//! Native compilation sequences stages 07-09. This owner constructs the exit
//! contract and seals the realization; retained readers can replay its joins.

use super::assembly::{expected_fixed_frame_manifest, fixed_frame_custody};
use super::carriers::{
    StagedFixedFrameFunctionRelativeRealization,
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
};
use super::error::FunctionRelativeOptimizationRealizationError;
use crate::exit_contract::stage_whole_function_exit_contract_for_layout;
use crate::frame_layout::TargetFrameLayoutPolicy;
use optimization_core::OptimizationExecutionPhase;
use post_allocation_machine_to_selected_form_encoding::{
    StagedOptimizedSelectedFormEncoding,
    validate_optimized_layout_independent_selected_form_encoding,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use resolved_layout_to_resolved_layout::{
    ResolvedLayoutOptimization, validate_resolved_layout_optimization,
};
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use selected_instructions_to_register_homes::RetainedAllocation;

/// Bind the pipeline's produced frame, encoding and layout to one allocation.
/// This checks their joins and exit contract without running a compiler stage.
pub fn assemble_fixed_frame_realization(
    allocation: RetainedAllocation,
    machine: StagedOptimizedPostAllocationMachinePlan,
    frame: super::FunctionRelativeFrame,
    encoding: StagedOptimizedSelectedFormEncoding,
    baseline_layout: StagedOptimizedResolvedSelectedFormLayout,
    layout_optimization: ResolvedLayoutOptimization,
) -> Result<StagedFixedFrameFunctionRelativeRealization, FunctionRelativeOptimizationRealizationError>
{
    // The allocation and the machine plan were built from the same current
    // facts in this compile; neither is replayed here.
    let current = allocation.current();
    let source = current.evidence().clone();
    let selected = current.selected();
    let environment = current.register_environment();
    let physical = environment.physical();
    let exit_contract = stage_whole_function_exit_contract_for_layout(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        &layout_optimization,
        Some((frame.layout(), frame.protocol())),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(
        &current,
        &machine,
        &encoding,
        &baseline_layout,
        &layout_optimization,
        frame.layout(),
        frame.protocol(),
        &exit_contract,
    )?;
    let custody = fixed_frame_custody(
        source.clone(),
        &machine,
        frame.requirements(),
        frame.storage(),
        frame.layout(),
        frame.protocol(),
        &exit_contract,
        &manifest,
    );
    Ok(StagedFixedFrameFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        baseline_layout,
        layout_optimization,
        frame,
        exit_contract,
        manifest,
        custody,
    })
}

/// Replay every join of a staged realization against its retained inputs.
/// The stage does not call this on what it just produced (verification
/// placement in `omega-rust/pipeline.md`); retained-artifact readers and tests
/// do.
pub(crate) fn validate_fixed_frame_function_relative_realization(
    staged: &StagedFixedFrameFunctionRelativeRealization,
) -> Result<
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let current = staged.allocation.current();
    let source = current.evidence().clone();
    let selected = current.selected();
    let environment = current.register_environment();
    let physical = environment.physical();
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        Some(staged.frame.layout().plan()),
        &staged.encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_resolved_layout_optimization(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
        &current
            .selections()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
        &staged.layout_optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)?;
    super::frame::validate_frame(
        &current,
        &staged.machine,
        &staged.frame,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )?;
    let frame = &staged.frame;
    // Encoding, exact source selections, layout and frame were replayed above
    // against these same immutable inputs. Only the exit record remains.
    crate::exit_contract::validate_exit_record_for_replayed_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout_optimization,
        Some((frame.layout(), frame.protocol())),
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout_optimization,
        frame.layout(),
        frame.protocol(),
        &staged.exit_contract,
    )?;
    let custody = fixed_frame_custody(
        source.clone(),
        &staged.machine,
        frame.requirements(),
        frame.storage(),
        frame.layout(),
        frame.protocol(),
        &staged.exit_contract,
        &manifest,
    );
    if &source != staged.custody.source()
        || manifest.record != staged.manifest.record
        || custody != staged.custody
    {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

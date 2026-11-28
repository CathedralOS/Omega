//! Apply encoded frame fragments, place functions and resolve internal calls before object construction.
#![forbid(unsafe_code)]

use std::sync::Arc;

pub mod code_layout;
mod exit_contract;
mod fragment_emission;
mod frame_application;
mod function_realization;
mod relative_branches;
#[cfg(test)]
mod test_plans;
mod text_placement;

pub use exit_contract::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitEvidence,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence,
};
pub use frame_application::{
    CheckedFunctionBranches, FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol,
    FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError, PlacedBodyEvidence,
    StagedFunctionFragmentFrameApplication,
};
pub use function_realization::FunctionRelativeOptimizationRealizationError;
pub use machine_code::fragment_origins::ResolvedFragmentEmissionError as FunctionFragmentEmissionError;

pub use text_placement::{
    FunctionPlacement, RelocationFreeTextSectionPlacementError,
    StagedOptimizedFixedFrameTextSection, validate_optimized_fixed_frame_text_section,
};

/// Place the function-relative alternatives and their carried frame bytes into
/// one closed text section, including internal calls and external relocations.
pub fn place_function_text<Contracts>(
    layout: placement::ResolvedFunctionLayout<Contracts>,
    fragments_entered: &std::sync::atomic::AtomicU64,
) -> Result<FunctionPlacement<Contracts>, FunctionTextLayoutError> {
    use FunctionTextLayoutError::*;
    let (layout, contracts) = layout.into_placement().ok_or(Placement(
        RelocationFreeTextSectionPlacementError::ArtifactMismatch,
    ))?;
    let exits = function_realization::checked_exits(&layout).map_err(Realization)?;
    let program = &layout.program;
    fragments_entered.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let selections = program.context.publication.selections().identity();
    let fragment_emission::FramedFunctions {
        application,
        unframed,
        branches,
    } = fragment_emission::emit_functions(
        program,
        contracts.retain_original,
        contracts.check_installed_branches,
    )?;
    let application = Arc::new(application);
    let body = Arc::new(PlacedBodyEvidence::capture(
        program,
        &application,
        &branches,
        contracts.check_installed_branches,
    ));
    let section = text_placement::place_fixed_frame(&application.fragments).map_err(Placement)?;
    let native_layouts =
        code_layout::requested_native_code_layouts(&section, &application.functions);
    let text = StagedOptimizedFixedFrameTextSection {
        contracts: contracts.publication,
        encoded: contracts.encoded,
        body: Arc::clone(&body),
        text_section: Arc::new(section),
        native_layouts,
        selections,
    };
    Ok(FunctionPlacement {
        text,
        frame_application: unframed.map(|unframed| StagedFunctionFragmentFrameApplication {
            layout,
            exits,
            unframed,
            placed_body: body,
        }),
    })
}

#[derive(Debug)]
pub enum FunctionTextLayoutError {
    Realization(FunctionRelativeOptimizationRealizationError),
    Fragment(FunctionFragmentEmissionError),
    Frame(FunctionFragmentFrameApplicationError),
    Placement(RelocationFreeTextSectionPlacementError),
}
impl std::fmt::Display for FunctionTextLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Realization(error) => {
                write!(f, "function-relative realization failed: {error:?}")
            }
            Self::Fragment(error) => write!(
                f,
                "native artifact function-fragment emission failed: {error}"
            ),
            Self::Frame(error) => write!(
                f,
                "native artifact function-fragment frame application failed: {error}"
            ),
            Self::Placement(error) => {
                write!(f, "native artifact framed text placement failed: {error}")
            }
        }
    }
}
impl std::error::Error for FunctionTextLayoutError {}

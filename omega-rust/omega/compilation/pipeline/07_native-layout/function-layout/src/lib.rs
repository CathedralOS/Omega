//! Apply encoded frame fragments, place functions and resolve internal calls before object construction.
#![forbid(unsafe_code)]

pub(crate) use ::wire::codec_bytes;

pub mod code_layout;

mod exit_contract;
mod fragment_emission;
mod frame_application;
mod function_realization;
mod text_placement;

pub use exit_contract::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
    WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};
pub use fragment_emission::{
    FunctionFragmentEmissionError, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionStage,
    FunctionFragmentEmissionStatistics, FunctionFragmentEmissionUnavailableData,
    StagedOptimizedFunctionFragmentEmissionSource,
};
pub use frame_application::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationIdentity,
    StagedFunctionFragmentFrameApplication, function_fragment_frame_application_identity,
    validate_current_relative_branch_rows,
};
pub use function_realization::{
    FunctionRelativeOptimizationRealizationError, StagedFixedFrameFunctionRelativeRealization,
};

pub use text_placement::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError,
    StagedOptimizedFixedFrameTextSection, validate_optimized_fixed_frame_text_section,
};

/// Place the function-relative alternatives and their carried frame bytes into
/// one closed text section, including internal calls and external relocations.
pub fn place_function_text(
    layout: machine_code::OptimizedMachineLayout,
    verification: native_verification::PhysicalVerificationEvidence,
    facts: physical_program::EmissionFacts,
    installations: std::sync::Arc<installation_binding::BoundInstallationAuthority>,
    fragments_entered: &std::sync::atomic::AtomicU64,
) -> Result<StagedOptimizedFixedFrameTextSection, FunctionTextLayoutError> {
    use FunctionTextLayoutError::*;
    let realized = function_realization::assemble_fixed_frame_realization(layout, verification)
        .map_err(Realization)?;
    let source = StagedOptimizedFunctionFragmentEmissionSource::new(realized, facts, installations)
        .map_err(Fragment)?;
    fragments_entered.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let fragments =
        fragment_emission::stage_optimized_function_fragment_emission(source).map_err(Fragment)?;
    let framed =
        frame_application::stage_function_fragment_frame_application(fragments).map_err(Frame)?;
    text_placement::stage_optimized_fixed_frame_text_section(framed).map_err(Placement)
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

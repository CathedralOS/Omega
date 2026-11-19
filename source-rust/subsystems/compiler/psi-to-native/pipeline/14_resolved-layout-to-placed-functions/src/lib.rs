//! Apply encoded frame fragments, place functions and resolve internal calls before object construction.
#![forbid(unsafe_code)]

pub(crate) use ::wire::codec_bytes;

pub mod code_layout;
pub mod entry_placement {
    pub use relaxation::entry_layout::{AdapterCallField, ResolvedBridge, ResolvedCallableEntry};
    use relaxation::entry_layout::{ResolvedForeignAdapter, ResolvedPrivateEntry};

    pub type PlacedForeignAdapter = super::PlacedEntry<ResolvedForeignAdapter>;
    pub type PlacedPrivateEntry = super::PlacedEntry<ResolvedPrivateEntry>;
    pub type PlacedCallableEntry = super::PlacedEntry<ResolvedCallableEntry>;
}

mod exit_contract;
mod fragment_emission;
mod frame_application;
mod function_realization;
mod relative_branches;
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
    CheckedFunctionBranches, FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol,
    FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError,
    FunctionFragmentFrameApplicationIdentity, PlacedBodyEvidence,
    StagedFunctionFragmentFrameApplication, function_fragment_frame_application_identity,
};
pub use function_realization::{
    FunctionRelativeOptimizationRealizationError, StagedFixedFrameFunctionRelativeRealization,
};

pub use text_placement::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, FunctionPlacement,
    RelocationFreeTextSectionPlacementError, StagedOptimizedFixedFrameTextSection,
    validate_optimized_fixed_frame_text_section,
};

/// Place the function-relative alternatives and their carried frame bytes into
/// one closed text section, including internal calls and external relocations.
pub fn place_function_text<Contracts>(
    layout: relaxation::RelaxedLayout<Contracts>,
    fragments_entered: &std::sync::atomic::AtomicU64,
) -> Result<FunctionPlacement<Contracts>, FunctionTextLayoutError> {
    use FunctionTextLayoutError::*;
    let (layout, contracts) = layout.into_placement().ok_or(Placement(
        RelocationFreeTextSectionPlacementError::ArtifactMismatch,
    ))?;
    let realized =
        function_realization::assemble_fixed_frame_realization(layout).map_err(Realization)?;
    let source = StagedOptimizedFunctionFragmentEmissionSource::new(realized);
    fragments_entered.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let fragments =
        fragment_emission::stage_optimized_function_fragment_emission(source).map_err(Fragment)?;
    let framed = frame_application::stage_function_fragment_frame_application(
        fragments,
        contracts.check_installed_branches,
        contracts.retain_original,
    )
    .map_err(Frame)?;
    text_placement::stage_optimized_fixed_frame_text_section(framed, contracts).map_err(Placement)
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

pub mod program_entry_placement {
    use relaxation::program_entry_layout::{ResolvedHostedProgram, ResolvedPhysicalProgram};

    pub type PlacedHostedProgram<C> = super::PlacedEntry<ResolvedHostedProgram<C>>;
    pub type PlacedPhysicalProgram<C> = super::PlacedEntry<ResolvedPhysicalProgram<C>>;
}

/// Each fixed entry occupies one fragment whose origin is offset zero.
pub struct PlacedEntry<T>(T);

/// A raw entry cannot skip relaxation.
///
/// ```compile_fail,E0308
/// fn skip_stage<O>(raw: O) {
///     let _ = function_layout::place_fixed_entry(raw);
/// }
/// ```
pub fn place_fixed_entry<T>(entry: relaxation::FixedEntry<T>) -> PlacedEntry<T> {
    PlacedEntry(entry.into_layout())
}

impl<T> PlacedEntry<T> {
    pub fn into_layout(self) -> T {
        self.0
    }
}

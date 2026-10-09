#![forbid(unsafe_code)]

//! Machine emission: the backend stages that turn one allocated
//! post-allocation machine plan into placed function text, and the
//! target-owned x86-64 byte recipes emitted outside that route.
//!
//! Native realization drives the route one retained stage at a time. Each
//! `stage_*` operation consumes the previous stage's product as produced and
//! runs its production. Each stage's `validate_*` replay of that join stays
//! available to retained-artifact readers and tests; the stages do not replay
//! what the same compile just produced (verification placement in
//! `omega-rust/pipeline.md`):
//!
//! 1. [`stage_fixed_frame_function_relative_realization`]
//!    ([`function_realization`]) reads the current allocation, stages the frame,
//!    encodes and lays out the selected forms, and stages the whole-function
//!    exit contract.
//! 2. [`stage_optimized_function_fragment_emission`] ([`fragment_emission`])
//!    projects that realization into unplaced function fragments.
//! 3. [`stage_function_fragment_frame_application`] ([`frame_application`])
//!    inserts the frame's prologue and epilogue bytes into those fragments.
//! 4. [`stage_optimized_fixed_frame_text_section`] ([`text_placement`])
//!    places the framed fragments into one dense relocation-free text section.
//!
//! Each of the last three stages keeps admission and custody at its folder
//! root and its current-data mechanism, with an independent checker, one
//! level down: `fragment_emission::projection`, `frame_application::insertion`
//! and `text_placement::placement`.
//!
//! [`private_callback`] projects placed callback text into a private function
//! record using the target ABI retained by that route. [`code_layout`]
//! projects placed text into the per-application entry-and-region
//! descriptions a retained `code_layout<Machine>()` query answers with.
//! Neither is another stage.
//!
//! Realization joins three evidence owners, each with its own stage and
//! independent replay: [`frame_layout`] (frame geometry and callee-save
//! storage), [`frame_protocol`] (the frame's encoded prologue and epilogue
//! bytes) and [`exit_contract`] (whole-function exit evidence).
//!
//! Outside the route, [`startup_trampoline`], [`entry_exit_stub`] and
//! [`x86_fma`] emit target-owned x86-64 byte recipes with their own replay.
//! The object join over placed text belongs to object-file and image-emission.

// 1. Function-relative realization.
mod function_realization;

pub use function_realization::{
    stage_fixed_frame_function_relative_realization,
};

pub use function_realization::{
    FunctionRelativeOptimizationRealizationError,
};
pub use function_realization::{
    StagedFixedFrameFunctionRelativeRealization};
pub use function_realization::{
    ValidatedFunctionRelativeOptimizationRealizationManifest};
pub use function_realization::{ FunctionRelativeOptimizationRealizationManifest};

// Evidence the realization joins: frame geometry, frame bytes, exit contract.
mod exit_contract;
pub mod frame_layout;
mod frame_protocol;


pub use frame_protocol::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, target_frame_protocol_encoding_identity,
};


pub use exit_contract::{ WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity, WholeFunctionExitEvidence,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence,
};
pub use exit_contract::{
    ValidatedWholeFunctionExitContract};

// 2. Fragment emission.
mod fragment_emission;

pub use fragment_emission::{
    stage_optimized_function_fragment_emission,
};

pub use fragment_emission::{ FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionManifestDecodeError, FunctionFragmentEmissionStage,
    FunctionFragmentEmissionStatistics, FunctionFragmentEmissionUnavailableData,
    StagedOptimizedFunctionFragmentEmissionSource,
};
pub use fragment_emission::{
    FunctionFragmentEmissionError};
pub use fragment_emission::{ StagedOptimizedFunctionFragmentEmission, ValidatedFunctionFragmentEmissionManifest};


// 3. Frame application.
mod frame_application;

pub use frame_application::{
    stage_function_fragment_frame_application,
};

pub use frame_application::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication, FunctionFragmentFrameApplicationIdentity,
    function_fragment_frame_application_identity,
};
pub use frame_application::{
    FunctionFragmentFrameApplicationError};
pub use frame_application::{ StagedFunctionFragmentFrameApplication};


// 4. Text placement.
mod text_placement;

pub use text_placement::{
    stage_optimized_fixed_frame_text_section, validate_optimized_fixed_frame_text_section,
};

pub use text_placement::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError, StagedOptimizedFixedFrameTextSection,
};
pub use text_placement::{
    StagedFixedFrameTextSectionCustodyReceipt,
    ValidatedFunctionFragmentTextSectionManifest};


// Private callback records projected from placed text.
mod private_callback;

pub use private_callback::{
    PrivateCallbackMaterializationRequest, materialize_private_callback_function,
};

// `code_layout<Machine>()` descriptions projected from placed text.


// Target-owned x86-64 byte recipes outside the route.



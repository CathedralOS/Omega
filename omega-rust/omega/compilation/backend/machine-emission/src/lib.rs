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
//! 1. [`stage_function_frame`] prepares frame geometry and its byte protocol.
//!    The native pipeline owns encoding and layout stages 07-09;
//!    [`assemble_fixed_frame_realization`] then joins their products and
//!    constructs the whole-function exit contract.
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
//! [`code_layout`] projects placed text into the per-application entry-and-region
//! descriptions a retained `code_layout<Machine>()` query answers with.
//! This is a query, not another stage.
//!
//! Realization joins three evidence owners, each with its own stage and
//! independent replay: [`frame_layout`] (frame geometry and callee-save
//! storage), [`frame_protocol`] (the frame's encoded prologue and epilogue
//! bytes) and [`exit_contract`] (whole-function exit evidence).
//!
//! Outside the route, [`native_entry_bridge`] snapshots admitted native
//! inputs, transports them into an unchanged ordinary body's convention and
//! retains its unresolved call plus exact frame demand. It owns no synthetic
//! semantic machine.
//! The object join over placed text belongs to object-file and image-emission.

// 1. Function-relative realization.
pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;
mod function_realization;
pub mod native_entry_bridge;

pub use function_realization::{
    FunctionRelativeFrame, StagedFixedFrameFunctionRelativeRealization,
    assemble_fixed_frame_realization, stage_function_frame,
};

pub use function_realization::FunctionRelativeOptimizationRealizationError;
// Evidence the realization joins: frame geometry, frame bytes, exit contract.
mod exit_contract;
pub mod frame_layout;
mod frame_protocol;

pub use frame_protocol::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, target_frame_protocol_encoding_identity,
};

pub use exit_contract::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
    WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};
// 2. Fragment emission.
mod fragment_emission;

pub use fragment_emission::stage_optimized_function_fragment_emission;

pub use fragment_emission::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionManifestDecodeError,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, StagedOptimizedFunctionFragmentEmissionSource,
};
// 3. Frame application.
mod frame_application;

pub use frame_application::stage_function_fragment_frame_application;

pub use frame_application::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationIdentity, function_fragment_frame_application_identity,
};
// 4. Text placement.
mod text_placement;

pub use text_placement::{
    stage_optimized_fixed_frame_text_section, validate_optimized_fixed_frame_text_section,
};

pub use text_placement::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError,
    StagedOptimizedFixedFrameTextSection,
};
// `code_layout<Machine>()` descriptions projected from placed text.
pub mod code_layout;

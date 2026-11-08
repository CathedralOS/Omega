/*
We lower allocated post-allocation machine plans (`PostAllocationMachinePlan`) and their stage 07–09 encodings and layouts into placed native function text (`StagedOptimizedFixedFrameTextSection`), and we emit the target-owned native entry and foreign C calling bridges (`native_entry_bridge`, `native_foreign_bridge`) that sit outside the ordinary function compilation route. Native realization drives our four route stages in sequence:
1. `stage_function_frame` and `assemble_fixed_frame_realization` (`function_realization`) compute per-function callee-save storage and frame geometry (`frame_layout`), encode target prologue and epilogue byte spans (`frame_protocol`), join the stage 07–09 encoding and layout artifacts, and stage whole-function exit evidence (`exit_contract`).
2. `stage_optimized_function_fragment_emission` (`fragment_emission`) projects the fixed-frame realization into unplaced function fragments while preserving maximal source-attributed byte intervals and unresolved call or image-address fixup records.
3. `stage_function_fragment_frame_application` (`frame_application`) inserts each function's prologue at block entry and epilogue before every retained return, reflows block, instruction, and fixup offsets, and re-encodes any relative branches whose displacements cross an inserted frame sequence.
4. `stage_optimized_fixed_frame_text_section` (`text_placement`) places the framed function fragments into one aligned text section and resolves internal machine calls.
Alongside the four route stages, `code_layout` answers `code_layout<Machine>()` queries from placed text, and `native_entry_bridge` / `native_foreign_bridge` / `native_bridge_encoding` synthesize replayable byte adapters between external C or interrupt frames and Omega calling plans.
*/

/*
Keeping frame geometry, frame byte protocols, whole-function exit contracts, fragment projection, frame insertion, and text-section placement as distinct stages with independent `validate_*` replay entrypoints lets downstream object and image owners verify retained artifacts at trust boundaries without re-running register allocation or re-verifying stage outputs during normal compilation. Separating prologue/epilogue insertion (`frame_application`) from pre-frame branch layout (`Omega 08–09`) ensures that branch relaxation operates on exact body displacements first and that any branch stretched by an inserted epilogue is deterministically widened and re-encoded before text-section placement fixes final section offsets.
*/

/*
- Route stages and query owners:
  - `function_realization`: `stage_function_frame` and `assemble_fixed_frame_realization`, joining `frame_layout`, `frame_protocol`, and `exit_contract`.
  - `fragment_emission`: `stage_optimized_function_fragment_emission` and manifest/replay custody for unplaced function fragments.
  - `frame_application`: `stage_function_fragment_frame_application`, inserting prologues/epilogues and reflowing relative branches.
  - `text_placement`: `stage_optimized_fixed_frame_text_section` and `validate_optimized_fixed_frame_text_section`.
  - `code_layout`: `describe_native_code_layout`, projecting attributed instruction and frame spans for `code_layout<Machine>()` queries.
- Native bridge owners:
  - `native_entry_bridge`: `emit_native_entry_bridge`, `emit_installed_fatal_entry`, and `emit_installed_interrupt_entry`.
  - `native_foreign_bridge`: `emit_native_foreign_bridge` and `validate_native_foreign_bridge`.
  - `native_bridge_encoding`: shared physical register, stack-slot, and over-aligned frame home transfers used by both native bridge modules.
*/

#![forbid(unsafe_code)]

pub(crate) use optimization_core::codec_bytes;
pub(crate) use optimization_core::digest_encoder;

pub mod code_layout;
mod exit_contract;
mod fragment_emission;
mod frame_application;
pub mod frame_layout;
mod frame_protocol;
mod function_realization;
mod native_bridge_encoding;
pub mod native_entry_bridge;
pub mod native_foreign_bridge;
mod text_placement;

pub use exit_contract::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
    WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};
pub use fragment_emission::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionManifestDecodeError,
    FunctionFragmentEmissionStage, FunctionFragmentEmissionStatistics,
    FunctionFragmentEmissionUnavailableData, StagedOptimizedFunctionFragmentEmissionSource,
    stage_optimized_function_fragment_emission,
};
pub use frame_application::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationError, FunctionFragmentFrameApplicationIdentity,
    StagedFunctionFragmentFrameApplication, function_fragment_frame_application_identity,
    stage_function_fragment_frame_application,
};
pub use frame_protocol::{
    FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
    TargetFrameProtocolEncodingIdentity, TargetFrameProtocolEncodingPlan,
    TargetFrameProtocolEncodingPolicy, target_frame_protocol_encoding_identity,
};
pub use function_realization::{
    FunctionRelativeFrame, FunctionRelativeOptimizationRealizationError,
    StagedFixedFrameFunctionRelativeRealization, assemble_fixed_frame_realization,
    stage_function_frame,
};
pub use text_placement::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError,
    StagedOptimizedFixedFrameTextSection, stage_optimized_fixed_frame_text_section,
    validate_optimized_fixed_frame_text_section,
};

pub(crate) fn count_where<T>(rows: &[T], predicate: impl Fn(&T) -> bool) -> usize {
    rows.iter().filter(|row| predicate(row)).count()
}

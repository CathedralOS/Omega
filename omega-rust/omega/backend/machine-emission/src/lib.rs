#![forbid(unsafe_code)]

//! Common physical-graph realization, frame application, and fragment emission.
//!
//! Function realization retains checked allocation, encoding, frame, and layout inputs.
//! Fragment emission, frame application, and text placement independently replay them.

mod function_realization;
pub use function_realization::*;
mod fragment_emission;
pub use fragment_emission::*;
mod text_placement;
pub use text_placement::custody::*;
pub use text_placement::{
    TextPlacementError, TextPlacementInput, place_fragment_text_section, text_section_statistics,
    validate_fragment_text_section,
};

mod exit_contract;
pub use exit_contract::*;
mod frame_application;
pub mod frame_layout;
mod frame_protocol;
pub use frame_application::{
    FrameApplicationError, apply_frame_protocol_to_fragments, validate_frame_protocol_application,
};
pub use frame_protocol::*;
mod fragments;
pub use fragments::{
    FunctionFragmentStatisticsOverflow, ResolvedFragmentEmissionError,
    emit_resolved_function_fragments, function_fragment_emission_statistics,
    validate_resolved_function_fragments,
};

mod x86_fma;
pub use x86_fma::{EmittedX86ScalarFmaFragment, emit_feature_required_x86_scalar_fma};

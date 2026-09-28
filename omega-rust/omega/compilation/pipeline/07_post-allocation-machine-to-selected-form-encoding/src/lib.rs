#![forbid(unsafe_code)]

//! Post-allocation machine plan to layout-independent selected-form encoding.
//!
//! The stage operation is
//! `stage_optimized_layout_independent_selected_form_encoding`
//! (`selected_form_encoding.rs`), and
//! `validate_optimized_layout_independent_selected_form_encoding` replays it.
//! The stage serializes selected instructions before any address-dependent
//! layout and keeps exact selected and physical roots. Beneath it,
//! `row_encoding` produces each row's bytes, `frame_address` resolves symbolic
//! frame addresses against the retained frame geometry, and `validation` admits
//! the produced bytes independently of the producer.

mod frame_address;
mod row_encoding;
mod selected_form_encoding;
mod validation;

pub use selected_form_encoding::{
    OptimizedSelectedFormEncodingError, StagedOptimizedSelectedFormEncoding,
    stage_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding,
};

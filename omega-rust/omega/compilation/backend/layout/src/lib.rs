//! Native target layout over source-free records: pointer geometry,
//! boundary-value shapes and Terminal callback storage.
//!
//! `boundary_values` constructs bounded native signature graphs and selected
//! opaque carrier occurrences from Psi's target-neutral value recipes.
//! `terminal_callback_layouts` and `terminal_layout_identity` close callback
//! storage and nominal layout identity from canonical Terminal records.
//! `sizing` owns the target's pointer and descriptor words. Layout never
//! authorizes an access; it only fixes where a value's bytes live. This crate
//! reads no source trees: source `size_of`/`align_of` measurement walks typed
//! declarations in provider planning (`data_layout_queries`), which supplies
//! the selected target to Psi's evaluation.

mod boundary_values;
mod terminal_layout_identity;
pub use terminal_layout_identity::{TerminalLayoutIdentity, terminal_layout_identity};
mod terminal_record_storage;
pub use terminal_record_storage::TerminalRecordStorage;
mod terminal_callback_layouts;
pub use terminal_callback_layouts::{
    CallbackLayoutCoordinates, CallbackStorageRoot, MaterializedCallbackLayout,
    TerminalCallbackLayouts, callback_storage_has_slots,
};
mod sizing;
mod target_context;

pub use sizing::{TypeLayout, fat_descriptor_layout, pointer_layout};

pub use target_context::LayoutTarget;

pub use boundary_values::{
    BOUNDARY_VALUE_FIELD_CAPACITY, BOUNDARY_VALUE_SHAPE_CAPACITY, BoundaryOpaqueOccurrence,
    BoundaryValueLayout, TerminalBoundaryRecipes, append_boundary_value_shape,
};

//! Manifest and custody construction for fixed-frame realization.
//!
//! `fixed_frame` builds the expected manifest and custody receipt from replayed
//! evidence, `layout_roots` checks the layout optimization against its source
//! selections and baseline layout, and `statistics` counts the final layout.

mod fixed_frame;
mod layout_roots;
mod statistics;

pub(super) use fixed_frame::{expected_fixed_frame_manifest, fixed_frame_custody};

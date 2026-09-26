//! Reading the checked program and the values running through it.
//!
//! These modules answer questions about what the program SAYS and what a
//! value currently IS, without advancing execution: `program_lookup` finds a
//! declaration, `type_metadata` describes a type, `record_views` and
//! `value_projections` read into a value, `array_windows` bounds a slice of
//! one, and `names_recasts_and_places` resolves an authored name to the place
//! it denotes. Execution itself lives beside this module, not beneath it.

pub(in crate::interpreter::evaluator) mod array_windows;
pub(in crate::interpreter::evaluator) mod names_recasts_and_places;
pub(in crate::interpreter::evaluator) mod program_lookup;
pub(in crate::interpreter::evaluator) mod record_views;
pub(in crate::interpreter::evaluator) mod type_metadata;
pub(in crate::interpreter::evaluator) mod value_projections;

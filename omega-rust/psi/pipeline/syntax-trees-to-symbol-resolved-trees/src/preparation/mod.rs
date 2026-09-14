//! Syntax to syntax, before any symbol exists.
//!
//! `module_normalization` validates the forest against the constant selector,
//! `generic_data` closes eligible generic data applications, and
//! `trait_defaults` materializes default trait machines as ordinary attached
//! machines. All three consume syntax and return syntax; their templates,
//! substitutions, and evaluation scratch are not program representations.

pub(crate) mod generic_data;
pub(crate) mod module_normalization;
pub(crate) mod trait_defaults;

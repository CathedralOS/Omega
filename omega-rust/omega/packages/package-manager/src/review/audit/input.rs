//! Deterministic source-custody and evidence joins for optional review tools.

mod assembly;
mod error;
#[allow(clippy::module_inception)] // This group owns both input assembly and the input record.
mod input;


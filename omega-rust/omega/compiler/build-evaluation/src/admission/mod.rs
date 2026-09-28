//! Lexical root requests and the aggregate checked build result.
//! Build-value settings belong to package execution; trust and wire admission to their
//! semantic owners; the execution coordinator retains their checked results.
//! `admitted_build_program.rs` is the route that consults them.

pub(crate) mod configuration;

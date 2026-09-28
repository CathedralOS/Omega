//! Build vocabulary, lexical root requests and configuration read back from
//! the evaluated `Build` value. Trust and wire admission belong to their
//! semantic owners; the execution coordinator retains their checked results.
//! `admitted_build_program.rs` is the route that consults them.

pub(crate) mod configuration;
pub(crate) mod selection;
pub(crate) mod vocabulary;

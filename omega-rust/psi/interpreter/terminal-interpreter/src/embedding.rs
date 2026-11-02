//! The embedding surface, by lifecycle: `program` is one owned or borrowed
//! immutable artifact (`TerminalProgram`) reusable across independent
//! instances; `instance` is one guest activation (`TerminalInstance`) whose
//! demanded import slots bind checked host adapters before `invoke` and
//! release on `close`; `adapters` is the typed contract a host implementation
//! declares so a slot can be checked before any effect runs.

pub(crate) mod adapters;
pub(crate) mod instance;
pub(crate) mod program;

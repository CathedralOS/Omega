//! Resolved source trees retain syntax, source locations and name bindings
//! so later stages can select declarations without running lexical lookup again.

#![forbid(unsafe_code)]

mod indexed_source;
mod model;
mod resolved_trees;

pub use indexed_source::{IndexedSource, TreeNodeId};
pub use model::{
    CarrierVisibility, CompiledInterface, DeclarationId, DeclarationName, DomainCandidate, DomainCarrier, InterfaceExport, NameContext, NameError, NameRole,
    NameUse, OperatorDeclaration, ResolutionIndex, ResolvedStage, Symbol,
};
pub use resolved_trees::{SymbolResolvedTree, SymbolResolvedTrees, TreeBindings};

/// Recognize the builtin type spellings shared by name resolution and typing.
pub fn is_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "core\\Level"
            | "core\\Type"
            | "core\\Strict"
            | "core\\Squash"
            | "bool"
            | "addr"
            | "f32"
            | "f64"
            | "Slice"
            | "SyscallResult"
            | "Terminal"
            | "UInt"
            | "Int"
            | "AtomicBool"
            | "AtomicU32"
            | "AtomicU64"
    ) || name
        .strip_prefix(['i', 'u'])
        .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
}

pub use omega_program_contracts::ResolvedSignature;

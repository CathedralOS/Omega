//! L2 binds names in the parsed files and dependency scope admitted for one stage.
//! Type-directed selections retain their candidates and the original source spans for L3.

#![forbid(unsafe_code)]

mod stage;
mod syntax;

pub use omega_resolved::{
    CarrierVisibility, CompiledInterface, DeclarationId, DeclarationName, DomainCandidate, DomainCarrier, IndexedSource, InterfaceExport, NameContext,
    NameError, NameRole, NameUse, OperatorDeclaration, ResolvedStage, Symbol, SymbolResolvedTree, SymbolResolvedTrees, TreeNodeId, is_builtin_type_name,
};
use omega_syntax::SyntaxTree;
use omega_tokens::Token;
pub use stage::{resolve_stage, resolve_stage_for_target, resolve_stage_with_exports};

/// A whole-file parse and its original UTF-8 source and tokens. The package loader
/// supplies a canonical, slash-separated logical path relative to the package root.
pub struct ParsedSource<'a, 'parse> {
    pub logical_path: &'a str,
    pub text: &'a str,
    pub tokens: &'parse [Token],
    pub tree: &'parse SyntaxTree,
}

/// The package identity and logical path are supplied by stage admission.
pub struct StageSource<'a, 'parse> {
    pub package: &'a str,
    pub source: ParsedSource<'a, 'parse>,
}

/// Only direct, requester-local edges admitted for this stage are visible.
#[derive(Clone, Copy)]
pub struct Dependency<'a> {
    pub requester: &'a str,
    pub alias: &'a str,
    pub package: &'a str,
}

/// Names admitted by the selected architecture. The target checker owns instruction semantics.
#[derive(Default)]
pub struct AssemblyVocabulary<'a> {
    pub instructions: &'a [&'a str],
    pub registers: &'a [&'a str],
}

//! L2 binds names in the parsed files and dependency scope admitted for one stage.
//! Type-directed selections retain their candidates and the original source spans for L3.

#![forbid(unsafe_code)]

mod resolved_trees;
mod stage;
mod syntax;
pub use resolved_trees::{SymbolResolvedTree, SymbolResolvedTrees};

use omega_lexer::Token;
use omega_parser::SyntaxTree;

pub use stage::{
    AssemblyVocabulary, CarrierVisibility, CompiledInterface, DeclarationId, DeclarationName, Dependency, DomainCandidate, DomainCarrier, InterfaceExport,
    NameContext, NameError, NameRole, NameUse, OperatorDeclaration, ResolvedStage, StageSource, Symbol, builtin as is_builtin_type_name, resolve_stage,
    resolve_stage_for_target, resolve_stage_with_exports,
};
pub use syntax::{File as IndexedSource, TreeNodeId};

/// A whole-file parse and its original UTF-8 source and tokens. The package loader
/// supplies a canonical, slash-separated logical path relative to the package root.
pub struct ParsedSource<'a, 'parse> {
    pub logical_path: &'a str,
    pub text: &'a str,
    pub tokens: &'parse [Token],
    pub tree: &'parse SyntaxTree,
}

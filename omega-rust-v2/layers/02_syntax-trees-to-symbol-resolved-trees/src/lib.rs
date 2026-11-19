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
    Application, ArgumentAlternative, AssemblyVocabulary, DeclarationId, DefinitionSite, Dependency, EnclosingMachine, InterfaceExport, NameError, NameRole,
    NameUse, OperatorDeclaration, OperatorRole, OperatorUse, ResolvedStage, StageSource, Symbol, resolve_stage, resolve_stage_for_target,
    resolve_stage_with_exports,
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

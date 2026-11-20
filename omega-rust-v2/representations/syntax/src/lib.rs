//! Syntax events and indexed views of their tokens and subtrees.
#![forbid(unsafe_code)]

mod indexed_syntax;
mod node_kind;
mod syntax_tree;
mod tree_event;
pub use indexed_syntax::{IndexedSyntax, Leaf as SyntaxLeaf, MathBinding, Node as SyntaxNode, PauseFlag, RangeBounds, SyntaxOperator, TerminationClause};
pub use node_kind::NodeKind;
pub use syntax_tree::SyntaxTree;
pub use tree_event::TreeEvent;

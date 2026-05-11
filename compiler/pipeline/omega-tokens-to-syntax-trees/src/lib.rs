pub mod source_trees;
pub mod parse_error;
pub mod parser;
pub mod syntax;

pub use source_trees::{
    SourceTrees, parse_source_trees, parse_source_trees_with_id, parse_source_trees_with_source,
};
pub use parse_error::ParseError;
pub use parser::{parse_syntax_tree, parse_syntax_tree_with_id, parse_syntax_tree_with_source};
pub use syntax::{SyntaxKind, SyntaxNode, SyntaxNodeHandle, SyntaxTable, SyntaxToken, SyntaxTree};

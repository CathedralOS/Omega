//! The machinery: the two entry points, the parser that walks the tokens and builds the tree, and the small types it hands to the grammar.

pub mod closed_node;
pub mod entry_points;
pub mod fragment_kind;
pub mod open_node;
pub mod parse_failed;
pub mod syntax_parser;

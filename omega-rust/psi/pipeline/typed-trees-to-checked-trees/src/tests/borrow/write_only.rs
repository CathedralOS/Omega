//! Fixtures shared by the write-only borrow tests.

mod length_metadata_and_slices;
mod record_path_fixed_byte_arrays;
mod whole_replacement_and_fixed_arrays;
mod write_only_subloans;

use crate::CheckingRequest;
use crate::lower_typed_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn rendered_rejection(source: &str) -> String {
    lower_typed_trees(typed(source), &CheckingRequest::settled())
        .expect_err("source should be rejected")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

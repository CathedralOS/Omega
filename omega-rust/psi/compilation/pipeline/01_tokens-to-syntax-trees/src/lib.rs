/*
We parse Omega token streams into unresolved source-shaped syntax arenas
(`SyntaxTrees`, `ParsedSources`, and `ParsedSyntaxPages`) across parallel
worker-owned pages.
*/

/*
Our recursive-descent parser separates page scheduling and root publication
(`parser.rs`), token cursor navigation and arena span builders
(`tokens_and_storage.rs`), top-level declaration dispatch (`declarations.rs`),
machine and trait-default body structure (`bodies_and_transitions.rs`),
statement and inline-assembly grammar (`statements_and_assembly.rs`), contract
and parameter clauses (`contracts_and_parameters.rs`), precedence expressions
(`expressions.rs`), and type references and structural patterns
(`type_syntax_and_patterns.rs`). This stage recognizes syntactic structure
without attaching symbol resolution or type meaning.
*/

/*
Stage 01 (`tokens-to-syntax-trees`) consumes `LexedSources` from
`source-files-to-tokens` (Stage 00) and produces `SyntaxTrees` for
`syntax-trees-to-symbol-resolved-trees` (Stage 02).
*/

#![forbid(unsafe_code)]

mod bodies_and_transitions;
mod contracts_and_parameters;
mod declarations;
mod expressions;
pub mod parser;
mod statements_and_assembly;
mod tokens_and_storage;
mod type_syntax_and_patterns;

pub(crate) use tokens_and_storage::{diagnostics, storage, tokens};

pub(crate) mod grammar {
    pub(crate) use crate::bodies_and_transitions::bodies;
    pub(crate) use crate::contracts_and_parameters::{contracts, parameters};
    pub(crate) use crate::declarations;
    pub(crate) use crate::expressions;
    pub(crate) use crate::type_syntax_and_patterns::{patterns, type_syntax};
}

pub use diagnostics::parse_error::ParseError;
pub use parser::{
    ParsedSyntaxPages, ParsedSyntaxSource, parse, parse_source_pages_from, parse_source_units,
    parse_sources, parse_syntax_trees, parse_syntax_trees_with_id,
};

#[cfg(test)]
mod tests;

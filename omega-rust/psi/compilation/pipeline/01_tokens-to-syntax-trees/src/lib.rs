#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.
//!
//! Start at `parser.rs`: [`parse_sources`] appends an ordered source batch;
//! [`parse`] owns the per-source grammar route. It walks tokens on a
//! `tokens::token_cursor::Input` and hands each top-level item to
//! `grammar::declarations::parse_declaration`, publishing the roots into the
//! caller's `SyntaxTrees`. [`parse_syntax_trees`] and
//! [`parse_syntax_trees_with_id`] are the same route with the arena created
//! for you.
//!
//! - `grammar` holds the recursive-descent rules, one module per construct:
//!   `declarations` dispatches one top-level item to the form that owns it,
//!   `bodies` reads a machine or trait-default body, `contracts` the contract
//!   clauses, `parameters` parameter lists and return parameters,
//!   `expressions` the precedence grammar, `type_syntax` one type reference,
//!   and `patterns` the field patterns of `match` and `transition`.
//! - `tokens` owns the cursor every grammar rule reads through.
//! - `storage` appends parsed nodes into the `SyntaxTrees` arenas.
//! - `diagnostics` owns the failure surface every rule reports to.
//!
//! Meaning belongs to the resolution and typing stages that follow; this stage
//! only recognizes shape.

// The entrance.
pub mod parser;

// The grammar, and what it reads through, appends into and reports to.
mod diagnostics;
mod grammar;
mod storage;
mod tokens;

// The entrance, and the two forms that create the arena for the caller.
pub use parser::{
    ParsedSyntaxPages, ParsedSyntaxSource, parse, parse_source_pages_from, parse_source_units,
    parse_sources, parse_syntax_trees, parse_syntax_trees_with_id,
};

// Per-source grammar failure; batch parsing adds source locations as diagnostics.
pub use diagnostics::parse_error::ParseError;

#[cfg(test)]
mod tests;

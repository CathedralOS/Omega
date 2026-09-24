#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.
//!
//! One entrance: [`parse`], in `parser.rs`. It walks one source's tokens on an
//! `input::token_cursor::Input` and hands each top-level item to
//! `declarations::parse_declaration`, publishing the roots into the caller's
//! `SyntaxTrees`. [`parse_syntax_trees`] and [`parse_syntax_trees_with_id`]
//! are the same route with the arena created for you, and
//! [`parse_syntax_trees_into_with_id`] is `parse` under its long name.
//!
//! The grammar modules below are not stages and run in no fixed order: this is
//! recursive descent, so each one calls the ones nested inside it, for as long
//! as the source nests. They are listed by that containment, outermost first,
//! and the list is the real reach direction — `declarations` calls all five
//! that follow it and `bodies` calls the four after it. Two pairs close back
//! on each other where the grammar itself does: `contracts` reads a return
//! type through `parameters` while `parameters` reads signature clauses and
//! conformance arguments through `contracts`, and `expressions` and
//! `type_syntax` each bottom out in the other.
//!
//! - `declarations` dispatches one top-level item to the form that owns it.
//! - `bodies` reads a machine or trait-default body.
//! - `contracts` reads the contract clauses on machines, signatures and states.
//! - `parameters` reads parameter lists and return parameters.
//! - `expressions` reads the precedence grammar.
//! - `type_syntax` reads one type reference.
//!
//! Beneath all of them, `input` owns the token cursor every module reads
//! through and `diagnostics` owns the failure surface every module reports to.
//!
//! Meaning belongs to the resolution and typing stages that follow; this stage
//! only recognizes shape.

// The entrance.
pub mod parser;

// The grammar, outermost form first; each reaches down this list.
mod bodies;
mod contracts;
mod declarations;
mod expressions;
mod parameters;
mod type_syntax;

// Beneath the grammar: what every module above reads through and reports to.
mod diagnostics;
mod input;

// The entrance, under the four names its callers use.
pub use parser::{
    parse, parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};

// The failure every one of those names can return.
pub use diagnostics::parse_error::ParseError;

#[cfg(test)]
mod tests;

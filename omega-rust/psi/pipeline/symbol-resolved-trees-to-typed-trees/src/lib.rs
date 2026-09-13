#![forbid(unsafe_code)]

//! Attaches type and signature meaning to Psi symbol-resolved source trees.
//!
//! [`lower_symbol_resolved_trees`] owns complete typing: validate resolved meaning,
//! lower declarations in dependency order, retain initializer custody, then settle
//! and normalize typed trees. The `lowerer` module shows that ordinary route.
//! Its `seeded_continuation` child owns append-only admission and transactional
//! recovery; both routes use the same declaration lowering operations.

mod call_results;
mod data;
mod domain;
mod domain_constraints;
mod equality;
mod equatable;
mod exhaustiveness;
mod expression;
mod fixed_byte_array_literals;
mod lowerer;
mod machine;
mod measure;
mod name;
mod operator;
mod progress;
mod proposition;
mod qualification_casts;
mod state;
mod statement;
mod trait_definition;
mod type_reference;
mod wire;

pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base, retained_typed_base_is_exact_prefix,
};
pub use lowerer::{lower_symbol_resolved_trees, lower_symbol_resolved_trees_owned};

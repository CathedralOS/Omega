#![forbid(unsafe_code)]

//! Attaches type and signature meaning to Psi symbol-resolved source trees.

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

pub use lowerer::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension, lower_symbol_resolved_trees,
    lower_symbol_resolved_trees_owned, lower_symbol_resolved_trees_to_seeded_base,
    retained_typed_base_is_exact_prefix,
};

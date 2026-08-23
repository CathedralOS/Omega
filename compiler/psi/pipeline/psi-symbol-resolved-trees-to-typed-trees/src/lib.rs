#![forbid(unsafe_code)]

//! Attaches type and signature meaning to Psi symbol-resolved source trees.

mod data;
mod domain;
mod domain_constraints;
mod equality;
mod equatable;
mod exhaustiveness;
mod expression;
mod invariant;
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

pub use lowerer::{lower_symbol_resolved_trees, lower_symbol_resolved_trees_owned};

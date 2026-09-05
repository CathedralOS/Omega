//! The checked-program root and its concept owners.
//!
//! `CheckedTrees` pairs the typed program with `CheckFacts`. The facts retain
//! borrowing, control flow, proof, value, and operator judgments; they are not
//! a history of the validation passes that established them.

pub use typed_trees::byte_predicates;
pub use typed_trees::typed_trees::ClosedConformanceConstArgument;
pub use typed_trees::{
    data, domain, expression, identity, machine, name, proof_only, proposition, signature, state,
    trait_definition, types, wire,
};

pub mod admissibility;
pub mod borrow;
pub mod facts;
pub mod flow;
pub mod operators;
pub mod proof;
pub mod service_parameter;
pub mod statement;
pub mod values;

pub use admissibility::*;
pub use borrow::*;
pub use facts::*;
pub use flow::*;
pub use operators::*;
pub use proof::*;
pub use service_parameter::*;
pub use values::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: typed_trees::TypedTrees, facts: CheckFacts) -> Self {
        Self { typed, facts }
    }
}

impl std::ops::Deref for CheckedTrees {
    type Target = typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &typed_trees::TypedTrees {
        &self.typed
    }
}

#[cfg(test)]
mod tests;

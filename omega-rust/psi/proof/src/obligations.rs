//!
//! `plan.rs` is the plan and its carriers, `collection.rs` builds one from a
//! typed program, `constraints.rs` derives what an expression proves,
//! `identity.rs` renders
//! an obligation's canonical semantic identity and `ranges.rs` turns
//! constraints back into ranges. Shared typed-expression queries live beside
//! this owner in `crate::typed_expressions`, also used by the checker.

mod collection;
mod constraints;
mod identity;
mod plan;
#[cfg(test)]
mod range_tests;
mod ranges;

pub use collection::build_proof_plan;
pub(crate) use collection::declared_integer_range;
pub(crate) use constraints::expression_constraints;
pub use identity::{KEY_SCHEMA, ProofObligationKey, proof_obligation_key};
pub(crate) use plan::FloatRange;
pub use plan::{
    BinaryValueOperands, BoundedAssignmentObligation, BoundedCallArgumentObligation,
    BoundedInitializerObligation, BoundedStateReturnObligation,
    BoundedTransitionArgumentObligation, BoundedValueObligation, GuardedTransitionObligation,
    IntegerRange, ProofConstraint, ProofObligation, ProofObligationOwner, ProofPlan,
};
pub(crate) use ranges::integer_binary_range;

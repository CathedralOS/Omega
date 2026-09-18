//!
//! `plan.rs` is the plan and its carriers, `collection.rs` builds one from a
//! program, `program_queries.rs` answers what the program declares,
//! `constraints.rs` derives what an expression proves and `ranges.rs` turns
//! constraints back into ranges.

mod collection;
mod constraints;
mod plan;
mod program_queries;
#[cfg(test)]
mod range_tests;
mod ranges;

pub use collection::build_proof_plan;
pub(crate) use collection::declared_integer_range;
pub(crate) use constraints::expression_constraints;
pub(crate) use plan::FloatRange;
pub use plan::{
    BinaryValueOperands, BoundedAssignmentObligation, BoundedCallArgumentObligation,
    BoundedInitializerObligation, BoundedStateReturnObligation,
    BoundedTransitionArgumentObligation, BoundedValueObligation, GuardedTransitionObligation,
    IntegerRange, ProofConstraint, ProofObligation, ProofObligationOwner, ProofPlan,
};
pub(crate) use program_queries::{
    data_field_type_by_name, dehoisted_condition, dehoisted_operand, expression_type_reference,
};
pub(crate) use ranges::integer_binary_range;

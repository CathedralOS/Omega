//! The proposition vocabulary: integer types and values, scalar terms,
//! proof terms, propositions and their context, and the value identities
//! they name. `integer_types.rs` holds the integer vocabulary,
//! `scalar_terms.rs` the scalar terms, `proof_terms.rs` the erased
//! proof-only terms, `propositions.rs` the propositions and their context,
//! and `value_ids.rs` the value identities they reference.

mod integer_types;
mod proof_terms;
mod propositions;
mod scalar_terms;
#[cfg(test)]
mod tests;
mod value_ids;

pub use integer_types::{
    IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
};
pub use proof_terms::{ProofTerm, ProofTermField};
pub use propositions::{Proposition, PropositionContext, PropositionError};
pub use scalar_terms::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, ScalarTerm, ScalarType,
    StructuralCaseSubject,
};

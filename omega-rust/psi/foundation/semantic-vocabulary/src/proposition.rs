//!
//! `integer_types.rs` holds the integer vocabulary, `scalar_terms.rs` the
//! terms, `propositions.rs` the propositions and their context, and
//! `value_ids.rs` the value identities they reference.

mod integer_types;
mod propositions;
mod scalar_terms;
#[cfg(test)]
mod tests;
mod value_ids;

pub use integer_types::{
    IntegerCarrier, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
};
pub use propositions::{Proposition, PropositionContext, PropositionError};
pub use scalar_terms::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, ScalarTerm, ScalarType,
    StructuralCaseSubject,
};

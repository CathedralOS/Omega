//! Sealed automatic primitive choice and shared exact execution classification.
//!
//! `selection` privately issues an automatic choice from original loaded
//! canonical source units, exact requirement/realization handles, signature,
//! satisfaction and selected-target custody. Explicit Build/default selections
//! stay with the ordinary selector. The classifier is shared with explicitly
//! selected rows; classification alone does not issue choice authority.
//! Calls retain their public requirement, and later consumers use the issued
//! execution without replaying catalog source admission.

mod execution_identities;
mod selection;
pub use selection::CompilerCatalogChoice;
pub(crate) use selection::retain_catalog_choices;
mod named_float_realizations;

pub use execution_identities::{
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution,
};

use crate::CompilerIntrinsicExecutionIdentity;
use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::RoundingDirection;
use numerics::literals::FloatFormat;
use symbols::BuiltinFunction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedFloatRealization {
    Builtin(BuiltinFunction),
    Negate(FloatFormat),
    MultiplyThenAdd(FloatFormat),
    FusedMultiplyAdd(FloatFormat),
    DirectedFusedMultiplyAdd(FloatFormat, RoundingDirection),
    DirectedSquareRoot(FloatFormat, RoundingDirection),
    DirectedBinary(DirectedFloatBinaryOperation, FloatFormat, RoundingDirection),
    Convert(ArithmeticDomain),
}

/// Closed compiler-owned execution child selected for one exact
/// `CompilerIntrinsic` boundary-operator row.
///
/// Primitive-expression realizations deliberately remain distinct from
/// builtin functions. Only explicitly closed children enter `Closed`; every
/// other compiler path remains `Unsupported` and package review rejects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCompilerIntrinsicExecutionIdentity {
    Closed(CompilerIntrinsicExecutionIdentity),
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedCompilerIntrinsicRealization {
    /// The closed primitive child the requirement's fixed token spells, from
    /// `IntrinsicRequirement::closed_execution`.
    ClosedPrimitive(CompilerIntrinsicExecutionIdentity),
    NamedFloat(NamedFloatRealization),
    OtherCompilerPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectedFloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

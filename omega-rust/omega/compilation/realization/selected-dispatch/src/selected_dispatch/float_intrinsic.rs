//! Exact compiler-intrinsic execution identities for selected requirements.
//!
//! These identities serve build-time folding and Terminal coverage. Calls
//! retain their public requirement; this owner neither rewrites them nor
//! revisits failed Psi plans to relabel their omissions.

mod execution_identities;
mod named_float_realizations;

pub use execution_identities::{
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution,
};

use numerics::arithmetic::ArithmeticDomain;
use numerics::float_semantics::RoundingDirection;
use numerics::literals::FloatFormat;
use provider_planning::CompilerIntrinsicExecutionIdentity;
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
    PrimitiveFloatBinary(CompilerIntrinsicExecutionIdentity),
    PrimitiveIntegerComparison(CompilerIntrinsicExecutionIdentity),
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

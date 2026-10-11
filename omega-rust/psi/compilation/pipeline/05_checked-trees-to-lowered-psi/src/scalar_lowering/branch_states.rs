//! Prepared scalar branch states: the ordered bindings, structural effects
//! and exit each state of a scalar-completion body evaluates, which the Unit
//! scalar-completion emitter (`unit::attached_unit::argument_evaluation`)
//! lowers.
//!
//! `prepared.rs` holds the state, terminator and effect types; `guards.rs`
//! lowers and evaluates a state's guarded exits once before dispatch;
//! `call_lowering.rs` lowers a direct scalar call binding against its
//! callee's Unit body; `unit_operations.rs` prepares a checked callable retained
//! by scalar evaluation, preserving body versus boundary dispatch;
//! `structural_values.rs` places structural
//! construction in the state's evaluation order. Shared primitive types and
//! lowered-expression checks belong to emission.

mod call_lowering;
pub(crate) mod guards;
pub(crate) mod prepared;
pub(crate) mod structural_values;
pub(crate) mod unit_operations;

use crate::emission::expression_validation::validate_boolean_parameter_types;
use crate::emission::scalar_types::terminal_scalar_type;
pub(crate) use call_lowering::lower_scalar_call;

use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, Multiplicity, PlaceId,
    QualifiedScalarType, ScalarType, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, scalar_carriers, unsupported,
};
use crate::expression_preparation::qualifications::PreparedScalarQualifications;

use crate::expression_preparation::bindings as storage;
use crate::scalar_lowering::computations;

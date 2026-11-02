//! Scalar lowering every Unit body shares.
//!
//! Ordinary, composed and cleanup Unit bodies are the one lowering route for
//! scalar machines; this family lowers the scalar pieces they embed:
//! - `computations` expands checked scalar computations (fields, cases,
//!   calls, arrays, subslices, byte comparisons) into private typed blocks;
//! - `contracts` lowers requirements, guarantees and proof terms over entry
//!   parameters and the normal-return result;
//! - `call_closure` names the Unit body a scalar call lowers
//!   (`call_closure::callee::CheckedScalarCallee`) and discovers the scalar
//!   helpers embedded beneath a source-owned root;
//! - `branch_states` prepares the per-state bindings, effects and exits the
//!   Unit scalar-completion emitter lowers, and `effects` emits those effects.
//!
//! Shared expression preparation and emission are consumed from their owners.

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedTerminalSignatureEligibility, CheckedTrees, CheckedUnitStructuralParameterPlan,
    ClosedScalarContractValue,
};
use language_semantics::Multiplicity;
use semantic_vocabulary::{
    IntegerValue, MachineId, PlaceId, Proposition, QualifiedScalarType, ScalarTerm, ScalarType,
    StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, ValueDeclaration,
};

use crate::emission::expression_validation::validate_direct_parameter_types;
use crate::emission::operation_emission::emit_direct_expression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, place_id};

pub(crate) mod branch_states;
pub(crate) mod call_closure;
pub(crate) mod computations;
pub(crate) mod contracts;
pub(crate) mod effects;

pub(crate) fn scalar_carriers(types: &[QualifiedScalarType]) -> Vec<ScalarType> {
    types.iter().map(|value| value.scalar_type).collect()
}

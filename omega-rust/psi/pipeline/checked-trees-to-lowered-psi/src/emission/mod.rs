//! Operation and store emission shared by Unit and scalar bodies.
//!
//! Owns the ordered scalar-binding emitter, primitive and structural store
//! emission, byte-sequence writes, and the call-operand source custody those
//! emitters replay.

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
use semantic_vocabulary::{
    PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, ValueDeclaration,
};

use crate::emission::operation_emission::emit_direct_expression;
use crate::machine_lowering::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::terminal_identities::{
    allocate_dense, obligation_id, place_id, structural_type_id, value_id,
};
use crate::scalar_graph::scalar_graph_lowering::{
    direct_expression_contains_short_circuit, lower_checked_scalar_expression,
    terminal_scalar_type, validate_direct_parameter_types,
};

pub(crate) mod byte_sequence_write;
pub(crate) mod call_source_custody;
pub(crate) mod operation_emission;
pub(crate) mod primitive_store;
pub(crate) mod structural_byte_sequence_index_store;
pub(crate) mod structural_byte_sequence_store;
pub(crate) mod structural_scalar_store;
pub(crate) mod structural_scalar_store_source;

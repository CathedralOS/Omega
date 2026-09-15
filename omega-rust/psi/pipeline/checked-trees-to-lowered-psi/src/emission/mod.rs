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

use crate::emission::expression_validation::{
    direct_expression_contains_short_circuit, validate_direct_parameter_types,
};
use crate::emission::operation_emission::emit_direct_expression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use crate::scalar_graph::scalar_graph_lowering::lower_checked_scalar_expression;
use crate::terminal_identities::{
    allocate_dense, obligation_id, place_id, structural_type_id, value_id,
};

pub(crate) mod byte_sequence_write;
pub(crate) mod call_source_custody;
pub(crate) mod operation_emission;
pub(crate) mod primitive_store;
pub(crate) mod structural_byte_sequence_index_store;
pub(crate) mod structural_byte_sequence_store;
pub(crate) mod structural_scalar_store;
pub(crate) mod structural_scalar_store_source;

pub(crate) mod expression_validation;
pub(crate) mod scalar_types;
pub(crate) mod store_destination;

pub(crate) mod boolean_control;
pub(crate) mod selected_comparison;

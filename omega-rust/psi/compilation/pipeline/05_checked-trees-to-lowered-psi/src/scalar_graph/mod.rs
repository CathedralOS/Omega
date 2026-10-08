//! Scalar-graph machine lowering.
//!
//! Owns scalar-graph preparation and partial evaluation, the reachable scalar
//! call closure, terminal module assembly, computation expansion and contracts.
//! Shared expression preparation and emission are consumed from their owners.

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarBranchDestination,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTerminalSignatureEligibility,
    CheckedTrees, CheckedUnitStructuralParameterPlan, ClosedScalarContractValue,
    ClosedScalarValueContractPlan,
};
use language_semantics::Multiplicity;
use semantic_vocabulary::{
    IntegerValue, MachineId, PlaceId, Proposition, QualifiedScalarType, ScalarTerm, ScalarType,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, ValueDeclaration,
};

use crate::emission::expression_validation::validate_direct_parameter_types;
use crate::emission::operation_emission::emit_direct_expression;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::crash_routes::lower_checked_crash_route_buckets;
use crate::terminal_identities::{allocate_dense, lookup_machine_id, place_id};

pub(crate) mod scalar_call_closure;
pub(crate) mod scalar_computations;
pub(crate) mod scalar_contracts;
pub(crate) mod scalar_graph_effects;
pub(crate) mod scalar_graph_lowering;

pub(crate) fn scalar_carriers(types: &[QualifiedScalarType]) -> Vec<ScalarType> {
    types.iter().map(|value| value.scalar_type).collect()
}

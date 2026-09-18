//! Scalar-graph machine lowering.
//!
//! Owns scalar-graph preparation and partial evaluation, the reachable scalar
//! call closure, terminal module assembly, computation expansion and contracts.
//! Shared expression preparation and emission are consumed from their owners.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryScalarReturnMachinePlan, CheckedIntegerComparisonKind,
    CheckedScalarBindingValue, CheckedScalarBranchDestination, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedScalarMachineGraph, CheckedScalarStateTerminator,
    CheckedScalarSuccessor, CheckedStructuralScalarReturnMachinePlan,
    CheckedTerminalSignatureEligibility, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, ClosedScalarContractValue, ClosedScalarValueContractPlan,
};
use language_semantics::{Multiplicity, PermissionClaimIdentity};
use lowered_psi::LoweredPsi;
use proof_admission::{EvidenceRoute, PrimitiveJudgment};
use semantic_vocabulary::{
    BlockId, ClaimId, IeeeFloatFormat, IntegerType, IntegerValue, MachineId, PlaceId, Proposition,
    QualifiedScalarType, ScalarTerm, ScalarType, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId,
};
use terminal_psi::{
    Block, ContentPartitionComposition, ContractClause, MachineContract, Operation, OperationKind,
    OperationResult, ScalarFloatRange, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ObligationEvidence, ProofBundle};

use crate::emission::boolean_control::{
    boolean_decision_block_count, boolean_decision_test_count, build_scalar_conditional_target,
    emit_inlined_boolean_guard_blocks, emit_inlined_boolean_value_blocks,
    emit_reserved_boolean_tuple_stage_blocks, lower_boolean_control_decision,
    lower_boolean_value_decision, scalar_source_block,
};
use crate::emission::expression_validation::{
    contains_short_circuit, direct_expression_contains_short_circuit,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::{
    emit_boolean_expression, emit_direct_expression, emit_scalar_binding,
    emit_staged_scalar_call_binding,
};
use crate::emission::scalar_types::terminal_scalar_type;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::content_conservation::{
    LoweredContentIdentityReshuffles, LoweredContentPartitionCompositions,
    lower_content_identity_reshuffles, lower_content_partition_compositions,
    merge_content_place_declaration,
};
use crate::proofs::crash_routes::{
    lower_checked_crash_exit, lower_checked_crash_predicates, lower_checked_crash_route_buckets,
};
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::scalar_graph::scalar_graph_lowering::{
    KnownDirectScalar, prepare_scalar_graph_machine, staged_short_circuit_bindings_terminator,
};
use crate::scalar_graph::scalar_graph_module::build_scalar_graph_module;
use crate::terminal_identities::{
    TERMINAL_MACHINE_IDENTITY_STRIDE, TERMINAL_UNIT_CALL_OBLIGATION_BASE, allocate_dense, block_id,
    contract_id, dense_identity, edge_id, lookup_machine_id, lookup_type_id, machine_id,
    obligation_id, place_id, value_id,
};

pub(crate) mod scalar_call_closure;
pub(crate) mod scalar_computations;
pub(crate) mod scalar_contracts;
pub(crate) mod scalar_graph_effects;
pub(crate) mod scalar_graph_lowering;
pub(crate) mod scalar_graph_module;
pub(crate) mod shared_runtime_parameters;

pub(crate) fn scalar_carriers(types: &[QualifiedScalarType]) -> Vec<ScalarType> {
    types.iter().map(|value| value.scalar_type).collect()
}

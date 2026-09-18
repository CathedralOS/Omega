//! Return-transfer machine lowering.
//!
//! Owns the producers dispatched for result-bearing plans: owned-affine
//! identity returns, boundary scalar results, payloadless structural cases and
//! guarded calls, general structural results, and structural scalar returns.
//! Structural-type retention shared by the result and control lanes lives here.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBoundaryMachinePlan, CheckedBoundaryScalarReturnMachinePlan,
    CheckedNominalAffineUnitCleanupMachinePlan, CheckedScalarBindingValue, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedSelectedOperatorStructuralScalarReturnMachinePlan,
    CheckedStructuralReturnMachinePlan, CheckedStructuralScalarIntegerBoundKind,
    CheckedStructuralScalarIntegerBoundPlan, CheckedStructuralScalarReturnCleanupAction,
    CheckedStructuralScalarReturnMachinePlan, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, ClosedScalarContractValue,
};
use language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachSummary,
};
use lowered_psi::{LoweredPsi, LoweredSelectedIeeeFloatFmaOccurrence, LoweredSourceCallOccurrence};
use semantic_vocabulary::{
    BoundaryMachineId, ContractId, DomainSemanticId, IeeeFloatFormat, IntegerSign, MachineId,
    PlaceId, Proposition, ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, ByteSequenceCarrier,
    CompletionReceipt, EntryClaim, MachineContract, Operation, OperationKind, ServiceDeclaration,
    StructuralAccess, StructuralArgument, StructuralCaseDeclaration, StructuralDomainDeclaration,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::ProofBundle;

use crate::emission::boolean_control::{
    bind_boolean_decision, boolean_decision_block_count, emit_inlined_boolean_value_blocks,
    lower_boolean_value_decision,
};
use crate::emission::expression_validation::{
    contains_short_circuit, validate_boolean_parameter_types, validate_direct_parameter_types,
};
use crate::emission::operation_emission::emit_direct_expression;
use crate::emission::scalar_types::{integer_value, terminal_scalar_type};
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression_at;
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::content_conservation;
use crate::proofs::content_conservation::{
    RESULT_STRUCTURAL_PLACE_ID, lower_boundary_content_guarantees,
    lower_content_identity_reshuffles,
};
use crate::proofs::crash_routes::{lower_boundary_crash_routes, lower_checked_crash_route_buckets};
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::returns::payloadless_case_return::lower_payloadless_case_return_machine;
use crate::returns::structural_types::{
    lower_mixed_cases, lower_mixed_fields, lower_structural_type_plans,
    retain_additional_structural_types, terminal_byte_sequence_carrier,
};
use crate::scalar_graph::shared_runtime_parameters::{
    normalize_shared_boolean_comparison_leaves, resolve_shared_boolean_member_fields,
    shared_boolean_runtime_parameters, valid_shared_boolean_runtime_inputs,
};
use crate::terminal_identities::{
    TERMINAL_MACHINE_IDENTITY_STRIDE, allocate_dense, block_id, boundary_machine_id, claim_id,
    contract_id, dense_identity, edge_id, lookup_claim_id, lookup_domain_id, lookup_machine_id,
    lookup_service_id, lookup_type_id, machine_id, obligation_id, operation_id, place_id,
    service_id, structural_domain_id, structural_field_id, structural_type_id, value_id,
};
use crate::unit::attached_unit::{
    checked_unit_boundary_identity, checked_unit_target_reach_matches,
    collect_installation_machine_contract_services, collect_published_contract_services,
    collect_service_summary, lower_installation_machine_service_ceiling,
    lower_published_service_ceiling, lower_root_service_reach, lower_structural_arguments,
    lower_unit_parameters, validate_transfer_shape,
};
use crate::unit::unit_cleanup::lower_nominal_affine_unit_cleanup_machine;

pub(crate) mod affine_return;
pub(crate) mod boundary_scalar_return;
pub(crate) mod payloadless_case_return;
pub(crate) mod payloadless_guarded_call_return;
pub(crate) mod structural_return;
pub(crate) mod structural_scalar_return;
pub(crate) mod structural_types;

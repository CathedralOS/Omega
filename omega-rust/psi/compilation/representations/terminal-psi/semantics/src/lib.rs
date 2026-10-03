#![forbid(unsafe_code)]

//! Closed, target-neutral semantic tables for terminal Psi.
//!
//! This crate owns declarative operation-row identity, local denotations, and
//! pure replay of scalar ordering and descriptor lifetimes over supplied graphs.
//! Representation owners reconstruct those graphs and authenticate their inputs;
//! the shared solvers do not decide evidence availability, proof reduction,
//! provider realization, or authority from source spellings.
//!
//! `semantic_rows.rs` is the root: the row every terminal
//! operation kind resolves to. `scalar_leaf_schema.rs` describes a goal-free
//! scalar leaf and `scalar_leaf_semantics.rs` interprets one;
//! `scalar_representation_view.rs` owns the closed carrier pairs and bit
//! meaning of a scalar re-view. The other modules cover call composition,
//! proof-bearing scalars, record fields, primitive places, scalar arrays and
//! structural effects.

mod arrival_storage;
pub mod borrowed_storage;
mod byte_carriers;
mod byte_sequence_concatenation;
mod byte_sequence_predicates;
mod call_composition;
mod collection_backing;
pub mod descriptor_results;
mod placed_view_referent;
mod primitive_place;
mod proof_bearing_scalar;
mod record_field;
mod reference_views;
mod scalar_array;
mod scalar_leaf_schema;
mod scalar_leaf_semantics;
pub mod scalar_ordering;
mod scalar_representation_view;
mod semantic_rows;
mod service_binding;
mod shared_mutation;
mod static_path;
mod storage_regions;
mod structural_effect;
mod structural_window;
#[cfg(test)]
mod tests;
mod zeroed_structural;

pub use arrival_storage::{StorageNames, arrival_parameter, root_before_edge, storage_names};
pub use byte_carriers::{ByteCarrierLayout, ByteCarrierScalarLeaf, byte_carrier_scalar_layout};
pub use byte_sequence_concatenation::{
    byte_sequence_concatenation_capacity_goal, byte_sequence_concatenation_length_equation,
};
pub use byte_sequence_predicates::{
    byte_copied_field_predicates_at_call, byte_field_root, byte_literal_qualifications_hold,
    byte_literal_qualifications_hold_in, byte_owned_call_result, byte_owned_result_predicates,
    byte_parameter_predicates, byte_parameter_predicates_from_domains,
    byte_parameter_predicates_in, byte_predicate_requirement, byte_qualification_predicates_in,
    byte_sequence_concatenation_predicates, byte_sequence_source_predicates, byte_view_call_result,
    indexed_byte_store_goal, shared_byte_field_copy_predicates,
};
pub use call_composition::{
    BorrowedReferentPresentation, CallArgumentRule, CallCompositionSchema,
    CallCompositionSemanticRow, CallCrashRule, CallEvidenceRule, CallFrontierRule, CallFuelPolicy,
    CallOutcomeRule, CallRequirementRule, CallResultRule, CallTargetRule, CallTransferRule,
    FixedByteArrayWindow, borrowed_referent_matches, borrowed_referent_presentation,
    boundary_buffer_capacity, bounded_byte_owner_capacity,
    bounded_byte_owner_capacity_from_catalogs, bounded_byte_owner_capacity_in,
    call_composition_semantic_row, exact_call_composition_semantic_row_in, fixed_byte_array_extent,
    fixed_byte_array_window, fixed_element_array_extent, shared_boundary_buffer_capacity,
    stored_view_field_presentation, validate_call_composition_semantic_rows,
};
pub use collection_backing::{CollectionBacking, CollectionStorage, collection_backing};
pub use placed_view_referent::{PlacedViewReferentError, validate_placed_view_referent};
pub use primitive_place::{
    canonical_primitive_path, canonical_static_projection, fixed_array_place_shape,
    primitive_place_type, primitive_projection_type, primitive_read_projection_type,
    runtime_element_field, runtime_structural_path_tip,
};
pub use proof_bearing_scalar::{
    CanonicalScalarGoal, ProofBearingIntegerPolicyBinding, ProofBearingScalarLeafElision,
    ProofBearingScalarLeafSchema, ProofBearingScalarLeafSemantics, ProofBearingScalarSemanticRow,
    elidable_proof_bearing_scalar_leaf, exact_proof_bearing_scalar_semantic_row_in,
    proof_bearing_integer_policy_binding, proof_bearing_scalar_leaf_semantics,
    proof_bearing_scalar_semantic_row, validate_proof_bearing_scalar_semantic_rows,
};
pub use record_field::{
    CasePayloadCarrier, RecordFieldCarrier, case_payload_carrier, observed_field_backing_access,
    observed_field_declaration, record_field_carrier,
};
pub use reference_views::{
    reference_source_compatible, scalar_reference_view_is_total, scalar_reference_view_value,
};
pub use scalar_array::scalar_array_leaf_shape;
pub use scalar_leaf_schema::{
    GoalFreeScalarLeafSchema, ScalarLeafCrashPolicy, ScalarLeafDenotation, ScalarLeafFactShape,
    ScalarLeafFrontierPolicy, ScalarLeafFuelPolicy, ScalarLeafGoalShape, ScalarLeafOperandShape,
    ScalarLeafResultShape,
};
pub use scalar_leaf_semantics::{
    GoalFreeScalarLeafSemantics, ScalarLeafLiteral, constant_goal_free_scalar_leaf,
    goal_free_scalar_leaf_semantics,
};
pub use scalar_representation_view::{ScalarRepresentationViewPair, ScalarViewValue};
pub use semantic_rows::{
    OperationSemanticCustody, OperationSemanticError, OperationSemanticRow, OperationSemanticTag,
    exact_operation_semantic_row_in, is_unconditionally_total_scalar, operation_semantic_row,
};
pub use service_binding::{fused_service_field_carrier, fused_service_field_matches};
pub use shared_mutation::{SharedMutation, shared_mutation};
pub use static_path::{
    canonical_case_leaf_type, canonical_structural_path_tip, interior_member_borrow,
    runtime_index_bound, runtime_index_selects, structural_paths_may_overlap,
    structural_paths_select_same_location,
};
pub use storage_regions::{
    StorageRegion, canonical_field_storage_region, projected_storage_region,
};
pub use structural_effect::{
    StructuralEffectAction, StructuralEffectCustody, StructuralEffectExternalEffect,
    StructuralEffectFrontierPolicy, StructuralEffectFuelPolicy, StructuralEffectGoalShape,
    StructuralEffectLeafSchema, StructuralEffectObservation, StructuralEffectResultShape,
    StructuralEffectSemanticRow, byte_equal_equation, byte_predicate_equation,
    element_establishment_length_equation, element_subslice_length_equation,
    element_view_leaf_scalar, exact_structural_effect_semantic_row_in, literal_length_equation,
    structural_effect_leaf_observation, structural_effect_leaf_observation_in,
    structural_effect_semantic_row, subslice_length_equation,
    validate_structural_effect_semantic_rows,
};
pub use structural_window::{
    StructuralWindowProjection, structural_window_projection, structural_window_selects_child,
};
pub use zeroed_structural::{
    ZeroedStructuralLeaf, zeroed_structural_leaves, zeroed_structural_shape,
};

pub mod descriptor_lifetimes;

mod representation_regions;
pub use representation_regions::{
    ElementViewRegionGeometry, RepresentationElementLayout, RepresentationScalarLeaf,
    element_view_region_geometry, representation_byte_extent, representation_element_layout,
    representation_region_goal, representation_region_length,
};

mod element_representation;
pub use element_representation::{
    ElementRepresentationLayout, ElementRepresentationLeaf, ElementRepresentationRelation,
    element_representation_layout, element_representation_relation,
};

pub use collection_backing::{collection_view_backing, collection_view_preserves_coordinates};

pub use structural_effect::{ElementViewLeaf, element_view_leaf, element_view_write_goal};

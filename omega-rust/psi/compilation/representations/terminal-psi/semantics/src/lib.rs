#![forbid(unsafe_code)]

//! Closed, target-neutral semantic tables for terminal Psi.
//!
//! This crate owns declarative operation-row identity, local denotations, and
//! pure replay of scalar ordering and descriptor lifetimes over supplied graphs.
//! Representation owners reconstruct those graphs and authenticate their inputs;
//! the shared solvers do not decide evidence availability, proof reduction,
//! provider realization, or authority from source spellings.
//!
//! `semantic_rows.rs` is the root: the row every terminal operation kind
//! resolves to. `scalars/scalar_leaf_schema.rs` describes a goal-free scalar
//! leaf and `scalars/scalar_leaf_semantics.rs` interprets one;
//! `scalars/scalar_representation_view.rs` owns the closed carrier pairs and
//! bit meaning of a scalar re-view. The other modules group by subject: `access/`,
//! `bytes/`, `calls/`, `descriptors/`, `references/`, `scalars/`, `storage/`
//! and `structural/`; `control_graph`, `substitution`, `quotients`,
//! `content_transformations` and `domain_preservation` stand alone.

pub use calls::closed_conformances;
pub use calls::closed_reach;
pub use storage::activation_storage;
pub use storage::borrowed_storage;
pub use storage::calling_storage::{
    ForeignRecordStorage, callback_storage_has_slots, calling_abi_extent_dependencies,
    foreign_opaque_carrier, foreign_record_storage, foreign_storage_extent_dependencies,
};
pub use storage::storage_dependencies::{
    inline_storage_extent_dependencies, storage_operand_dependencies,
};
pub mod control_graph;
pub use access::access_lifecycles::{
    AccessResultContract, access_result_contract, access_view_reference_slot,
    validate_access_lifecycle_plans, validate_access_view_content,
};
pub use access::access_ram_correspondence::{
    AccessRamCorrespondence, AccessRamFieldCorrespondence, access_ram_correspondence,
    access_ram_plan_correspondence, access_reference_correspondence,
};
pub use access::access_view_plans::{
    AccessViewPlanError, contains_access_carrier, contains_access_carrier_in_catalog,
    validate_access_view_catalog, validate_access_view_plans,
};
pub use access::accessor_scalar_boundaries::{
    AccessorScalarBoundaryDemand, access_region_projection, accessor_scalar_boundary_demand,
};
pub use descriptors::descriptor_results;
pub(crate) use optimization::digest_encoder;
pub use references::resident_references::{
    resident_reference_plan, validate_resident_owner_fields,
};
pub use structural::nominal_cleanup;
pub use structural::owned_structure;
mod access;
mod bytes;
mod calls;
pub mod content_transformations;
mod descriptors;
mod domain_preservation;
mod exactly_one;
mod references;
mod scalars;
mod storage;
mod structural;
pub use calls::invocation_inputs::validate_empty_erased_invocation_inputs;
pub use descriptors::membership_presentations::{
    present_structural_membership_contract, present_structural_memberships,
};
pub use descriptors::selected_memberships::{
    reference_carrier_membership_contract, reference_carrier_memberships,
    reference_source_memberships,
};
pub use domain_preservation::{
    domain_predicate_is_closed, domain_predicate_observes_no_storage, domain_write_is_disjoint,
    reference_preserves_ancestor_qualifications, scalar_field_store_preserves_content,
    scalar_field_store_preserves_domain_content, store_preserves_content,
    store_preserves_domain_content, store_preserves_qualifications,
};
pub use references::reference_completion::{
    ReferenceCompletionError, ReferenceCompletionOccurrence, borrowed_entry_reference_referent,
    cleanup_disposes_reference, validate_reference_result_completion,
};
pub use references::reference_origins::{
    BorrowedResultLeaf, BorrowedResultLeafKind, ReferenceCallContract, ReferenceCallOrigin,
    ReferenceCallResult, ReferenceCallSource, ReferenceLeaf, ReferenceOriginBinding,
    ReferenceResultMemberships, bind_public_reference_call_results, bind_reference_call_results,
    bind_reference_origin, borrowed_result_leaves, captured_reference_origin_type,
    collection_element_type, reference_call_preserved_memberships, reference_leaves,
    reference_origin_is_covered, reference_origin_paths_may_overlap, reference_origin_type,
    reference_result_memberships_weaken, reference_result_refines_regions,
    validate_boundary_reference_regions, validate_descriptor_result_regions,
    validate_dynamic_reference_regions,
};
pub use references::reference_storage_geometry::{
    RecordStorageResolver, ReferenceArrayWindow, ReferenceRecordField, ReferenceRepeatedField,
    ReferenceScalarStorage, ReferenceStorageGeometry, ReferenceStorageProjection,
    ReferenceStorageRegion, StorageGeometryError, UnselectedRecordStorage,
    reference_scalar_storage, reference_storage_dependencies, reference_storage_layout,
    reference_storage_projection, reference_storage_region, reference_storage_size,
};
pub use scalars::scalar_ordering;
pub use structural::inline_carriers::{
    InlineCarrierError, inline_carrier_declaration, inline_field_presentation,
    validate_inline_carriers,
};
mod semantic_rows;
pub use structural::structural_store::{discardable_store_value, structural_store_value_admitted};
pub mod substitution;

pub use access::access_view_referent::{
    AccessViewInputError, AccessViewReferentError, access_view_input_parameter,
    validate_access_view_referent,
};
pub use bytes::byte_carriers::{
    ByteCarrierLayout, ByteCarrierScalarLeaf, byte_carrier_scalar_layout,
};
pub use bytes::byte_sequence_concatenation::{
    byte_sequence_concatenation_capacity_goal, byte_sequence_concatenation_length_equation,
};
pub use bytes::byte_sequence_predicates::{
    ScalarConstantOrigin, byte_copied_field_predicates_at_transfer, byte_field_root,
    byte_literal_qualifications_hold, byte_literal_qualifications_hold_in, byte_owned_call_result,
    byte_owned_result_predicates, byte_parameter_predicates,
    byte_parameter_predicates_from_domains, byte_parameter_predicates_in, byte_predicate_carrier,
    byte_predicate_requirement, byte_qualification_predicates_in,
    byte_sequence_concatenation_predicates, byte_sequence_source_predicates, byte_view_call_result,
    indexed_byte_store_goal, machine_scalar_constant_origin, pure_byte_qualifications,
    scalar_array_qualifications_hold, scalar_constant_value, shared_byte_field_copy_predicates,
};
pub use calls::call_composition::{
    BorrowedReferentPresentation, CallArgumentRule, CallCompositionSchema,
    CallCompositionSemanticRow, CallCrashRule, CallEvidenceRule, CallFrontierRule, CallFuelPolicy,
    CallOutcomeRule, CallRequirementRule, CallResultRule, CallTargetRule, CallTransferRule,
    FixedCollectionWindow, borrowed_referent_matches, borrowed_referent_presentation,
    boundary_buffer_capacity, bounded_byte_owner_capacity,
    bounded_byte_owner_capacity_from_catalogs, bounded_byte_owner_capacity_in,
    call_composition_semantic_row, exact_call_composition_semantic_row_in, fixed_byte_array_extent,
    fixed_byte_array_loan, fixed_byte_array_window, fixed_byte_extent, fixed_byte_storage_window,
    fixed_collection_storage_window, fixed_element_array_extent, shared_boundary_buffer_capacity,
    stored_view_field_presentation, validate_call_composition_semantic_rows,
};
pub use references::reference_views::{
    descriptor_geometry_matches, descriptor_reborrow_matches, descriptor_reborrow_shapes_match,
    descriptor_source_projection_accessible, descriptor_value_transfer_matches,
    is_copyable_carrier, is_shared_reference_carrier, reference_byte_field_compatible,
    reference_source_compatible, reference_source_projection_accessible,
    reference_source_projection_compatible, reference_source_storage_path,
    reference_storage_byte_width, reference_storage_export_byte_width,
    reference_storage_region_source_compatible, reference_storage_source_compatible,
    reference_storage_view_is_compatible, representation_view_declaration,
    scalar_reference_view_is_total, scalar_reference_view_value, shared_descriptor_copy_matches,
    stored_reference_access,
};
pub use scalars::proof_bearing_scalar::{
    CanonicalScalarGoal, ProofBearingIntegerPolicyBinding, ProofBearingScalarLeafElision,
    ProofBearingScalarLeafSchema, ProofBearingScalarLeafSemantics, ProofBearingScalarSemanticRow,
    elidable_proof_bearing_scalar_leaf, exact_proof_bearing_scalar_semantic_row_in,
    proof_bearing_integer_policy_binding, proof_bearing_scalar_leaf_semantics,
    proof_bearing_scalar_semantic_row, validate_proof_bearing_scalar_semantic_rows,
};
pub use scalars::scalar_array::{
    scalar_array_leaf_declaration, scalar_array_leaf_path, scalar_array_leaf_shape,
};
pub use scalars::scalar_field_requirements::{
    atomic_resident_requirement, scalar_field_accepts_literal, scalar_field_accepts_zero,
    scalar_field_entails, scalar_field_initialization_requirement, scalar_field_requirement,
    scalar_field_requires_evidence, scalar_field_requires_initialization_evidence,
};
pub use scalars::scalar_leaf_schema::{
    GoalFreeScalarLeafSchema, ScalarLeafCrashPolicy, ScalarLeafDenotation, ScalarLeafFactShape,
    ScalarLeafFrontierPolicy, ScalarLeafFuelPolicy, ScalarLeafGoalShape, ScalarLeafOperandShape,
    ScalarLeafResultShape, ScalarLeafSchema,
};
pub use scalars::scalar_leaf_semantics::{
    GoalFreeScalarLeafSemantics, ScalarLeafLiteral, constant_goal_free_scalar_leaf,
    goal_free_scalar_leaf_semantics,
};
pub use scalars::scalar_representation_view::{ScalarRepresentationViewPair, ScalarViewValue};
pub use semantic_rows::{
    OperationSemanticCustody, OperationSemanticError, OperationSemanticRow, OperationSemanticTag,
    exact_operation_semantic_row_in, is_unconditionally_total_scalar, operation_semantic_row,
};
pub use storage::arrival_storage::{
    StorageNames, arrival_parameter, borrowed_storage_parameter,
    borrowed_storage_reference_ceiling, descriptor_parameter_access, projection_before_edge,
    root_before_edge, storage_names, view_descriptor_parameter,
    view_descriptor_projected_qualifications,
};
pub use storage::collection_backing::{
    CollectionBacking, CollectionExtent, CollectionStorage, CollectionViewDescriptor,
    collection_backing, collection_view_descriptor,
};
pub use storage::storage_regions::{
    StorageRegion, canonical_field_storage_region, crosses_unaliased_reference,
    projected_storage_region,
};
pub use storage::zero_valid_storage::{scalar_declaration_accepts_zero, zero_valid_record_storage};
pub use structural::partial_affine::partial_affine_residuals;
pub use structural::primitive_place::{
    Position, StructuralStorageSelection, borrowed_primitive_projection_declaration,
    byte_carrier_geometry_matches, canonical_primitive_path, canonical_primitive_read_path,
    canonical_static_projection, fixed_array_place_shape, primitive_place_type,
    primitive_projection_declaration, primitive_projection_restriction, primitive_projection_type,
    primitive_read_field, primitive_read_projection_type, primitive_read_requirement,
    projection_stays_in_inline_storage, projection_storage_dependencies,
    runtime_structural_copy_type, runtime_structural_path_tip, stored_descriptor_paths,
};
pub use structural::record_field::{
    CasePayloadCarrier, RecordFieldCarrier, SequenceFieldObservation, case_payload_carrier,
    observed_field_backing_access, observed_field_declaration, projected_field_declaration,
    record_field_carrier, runtime_structural_field, sequence_field_allows_observation,
    static_field_projection,
};
pub use structural::shared_mutation::{SharedMutation, shared_mutation};
pub use structural::static_path::{
    canonical_case_leaf_type, canonical_storage_dependencies, canonical_structural_path_tip,
    canonical_structural_storage_path, constant_runtime_index, interior_member_borrow,
    qualification_path_carrier, runtime_index_bound, runtime_index_selects,
    structural_memberships_disjoint_from_write, structural_paths_may_overlap,
    structural_paths_select_same_location,
};
pub use structural::structural_effect::{
    StructuralEffectAction, StructuralEffectCustody, StructuralEffectExternalEffect,
    StructuralEffectFrontierPolicy, StructuralEffectFuelPolicy, StructuralEffectGoalShape,
    StructuralEffectLeafSchema, StructuralEffectObservation, StructuralEffectResultShape,
    StructuralEffectSemanticRow, byte_equal_equation, byte_predicate_equation,
    element_establishment_length_equation, element_subslice_length_equation,
    element_view_leaf_scalar, exact_structural_effect_semantic_row_in, literal_length_equation,
    same_descriptor_coordinate, structural_effect_leaf_observation,
    structural_effect_leaf_observation_in, structural_effect_semantic_row,
    subslice_length_equation, validate_structural_effect_semantic_rows,
};
pub use structural::structural_window::{
    StructuralWindowProjection, structural_window_origins, structural_window_projection,
    structural_window_selects_child,
};
pub use structural::zeroed_structural::{
    EstablishedErasedStructuralField, ZeroedStructuralLeaf,
    zeroed_projected_byte_qualifications_hold, zeroed_structural_fields_satisfy,
    zeroed_structural_leaves, zeroed_structural_leaves_modulo_references,
    zeroed_structural_leaves_with_established_erased_fields, zeroed_structural_shape,
};

pub use descriptors::descriptor_lifetimes;

pub use storage::representation_regions::{
    ElementViewRegionGeometry, RepresentationElementLayout, RepresentationScalarLeaf,
    element_view_region_geometry, element_view_shape_region_geometry, representation_byte_extent,
    representation_element_layout, representation_region_goal, representation_region_length,
    whole_representation_count_goal,
};

pub use storage::element_representation::{
    ElementRepresentationLayout, ElementRepresentationLeaf, ElementRepresentationRelation,
    element_representation_layout, element_representation_relation,
};

pub use storage::collection_backing::{
    collection_view_backing, collection_view_preserves_coordinates,
};

pub use structural::structural_claims::{
    result_claim_catalog_matches_returns, result_claim_paths_match_returns,
};
pub use structural::structural_effect::{
    ElementViewLeaf, element_view_leaf, element_view_write_goal,
};

pub use scalars::scalar_observations::structural_read_scalar_type;

pub mod quotients;
pub use access::access_ram_origins::{
    AccessRamFormation, AccessRamOrigin, AccessRamOriginAnalysis,
};

pub use descriptors::primitive_store_memberships::{
    primitive_scalar_memberships, primitive_store_byte_predicates,
    primitive_store_preserves_scalar_memberships, primitive_store_restriction,
    scalar_projection_entails_membership,
};

pub use calls::evidence_calls;
pub use calls::evidence_calls::validate_runtime_evidence_inputs;
pub use scalars::unchanged_scalar::{UnchangedScalarOrigin, unchanged_scalar_identity};

pub(crate) use structural::structural_declarations::{structural_declaration, structural_shape};

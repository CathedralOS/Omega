#![forbid(unsafe_code)]

//! Shared judgments over Terminal Psi. Source checking, native lowering and the
//! interpreter use these rules for operation meaning, storage, calls and references.
//! The operation catalog is in `operations.rs`. Descriptor result and lifetime
//! queries share the replay engine under `descriptors/replay/`.

pub(crate) mod access;
pub mod body_services;
pub(crate) mod bytes;
pub mod call_projection;
pub mod callable_envelopes;
pub mod callable_values;
pub mod calls;
pub mod content_transformations;
pub mod entry_values;
pub mod evaluation_crashes;
pub mod evaluation_scope;
pub mod external_completions;
pub mod scalar_conditions;
pub mod synchronous_calls;
pub use content_transformations::{domain_preservation, quotients};
pub mod control_graph;
pub(crate) mod descriptors;
mod domain_issuance;
pub use domain_issuance::DeniedEstablishment;
pub mod float_meaning_sources;
pub mod inhabitation;
mod membership_facts;
pub mod module_analyses;
pub use membership_facts::{
    BodyMemberships, MembershipTest, TestedMembership, test_memberships_in,
    validate_boundary_normal_memberships,
};
pub use module_analyses::{
    DescriptorRowFamily, DescriptorRowView, DescriptorRows, MachineSlots, ModuleAnalyses,
};
pub mod normal_completion;
pub mod numeric_primitives;
pub mod plain_affine;
pub mod reference_ends;
pub mod references;
pub mod scalar_qualifications;
mod scalar_target_ranges;
pub use scalar_target_ranges::{
    TargetScalarRange, array_extent_literal_conditions, array_extent_literal_conditions_in,
    layout_call_conditions, layout_call_conditions_in, target_scalar_ranges,
    target_scalar_ranges_in,
};
pub(crate) mod scalars;
pub mod static_reach;
pub mod storage;
pub mod structural;
pub mod structural_qualifications;
pub mod substitution;
pub mod suspension_frontier;
pub mod theorems;

pub(crate) use ::wire::digest_encoder;
pub use access::access_lifecycles::{
    AccessResultContract, access_result_contract, access_view_reference_slot,
    validate_access_lifecycle_plans, validate_access_view_content,
};
pub use access::access_ram_correspondence::{
    AccessRamCorrespondence, access_ram_plan_correspondence, access_reference_correspondence,
};
pub use access::access_view_plans::{
    access_view_matches_lifecycle, contains_access_carrier, validate_access_view_plans,
};
pub(crate) use access::access_view_plans::{
    contains_access_carrier_in_catalog, validate_access_view_catalog,
};
pub use access::accessor_scalar_boundaries::{
    AccessorScalarBoundaryDemand, access_region_projection, accessor_scalar_boundary_demand,
};
pub use calls::closed_conformances;
pub use calls::closed_reach;
pub use descriptors::descriptor_results;
pub use references::resident_references::{
    resident_reference_plan, validate_resident_owner_fields,
};
mod owned_sequence_bounds;
pub use owned_sequence_bounds::closed_owned_sequence_bounds;
pub use storage::activation_storage;
pub use storage::borrowed_storage;
pub use storage::calling_storage::{callback_storage_has_slots, calling_abi_extent_dependencies};
pub use storage::run_custody::{EmptyRunReleaseContract, empty_run_release_contract};
pub use storage::storage_dependencies::{
    inline_storage_extent_dependencies, storage_operand_dependencies,
};
pub use storage::structural_locals::validate_vacant_structural_local;
pub use structural::nominal_cleanup;
pub use structural::owned_structure;
pub(crate) mod exactly_one {

    pub(crate) fn exactly_one<Row>(
        rows: impl Iterator<Item = Row>,
        keep: impl Fn(&Row) -> bool,
    ) -> Option<Row> {
        let mut matching = rows.filter(|row| keep(row));
        let row = matching.next()?;
        matching.next().is_none().then_some(row)
    }
}
pub use calls::invocation_inputs::validate_empty_erased_invocation_inputs;
pub use descriptors::membership_presentations::present_structural_memberships;
pub use domain_preservation::{
    domain_predicate_is_closed, domain_predicate_observes_no_storage,
    reference_preserves_ancestor_qualifications, scalar_field_store_preserves_content,
    scalar_field_store_preserves_domain_content, store_preserves_content,
};
pub use references::reference_completion::{
    BorrowedLoanSource, BorrowedLoanSources, CallLifetimeIndex, ReferenceCompletionError,
    ReferenceCompletionOccurrence, RetainedCallLoan, borrowed_loan_source, borrowed_loan_sources,
    disposed_reference_on_return, incoming_view_occurrence, reference_call_lifetimes,
    reference_call_lifetimes_indexed, reference_loan_parent, validate_reference_result_completion,
};
pub use references::reference_origins::{
    BorrowedResultLeaf, BorrowedResultLeafKind, ReferenceCallContract, ReferenceCallOrigin,
    ReferenceCallResult, ReferenceCallSource, ReferenceOriginBinding, ReferenceResultMemberships,
    bind_reference_call_results, bind_reference_origin, borrowed_result_leaves,
    reference_origin_type, reference_result_contents, reference_result_refines_regions,
    validate_descriptor_result_regions, validate_dynamic_reference_regions,
};
pub(crate) use references::reference_origins::{ReferenceInventory, reference_inventory};
pub use references::reference_storage_geometry::{
    RecordStorageResolver, ReferenceArrayWindow, ReferenceRecordField, ReferenceRepeatedField,
    ReferenceScalarStorage, ReferenceStorageGeometry, ReferenceStorageProjection,
    ReferenceStorageRegion, StorageGeometryError, UnselectedRecordStorage,
    authored_field_storage_extent, reference_scalar_storage, reference_storage_layout,
    reference_storage_projection, reference_storage_region, reference_storage_size,
    scalar_field_storage_geometry, schema_field_layout_source,
    validate_storage_destination_intervals,
};
pub use references::{DeclaredReferenceResult, selected_reference_result};
pub use references::{
    boundary_result_lifetimes, boundary_result_lifetimes_match, boundary_result_slots,
};
pub(crate) use references::{public_reference_result, requirement_reference_result};
pub use scalars::scalar_ordering;
pub use structural::inline_carriers::{
    InlineCarrierError, inline_carrier_declaration, inline_field_presentation,
    validate_inline_carriers,
};
pub(crate) mod operations;
pub use structural::structural_store::{
    StructuralStoreValueError, structural_store_value_admitted, validate_structural_store_values,
};

pub use access::access_view_referent::{
    AccessViewInputError, AccessViewReferentError, access_view_input_parameter,
    validate_access_view_referent,
};
pub use bytes::byte_carriers::{
    ByteCarrierLayout, ByteCarrierScalarLeaf, byte_carrier_scalar_layout,
};
pub use bytes::byte_sequence_predicates::{
    MachineScalarIndex, ScalarConstantOrigin, ScalarConstants, byte_literal_qualifications_hold,
    byte_literal_qualifications_hold_in, byte_owned_call_result, byte_owned_result_predicates,
    byte_parameter_predicates, byte_parameter_predicates_in, byte_predicate_carrier,
    byte_predicate_requirement, byte_qualification_predicates_in,
    byte_sequence_argument_predicates, byte_sequence_source_predicates, byte_subslice_is_whole,
    byte_transfer_qualifications_hold, incoming_byte_sequence_argument_predicates,
    pure_byte_qualifications, scalar_array_qualifications_hold, scalar_operation_literal,
    scalar_operation_origin, shared_byte_field_copy_predicates,
};
pub use bytes::operation_bounds::operation_safety_goal;
pub use calls::call_composition::{
    BorrowedReferentPresentation, FixedCollectionWindow, borrowed_referent_matches,
    borrowed_referent_presentation, boundary_buffer_capacity, bounded_byte_owner_capacity,
    fixed_byte_array_loan, fixed_byte_array_window, fixed_byte_extent, fixed_byte_storage_window,
    fixed_collection_storage_window, fixed_element_array_extent, shared_boundary_buffer_capacity,
    stored_view_field_presentation,
};
pub(crate) use operations::{OperationSemanticCustody, operation_semantic_row};
pub use operations::{OperationSemanticError, OperationSemanticTag};
pub use references::reference_views::{
    borrowed_field_presentation, descriptor_geometry_matches, descriptor_reborrow_matches,
    descriptor_reborrow_shapes_match, descriptor_source_projection_accessible,
    descriptor_value_transfer_matches, is_copyable_carrier, reference_byte_field_compatible,
    reference_source_compatible, reference_source_projection_accessible,
    reference_source_projection_compatible, reference_source_storage_path,
    reference_storage_export_byte_width, reference_storage_source_compatible,
    scalar_reference_view_admits, shared_byte_descriptor_storage_matches,
    shared_descriptor_copy_matches, shared_direct_byte_descriptor_source, stored_reference_access,
    successor_argument_carrier_matches,
};
pub use references::representation_geometry::{
    all_bit_patterns_are_valid, reference_view_compatible, reference_view_offset_is_zero,
    validate_reference_view_source,
};
pub(crate) use scalars::proof_bearing_scalar::ProofBearingScalarSemanticRow;
pub use scalars::proof_bearing_scalar::{
    CanonicalScalarGoal, ProofBearingScalarLeafSemantics, TrappingIntegerSemantics,
    proof_bearing_scalar_leaf_semantics, trapping_integer_semantics,
};
pub use scalars::scalar_array::{
    PackedArrayRole, packed_integer_array_type, packed_integer_unary_array_type,
    scalar_array_application_leaf, scalar_array_leaf_declaration, scalar_array_leaf_path,
    scalar_array_leaf_shape,
};
pub use scalars::scalar_field_requirements::{
    scalar_field_accepts_literal, scalar_field_requires_evidence,
    scalar_field_requires_initialization_evidence,
};
pub(crate) use scalars::scalar_field_requirements::{
    scalar_field_accepts_zero, scalar_field_entails, scalar_field_requirement,
};
pub(crate) use scalars::scalar_leaf_schema::GoalFreeScalarLeafSchema;
pub use scalars::scalar_leaf_schema::{
    ScalarLeafCrashPolicy, ScalarLeafDenotation, ScalarLeafFactShape, ScalarLeafFrontierPolicy,
    ScalarLeafGoalShape, ScalarLeafOperandShape, ScalarLeafResultShape, ScalarLeafSchema,
    ScalarLeafWorkPolicy,
};
pub use scalars::scalar_leaf_semantics::{
    GoalFreeScalarLeafSemantics, SortedValueTypes, ValueTypes, goal_free_scalar_leaf_semantics,
};
pub use storage::arrival_storage::{
    arrival_parameter, borrowed_storage_parameter, borrowed_storage_reference_ceiling,
    borrowed_view_payload_parameter, descriptor_parameter_access, projection_before_edge,
    structural_argument_source, structural_projection_authority, view_descriptor_parameter,
    view_descriptor_projected_qualifications,
};
pub use storage::collection_backing::{
    CollectionBacking, CollectionExtent, CollectionStorage, CollectionViewDescriptor,
    collection_backing, collection_view_descriptor,
};
pub(crate) use storage::storage_regions::{
    canonical_field_storage_region, crosses_unaliased_reference, projected_storage_region,
};
pub(crate) use storage::zero_valid_storage::scalar_declaration_accepts_zero;
pub use structural::atomic_cell::atomic_cell_type;
pub(crate) use structural::partial_affine::{
    partial_affine_residuals, partial_affine_residuals_in_regions, structural_region_is_vacant,
};
pub use structural::primitive_place::{
    Position, RuntimeIndexGoalError, StructuralDeclarations, StructuralStorageSelection,
    borrowed_primitive_projection_declaration, borrowed_projection_type,
    borrowed_storage_field_type, canonical_inline_region, canonical_primitive_path,
    canonical_primitive_read_path, canonical_primitive_read_type, canonical_read_projection,
    canonical_static_projection, inline_descriptor_type, preceding_collection_length,
    primitive_projection_declaration, primitive_projection_restriction, primitive_projection_type,
    primitive_read_projection_type, projection_stays_in_inline_storage,
    runtime_read_projection_tip, runtime_structural_copy_type, runtime_structural_path_tip,
    scalar_index_bound, stored_descriptor_paths, visit_runtime_index_goals,
};
pub(crate) use structural::primitive_place::{
    byte_carrier_geometry_matches, primitive_operation_projection_type, primitive_read_field,
    projection_storage_dependencies,
};
pub use structural::record_field::{
    CasePayloadCarrier, RecordFieldCarrier, SequenceFieldObservation, case_payload_carrier,
    observed_field_backing_access, observed_field_declaration, projected_field_declaration,
    record_field_carrier, resolved_structural_path, runtime_structural_field,
    sequence_field_allows_observation, static_field_projection,
};
pub use structural::shared_mutation::{SharedMutation, shared_mutation};
pub(crate) use structural::static_path::canonical_storage_dependencies;
pub use structural::static_path::{
    canonical_case_leaf_type, canonical_structural_path_tip, canonical_structural_storage_path,
    constant_runtime_index, qualification_path_carrier, runtime_index_selects,
    structural_memberships_disjoint_from_write, structural_paths_may_overlap,
    structural_paths_select_same_location, visit_canonical_inline_carriers,
};
pub(crate) use structural::structural_effect::StructuralEffectExternalEffect;
pub use structural::structural_effect::{
    StructuralEffectAction, StructuralEffectCustody, StructuralEffectFrontierPolicy,
    StructuralEffectGoalShape, StructuralEffectLeafSchema, StructuralEffectObservation,
    StructuralEffectResultShape, StructuralEffectSemanticRow, StructuralEffectWorkPolicy,
    byte_predicate_equation, element_view_leaf_scalar, element_view_read_scalar,
    same_descriptor_coordinate, structural_effect_leaf_observation,
};
pub use structural::structural_window::{
    StructuralWindowProjection, structural_window_origins, structural_window_projection,
    structural_window_selects_child,
};
pub(crate) use structural::zeroed_structural::zeroed_structural_types_and_fields_satisfy;
pub use structural::zeroed_structural::{
    EstablishedErasedStructuralField, RepeatedZeroedElement, ZeroedStructuralLeaf,
    ZeroedStructuralStorage, zeroed_projected_byte_qualifications_hold, zeroed_structural_leaves,
    zeroed_structural_leaves_modulo_references, zeroed_structural_shape, zeroed_structural_storage,
};
pub use terminal_psi::types::representation_views::{
    ScalarRepresentationViewPair, ScalarViewValue, declaration as representation_view_declaration,
    reference_storage_byte_width, reference_storage_view_is_compatible,
    scalar_reference_view_is_total, scalar_reference_view_value,
};

pub use descriptors::descriptor_lifetimes;

pub use storage::representation_regions::{
    ElementViewRegionGeometry, RepresentationElementLayout, RepresentationScalarLeaf,
    element_view_shape_region_geometry, representation_byte_extent, representation_element_layout,
    whole_representation_count_goal, whole_representation_region_goal,
};

pub(crate) use storage::element_representation::element_representation_layout;
pub use storage::element_representation::{
    ElementRepresentationLayout, ElementRepresentationLeaf, ElementRepresentationRelation,
    element_representation_relation,
};

pub use storage::collection_backing::collection_view_preserves_coordinates;

pub use structural::structural_claims::{
    ResultClaimCatalogError, result_claim_paths_match_returns, structural_claim_paths_may_overlap,
    validate_result_claim_catalog,
};

pub use scalars::scalar_observations::{
    scalar_application_read_equation, storage_alignment_fact, structural_read_scalar_type,
    structural_read_type_with_crossings, structural_read_type_with_extents,
};

pub use access::access_ram_origins::{
    AccessRamFormation, AccessRamOrigin, AccessRamOriginAnalysis,
};

pub use descriptors::primitive_store_memberships::{
    IntrinsicMemberships, primitive_scalar_memberships, primitive_store_byte_predicates,
    primitive_store_preserves_scalar_memberships, primitive_store_restriction,
};

pub use scalars::unchanged_scalar::{UnchangedScalarOrigin, unchanged_scalar_identity};

pub(crate) use structural::structural_declarations::{structural_declaration, structural_shape};

pub mod provider_candidates;

pub mod logical_providers;
pub mod program_storage_entry;

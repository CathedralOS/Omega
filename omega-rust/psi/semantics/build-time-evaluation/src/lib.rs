#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.

mod access_plans;
mod admission;
mod build_machines;
mod build_time_evaluation;
mod const_domain_facts;
mod const_generic_calls;
mod const_generic_expressions;
mod const_initializers;
mod const_lengths;
mod layout_plans;
mod placed_views;
mod plan_laid;
mod range_arguments;
mod range_endpoints;
mod reflection;
mod syntax_probes;
mod wire_plans;

pub use access_plans::{compute_access_plan, compute_placement_plan};
pub use admission::{
    BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeSelectionAuthority,
};
pub use build_machines::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineEvaluationError,
    BuildMachineExecutionMode, BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemGrants,
    BuildMachineFilesystemMetadataLayout, BuildMachineFilesystemSponsor, PreparedBuildMachineEntry,
    PreparedBuildMachineProgram, evaluate_build_machine_arguments_measured,
    evaluate_build_machine_arguments_measured_with_sponsor,
    evaluate_build_machine_entry_arguments_measured,
    evaluate_build_machine_entry_arguments_measured_with_sponsor,
};
pub use checked_interpreter::{
    CURRENT_EVALUATION_SEMANTICS, EvaluationUsage, MeasuredEvaluation,
    SelectedBuildTimeBinaryOperator,
};
mod selected_operators;
pub use const_domain_facts::{
    evaluate_const_domain_facts, evaluate_const_domain_facts_with_authority,
};
pub use const_generic_calls::evaluate_const_generic_calls;
pub use const_lengths::{FoldedArrayLength, validate_folded_array_lengths};
pub use const_lengths::{
    evaluate_const_array_lengths, evaluate_const_array_lengths_with_authority,
    evaluate_zero_argument_machine, evaluate_zero_argument_machine_for_invocation,
};
pub use layout_plans::{
    BuildTimeValue, ValidatedConstMaterialization,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumFieldMaterialization,
    ValidatedConstRecordWithNestedSumRecordMaterialization,
    ValidatedConstRecordWithNestedSumRecordsMaterialization,
    ValidatedConstRecordWithRecursiveNestedSumsMaterialization,
    ValidatedConstRecordWithSumArrayMaterialization,
    ValidatedConstRecordWithSumArraysMaterialization, ValidatedConstRecordWithSumMaterialization,
    ValidatedConstSumMaterialization, compute_layout_plan, compute_layout_plan_with_authority,
    compute_native_layout_plan, compute_native_layout_plan_with_authority,
    evaluate_and_materialize_typed_owned_layout_into, materialize_typed_owned_layout_into,
    normalized_schema_report_fingerprint, validate_const_materializable_conventional_sum,
    validate_const_materializable_record_with_conventional_sum,
    validate_const_materializable_record_with_conventional_sum_array,
    validate_const_materializable_record_with_conventional_sum_arrays,
    validate_const_materializable_record_with_conventional_sums,
    validate_const_materializable_record_with_nested_sum_record,
    validate_const_materializable_record_with_nested_sum_records,
    validate_const_materializable_record_with_recursive_nested_sums,
    validate_const_materializable_typed_owned_layout,
};
pub use placed_views::{
    PlacedViewRecord, desugar_placed_views, validate_placed_view_plans,
    validate_placed_view_plans_with_authority,
};
pub use plan_laid::{
    PlanLaidRecord, compute_plan_laid_layouts, compute_plan_laid_layouts_with_authority,
    desugar_plan_laid_value_types,
};
pub use range_endpoints::evaluate_const_range_endpoints_with_authority;
pub use reflection::{
    CaseDescription, DeclarationDescription, FieldDescription, FieldInfo, MemberKind,
    MemberSelectionKey, NominalReferenceDescription, RuntimeVisitationPlan, SchemaNode,
    SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, ScopedSelectionReceiver, SelectionChoice,
    SelectionCoverage, SelectionProjection, SelectionRecord, SelectionRequirement,
    SelectionSnapshot, SemanticSchemaGraph, TypeParameterDescription, VisitationCall,
    VisitationOperation, construct_semantic_schema_graph, replay_runtime_visitation_plan,
    replay_selection_snapshot, replay_semantic_schema_graph,
};
pub use selected_operators::{
    SelectedBuildTimeProviderBody, validate_selected_operators, validate_selected_provider_bodies,
};
pub use wire_plans::{compute_wire_plans, compute_wire_plans_with_authority};

pub use build_time_evaluation::{
    BuildTimeEvaluationRequest, BuildTimeSourceContext, PreCheckEvaluation,
    PreResolutionEvaluation, evaluate_pre_resolution,
};

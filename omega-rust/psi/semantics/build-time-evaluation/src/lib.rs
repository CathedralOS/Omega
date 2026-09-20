#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.
//!
//! Start at `build_time_evaluation.rs`: build machines are admitted against the
//! checked program, executed through the checked interpreter, and their results
//! installed as const lengths, const domain facts, layout plans, placed views,
//! and wire plans. Omega schedules these services; it never reinterprets them.
//!
//! `build_time_evaluation.rs` is the lifecycle root. The evaluators are grouped
//! by subject: `const_evaluation/`, `layouts/` and `machine_execution/`, each
//! opened by a route file that lists its modules.

mod build_time_evaluation;
mod const_evaluation;
mod layouts;
mod machine_execution;

pub use checked_interpreter::{
    CURRENT_EVALUATION_SEMANTICS, EvaluationUsage, MeasuredEvaluation,
    SelectedBuildTimeBinaryOperator,
};
pub use const_evaluation::const_domain_facts::evaluate_const_domain_facts;
pub use const_evaluation::const_generic_calls::evaluate_const_generic_calls;
pub use const_evaluation::const_lengths::{FoldedArrayLength, validate_folded_array_lengths};
pub use const_evaluation::const_lengths::{
    evaluate_const_array_lengths, evaluate_zero_argument_machine,
};
pub use const_evaluation::range_endpoints::evaluate_const_range_endpoints;
pub use layouts::access_plans::{compute_access_plan, compute_placement_plan};
pub use layouts::layout_plans::{
    BuildTimeValue, ValidatedConstMaterialization,
    ValidatedConstNestedSumRecordOccurrenceMaterialization,
    ValidatedConstRecordArrayElementSelection, ValidatedConstRecordArrayFieldMaterialization,
    ValidatedConstRecordSumArrayElementMaterialization,
    ValidatedConstRecordSumArrayElementSelection, ValidatedConstRecordSumArrayFieldMaterialization,
    ValidatedConstRecordSumChildMaterialization, ValidatedConstRecordSumFieldMaterialization,
    ValidatedConstRecordWithNestedSumRecordMaterialization,
    ValidatedConstRecordWithNestedSumRecordsMaterialization,
    ValidatedConstRecordWithRecursiveNestedSumsMaterialization,
    ValidatedConstRecordWithSumArrayMaterialization,
    ValidatedConstRecordWithSumArraysMaterialization, ValidatedConstRecordWithSumMaterialization,
    ValidatedConstSumMaterialization, compute_layout_plan, compute_native_layout_plan,
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
pub use layouts::placed_views::{
    PlacedViewRecord, desugar_placed_views, validate_placed_view_plans,
};
pub use layouts::plan_laid::{
    PlanLaidRecord, compute_plan_laid_layouts, desugar_plan_laid_value_types,
};
pub use layouts::wire_plans::compute_wire_plans;
pub use machine_execution::admission::{
    BuildTimeAdmissionPlan, BuildTimeAdmissionRejection, BuildTimeInvocationCustody,
    BuildTimeSelectionAuthority,
};
pub use machine_execution::build_machines::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineEvaluationError,
    BuildMachineExecutionMode, BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemGrants,
    BuildMachineFilesystemMetadataLayout, BuildMachineFilesystemSponsor, BuildMachineInvocation,
    PreparedBuildMachine, PreparedBuildMachineEntry, PreparedBuildMachineProgram,
    evaluate_build_machine_measured,
};
pub use machine_execution::reflection::{
    CaseDescription, DeclarationDescription, FieldDescription, FieldInfo, MemberKind,
    MemberSelectionKey, NominalReferenceDescription, RuntimeVisitationPlan, SchemaNode,
    SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, ScopedSelectionReceiver, SelectionChoice,
    SelectionCoverage, SelectionProjection, SelectionRecord, SelectionRequirement,
    SelectionSnapshot, SemanticSchemaGraph, TypeParameterDescription, VisitationCall,
    VisitationOperation, construct_semantic_schema_graph, replay_runtime_visitation_plan,
    replay_selection_snapshot, replay_semantic_schema_graph,
};
pub use machine_execution::selected_operators::{
    SelectedBuildTimeProviderBody, validate_selected_operators, validate_selected_provider_bodies,
};

pub use build_time_evaluation::{
    AuthorizedSelection, BuildTimeSources, SelectedBuildTimeOperators,
};
pub use build_time_evaluation::{
    BuildTimeEvaluationRequest, BuildTimeSourceContext, PreCheckEvaluation,
    PreResolutionEvaluation, evaluate_pre_resolution,
};

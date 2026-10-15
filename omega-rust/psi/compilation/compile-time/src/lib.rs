#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.
//!
//! Start at `build_time_evaluation.rs`: build machines are admitted against the
//! checked program, executed through the checked interpreter, and their results
//! installed as const lengths, const domain facts, layout plans, placed views,
//! and wire plans. Omega schedules these services; it never reinterprets them.
//!
//! `build_time_evaluation.rs` is the lifecycle root. The evaluators are grouped
//! by subject: `const_evaluation/`, `layouts/`, and `machine_execution/`.
//! `syntax_probes.rs` owns their temporary frontend work; `reflection/` owns
//! semantic schema, selection, and visitation plans and their independent replay.
//! `wire_demands.rs` decodes authored wire demands. Their compatibility
//! judgment belongs to Psi's typed-to-checked stage, not Build execution.
//! `provider_declarations.rs` decodes target defaults and executed Build choices
//! into exact source selections. Omega consumes them to realize providers.

// The build-time-evaluation tests' front-end pipelines, one file shared with
// the integration targets; see its module documentation.

mod build_time_evaluation;
mod const_evaluation;
mod fnv;
mod layouts;
mod machine_execution;
pub mod provider_declarations;
mod reflection;
mod syntax_probes;
mod wire_demands;

pub use language_semantics::wire::WireCompatibilityDemand;
pub use wire_demands::harvest_wire_compatibility_demands;

pub use checked_interpreter::{
    CURRENT_EVALUATION_SEMANTICS, EvaluationUsage, MeasuredEvaluation,
    SelectedBuildTimeBinaryOperator,
};
pub use const_evaluation::const_domain_facts::evaluate_const_domain_facts;
pub use const_evaluation::const_generic_calls::evaluate_const_generic_calls;
pub use const_evaluation::const_lengths::{
    evaluate_const_array_lengths, evaluate_zero_argument_machine,
};
pub use const_evaluation::range_endpoints::evaluate_const_range_endpoints;
pub use layouts::access_plans::{compute_access_plan, compute_placement_plan};
pub use layouts::layout_plans::{
    compute_layout_plan, compute_native_layout_plan, compute_native_layout_plan_for_machine,
};
pub use layouts::materialization::{
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
    ValidatedConstSumMaterialization, evaluate_and_materialize_typed_owned_layout_into,
    materialize_typed_owned_layout_into, validate_const_materializable_conventional_sum,
    validate_const_materializable_record_with_conventional_sums,
};
pub use layouts::placed_views::{PlacedViewRecord, desugar_placed_views};
pub use layouts::plan_laid::{
    PlanLaidRecord, compute_plan_laid_layouts, desugar_plan_laid_value_types,
};
pub use layouts::schema_reflection::normalized_schema_report_fingerprint;
pub use layouts::wire_plans::compute_wire_plans;
pub use machine_execution::admission::{
    BuildTimeAdmissionPlan, BuildTimeAdmissionRejection, BuildTimeInvocationCustody,
    BuildTimeProgram, BuildTimeSelectionAuthority,
};
pub use machine_execution::build_activation::{
    admit_toolchain_build_activation, is_exact_toolchain_build_prelude_data,
};
pub use machine_execution::build_machines::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineCapturedFilesystemRoot,
    BuildMachineEvaluationError, BuildMachineExecutionMode, BuildMachineFilesystemAccess,
    BuildMachineFilesystemGrantRoot, BuildMachineFilesystemGrantRootIdentity,
    BuildMachineFilesystemGrants, BuildMachineFilesystemMetadataLayout,
    BuildMachineFilesystemSponsor, BuildMachineFilesystemSponsorLimits, BuildMachineInvocation,
    CheckedBuildProgram, PreparedBuildMachineEntry, PreparedBuildMachineProgram,
    evaluate_build_machine_measured,
};
pub use machine_execution::selected_operators::{
    SelectedBuildTimeProviderBody, validate_selected_operators, validate_selected_provider_bodies,
};
pub use reflection::{
    CaseDescription, DeclarationDescription, FieldDescription, FieldInfo, MemberKind,
    MemberSelectionKey, NominalReferenceDescription, RuntimeVisitationPlan, SchemaNode,
    SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, ScopedSelectionReceiver, SelectionChoice,
    SelectionCoverage, SelectionProjection, SelectionRecord, SelectionRequirement,
    SelectionSnapshot, SemanticSchemaGraph, TypeParameterDescription, VisitationCall,
    VisitationOperation, construct_semantic_schema_graph, replay_runtime_visitation_plan,
    replay_selection_snapshot, replay_semantic_schema_graph,
};

pub use build_time_evaluation::{
    AuthorizedSelection, BuildTimeEvaluationRequest, BuildTimeSourceContext, BuildTimeSources,
    EvaluatedPreCheck, PreCheckEvaluation, PreResolutionEvaluation, SelectedBuildTimeOperators,
    evaluate_pre_resolution,
};

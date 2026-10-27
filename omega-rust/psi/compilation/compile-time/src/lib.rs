#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.
//!
//! Start at `build_time_evaluation.rs`: build machines are admitted against the
//! checked program. Admitted constants use common Terminal production and
//! execution; other Build jobs retain their checked-interpreter service. Results
//! are installed as const lengths, const domain facts, layout plans, placed views,
//! and wire plans. Omega schedules these services; it never reinterprets them.
//!
//! `build_time_evaluation.rs` is the lifecycle root. The evaluators are grouped
//! by subject: `const_evaluation/`, `layouts/`, and `machine_execution/`.
//! `const_evaluation/selected_constants.rs` owns deferred evaluation order;
//! layout providers answer geometry without choosing source checks or mutations.
//! `syntax_probes.rs` owns their temporary frontend work; `reflection/` owns
//! semantic schema, selection, and visitation plans and their independent replay.
//! `wire_demands.rs` decodes authored wire demands. Their compatibility
//! judgment belongs to Psi's typed-to-checked stage, not Build execution.
//! `provider_declarations.rs` decodes target defaults and executed Build choices
//! into exact source selections. Omega consumes them to realize providers.

// The build-time-evaluation tests' front-end pipelines, one file shared with
// the integration targets; see its module documentation.

pub mod assumed_bindings;
mod build_time_evaluation;
mod const_evaluation;
mod evaluation_accounting;
mod fnv;
pub use evaluation_accounting::{
    PureEvaluationOutcome, PureEvaluationProgram, PureEvaluationReceipt, PureEvaluationRecord,
    PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt, RecordedPreCheck,
    SourceCompletionAccounting,
};
mod layouts;
mod machine_execution;
pub mod provider_declarations;
mod reflection;
mod syntax_probes;
mod wire_demands;

pub use language_semantics::wire::WireCompatibilityDemand;
pub use wire_demands::harvest_wire_compatibility_demands;

pub use build_time_values::BuildTimeValue;
pub use checked_interpreter::{
    CURRENT_EVALUATION_SEMANTICS, EvaluationUsage, MeasuredEvaluation,
    SelectedBuildTimeBinaryOperator, SelectedBuildTimeMachineBody,
};
pub use const_evaluation::const_domain_facts::evaluate_const_domain_facts;
pub use const_evaluation::const_generic_calls::evaluate_const_generic_calls;
pub use const_evaluation::const_lengths::{
    evaluate_const_array_lengths, evaluate_zero_argument_machine,
};
pub use const_evaluation::data_layout_queries::{
    LayoutQueryMeasurements, LayoutQueryProvider, fold_data_layout_queries,
};
pub use const_evaluation::range_endpoints::evaluate_const_range_endpoints;
pub use const_evaluation::retained_constants::{
    ConstantValueClosureRequest, RetainedConstantEvaluation,
};
pub use const_evaluation::selected_constants::SelectedConstantEvaluationRequest;
pub use layouts::layout_plans::compute_native_layout_plan_for_machine;
pub use layouts::placed_views::{PlacedViewRecord, desugar_placed_views};
pub use layouts::placement_lifecycles::{
    admit_placement_lifecycle_plans, retain_placement_lifecycle_plans,
};
pub use layouts::plan_laid::{PlanLaidRecord, desugar_plan_laid_value_types};
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
    AuthorizedSelection, BuildTimeEvaluationRequest, BuildTimeSources, EvaluatedPreCheck,
    PreCheckEvaluation, PreResolutionEvaluation, SelectedBuildTimeOperators,
    evaluate_pre_resolution,
};

pub use machine_execution::selected_target_bodies::{
    SelectedBuildTimeTargetBody, prepare_selected_target_family_calls,
};

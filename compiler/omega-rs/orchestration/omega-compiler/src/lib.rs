mod pipeline;

pub(crate) use omega_core::source;
pub(crate) use omega_source_files_to_tokens as lexer;
pub(crate) use omega_tokens_to_syntax_trees as parser;

pub use pipeline::{
    ByteOrder, CheckedCompilation, CompileOptions, CompileReport, ConsumptionInstant, DataSymbolId,
    EntryStubId, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    MaterializationAction, MaterializationContext, MaterializationDiagnostic, MaterializationWrite,
    RelocationTarget, ScalarFieldSchema, ScalarFieldValue, SelectedExternalRootProviderPlan,
    SymbolicFieldValue, SymbolicMaterializationPlan, ValidatedAccessPlan, ValidatedPlacementPlan,
    compile, compile_to_checked, compute_access_plan, compute_layout_plan, compute_placement_plan,
    decode_scalar_layout, derive_symbolic_materialization, evaluate_calling_policy_plan,
    materialize_scalar_layout_into, normalized_layout_plan_fingerprint,
    selected_external_root_provider_plan, selected_external_root_provider_plan_id,
};

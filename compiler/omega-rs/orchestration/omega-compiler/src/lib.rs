mod pipeline;

pub(crate) use omega_core::source;
pub(crate) use omega_source_files_to_tokens as lexer;
pub(crate) use omega_tokens_to_syntax_trees as parser;

pub use pipeline::{
    ByteOrder, CompileOptions, CompileReport, ConsumptionInstant, DataSymbolId, EntryStubId,
    GeneratedIdtLoadLowering, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    MaterializationAction, MaterializationContext, MaterializationDiagnostic, MaterializationWrite,
    RelocationTarget, SymbolicFieldValue, SymbolicMaterializationPlan, compile, compile_to_checked,
    compute_layout_plan, derive_symbolic_materialization, evaluate_calling_policy_plan,
    lower_prepared_idt_load, selected_external_root_provider_plan_id,
};

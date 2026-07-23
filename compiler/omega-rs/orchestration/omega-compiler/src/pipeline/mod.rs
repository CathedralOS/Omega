mod adapter_dispatch;
mod artifacts;
mod boundary_report;
mod build_config;
mod calling_policy_plans;
mod checked_entry;
pub mod compile_options;
pub mod compile_report;
pub mod compiler;
mod const_domain_facts;
mod const_generic_calls;
mod const_lengths;
pub mod frontend;
mod generic_instances;
mod layout_plans;
mod output;
mod plan_laid;
mod project;
mod provider_plans;
pub mod source;
mod stage;
mod stages;
mod target_machines;
mod task_plans;
mod timing;
mod trait_defaults;
mod trust_lockfile;
mod trust_report;
mod wire_plans;
mod wire_report;

pub use calling_policy_plans::evaluate_calling_policy_plan;
pub use checked_entry::compile_to_checked;
pub use compile_options::CompileOptions;
pub use compile_report::CompileReport;
pub use compiler::compile;
pub use layout_plans::{
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, compute_layout_plan,
};
pub use omega_layout_plans::{
    ByteOrder, ConsumptionInstant, DataSymbolId, EntryStubId, MaterializationAction,
    MaterializationContext, MaterializationDiagnostic, MaterializationWrite, RelocationTarget,
    SymbolicFieldValue, SymbolicMaterializationPlan, derive_symbolic_materialization,
};
pub use provider_plans::{
    GeneratedIdtLoadLowering, lower_prepared_idt_load, selected_external_root_provider_plan_id,
};

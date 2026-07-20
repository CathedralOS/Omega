mod adapter_dispatch;
mod artifacts;
mod boundary_report;
mod build_config;
mod checked_entry;
pub mod compile_options;
pub mod compile_report;
pub mod compiler;
mod const_lengths;
pub mod frontend;
mod generic_instances;
mod layout_plans;
mod output;
mod plan_laid;
mod project;
mod provider_plans;
mod provides_values;
pub mod source;
mod stage;
mod stages;
mod target_machines;
mod timing;
mod trust_lockfile;
mod trust_report;
mod wire_plans;
mod wire_report;

pub use checked_entry::compile_to_checked;
pub use compile_options::CompileOptions;
pub use compile_report::CompileReport;
pub use compiler::compile;
pub use layout_plans::{
    LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, compute_layout_plan,
};

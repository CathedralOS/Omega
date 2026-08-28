mod artifacts;
pub(crate) use omega_build_evaluation as build_config;
pub(crate) use omega_build_evaluation as build_replay_record;
pub(crate) use omega_provider_planning::calling_policy_plans;
pub(crate) mod checked_entry;
pub(crate) use omega_provider_planning::component_progress;
pub mod frontend;
#[path = "package/declaration_admission.rs"]
mod package_declaration_admission;
mod project;
pub(crate) use omega_provider_planning::approval as provider_approval;
pub(crate) use omega_provider_planning::plans as provider_plans;
pub mod source;
pub(crate) mod source_inspection;
mod stage;
mod stages;
#[path = "provider/target_machines.rs"]
mod target_machines;
pub(crate) use omega_provider_planning::task_plans;
mod timing;
mod wire_report;

pub(crate) use crate::compiler::CompileOptions;

pub(crate) use artifacts::write_checked_snapshot;
pub(crate) use checked_entry::CheckedCompilation;
pub(crate) use omega_build_evaluation::{
    BuildFilesystemReplayRecordLimits, ReviewOnlyBuildFilesystemReplayRecord,
};
pub(crate) use omega_package_compilation::{
    PackageCompilationInputs, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceConsumptionCommitment,
};

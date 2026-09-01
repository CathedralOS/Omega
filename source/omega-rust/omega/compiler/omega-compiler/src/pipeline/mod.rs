mod artifacts;
pub(crate) use omega_build_evaluation as build_config;
pub(crate) use omega_build_evaluation as build_replay_record;
pub(crate) use omega_provider_planning::calling_policy_plans;
pub(crate) mod checked_entry;
pub(crate) use omega_provider_planning::component_progress;
pub mod frontend;
mod optimization;
#[path = "package/declaration_admission.rs"]
mod package_declaration_admission;
mod phase_transitions;
mod project;
pub(crate) mod reporting;
pub(crate) use omega_provider_planning::approval as provider_approval;
pub(crate) use omega_provider_planning::plans as provider_plans;
pub mod source;
mod source_assembly;
mod stage;
#[path = "provider/target_machines.rs"]
mod target_machines;
pub(crate) use omega_provider_planning::task_plans;
mod timing;
pub(crate) mod x86_fma_plan_association;

pub(crate) use crate::compiler::CompileOptions;
pub(crate) use checked_entry::CheckedCompilation;
pub(crate) use omega_build_evaluation::{
    BuildFilesystemReplayRecordLimits, ReviewOnlyBuildFilesystemReplayRecord,
};
pub(crate) use omega_package_compilation::{
    PackageCompilationInputs, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceConsumptionCommitment,
};

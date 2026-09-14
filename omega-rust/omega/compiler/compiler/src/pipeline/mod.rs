//! Compilation pipeline stages, in route order beneath [`checked_entry`]:
//! source assembly and the frontend, package declaration admission, provider
//! selection over the selected target machines, optimization, phase
//! transitions, artifacts, and reporting.

mod artifacts;
mod build_scope;
pub(crate) mod checked_entry;
pub mod frontend;
mod optimization;
mod package;
mod phase_transitions;
mod project;
mod provider;
pub(crate) mod reporting;
pub mod source;
mod source_assembly;
mod stage;
mod timing;

pub(crate) use crate::CompileOptions;
pub(crate) use build_evaluation::{
    BuildFilesystemReplayRecordLimits, ReviewOnlyBuildFilesystemReplayRecord,
};
pub(crate) use checked_entry::CheckedCompilation;
pub(crate) use package_compilation::{
    PackageCompilationInputs, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceConsumptionCommitment,
};

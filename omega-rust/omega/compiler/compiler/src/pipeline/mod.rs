//! Compilation pipeline stages, in route order beneath [`checked_entry`]:
//! package declaration admission, provider selection over the selected
//! target machines, optimization, phase transitions, artifacts, and
//! reporting. Source assembly is the `source_assembly` crate.

mod artifacts;
pub(crate) mod checked_entry;
mod optimization;
mod package;
mod phase_transitions;
pub(crate) mod reporting;

pub(crate) use crate::CompileOptions;
pub(crate) use build_evaluation::ReviewOnlyBuildFilesystemReplayRecord;
pub(crate) use checked_entry::CheckedCompilation;
pub(crate) use package_compilation::{
    PackageCompilationInputs, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceConsumptionCommitment,
};

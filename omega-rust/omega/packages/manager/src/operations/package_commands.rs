//! Install and update orchestration. CLI parsing and printing stay in the binary.

mod candidate;
mod model;
mod planning;
mod proposal;
mod review;
mod state;

pub use model::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandStatus,
};

use super::{PackageFileTransaction, PackagePublicationLimits};
use model::failure;
use package_source::SourceResolverStorage;

/// Execute against the operator's ordinary resolver storage. Acquired packages
/// cannot choose the accepted lock, review directory, or publication authority.
pub fn execute_package_command(
    command: PackageCommand,
    options: PackageCommandOptions,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    let mut transaction =
        PackageFileTransaction::open(&options.project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    transaction.recover().map_err(failure)?;
    if matches!(command, PackageCommand::DiscardReview) {
        return state::discard(&transaction);
    }
    let storage =
        SourceResolverStorage::for_current_user_excluding_primary_git_roots(&[transaction
            .project_root()
            .to_path_buf()])
        .map_err(failure)?;
    candidate::execute(command, options.targets, &mut transaction, &storage)
}

/// The same workflow with caller-selected resolver storage, useful to embedded
/// callers and integration tests. Publication still belongs to the project.
pub fn execute_package_command_with_storage(
    command: PackageCommand,
    options: PackageCommandOptions,
    storage: &SourceResolverStorage,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    let mut transaction =
        PackageFileTransaction::open(&options.project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    transaction.recover().map_err(failure)?;
    if matches!(command, PackageCommand::DiscardReview) {
        return state::discard(&transaction);
    }
    candidate::execute(command, options.targets, &mut transaction, storage)
}

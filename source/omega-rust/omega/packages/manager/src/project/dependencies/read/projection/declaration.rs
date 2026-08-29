//! Build-entry declaration rejection mapping for dependency projection.

use super::super::error::DependencyProjectionError;
use crate::project::roles::BuildDeclarationError;

pub(super) fn map_build_declaration_error(
    error: BuildDeclarationError,
) -> DependencyProjectionError {
    match error {
        BuildDeclarationError::ScopedBuildMachine { scope } => {
            DependencyProjectionError::ScopedBuildMachine { scope }
        }
        BuildDeclarationError::DuplicateBuildMachines { count } => {
            DependencyProjectionError::DuplicateBuildMachines { count }
        }
        BuildDeclarationError::InvalidBuildMachine => {
            DependencyProjectionError::InvalidBuildMachine
        }
        BuildDeclarationError::MissingBuildEntry => DependencyProjectionError::MissingBuildEntry,
        BuildDeclarationError::InvalidBuildParameter => {
            DependencyProjectionError::InvalidBuildParameter
        }
        error => DependencyProjectionError::BuildDeclaration(Box::new(error)),
    }
}

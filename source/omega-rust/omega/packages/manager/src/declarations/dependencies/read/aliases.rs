//! Alias validation across the one flat dependency set.

use super::model::ProjectedDependencies;
use crate::declarations::{AliasName, PackageName};
use std::collections::BTreeMap;
use std::fmt;

/// Failure to validate requester-local aliases after package selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyAliasError {
    SelectedPackageCountMismatch {
        dependency_occurrences: usize,
        selected_packages: usize,
    },
    DuplicateAlias {
        alias: AliasName,
        first_occurrence: usize,
        conflicting_occurrence: usize,
    },
}

impl fmt::Display for DependencyAliasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelectedPackageCountMismatch {
                dependency_occurrences,
                selected_packages,
            } => write!(
                formatter,
                "cannot validate dependency aliases: {dependency_occurrences} dependency occurrences have {selected_packages} selected package declarations",
            ),
            Self::DuplicateAlias {
                alias,
                first_occurrence,
                conflicting_occurrence,
            } => write!(
                formatter,
                "dependency alias `{}` conflicts between occurrences {first_occurrence} and {conflicting_occurrence}",
                alias.as_str(),
            ),
        }
    }
}

impl std::error::Error for DependencyAliasError {}

pub(super) fn validate_alias_uniqueness(
    dependencies: &ProjectedDependencies,
    selected_package_names: &[PackageName],
) -> Result<(), DependencyAliasError> {
    let authored_dependencies = dependencies.authored_dependencies();
    if selected_package_names.len() != authored_dependencies.len() {
        return Err(DependencyAliasError::SelectedPackageCountMismatch {
            dependency_occurrences: authored_dependencies.len(),
            selected_packages: selected_package_names.len(),
        });
    }

    let mut aliases = BTreeMap::<AliasName, usize>::new();
    for (occurrence, (dependency, package_name)) in authored_dependencies
        .iter()
        .zip(selected_package_names)
        .enumerate()
    {
        let alias = dependency.resolved_alias(package_name);
        if let Some(first_occurrence) = aliases.insert(alias.clone(), occurrence) {
            return Err(DependencyAliasError::DuplicateAlias {
                alias,
                first_occurrence,
                conflicting_occurrence: occurrence,
            });
        }
    }
    Ok(())
}

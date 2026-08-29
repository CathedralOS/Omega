//! Alias validation across every statically projected active dependency set.

use super::model::ProjectedDependencies;
use crate::identity::{AliasName, PackageName};
use omega_target::TargetProfile;
use std::collections::BTreeMap;
use std::fmt;

/// The projected request set in which one requester-local alias conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveDependencyAliasScope {
    /// The alias occurs more than once among unconditional dependencies.
    Common,
    /// The alias conflicts in `common + by_profile[profile]`.
    Profile(TargetProfile),
}

/// Failure to validate the aliases derived from selected package declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveDependencyAliasError {
    ResolvedPackageCountMismatch {
        authored_occurrences: usize,
        selected_packages: usize,
    },
    DuplicateAlias {
        scope: ActiveDependencyAliasScope,
        alias: AliasName,
        first_occurrence: usize,
        conflicting_occurrence: usize,
    },
}

impl fmt::Display for ActiveDependencyAliasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResolvedPackageCountMismatch {
                authored_occurrences,
                selected_packages,
            } => write!(
                formatter,
                "cannot validate dependency aliases: {authored_occurrences} authored occurrences have {selected_packages} selected package declarations",
            ),
            Self::DuplicateAlias {
                scope,
                alias,
                first_occurrence,
                conflicting_occurrence,
            } => match scope {
                ActiveDependencyAliasScope::Common => write!(
                    formatter,
                    "dependency alias `{}` conflicts between common occurrences {first_occurrence} and {conflicting_occurrence}",
                    alias.as_str(),
                ),
                ActiveDependencyAliasScope::Profile(profile) => write!(
                    formatter,
                    "dependency alias `{}` conflicts for target profile `{}` between occurrences {first_occurrence} and {conflicting_occurrence}",
                    alias.as_str(),
                    profile.target_name(),
                ),
            },
        }
    }
}

impl std::error::Error for ActiveDependencyAliasError {}

pub(super) fn validate_active_alias_uniqueness(
    dependencies: &ProjectedDependencies,
    selected_package_names: &[PackageName],
) -> Result<(), ActiveDependencyAliasError> {
    if selected_package_names.len() != dependencies.authored_dependencies().len() {
        return Err(ActiveDependencyAliasError::ResolvedPackageCountMismatch {
            authored_occurrences: dependencies.authored_dependencies().len(),
            selected_packages: selected_package_names.len(),
        });
    }

    let resolved_aliases = dependencies
        .authored_dependencies()
        .iter()
        .zip(selected_package_names)
        .map(|(request, package_name)| request.resolved_alias(package_name))
        .collect::<Vec<_>>();
    let common_aliases = validate_column(
        dependencies.common_occurrence_indices(),
        &resolved_aliases,
        ActiveDependencyAliasScope::Common,
        BTreeMap::new(),
    )?;

    for column in dependencies.by_profile() {
        validate_column(
            column.occurrence_indices(),
            &resolved_aliases,
            ActiveDependencyAliasScope::Profile(column.profile()),
            common_aliases.clone(),
        )?;
    }

    Ok(())
}

fn validate_column(
    occurrence_indices: &[usize],
    resolved_aliases: &[AliasName],
    scope: ActiveDependencyAliasScope,
    mut aliases: BTreeMap<AliasName, usize>,
) -> Result<BTreeMap<AliasName, usize>, ActiveDependencyAliasError> {
    for occurrence_index in occurrence_indices {
        let alias = &resolved_aliases[*occurrence_index];
        if let Some(first_occurrence) = aliases.insert(alias.clone(), *occurrence_index) {
            return Err(ActiveDependencyAliasError::DuplicateAlias {
                scope,
                alias: alias.clone(),
                first_occurrence,
                conflicting_occurrence: *occurrence_index,
            });
        }
    }
    Ok(aliases)
}

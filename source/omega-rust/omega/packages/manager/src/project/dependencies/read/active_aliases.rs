//! Alias validation across every statically projected active dependency set.

use super::model::ProjectedDependencies;
use crate::package::{AliasName, PackageName};
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
        active_occurrences: usize,
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
                active_occurrences,
                selected_packages,
            } => write!(
                formatter,
                "cannot validate dependency aliases: {active_occurrences} active occurrences have {selected_packages} selected package declarations",
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
    profile: TargetProfile,
    selected_package_names: &[PackageName],
) -> Result<(), ActiveDependencyAliasError> {
    let active_occurrences = dependencies
        .occurrence_indices_for_profile(profile)
        .collect::<Vec<_>>();
    if selected_package_names.len() != active_occurrences.len() {
        return Err(ActiveDependencyAliasError::ResolvedPackageCountMismatch {
            active_occurrences: active_occurrences.len(),
            selected_packages: selected_package_names.len(),
        });
    }

    let resolved_aliases = active_occurrences
        .iter()
        .zip(selected_package_names)
        .map(|(occurrence, package_name)| {
            (
                *occurrence,
                dependencies.authored_dependencies()[*occurrence].resolved_alias(package_name),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let common_aliases = validate_column(
        dependencies.common_occurrence_indices(),
        &resolved_aliases,
        ActiveDependencyAliasScope::Common,
        BTreeMap::new(),
    )?;

    if let Some(column) = dependencies
        .by_profile()
        .iter()
        .find(|column| column.profile() == profile)
    {
        validate_column(
            column.occurrence_indices(),
            &resolved_aliases,
            ActiveDependencyAliasScope::Profile(profile),
            common_aliases.clone(),
        )?;
    }

    Ok(())
}

fn validate_column(
    occurrence_indices: &[usize],
    resolved_aliases: &BTreeMap<usize, AliasName>,
    scope: ActiveDependencyAliasScope,
    mut aliases: BTreeMap<AliasName, usize>,
) -> Result<BTreeMap<AliasName, usize>, ActiveDependencyAliasError> {
    for occurrence_index in occurrence_indices {
        let alias = resolved_aliases
            .get(occurrence_index)
            .expect("validated active occurrence has a selected package name");
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

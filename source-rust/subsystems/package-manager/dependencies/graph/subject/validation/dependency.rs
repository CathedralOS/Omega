// We validate dependency request byte limits, optional explicit aliases, and
// path/Git selection compatibility with the selected source lineage and
// navigation.

use super::super::{
    CanonicalDependencySourceRequest, CanonicalSourceClosureSubjectError, usage::Budget,
};
use super::source::{validate_git_selection, validate_request_bytes};
use crate::declarations::AliasName;
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;
use source_acquisition::{ImmutableSourceResolution, SourceLineage};

pub(in super::super) fn validate_dependency_selection_kind(
    request: &CanonicalDependencySourceRequest,
    requester_lineage: &SourceLineage,
    requester_resolution: &ImmutableSourceResolution,
    selected: &ResolvedSourceIdentity,
    selected_navigation: &PackageSourceNavigation,
    budget: &mut Budget,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match request {
        CanonicalDependencySourceRequest::Bundled { name, .. } => {
            if selected.key().name() != name
                || !matches!(
                    selected.key().source_lineage(),
                    SourceLineage::Bundled { .. }
                )
                || !matches!(
                    selected.resolution(),
                    ImmutableSourceResolution::Bundled { .. }
                )
                || selected_navigation != &PackageSourceNavigation::Root
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "bundled request disagrees with its selected package",
                ));
            }
        }
        CanonicalDependencySourceRequest::Path { .. } => match selected.key().source_lineage() {
            SourceLineage::Bundled { library, .. } => {
                if !matches!(requester_lineage, SourceLineage::Bundled { library: owner, .. } if owner == library)
                    || selected_navigation != &PackageSourceNavigation::Root
                {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "bundled relative path escaped its library",
                    ));
                }
            }
            SourceLineage::Workspace(_) | SourceLineage::ExternalLocal(_) => {
                if selected_navigation != &PackageSourceNavigation::Root {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "path request selected invalid package navigation",
                    ));
                }
            }
            SourceLineage::GitHub(_) | SourceLineage::GitLab(_) | SourceLineage::Git(_) => {
                if requester_lineage != selected.key().source_lineage()
                    || requester_resolution != selected.resolution()
                    || !matches!(selected_navigation, PackageSourceNavigation::Member(_))
                {
                    return Err(CanonicalSourceClosureSubjectError::new(
                        "Git member path request escaped its repository resolution",
                    ));
                }
            }
        },
        CanonicalDependencySourceRequest::Git {
            repository,
            revision,
            selection: package_selection,
            ..
        } => {
            let request = super::source::git_request(repository, revision, budget)?;
            if request.lineage() != selected.key().source_lineage()
                || !matches!(selected.resolution(), ImmutableSourceResolution::Git { .. })
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency Git request disagrees with its selected source",
                ));
            }
            validate_git_selection(package_selection, selected, selected_navigation)?;
        }
    }
    Ok(())
}

pub(in super::super) fn validate_dependency_request(
    request: &CanonicalDependencySourceRequest,
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match request {
        CanonicalDependencySourceRequest::Bundled {
            explicit_alias,
            name,
        } => {
            validate_optional_alias(explicit_alias.as_ref())?;
            validate_request_bytes(name.as_str().as_bytes(), maximum_request_bytes)
        }
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => {
            validate_optional_alias(explicit_alias.as_ref())?;
            validate_request_bytes(location.as_bytes(), maximum_request_bytes)
        }
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
            selection: _,
        } => {
            validate_optional_alias(explicit_alias.as_ref())?;
            validate_request_bytes(repository.as_bytes(), maximum_request_bytes)?;
            validate_request_bytes(revision.as_bytes(), maximum_request_bytes)
        }
    }
}

fn validate_optional_alias(
    alias: Option<&AliasName>,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if alias
        .as_ref()
        .is_some_and(|alias| alias.as_str().is_empty())
    {
        Err(CanonicalSourceClosureSubjectError::new(
            "dependency request contains an empty explicit alias",
        ))
    } else {
        Ok(())
    }
}

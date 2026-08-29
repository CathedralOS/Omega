use super::super::CanonicalSourceClosureSubjectError;
use crate::discovery::PackageSourceNavigation;
use crate::graph::ResolvedSourceIdentity;
use crate::identity::PackageKey;
use omega_package_source::{ImmutableSourceResolution, SourceLineage};

pub(super) fn validate_package_navigation(
    source: &ResolvedSourceIdentity,
    navigation: &PackageSourceNavigation,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if matches!(navigation, PackageSourceNavigation::Member(_))
        && (!matches!(
            source.key().source_lineage(),
            SourceLineage::GitHub(_) | SourceLineage::GitLab(_) | SourceLineage::Git(_)
        ) || !matches!(source.resolution(), ImmutableSourceResolution::Git { .. }))
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "member navigation requires an immutable Git package",
        ));
    }
    Ok(())
}

pub(super) fn validate_git_selection(
    selection: &crate::declarations::PackageSelection,
    selected: &ResolvedSourceIdentity,
    navigation: &PackageSourceNavigation,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match (selection, navigation) {
        (crate::declarations::PackageSelection::Root, PackageSourceNavigation::Root) => Ok(()),
        (
            crate::declarations::PackageSelection::Named(package),
            PackageSourceNavigation::Member(_),
        ) if package == selected.key().name() => Ok(()),
        (crate::declarations::PackageSelection::Named(package), _)
            if package != selected.key().name() =>
        {
            Err(CanonicalSourceClosureSubjectError::new(
                "named Git package selection disagrees with its selected package",
            ))
        }
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "Git package selection disagrees with package navigation",
        )),
    }
}

pub(super) fn validate_request_bytes(
    bytes: &[u8],
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if bytes.len() > maximum_request_bytes {
        Err(CanonicalSourceClosureSubjectError::new(
            "source request violates its byte limit",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_source_identity(
    source: &ResolvedSourceIdentity,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_package_key(source.key(), maximum_identity_bytes)?;
    if !source
        .resolution()
        .matches_lineage(source.key().source_lineage())
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source resolution disagrees with package lineage",
        ));
    }
    Ok(())
}

pub(in super::super) fn validate_package_key(
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    validate_identity_string(key.name().as_str(), maximum_identity_bytes)?;
    validate_source_lineage(key.source_lineage(), maximum_identity_bytes)
}

pub(in super::super) fn validate_source_lineage(
    lineage: &SourceLineage,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    let check = |value: &str| validate_identity_string(value, maximum_identity_bytes);
    match lineage {
        SourceLineage::GitHub(lineage) => {
            check(lineage.owner())?;
            check(lineage.repository())
        }
        SourceLineage::GitLab(lineage) => check(lineage.repository_path()),
        SourceLineage::Git(lineage) => {
            if let Some(user) = lineage.user() {
                check(user)?;
            }
            check(lineage.host())?;
            check(lineage.repository_path())
        }
        SourceLineage::Workspace(lineage) => check(lineage.member_path().as_str()),
        SourceLineage::ExternalLocal(lineage) => {
            check(lineage.canonical_absolute_path().to_str().ok_or_else(|| {
                CanonicalSourceClosureSubjectError::new("external-local lineage path is not UTF-8")
            })?)
        }
    }
}

fn validate_identity_string(
    value: &str,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if value.is_empty() || value.len() > maximum_identity_bytes {
        Err(CanonicalSourceClosureSubjectError::new(
            "source identity violates its byte bounds",
        ))
    } else {
        Ok(())
    }
}

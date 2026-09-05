//! Exact source replacement associations and budgeted owned field copies.
use super::*;
use crate::review::{PackagePolicyReplacementSite, PackagePolicySourceReplacement};

pub(super) fn capture(
    replacement: &PackagePolicySourceReplacement,
    source: &Source,
    limits: Limits,
    usage: &mut Usage,
    maximum_owned_bytes: usize,
    fragment_bytes: &mut usize,
) -> Result<Subject, Error> {
    let package_index = source
        .packages()
        .binary_search_by(|package| package.key().cmp(replacement.candidate()))
        .map_err(|_| Error::UnknownPackage)?;
    let site = match replacement.site() {
        PackagePolicyReplacementSite::Root => ReplacementSite::Root,
        PackagePolicyReplacementSite::Dependency { requester, alias } => {
            *fragment_bytes = fragment_bytes
                .checked_add(alias.as_str().len())
                .filter(|bytes| *bytes <= limits.maximum_bytes)
                .ok_or(Error::ByteLimitExceeded)?;
            ReplacementSite::Dependency {
                requester_index: source
                    .packages()
                    .binary_search_by(|package| package.key().cmp(requester))
                    .map_err(|_| Error::UnknownPackage)?,
                alias: copy_alias(alias.as_str(), usage, maximum_owned_bytes)?,
            }
        }
    };
    validate(replacement.baseline(), package_index, &site, source)?;
    let baseline = copy_key(
        replacement.baseline(),
        limits,
        usage,
        maximum_owned_bytes,
        fragment_bytes,
    )?;
    Ok(Subject::SourceReplacement {
        baseline,
        package_index,
        site,
    })
}

pub(super) fn copy_key(
    key: &PackageKey,
    limits: Limits,
    usage: &mut Usage,
    maximum_owned_bytes: usize,
    fragment_bytes: &mut usize,
) -> Result<PackageKey, Error> {
    let (fragment, owned) = write_package_key_text(
        key,
        key_limits(limits.maximum_bytes),
        maximum_owned_bytes - usage.owned_bytes,
    )
    .map_err(source_key_error)?;
    usage.charge(owned, maximum_owned_bytes)?;
    *fragment_bytes = fragment_bytes
        .checked_add(fragment.len())
        .filter(|bytes| *bytes <= limits.maximum_bytes)
        .ok_or(Error::ByteLimitExceeded)?;
    let (copy, owned) = recover_package_key_text(
        &fragment,
        key_limits(limits.maximum_bytes),
        maximum_owned_bytes - usage.owned_bytes,
    )
    .map_err(source_key_error)?;
    usage.charge(owned, maximum_owned_bytes)?;
    Ok(copy)
}

pub(super) fn copy_alias(
    text: &str,
    usage: &mut Usage,
    maximum_owned_bytes: usize,
) -> Result<AliasName, Error> {
    if text.len() > 1024 * 1024 || !AliasName::is_valid(text) {
        return Err(Error::InvalidSubject);
    }
    usage.charge(text.len(), maximum_owned_bytes)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| Error::AllocationFailed)?;
    owned.push_str(text);
    // The owning borrowed predicate above excludes the parser's formatted
    // error allocation; successful parsing moves this exact owned string.
    AliasName::parse(owned).map_err(|_| Error::InvalidSubject)
}

pub(super) fn validate(
    baseline: &PackageKey,
    package_index: usize,
    site: &ReplacementSite,
    source: &Source,
) -> Result<(), Error> {
    let candidate = source
        .packages()
        .get(package_index)
        .ok_or(Error::UnknownPackage)?
        .key();
    if baseline == candidate {
        return Err(Error::InvalidSubject);
    }
    match site {
        ReplacementSite::Root if candidate == source.root().selected().key() => Ok(()),
        ReplacementSite::Dependency {
            requester_index,
            alias,
        } => {
            let requester = source
                .packages()
                .get(*requester_index)
                .ok_or(Error::UnknownPackage)?
                .key();
            // Source selections are canonical by requester then authored index.
            // Restrict the borrowed scan to this exact requester's rows.
            let requests = source.dependency_requests();
            let start = requests.partition_point(|request| request.requester() < requester);
            let end = requests.partition_point(|request| request.requester() <= requester);
            if requests[start..end]
                .iter()
                .any(|request| request.alias() == alias && request.selected().key() == candidate)
            {
                Ok(())
            } else {
                Err(Error::InvalidSubject)
            }
        }
        _ => Err(Error::InvalidSubject),
    }
}

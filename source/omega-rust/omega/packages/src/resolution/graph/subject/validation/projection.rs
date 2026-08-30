//! Validation of complete target-conditioned dependency maps.

use super::super::{
    CanonicalDependencySourceRequest, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectLimits,
};
use super::dependency::validate_dependency_request;
use crate::declarations::dependencies::read::{
    ProjectedDependencies, TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION,
};
use omega_target::TargetProfile;

pub(super) fn validate_dependency_projection(
    projection: &ProjectedDependencies,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(usize, usize), CanonicalSourceClosureSubjectError> {
    if projection.condition_schema().version() != TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure dependency projection uses an unsupported condition schema",
        ));
    }
    let occurrences = projection.authored_dependencies();
    if occurrences.len() > limits.maximum_dependency_requests {
        return Err(CanonicalSourceClosureSubjectError::new(
            "source-closure dependency projection exceeds its request-count limit",
        ));
    }
    for request in occurrences {
        validate_dependency_request(
            &CanonicalDependencySourceRequest::from(request),
            limits.maximum_request_bytes,
        )?;
    }

    let referenced = projection
        .condition_schema()
        .referenced_profile_identities();
    if referenced.len() > TargetProfile::ALL.len()
        || referenced
            .windows(2)
            .any(|pair| profile_identity_index(pair[0]) >= profile_identity_index(pair[1]))
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "referenced target profiles are not in strict canonical order",
        ));
    }

    let mut memberships = vec![0u8; occurrences.len()];
    validate_indices(
        projection.common_occurrence_indices(),
        occurrences.len(),
        &mut memberships,
        true,
    )?;
    let mut membership_count = projection.common_occurrence_indices().len();
    let columns = projection.by_profile();
    if columns.len() > TargetProfile::ALL.len()
        || columns
            .windows(2)
            .any(|pair| profile_index(pair[0].profile()) >= profile_index(pair[1].profile()))
    {
        return Err(CanonicalSourceClosureSubjectError::new(
            "target dependency columns are not in strict canonical order",
        ));
    }
    for column in columns {
        if column.occurrence_indices().is_empty()
            || !referenced.contains(&column.profile().identity())
        {
            return Err(CanonicalSourceClosureSubjectError::new(
                "target dependency column is empty or absent from its condition schema",
            ));
        }
        validate_indices(
            column.occurrence_indices(),
            occurrences.len(),
            &mut memberships,
            false,
        )?;
        membership_count = membership_count
            .checked_add(column.occurrence_indices().len())
            .ok_or_else(|| {
                CanonicalSourceClosureSubjectError::new(
                    "target dependency membership count overflowed",
                )
            })?;
    }
    if memberships.contains(&0) {
        return Err(CanonicalSourceClosureSubjectError::new(
            "authored dependency occurrence has no common or target column",
        ));
    }
    Ok((occurrences.len(), membership_count))
}

fn validate_indices(
    indices: &[usize],
    occurrence_count: usize,
    memberships: &mut [u8],
    common: bool,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if indices.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalSourceClosureSubjectError::new(
            "dependency occurrence indices are not in strict canonical order",
        ));
    }
    for index in indices {
        if *index >= occurrence_count {
            return Err(CanonicalSourceClosureSubjectError::new(
                "dependency occurrence index is out of range",
            ));
        }
        if common {
            if memberships[*index] != 0 {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency occurrence has duplicate common membership",
                ));
            }
            memberships[*index] = u8::MAX;
        } else if memberships[*index] == u8::MAX {
            return Err(CanonicalSourceClosureSubjectError::new(
                "common dependency occurrence also appears in a target column",
            ));
        } else {
            memberships[*index] = memberships[*index].checked_add(1).ok_or_else(|| {
                CanonicalSourceClosureSubjectError::new(
                    "dependency occurrence membership count overflowed",
                )
            })?;
        }
    }
    Ok(())
}

fn profile_index(profile: TargetProfile) -> usize {
    TargetProfile::ALL
        .iter()
        .position(|candidate| *candidate == profile)
        .expect("profile belongs to trusted catalog")
}

fn profile_identity_index(identity: omega_target::TargetProfileIdentity) -> usize {
    TargetProfile::ALL
        .iter()
        .position(|profile| profile.identity() == identity)
        .expect("profile identity belongs to trusted catalog")
}

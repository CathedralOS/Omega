//! Projection of crash predicates and permission-frontier claims.

use super::super::super::semantics::declarations::nominal_identity;
use crate::record::{
    PackageReviewCrashPredicate, PackageReviewPermissionClaim, PackageReviewPermissionSource,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;

pub(super) fn project_crash_predicates(
    predicates: &[checked_trees::CrashPredicateIdentity],
) -> Vec<PackageReviewCrashPredicate> {
    let mut projected = predicates
        .iter()
        .map(project_crash_predicate)
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

pub(super) fn project_crash_predicate(
    predicate: &checked_trees::CrashPredicateIdentity,
) -> PackageReviewCrashPredicate {
    PackageReviewCrashPredicate {
        canonical_bytes: predicate.canonical_bytes().to_vec(),
    }
}

pub(super) fn project_permission_claim(
    compilation: &CheckedCompilation,
    claim: language_semantics::PermissionClaimIdentity,
) -> Result<PackageReviewPermissionClaim, Vec<Diagnostic>> {
    let language_semantics::PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = claim
    else {
        return Err(vec![Diagnostic::error(
            "package review crash frontier contains an unidentified permission claim",
        )]);
    };
    let source = match source {
        language_semantics::PermissionEventSource::StateEntry => {
            PackageReviewPermissionSource::StateEntry
        }
        language_semantics::PermissionEventSource::Statement { statement_index } => {
            PackageReviewPermissionSource::Statement {
                statement_ordinal: portable_ordinal(statement_index)?,
            }
        }
        language_semantics::PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PackageReviewPermissionSource::Call {
            statement_ordinal: portable_ordinal(statement_index)?,
            call_ordinal: portable_ordinal(call_ordinal)?,
            target: nominal_identity(compilation, target_symbol)?,
        },
        language_semantics::PermissionEventSource::StateExit => {
            PackageReviewPermissionSource::StateExit
        }
    };
    Ok(PackageReviewPermissionClaim {
        machine: nominal_identity(compilation, machine_symbol)?,
        state: nominal_identity(compilation, state_symbol)?,
        source,
        ordinal,
    })
}

fn portable_ordinal(ordinal: usize) -> Result<u64, Vec<Diagnostic>> {
    u64::try_from(ordinal).map_err(|_| {
        vec![Diagnostic::error(
            "package review semantic ordinal exceeds the portable identity range",
        )]
    })
}

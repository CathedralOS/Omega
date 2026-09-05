use super::*;
use crate::resolution::graph::PackageSourceClosureLimitKind;
use crate::review::{
    PackageSourceReviewLimits, PackageTriageDisposition, PackageTriageReason,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictLimits,
    assemble_update_source_review, compare_review_only_capabilities,
    compile_resolved_package_candidate_reviews, triage_review_update,
};
use package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewSourceLocationRole,
};
use package_source::GitTransportProfile;
use std::collections::BTreeSet;

mod support;
use support::*;

mod capability_review;
mod external_local;
mod git_cache;
mod git_pins;
mod git_requests;
mod limits;
#[cfg(unix)]
mod offline;
#[cfg(not(unix))]
#[test]
#[ignore = "offline Git transport counting requires the Unix test-only SSH transport"]
fn offline_git_transport_fixture_requires_unix_shell() {}
mod selective_updates;
mod staged_external_local;
mod target_profiles;
mod workspace;

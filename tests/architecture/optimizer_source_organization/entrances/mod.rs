//! Meaningful optimizer-entrance and semantic-ladder audit entrance.
//!
//! The audit proceeds from the six rule-owning stages, through domain-grouped
//! executable joins and semantic ladders, to stage-specific protocol custody.

mod domains;
mod protocols;
mod requirements;
mod rule_stages;

use crate::Audit;

pub(super) use requirements::is_required_coordination_entrance;

pub(crate) fn check(audit: &mut Audit) {
    rule_stages::check(audit);
    domains::check(audit);
    protocols::check(audit);
}

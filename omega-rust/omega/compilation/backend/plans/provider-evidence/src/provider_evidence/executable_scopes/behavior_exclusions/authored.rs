//! Project Psi's authored restrictions into one Terminal composition's catalog.
//! Source admission is complete; this join neither reopens source nor infers
//! behavioral absence. Closure and mechanism checking consume the resulting set.

use crate::{BehaviorExclusion, BehaviorExclusions};
pub use language_semantics::declaration_selection::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind,
};
use terminal_psi::TerminalModule;

/// Resolve an authored exclusion in `module`. An absent service identity has
/// no boundary in that composition to restrict, so it resolves to `None`.
pub fn resolve_authored_behavior_exclusion(
    row: &AuthoredBehaviorExclusion,
    module: &TerminalModule,
) -> Option<BehaviorExclusion> {
    match &row.kind {
        AuthoredBehaviorExclusionKind::CrashCause { cause, .. } => {
            Some(BehaviorExclusion::CrashCause(*cause))
        }
        AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { class, .. } => {
            Some(BehaviorExclusion::PhysicalAuthorityClass(*class))
        }
        AuthoredBehaviorExclusionKind::Service { identity, .. } => module
            .services
            .iter()
            .find(|service| module.identities.text(service.identity) == Some(identity.as_str()))
            .map(|service| BehaviorExclusion::Service(service.id)),
    }
}

/// Resolve service identities against this module and form the canonical union.
/// Duplicate selections and authored order do not change its meaning.
pub fn authored_behavior_exclusion_set_in(
    exclusions: &[AuthoredBehaviorExclusion],
    module: &TerminalModule,
) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(
        exclusions
            .iter()
            .filter_map(|row| resolve_authored_behavior_exclusion(row, module)),
    )
}

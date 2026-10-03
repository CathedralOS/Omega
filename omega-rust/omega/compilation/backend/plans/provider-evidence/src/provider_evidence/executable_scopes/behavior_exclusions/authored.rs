//! Executed Build exclusions and their authored source provenance.
//!
//! Build execution selects exact declarations and retains the selecting machine
//! and source span. These rows resolve services against each Terminal module's
//! catalog and form the canonical exclusion union consumed by backend evidence
//! checking. Closure traversal and physical authority verdicts belong there,
//! not to the Build evaluator.

use crate::{BehaviorExclusion, BehaviorExclusions, TerminalAuthorityClass};
use symbols::SymbolHandle;
use terminal_psi::{CrashCause, TerminalModule};

/// One authored exclusion kind with its exact typed-stage declaration
/// identity. The terminal-facing [`BehaviorExclusion`] set derives from these
/// rows at the product-admission join, once selected service identities exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredBehaviorExclusionKind {
    /// `Build::exclude_crash(builder, CrashCause::X)`: the terminal crash cause plus
    /// the exact toolchain `CrashCause` case symbol the authored value named.
    CrashCause {
        cause: CrashCause,
        case_symbol: SymbolHandle,
    },
    /// Exact toolchain case selected by `Build::exclude_physical_authority`.
    PhysicalAuthorityClass {
        class: TerminalAuthorityClass,
        case_symbol: SymbolHandle,
    },
    /// `builder.exclude_service<Trait>()`: the exact boundary-trait symbol
    /// the authored type path resolved to. The Terminal service identity it
    /// names exists only per module, so the row resolves at the
    /// product-admission join against that module's service catalog.
    Service { trait_symbol: SymbolHandle },
}

/// One authored exclusion selection retained from the root build machine's
/// checked call scope. Rows keep their authored occurrence provenance;
/// repeated selections of the same kind union idempotently at the join, so
/// authored order cannot change the resulting admission requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredBehaviorExclusion {
    pub kind: AuthoredBehaviorExclusionKind,
    /// The typed machine that spelled the selection. A specialization
    /// template and its instances retain the same authored span under their
    /// own machine symbols.
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}

impl AuthoredBehaviorExclusion {
    /// The canonical exclusion this authored row selects in `module`.
    /// `trait_identity` maps a checked boundary-trait symbol to the identity
    /// Terminal service declarations carry. A service exclusion whose
    /// identity `module` never declares resolves to `None`: no boundary in
    /// that composition can invoke it, so it imposes no requirement there.
    pub fn resolve(
        &self,
        module: &TerminalModule,
        trait_identity: &dyn Fn(SymbolHandle) -> Option<String>,
    ) -> Option<BehaviorExclusion> {
        match self.kind {
            AuthoredBehaviorExclusionKind::CrashCause { cause, .. } => {
                Some(BehaviorExclusion::CrashCause(cause))
            }
            AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { class, .. } => {
                Some(BehaviorExclusion::PhysicalAuthorityClass(class))
            }
            AuthoredBehaviorExclusionKind::Service { trait_symbol } => {
                let identity = trait_identity(trait_symbol)?;
                module
                    .services
                    .iter()
                    .find(|service| service.identity == identity)
                    .map(|service| BehaviorExclusion::Service(service.id))
            }
        }
    }
}

/// The canonical crash-cause and physical-class union authored rows select. Service
/// rows need a module to resolve their identity and are not part of this
/// set; product admission uses [`authored_behavior_exclusion_set_in`].
pub fn authored_behavior_exclusion_set(
    exclusions: &[AuthoredBehaviorExclusion],
) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(exclusions.iter().filter_map(|row| match row.kind {
        AuthoredBehaviorExclusionKind::CrashCause { cause, .. } => {
            Some(BehaviorExclusion::CrashCause(cause))
        }
        AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { class, .. } => {
            Some(BehaviorExclusion::PhysicalAuthorityClass(class))
        }
        AuthoredBehaviorExclusionKind::Service { .. } => None,
    }))
}

/// The canonical union a set of authored rows selects in `module`, service
/// rows resolved against that module's service catalog. Duplicates and
/// authored order collapse under the same deduplication the terminal-side
/// checker consumes.
pub fn authored_behavior_exclusion_set_in(
    exclusions: &[AuthoredBehaviorExclusion],
    module: &TerminalModule,
    trait_identity: &dyn Fn(SymbolHandle) -> Option<String>,
) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(
        exclusions
            .iter()
            .filter_map(|row| row.resolve(module, trait_identity)),
    )
}

#[cfg(test)]
mod tests;

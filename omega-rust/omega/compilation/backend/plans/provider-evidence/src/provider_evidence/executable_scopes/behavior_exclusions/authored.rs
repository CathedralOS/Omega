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

/// One authored exclusion kind with its selected semantic identity.
/// The terminal-facing [`BehaviorExclusion`] set derives from these
/// rows at the product-admission join, once selected service identities exist.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// `Build::exclude_service<Trait>(builder)`: the selected boundary trait's
    /// hermetic package/module declaration identity, retained while Build owns
    /// its typed inputs. It is the same identity Terminal service catalogs
    /// publish, so only the module-local ServiceId remains to be resolved
    /// after Terminal. `name` is the declaration's diagnostic label, never a
    /// join key: distinct modules may each declare `Sink`.
    Service { identity: String, name: String },
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
    /// The selection as its author spelled it, for diagnostics that would
    /// otherwise print a service's hermetic identity. Crash and physical
    /// rows have no separate authored label.
    pub fn authored_label(&self) -> Option<String> {
        match &self.kind {
            AuthoredBehaviorExclusionKind::Service { name, .. } => {
                Some(format!("service `{name}`"))
            }
            AuthoredBehaviorExclusionKind::CrashCause { .. }
            | AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { .. } => None,
        }
    }

    /// The canonical exclusion this authored row selects in `module`.
    /// A service identity `module` never declares resolves to `None`: no boundary in
    /// that composition can invoke it, so it imposes no requirement there.
    pub fn resolve(&self, module: &TerminalModule) -> Option<BehaviorExclusion> {
        match &self.kind {
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
}

/// The canonical crash-cause and physical-class union authored rows select. Service
/// rows need a module to resolve their identity and are not part of this
/// set; product admission uses [`authored_behavior_exclusion_set_in`].
/// The canonical union a set of authored rows selects in `module`, service
/// rows resolved against that module's service catalog. Duplicates and
/// authored order collapse under the same deduplication the terminal-side
/// checker consumes.
pub fn authored_behavior_exclusion_set_in(
    exclusions: &[AuthoredBehaviorExclusion],
    module: &TerminalModule,
) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(exclusions.iter().filter_map(|row| row.resolve(module)))
}

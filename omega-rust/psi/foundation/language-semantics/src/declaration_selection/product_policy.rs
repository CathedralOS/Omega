//! Executed Build exclusions and their authored source provenance.
//!
//! Psi resolves source selections and retains the selecting machine and span.
//! Omega consumes these rows to resolve selected providers and to judge the
//! executable composition. A restriction record never proves behavioral absence.

use semantic_vocabulary::{CrashCause, TerminalAuthorityClass};
use symbols::SymbolHandle;

/// One executed root grant and its exact authored custody.
///
/// Selection strings remain transitional compiler input. The machine and span
/// prevent package review from rediscovering authority from a spelling after
/// the typed build program has been consumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredRootGrant {
    pub selector: String,
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}

/// One authored exclusion kind with its selected semantic identity.
/// The terminal-facing exclusion set derives from these
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
}

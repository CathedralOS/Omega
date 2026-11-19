//! Record the exact exclusions requested by a Build activation. Each request
//! keeps its selecting machine and source span for admission diagnostics.

use semantic_vocabulary::{CrashCause, TerminalAuthorityClass};
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredBehaviorExclusionKind {
    CrashCause {
        cause: CrashCause,
        case_symbol: SymbolHandle,
    },
    PhysicalAuthorityClass {
        class: TerminalAuthorityClass,
        case_symbol: SymbolHandle,
    },
    Service {
        identity: String,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredBehaviorExclusion {
    pub kind: AuthoredBehaviorExclusionKind,
    pub selecting_machine: SymbolHandle,
    pub source_span: source_text::SourceSpan,
}

use source_text::SourceSpan;

use semantic_vocabulary::{CrashCause, TerminalAuthorityClass};
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredRootGrant {
    /// Authored display text; settlement uses `subject`.
    pub selector: String,
    pub subject: RootGrantSubject,
    pub subject_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub selecting_machine: SymbolHandle,
    pub source_span: source_text::SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootGrantSubject {
    Provider {
        declaration: String,
        application: Option<String>,
    },
    BoundaryRequirement {
        declaration: String,
        callable: String,
    },
    AcceptedMachine {
        declaration: String,
        machine: SymbolHandle,
    },
}

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

pub use semantic_vocabulary::source_selections::{
    CompositionMode, NativeBindingProducer, ProductTarget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingContract {
    pub required: SymbolHandle,
    pub supplied: SymbolHandle,
    pub selecting_machine: SymbolHandle,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
    pub implementation_symbol: SymbolHandle,
    pub source_span: Option<SourceSpan>,
}

//! Preserve the declaring machine, package, source location and exact subject
//! of an authored root grant for the verifier that checks the selected binding.
use crate::SymbolHandle;
use source_text::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredRootGrant {
    /// Authored display text; settlement uses `subject`.
    pub selector: String,
    pub subject: RootGrantSubject,
    pub subject_package: Option<source_provenance::PackageKeyIdentity>,
    pub selecting_machine: SymbolHandle,
    pub source_span: SourceSpan,
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

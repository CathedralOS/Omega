//! Complete invocation-owned declarations consumed by realization trust policy.
//!
//! Psi selects eligible accepted declarations and records their checked evidence
//! before target choice. Missing or duplicate evidence stays data until that
//! exact declaration is granted; unrelated declarations cannot add obligations.
//! These records neither accept a grant nor form independently admitted evidence.

use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustDeclarationCardinality {
    Missing,
    Duplicate,
}

/// A universal template and a concrete contract have different commitment
/// domains. Keep their declarations distinct even when their bytes happen to
/// agree; realization preserves the corresponding trust-digest domain tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedTemplateDeclaration {
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedContractDeclaration {
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedMachineDeclaration {
    pub machine: SymbolHandle,
    /// Authored grant selection and diagnostic payload; `machine` retains the
    /// exact declaration identity independently of this spelling.
    pub name: String,
    pub template: Result<Option<AcceptedTemplateDeclaration>, TrustDeclarationCardinality>,
    pub contract: Result<AcceptedContractDeclaration, TrustDeclarationCardinality>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustDeclarations {
    pub machines: Box<[AcceptedMachineDeclaration]>,
}

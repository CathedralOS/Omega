//! Whole-provider choice and admission over prepared declaration records.
//!
//! Source adapters retain invocation-local correspondence; portable adapters
//! retain exact closed declarations. The policy compares those supplied identities
//! and complete applications, never resolves names or opens a source program.
//! Opaque owner data travels with a candidate or choice and is not selection
//! authority. No index escapes to be reinterpreted against another inventory.

mod declarations;
mod independent_components;
mod installation_reach;
mod provider_selection;
mod publication;
mod qualification_grants;
mod selection;

pub use publication::{
    CanonicalProviderDeclaration, PreparedProviderCandidate, PreparedProviderRealizations,
    PreparedTargetChoices, PreparedTargetProviderPublication,
};

pub use declarations::{
    CandidateDeclarations, ProviderCandidate, ProviderDeclaration, ProviderFamily,
    ProviderFamilyCoordinate, ProviderIdentity, ProviderSubject, SelectedProvider,
    SelectionProvenance,
};
pub use installation_reach::{InstallationReachRow, resolve_installation_reach};
pub use language_semantics::declaration_selection::CompositionMode;
pub use provider_selection::{admit, choose};
pub use qualification_grants::validate_qualification_grants;

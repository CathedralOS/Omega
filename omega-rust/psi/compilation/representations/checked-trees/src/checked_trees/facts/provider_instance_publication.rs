//! Frozen provider publication decisions, independent of executable retention.
//!
//! Checking resolves original applications and records their callable identities
//! once, as handles. Lowering selects retained interfaces, renders each handle and
//! interns the text; it does not reconstruct a requirement/body or receiving-region
//! association.

use arena::{Arena, Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::type_identity::CallableIdentityId;

/// One admitted body/requirement relation. The public provider declaration may
/// differ from the machine that owns its explicit satisfaction clause.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderCandidate {
    pub boundary: SymbolHandle,
    pub candidate: SymbolHandle,
    pub declaration: Result<CheckedProviderCandidateDeclaration, String>,
}

impl Default for CheckedProviderCandidate {
    fn default() -> Self {
        Self {
            boundary: SymbolHandle::default(),
            candidate: SymbolHandle::default(),
            declaration: Err("missing checked provider candidate".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderCandidateDeclaration {
    pub provider: SymbolHandle,
    pub receiving: Handle<CheckedProviderReceivingApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderInstanceRequirement {
    pub template: SymbolHandle,
    pub boundary: SymbolHandle,
    /// Zero denotes an implementation-independent requirement obligation.
    pub provider: SymbolHandle,
    pub declaration: Result<CheckedProviderInstanceDeclaration, String>,
}

impl Default for CheckedProviderInstanceRequirement {
    fn default() -> Self {
        Self {
            template: SymbolHandle::default(),
            boundary: SymbolHandle::default(),
            provider: SymbolHandle::default(),
            declaration: Err("missing checked provider instance declaration".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderInstanceDeclaration {
    pub template_identity: CallableIdentityId,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub selecting_declaration: SymbolHandle,
    /// Zero for a requirement-only obligation, which has no source candidate.
    pub source_candidate_identity: CallableIdentityId,
    pub requirement_identity: CallableIdentityId,
    /// Zero for a requirement-only obligation, which has no closed candidate.
    pub candidate_identity: CallableIdentityId,
    pub receiving: Handle<CheckedProviderReceivingApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderReceivingApplication {
    pub association: super::CheckedProviderClosedRequirement,
    pub description: Result<CheckedProviderReceivingDescription, String>,
}

impl Default for CheckedProviderReceivingApplication {
    fn default() -> Self {
        Self {
            association: Default::default(),
            description: Err("missing checked provider receiving publication".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderReceivingDescription {
    pub declaration_application: String,
    pub application: String,
    pub regions: HandleSpan<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderInstancePublication {
    pub candidates: Arena<CheckedProviderCandidate>,
    pub requirements: Arena<CheckedProviderInstanceRequirement>,
    pub receivers: Arena<CheckedProviderReceivingApplication>,
    pub regions: Arena<String>,
}

impl CheckedProviderInstancePublication {
    pub fn receiving(
        &self,
        association: &super::CheckedProviderClosedRequirement,
    ) -> Result<&CheckedProviderReceivingDescription, &str> {
        let mut matches = self
            .receivers
            .iter()
            .filter(|(_, row)| row.association == *association);
        let (_, row) = matches
            .next()
            .ok_or("provider lost its checked receiving publication")?;
        if matches.next().is_some() {
            return Err("provider has multiple checked receiving publications");
        }
        row.description.as_ref().map_err(String::as_str)
    }
}

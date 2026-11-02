//! Frozen provider publication decisions, independent of executable retention.
//!
//! Checking resolves original applications and normalizes their portable identity
//! payloads once. Lowering selects retained interfaces and interns these payloads;
//! it does not reconstruct a requirement/body or receiving-region association.

use arena::{Arena, Handle, HandleSpan};
use symbols::SymbolHandle;

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
    pub template_identity: String,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub selecting_declaration: SymbolHandle,
    pub source_candidate_identity: String,
    pub requirement_identity: String,
    pub candidate_identity: String,
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
    pub application: String,
    pub regions: HandleSpan<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedProviderInstancePublication {
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

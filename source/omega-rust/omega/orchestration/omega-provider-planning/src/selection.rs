use psi_symbols::SymbolHandle;

/// One exact declaration identity participating in a provider selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelectionIdentity {
    pub symbol: SymbolHandle,
    pub package: Option<psi_core::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
}

/// Build-selected provider realization for one exact boundary service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub boundary_trait: ProviderSelectionIdentity,
    pub provider_type: ProviderSelectionIdentity,
    pub selecting_machine: SymbolHandle,
    pub source_span: psi_source::SourceSpan,
}

#[cfg(test)]
impl ProviderSelection {
    pub fn exact_for_test(boundary_trait: &str, provider_type: &str) -> Self {
        Self {
            boundary_trait: ProviderSelectionIdentity {
                symbol: SymbolHandle::invalid(),
                package: None,
                canonical_path: boundary_trait.to_owned(),
                authored_path: boundary_trait.to_owned(),
            },
            provider_type: ProviderSelectionIdentity {
                symbol: SymbolHandle::invalid(),
                package: None,
                canonical_path: provider_type.to_owned(),
                authored_path: provider_type.to_owned(),
            },
            selecting_machine: SymbolHandle::invalid(),
            source_span: psi_source::SourceSpan::default(),
        }
    }
}

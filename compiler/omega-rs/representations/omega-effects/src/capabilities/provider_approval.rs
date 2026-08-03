//! Exact approval for boundary-capability providers.
//!
//! Service reach and authority are independent axes. A boundary capability's
//! symbol identifies the authority surface being exercised; provider approval
//! decides whether that exact surface is supplied by an admitted external edge
//! or illicitly minted by an ordinary in-package implementation. No service
//! spelling, compatibility effect bit, or unrelated provider participates.

use psi_symbols::SymbolHandle;

/// Approval state for one boundary capability's provider edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryProviderApproval {
    pub trait_symbol: SymbolHandle,
    pub approved: bool,
}

impl BoundaryProviderApproval {
    pub const fn new(trait_symbol: SymbolHandle, approved: bool) -> Self {
        Self {
            trait_symbol,
            approved,
        }
    }
}

/// Exact, symbol-keyed approval registry for boundary capability providers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryProviderApprovalRegistry {
    providers: Vec<BoundaryProviderApproval>,
}

impl BoundaryProviderApprovalRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_providers(providers: Vec<BoundaryProviderApproval>) -> Self {
        Self { providers }
    }

    pub fn register(&mut self, provider: BoundaryProviderApproval) {
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|candidate| candidate.trait_symbol == provider.trait_symbol)
        {
            existing.approved |= provider.approved;
            return;
        }
        self.providers.push(provider);
    }

    pub fn providers(&self) -> &[BoundaryProviderApproval] {
        &self.providers
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn provider(&self, trait_symbol: SymbolHandle) -> Option<&BoundaryProviderApproval> {
        self.providers
            .iter()
            .find(|provider| provider.trait_symbol == trait_symbol)
    }

    /// Authorize only the exact boundary capability reached by the call.
    pub fn authorize_boundary_call(&self, provider_trait: SymbolHandle) -> BoundaryCallApproval {
        if self
            .provider(provider_trait)
            .is_some_and(|provider| provider.approved)
        {
            BoundaryCallApproval::Approved
        } else {
            BoundaryCallApproval::Unapproved
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryCallApproval {
    Approved,
    Unapproved,
}

impl BoundaryCallApproval {
    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_provider_authorizes_its_exact_boundary_capability() {
        let provider_symbol = SymbolHandle::from_arena_index(1);
        let mut registry = BoundaryProviderApprovalRegistry::new();
        registry.register(BoundaryProviderApproval::new(provider_symbol, true));

        let decision = registry.authorize_boundary_call(provider_symbol);
        assert_eq!(decision, BoundaryCallApproval::Approved);
        assert!(decision.is_approved());
    }

    #[test]
    fn unapproved_provider_is_rejected() {
        let provider_symbol = SymbolHandle::from_arena_index(2);
        let mut registry = BoundaryProviderApprovalRegistry::new();
        registry.register(BoundaryProviderApproval::new(provider_symbol, false));

        let decision = registry.authorize_boundary_call(provider_symbol);
        assert_eq!(decision, BoundaryCallApproval::Unapproved);
        assert!(!decision.is_approved());
    }

    #[test]
    fn unrelated_approved_provider_cannot_authorize_another_capability() {
        let approved = SymbolHandle::from_arena_index(3);
        let requested = SymbolHandle::from_arena_index(4);
        let registry =
            BoundaryProviderApprovalRegistry::with_providers(vec![BoundaryProviderApproval::new(
                approved, true,
            )]);

        assert_eq!(
            registry.authorize_boundary_call(requested),
            BoundaryCallApproval::Unapproved
        );
    }
}

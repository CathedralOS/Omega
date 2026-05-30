//! Host-call authorization over boundary providers.
//!
//! A boundary trait that mints or carries real-world authority is a *host
//! authority provider*. The build policy decides which providers a package is
//! allowed to reach: ordinary application code should not be able to mint new
//! host providers as a proof escape hatch (chapter 18, "Host Providers" /
//! "Boundary Primitive Registry").
//!
//! This module models that policy as a whitelist of authorized providers plus a
//! resolution gate (`authorize_host_call`) that decides whether a host call
//! requiring a given authority effect set is backed by an approved provider.
//! It is the host-authority companion to the operator-binding registry in
//! [`super::providers`].

use omega_core::symbols::SymbolHandle;

use crate::EffectSet;

/// Standard effects that mint or carry real-world *authority*, as opposed to
/// pure-compute effects (`alloc`, `dealloc`, thread/sync) that never reach the
/// host surface. A boundary call that declares any of these effects is treated
/// as a host call that must be authorized by a provider.
pub const HOST_AUTHORITY_EFFECT_NAMES: &[&str] = &[
    "stdin_io",
    "stdout_io",
    "stderr_io",
    "filesystem_io",
    "network_io",
    "process_spawn",
    "process_exit",
    "process_signal",
    "env_read",
    "env_write",
    "clock_read",
    "random_read",
    "device_io",
    "memory_map",
    "dynamic_link",
];

/// Returns the set of standard effects that mint or carry host authority.
pub fn host_authority_effects() -> EffectSet {
    let mut effects = EffectSet::empty();
    for name in HOST_AUTHORITY_EFFECT_NAMES {
        effects.insert_name(name);
    }
    effects
}

/// Returns the authority-bearing subset of `effects`.
pub fn authority_effects(effects: EffectSet) -> EffectSet {
    intersection(effects, host_authority_effects())
}

/// Returns true when `effects` reaches any host-authority effect.
pub fn requires_host_authority(effects: EffectSet) -> bool {
    effects.intersects(host_authority_effects())
}

/// A boundary trait that the package can reach, together with the authority it
/// provides. Equivalent to one row in the boundary primitive registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAuthorityProvider {
    /// Boundary trait symbol that names the provider.
    pub trait_symbol: SymbolHandle,
    /// The authority-bearing effects this provider mints or carries.
    pub effects: EffectSet,
    /// Whether the build policy whitelists this provider for the package.
    ///
    /// Toolchain/core providers and explicitly whitelisted boundary packages are
    /// approved; an ad-hoc provider declared by ordinary application code is not.
    pub whitelisted: bool,
}

impl HostAuthorityProvider {
    pub fn new(trait_symbol: SymbolHandle, effects: EffectSet, whitelisted: bool) -> Self {
        Self {
            trait_symbol,
            effects,
            whitelisted,
        }
    }

    /// Whether this provider can satisfy a host call requiring `requested`
    /// authority. A whitelisted provider authorizes any host call whose
    /// authority is covered by the authority it provides.
    pub fn authorizes(&self, requested: EffectSet) -> bool {
        self.whitelisted && self.effects.contains_all(authority_effects(requested))
    }
}

/// Whitelist of host authority providers a package is allowed to reach, used to
/// gate host calls. Resolution walks the registry and asks whether any approved
/// provider covers the requested authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostAuthorityRegistry {
    providers: Vec<HostAuthorityProvider>,
}

impl HostAuthorityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_providers(providers: Vec<HostAuthorityProvider>) -> Self {
        Self { providers }
    }

    /// Registers a provider, merging effects when the trait is already present.
    pub fn register(&mut self, provider: HostAuthorityProvider) {
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|candidate| candidate.trait_symbol == provider.trait_symbol)
        {
            existing.effects.insert_all(provider.effects);
            existing.whitelisted |= provider.whitelisted;
            return;
        }
        self.providers.push(provider);
    }

    pub fn providers(&self) -> &[HostAuthorityProvider] {
        &self.providers
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn provider(&self, trait_symbol: SymbolHandle) -> Option<&HostAuthorityProvider> {
        self.providers
            .iter()
            .find(|provider| provider.trait_symbol == trait_symbol)
    }

    /// The union of authority effects provided by whitelisted providers.
    pub fn authorized_authority(&self) -> EffectSet {
        let mut effects = EffectSet::empty();
        for provider in &self.providers {
            if provider.whitelisted {
                effects.insert_all(provider.effects);
            }
        }
        effects
    }

    /// Resolution gate: whether a host call through `provider_trait` requiring
    /// `requested` authority is authorized.
    ///
    /// The call is allowed when the named provider is whitelisted and its
    /// authority covers the request, or when any whitelisted provider in the
    /// registry covers the requested authority (a transitive provider edge).
    pub fn authorize_host_call(
        &self,
        provider_trait: SymbolHandle,
        requested: EffectSet,
    ) -> HostCallAuthorization {
        let requested_authority = authority_effects(requested);
        if requested_authority.is_empty() {
            return HostCallAuthorization::NotAHostCall;
        }

        if let Some(provider) = self.provider(provider_trait) {
            if provider.authorizes(requested) {
                return HostCallAuthorization::Authorized;
            }
            if !provider.whitelisted {
                return HostCallAuthorization::Unapproved {
                    missing: requested_authority,
                };
            }
        }

        if self
            .authorized_authority()
            .contains_all(requested_authority)
        {
            return HostCallAuthorization::Authorized;
        }

        HostCallAuthorization::Unapproved {
            missing: requested_authority.difference(self.authorized_authority()),
        }
    }
}

/// Outcome of resolving a host call against the provider registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCallAuthorization {
    /// The call requests no host authority; nothing to authorize.
    NotAHostCall,
    /// An approved provider covers the requested authority.
    Authorized,
    /// No whitelisted provider covers the requested authority.
    Unapproved { missing: EffectSet },
}

impl HostCallAuthorization {
    pub fn is_authorized(self) -> bool {
        matches!(self, Self::Authorized | Self::NotAHostCall)
    }
}

fn intersection(left: EffectSet, right: EffectSet) -> EffectSet {
    // EffectSet exposes difference but not intersection; `a - (a - b) = a & b`.
    left.difference(left.difference(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effects(names: &[&str]) -> EffectSet {
        let mut set = EffectSet::empty();
        for name in names {
            assert!(set.insert_name(name), "unknown effect {name}");
        }
        set
    }

    #[test]
    fn authority_effects_exclude_pure_compute() {
        let mixed = effects(&["alloc", "filesystem_io", "thread_spawn"]);
        let authority = authority_effects(mixed);
        assert_eq!(authority.names().collect::<Vec<_>>(), ["filesystem_io"]);
        assert!(requires_host_authority(mixed));
        assert!(!requires_host_authority(effects(&["alloc", "thread_spawn"])));
    }

    #[test]
    fn whitelisted_provider_authorizes_covered_host_call() {
        let provider_symbol = SymbolHandle::from_arena_index(1);
        let mut registry = HostAuthorityRegistry::new();
        registry.register(HostAuthorityProvider::new(
            provider_symbol,
            effects(&["filesystem_io"]),
            true,
        ));

        let decision = registry.authorize_host_call(provider_symbol, effects(&["filesystem_io"]));
        assert_eq!(decision, HostCallAuthorization::Authorized);
        assert!(decision.is_authorized());
    }

    #[test]
    fn unwhitelisted_provider_is_unapproved() {
        let provider_symbol = SymbolHandle::from_arena_index(2);
        let mut registry = HostAuthorityRegistry::new();
        registry.register(HostAuthorityProvider::new(
            provider_symbol,
            effects(&["filesystem_io"]),
            false,
        ));

        let decision = registry.authorize_host_call(provider_symbol, effects(&["filesystem_io"]));
        assert!(!decision.is_authorized());
        match decision {
            HostCallAuthorization::Unapproved { missing } => {
                assert_eq!(missing.names().collect::<Vec<_>>(), ["filesystem_io"]);
            }
            other => panic!("expected unapproved, got {other:?}"),
        }
    }

    #[test]
    fn pure_compute_call_is_not_a_host_call() {
        let provider_symbol = SymbolHandle::from_arena_index(3);
        let registry = HostAuthorityRegistry::new();
        assert_eq!(
            registry.authorize_host_call(provider_symbol, effects(&["alloc"])),
            HostCallAuthorization::NotAHostCall
        );
    }
}

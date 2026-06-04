//! Boundary primitive provider registry (frozen Wave 0 decision #4).
//!
//! A [`BoundaryProvider`] is declared by a `provider <QualifiedName> :
//! <Category>;` item. Core primitives bind a named provider via the `provider`
//! clause on a boundary `operator`. Only whitelisted packages may declare
//! providers; every boundary binding must resolve to a registered provider.

use omega_core::diagnostics::Diagnostic;
use omega_core::operator_spelling::ProviderCategory;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    BoundaryLevel, CapabilityContract, CapabilityContractKind, Item, OperatorDefinition,
};

use crate::EffectSet;
use crate::capabilities::host_authority::host_authority_effects;

/// Package path prefixes that are permitted to declare boundary providers.
///
/// Aligned with the bundled-omega-path / `allows_boundary_policy` host registry
/// logic: only the language core, host packages, and the toolchain may register
/// providers.
pub const PROVIDER_WHITELIST_PREFIXES: &[&[&str]] = &[
    &["omega", "language", "core"],
    &["omega", "host"],
    &["toolchain"],
];

/// A registered boundary primitive provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryProvider {
    pub name: String,
    pub category: ProviderCategory,
    /// Reference to the contract that governs this provider, if any. Sourced
    /// from the bound boundary operator's `requires` clause.
    pub contract_ref: Option<String>,
    pub effect_set: EffectSet,
    /// Which targets this provider applies to; empty means all targets.
    pub target_applicability: Vec<String>,
    /// The package path that declared this provider.
    pub origin_package: String,
}

/// The set of registered providers for a program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundaryProviderRegistry {
    providers: Vec<BoundaryProvider>,
}

impl BoundaryProviderRegistry {
    pub fn providers(&self) -> &[BoundaryProvider] {
        &self.providers
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Look up a registered provider by its fully-qualified name.
    pub fn resolve(&self, name: &str) -> Option<&BoundaryProvider> {
        self.providers.iter().find(|provider| provider.name == name)
    }
}

/// Builds the provider registry from all `provider` declarations in the program,
/// emitting a diagnostic for any provider declared by a non-whitelisted package.
pub fn build_provider_registry(
    syntax: &SyntaxTrees,
    diagnostics: &mut Vec<Diagnostic>,
) -> BoundaryProviderRegistry {
    let mut providers = Vec::new();

    for item in syntax.root_items() {
        let Item::Provider(declaration) = item else {
            continue;
        };

        let members = syntax.items.identifier_path_members(declaration.name);
        let name = join_path(members);
        let origin_package = package_prefix(members);

        if !is_whitelisted_provider_path(members) {
            diagnostics.push(Diagnostic::error(format!(
                "boundary provider `{name}` may only be declared by a whitelisted package \
                 (omega::language::core, omega::host::**, or toolchain)"
            )));
            continue;
        }

        if providers
            .iter()
            .any(|existing: &BoundaryProvider| existing.name == name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate boundary provider declaration `{name}`"
            )));
            continue;
        }

        providers.push(BoundaryProvider {
            name,
            category: declaration.category,
            contract_ref: None,
            effect_set: EffectSet::empty(),
            target_applicability: Vec::new(),
            origin_package,
        });
    }

    populate_provider_metadata(syntax, &mut providers);

    BoundaryProviderRegistry { providers }
}

/// Fills `contract_ref` / `effect_set` / `target_applicability` on each
/// registered provider from the boundary operator(s) bound to it. A provider is
/// otherwise just a declared row; the operator that binds it via its `provider`
/// clause is what carries the governing contract, the declared effects, and the
/// targets it applies to. Multiple operators may bind the same provider, so we
/// merge their contributions.
fn populate_provider_metadata(syntax: &SyntaxTrees, providers: &mut [BoundaryProvider]) {
    for item in syntax.root_items() {
        match item {
            Item::Operator(operator) => {
                apply_operator_to_providers(syntax, operator, providers);
            }
            Item::Domain(domain) => {
                for operator in syntax.items.operators(domain.operators) {
                    apply_operator_to_providers(syntax, operator, providers);
                }
            }
            _ => {}
        }
    }
}

fn apply_operator_to_providers(
    syntax: &SyntaxTrees,
    operator: &OperatorDefinition,
    providers: &mut [BoundaryProvider],
) {
    let Some(provider_path) = operator.provider else {
        return;
    };

    let provider_name = join_path(syntax.items.identifier_path_members(provider_path));
    let Some(provider) = providers
        .iter_mut()
        .find(|provider| provider.name == provider_name)
    else {
        return;
    };

    let contracts = syntax.items.capability_contracts(operator.contracts);

    // `contract_ref`: the bound operator's contract obligation. The operator's
    // `requires`/`ensures` clause IS the governing contract, referenced by the
    // operator's qualified name. Falls back to a declared boundary level.
    if provider.contract_ref.is_none() {
        if let Some(reference) = operator_contract_ref(syntax, operator, contracts) {
            provider.contract_ref = Some(reference);
        }
    }

    // `effect_set`: the authority the bound operator carries. Operators declare
    // no `effects` clause; their authority is implied by the provider category,
    // so a host-ABI provider mints host authority while pure-compute primitives
    // (slice indexing, pointer math, allocation) carry none.
    provider
        .effect_set
        .insert_all(category_effects(provider.category));

    // `target_applicability`: the named boundary levels the operator's contracts
    // scope it to. An operator with no named boundary applies to all targets
    // (empty list).
    for level_name in boundary_target_names(contracts) {
        if !provider.target_applicability.contains(&level_name) {
            provider.target_applicability.push(level_name);
        }
    }
}

/// The contract reference governing `operator`: its qualified name when it
/// declares a `requires`/`ensures` contract, else the named boundary level it
/// declares, if any.
fn operator_contract_ref(
    syntax: &SyntaxTrees,
    operator: &OperatorDefinition,
    contracts: &[CapabilityContract],
) -> Option<String> {
    let has_proof_contract = contracts.iter().any(|contract| {
        matches!(
            contract.kind,
            CapabilityContractKind::Requires | CapabilityContractKind::Ensures
        )
    });

    if has_proof_contract {
        return Some(join_path(
            syntax.items.identifier_path_members(operator.name),
        ));
    }

    boundary_target_names(contracts).into_iter().next()
}

/// The named boundary levels declared by `contracts` (`boundary <name>`),
/// in declaration order.
fn boundary_target_names(contracts: &[CapabilityContract]) -> Vec<String> {
    let mut names = Vec::new();
    for contract in contracts {
        let CapabilityContractKind::Boundary(level) = &contract.kind else {
            continue;
        };
        let name = match level {
            BoundaryLevel::Host => "host".to_owned(),
            BoundaryLevel::Named(identifier) => identifier.as_str().to_owned(),
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The authority effects a provider of `category` carries. Only host-ABI calls
/// reach the host surface; the remaining categories are pure-compute primitives.
fn category_effects(category: ProviderCategory) -> EffectSet {
    match category {
        ProviderCategory::HostAbiCall => host_authority_effects(),
        ProviderCategory::SliceIndexing
        | ProviderCategory::PointerOffset
        | ProviderCategory::PointerAccess
        | ProviderCategory::DescriptorConstruction
        | ProviderCategory::Allocation => EffectSet::empty(),
    }
}

/// Validates that every boundary operator `provider` binding resolves to a
/// provider registered in the registry. Unregistered names are rejected.
pub fn validate_provider_bindings(
    syntax: &SyntaxTrees,
    registry: &BoundaryProviderRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in syntax.root_items() {
        match item {
            Item::Operator(operator) => {
                validate_operator_binding(syntax, registry, operator, diagnostics);
            }
            Item::Domain(domain) => {
                for operator in syntax.items.operators(domain.operators) {
                    validate_operator_binding(syntax, registry, operator, diagnostics);
                }
            }
            _ => {}
        }
    }
}

fn validate_operator_binding(
    syntax: &SyntaxTrees,
    registry: &BoundaryProviderRegistry,
    operator: &omega_syntax_trees::item::OperatorDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(provider) = operator.provider else {
        return;
    };

    let provider_name = join_path(syntax.items.identifier_path_members(provider));
    if registry.resolve(&provider_name).is_none() {
        let operator_name = join_path(syntax.items.identifier_path_members(operator.name));
        diagnostics.push(Diagnostic::error(format!(
            "boundary operator `{operator_name}` binds unregistered provider `{provider_name}`"
        )));
    }
}

fn is_whitelisted_provider_path(members: &[Identifier]) -> bool {
    PROVIDER_WHITELIST_PREFIXES.iter().any(|prefix| {
        members.len() >= prefix.len()
            && members
                .iter()
                .zip(prefix.iter())
                .all(|(member, expected)| member.as_str() == *expected)
    })
}

fn package_prefix(members: &[Identifier]) -> String {
    // Drop the final segment (the provider's own name) to leave the package.
    let package_len = members.len().saturating_sub(1);
    join_path(&members[..package_len])
}

fn join_path(members: &[Identifier]) -> String {
    members
        .iter()
        .map(Identifier::as_str)
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::operator_spelling::ProviderCategory;
    use omega_source_files_to_tokens::Lexer;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    fn parse(source: &str) -> SyntaxTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        parse_syntax_trees(&tokens).expect("parse")
    }

    #[test]
    fn host_abi_provider_metadata_comes_from_bound_operator() {
        let syntax = parse(
            r#"
            provider omega::host::WriteBytes : HostAbiCall;

            boundary operator omega::host::write(handle: i64, length: usize) -> usize
            provider omega::host::WriteBytes
            requires
                length > 0;
            "#,
        );

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::host::WriteBytes")
            .expect("provider should be registered");
        assert_eq!(provider.category, ProviderCategory::HostAbiCall);

        // contract_ref: sourced from the bound operator's `requires` clause,
        // referenced by the operator's qualified name.
        assert_eq!(provider.contract_ref.as_deref(), Some("omega::host::write"));

        // effect_set: a host-ABI provider carries host authority.
        assert!(
            !provider.effect_set.is_empty(),
            "host-ABI provider should carry host authority effects"
        );
        assert_eq!(
            provider.effect_set,
            host_authority_effects(),
            "host-ABI provider should carry the full host authority set"
        );
    }

    #[test]
    fn compute_provider_carries_no_authority() {
        let syntax = parse(
            r#"
            provider omega::language::core::Slice : SliceIndexing;

            boundary operator omega::language::core::index<T>(items: &[T], index: usize) -> T
            spelling []
            provider omega::language::core::Slice
            requires
                index < items.len;
            "#,
        );

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::language::core::Slice")
            .expect("provider should be registered");
        assert_eq!(provider.category, ProviderCategory::SliceIndexing);
        assert_eq!(
            provider.contract_ref.as_deref(),
            Some("omega::language::core::index")
        );
        // Pure-compute primitive: no host authority.
        assert!(
            provider.effect_set.is_empty(),
            "slice indexing provider should not carry host authority"
        );
    }

    #[test]
    fn unbound_provider_keeps_empty_metadata() {
        let syntax = parse("provider omega::host::Unbound : HostAbiCall;");

        let mut diagnostics = Vec::new();
        let registry = build_provider_registry(&syntax, &mut diagnostics);
        assert!(
            diagnostics.is_empty(),
            "unexpected diagnostics: {diagnostics:?}"
        );

        let provider = registry
            .resolve("omega::host::Unbound")
            .expect("provider should be registered");
        // No operator binds it, so metadata stays at empty defaults.
        assert_eq!(provider.contract_ref, None);
        assert!(provider.effect_set.is_empty());
        assert!(provider.target_applicability.is_empty());
    }
}

//! Boundary primitive provider registry (frozen Wave 0 decision #4).
//!
//! A [`BoundaryProvider`] is declared by a `provider <QualifiedName> :
//! <Category>;` item. Core primitives bind a named provider via the `provider`
//! clause on a boundary `operator`. Only whitelisted packages may declare
//! providers; every boundary binding must resolve to a registered provider.

use omega_core::diagnostics::Diagnostic;
use omega_core::operator_spelling::ProviderCategory;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::Item;
use omega_syntax_trees::identifier::Identifier;

use crate::EffectSet;

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
        self.providers
            .iter()
            .find(|provider| provider.name == name)
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

    BoundaryProviderRegistry { providers }
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

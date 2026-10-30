//! Project checked instance associations, without selecting a provider plan.
//!
//! Source coordinates remain here. The returned declaration records retain
//! candidate groups and deferred failures so Omega owns counts and coverage.

use crate::CheckFacts;
use language_semantics::provider_declarations::{
    ClosedProviderInstance, ProviderInstanceApplication,
};
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

pub fn inherited_instances(
    typed: &TypedTrees,
    facts: &CheckFacts,
    requirement: &str,
    provider: &Machine,
    selecting_declaration: Option<SymbolHandle>,
    package: Option<PackageKeyIdentity>,
) -> Vec<ProviderInstanceApplication> {
    typed.closed_requirement_applications().filter(|application| {
        typed.trait_machine_signature(application.template).is_some_and(|(owner, signature)| {
            typed.trait_requirement_callable_identity_text(owner, signature) == requirement
        })
    }).map(|application| {
        // Named selection owns its applications; a direct satisfaction is
        // owned by the concrete instance, not the selected template. Preserve
        // the checked receiving application's complete region substitution.
        let candidates = facts.provider_closed_requirements.iter().filter(|fact| {
            fact.template == application.template
                && fact.closed_requirement == application.signature.symbol
                && fact.receiving_application == application.trait_application_identity
                && fact.selecting_declaration == selecting_declaration.unwrap_or(fact.provider)
                && typed.requirement_realization_matches(
                    fact.closed_requirement, provider.symbol, fact.provider,
                )
        }).map(|fact| {
            let instance = typed.machine(fact.provider)
                .ok_or("closed provider fact lost its realization body")?;
            if !typed.machine_type_parameters(instance).is_empty()
                || typed.symbols.symbol_package_identity(instance.symbol) != package
            {
                return Err("closed provider realization has an open telescope or a different package".into());
            }
            let owner = typed.trait_definition_by_symbol(application.declaring_trait)
                .ok_or("closed requirement fact lost its declaring trait")?;
            let requirement = typed.trait_requirement_callable_identity_text(owner, application.signature);
            let realization = typed.machine_callable_identity_text(instance).unwrap_or_default();
            if requirement.is_empty() || realization.is_empty() {
                return Err("closed provider coordinate has no normalized callable identity".into());
            }
            Ok(ClosedProviderInstance { requirement, realization })
        }).collect();
        ProviderInstanceApplication { candidates }
    }).collect()
}

pub fn generic_instances(
    typed: &TypedTrees,
    facts: &CheckFacts,
    provider: &Machine,
    template: &Machine,
    package: Option<PackageKeyIdentity>,
) -> Vec<Result<ClosedProviderInstance, String>> {
    typed.machine_specializations.iter().filter(|receipt| {
        receipt.template == provider.symbol && receipt.operator_realizations.iter()
            .any(|application| application.requirement_symbol == template.symbol)
    }).map(|receipt| {
        let instance = typed.machine(receipt.instance).ok_or_else(|| format!(
            "selected provider `{}` specialization retains no typed instance", provider.name,
        ))?;
        if !typed.machine_type_parameters(instance).is_empty() {
            return Err(format!(
                "selected provider specialization instance `{}` remains generic", instance.name,
            ));
        }
        if typed.symbols.symbol_package_identity(instance.symbol) != package {
            return Err(format!(
                "selected provider instance `{}` drifts from its template's exact package identity", instance.name,
            ));
        }
        let closed = satisfied_requirement(typed, facts, instance, template).ok_or_else(|| format!(
            "selected provider instance `{}` realizes requirement template `{}` without one exact closed requirement",
            instance.name, template.name,
        ))?;
        if closed.symbol == template.symbol {
            return Err(format!(
                "selected provider instance `{}` replays requirement template `{}` onto itself", instance.name, template.name,
            ));
        }
        let requirement = typed.machine_callable_identity_text(closed).unwrap_or_default();
        let realization = typed.machine_callable_identity_text(instance).unwrap_or_default();
        if requirement.is_empty() || realization.is_empty() {
            return Err(format!(
                "selected provider instance `{}` or closed requirement `{}` has no normalized identity", instance.name, closed.name,
            ));
        }
        Ok(ClosedProviderInstance { requirement, realization })
    }).collect()
}

pub fn closed_requirements(typed: &TypedTrees, template: &Machine) -> Vec<String> {
    typed
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.template == template.symbol)
        .filter_map(|receipt| typed.machine(receipt.instance))
        .filter(|instance| {
            instance.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
        .filter_map(|instance| typed.machine_callable_identity_text(instance))
        .collect()
}

/// Checking already resolved the exact template-to-closed satisfaction edge.
/// A template never aliases the closed requirement it introduced.
fn satisfied_requirement<'typed>(
    typed: &'typed TypedTrees,
    facts: &CheckFacts,
    provider: &Machine,
    requirement: &'typed Machine,
) -> Option<&'typed Machine> {
    if typed.machine_type_parameters(requirement).is_empty() {
        return Some(requirement);
    }
    let mut applications = facts.provider_closed_requirements.iter().filter(|row| {
        row.provider == provider.symbol
            && row.selecting_declaration == provider.symbol
            && row.template == requirement.symbol
    });
    let application = applications.next()?;
    if applications.next().is_some() {
        return None;
    }
    typed.machine(application.closed_requirement)
}

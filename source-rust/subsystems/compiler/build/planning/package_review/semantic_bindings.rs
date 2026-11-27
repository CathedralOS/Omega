//! Derive host service roles from the checked Product and its selected providers.

use super::input::PolicySource;
use diagnostics::Diagnostic;
use build_providers::acceptance::AcceptedSemanticBinding;
use build_providers::plans::ServiceSchema;
use review_evidence::record::SemanticBindingReviewCandidate;
use source_compiler::representations::source_state::trait_definition::TraitDefinition;

use super::declarations::unavailable;
use build_providers::acceptance::AcceptedSemanticBindingRole as Role;
use review_evidence::record::{PackagePolicyBaseline, PackagePolicyCallableRole};
use std::collections::BTreeSet;

/// A Build may import the same boundary to select its provider. Only the
/// Product occurrence can supply the declaration a Product role denotes.
fn product_boundaries<'a>(input: &PolicySource<'a>) -> impl Iterator<Item = &'a TraitDefinition> {
    let source = input.typed();
    source.traits().iter().filter(move |slot| {
        slot.is_boundary
            && source.symbols.symbol_is_in_dependency_scope(
                slot.symbol,
                source_provenance::DependencyScope::Product,
            )
    })
}

/// Retain the declaration that actually supplies a proposed role. ProcessExit
/// belongs to core, while the reviewed binding names its selected provider.
pub(super) fn checked_service(
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
    binding: &AcceptedSemanticBinding,
) -> Result<(symbols::SymbolHandle, ServiceSchema), Vec<Diagnostic>> {
    let source = input.typed();
    let path = binding.declaration_path();
    let core = binding.role() == Role::ProcessExitExitProcessI32;
    let selected_plan = binding
        .selected_provider_plan_digest()
        .map(|digest| {
            let mut plans = selected.plans().plans().iter().filter(|plan| {
                plan.identity_digest() == digest
                    && plan.provider_type_package_identity == Some(binding.package())
                    && plan.origin_package_identity == Some(binding.package())
            });
            match (plans.next(), plans.next()) {
                (Some(plan), None) => Ok(plan),
                _ => Err(unavailable(
                    "the uniquely selected provider of a semantic binding",
                )),
            }
        })
        .transpose()?;
    let mut slots = product_boundaries(input).filter(|d| {
        source.symbols.display_path(d.symbol, "::") == path
            && if core {
                source.core_vocabulary(d.symbol)
                    == Some(language_core::core_vocabulary::CoreVocabulary::ProcessExitService)
            } else {
                source.symbols.symbol_package_identity(d.symbol) == Some(binding.package())
            }
    });
    let (Some(slot), None) = (slots.next(), slots.next()) else {
        return Err(unavailable("the uniquely owned semantic role declaration"));
    };
    input.nominal(slot.symbol)?;
    let declaration = ServiceSchema::from_declaration(
        input
            .accepted()
            .slot_schema(slot.symbol)
            .ok_or_else(|| unavailable("the complete checked role declaration schema"))?,
    );
    let schema = if let Some(plan) = selected_plan {
        if plan.schema.declaration_identity_digest() != declaration.declaration_identity_digest() {
            return Err(unavailable(
                "a selected schema differing from the owned declaration",
            ));
        }
        plan.schema.clone()
    } else {
        declaration
    };
    if schema.identity_digest() != binding.normalized_schema_digest() {
        return Err(unavailable("a mismatched semantic role schema digest"));
    }
    Ok((slot.symbol, schema))
}

/// Review exact package-owned service references of the consumer's callable
/// contracts and its Build-selected physical entry. The supplier itself gets no
/// authority merely for declaring a service with one of the discovery names.
fn host_services(
    source_services: &[super::services::ReviewedService],
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
    policy: &PackagePolicyBaseline,
    target: target::TargetProfile,
) -> Result<Vec<SemanticBindingReviewCandidate>, Vec<Diagnostic>> {
    let mut services: BTreeSet<_> = policy
        .callables
        .callables
        .iter()
        .filter(|row| {
            matches!(
                row.role,
                PackagePolicyCallableRole::Build
                    | PackagePolicyCallableRole::Public
                    | PackagePolicyCallableRole::Boundary
            )
        })
        .flat_map(super::callables::referenced_services)
        .cloned()
        .collect();
    let module = input.accepted().module();
    if module
        .interfaces
        .program_entry_contracts
        .iter()
        .any(|entry| entry.target == target.target_name())
    {
        let entry = module
            .program_entry_contract(target.target_name())
            .map_err(|e| unavailable(&e))?;
        let machine = module
            .machine(entry.entry)
            .ok_or_else(|| unavailable("the checked physical entry machine"))?;
        if let Some(reach) = &machine.contract.declared_service_reach {
            services.extend(super::services::reviewed_services(source_services, reach)?);
        }
    }
    let mut rows = Vec::new();
    for (path, role) in [
        ("FilesystemHost", Role::FilesystemHostService),
        ("TimeHost", Role::TimeHostService),
    ] {
        // The discovery name belongs to the role. Only a reference to the
        // exact package and declaring file nominates a Product declaration.
        let source = input.typed();
        let mut referenced = None;
        let mut product = None;
        for slot in source.traits().iter().filter(|slot| {
            slot.is_boundary && source.symbols.display_path(slot.symbol, "::") == path
        }) {
            let Some(owner) = source.symbols.symbol_package_identity(slot.symbol) else {
                continue;
            };
            let Some(service) = services.get(&input.nominal(slot.symbol)?.into()) else {
                continue;
            };
            if referenced
                .replace(service)
                .is_some_and(|prior| prior != service)
            {
                return Err(unavailable("an ambiguous consumer service role"));
            }
            if source.symbols.symbol_is_in_dependency_scope(
                slot.symbol,
                source_provenance::DependencyScope::Product,
            ) && product.replace((owner, slot)).is_some()
            {
                return Err(unavailable("the consumer's referenced service declaration"));
            }
        }
        if referenced.is_none() {
            continue;
        }
        let (owner, slot) =
            product.ok_or_else(|| unavailable("the consumer's referenced service declaration"))?;
        let schema = ServiceSchema::from_declaration(
            input
                .accepted()
                .slot_schema(slot.symbol)
                .ok_or_else(|| unavailable("the consumer's referenced service schema"))?,
        );
        let binding =
            AcceptedSemanticBinding::new_service(role, owner, path, schema.identity_digest())
                .map_err(unavailable)?;
        let (_, schema) = checked_service(input, selected, &binding)?;
        rows.push(SemanticBindingReviewCandidate::new(binding, schema));
    }
    Ok(rows)
}

pub(super) fn candidates(
    source_services: &[super::services::ReviewedService],
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
    policy: &PackagePolicyBaseline,
    target: target::TargetProfile,
) -> Result<Vec<SemanticBindingReviewCandidate>, Vec<Diagnostic>> {
    let mut rows = host_services(source_services, input, selected, policy, target)?;
    rows.extend(super::hosted_services::nominations(input, selected)?);
    // Consent can constrain an active role. Only a new nomination produces
    // a candidate; replaying an old binding must not nominate its supplier.
    for candidate in &rows {
        let fresh = candidate.binding();
        if let Some(previous) = input.sources().accepted_semantic_binding(fresh.role())
            && (fresh.package() != previous.package()
                || fresh.declaration_path() != previous.declaration_path()
                || fresh.normalized_schema_digest() != previous.normalized_schema_digest()
                || fresh.selected_provider_plan_digest()
                    != previous.selected_provider_plan_digest())
        {
            return Err(unavailable(
                "a fresh nomination conflicting with its accepted supplier",
            ));
        }
    }
    rows.sort_by_key(|row| row.binding().role());
    Ok(rows)
}

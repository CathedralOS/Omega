//! Nominate a target entry from its exact checked declaration and source.

use super::input::PolicySource;
use diagnostics::Diagnostic;
use provider_acceptance::{AcceptedSemanticBinding, accepted_service_schema_digest};
use provider_plans::ServiceSchema;
use review_evidence::record::SemanticBindingReviewCandidate;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::Path;

use super::declarations::unavailable;
use provider_acceptance::AcceptedSemanticBindingRole as Role;
use review_evidence::record::{
    PackagePolicyBaseline, PackagePolicyCallableRole, PackageReviewNominalOwner,
};
use std::collections::BTreeSet;

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
    let mut slots = source.traits().iter().filter(|d| {
        d.is_boundary
            && source.symbols.display_path(d.symbol, "::") == path
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
    if accepted_service_schema_digest(binding.role(), &schema) != binding.normalized_schema_digest()
    {
        return Err(unavailable("a mismatched semantic role schema digest"));
    }
    Ok((slot.symbol, schema))
}

/// Review exact package-owned service references of the consumer's callable
/// contracts and its Build-selected physical entry. The supplier itself gets no
/// authority merely for declaring a service with one of the discovery names.
fn host_services(
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
        let origins = super::callables::service_origins(
            input.accepted(),
            source_provenance::DependencyScope::Product,
        )?;
        let ids = super::callables::service_identities(
            input.typed(),
            module,
            input.sources().exact_toolchain_sources(),
            &origins,
        )?;
        if let Some(reach) = &machine.contract.declared_service_reach {
            services.extend(super::callables::reviewed_services(&ids, reach)?);
        }
    }
    let mut rows = Vec::new();
    for (path, role) in [
        ("FilesystemHost", Role::FilesystemHostService),
        ("TimeHost", Role::TimeHostService),
    ] {
        let mut matches = services.iter().filter_map(|service| match service.owner() {
            PackageReviewNominalOwner::Package(owner) if service.application.is_none() && service.path() == path => Some(owner),
            _ => None,
        });
        let Some(owner) = matches.next() else {
            continue;
        };
        if matches.next().is_some() {
            return Err(unavailable("an ambiguous consumer service role"));
        }
        let source = input.typed();
        let mut declarations = source.traits().iter().filter(|slot| {
            slot.is_boundary
                && source.symbols.symbol_package_identity(slot.symbol) == Some(owner)
                && source.symbols.display_path(slot.symbol, "::") == path
        });
        let (Some(slot), None) = (declarations.next(), declarations.next()) else {
            return Err(unavailable("the consumer's referenced service declaration"));
        };
        let schema = ServiceSchema::from_declaration(
            input
                .accepted()
                .slot_schema(slot.symbol)
                .ok_or_else(|| unavailable("the consumer's referenced service schema"))?,
        );
        let binding = AcceptedSemanticBinding::new_service(
            role,
            owner,
            path,
            accepted_service_schema_digest(role, &schema),
        )
        .map_err(unavailable)?;
        let (_, schema) = checked_service(input, selected, &binding)?;
        rows.push(SemanticBindingReviewCandidate::new(binding, schema));
    }
    Ok(rows)
}

pub(super) fn candidates(
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
    policy: &PackagePolicyBaseline,
    target: target::TargetProfile,
    owner: PackageKeyIdentity,
) -> Result<
    (
        Vec<SemanticBindingReviewCandidate>,
        Option<SemanticBindingReviewCandidate>,
    ),
    Vec<Diagnostic>,
> {
    let source = input.typed();
    let accepted = input.accepted();
    let mut rows = host_services(input, selected, policy, target)?;
    rows.extend(super::hosted_services::nominations(input, selected)?);
    // Consent can constrain an active role. Only a new nomination produces
    // a candidate; replaying an old binding must not nominate its supplier.
    for candidate in &rows {
        let fresh = candidate.binding();
        if let Some(previous) = input.sources().accepted_semantic_binding(fresh.role())
            && (fresh.package() != previous.package()
                || fresh.declaration_path() != previous.declaration_path()
                || fresh.normalized_schema_digest() != previous.normalized_schema_digest()
                || fresh.selected_provider_plan_digest() != previous.selected_provider_plan_digest())
        {
            return Err(unavailable(
                "a fresh nomination conflicting with its accepted supplier",
            ));
        }
    }
    rows.sort_by_key(|row| row.binding().role());
    let slot = target.program_entry_slot();
    let (Some(contract), Some(schema_name)) =
        (slot.physical_contract_package, slot.boundary_schema)
    else {
        return Ok((rows, None));
    };
    let role = provider_acceptance::program_entry_semantic_binding_role(contract);
    let mut definitions = source.traits().iter().filter_map(|d| {
        if !d.is_boundary
            || d.name.as_str() != schema_name
            || source.symbols.symbol_package_identity(d.symbol) != Some(owner)
        {
            return None;
        }
        let file = source
            .symbols
            .symbol_provenance_source_span(d.symbol)
            .and_then(|s| source.symbols.source_file(s))?;
        (file.package_identity == Some(owner)
            && file.path.strip_prefix(&file.package_root).ok()
                == Some(Path::new(contract.package_relative_source())))
        .then_some((d, file))
    });
    let Some((decl, file)) = definitions.next() else {
        return Ok((rows, None));
    };
    if definitions.next().is_some() {
        return Err(vec![Diagnostic::error("ambiguous target-entry nomination")]);
    }
    let consumed = source_admission::source_identity::consumed_source_unit(file, false)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    if !input
        .sources()
        .package_subject()
        .is_some_and(|subject| subject.consumed_units().binary_search(&consumed).is_ok())
    {
        return Err(vec![Diagnostic::error(
            "target-entry nomination has no corresponding consumed source",
        )]);
    }
    let schema =
        ServiceSchema::from_declaration(accepted.slot_schema(decl.symbol).ok_or_else(|| {
            vec![Diagnostic::error(
                "target-entry declaration lacks the checked service schema",
            )]
        })?);
    let binding = AcceptedSemanticBinding::new_service(
        role,
        owner,
        source.symbols.display_path(decl.symbol, "::"),
        accepted_service_schema_digest(role, &schema),
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let entry = SemanticBindingReviewCandidate::new(binding, schema);
    Ok((rows, Some(entry)))
}

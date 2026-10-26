//! Close normalized provider identities under one explicit planning context.
//! Source-call decoding and executed Product requests meet here; neither can
//! bypass declaration roles, inherited lifetime closure or conflicting choices.

use super::{
    CompositionMode, ProviderOperatorFamilySelection, ProviderPlanningContext, ProviderSelection,
    ProviderSelectionIdentity, ProviderSelectionSubject, identities,
};
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};

pub(super) struct ProviderDeclarations {
    subject: ProviderSelectionSubject,
    provider_type: ProviderSelectionIdentity,
}

pub(super) fn form(
    context: ProviderPlanningContext<'_>,
    boundary_identity: ProviderSelectionIdentity,
    provider_type: ProviderSelectionIdentity,
) -> Result<ProviderDeclarations, Diagnostic> {
    let typed = context.typed;
    if !context.contains(boundary_identity.symbol) || !context.contains(provider_type.symbol) {
        return Err(Diagnostic::error(format!(
            "provider selection subject or provider is outside the {:?} planning context",
            context.scope
        )));
    }
    let subject = if boundary_identity.symbol.is_valid()
        && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Trait
        && typed.traits().iter().any(|definition| {
            definition.symbol == boundary_identity.symbol && definition.is_boundary
        }) {
        ProviderSelectionSubject::BoundaryTrait(boundary_identity)
    } else if boundary_identity.symbol.is_valid()
        && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Machine
        && typed.machines().iter().any(|requirement| {
            requirement.symbol == boundary_identity.symbol
                && requirement.spelling.is_none()
                && requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
    {
        ProviderSelectionSubject::BoundaryRequirement(boundary_identity)
    } else if boundary_identity.symbol.is_valid()
        && (typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Operator
            || typed
                .machine_token_bindings()
                .iter()
                .any(|binding| binding.is_boundary && binding.symbol == boundary_identity.symbol))
    {
        // One resolved overload symbol names its complete package-
        // qualified family; provider-planning derives the canonical
        // roster so harvest and later replay share one definition.
        match ProviderOperatorFamilySelection::derive(
            context,
            boundary_identity.symbol,
            boundary_identity.authored_path,
        ) {
            Ok(family) => ProviderSelectionSubject::BoundaryOperatorFamily(family),
            Err(reason) => {
                return Err(Diagnostic::error(reason));
            }
        }
    } else {
        return Err(Diagnostic::error(format!(
            "provider selection subject `{}` does not resolve to an exact boundary trait, top-level boundary requirement, or boundary-operator family",
            boundary_identity.authored_path
        )));
    };
    if !provider_type.symbol.is_valid()
        || (typed.symbols.get(provider_type.symbol).kind != SymbolKind::Data
            && !identities::is_named_provider(typed, provider_type.symbol))
    {
        return Err(Diagnostic::error(format!(
            "provider selection type `{}` does not resolve to an exact provider data declaration or named implementation",
            provider_type.authored_path
        )));
    }
    if let Some(implementation) = typed.conformances().iter().find(|implementation| {
        implementation.symbol == provider_type.symbol && identities::eligible(implementation)
    }) {
        for row in &implementation.rows {
            if identities::named_requirement_lifetime_arguments(
                typed,
                implementation,
                row.declaring_trait,
            )
            .is_none()
            {
                return Err(Diagnostic::error(format!(
                    "named provider `{}` requires an unsupported inherited declaring-trait lifetime application for `{}`; the exact parent substitution is not retained",
                    provider_type.authored_path, row.requirement_name,
                )));
            }
        }
    }
    Ok(ProviderDeclarations {
        subject,
        provider_type,
    })
}

pub(super) fn admit(
    context: ProviderPlanningContext<'_>,
    boundary_identity: ProviderSelectionIdentity,
    provider_type: ProviderSelectionIdentity,
    composition_mode: CompositionMode,
    machine: SymbolHandle,
    source_span: source::SourceSpan,
    previous: &[ProviderSelection],
) -> Result<ProviderSelection, Diagnostic> {
    let ProviderDeclarations {
        subject,
        provider_type,
    } = form(context, boundary_identity, provider_type)?;
    if let Some(existing) = previous
        .iter()
        .find(|selection| selection.subject.same_declaration_as(&subject))
    {
        if existing.provider_type.symbol != provider_type.symbol {
            return Err(Diagnostic::error(format!(
                "build selects two provider types for slot `{}`: `{}` and `{}`",
                subject.canonical_path(),
                existing.provider_type.canonical_path,
                provider_type.canonical_path,
            )));
        }
        if existing.composition_mode != composition_mode {
            return Err(Diagnostic::error(format!(
                "build selects provider `{}` for slot `{}` with conflicting composition modes {:?} and {:?}",
                provider_type.canonical_path,
                subject.canonical_path(),
                existing.composition_mode,
                composition_mode,
            )));
        }
    }
    Ok(ProviderSelection {
        subject,
        provider_type,
        composition_mode,
        selecting_machine: machine,
        source_span,
    })
}

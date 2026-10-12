//! Exact Fused routed-service establishment for a selected `ProgramEntry`
//! receiver.
//!
//! Psi records each logical `Binding<R>` carrier without selecting a provider.
//! This owner joins those occurrences to the selected Fused plans and emits
//! entry establishment; a logical marker alone grants no runtime authority.

mod root;

pub(crate) use root::derive_fused_program_entry_establishments;
pub(crate) use root::program_entry_has_unsettled_service_fields;
pub use root::program_entry_service_requirements;

use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};
use diagnostics::Diagnostic;
use provider_planning::CompositionMode;
use provider_planning::SelectedProviderReviewProvenance;
use typed_trees::data::{DataDefinition, DataField};

/// Selection presence is used for discovery tolerance, never to mint custody.
/// It retains the complete application even for an ancestor view beneath a
/// selected descendant. Establishment separately retains that whole plan.
pub(crate) fn has_fused_selection(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    arguments: &[typed_trees::types::TypeReferenceHandle],
) -> bool {
    let Some(schema) = carrier_schema(checked, &[], application, arguments) else {
        return false;
    };
    selected.iter().any(|candidate| {
        provider_planning::provider_receiver_application(
            checked,
            &candidate.plan,
            application,
            &schema,
        )
        .is_some()
            && candidate.selected_by.composition_mode() == Ok(CompositionMode::Fused)
    })
}

fn receiving_requirements(
    checked: &CheckedTrees,
    application: &provider_planning::ProviderReceiverApplication<'_>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<Vec<program_entry_plan::ProgramEntryReceivingRequirement>, &'static str> {
    let typed = &checked.typed;
    application
        .requirements
        .iter()
        .map(|row| {
            let closed = typed
                .closed_requirement_application(row.closed_requirement)
                .ok_or("receiving requirement lost its closed source application")?;
            let (owner, template) = typed
                .trait_machine_signature(row.template)
                .ok_or("receiving requirement lost its original declaration")?;
            let machine = typed
                .machine(row.provider)
                .ok_or("receiving requirement lost its checked realization")?;
            let requirement =
                typed.trait_requirement_callable_identity_text(owner, closed.signature);
            let source_requirement =
                typed.trait_requirement_callable_identity_text(owner, template);
            let candidate = typed
                .machine_callable_identity_text(machine)
                .ok_or("receiving requirement lost its realization identity")?;
            let declaration = typed
                .exact_owner_nominal_identity(row.selecting_declaration, &[])
                .and_then(|identity| typed.type_identity_text(identity))
                .ok_or("receiving requirement lost its exact choosing declaration")?;
            let receiving = typed
                .type_identity_text(closed.portable_trait_application_identity)
                .ok_or("receiving requirement lost its portable application")?;
            let regions = checked
                .facts
                .provider_closed_requirements
                .receiving_regions(row)
                .ok_or("receiving requirement lost its checked region span")?
                .iter()
                .map(|region| {
                    let text = typed_trees::type_identity::portable_lifetime_parameter_identity(
                        typed, *region,
                    )
                    .ok_or("receiving requirement lost a provider region coordinate")?;
                    Ok(identities.intern(&text))
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            Ok(program_entry_plan::ProgramEntryReceivingRequirement {
                source_requirement: identities.intern(&source_requirement),
                requirement: identities.intern(&requirement),
                candidate: identities.intern(&candidate),
                provider_declaration: identities.intern(&declaration),
                receiving_application: identities.intern(&receiving),
                receiving_regions: regions,
            })
        })
        .collect()
}

fn carrier_schema(
    checked: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    application: &typed_trees::trait_definition::AppliedTraitScope<'_>,
    arguments: &[typed_trees::types::TypeReferenceHandle],
) -> Option<effects::provider_plan::ServiceSchema> {
    provider_planning::service_schema::from_typed_application(
        provider_planning::ProviderPlanningContext::new(checked, source::DependencyScope::Product),
        calling_plans,
        application.clone(),
        arguments,
    )
}

fn data_field_identity(field: &DataField) -> String {
    field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned())
}

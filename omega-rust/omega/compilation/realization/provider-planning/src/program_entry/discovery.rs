//! Resolve package-review entry proposals using the consumer's checked Build
//! selection. This is nomination only: ordinary compilation still requires the
//! exact accepted binding, and review recompiles under that binding before it
//! offers an acceptance decision.

use super::{RootBinding, arrival_requirement_contract, selected_program_entry_machine};
use crate::{ProviderPlanningContext, ProviderSelection, ProviderSelectionSubject};
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use package_compilation::{AcceptedSemanticBinding, PackageCompilationInputs};

/// Disambiguate entry schemas only through an explicit selection of their
/// physical requirement. Same-spelled requirements in unrelated packages do
/// not match; two schemas inheriting the selected requirement still reject.
pub fn discover_explicit_entry_binding(
    typed: &TypedTrees,
    root_bindings: &[RootBinding],
    target: Option<target::TargetProfile>,
    selections: &[ProviderSelection],
    inputs: &PackageCompilationInputs,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    if !inputs.semantic_role_discovery() {
        return Ok(None);
    }
    let Some(entry) = selected_program_entry_machine(root_bindings, target)? else {
        return Ok(None);
    };
    let (Some(package), Some(schema_name), Some(physical_requirement)) = (
        entry.slot.physical_contract_package,
        entry.slot.boundary_schema,
        entry.slot.physical_arrival_requirement,
    ) else {
        return Ok(None);
    };
    let role = package_compilation::program_entry_semantic_binding_role(package);
    if inputs.accepted_semantic_binding(role).is_some() {
        return Ok(None);
    }
    let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut selected = None;
    for definition in typed.traits().iter().filter(|definition| {
        definition.is_boundary
            && definition.name.as_str() == schema_name
            && context.contains(definition.symbol)
    }) {
        let Some(owner) = typed.symbols.symbol_package_identity(definition.symbol) else {
            continue;
        };
        if inputs.package_root(owner).is_none() {
            continue;
        }
        let Ok(requirement) = arrival_requirement_contract(typed, definition, physical_requirement)
        else {
            continue;
        };
        let explicitly_selected = selections.iter().any(|selection| match &selection.subject {
            ProviderSelectionSubject::BoundaryRequirement(identity) => {
                identity.symbol == requirement.signature
            }
            ProviderSelectionSubject::BoundaryTrait(identity) => typed
                .trait_definition_by_symbol(identity.symbol)
                .is_some_and(|subject| {
                    typed
                        .trait_machine_signatures(subject)
                        .iter()
                        .any(|signature| signature.symbol == requirement.signature)
                }),
            ProviderSelectionSubject::BoundaryOperatorFamily(_) => false,
        });
        if !explicitly_selected {
            continue;
        }
        if selected.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "explicit physical-entry provider selection matches multiple package-owned `{schema_name}` schemas; package review requires one exact schema"
            ))]);
        }
        // Calling applications are checked during the following selection;
        // entry-role schema identity deliberately excludes those target rows.
        let schema = crate::service_schema::from_typed(context, &[], definition)
            .ok_or_else(|| vec![Diagnostic::error("selected entry schema cannot be reified")])?;
        selected = Some(
            AcceptedSemanticBinding::new_service(
                role,
                owner,
                typed.symbols.display_path(definition.symbol, "::"),
                package_compilation::accepted_service_schema_digest(role, &schema),
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        );
    }
    Ok(selected)
}

//! Closed named implementations retain provider identity independently of bodies.

use super::{CatalogRequirement, CheckedTrees, LoweringError, Machine, unsupported};

pub(super) fn provider_identities(
    checked: &CheckedTrees,
    machine: &Machine,
    requirement: CatalogRequirement<'_>,
) -> Result<Vec<(symbols::SymbolHandle, String)>, LoweringError> {
    let CatalogRequirement::Trait {
        definition,
        signature,
        application,
    } = requirement
    else {
        return Ok(Vec::new());
    };
    let typed = &checked.typed;
    let family_application = typed.closed_requirement_application(signature.symbol);
    let template = family_application.map_or(signature.symbol, |application| application.template);
    let mut identities = Vec::new();
    for (conformance, implementation) in typed
        .conformances
        .iter()
        .filter(|(_, implementation)| implementation.type_parameters.is_empty())
    {
        let rows = &implementation.rows;
        let original_requirement =
            application.map_or(template, |application| application.requirement_template);
        let mut matching = rows.iter().filter(|row| {
            row.declaring_trait == definition.symbol && row.requirement == original_requirement
        });
        let Some(row) = matching.next() else {
            continue;
        };
        if matching.next().is_some() {
            return unsupported("named provider repeats an exact boundary requirement row");
        }
        let Some(realization) = typed.machine(row.realization_machine) else {
            return unsupported("named provider row has no exact realization machine");
        };
        let states = typed.machine_states(realization);
        if !states
            .first()
            .is_some_and(|entry| entry.symbol == row.realization_state)
        {
            return unsupported("named provider row has no exact realization entry");
        }
        if family_application.is_some() {
            if !checked
                .facts
                .provider_closed_requirements
                .iter()
                .any(|row| {
                    row.provider == machine.symbol
                        && row.selecting_declaration == implementation.symbol
                        && row.template == template
                        && row.closed_requirement == signature.symbol
                        && family_application.is_some_and(|application| {
                            row.receiving_application == application.trait_application_identity
                        })
                })
                || !typed.requirement_realization_matches(
                    signature.symbol,
                    realization.symbol,
                    machine.symbol,
                )
            {
                continue;
            }
        } else if let Some(application) = application {
            if !super::requirements::realizes_closed_requirement(
                checked,
                machine,
                realization.symbol,
                conformance,
                application,
            )? {
                continue;
            }
        } else if !implementation.lifetime_parameters.is_empty() {
            // Regional candidates require the retained receiving application
            // and its checked public slot mapping; nominal lineage is insufficient.
            continue;
        } else {
            // Original requirements need no produced public header. Recover
            // this candidate's exact instance owner from the same source and
            // family receipts used by complete applied requirement headers.
            if !super::requirements::realization_candidate_matches(
                typed,
                realization.symbol,
                machine.symbol,
            ) {
                continue;
            }
            if typed.machine_states(machine).len() != states.len() {
                return unsupported("named provider lost its exact specialization entry");
            }
        }
        identities.push((
            implementation.symbol,
            typed.symbols.display_path(implementation.symbol, "::"),
        ));
    }
    Ok(identities)
}

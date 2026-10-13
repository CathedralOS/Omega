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
    } = requirement
    else {
        return Ok(Vec::new());
    };
    let typed = &checked.typed;
    let application = typed.closed_requirement_application(signature.symbol);
    let template = application.map_or(signature.symbol, |application| application.template);
    let mut identities = Vec::new();
    for implementation in typed
        .conformances()
        .iter()
        .filter(|implementation| implementation.type_parameters.is_empty())
    {
        let rows = &implementation.rows;
        let mut matching = rows
            .iter()
            .filter(|row| row.declaring_trait == definition.symbol && row.requirement == template);
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
        if application.is_some() {
            // Checking has joined the complete receiving application to this
            // map's public slots. Comparing the unapplied map with the receiver
            // here would confuse declaration parameters with their actuals.
            if !checked
                .facts
                .provider_closed_requirements
                .iter()
                .any(|row| {
                    row.provider == machine.symbol
                        && row.selecting_declaration == implementation.symbol
                        && row.template == template
                        && row.closed_requirement == signature.symbol
                        && application.is_some_and(|application| {
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
        } else if !implementation.lifetime_parameters.is_empty() {
            // A regional candidate needs a retained closed application and its
            // checked slot mapping; the nominal template supplies neither.
            continue;
        } else if realization.target_family_interface {
            // The row names a target family's head, the declaration every
            // member body implements; each member is a candidate, and Omega
            // installs the selected target's member. The head itself has no
            // body and never enters the catalog.
            if !typed.target_machine_families.iter().any(|family| {
                family.canonical == realization.symbol
                    && family
                        .members
                        .iter()
                        .any(|member| member.machine == machine.symbol)
            }) {
                continue;
            }
        } else if realization.symbol != machine.symbol {
            // Specialization owns the ordered template-to-instance state
            // correspondence; the map's entry above must remain its origin.
            let receipts = typed
                .machine_specializations
                .iter()
                .filter(|receipt| {
                    receipt.template == realization.symbol && receipt.instance == machine.symbol
                })
                .count();
            if receipts == 0 {
                continue;
            }
            if receipts != 1 || typed.machine_states(machine).len() != states.len() {
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

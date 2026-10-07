//! Closed named implementations retain provider identity independently of bodies.

use super::{CatalogRequirement, CheckedTrees, LoweringError, Machine, unsupported};

pub(super) fn provider_identities(
    checked: &CheckedTrees,
    machine: &Machine,
    requirement: CatalogRequirement<'_>,
) -> Result<Vec<String>, LoweringError> {
    let CatalogRequirement::Trait {
        definition,
        signature,
    } = requirement
    else {
        return Ok(Vec::new());
    };
    let typed = &checked.typed;
    let mut identities = Vec::new();
    for implementation in typed.conformances().iter().filter(|implementation| {
        implementation.type_parameters.is_empty() && implementation.lifetime_parameters.is_empty()
    }) {
        let rows = &implementation.rows;
        let mut matching = rows.iter().filter(|row| {
            row.declaring_trait == definition.symbol && row.requirement == signature.symbol
        });
        let Some(row) = matching.next() else {
            continue;
        };
        if matching.next().is_some() {
            return unsupported("named provider repeats an exact boundary requirement row");
        }
        let Some(realization) = typed
            .machines()
            .iter()
            .find(|realization| realization.symbol == row.realization_machine)
        else {
            return unsupported("named provider row has no exact realization machine");
        };
        let states = typed.machine_states(realization);
        if !states
            .first()
            .is_some_and(|entry| entry.symbol == row.realization_state)
        {
            return unsupported("named provider row has no exact realization entry");
        }
        if realization.symbol != machine.symbol {
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
        identities.push(typed.symbols.display_path(implementation.symbol, "::"));
    }
    Ok(identities)
}

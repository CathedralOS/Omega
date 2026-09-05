//! Rebind compiler callback coordinates to receipt-free semantic rows.

mod binders;
mod layouts;

use crate::record::{
    PackagePolicyCallbackDemand, PackagePolicyCallbackDestination,
    PackagePolicyCallbackMaterialization, PackagePolicyCallbacks,
};
use omega_calling_conventions::{NativePlace, ValidatedBoundaryEntryPlan};
use omega_compiler::CheckedCompilation;
use omega_provider_planning::calling_policy_plans::{
    BoundaryNativeParameterOrigin, MaterializedBoundarySignature,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::name::Identifier;

/// Called only after the containing owner replays the complete validated
/// application. Compact identifiers are transient exact joins, never output.
pub(crate) fn project_callback_policy(
    compilation: &CheckedCompilation,
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyCallbacks, Vec<Diagnostic>> {
    let mut binders = signature
        .callback_binders()
        .iter()
        .map(|binder| Ok((binder, binders::project(compilation, signature, binder)?)))
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    binders.sort_by_key(|(_, binder)| binder.static_machine_ordinal);
    if binders
        .windows(2)
        .any(|pair| pair[0].1.static_machine_ordinal == pair[1].1.static_machine_ordinal)
    {
        return Err(rejected(
            "callback binders repeat a semantic telescope ordinal",
        ));
    }
    let mut catalog = signature
        .callback_layout_catalog()
        .iter()
        .map(|entry| {
            Ok((
                entry,
                layouts::project(compilation, entry, lifetime_binders)?,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    catalog.sort_by(|(_, left), (_, right)| left.cmp(right));
    if catalog.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return Err(rejected(
            "callback catalog repeats a semantic layout destination",
        ));
    }
    let destination =
        |place: &NativePlace| -> Result<PackagePolicyCallbackDestination, Vec<Diagnostic>> {
            let parameter = match place {
                NativePlace::Parameter(parameter) | NativePlace::Field { parameter, .. } => {
                    *parameter
                }
            };
            let parameters = signature
                .native_parameters()
                .iter()
                .filter(|candidate| candidate.identity() == parameter)
                .collect::<Vec<_>>();
            let [parameter] = parameters.as_slice() else {
                return Err(rejected(
                    "callback destination has no exact native parameter",
                ));
            };
            let native_ordinal = parameter.native_ordinal();
            if signature
                .native_parameters()
                .get(native_ordinal as usize)
                .is_none_or(|candidate| candidate != *parameter)
            {
                return Err(rejected(
                    "callback native parameter changed its authored ordinal",
                ));
            }
            match place {
                NativePlace::Parameter(_) => {
                    if !matches!(
                        parameter.origin(),
                        BoundaryNativeParameterOrigin::PrivateCallback { .. }
                    ) {
                        return Err(rejected("whole-parameter callback lost its private origin"));
                    }
                    Ok(PackagePolicyCallbackDestination::Parameter { native_ordinal })
                }
                NativePlace::Field { .. } => {
                    let entries = catalog
                        .iter()
                        .enumerate()
                        .filter(|(_, (entry, _))| entry.destination() == place)
                        .collect::<Vec<_>>();
                    let [(index, (_, entry))] = entries.as_slice() else {
                        return Err(rejected(
                            "callback field destination lost its exact named catalog entry",
                        ));
                    };
                    if entry.native_ordinal != native_ordinal {
                        return Err(rejected("callback field changed its native ordinal"));
                    }
                    Ok(PackagePolicyCallbackDestination::Field {
                        native_ordinal,
                        layout_index: u32::try_from(*index)
                            .map_err(|_| rejected("callback layout index exceeds u32"))?,
                    })
                }
            }
        };
    let mut demands = Vec::new();
    for demand in signature.callback_demands() {
        let requirements = binders
            .iter()
            .filter(|(binder, _)| binder.requirement == demand.requirement)
            .map(|(_, binder)| &binder.requirement)
            .collect::<Vec<_>>();
        let Some(requirement) = requirements.first() else {
            return Err(rejected(
                "callback demand has no exact declared requirement",
            ));
        };
        if requirements
            .iter()
            .any(|candidate| candidate != requirement)
        {
            return Err(rejected(
                "callback compact requirement collides across semantic declarations",
            ));
        }
        demands.push(PackagePolicyCallbackDemand {
            destination: destination(&demand.destination)?,
            requirement: (*requirement).clone(),
        });
    }
    demands.sort();
    if demands
        .windows(2)
        .any(|pair| pair[0].destination == pair[1].destination)
    {
        return Err(rejected(
            "callback policy repeats a native destination demand",
        ));
    }
    let mut materializations = Vec::new();
    for row in &validated.plan().call.callback_materializations {
        let candidates = binders
            .iter()
            .enumerate()
            .filter(|(_, (binder, _))| binder.binder == row.binder)
            .collect::<Vec<_>>();
        let [(binder_index, (_, binder))] = candidates.as_slice() else {
            return Err(rejected(
                "callback materialization has no unique semantic binder",
            ));
        };
        let destination = destination(&row.destination)?;
        if demands
            .iter()
            .filter(|demand| {
                demand.destination == destination && demand.requirement == binder.requirement
            })
            .count()
            != 1
        {
            return Err(rejected(
                "callback materialization differs from its exact demand",
            ));
        }
        materializations.push(PackagePolicyCallbackMaterialization {
            binder_index: u32::try_from(*binder_index)
                .map_err(|_| rejected("callback binder index exceeds u32"))?,
            destination,
        });
    }
    materializations.sort();
    if demands.len() != materializations.len()
        || demands.iter().any(|demand| {
            materializations
                .iter()
                .filter(|row| row.destination == demand.destination)
                .count()
                != 1
        })
    {
        return Err(rejected(
            "callback materializations do not supply every demand exactly once",
        ));
    }
    if catalog.len()
        != demands
            .iter()
            .filter(|demand| {
                matches!(
                    demand.destination,
                    PackagePolicyCallbackDestination::Field { .. }
                )
            })
            .count()
    {
        return Err(rejected(
            "callback layout catalog contains an unconsumed semantic entry",
        ));
    }
    Ok(PackagePolicyCallbacks {
        binders: binders.into_iter().map(|(_, binder)| binder).collect(),
        demands,
        materializations,
        layouts: catalog.into_iter().map(|(_, layout)| layout).collect(),
    })
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "calling policy rejects {reason}"
    ))]
}

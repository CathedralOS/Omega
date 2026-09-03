//! Source-selected import and syscall demand to exact mechanism admission.

use std::collections::BTreeSet;

use crate::realization::model::{NativeBoundaryRealization, NativeProviderSettlement};
use crate::realization::providers::AdmittedTerminalMechanism;
use psi_diagnostics::Diagnostic;

pub(super) fn validate_source_evaluated_import_coverage(
    plan: &omega_abstract_operations::AbstractOperationPlan,
    selected_plans: &omega_effects::SelectedProviderPlanFacts,
    policy: &crate::realization::TerminalAuthorityPolicy,
    target: omega_target::NativeTarget,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    settlements: &[NativeProviderSettlement<'_>],
    native_callbacks: &[
        omega_abstract_operations_to_target_operations::AdmittedNativeCallbackArgument
    ],
) -> Result<Vec<AdmittedTerminalMechanism>, Vec<Diagnostic>> {
    let boundary_identities = plan
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeSet::new();
    for operation in plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
    {
        let omega_abstract_operations::AbstractOperation::BoundaryCall { boundary, .. } = operation
        else {
            continue;
        };
        let Some(requirement) = boundary_identities.get(boundary) else {
            return Err(vec![Diagnostic::error(format!(
                "external-binding demand cites absent boundary {boundary:?}"
            ))]);
        };
        demanded.insert(*requirement);
    }

    let mut callbacks_by_requirement = std::collections::BTreeMap::<
        &str,
        Vec<&omega_abstract_operations_to_target_operations::AdmittedNativeCallbackArgument>,
    >::new();
    for callback in native_callbacks {
        let matching = plan
            .functions
            .iter()
            .flat_map(|function| &function.operations)
            .filter_map(|operation| {
                let omega_abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation,
                    boundary,
                    ..
                } = operation
                else {
                    return None;
                };
                (*psi_operation == callback.terminal_operation).then_some(boundary)
            })
            .collect::<Vec<_>>();
        let [boundary] = matching.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "native callback operation {} resolves to {} abstract boundary calls during source-import coverage",
                callback.terminal_operation.get(),
                matching.len(),
            ))]);
        };
        let requirement = boundary_identities.get(boundary).copied().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "native callback operation {} cites absent boundary {:?}",
                callback.terminal_operation.get(),
                boundary,
            ))]
        })?;
        callbacks_by_requirement
            .entry(requirement)
            .or_default()
            .push(callback);
    }

    let mut required_imports = BTreeSet::new();
    let mut admitted_mechanisms = Vec::new();
    for requirement in demanded {
        let selected_rows = selected_plans
            .plans()
            .iter()
            .flat_map(|provider_plan| {
                provider_plan
                    .rows
                    .iter()
                    .filter(move |row| row.requirement_identity == requirement)
                    .map(move |row| (provider_plan, row))
            })
            .collect::<Vec<_>>();
        if selected_rows.len() > 1
            && selected_rows.iter().any(|(_, row)| {
                matches!(
                    row.binding,
                    omega_effects::provider_plan::ProviderBinding::Import { .. }
                        | omega_effects::provider_plan::ProviderBinding::Syscall { .. }
                        | omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap { .. }
                )
            })
        {
            return Err(vec![Diagnostic::error(format!(
                "demanded external binding `{requirement}` resolves to {} selected provider rows",
                selected_rows.len(),
            ))]);
        }
        if selected_rows.iter().any(|(_, row)| {
            matches!(
                row.binding,
                omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap { .. }
            )
        }) {
            return Err(vec![Diagnostic::error(format!(
                "demanded import `{requirement}` retains a legacy string-backed binding with no normalized terminal-mechanism identity"
            ))]);
        }
        let syscall_matches = selected_rows
            .iter()
            .filter(|(_, row)| {
                matches!(
                    row.binding,
                    omega_effects::provider_plan::ProviderBinding::Syscall { .. }
                )
            })
            .copied()
            .collect::<Vec<_>>();
        match syscall_matches.as_slice() {
            [] => {}
            [(provider_plan, row)] => {
                let omega_effects::provider_plan::ProviderBinding::Syscall { number } = row.binding
                else {
                    unreachable!("filtered syscall row")
                };
                let target_profile =
                    omega_target::TargetProfile::from_canonical_target_name(&provider_plan.target)
                        .map_err(|diagnostic| vec![diagnostic])?;
                if target_profile.native_target() != target {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` targets `{}` rather than the receiving native target",
                        provider_plan.target,
                    ))]);
                }
                let external_rows = external_binding_rows
                    .iter()
                    .filter(|external| external.requirement_identity == requirement)
                    .collect::<Vec<_>>();
                let [external] = external_rows.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` resolves to {} retained external binding rows",
                        external_rows.len(),
                    ))]);
                };
                let omega_calling_conventions::ExternalBindingKind::Syscall {
                    number: external_number,
                } = external.binding
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` does not retain one exact syscall external binding"
                    ))]);
                };
                let external_target_profile =
                    omega_target::TargetProfile::from_canonical_target_name(&external.target_name)
                        .map_err(|diagnostic| vec![diagnostic])?;
                if external_target_profile != target_profile {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` substituted its retained external target profile"
                    ))]);
                }
                if external_number != number {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` substituted its normalized syscall number"
                    ))]);
                }
                let matching_boundaries = boundary_identities
                    .iter()
                    .filter(|(_, identity)| **identity == requirement)
                    .map(|(boundary, _)| *boundary)
                    .collect::<Vec<_>>();
                let [boundary] = matching_boundaries.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` resolves to {} Terminal boundaries",
                        matching_boundaries.len(),
                    ))]);
                };
                let mechanism = crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
                    target_profile,
                    number,
                    plan,
                    *boundary,
                )
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` has no exact checked argument contract: {error}"
                    ))]
                })?;
                policy.classify(mechanism).map_err(|unclassified| {
                    vec![Diagnostic::error(format!(
                        "receiving terminal-authority policy version {} does not classify syscall mechanism {:?} required by `{requirement}`",
                        policy.identity().version(),
                        unclassified.mechanism(),
                    ))]
                })?;
                admitted_mechanisms.push(AdmittedTerminalMechanism {
                    boundary: *boundary,
                    mechanism,
                });
            }
            matches => {
                return Err(vec![Diagnostic::error(format!(
                    "demanded syscall `{requirement}` resolves to {} selected syscall rows",
                    matches.len(),
                ))]);
            }
        }
        let import_matches = selected_rows
            .iter()
            .filter(|(_, row)| {
                matches!(
                    row.binding,
                    omega_effects::provider_plan::ProviderBinding::Import { .. }
                )
            })
            .copied()
            .collect::<Vec<_>>();
        match import_matches.as_slice() {
            [] => {}
            [(_provider_plan, row)] => {
                let omega_effects::provider_plan::ProviderBinding::Import { evaluated } =
                    &row.binding
                else {
                    unreachable!("filtered import row")
                };
                if evaluated.locator().target().native_target() != target {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` targets `{}` rather than the receiving native target",
                        evaluated.locator().target().target_name(),
                    ))]);
                }
                let external_rows = external_binding_rows
                    .iter()
                    .filter(|external| external.requirement_identity == requirement)
                    .collect::<Vec<_>>();
                let [external] = external_rows.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` resolves to {} retained external implementation contracts",
                        external_rows.len(),
                    ))]);
                };
                let (
                    omega_calling_conventions::ExternalBindingKind::Import {
                        locator: external_locator,
                    },
                    Some(boundary_entry_plan),
                ) = (&external.binding, &external.boundary_entry_plan)
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` does not retain one normalized locator and admitted boundary contract"
                    ))]);
                };
                if external_locator != evaluated.locator() {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` substituted its retained external locator"
                    ))]);
                }
                let matching_callbacks = callbacks_by_requirement
                    .get(requirement)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let mechanism = match matching_callbacks {
                    [] => crate::realization::normalized_foreign_terminal_mechanism(
                        evaluated.locator(),
                        boundary_entry_plan,
                    ),
                    [callback]
                        if callback.registrar_boundary_entry_plan == *boundary_entry_plan =>
                    {
                        crate::realization::normalized_foreign_terminal_mechanism_with_callback_materializations(
                            evaluated.locator(),
                            boundary_entry_plan,
                            &callback.registrar_context,
                        )
                    }
                    callbacks => Err(format!(
                        "retained implementation contract rejoins {} exact native callbacks with no unique matching registrar plan",
                        callbacks.len(),
                    )),
                }
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` has an invalid admitted implementation contract: {error}"
                    ))]
                })?;
                policy.classify(mechanism).map_err(|unclassified| {
                    vec![Diagnostic::error(format!(
                        "receiving terminal-authority policy version {} does not classify normalized foreign mechanism {:?} required by `{requirement}`",
                        policy.identity().version(),
                        unclassified.mechanism(),
                    ))]
                })?;
                let matching_boundaries = boundary_identities
                    .iter()
                    .filter(|(_, identity)| **identity == requirement)
                    .map(|(boundary, _)| *boundary)
                    .collect::<Vec<_>>();
                let [boundary] = matching_boundaries.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` resolves to {} Terminal boundaries",
                        matching_boundaries.len()
                    ))]);
                };
                admitted_mechanisms.push(AdmittedTerminalMechanism {
                    boundary: *boundary,
                    mechanism,
                });
                required_imports.insert(requirement);
            }
            matches => {
                return Err(vec![Diagnostic::error(format!(
                    "demanded source-evaluated import `{requirement}` resolves to {} selected import rows",
                    matches.len()
                ))]);
            }
        }
    }

    let mut covered_imports = BTreeSet::new();
    for settlement in settlements {
        let requirement = settlement.provider_execution.requirement_identity();
        let is_normalized = matches!(
            settlement.realization,
            NativeBoundaryRealization::NormalizedForeignCall(_)
        );
        if required_imports.contains(requirement) {
            if !is_normalized {
                return Err(vec![Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` requires a normalized foreign-call settlement"
                ))]);
            }
            if !covered_imports.insert(requirement) {
                return Err(vec![Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` received more than one normalized settlement"
                ))]);
            }
        } else if is_normalized {
            return Err(vec![Diagnostic::error(format!(
                "normalized foreign-call settlement for `{requirement}` does not cover a demanded selected import"
            ))]);
        }
    }
    if let Some(missing) = required_imports
        .iter()
        .find(|requirement| !covered_imports.contains(**requirement))
    {
        return Err(vec![Diagnostic::error(format!(
            "demanded source-evaluated import `{missing}` has no admitted native settlement"
        ))]);
    }
    admitted_mechanisms.sort_by_key(|row| row.boundary);
    Ok(admitted_mechanisms)
}

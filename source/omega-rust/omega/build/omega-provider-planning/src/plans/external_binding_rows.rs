use psi_diagnostics::Diagnostic;
use std::sync::Arc;

/// Transactionally binds the selected provider projection to the checked
/// phase result that carries it into backend planning. Complete projection is
/// staged before the retained sidecar can change.
pub fn settle_external_binding_rows(
    retained: &mut Arc<[omega_calling_conventions::ExternalBindingRow]>,
    typed: &psi_typed_trees::TypedTrees,
    selected_target: Option<&str>,
    native_target: omega_target::NativeTarget,
    selected_plans: &[omega_effects::provider_plan::ProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
) -> Result<(), Vec<Diagnostic>> {
    let rows = extract_external_binding_rows(
        selected_target,
        native_target,
        selected_plans,
        boundary_calling_plan_realizations,
        typed,
    )?;
    if retained.as_ref() == rows.as_slice() {
        return Ok(());
    }
    *retained = Arc::from(rows);
    Ok(())
}

/// Extract bodyless external leaves into the calling-convention rows consumed
/// by the freestanding ABI builder.
pub fn extract_external_binding_rows(
    selected_target: Option<&str>,
    native_target: omega_target::NativeTarget,
    selected_plans: &[omega_effects::provider_plan::ProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    typed: &psi_typed_trees::TypedTrees,
) -> Result<Vec<omega_calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    extract_external_binding_rows_for_scope(
        selected_target,
        native_target,
        selected_plans,
        boundary_calling_plan_realizations,
        typed,
        false,
    )
}

/// Extract only normalized import leaves for compiler-to-native custody.
/// Other selected provider mechanisms retain their existing specialized
/// lowering and must not be forced through host-ABI compatibility planning.
pub fn extract_normalized_import_binding_rows(
    selected_target: Option<&str>,
    native_target: omega_target::NativeTarget,
    selected_plans: &[omega_effects::provider_plan::ProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    typed: &psi_typed_trees::TypedTrees,
) -> Result<Vec<omega_calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    extract_external_binding_rows_for_scope(
        selected_target,
        native_target,
        selected_plans,
        boundary_calling_plan_realizations,
        typed,
        true,
    )
}

fn extract_external_binding_rows_for_scope(
    selected_target: Option<&str>,
    native_target: omega_target::NativeTarget,
    selected_plans: &[omega_effects::provider_plan::ProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    typed: &psi_typed_trees::TypedTrees,
    normalized_imports_only: bool,
) -> Result<Vec<omega_calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    use omega_calling_conventions::{CallingPolicy, ExternalBindingKind, ExternalBindingRow};
    use omega_effects::provider_plan::ProviderBinding;

    let mut rows = Vec::new();
    // The selected ProviderPlan set is the immutable normalization boundary.
    // Do not rescan source `via` clauses after selection: doing so would create
    // a second binding authority beside the retained typed identity.
    for plan in selected_plans {
        for row in &plan.rows {
            if normalized_imports_only && !matches!(&row.binding, ProviderBinding::Import { .. }) {
                continue;
            }
            let binding = match &row.binding {
                ProviderBinding::Import { locator } => ExternalBindingKind::Import {
                    locator: locator.clone(),
                },
                ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
                    ExternalBindingKind::StringBackedImportBootstrap {
                        module: library.clone(),
                        symbol: symbol.clone(),
                    }
                }
                ProviderBinding::Syscall { number } => {
                    ExternalBindingKind::Syscall { number: *number }
                }
                ProviderBinding::CompilerIntrinsic { machine } => {
                    ExternalBindingKind::CompilerIntrinsic {
                        machine: machine.clone(),
                    }
                }
                ProviderBinding::VtableSlot { index } => {
                    ExternalBindingKind::VtableSlot { index: *index }
                }
                ProviderBinding::VtableField { field, .. } => ExternalBindingKind::VtableField {
                    field: field.clone(),
                },
                ProviderBinding::TableFunction { field, .. } => {
                    ExternalBindingKind::TableFunction {
                        field: field.clone(),
                    }
                }
                ProviderBinding::CheckedAdapter { .. } => continue,
            };
            let boundary_entry_plan = selected_source_boundary_entry_plan(
                typed,
                boundary_calling_plan_realizations,
                plan,
                &plan.schema.trait_name,
                &row.method,
                &row.requirement_identity,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            // Boundary operators are consumed by checked operator dispatch,
            // where the exact selected ProviderPlan realization is retained
            // on the operator-use fact. They are not host ABI calls and must
            // not be reinterpreted as platform-call catalog entries merely
            // because their selected realization is compiler-owned.
            if matches!(&binding, ExternalBindingKind::CompilerIntrinsic { .. })
                && typed.operators().iter().any(|operator| {
                    operator.is_boundary
                        && psi_typed_trees::operator::boundary_operator_requirement_identity(
                            typed, operator,
                        ) == plan.schema.trait_name
                })
            {
                continue;
            }
            let compatibility_policy = match &binding {
                ExternalBindingKind::CompilerIntrinsic { .. } => None,
                ExternalBindingKind::Syscall { .. } => {
                    match (native_target.object_format, native_target.architecture) {
                        (omega_target::ObjectFormat::Elf, omega_target::Architecture::X86_64) => {
                            Some(CallingPolicy::LinuxSyscallX86_64)
                        }
                        (omega_target::ObjectFormat::Elf, omega_target::Architecture::Aarch64) => {
                            Some(CallingPolicy::LinuxSyscallAarch64)
                        }
                        _ => None,
                    }
                }
                _ => Some(CallingPolicy::native_for_target(native_target)),
            };
            let boundary_entry_plan = match (boundary_entry_plan, compatibility_policy) {
                (Some(plan), _) => Some(plan),
                (None, Some(policy)) => {
                    crate::calling_policy_plans::evaluate_compatibility_boundary_entry_plan(
                        typed,
                        &plan.schema.trait_name,
                        &row.method,
                        &row.requirement_identity,
                        policy,
                        usize::from(matches!(
                            &binding,
                            ExternalBindingKind::TableFunction { .. }
                        )),
                    )
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "cannot evaluate compatibility calling plan for `{}::{}`: {reason}",
                            plan.schema.trait_name, row.method
                        ))]
                    })?
                }
                (None, None) => None,
            };
            rows.push(ExternalBindingRow {
                target_name: if plan.target.is_empty() {
                    selected_target.unwrap_or("cross_platform_cli").to_owned()
                } else {
                    plan.target.clone()
                },
                trait_name: plan.schema.trait_name.clone(),
                method: row.method.clone(),
                requirement_identity: row.requirement_identity.clone(),
                table_type: plan.provider_type.clone(),
                boundary_entry_plan,
                binding,
            });
        }
    }
    Ok(rows)
}

/// Resolve implementation evidence only through the provider candidate that
/// selection admitted. The public provider/schema identity carries the
/// canonical fingerprint; the typed program retains the corresponding plan
/// internally so lowering never has to rediscover or re-run policy source.
fn selected_source_boundary_entry_plan(
    typed: &psi_typed_trees::TypedTrees,
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    plan: &omega_effects::provider_plan::ProviderPlan,
    trait_name: &str,
    method_name: &str,
    requirement_identity: &str,
) -> Result<Option<omega_calling_conventions::BoundaryEntryPlan>, Diagnostic> {
    if plan.name.is_empty() {
        return Err(Diagnostic::error(
            "selected source boundary entry plan has an empty ProviderPlan name",
        ));
    }
    let provider_plan_name = plan.name.as_str();
    if plan.schema.trait_name != trait_name {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{}` serves schema `{}`, not exact requested schema `{trait_name}`",
            plan.name, plan.schema.trait_name
        )));
    }
    if requirement_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` has an empty exact requirement overload identity"
        )));
    }

    let matching_methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.name == method_name && method.requirement_identity == requirement_identity
        })
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` binds {} exact schema methods for `{trait_name}::{method_name}` / `{requirement_identity}`",
            matching_methods.len()
        )));
    };
    if method.requirement_owner.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` method `{method_name}` has an empty exact requirement owner"
        )));
    }

    let schema_operators = typed
        .operators()
        .iter()
        .filter(|operator| {
            operator.is_boundary
                && psi_typed_trees::operator::boundary_operator_requirement_identity(
                    typed, operator,
                ) == plan.schema.trait_name
        })
        .collect::<Vec<_>>();
    if !schema_operators.is_empty() {
        let [operator] = schema_operators.as_slice() else {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry schema `{}` resolves to {} exact typed boundary operators",
                plan.schema.trait_name,
                schema_operators.len()
            )));
        };
        let operator_identity =
            psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
        if method.name != "realize"
            || method.requirement_owner != operator_identity
            || method.requirement_identity != operator_identity
        {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry ProviderPlan `{provider_plan_name}` does not bind exact boundary operator `{operator_identity}`"
            )));
        }
        return match (
            method.calling_plan_report_fingerprint,
            method.calling_plan_commitment,
        ) {
            (None, None) => Ok(None),
            _ => Err(Diagnostic::error(format!(
                "selected source boundary operator `{operator_identity}` retains a trait calling-plan fingerprint"
            ))),
        };
    }

    let schema_owners = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == plan.schema.trait_name
        })
        .collect::<Vec<_>>();
    let [schema_owner] = schema_owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry schema `{}` resolves to {} exact typed boundary traits",
            plan.schema.trait_name,
            schema_owners.len()
        )));
    };

    let requirement_owners = typed
        .traits()
        .iter()
        .filter(|definition| definition.name.as_str() == method.requirement_owner)
        .collect::<Vec<_>>();
    let [requirement_owner] = requirement_owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` resolves to {} exact typed traits",
            method.requirement_owner,
            requirement_owners.len()
        )));
    };
    if !requirement_owner.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` is not an exact boundary trait",
            method.requirement_owner
        )));
    }

    let matching_signatures = typed
        .trait_machine_signatures(requirement_owner)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == method_name
                && typed
                    .normalized_trait_requirement_overload_identity(requirement_owner, signature)
                    .identity()
                    == requirement_identity
        })
        .collect::<Vec<_>>();
    let [signature] = matching_signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement `{}` binds {} exact typed signatures for `{method_name}` / `{requirement_identity}`",
            method.requirement_owner,
            matching_signatures.len()
        )));
    };

    let (Some(fingerprint), Some(commitment)) = (
        method.calling_plan_report_fingerprint,
        method.calling_plan_commitment,
    ) else {
        return if method.calling_plan_report_fingerprint.is_none()
            && method.calling_plan_commitment.is_none()
        {
            Ok(None)
        } else {
            Err(Diagnostic::error(format!(
                "selected source boundary entry `{trait_name}::{method_name}` does not retain its calling-plan report coordinate and commitment together"
            )))
        };
    };
    if fingerprint == 0 {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` has a zero calling-plan fingerprint"
        )));
    }

    let matching_realizations = boundary_calling_plan_realizations
        .iter()
        .filter(|realization| {
            realization.report_fingerprint == fingerprint
                && realization.commitment == commitment
                && realization.boundary_trait == schema_owner.symbol
                && realization.requirement_machine == signature.symbol
        })
        .collect::<Vec<_>>();
    let [realization] = matching_realizations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` resolves to {} exact calling-plan realizations for fingerprint 0x{fingerprint:016x}",
            matching_realizations.len()
        )));
    };
    let validated = realization.replayed_validated_plan().map_err(|error| {
        Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` retained an invalid target calling-plan realization: {error}"
        ))
    })?;
    if validated.plan() != realization.exact_boundary_entry_plan() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` substituted a calling plan behind its compact report fingerprint"
        )));
    }
    if realization.report_fingerprint != validated.contract_report_fingerprint()
        || realization.commitment.as_bytes() != validated.contract_commitment_digest()
    {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` substituted a calling-plan commitment behind its compact report fingerprint"
        )));
    }

    Ok(Some(realization.exact_boundary_entry_plan().clone()))
}

#[cfg(test)]
#[path = "external_binding_rows/tests.rs"]
mod tests;

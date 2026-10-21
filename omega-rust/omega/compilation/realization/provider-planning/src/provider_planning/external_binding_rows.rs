use diagnostics::Diagnostic;

mod table_dispatch;

/// Retain selected external mechanisms for compiler-to-native custody.
/// Imports, kernel entries and table fields rejoin the same selected row;
/// table offsets additionally come from that declaration's authoritative layout.
/// These rows grant no terminal authority or execution permission: the receiving
/// policy and provider settlements remain independently required.
/// Other selected provider mechanisms retain their existing specialized
/// lowering and must not be forced through host-ABI compatibility planning.
pub(crate) fn extract_native_external_binding_rows(
    selected_target: Option<&str>,
    native_target: target::NativeTarget,
    selected_plans: &[effects::provider_plan::ProviderPlan],
    sources: &[crate::DerivedProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    typed: &checked_trees::typed_trees::TypedTrees,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<Vec<calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    use crate::calling_policy_plans::selected_source_calling_application;
    use calling_conventions::{CallingPolicy, ExternalBindingKind, ExternalBindingRow};
    use effects::provider_plan::ProviderBinding;

    let mut rows = Vec::new();
    // The selected ProviderPlan set is the immutable normalization boundary.
    // Do not rescan source `via` clauses after selection: doing so would create
    // a second binding authority beside the retained typed identity.
    for plan in selected_plans {
        for (row_index, row) in plan.rows.iter().enumerate() {
            let binding = match &row.binding {
                ProviderBinding::Import { evaluated } => ExternalBindingKind::Import {
                    // The selected plan already commits the atomic evaluation
                    // receipt; this ABI-facing row owns only physical locator
                    // coordinates.
                    locator: evaluated.locator().clone(),
                },
                ProviderBinding::Syscall { number } => {
                    ExternalBindingKind::Syscall { number: *number }
                }
                ProviderBinding::VtableField { field, .. }
                | ProviderBinding::TableFunction { field, .. } => {
                    let source = sources
                        .iter()
                        .find(|source| source.plan == *plan)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "selected table binding has no source provenance",
                            )]
                        })?;
                    let byte_offset = table_dispatch::field_offset(
                        typed,
                        native_target,
                        &source.provenance,
                        row_index,
                        field,
                        opaque_representation_selections,
                    )
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "cannot realize table field `{field}` for `{}`: {reason}",
                            row.method
                        ))]
                    })?;
                    match row.binding {
                        ProviderBinding::VtableField { .. } => ExternalBindingKind::VtableField {
                            field: field.clone(),
                            byte_offset,
                        },
                        _ => ExternalBindingKind::TableFunction {
                            field: field.clone(),
                            byte_offset,
                        },
                    }
                }
                ProviderBinding::CompilerIntrinsic { .. }
                | ProviderBinding::VtableSlot { .. }
                | ProviderBinding::CheckedAdapter { .. } => continue,
            };
            let calling_application = selected_source_calling_application(
                typed,
                boundary_calling_plan_realizations,
                plan,
                plan.schema.identity_text(plan.schema.trait_name),
                &row.method,
                plan.identities.text(row.requirement_identity),
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            let compatibility_policy = match &binding {
                ExternalBindingKind::Syscall { .. } => {
                    CallingPolicy::native_syscall_for_target(native_target)
                }
                _ => Some(CallingPolicy::native_for_target(native_target)),
            };
            let calling_application = match (calling_application, compatibility_policy) {
                (Some(plan), _) => Some(plan),
                (None, Some(policy)) => {
                    crate::calling_policy_plans::evaluate_default_calling_application(
                        typed,
                        native_target,
                        plan.schema.identity_text(plan.schema.trait_name),
                        plan.schema.trait_package_identity,
                        &row.method,
                        plan.identities.text(row.requirement_identity),
                        policy,
                        usize::from(matches!(binding, ExternalBindingKind::TableFunction { .. })),
                        opaque_representation_selections,
                    )
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "cannot evaluate default native calling application for `{}::{}`: {reason}",
                            plan.schema.identity_text(plan.schema.trait_name),
                            row.method
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
                trait_name: plan.schema.identity_text(plan.schema.trait_name).to_owned(),
                method: row.method.clone(),
                requirement_identity: plan.identities.text(row.requirement_identity).to_owned(),
                table_type: plan.provider_type.clone(),
                calling_application,
                binding,
            });
        }
    }
    Ok(rows)
}

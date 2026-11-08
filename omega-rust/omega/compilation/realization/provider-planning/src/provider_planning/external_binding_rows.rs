use crate::provider_planning::trait_name_text;
use diagnostics::Diagnostic;

mod table_dispatch;

/// Retain selected external mechanisms for compiler-to-native custody.
/// Imports, kernel entries and table fields rejoin the same selected row;
/// table offsets additionally come from that declaration's authoritative layout.
/// These rows grant no terminal authority or execution permission: the receiving
/// policy and provider settlements remain independently required.
/// Other selected provider mechanisms retain their existing specialized
/// lowering and must not be forced through host-ABI compatibility planning.
pub fn extract_native_external_binding_rows(
    selected_target: Option<&str>,
    native_target: target::NativeTarget,
    plans: &[&effects::provider_plan::ProviderPlan],
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    calling_program: &crate::calling_policy_plans::CallingProgram,
) -> Result<Vec<calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    let context = ExternalBindingContext {
        selected_target,
        native_target,
        boundary_calling_plan_realizations,
        calling_program,
    };
    let mut rows = Vec::new();
    // Each complete plan retains its original row coordinates.
    // A direct-call projection borrows this same operation before changing
    // its demand identity; no loose plan rediscovers its source owner.
    for plan in plans {
        for row_index in 0..plan.rows.len() {
            if let Some(row) = context.row(plan, row_index)? {
                rows.push(row);
            }
        }
    }
    Ok(rows)
}

/// Shared canonical inputs for preparing physical rows. Preparation retains
/// either the complete ABI result or its refusal; it grants no execution.
pub(crate) struct ExternalBindingContext<'a> {
    pub selected_target: Option<&'a str>,
    pub native_target: target::NativeTarget,
    pub boundary_calling_plan_realizations:
        &'a [crate::calling_policy_plans::BoundaryCallingPlanRealization],
    pub calling_program: &'a crate::calling_policy_plans::CallingProgram,
}

impl ExternalBindingContext<'_> {
    /// Portable plans inherit the selected scope; scoped plans must name it.
    /// Preparing an outside-scope candidate retains this refusal as review
    /// content, while selected row extraction requires a successful binding.
    fn target_profile(
        &self,
        plan: &effects::provider_plan::ProviderPlan,
    ) -> Result<target::TargetProfile, Vec<Diagnostic>> {
        let name = if plan.target.is_empty() {
            self.selected_target
        } else {
            if self
                .selected_target
                .is_some_and(|selected| selected != plan.target)
            {
                return Err(vec![Diagnostic::error(
                    "external ABI plan is outside the selected deployment scope",
                )]);
            }
            Some(plan.target.as_str())
        };
        let profile = match name {
            Some(name) => target::TargetProfile::from_canonical_target_name(name)
                .map_err(|error| vec![error])?,
            None => {
                return Err(vec![Diagnostic::error(
                    "external ABI requires an exact deployment profile",
                )]);
            }
        };
        if profile.native_target() != self.native_target {
            return Err(vec![Diagnostic::error(
                "external ABI layout names another selected native target",
            )]);
        }
        Ok(profile)
    }

    pub(crate) fn row(
        &self,
        plan: &effects::provider_plan::ProviderPlan,
        row_index: usize,
    ) -> Result<Option<calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
        use crate::calling_policy_plans::selected_calling_applications;
        use calling_conventions::{CallingPolicy, ExternalBindingKind, ExternalBindingRow};
        use effects::provider_plan::ProviderBinding;
        let Self {
            selected_target,
            native_target,
            boundary_calling_plan_realizations,
            calling_program,
        } = *self;
        let row = plan.rows.get(row_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "external binding lost its original provider row",
            )]
        })?;
        if matches!(
            row.binding,
            ProviderBinding::CompilerIntrinsic { .. }
                | ProviderBinding::VtableSlot { .. }
                | ProviderBinding::CheckedAdapter { .. }
        ) {
            return Ok(None);
        }
        let target_profile = self.target_profile(plan)?;
        let selected = selected_calling_applications(
            calling_program.artifact.semantic_module(),
            boundary_calling_plan_realizations,
            plan,
            row_index,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let binding = match &row.binding {
            ProviderBinding::Import { evaluated } => {
                ExternalBindingKind::Import {
                    // The selected plan already commits the atomic evaluation
                    // receipt; this ABI-facing row owns only physical locator
                    // coordinates.
                    locator: evaluated.locator().clone(),
                }
            }
            ProviderBinding::Syscall { number } => ExternalBindingKind::Syscall { number: *number },
            ProviderBinding::VtableField { field, storage, .. }
            | ProviderBinding::TableFunction { field, storage, .. } => {
                let applications = if selected.applications.is_empty() {
                    selected
                        .default_abis
                        .as_ref()
                        .map_err(|error| vec![error.clone()])?
                        .iter()
                        .collect::<Vec<_>>()
                } else {
                    selected
                        .applications
                        .iter()
                        .map(|application| &application.abi)
                        .collect()
                };
                let byte_offset = table_dispatch::field_offset(
                    calling_program,
                    target_profile,
                    &applications,
                    storage,
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
            | ProviderBinding::CheckedAdapter { .. } => return Ok(None),
        };
        let mut calling_applications = selected.applications;
        let compatibility_policy = match &binding {
            ExternalBindingKind::Syscall { .. } => {
                CallingPolicy::native_syscall_for_target(native_target)
            }
            _ => Some(CallingPolicy::native_for_target(native_target)),
        };
        if calling_applications.is_empty()
            && let Some(policy) = compatibility_policy
        {
            calling_applications.extend(
                crate::calling_policy_plans::evaluate_default_calling_applications(
                    calling_program,
                    selected
                        .default_abis
                        .as_ref()
                        .map_err(|error| vec![error.clone()])?,
                    target_profile,
                    policy,
                    usize::from(matches!(binding, ExternalBindingKind::TableFunction { .. })),
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "cannot evaluate default native calling application for `{}::{}`: {reason}",
                        trait_name_text(&plan.schema),
                        row.method
                    ))]
                })?,
            );
        }
        Ok(Some(ExternalBindingRow {
            target_name: if plan.target.is_empty() {
                selected_target.unwrap_or("cross_platform_cli").to_owned()
            } else {
                plan.target.clone()
            },
            trait_name: trait_name_text(&plan.schema).to_owned(),
            method: row.method.clone(),
            requirement_identity: plan.identities.text(row.requirement_identity).to_owned(),
            provider_type: plan.provider_type.clone(),
            calling_applications,
            binding,
        }))
    }
}

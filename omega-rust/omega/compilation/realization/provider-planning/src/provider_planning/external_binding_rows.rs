// We extract normalized external binding rows (`Import`, `Syscall`,
// `VtableField`, `TableFunction`) and their evaluated Calling applications for
// compiler-to-native custody. Imports, kernel entries, and table fields rejoin
// the exact selected provider row; table field byte offsets are resolved
// through `field_offset` against the artifact's canonical storage namespace and
// foreign record layout rather than field spelling or equal shape.

use crate::calling_policy_plans::CallingProgram;
use crate::provider_planning::trait_name_text;
use diagnostics::Diagnostic;
use effects::provider_plan::TableFieldStorage;
use semantic_vocabulary::CallingAbiIdentity;
use terminal_psi::StructuralTypeShape;

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
    for plan in plans {
        for row_index in 0..plan.rows.len() {
            if let Some(row) = context.row(plan, row_index)? {
                rows.push(row);
            }
        }
    }
    Ok(rows)
}

pub(crate) struct ExternalBindingContext<'a> {
    pub selected_target: Option<&'a str>,
    pub native_target: target::NativeTarget,
    pub boundary_calling_plan_realizations:
        &'a [crate::calling_policy_plans::BoundaryCallingPlanRealization],
    pub calling_program: &'a crate::calling_policy_plans::CallingProgram,
}

impl ExternalBindingContext<'_> {
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
            ProviderBinding::Import { evaluated } => ExternalBindingKind::Import {
                locator: evaluated.locator().clone(),
            },
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
                let byte_offset =
                    field_offset(calling_program, target_profile, &applications, storage).map_err(
                        |reason| {
                            vec![Diagnostic::error(format!(
                                "cannot realize table field `{field}` for `{}`: {reason}",
                                row.method
                            ))]
                        },
                    )?;

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

fn field_offset(
    program: &CallingProgram,
    target: target::TargetProfile,
    applications: &[&CallingAbiIdentity],
    storage: &Result<TableFieldStorage, String>,
) -> Result<u32, String> {
    let storage = storage.as_ref().map_err(Clone::clone)?;
    let module = program.artifact.semantic_module();
    let mut records = module.structural_types.iter().filter(|record| {
        module.identities.text(record.identity) == Some(storage.table_application.as_str())
    });
    let record = records
        .next()
        .ok_or("dispatch table has no canonical storage application")?;
    if records.next().is_some() {
        return Err("dispatch table has competing canonical storage applications".into());
    }
    let StructuralTypeShape::Record { fields, .. } = &record.shape else {
        return Err("dispatch table does not have record storage".into());
    };
    let native = module
        .native_layouts
        .record(record.id)
        .ok_or("dispatch table lost its nominal field correspondence")?;
    let mut candidates = module
        .native_layouts
        .fields(native)?
        .iter()
        .filter(|field| {
            module.identities.text(field.declaration_identity)
                == Some(storage.field_declaration.as_str())
        });
    let field = candidates
        .next()
        .ok_or("dispatch field has no canonical declaration correspondence")?;
    if candidates.next().is_some() {
        return Err("dispatch field has competing canonical declaration correspondences".into());
    }
    let declaration = fields
        .iter()
        .find(|candidate| candidate.id == field.field && !candidate.relevance.is_erased())
        .ok_or("dispatch field is not a live field of the exact table")?;
    if declaration.field_type.atomic_scalar_type().is_some()
        || !matches!(declaration.field_type.scalar_type(),
        Some(semantic_vocabulary::ScalarType::Integer(integer)) if integer.is_address())
    {
        return Err("table dispatch field must have type addr".into());
    }
    let mut selected_offset = None;
    for application in applications {
        let signature = module
            .calling_abi_signature(application.id())
            .ok_or("table dispatch has no original semantic ABI")?;
        if signature.identity(&module.identities)? != **application {
            return Err("table dispatch differs from its full selected ABI key".into());
        }
        let parameter = signature
            .storage
            .as_ref()
            .map_err(Clone::clone)?
            .parameters
            .first()
            .ok_or("table dispatch has no first semantic formal")?;
        if parameter.structural_type != record.id
            || !matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            )
        {
            return Err(
                "table dispatch requires the exact readable table borrow as semantic formal zero"
                    .into(),
            );
        }
        let offset = program.describe_borrowed_storage(application, target, 0, |extents| {
            let mut recipes =
                layout::TerminalBoundaryRecipes::new(module, target.target_name(), extents);
            let (recipe, ordinal) = recipes.record_field(record.id, field.field)?;
            let mut layout =
                layout::BoundaryValueLayout::new(layout::LayoutTarget::for_profile(target)?);
            let (_, root) = layout.value_type(recipes.recipes(), recipe)?;
            let (shapes, fields, _) = layout.into_parts();
            let calling_conventions::BoundaryValueClass::Record {
                first_field,
                field_count,
            } = shapes[usize::from(root)].class
            else {
                return Err("dispatch table does not have a foreign record layout".into());
            };
            if ordinal >= usize::from(field_count) {
                return Err("dispatch field is absent from the foreign record layout".into());
            }
            fields
                .get(usize::from(first_field) + ordinal)
                .map(|field| u32::from(field.byte_offset))
                .ok_or_else(|| "dispatch field lost its physical layout coordinate".into())
        })?;
        if selected_offset.is_some_and(|prior| prior != offset) {
            return Err("selected table ABIs disagree on their dispatch field offset".into());
        }
        selected_offset = Some(offset);
    }
    selected_offset.ok_or_else(|| "table dispatch has no selected semantic ABI application".into())
}

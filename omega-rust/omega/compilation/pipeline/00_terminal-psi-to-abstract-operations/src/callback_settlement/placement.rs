//! Join canonical callback uses to target-owned inbound and registrar plans.
//! Source selection and contract refinement are Psi's work. This operation
//! consumes their portable result, equally for fresh and recovered artifacts.

use super::CallbackTerminalOperations;
use backend_plan::{
    BoundCallbackPrivateMaterialization, NativeCallbackEntryParameter, NativeCallbackPlacement,
    PrivateCallbackDestinationApplication,
};
use calling_conventions::{
    BoundaryCallingApplication, BoundaryNativeParameterGeometry, BoundaryNativeParameterOrigin,
    BoundaryNativeParameterShape, NativeParameterApplication, NativePlace,
    TerminalCallingTelescope, ValueShape,
};
use terminal_psi::{TerminalCallbackApplication, TerminalModule};

/// Place reached foreign callbacks using canonical declarations and this
/// target's selected calling applications. Works after releasing source and
/// on decoded artifacts; unselected candidates create no placement demand.
pub fn settle_native_callback_placements(
    module: &TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    calling: &[BoundaryCallingApplication],
) -> Result<Vec<NativeCallbackPlacement>, String> {
    let operations = CallbackTerminalOperations::new(
        module
            .callback_applications
            .iter()
            .map(|application| application.registrar),
        module.operations(),
    );
    let mut placements = Vec::new();
    for application in &module.callback_applications {
        let operation = operations.find(application.registrar).map_err(|count| {
            format!("callback registrar resolves to {count} Terminal operations; exactly one is required")
        })?;
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            return Err("callback registrar is not a boundary operation".into());
        };
        let declaration = module
            .boundary_machine(boundary)
            .ok_or("callback registrar has no boundary declaration")?;
        let requirement = module
            .identities
            .text(declaration.identity)
            .ok_or("callback registrar has no boundary identity")?;
        let Some((selected_plan, row)) = selected.selected_requirement_row(requirement) else {
            // A canonical module can retain other targets and unselected
            // candidates. Selection creates placement demand; actual executable
            // boundary admission still refuses any reached unsatisfied call.
            continue;
        };
        if matches!(
            row.binding,
            effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
        ) {
            // Forwarding selects an ordinary checked body, not a private native
            // entry. Occurrence settlement rejoins that body's exact identity.
            continue;
        }
        if !application.entry_admitted {
            return Err(
                "foreign callback registration requires an explicit selected callback calling plan"
                    .into(),
            );
        }
        let inbound_boundary = application
            .boundary
            .ok_or("admitted callback lost its exact inbound declaration")?;
        let inbound = calling_application(
            calling,
            &module.calling_abi_identity(inbound_boundary)?,
            None,
        )?;
        if inbound.dispatch_only_parameter_count != 0 {
            return Err("callback inbound entry cannot omit dispatch-only formals".into());
        }
        let registrar_abi = module.calling_abi_identity(boundary)?;
        let selected_calling = selected_plan
            .schema
            .calling_application(&registrar_abi)
            .ok_or(
                "callback registrar lost its independently selected exact calling application",
            )?;
        let registrar = calling_application(calling, &registrar_abi, Some(selected_calling))?;
        let inbound_telescope =
            TerminalCallingTelescope::for_application(module, inbound_boundary, inbound)?;
        let registrar_telescope =
            TerminalCallingTelescope::for_application(module, boundary, registrar)?;
        let entry_parameters = inbound_telescope
            .parameters
            .iter()
            .map(|parameter| {
                Ok(NativeCallbackEntryParameter {
                    application: parameter_application(inbound, parameter)?,
                    origin: parameter.origin,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        placements.push(NativeCallbackPlacement {
            application_index: application.application_index,
            static_machine_ordinal: application.static_machine_ordinal,
            canonical_requirement_overload: application.requirement.clone(),
            contract_commitment: application.body.contract,
            boundary_entry_plan: inbound.plan.plan().clone(),
            entry_parameters,
            private_materialization: private_materialization(
                application,
                registrar,
                &registrar_telescope,
            )?,
        });
    }
    Ok(placements)
}

fn calling_application<'plans>(
    calling: &'plans [BoundaryCallingApplication],
    abi: &semantic_vocabulary::CallingAbiIdentity,
    expected: Option<&effects::provider_plan::ServiceCallingPlan>,
) -> Result<&'plans BoundaryCallingApplication, String> {
    let mut matching = calling.iter().filter(|application| {
        &application.abi == abi
            && expected
                .is_none_or(|expected| application.commitment == expected.commitment.as_bytes())
    });
    let requirement = &abi.requirement_identity;
    match (matching.next(), matching.next()) {
        (Some(application), None)
            if expected.is_none_or(|expected| {
                expected.report_fingerprint == application.report_fingerprint
            }) =>
        {
            Ok(application)
        }
        _ => Err(format!(
            "callback requirement `{requirement}` has no unique target calling application"
        )),
    }
}

fn parameter_application(
    calling: &BoundaryCallingApplication,
    parameter: &BoundaryNativeParameterGeometry,
) -> Result<NativeParameterApplication, String> {
    let ordinal = calling
        .call_projection()?
        .parameter_ordinal(parameter.native_ordinal)?
        .ok_or("dispatch-only formal cannot carry a native callback")?;
    let placement = calling
        .plan
        .plan()
        .call
        .parameters
        .get(ordinal)
        .ok_or("callback native parameter has no plan placement")?;
    Ok(NativeParameterApplication {
        parameter: parameter.identity,
        native_ordinal: u32::try_from(ordinal).map_err(|_| "callback wire ordinal exceeds u32")?,
        shape: placement.shape,
        placement: placement.clone(),
    })
}

fn private_materialization(
    application: &TerminalCallbackApplication,
    registrar: &BoundaryCallingApplication,
    telescope: &TerminalCallingTelescope,
) -> Result<Option<BoundCallbackPrivateMaterialization>, String> {
    let mut matching = telescope.binders.iter().filter(|binding| {
        binding.static_machine_ordinal == application.static_machine_ordinal
            && binding.requirement_identity == application.requirement
    });
    let (Some(binding), None) = (matching.next(), matching.next()) else {
        return Err(
            "callback static ordinal and exact requirement have no unique registrar binder".into(),
        );
    };
    let plan = registrar.plan.plan();
    if !registrar.callback_context_closed {
        if !plan.call.callback_materializations.is_empty() {
            return Err("unclosed registrar retains private callback materializations".into());
        }
        return Ok(None);
    }
    let mut rows = plan
        .call
        .callback_materializations
        .iter()
        .filter(|row| row.binder == binding.binder);
    let (Some(row), None) = (rows.next(), rows.next()) else {
        return Err("callback binder has no unique private destination".into());
    };
    let mut demands = registrar.callback_context.demands.iter().filter(|demand| {
        demand.destination == row.destination && demand.requirement == binding.requirement
    });
    let (Some(demand), None) = (demands.next(), demands.next()) else {
        return Err("callback binder destination has no unique exact requirement demand".into());
    };
    let parameter = match row.destination {
        NativePlace::Parameter(parameter) | NativePlace::Field { parameter, .. } => parameter,
    };
    let mut parameters = telescope
        .parameters
        .iter()
        .filter(|candidate| candidate.identity == parameter);
    let (Some(parameter), None) = (parameters.next(), parameters.next()) else {
        return Err("callback destination has no unique native parameter".into());
    };
    let destination = parameter_application(registrar, parameter)?;
    let destination_application = match &row.destination {
        NativePlace::Parameter(_) => {
            if parameter.origin
                != (BoundaryNativeParameterOrigin::PrivateCallback {
                    binder: binding.binder,
                    requirement: binding.requirement,
                })
            {
                return Err("callback destination changed its binder or requirement".into());
            }
            let BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            } = parameter.shape
            else {
                return Err(
                    "direct callback destination lacks target function-pointer geometry".into(),
                );
            };
            if destination.shape != ValueShape::integer(u32::from(byte_size), alignment) {
                return Err("callback destination geometry differs from its plan placement".into());
            }
            PrivateCallbackDestinationApplication::Direct {
                application: destination,
            }
        }
        NativePlace::Field { .. } => {
            let catalog = registrar
                .signature
                .callback_layouts
                .span(registrar.signature.callback_layout_rows)
                .ok_or("registrar has stale callback layout storage")?;
            let mut layouts = catalog
                .iter()
                .filter(|layout| layout.destination == row.destination);
            let (Some(layout), None) = (layouts.next(), layouts.next()) else {
                return Err("callback destination has no unique named layout".into());
            };
            if layout.native_ordinal != parameter.native_ordinal {
                return Err("callback layout differs from its native parameter ordinal".into());
            }
            PrivateCallbackDestinationApplication::Field {
                root: destination,
                byte_offset: layout.field_offset(registrar.target)?,
            }
        }
    };
    Ok(Some(BoundCallbackPrivateMaterialization {
        binder: binding.binder,
        destination: row.destination.clone(),
        requirement: demand.requirement,
        registrar_boundary_entry_plan: plan.clone(),
        registrar_calling_plan_report_fingerprint: registrar.plan.contract_report_fingerprint(),
        registrar_application_report_fingerprint: registrar.report_fingerprint,
        registrar_application_commitment: registrar.commitment,
        registrar_application: registrar.clone(),
        destination_application,
        context: registrar.callback_context.clone(),
        registrar_invokes_callback: binding.invokes_synchronously,
    }))
}

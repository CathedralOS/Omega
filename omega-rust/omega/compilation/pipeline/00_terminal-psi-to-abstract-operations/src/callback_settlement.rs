/*
  Native callback placement and registrar-import settlement
  (`settle_native_callback_placements`, `settle_installed_callback_placements`,
  `settle_callback_registrar_imports`).

  Rejoins each compiled callback artifact's Terminal interface to its selected
  calling application telescope and pairs ordered registrar occurrences with
  their outbound calling plans and selected import rows.
*/
mod operations {
    use semantic_vocabulary::OperationId;
    use terminal_psi::Operation;

    struct RequestedCallbackOperation<'terminal> {
        identity: OperationId,
        first_match: Option<&'terminal Operation>,
        match_count: usize,
    }

    pub(crate) struct CallbackTerminalOperations<'terminal> {
        requested: Vec<RequestedCallbackOperation<'terminal>>,
    }

    impl<'terminal> CallbackTerminalOperations<'terminal> {
        pub(crate) fn new(
            operation_ids: impl Iterator<Item = OperationId>,
            operations: impl Iterator<Item = &'terminal Operation>,
        ) -> Self {
            let mut requested = operation_ids
                .map(|identity| RequestedCallbackOperation {
                    identity,
                    first_match: None,
                    match_count: 0,
                })
                .collect::<Vec<_>>();
            requested.sort_unstable_by_key(|entry| entry.identity);
            requested.dedup_by_key(|entry| entry.identity);
            if !requested.is_empty() {
                for operation in operations {
                    if let Ok(position) =
                        requested.binary_search_by_key(&operation.id, |entry| entry.identity)
                    {
                        let entry = &mut requested[position];
                        entry.first_match.get_or_insert(operation);
                        entry.match_count += 1;
                    }
                }
            }
            Self { requested }
        }

        pub(crate) fn find(&self, operation: OperationId) -> Result<&'terminal Operation, usize> {
            let position = self
                .requested
                .binary_search_by_key(&operation, |entry| entry.identity)
                .map_err(|_| 0usize)?;
            let entry = &self.requested[position];
            match (entry.match_count, entry.first_match) {
                (1, Some(operation)) => Ok(operation),
                (match_count, _) => Err(match_count),
            }
        }
    }
}
mod placement {
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

    pub fn settle_native_callback_placements(
        module: &TerminalModule,
        targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
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
                continue;
            };
            if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            ) {
                continue;
            }
            if !application.entry_admitted {
                return Err(
                "foreign callback registration requires an explicit selected callback calling plan"
                    .into(),
            );
            }
            let (inbound_boundary, inbound) =
                inbound_application(module, targets, calling, application)?;
            let registrar_boundary =
                ::provider_admission::selected_calling_boundary(module, targets, boundary)?;
            let registrar_abi = module.calling_abi_identity(registrar_boundary)?;
            let selected_calling = selected_plan
                .schema
                .calling_application(&registrar_abi)
                .ok_or(
                    "callback registrar lost its independently selected exact calling application",
                )?;
            let registrar =
                calling_application(calling, &registrar_abi, None, Some(selected_calling))?;
            placements.push(place_callback(
                module,
                application,
                inbound_boundary,
                inbound,
                registrar_boundary,
                registrar,
            )?);
        }
        Ok(placements)
    }

    pub(super) fn inbound_application<'plans>(
        module: &TerminalModule,
        targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
        calling: &'plans [BoundaryCallingApplication],
        application: &TerminalCallbackApplication,
    ) -> Result<
        (
            semantic_vocabulary::BoundaryMachineId,
            &'plans BoundaryCallingApplication,
        ),
        String,
    > {
        let boundary = application
            .boundary
            .ok_or("admitted callback lost its exact inbound declaration")?;
        let boundary = ::provider_admission::selected_calling_boundary(module, targets, boundary)?;
        let inbound = calling_application(
            calling,
            &module.calling_abi_identity(boundary)?,
            Some(&application.calling_owner_application),
            None,
        )?;
        if inbound.dispatch_only_parameter_count != 0 {
            return Err("callback inbound entry cannot omit dispatch-only formals".into());
        }
        Ok((boundary, inbound))
    }

    pub(super) fn place_callback(
        module: &TerminalModule,
        application: &TerminalCallbackApplication,
        inbound_boundary: semantic_vocabulary::BoundaryMachineId,
        inbound: &BoundaryCallingApplication,
        registrar_boundary: semantic_vocabulary::BoundaryMachineId,
        registrar: &BoundaryCallingApplication,
    ) -> Result<NativeCallbackPlacement, String> {
        let inbound_telescope =
            TerminalCallingTelescope::for_application(module, inbound_boundary, inbound)?;
        let registrar_telescope =
            TerminalCallingTelescope::for_application(module, registrar_boundary, registrar)?;
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
        Ok(NativeCallbackPlacement {
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
        })
    }

    pub(super) fn calling_application<'plans>(
        calling: &'plans [BoundaryCallingApplication],
        abi: &semantic_vocabulary::CallingAbiIdentity,
        calling_owner: Option<&str>,
        expected: Option<&effects::provider_plan::ServiceCallingPlan>,
    ) -> Result<&'plans BoundaryCallingApplication, String> {
        let mut matching = calling.iter().filter(|application| {
            &application.abi == abi
                && calling_owner.is_none_or(|owner| {
                    application.calling_owner_application.as_deref() == Some(owner)
                })
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
            native_ordinal: u32::try_from(ordinal)
                .map_err(|_| "callback wire ordinal exceeds u32")?,
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
                "callback static ordinal and exact requirement have no unique registrar binder"
                    .into(),
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
            return Err(
                "callback binder destination has no unique exact requirement demand".into(),
            );
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
                    return Err(
                        "callback destination geometry differs from its plan placement".into(),
                    );
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
                    placements: calling_conventions::PrivateCallbackPlacements::new(
                        &layout.field_placements(registrar.target)?,
                    ),
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
}
pub use placement::settle_native_callback_placements;
mod installed_placement {
    use backend_plan::NativeCallbackPlacement;
    use calling_conventions::{BoundaryCallingApplication, NativePlace};
    use language_semantics::callback_declarations::PrivateCallbackSlotSelection;
    use terminal_psi::{TerminalInstalledInterruptRegistration, TerminalModule};

    use super::placement::{calling_application, inbound_application, place_callback};

    pub fn settle_installed_callback_placements(
        module: &TerminalModule,
        target_profile: target::TargetProfile,
        targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
        calling: &[BoundaryCallingApplication],
    ) -> Result<Vec<NativeCallbackPlacement>, String> {
        let mut placements = Vec::new();
        for registration in &module.installed_interrupt_registrations {
            let registrar_boundary = ::provider_admission::selected_calling_boundary(
                module,
                targets,
                registration.registrar_boundary,
            )?;
            let registrar = calling_application(
                calling,
                &module.calling_abi_identity(registrar_boundary)?,
                Some(&registration.registrar_application),
                None,
            )?;
            if registrar.target != target_profile.native_target() {
                return Err("installed registrar calling plan belongs to another target".into());
            }
            let mut timer_destinations = 0usize;
            for callback in &registration.callbacks {
                if callback.registrar != registration.operation || !callback.entry_admitted {
                    return Err(
                        "installed callback lost its checked occurrence or entry contract".into(),
                    );
                }
                let (inbound_boundary, inbound) =
                    inbound_application(module, targets, calling, callback)?;
                if inbound.target != registrar.target {
                    return Err(
                        "installed callback entry and registrar plans select different targets"
                            .into(),
                    );
                }
                let placement = place_callback(
                    module,
                    callback,
                    inbound_boundary,
                    inbound,
                    registrar_boundary,
                    registrar,
                )?;
                let materialization = placement
                    .private_materialization
                    .as_ref()
                    .ok_or("installed callback has no complete private materialization")?;
                if let NativePlace::Field { .. } = materialization.destination {
                    let layouts = registrar
                        .signature
                        .callback_layouts
                        .span(registrar.signature.callback_layout_rows)
                        .ok_or("installed registrar lost its callback layout catalog")?;
                    let mut matching = layouts
                        .iter()
                        .filter(|layout| layout.destination == materialization.destination);
                    let (Some(layout), None) = (matching.next(), matching.next()) else {
                        return Err("installed callback has no unique destination layout".into());
                    };
                    let slot = calling_conventions::callback_destination_slot(
                        module,
                        target_profile,
                        registrar,
                        layout,
                    )?;
                    if slot.application_commitment == registration.slot_commitment {
                        require_timer_slot(registration, slot)?;
                        timer_destinations += 1;
                    }
                }
                placements.push(placement);
            }
            if timer_destinations != 1 {
                return Err(format!(
                    "installation timer slot resolves to {timer_destinations} callback destinations; exactly one is required"
                ));
            }
        }
        Ok(placements)
    }

    fn require_timer_slot(
        registration: &TerminalInstalledInterruptRegistration,
        slot: &PrivateCallbackSlotSelection,
    ) -> Result<(), String> {
        if slot.application_encoding.as_ref() != registration.slot_encoding.as_slice() {
            return Err("installed timer destination changed its exact slot application".into());
        }
        Ok(())
    }
}
pub use installed_placement::settle_installed_callback_placements;
mod registrar_imports {
    use super::CallbackTerminalOperations;
    use backend_plan::NativeCallbackPlacement;
    use calling_conventions::BoundaryCallingApplication;
    use calling_conventions::ExternalBindingRow;
    use semantic_vocabulary::OperationId;
    use terminal_psi::TerminalModule;

    #[derive(Clone, Copy)]
    pub(crate) struct CallbackRegistrarOccurrence {
        pub placement_index: usize,
        pub terminal_operation: OperationId,
    }

    pub(crate) fn settle_callback_registrar_imports(
        terminal_module: &TerminalModule,
        targets: Option<&terminal_installation::AdmittedTargetMachineSelections>,
        selected: &effects::SelectedProviderPlanFacts,
        callback_placements: &[NativeCallbackPlacement],
        callback_occurrences: impl Iterator<Item = CallbackRegistrarOccurrence> + Clone,
        calling_applications: &[BoundaryCallingApplication],
        external_binding_rows: &[ExternalBindingRow],
    ) -> Result<Vec<ExternalBindingRow>, String> {
        let mut rows = external_binding_rows.to_vec();
        if callback_occurrences.clone().next().is_none() {
            return Ok(rows);
        }
        let operations = CallbackTerminalOperations::new(
            callback_occurrences
                .clone()
                .map(|occurrence| occurrence.terminal_operation),
            terminal_module.operations(),
        );
        let declarations = terminal_module
            .boundary_machines
            .iter()
            .map(|declaration| {
                (
                    declaration.id,
                    terminal_module
                        .identities
                        .text(declaration.identity)
                        .unwrap_or_default(),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();

        for occurrence in callback_occurrences {
            let placement = callback_placements
                .get(occurrence.placement_index)
                .ok_or_else(|| {
                    "callback registrar import closure names an absent retained placement"
                        .to_owned()
                })?;
            let materialization = placement.private_materialization.as_ref().ok_or_else(|| {
                "callback registrar import closure requires one private materialization".to_owned()
            })?;
            let operation = operations.find(occurrence.terminal_operation).map_err(|count| {
            format!(
                "callback registrar import closure resolves Terminal operation {} to {count} operations",
                occurrence.terminal_operation.get(),
            )
        })?;
            let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
                return Err(
                    "callback registrar import closure names a non-boundary Terminal operation"
                        .to_owned(),
                );
            };
            let requirement = declarations.get(boundary).copied().ok_or_else(|| {
                "callback registrar import closure names an absent Terminal boundary".to_owned()
            })?;
            let selected_requirement = selected.selected_requirement_identity(requirement);
            let calling_boundary = ::provider_admission::selected_calling_boundary(
                terminal_module,
                targets,
                *boundary,
            )?;
            let abi = terminal_module.calling_abi_identity(calling_boundary)?;

            let matching_realizations = calling_applications
                .iter()
                .filter(|realization| {
                    realization.abi == abi
                        && realization.callback_context_closed
                        && realization == &&materialization.registrar_application
                })
                .collect::<Vec<_>>();
            let [_] = matching_realizations.as_slice() else {
                return Err(format!(
                    "callback registrar `{requirement}` rejoins {} exact target-closed calling-plan realizations",
                    matching_realizations.len(),
                ));
            };
            let mut matching_rows = rows
                .iter_mut()
                .filter(|row| {
                    row.requirement_identity == selected_requirement
                        && matches!(
                            row.binding,
                            calling_conventions::ExternalBindingKind::Import { .. }
                        )
                })
                .collect::<Vec<_>>();
            if matching_rows.is_empty() {
                continue;
            }
            let [row] = matching_rows.as_mut_slice() else {
                return Err(format!(
                    "callback registrar `{requirement}` rejoins {} retained external import rows",
                    matching_rows.len(),
                ));
            };
            let mut applications = row
                .calling_applications
                .iter_mut()
                .filter(|application| application.abi == abi);
            let (Some(application), None) = (applications.next(), applications.next()) else {
                return Err(
                    "callback registrar external row lost its exact ABI application".into(),
                );
            };
            *application = materialization.registrar_application.clone();
        }

        Ok(rows)
    }
}
pub(crate) use operations::CallbackTerminalOperations;

pub(crate) use registrar_imports::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
};

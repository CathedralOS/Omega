//! Lower each settled callback thunk's canonical Terminal artifact to machine
//! objects inside the same realization as the semantic program.
//!
//! The settlement carries a Terminal artifact, a caller-scoped private function
//! identity and a boundary entry plan. Every thunk follows the ordinary native
//! pipeline. The object linker retains the complete local closure under the
//! caller-scoped identity, not a synthetic single-function byte projection.

use crate::request::NativeRealizationRequest;
mod installed_entry_plans;
use backend_plan::NativeCallbackThunkSettlement;
use diagnostics::Diagnostic;
use image_emission::PrivateFunctionObject;
use installed_entry_plans::PreparedEntryPlans;
use terminal_psi_to_abstract_operations::admit_prepared_native_realization;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;

/// Installed handlers are emitted before their installer needs a numeric
/// stack guard. Final closure emission consumes these same objects, so their
/// bodies are neither recompiled nor emitted twice.
pub(crate) struct PreparedCallbackObjects {
    objects: Vec<Option<PrivateFunctionObject>>,
    active: Vec<bool>,
    settlement_positions: Vec<Option<usize>>,
    installed_plans: PreparedEntryPlans,
}

impl PreparedCallbackObjects {
    pub fn new(request: &NativeRealizationRequest<'_>) -> Result<Self, Vec<Diagnostic>> {
        let placement_count = request.callback_thunks.len();
        let mut settlement_positions = vec![None; placement_count];
        for (position, row) in request.callback_thunks.iter().enumerate() {
            let Some(slot) = settlement_positions.get_mut(row.placement_index) else {
                return Err(realization_error(
                    "callback body closure",
                    "placement falls outside complete original roster",
                ));
            };
            if slot.replace(position).is_some() {
                return Err(realization_error(
                    "callback body closure",
                    "original callback placement is duplicated",
                ));
            }
        }
        Ok(Self {
            objects: std::iter::repeat_with(|| None)
                .take(placement_count)
                .collect(),
            active: vec![false; placement_count],
            settlement_positions,
            installed_plans: PreparedEntryPlans::new(placement_count),
        })
    }

    fn realize<'a>(
        &'a mut self,
        request: &NativeRealizationRequest<'_>,
        settlement: &NativeCallbackThunkSettlement<'_>,
    ) -> Result<&'a PrivateFunctionObject, Vec<Diagnostic>> {
        let index = settlement.placement_index;
        let Some(known) = self.objects.get(index) else {
            return Err(realization_error(
                "callback body closure",
                "placement falls outside the complete original roster",
            ));
        };
        if let Some(known) = known {
            if known.identity != settlement.callback_function
                || known.body.entry() != settlement.entry
            {
                return Err(realization_error(
                    "callback body closure",
                    "one placement selects incompatible private bodies",
                ));
            }
        } else {
            if self.active[index] {
                return Err(realization_error(
                    "installed handler stack",
                    "installer and handler have a cyclic executable resource dependency",
                ));
            }
            self.active[index] = true;
            let result = lower_callback_thunk(request, settlement, self);
            self.active[index] = false;
            self.objects[index] = Some(result?);
        }
        self.objects[index].as_ref().ok_or_else(|| {
            realization_error(
                "callback body closure",
                "private body realization did not retain its object",
            )
        })
    }
}

pub(crate) fn prepare_installed_callbacks(
    request: &NativeRealizationRequest<'_>,
    target: &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    objects: &mut PreparedCallbackObjects,
    physical_entry: Option<installation_evidence::InstalledPhysicalStackRoot>,
) -> Result<Vec<installation_evidence::InstalledInterruptStackRequirement>, Vec<Diagnostic>> {
    let cohort = target.lowering_facts().installed_interrupts();
    let mut operations: Vec<_> = target
        .target_operations()
        .functions
        .iter()
        .flat_map(|function| function.graph.operations())
        .filter_map(|operation| operation.installation_operation())
        .collect();
    operations.sort_unstable();
    operations.dedup();
    let mut entries: Vec<_> = cohort.entries().iter().collect();
    entries.sort_unstable_by_key(|entry| (entry.operation, entry.placement.application_index));
    if entries.windows(2).any(|pair| {
        (pair[0].operation, pair[0].placement.application_index)
            == (pair[1].operation, pair[1].placement.application_index)
    }) {
        return Err(realization_error(
            "installed handler stack",
            "installation application is duplicated",
        ));
    }
    let mut requirements = Vec::new();
    for scope in cohort
        .scopes()
        .iter()
        .filter(|scope| operations.binary_search(&scope.operation()).is_ok())
    {
        let first = entries.partition_point(|entry| entry.operation < scope.operation());
        let last = entries.partition_point(|entry| entry.operation <= scope.operation());
        let scope_entries = &entries[first..last];
        objects.installed_plans.retain(scope, scope_entries)?;
        let first_requirement = requirements.len();
        for entry in scope_entries {
            let settlement = objects
                .settlement_positions
                .get(entry.placement_index)
                .copied()
                .flatten()
                .and_then(|position| request.callback_thunks.get(position))
                .filter(|row| {
                    row.callback_function == entry.callback_function
                        && row.terminal_operation == entry.operation
                })
                .ok_or_else(|| {
                    realization_error(
                        "installed handler stack",
                        "arrival has no matching retained body and entry plan",
                    )
                })?;
            let object = objects.realize(request, settlement)?;
            let requirement =
                image_emission::installed_interrupt_stack_requirement(object, entry.operation)
                    .map_err(|error| realization_error("installed handler stack", error))?;
            requirements.push(requirement);
        }
        image_emission::compose_installed_arrival_demands(
            scope,
            scope_entries,
            &mut requirements[first_requirement..],
            physical_entry,
        )
        .map_err(|error| realization_error("installed arrival stack", error))?;
    }
    Ok(requirements)
}

/// Realize the executable scope's callback thunks into compiler-private code,
/// preserving original placement identities and relative request order.
///
/// Each thunk selects an entry in the shared Terminal namespace. Admission
/// uses the same prepared providers and independent receiving policy without
/// manufacturing a ProgramEntry for the private body.
pub(crate) fn lower_callback_thunks(
    request: &NativeRealizationRequest<'_>,
    native_callbacks: &[terminal_psi_to_abstract_operations::AdmittedNativeCallbackArgument],
    installed_interrupts: &terminal_psi_to_abstract_operations::provider_admission::AdmittedInstalledInterruptCohort,
    root: &image_emission::ObjectArtifact,
    mut prepared_objects: PreparedCallbackObjects,
) -> Result<Vec<PrivateFunctionObject>, Vec<Diagnostic>> {
    if installed_interrupts.psi() != root.psi() {
        return Err(realization_error(
            "installed interrupt callback emission",
            "admitted entries and emitted object belong to different Terminal programs",
        ));
    }
    let mut pending = request
        .callback_thunks
        .iter()
        .filter(|settlement| {
            root.object()
                .layout
                .function_symbols
                .iter()
                .any(|(_, reference)| reference.identity == settlement.callback_function)
                && (native_callbacks.iter().any(|callback| {
                    callback.terminal_operation == settlement.terminal_operation
                        && callback.placement_index == settlement.placement_index
                        && callback.callback_function == settlement.callback_function
                }) || installed_interrupts.entries().iter().any(|entry| {
                    entry.operation == settlement.terminal_operation
                        && entry.placement_index == settlement.placement_index
                        && entry.callback_function == settlement.callback_function
                }))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut objects = Vec::new();
    let mut next_callback = 0;
    while next_callback < pending.len() {
        let settlement = &pending[next_callback];
        let object = prepared_objects.realize(request, settlement)?;
        for child in request.callback_thunks.iter().copied() {
            if !object
                .body
                .object()
                .layout
                .function_symbols
                .iter()
                .any(|(_, reference)| reference.identity == child.callback_function)
            {
                continue;
            }
            if let Some(known) = pending
                .iter()
                .find(|known| known.callback_function == child.callback_function)
            {
                if known.artifact.manifest().identity() != child.artifact.manifest().identity()
                    || known.entry != child.entry
                    || known.boundary_entry_plan != child.boundary_entry_plan
                    || known.parameter_formals != child.parameter_formals
                    || known.entry_parameters != child.entry_parameters
                {
                    return Err(realization_error(
                        "callback body closure",
                        "one private identity selects incompatible bodies or entry plans",
                    ));
                }
            } else {
                pending.push(child);
            }
        }
        next_callback += 1;
    }
    for settlement in pending {
        let object = prepared_objects.objects[settlement.placement_index]
            .take()
            .ok_or_else(|| {
                realization_error(
                    "callback body closure",
                    "reachable private body was not retained",
                )
            })?;
        objects.push(object);
    }
    Ok(objects)
}

fn lower_callback_thunk(
    request: &NativeRealizationRequest<'_>,
    settlement: &NativeCallbackThunkSettlement<'_>,
    prepared_objects: &mut PreparedCallbackObjects,
) -> Result<PrivateFunctionObject, Vec<Diagnostic>> {
    let admission = terminal_psi_to_abstract_operations::NativeAdmissionRequest {
        private_entry_ram: &[],
        entry: terminal_psi::TargetMachineCallable::Machine(settlement.entry),
        ..request.admission
    };
    let input = request.execution.lower_callable(
        settlement.artifact,
        request.proposal,
        admission.entry,
        request.scalar_applications.as_ref().map(|scalars| {
            terminal_psi_to_abstract_operations::NativeScalarApplications {
                completed: scalars.completed,
                provider_declarations: scalars.provider_declarations,
            }
        }),
    )?;
    let admitted = admit_prepared_native_realization(input, settlement.artifact, &admission)?;
    let cohort = admitted
        .input
        .optimization_input()
        .admitted_installed_interrupts();
    let installed_plan = prepared_objects
        .installed_plans
        .for_body(settlement, cohort)?;
    let mut prepared = super::callable_objects::prepare_callable_object(
        admitted,
        admission,
        request.optimization_selections,
        request.optimization_budget(),
        settlement.artifact,
    )?;
    if let Some((_, execution)) = &installed_plan {
        prepared.target = prepared
            .target
            .with_installed_entry_execution(execution.clone())
            .map_err(|reason| realization_error("installed execution", reason))?;
    }
    let requirements =
        prepare_installed_callbacks(request, &prepared.target, prepared_objects, None)?;
    prepared.target = prepared
        .target
        .with_installed_stack_requirements(requirements)
        .map_err(|reason| realization_error("installed handler stack", reason))?;
    let projection = super::callable_objects::emit_prepared_callable_object(prepared)?.projection;
    let emitted = projection.entry_call_plan()?;
    let declared = &settlement.boundary_entry_plan.call;
    // Join independent Terminal lanes through their exact nominal origins.
    // The body keeps its ordinary ABI; a differing admitted native recipe is
    // realized by a separate physical entry, not by rewriting helper/recursion
    // conventions or treating a thin scalar pointer as a descriptor.
    if emitted.parameters.len() != declared.parameters.len()
        || emitted.parameters.len() != settlement.parameter_formals.len()
        || settlement.entry_parameters.len() != declared.parameters.len()
    {
        return Err(realization_error(
            "callback entry ABI",
            "callback signature drifts from its boundary entry plan",
        ));
    }
    let inbound_ordinals = settlement
        .parameter_formals
        .iter()
        .map(|formal| {
            let mut matching = settlement.entry_parameters.iter().filter(|parameter| {
                parameter.origin
                    == calling_conventions::BoundaryNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: *formal,
                    }
            });
            let Some(parameter) = matching.next() else {
                return Err(realization_error(
                    "callback entry ABI",
                    "callback formal has no native parameter origin",
                ));
            };
            let boundary = usize::try_from(parameter.application.native_ordinal)
                .ok()
                .and_then(|ordinal| declared.parameters.get(ordinal));
            if matching.next().is_some()
                || boundary.is_none_or(|boundary| {
                    parameter.application.placement != *boundary
                        || parameter.application.shape != boundary.shape
                })
            {
                return Err(realization_error(
                    "callback entry ABI",
                    "callback native parameter application differs",
                ));
            }
            Ok(parameter.application.native_ordinal)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let body = std::sync::Arc::new(projection.into_object());
    if let Some((plan, _)) = installed_plan {
        // The installer must check this exact hardware frame before storing
        // the entry address. The bridge recipe is not evidence of that check.
        image_emission::create_installed_private_function_object(
            settlement.callback_function,
            body,
            settlement.boundary_entry_plan,
            &inbound_ordinals,
            plan,
        )
    } else {
        image_emission::create_private_function_object(
            settlement.callback_function,
            body,
            settlement.boundary_entry_plan,
            &inbound_ordinals,
        )
    }
    .map_err(|error| realization_error("callback entry ABI", error))
}

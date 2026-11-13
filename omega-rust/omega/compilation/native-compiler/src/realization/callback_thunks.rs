/*
Lower settled callback thunks and installed interrupt handlers to machine objects
inside the same realization as the semantic program.

Each settlement carries a canonical Terminal artifact, a caller-scoped private
function identity, and a boundary entry plan. Installed interrupt handlers are
emitted before their installer needs a numeric stack guard, and final closure
emission consumes those same retained objects so no body is compiled or emitted
twice.
*/

/*
`PreparedEntryPlans` carries the executing installation's admitted entry recipe
(timer acknowledgement frame or fatal-context gate offsets) into each private
handler body and verifies that reentry never changes the parent installation,
artifact, or callback association.
*/

use crate::request::NativeRealizationRequest;
use backend_plan::NativeCallbackThunkSettlement;
use diagnostics::Diagnostic;
use image_emission::PrivateFunctionObject;
use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};
use std::sync::Arc;
use terminal_psi_to_abstract_operations::admit_prepared_native_realization;
use terminal_psi_to_abstract_operations::provider_admission::{
    AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
    AdmittedInstalledInterruptScope, realization_error,
};

pub(crate) struct PreparedCallbackObjects {
    objects: Vec<Option<PrivateFunctionObject>>,
    active: Vec<bool>,
    settlement_positions: Vec<Option<usize>>,
    installed_plans: PreparedEntryPlans,
    program_provider_context: Arc<
        terminal_psi_to_abstract_operations::provider_admission::AdmittedProgramProviderContext,
    >,
}

impl PreparedCallbackObjects {
    pub fn new(
        request: &NativeRealizationRequest<'_>,
        program_provider_context: &Arc<
            terminal_psi_to_abstract_operations::provider_admission::AdmittedProgramProviderContext,
        >,
    ) -> Result<Self, Vec<Diagnostic>> {
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
            program_provider_context: program_provider_context.clone(),
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

pub(crate) fn lower_callback_thunks(
    request: &NativeRealizationRequest<'_>,
    native_callbacks: &[terminal_psi_to_abstract_operations::AdmittedNativeCallbackArgument],
    installed_interrupts: &AdmittedInstalledInterruptCohort,
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
        providers:
            terminal_psi_to_abstract_operations::provider_admission::ProviderAdmissionRequest {
                program_provider_context: Some(&prepared_objects.program_provider_context),
                ..request.admission.providers
            },
        ..request.admission
    };
    let (input, boundary_coverage) = terminal_psi_to_abstract_operations::lower_native_callable(
        request.execution,
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
    let admission = terminal_psi_to_abstract_operations::NativeAdmissionRequest {
        boundary_application_coverage: Some(&boundary_coverage),
        ..admission
    };
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
    let body = Arc::new(projection.into_object());
    if let Some((plan, _)) = installed_plan {
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

#[derive(Clone, PartialEq, Eq)]
struct PreparedEntry {
    scope: Arc<AdmittedInstalledInterruptScope>,
    entry: AdmittedInstalledInterruptEntry,
    plan: InstalledInterruptEntryPlan,
}

struct PreparedEntryPlans {
    entries: Vec<Option<PreparedEntry>>,
}

impl PreparedEntryPlans {
    fn new(count: usize) -> Self {
        Self {
            entries: vec![None; count],
        }
    }

    fn retain(
        &mut self,
        scope: &AdmittedInstalledInterruptScope,
        entries: &[&AdmittedInstalledInterruptEntry],
    ) -> Result<(), Vec<Diagnostic>> {
        let fail = |reason| realization_error("installed entry preparation", reason);
        let anchor = scope
            .context_anchor()
            .ok_or_else(|| fail("executing installation has no retained timer anchor"))?;
        let mut matching = entries
            .iter()
            .filter(|entry| entry.placement.application_index == anchor.application());
        let (Some(timer), None) = (matching.next(), matching.next()) else {
            return Err(fail("timer anchor has no unique admitted callback"));
        };
        if timer.callback_function != anchor.function()
            || timer.operation != scope.operation()
            || timer.placement.application_index != scope.acknowledgement_application()
        {
            return Err(fail(
                "timer anchor differs from its admitted installation entry",
            ));
        }
        let scope = Arc::new(scope.clone());
        for entry in entries {
            if entry.operation != scope.operation()
                || entry.entry().psi() != timer.entry().psi()
                || entry.entry().target() != timer.entry().target()
            {
                return Err(fail(
                    "installed entry changed its parent program, target or operation",
                ));
            }
            let plan = if entry.placement.application_index == scope.acknowledgement_application() {
                InstalledInterruptEntryPlan::new(InstalledInterruptFrame::NoErrorCode)
            } else {
                let mut matching = scope.fatal_entries().iter().filter(|fatal| {
                    fatal.application() == entry.placement.application_index
                        && fatal.machine() == entry.entry().machine()
                });
                let (Some(fatal), None) = (matching.next(), matching.next()) else {
                    return Err(fail("private entry has no exact admitted fatal handler"));
                };
                InstalledInterruptEntryPlan::fatal_context(
                    anchor.gate_byte_offset(),
                    fatal.gate_byte_offset(),
                    anchor.table_bytes(),
                )
                .ok_or_else(|| fail("fatal context has invalid retained gate geometry"))?
            };
            let retained = PreparedEntry {
                scope: Arc::clone(&scope),
                entry: (*entry).clone(),
                plan,
            };
            let slot = self
                .entries
                .get_mut(entry.placement_index)
                .ok_or_else(|| fail("installed placement exceeds the original callback roster"))?;
            if slot.as_ref().is_some_and(|prior| prior != &retained) {
                return Err(fail(
                    "one private placement has incompatible parent installation evidence",
                ));
            }
            *slot = Some(retained);
        }
        Ok(())
    }

    fn for_body(
        &self,
        settlement: &NativeCallbackThunkSettlement<'_>,
        cohort: &AdmittedInstalledInterruptCohort,
    ) -> Result<
        Option<(
            InstalledInterruptEntryPlan,
            terminal_psi_to_abstract_operations::AdmittedInstalledEntryExecution,
        )>,
        Vec<Diagnostic>,
    > {
        let fail = |reason| realization_error("installed private entry", reason);
        let mut matching = cohort
            .entries()
            .iter()
            .filter(|entry| entry.placement_index == settlement.placement_index);
        let entry = matching.next();
        if matching.next().is_some() {
            return Err(fail(
                "private body has duplicate admitted placement identities",
            ));
        }
        let retained = self
            .entries
            .get(settlement.placement_index)
            .ok_or_else(|| fail("private placement exceeds the original callback roster"))?;
        match (entry, retained) {
            (None, None) => Ok(None),
            (Some(entry), Some(retained))
                if *entry == retained.entry
                    && entry.entry().psi() == settlement.artifact.view().identity()
                    && entry.entry().machine() == settlement.entry
                    && entry.operation == settlement.terminal_operation
                    && entry.callback_function == settlement.callback_function
                    && retained.scope.operation() == entry.operation =>
            {
                let execution = terminal_psi_to_abstract_operations::AdmittedInstalledEntryExecution::for_entry(
                    Arc::clone(&retained.scope),
                    entry,
                    cohort,
                ).map_err(fail)?;
                Ok(Some((retained.plan, execution)))
            }
            _ => Err(fail(
                "private entry changed its original installation, artifact or callback association",
            )),
        }
    }
}

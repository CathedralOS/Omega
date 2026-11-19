// Close the retained target references before allocating any callback. Installed
// handlers supply their physical stack and fault summaries before allocation of
// the installer. Binding checks these measurements across the full program.

/*
`PreparedEntryPlans` carries the executing installation's admitted entry recipe
(timer acknowledgement frame or fatal-context gate offsets) into each private
handler body and verifies that reentry never changes the parent installation,
artifact, or callback association.
*/

use super::plan_callables::{CallableTargetProduct, PrivateEntryContract};
use diagnostics::Diagnostic;
use image_emission::PrivateFunctionObject;
use provider_admission::provider_admission::{
    AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
    AdmittedInstalledInterruptScope, realization_error,
};
use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};
use std::sync::Arc;

pub(crate) struct PreparedCallbackBodies<'a> {
    selected: Vec<Option<PrivateEntryContract<'a>>>,
    target_bodies: Vec<Option<CallableTargetProduct>>,
    order: Vec<usize>,
    bodies: Vec<Option<PlannedPrivateBody>>,
    after_root: Vec<usize>,
    installed_plans: PreparedEntryPlans,
}

pub(crate) struct PlannedPrivateBody {
    pub identity: function_identity::MachineFunctionIdentity,
    pub callable: super::callable_objects::CallablePhysicalProduct,
    pub entry: entry_planning::callable_entry::PlannedPrivateEntry,
    measurement: std::sync::OnceLock<
        Result<installation_binding::installed_handlers::MeasuredInstalledHandler, Vec<Diagnostic>>,
    >,
}

pub(super) type EncodedPrivateBody = PrivateBody<
    super::callable_objects::EncodedCallable,
    native_encoding::callable_entry::EncodedPrivateEntry,
>;
pub(super) type LaidOutPrivateBody = PrivateBody<
    super::callable_objects::LaidOutCallable,
    function_layout::entry_placement::PlacedPrivateEntry,
>;

pub(super) struct PrivateBody<Callable, Entry> {
    callable: Callable,
    entry: Entry,
}

pub(super) struct BoundPrivateBody {
    callable: super::callable_objects::BoundCallable,
    entry: installation_binding::entries::BoundPrivateEntry,
}
impl PlannedPrivateBody {
    fn measured_handler(
        &self,
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
    ) -> Result<&installation_binding::installed_handlers::MeasuredInstalledHandler, Vec<Diagnostic>>
    {
        let measured = self
            .measurement
            .get_or_init(|| {
                let bridge = self.entry.bridge().ok_or_else(|| {
                    realization_error(
                        "installed handler stack",
                        "installed handler has no arrival bridge",
                    )
                })?;
                installation_binding::installed_handlers::measure_installed_handler(
                    self.identity,
                    &self.callable.body.stage.program().frame,
                    self.callable.body.stage.facts(),
                    self.callable.body.stage.installations(),
                    bridge,
                    self.entry.cpu_state(),
                )
                .map_err(|error| realization_error("installed handler stack", error))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let row = measured.requirement();
        if (row.installation_machine, row.operation) != (machine, operation) {
            return Err(realization_error(
                "installed handler stack",
                "installed stack guard has no retained physical handler",
            ));
        }
        Ok(measured)
    }

    pub(super) fn bind(
        self,
        handlers: &installation_binding::RetainedHandlerMeasurements,
    ) -> Result<BoundPrivateBody, Vec<Diagnostic>> {
        Ok(BoundPrivateBody {
            callable: self.callable.bind(handlers)?,
            entry: installation_binding::entries::bind_private_entry(self.entry),
        })
    }
}
impl BoundPrivateBody {
    pub(super) fn encode(self) -> Result<EncodedPrivateBody, Vec<Diagnostic>> {
        let callable = self.callable.encode()?;
        let entry = native_encoding::callable_entry::encode_private_function_entry(self.entry)
            .map_err(|error| {
                realization_error(
                    "callback entry ABI",
                    format!("native entry transport: {error}"),
                )
            })?;
        Ok(PrivateBody { callable, entry })
    }
}

impl EncodedPrivateBody {
    pub(super) fn layout(self) -> Result<LaidOutPrivateBody, Vec<Diagnostic>> {
        Ok(PrivateBody {
            callable: self.callable.layout()?,
            entry: placement::entry_layout::resolve_private_entry(self.entry)
                .map(relaxation::retain_fixed_entry)
                .map(function_layout::place_fixed_entry)
                .map_err(|error| realization_error("callback entry layout", error))?,
        })
    }
}

pub(super) struct PrivateBodyObject {
    pub object: PrivateFunctionObject,
    pub physical_original: native_artifact::PrivatePhysicalOriginal,
    pub stage_originals: Option<Box<published_object_verification::RetainedPhysicalInputs>>,
}

impl LaidOutPrivateBody {
    pub(super) fn construct_object(self) -> Result<PrivateBodyObject, Vec<Diagnostic>> {
        let emitted = self.callable.construct_object()?;
        let object = image_emission::bind_private_function_entry(
            object_file::PrivateEntryObject::from_placement(self.entry),
            Arc::new(emitted.projection.into_object()),
        )
        .map_err(|error| realization_error("callback entry ABI", error))?;
        Ok(PrivateBodyObject {
            object,
            physical_original: emitted.physical_original,
            stage_originals: emitted.stage_originals,
        })
    }
}

impl<'contract> PreparedCallbackBodies<'contract> {
    pub fn from_roster(
        roster: super::plan_callables::PlannedProgramBodies<'contract>,
    ) -> Result<(target_lowering::NativeBodyPlan, Self), Vec<Diagnostic>> {
        let (root, placement_count, planned) = roster.into_parts();
        let mut selected = vec![None; placement_count];
        let mut target_bodies = (0..placement_count).map(|_| None).collect::<Vec<_>>();
        let mut order = Vec::with_capacity(planned.len());
        for (contract, body) in planned {
            let index = contract.placement_index;
            selected[index] = Some(contract);
            target_bodies[index] = Some(body);
            order.push(index);
        }
        let mut installed_plans = PreparedEntryPlans::new(placement_count);
        let root_handlers = installed_plans.dependencies(&root, &selected)?;
        let mut edges = vec![Vec::new(); placement_count];
        for &index in &order {
            let body = target_bodies[index]
                .as_ref()
                .expect("closed roster contains this target");
            edges[index] = installed_plans.dependencies(&body.target, &selected)?;
        }
        let (before_root, after_root) =
            super::handler_order::handler_planning_order(&edges, &root_handlers, &order)
                .map_err(|reason| realization_error("installed handler stack", reason))?;
        let mut bodies = Self {
            selected,
            target_bodies,
            order,
            after_root,
            installed_plans,
            bodies: (0..placement_count).map(|_| None).collect(),
        };
        for index in before_root {
            bodies.plan(index)?;
        }
        Ok((root, bodies))
    }

    fn plan(&mut self, index: usize) -> Result<(), Vec<Diagnostic>> {
        let settlement = self.selected[index].ok_or_else(|| {
            realization_error(
                "callback body closure",
                "closed private entry contract is missing",
            )
        })?;
        let target = self.target_bodies[index].take().ok_or_else(|| {
            realization_error(
                "callback body closure",
                "retained private target is missing",
            )
        })?;
        self.bodies[index] = Some(plan_physical_callback(target, &settlement, self)?);
        Ok(())
    }
}

/// Measure only placements that a surviving guard actually references, after
/// physical planning has closed the callback roster.
pub(super) fn retained_handler_measurements(
    root: &super::physical_body::PreparedPhysicalBody,
    private: &[PlannedPrivateBody],
) -> Result<installation_binding::RetainedHandlerMeasurements, Vec<Diagnostic>> {
    use std::collections::{BTreeMap, BTreeSet};
    let demanded = std::iter::once(root)
        .chain(private.iter().map(|body| &body.callable.body))
        .flat_map(|body| body.stage.installations().installed_stack_requirements())
        .map(|row| (row.installation_machine, row.operation, row.placement_index))
        .collect::<BTreeSet<_>>();
    let positions = private
        .iter()
        .filter_map(|body| {
            body.identity
                .callback_thunk_placement_index()
                .map(|i| (i, body))
        })
        .collect::<BTreeMap<_, _>>();
    let mut measured = Vec::with_capacity(demanded.len());
    for (machine, operation, placement) in demanded {
        let body = positions.get(&placement).ok_or_else(|| {
            realization_error(
                "installed handler stack",
                "installed stack guard has no retained physical handler",
            )
        })?;
        measured.push(body.measured_handler(machine, operation)?.clone());
    }
    installation_binding::RetainedHandlerMeasurements::new(measured)
        .map_err(|reason| realization_error("installed handler stack", reason))
}

struct RetainedInstallation<'a> {
    scope: &'a AdmittedInstalledInterruptScope,
    entries: Vec<&'a AdmittedInstalledInterruptEntry>,
}

fn retained_installations(
    target: &target_lowering::NativeBodyPlan,
) -> Result<Vec<RetainedInstallation<'_>>, Vec<Diagnostic>> {
    let cohort = target.semantic_facts().admission.installed_interrupts();
    // The plan lists the installations that survived lowering, sorted and unique.
    let operations = &target.execution_plan().plan().installation_operations;
    let mut entries: Vec<_> = cohort.entries().iter().collect();
    entries.sort_unstable_by_key(|entry| {
        (
            (entry.machine, entry.operation),
            entry.placement.application_index,
        )
    });
    if entries.windows(2).any(|pair| {
        (
            (pair[0].machine, pair[0].operation),
            pair[0].placement.application_index,
        ) == (
            (pair[1].machine, pair[1].operation),
            pair[1].placement.application_index,
        )
    }) {
        return Err(realization_error(
            "installed handler stack",
            "installation application is duplicated",
        ));
    }
    let mut scopes = Vec::new();
    for scope in cohort.scopes().iter().filter(|scope| {
        operations
            .binary_search(&(scope.machine(), scope.operation()))
            .is_ok()
    }) {
        let first = entries.partition_point(|entry| {
            (entry.machine, entry.operation) < (scope.machine(), scope.operation())
        });
        let last = entries.partition_point(|entry| {
            (entry.machine, entry.operation) <= (scope.machine(), scope.operation())
        });
        scopes.push(RetainedInstallation {
            scope,
            entries: entries[first..last].to_vec(),
        });
    }
    Ok(scopes)
}

pub(crate) fn prepare_installed_callbacks(
    target: &target_lowering::NativeBodyPlan,
    bodies: &PreparedCallbackBodies<'_>,
    physical_entry: Option<installation_conditions::InstalledPhysicalStackRoot>,
) -> Result<Vec<installation_conditions::InstalledInterruptStackRequirement>, Vec<Diagnostic>> {
    let mut requirements = Vec::new();
    for RetainedInstallation { scope, entries } in retained_installations(target)? {
        let first_requirement = requirements.len();
        for &entry in &entries {
            let body = bodies
                .bodies
                .get(entry.placement_index)
                .and_then(Option::as_ref)
                .filter(|body| {
                    body.identity == entry.callback_function
                        && body.callable.body.stage.program().frame.instructions.entry
                            == entry.entry().machine()
                })
                .ok_or_else(|| {
                    realization_error(
                        "installed handler stack",
                        "arrival has no matching retained physical handler",
                    )
                })?;
            let measured = body.measured_handler(entry.machine, entry.operation)?;
            requirements.push(measured.requirement().clone());
        }
        installation_admission::arrival::compose(
            scope,
            &entries,
            &mut requirements[first_requirement..],
            physical_entry,
        )
        .map_err(|error| realization_error("installed arrival stack", error))?;
    }
    Ok(requirements)
}

impl PreparedCallbackBodies<'_> {
    pub fn finish_physical(mut self) -> Result<Vec<PlannedPrivateBody>, Vec<Diagnostic>> {
        for index in std::mem::take(&mut self.after_root) {
            self.plan(index)?;
        }
        self.order
            .into_iter()
            .map(|index| {
                self.bodies[index].take().ok_or_else(|| {
                    realization_error(
                        "callback body closure",
                        "reachable private body was not retained",
                    )
                })
            })
            .collect()
    }
}

fn plan_physical_callback(
    prepared: CallableTargetProduct,
    settlement: &PrivateEntryContract<'_>,
    prepared_objects: &PreparedCallbackBodies<'_>,
) -> Result<PlannedPrivateBody, Vec<Diagnostic>> {
    let installed_plan = prepared_objects.installed_plans.for_body(
        settlement,
        prepared
            .target
            .semantic_facts()
            .admission
            .installed_interrupts(),
    )?;
    let execution = installed_plan
        .as_ref()
        .map(|(_, execution)| execution.clone());
    let requirements = prepare_installed_callbacks(&prepared.target, prepared_objects, None)?;
    let callable =
        super::callable_objects::prepare_callable_physical(prepared, execution, requirements)?;
    let emitted = &callable
        .body
        .stage
        .program()
        .frame
        .instructions
        .contracts
        .function_abi(settlement.entry)
        .ok_or_else(|| realization_error("callback entry ABI", "callable body entry plan missing"))?
        .call_plan;
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
    let entry = entry_planning::callable_entry::plan_private_function_entry(
        settlement.callback_function,
        &callable.body.stage.program().frame,
        callable.body.stage.installations().for_entry_planning(),
        settlement.boundary_entry_plan,
        &inbound_ordinals,
        installed_plan.map(|(plan, _)| plan),
    )
    .map_err(|error| realization_error("callback entry ABI", error))?;
    Ok(PlannedPrivateBody {
        measurement: std::sync::OnceLock::new(),
        identity: settlement.callback_function,
        callable,
        entry,
    })
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

    fn dependencies(
        &mut self,
        body: &target_lowering::NativeBodyPlan,
        selected: &[Option<PrivateEntryContract<'_>>],
    ) -> Result<Vec<usize>, Vec<Diagnostic>> {
        let mut placements = Vec::new();
        for RetainedInstallation { scope, entries } in retained_installations(body)? {
            self.retain(scope, &entries)?;
            for entry in entries {
                let known = selected.get(entry.placement_index).and_then(Option::as_ref);
                if !known.is_some_and(|row| {
                    row.callback_function == entry.callback_function
                        && row.terminal_operation == entry.operation
                        && row.entry == entry.entry().machine()
                }) {
                    return Err(realization_error(
                        "installed handler stack",
                        "arrival has no matching retained body and entry plan",
                    ));
                }
                placements.push(entry.placement_index);
            }
        }
        Ok(placements)
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
            || timer.machine != scope.machine()
            || timer.operation != scope.operation()
            || timer.placement.application_index != scope.acknowledgement_application()
        {
            return Err(fail(
                "timer anchor differs from its admitted installation entry",
            ));
        }
        let scope = Arc::new(scope.clone());
        for entry in entries {
            if entry.machine != scope.machine()
                || entry.operation != scope.operation()
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
        settlement: &PrivateEntryContract<'_>,
        cohort: &AdmittedInstalledInterruptCohort,
    ) -> Result<
        Option<(
            InstalledInterruptEntryPlan,
            provider_admission::AdmittedInstalledEntryExecution,
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
                    && entry.entry().psi() == settlement.psi
                    && entry.entry().machine() == settlement.entry
                    && entry.operation == settlement.terminal_operation
                    && entry.callback_function == settlement.callback_function
                    && retained.scope.machine() == entry.machine
                    && retained.scope.operation() == entry.operation =>
            {
                let execution = provider_admission::AdmittedInstalledEntryExecution::for_entry(
                    Arc::clone(&retained.scope),
                    entry,
                    cohort,
                )
                .map_err(fail)?;
                Ok(Some((retained.plan, execution)))
            }
            _ => Err(fail(
                "private entry changed its original installation, artifact or callback association",
            )),
        }
    }
}

/*
We close portable Terminal task activation demands under one selected runtime
provider and target profile (`realize_task_activations`).

Psi has already established source contracts, suspension crossings, and call
identities. Here we resolve the exact `TaskRuntime` provider plan from
`SelectedProviderPlanFacts`, compute the target-specific storage geometry for
alternative task states and parked call arguments (`TaskStorage`), compose the
whole-call-graph WCSU stack demand, and validate the resulting
`TaskActivationPlanSet` without reading source symbols or source arenas.
*/

use crate::*;
use diagnostics::Diagnostic;
use effects::provider_plan::{ServiceMethod, ServiceSchema};
use semantic_vocabulary::Fnv1a;
use std::borrow::Cow;
use terminal_psi::*;

pub fn realize_task_activations(
    demands: &TerminalTaskActivationDemands,
    identities: &TerminalIdentityTable,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    profile: target::TargetProfile,
) -> Result<TaskActivationPlanSet, Vec<Diagnostic>> {
    if demands.demands.is_empty() {
        return Ok(TaskActivationPlanSet::default());
    }
    let storage = TaskStorage::new(demands, profile)?;
    let target = profile.native_target();
    let mut activations = Vec::new();
    let mut plan_identities = TerminalIdentityTableBuilder::default();
    for demand in &demands.demands {
        let selected_runtime =
            selected_task_runtime_provider(identities, selected_provider_plans, demand)?;
        let arguments = demand
            .signature_arguments
            .iter()
            .map(|value| {
                storage
                    .value(value.value_type)
                    .map(|shape| (value.normalized_identity, shape))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let argument_identity = signature_layout_identity(identities, &arguments)?;
        let argument_layout = TaskArgumentLayout::new(
            normalized_id(argument_identity, ValueLayoutId::from_normalized_identity)?,
            &arguments
                .iter()
                .map(|(_, shape)| (shape.0, shape.1))
                .collect::<Vec<_>>(),
        )
        .map_err(plan_diagnostic)?;
        let outcome = storage.value(demand.signature_outcome.value_type)?;
        let outcome_identity = signature_layout_identity(
            identities,
            &[(demand.signature_outcome.normalized_identity, outcome)],
        )?;
        let mut calling = Fnv1a::new();
        calling.bytes(&target.wire_identity_bytes());
        calling.u64(demand.entry_report_fingerprint);
        calling.u64(argument_identity);
        calling.u64(outcome_identity);
        let frames = demand
            .frames
            .iter()
            .map(|frame| storage.frame(frame, identities))
            .collect::<Result<Vec<_>, _>>()?;
        let stack_demand = compose_task_stack_demand(
            normalized_id(
                demand.root_frame,
                TaskStackFrameId::from_normalized_identity,
            )?,
            frames,
        )
        .map_err(plan_diagnostic)?;
        let mut representation = Fnv1a::new();
        representation.byte(0x53);
        representation.string_terminated("fixed-nonmoving-stack-v1");
        representation.bytes(&target.wire_identity_bytes());
        let projection = project_wcsu_stack_plan(
            &stack_demand,
            normalized_id(
                representation.finish_or(1),
                StackRepresentationId::from_normalized_identity,
            )?,
        );
        let crossings = demand
            .crossings
            .iter()
            .map(crossing)
            .collect::<Result<Vec<_>, _>>()?;
        let plan = validate_wcsu_activation_plan(
            ActivationPlanCandidate {
                machine_contract: normalized_id(
                    demand.contract_report_fingerprint,
                    MachineContractId::from_normalized_identity,
                )?,
                entry: normalized_id(
                    demand.entry_report_fingerprint,
                    MachineEntryId::from_normalized_identity,
                )?,
                argument_layout,
                terminal_outcome_layout: normalized_id(
                    outcome_identity,
                    ValueLayoutId::from_normalized_identity,
                )?,
                calling_plan: normalized_id(
                    calling.finish_or(1),
                    CallingPlanId::from_normalized_identity,
                )?,
                stack_plan: projection.stack_plan(),
                may_suspend: demand.may_suspend,
                may_block: demand.may_block,
                canonical_suspension_crossings: crossings,
                carry_obligations: ActivationCarryObligations {
                    preserve_cpu: demand.carry_obligations.preserve_cpu,
                    preserve_host_thread: demand.carry_obligations.preserve_host_thread,
                },
                cancellation_required: demand.cancellation_required,
            },
            projection,
        )
        .map_err(plan_diagnostic)?;
        activations.push(TaskActivationPlanFact {
            start_requirement: plan_identities.intern(identity_text(
                identities,
                demand.requirement_callable_identity,
            )?),
            target_machine: plan_identities
                .intern(identity_text(identities, demand.machine_callable_identity)?),
            target_entry: plan_identities.intern(identity_text(identities, demand.entry)?),
            specialization_report_fingerprint: demand.specialization_report_fingerprint,
            specialization_commitment: TaskSpecializationCommitment::from_digest(
                demand.specialization_commitment,
            ),
            operation: match demand.operation {
                TerminalTaskStartOperation::Start => TaskStartOperation::Start,
                TerminalTaskStartOperation::TryStart => TaskStartOperation::TryStart,
            },
            selected_runtime,
            plan,
        });
    }
    Ok(TaskActivationPlanSet {
        identities: plan_identities.build(),
        activations,
    })
}

fn selected_task_runtime_provider(
    identities: &TerminalIdentityTable,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    demand: &TerminalTaskActivationDemand,
) -> Result<SelectedTaskRuntimeProviderFact, Vec<Diagnostic>> {
    let requirement_owner_package = demand.requirement_owner_package;
    let requirement_owner_path = identity_text(identities, demand.requirement_owner_path)?;
    let authored_requirement_identity =
        identity_text(identities, demand.authored_requirement_identity)?;
    let binds_authored_requirement = |schema: &ServiceSchema, method: &ServiceMethod| {
        method.requirement_owner == requirement_owner_path
            && method.requirement_owner_package_identity == requirement_owner_package
            && schema.identity_text(method.requirement_identity) == authored_requirement_identity
    };
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| {
            demand.requirement_in_scope
                && plan
                    .schema
                    .identity_text(plan.schema.declaration.trait_name)
                    == requirement_owner_path
                && plan.schema.declaration.trait_package_identity == requirement_owner_package
                && plan
                    .schema
                    .declaration
                    .methods
                    .iter()
                    .any(|method| binds_authored_requirement(&plan.schema, method))
        })
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(match matches.len() {
            0 => "TaskRuntime boundary slot has no retained selected provider plan".to_owned(),
            count => format!(
                "TaskRuntime boundary slot matches {count} retained selected provider plans"
            ),
        })]);
    };
    let requirement_name = match demand.operation {
        TerminalTaskStartOperation::Start => "start",
        TerminalTaskStartOperation::TryStart => "try_start",
    };
    let requirements = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| binds_authored_requirement(&plan.schema, method))
        .collect::<Vec<_>>();
    let [method] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(match requirements.len() {
            0 => format!(
                "selected TaskRuntime provider plan `{}` has no `{requirement_name}` requirement",
                plan.name
            ),
            count => format!(
                "selected TaskRuntime provider plan `{}` has {count} `{requirement_name}` requirements; operation binding must be exact",
                plan.name
            ),
        })]);
    };
    let covering_rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method, &plan.identities))
        .count();
    if covering_rows != 1 {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` {covering_rows} times; exactly one operation realization is required",
            plan.name
        ))]);
    }
    let runtime = TaskRuntimeId::from_normalized_identity(plan.report_fingerprint())
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    Ok(SelectedTaskRuntimeProviderFact {
        runtime,
        provider_plan_name: plan.name.clone(),
    })
}

struct TaskStorage {
    shapes: semantic_vocabulary::IdTable<
        semantic_vocabulary::StructuralTypeId,
        Option<calling_conventions::ValueShape>,
    >,
    pointer: (u64, u64),
}

impl TaskStorage {
    fn new(
        demands: &TerminalTaskActivationDemands,
        profile: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut declarations = Cow::Borrowed(demands.structural_types.as_slice());
        if !demands.target_layout_applications.is_empty() {
            for application in &demands.target_layout_applications {
                let rows = application
                    .realizations
                    .iter()
                    .filter(|row| row.target == profile.target_name())
                    .collect::<Vec<_>>();
                let [row] = rows.as_slice() else {
                    return Err(vec![Diagnostic::error(
                        "task layout requires exactly one selected target realization",
                    )]);
                };
                let declaration = declarations
                    .to_mut()
                    .iter_mut()
                    .find(|declaration| declaration.id == application.record)
                    .ok_or_else(|| vec![Diagnostic::error("task target layout has no record")])?;
                let StructuralTypeShape::Record { storage, .. } = &mut declaration.shape else {
                    return Err(vec![Diagnostic::error(
                        "task target layout does not select a record",
                    )]);
                };
                if *storage != StructuralRecordStorage::TargetLayout(application.id) {
                    return Err(vec![Diagnostic::error(
                        "task layout application does not match its record",
                    )]);
                }
                *storage = StructuralRecordStorage::Authored(row.storage);
            }
        }
        let layouts =
            calling_conventions::terminal_layout::StructuralLayouts::new(declarations.iter())
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "task structural types are not one dense identity run: {error:?}"
                    ))]
                })?;
        for demand in &demands.demands {
            let signature = demand
                .signature_arguments
                .iter()
                .chain([&demand.signature_outcome])
                .map(|value| value.value_type);
            let frames = demand
                .frames
                .iter()
                .flat_map(|frame| &frame.states)
                .flat_map(|state| {
                    state
                        .resident
                        .iter()
                        .chain(state.crossing_arguments.iter().flatten())
                })
                .map(|value| value.value_type);
            for value in signature.chain(frames) {
                if let TerminalTaskValueType::Structural(identity) = value {
                    layouts.shape(identity).map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "task storage geometry: {error:?}"
                        ))]
                    })?;
                }
            }
        }
        let target = profile.native_target();
        Ok(Self {
            shapes: layouts.map(|identity, _| layouts.shape(identity).ok()),
            pointer: (target.pointer_size as u64, target.pointer_alignment as u64),
        })
    }

    fn value(&self, value: TerminalTaskValueType) -> Result<(u64, u64), Vec<Diagnostic>> {
        let shape = match value {
            TerminalTaskValueType::Unit => return Ok((0, 1)),
            TerminalTaskValueType::Scalar(scalar) => {
                calling_conventions::terminal_layout::scalar_shape(scalar)
            }
            TerminalTaskValueType::Structural(identity) => self
                .shapes
                .get(identity)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "task value is absent from its realized storage catalog",
                    )]
                })?,
        };
        Ok((u64::from(shape.byte_size), u64::from(shape.alignment)))
    }

    fn frame(
        &self,
        frame: &TerminalTaskFrame,
        identities: &TerminalIdentityTable,
    ) -> Result<ValidatedTaskStackFrameSummary, Vec<Diagnostic>> {
        let identity = normalized_id(frame.identity, TaskStackFrameId::from_normalized_identity)?;
        let mut local_bytes = align(self.pointer.0, self.pointer.1)?;
        let mut local_alignment = self.pointer.1;
        for state in &frame.states {
            let mut size = align(self.pointer.0, self.pointer.1)?;
            let mut alignment = self.pointer.1;
            for value in &state.resident {
                append(&mut size, &mut alignment, self.value(value.value_type)?)?;
            }
            let mut state_peak = align(size, alignment)?;
            let mut state_alignment = alignment;
            for arguments in &state.crossing_arguments {
                let mut crossing_size = size;
                let mut crossing_alignment = alignment;
                for argument in arguments {
                    append(
                        &mut crossing_size,
                        &mut crossing_alignment,
                        self.value(argument.value_type)?,
                    )?;
                }
                state_peak = state_peak.max(align(crossing_size, crossing_alignment)?);
                state_alignment = state_alignment.max(crossing_alignment);
            }
            local_bytes = local_bytes.max(state_peak);
            local_alignment = local_alignment.max(state_alignment);
        }
        let calls = frame
            .calls
            .iter()
            .map(|call| {
                normalized_id(call.callee, TaskStackFrameId::from_normalized_identity)
                    .map(|callee| StackCallContribution::Checked { callee })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unresolved_calls = frame
            .unresolved_calls
            .iter()
            .map(|site| {
                Ok(UnresolvedCallSite {
                    frame: identity,
                    state: identity_text(identities, site.state)?.to_owned(),
                    statement_index: site.statement_index as usize,
                    call_ordinal: site.call_ordinal as usize,
                    kind: match site.kind {
                        TerminalTaskUnresolvedCallKind::UnresolvedTarget => {
                            UnresolvedCallKind::UnresolvedTarget
                        }
                        TerminalTaskUnresolvedCallKind::NonCheckedSupply => {
                            UnresolvedCallKind::NonCheckedSupply
                        }
                    },
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let validation = task_stack_frame_validation_identity(
            identity,
            local_bytes,
            local_alignment,
            &calls,
            &unresolved_calls,
        );
        validate_task_stack_frame_summary(TaskStackFrameSummary {
            frame: identity,
            local_bytes,
            alignment: local_alignment,
            validation,
            calls,
            unresolved_calls,
        })
        .map_err(plan_diagnostic)
    }
}

fn append(size: &mut u64, alignment: &mut u64, value: (u64, u64)) -> Result<(), Vec<Diagnostic>> {
    *size = align(*size, value.1)?
        .checked_add(value.0)
        .ok_or_else(|| vec![Diagnostic::error("task WCSU frame layout size overflow")])?;
    *alignment = (*alignment).max(value.1);
    Ok(())
}

fn align(size: u64, alignment: u64) -> Result<u64, Vec<Diagnostic>> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(vec![Diagnostic::error(
            "task WCSU frame has invalid alignment",
        )]);
    }
    size.checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task WCSU frame layout alignment overflow",
            )]
        })
}

fn crossing(
    crossing: &TerminalTaskSuspensionCrossing,
) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
    let live_carry = crossing
        .live_carry
        .iter()
        .map(|live| {
            Ok(LiveCarryDemand {
                place: normalized_id(live.place, LiveCarryPlaceId::from_normalized_identity)?,
                ty: normalized_id(live.value_type, LiveCarryTypeId::from_normalized_identity)?,
                storage: match live.storage {
                    TerminalTaskCarryStorage::Persistent => LiveCarryStorage::Persistent,
                    TerminalTaskCarryStorage::Parameter => LiveCarryStorage::Parameter,
                    TerminalTaskCarryStorage::Local => LiveCarryStorage::Local,
                    TerminalTaskCarryStorage::CallArgument => LiveCarryStorage::CallArgument,
                },
                claims: live
                    .claims
                    .iter()
                    .map(|claim| {
                        ClaimId::new(*claim).ok_or_else(|| {
                            vec![Diagnostic::error("task carry claim has zero identity")]
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                effective: live.effective,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(CanonicalSuspensionCrossing {
        identity: SuspensionCrossingId::new(crossing.identity)
            .ok_or_else(|| vec![Diagnostic::error("task crossing has zero identity")])?,
        suspension_allowed: crossing.suspension_allowed,
        preserve_cpu: crossing.preserve_cpu,
        preserve_host_thread: crossing.preserve_host_thread,
        live_carry,
    })
}

fn signature_layout_identity(
    identities: &TerminalIdentityTable,
    values: &[(TerminalIdentityIndex, (u64, u64))],
) -> Result<u64, Vec<Diagnostic>> {
    let mut hash = Fnv1a::new();
    hash.byte(0x51);
    for (identity, (size, alignment)) in values {
        hash.string_terminated(identity_text(identities, *identity)?);
        hash.u64(*size);
        hash.u64(*alignment);
        hash.byte(0xff);
    }
    Ok(hash.finish_or(1))
}

fn identity_text(
    identities: &TerminalIdentityTable,
    identity: TerminalIdentityIndex,
) -> Result<&str, Vec<Diagnostic>> {
    identities
        .text(identity)
        .ok_or_else(|| vec![Diagnostic::error("task demand has an unresolved identity")])
}

fn normalized_id<T>(
    identity: u64,
    constructor: impl FnOnce(u64) -> Result<T, TaskPlanDiagnostic>,
) -> Result<T, Vec<Diagnostic>> {
    constructor(identity).map_err(plan_diagnostic)
}

fn plan_diagnostic(error: TaskPlanDiagnostic) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
}

//! Rejoin checked rebound, stored and forwarded dynamic-call plans to their
//! exact Terminal descriptor-table occurrences.
//!
//! The local descriptor-table materializations are the rebound and stored
//! families. One checked rebound plan emits one
//! `CallDynamicScalar`/`CallDynamicUnit` operation, one indirect-dispatch
//! row, one rebound descriptor, and two conformance selections naming closed
//! applications. One checked stored scalar plan emits one `CallDynamicScalar`
//! operation, one stored-dispatch row, one stored descriptor established by
//! an exact `StoreDynamicDescriptor` operation, and one conformance
//! selection the reload consumes; a stored Unit call is refused at lowering,
//! so the stored family is scalar-only.
//!
//! The register-indirect family is the forwarded chain. One checked
//! forwarded plan emits the root caller's ordinary call into the first
//! generated helper (one per branch for a checked two-predecessor join), one
//! ordinary forwarding call per retained transfer inside each generated
//! helper, and the final helper's `CallDynamicParameterScalar`/
//! `CallDynamicParameterUnit` dispatch through the chain's existential
//! descriptor parameter. The catalog shares one parameter interface across
//! every helper and one argument row per hop; the occurrence rejoins only
//! the final dispatch, with every hop's operation, argument and transfer
//! custody checked on the way. A direct plan emits direct-dispatch rows and
//! never enters this roster.

use super::{CheckedDynamicCallLane, CheckedDynamicCallOccurrence, unsupported};
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedDynamicBinding, CheckedDynamicDescriptorTransferPlan,
    CheckedDynamicDescriptorTransferSource, CheckedDynamicDispatchPlan,
    CheckedDynamicJoinBranchPlan, CheckedDynamicScalarCallOrigin, CheckedDynamicScalarCallPlan,
    CheckedDynamicSelectionPlan, CheckedDynamicStoredDescriptorPlan, CheckedDynamicUnitCallOrigin,
    CheckedDynamicUnitCallPlan, CheckedStructuralAccess, CheckedTrees, CheckedUnitCallCoordinate,
    CheckedUnitStructuralPathSegment, DynamicConformanceRowFact,
};
use lowered_psi::{LoweredPsi, LoweredSourceCallOccurrence};
use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
};
use std::collections::BTreeSet;
use symbols::SymbolHandle;
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceCallableResult, ClosedConformanceRow, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument, StructuralPathSegment,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorSource,
    TerminalDynamicRequirement, TerminalIndirectDynamicDispatch, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalStoredDynamicDispatch,
};

/// Borrowed view over the dispatch coordinates shared by the scalar and Unit
/// plan types.
struct DispatchView<'a> {
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    receiver_binding: SymbolHandle,
    requirement: SymbolHandle,
    requirement_identity: &'a str,
    declaring_trait: SymbolHandle,
    realization_state: SymbolHandle,
    target_trait: SymbolHandle,
    selected_conformance: SymbolHandle,
    family_tuple: &'a [String],
    selection: &'a checked_trees::DynamicConformanceBindingFact,
    source_path: &'a [CheckedUnitStructuralPathSegment],
    source_type_identity: &'a str,
    source_parameter_position: u32,
    source_access: CheckedStructuralAccess,
}

/// Both call plans name their shared view identically, so each lane borrows
/// it here rather than repeating every field at every call site.
macro_rules! dispatch_view {
    ($plan:expr) => {
        DispatchView {
            caller_machine: $plan.caller_machine,
            caller_state: $plan.caller_state,
            coordinate: $plan.coordinate,
            receiver_binding: $plan.receiver_binding,
            requirement: $plan.requirement,
            requirement_identity: &$plan.requirement_identity,
            declaring_trait: $plan.declaring_trait,
            realization_state: $plan.realization_state,
            target_trait: $plan.target_trait,
            selected_conformance: $plan.selected_conformance,
            family_tuple: &$plan.family_tuple,
            selection: &$plan.selection,
            source_path: &$plan.source_path,
            source_type_identity: &$plan.source_type_identity,
            source_parameter_position: $plan.source_parameter_position,
            source_access: $plan.source_access,
        }
    };
}

/// Which descriptor-table species a replayed dispatch belongs to. Error
/// messages keep naming the checked family a drifted row belongs to, so all
/// families share one join without conflating their custody.
#[derive(Clone, Copy)]
enum DispatchFamily {
    Rebound,
    Stored,
    Forwarded,
}

impl DispatchFamily {
    const fn of(lane: CheckedDynamicCallLane) -> Self {
        match lane {
            CheckedDynamicCallLane::ReboundScalar | CheckedDynamicCallLane::ReboundUnit => {
                Self::Rebound
            }
            CheckedDynamicCallLane::StoredScalar => Self::Stored,
            CheckedDynamicCallLane::ForwardedScalar | CheckedDynamicCallLane::ForwardedUnit => {
                Self::Forwarded
            }
        }
    }

    const fn message(
        self,
        rebound: &'static str,
        stored: &'static str,
        forwarded: &'static str,
    ) -> &'static str {
        match self {
            Self::Rebound => rebound,
            Self::Stored => stored,
            Self::Forwarded => forwarded,
        }
    }
}

/// The dispatch-row coordinates the indirect (rebound) and stored catalog
/// species carry identically: the descriptor ordinal the operation consumes
/// and the exact selected-row identities.
#[derive(Clone, Copy)]
struct DispatchRow<'a> {
    descriptor_ordinal: u32,
    declaring_trait_identity: &'a str,
    public_requirement_identity: &'a str,
    family_tuple: &'a [String],
    requirement_identity: &'a str,
    realization_identity: &'a str,
    realization_callable_identity: &'a str,
    realization: MachineId,
}

impl<'a> From<&'a TerminalIndirectDynamicDispatch> for DispatchRow<'a> {
    fn from(dispatch: &'a TerminalIndirectDynamicDispatch) -> Self {
        Self {
            descriptor_ordinal: dispatch.descriptor_ordinal,
            declaring_trait_identity: &dispatch.declaring_trait_identity,
            public_requirement_identity: &dispatch.public_requirement_identity,
            family_tuple: &dispatch.family_tuple,
            requirement_identity: &dispatch.requirement_identity,
            realization_identity: &dispatch.realization_identity,
            realization_callable_identity: &dispatch.realization_callable_identity,
            realization: dispatch.realization,
        }
    }
}

impl<'a> From<&'a TerminalStoredDynamicDispatch> for DispatchRow<'a> {
    fn from(dispatch: &'a TerminalStoredDynamicDispatch) -> Self {
        Self {
            descriptor_ordinal: dispatch.descriptor_ordinal,
            declaring_trait_identity: &dispatch.declaring_trait_identity,
            public_requirement_identity: &dispatch.public_requirement_identity,
            family_tuple: &dispatch.family_tuple,
            requirement_identity: &dispatch.requirement_identity,
            realization_identity: &dispatch.realization_identity,
            realization_callable_identity: &dispatch.realization_callable_identity,
            realization: dispatch.realization,
        }
    }
}

pub(super) fn replay(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
) -> Result<Vec<CheckedDynamicCallOccurrence>, &'static str> {
    let plans = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let module = &lowered.semantic_module;
    let source_calls = &lowered.source_call_occurrences;
    let mut occurrences = Vec::new();
    let mut claimed_parameters = BTreeSet::new();
    let mut claimed_arguments = BTreeSet::new();
    for (plan_index, plan) in plans.calls.iter().enumerate() {
        let occurrence = match plan {
            CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Rebound {
                initial,
                latest,
            }) => match latest.origin {
                CheckedDynamicScalarCallOrigin::Local => replay_rebound(
                    checked,
                    module,
                    source_calls,
                    dispatch_view!(latest),
                    initial,
                    plan_index,
                    CheckedDynamicCallLane::ReboundScalar,
                )?,
                CheckedDynamicScalarCallOrigin::Forwarded {
                    machine,
                    state,
                    coordinate,
                    parameter,
                } => replay_forwarded(
                    checked,
                    module,
                    source_calls,
                    plans,
                    &[ForwardedRoot {
                        view: dispatch_view!(latest),
                        initial: Some(initial),
                    }],
                    ForwardedOrigin {
                        machine,
                        state,
                        coordinate,
                        parameter,
                    },
                    &latest.forwarding_transfers,
                    &helper_sources(&latest.forwarding_helpers),
                    Some(latest.result.primitive_type),
                    plan_index,
                    CheckedDynamicCallLane::ForwardedScalar,
                    &mut claimed_parameters,
                    &mut claimed_arguments,
                )?,
            },
            CheckedDynamicDispatchPlan::Unit(CheckedDynamicBinding::Rebound {
                initial,
                latest,
            }) => match latest.origin {
                CheckedDynamicUnitCallOrigin::Local => replay_rebound(
                    checked,
                    module,
                    source_calls,
                    dispatch_view!(latest),
                    initial,
                    plan_index,
                    CheckedDynamicCallLane::ReboundUnit,
                )?,
                CheckedDynamicUnitCallOrigin::Forwarded {
                    machine,
                    state,
                    coordinate,
                    parameter,
                } => replay_forwarded(
                    checked,
                    module,
                    source_calls,
                    plans,
                    &[ForwardedRoot {
                        view: dispatch_view!(latest),
                        initial: Some(initial),
                    }],
                    ForwardedOrigin {
                        machine,
                        state,
                        coordinate,
                        parameter,
                    },
                    &latest.forwarding_transfers,
                    &helper_sources(&latest.forwarding_helpers),
                    None,
                    plan_index,
                    CheckedDynamicCallLane::ForwardedUnit,
                    &mut claimed_parameters,
                    &mut claimed_arguments,
                )?,
            },
            CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Stored {
                descriptor,
                call,
            }) => {
                if !matches!(call.origin, CheckedDynamicScalarCallOrigin::Local) {
                    continue;
                }
                replay_stored(
                    checked,
                    module,
                    source_calls,
                    dispatch_view!(call),
                    descriptor,
                    plan_index,
                )?
            }
            CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Direct(call)) => {
                let CheckedDynamicScalarCallOrigin::Forwarded {
                    machine,
                    state,
                    coordinate,
                    parameter,
                } = call.origin
                else {
                    continue;
                };
                replay_forwarded(
                    checked,
                    module,
                    source_calls,
                    plans,
                    &[ForwardedRoot {
                        view: dispatch_view!(call),
                        initial: None,
                    }],
                    ForwardedOrigin {
                        machine,
                        state,
                        coordinate,
                        parameter,
                    },
                    &call.forwarding_transfers,
                    &helper_sources(&call.forwarding_helpers),
                    Some(call.result.primitive_type),
                    plan_index,
                    CheckedDynamicCallLane::ForwardedScalar,
                    &mut claimed_parameters,
                    &mut claimed_arguments,
                )?
            }
            CheckedDynamicDispatchPlan::Unit(CheckedDynamicBinding::Direct(call)) => {
                let CheckedDynamicUnitCallOrigin::Forwarded {
                    machine,
                    state,
                    coordinate,
                    parameter,
                } = call.origin
                else {
                    continue;
                };
                replay_forwarded(
                    checked,
                    module,
                    source_calls,
                    plans,
                    &[ForwardedRoot {
                        view: dispatch_view!(call),
                        initial: None,
                    }],
                    ForwardedOrigin {
                        machine,
                        state,
                        coordinate,
                        parameter,
                    },
                    &call.forwarding_transfers,
                    &helper_sources(&call.forwarding_helpers),
                    None,
                    plan_index,
                    CheckedDynamicCallLane::ForwardedUnit,
                    &mut claimed_parameters,
                    &mut claimed_arguments,
                )?
            }
            CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Joined {
                when_true,
                when_false,
                ..
            }) => {
                let Some(origin) = joined_scalar_origin(when_true, when_false)? else {
                    continue;
                };
                if when_true.call.forwarding_transfers != when_false.call.forwarding_transfers
                    || when_true.call.forwarding_helpers != when_false.call.forwarding_helpers
                    || when_true.call.result.primitive_type != when_false.call.result.primitive_type
                {
                    return unsupported(
                        "joined dynamic branches drifted on their shared forwarded chain",
                    );
                }
                replay_forwarded(
                    checked,
                    module,
                    source_calls,
                    plans,
                    &[
                        ForwardedRoot {
                            view: dispatch_view!((&when_true.call)),
                            initial: None,
                        },
                        ForwardedRoot {
                            view: dispatch_view!((&when_false.call)),
                            initial: None,
                        },
                    ],
                    origin,
                    &when_true.call.forwarding_transfers,
                    &helper_sources(&when_true.call.forwarding_helpers),
                    Some(when_true.call.result.primitive_type),
                    plan_index,
                    CheckedDynamicCallLane::ForwardedScalar,
                    &mut claimed_parameters,
                    &mut claimed_arguments,
                )?
            }
            CheckedDynamicDispatchPlan::Unit(CheckedDynamicBinding::Joined {
                when_true,
                when_false,
                ..
            }) => {
                let Some(origin) = joined_unit_origin(when_true, when_false)? else {
                    continue;
                };
                if when_true.call.forwarding_transfers != when_false.call.forwarding_transfers
                    || when_true.call.forwarding_helpers != when_false.call.forwarding_helpers
                {
                    return unsupported(
                        "joined dynamic branches drifted on their shared forwarded chain",
                    );
                }
                replay_forwarded(
                    checked,
                    module,
                    source_calls,
                    plans,
                    &[
                        ForwardedRoot {
                            view: dispatch_view!((&when_true.call)),
                            initial: None,
                        },
                        ForwardedRoot {
                            view: dispatch_view!((&when_false.call)),
                            initial: None,
                        },
                    ],
                    origin,
                    &when_true.call.forwarding_transfers,
                    &helper_sources(&when_true.call.forwarding_helpers),
                    None,
                    plan_index,
                    CheckedDynamicCallLane::ForwardedUnit,
                    &mut claimed_parameters,
                    &mut claimed_arguments,
                )?
            }
            // A stored Unit call is refused at lowering, and a local direct
            // plan emits direct-dispatch rows: neither enters this roster.
            _ => continue,
        };
        if let Some(occurrence) = occurrence {
            occurrences.push(occurrence);
        }
    }
    occurrences.sort_by_key(|occurrence| {
        (
            occurrence.terminal_machine().get(),
            occurrence.terminal_operation().get(),
        )
    });
    let mut joined = BTreeSet::new();
    for occurrence in &occurrences {
        if !joined.insert((
            occurrence.terminal_machine(),
            occurrence.terminal_operation(),
        )) {
            return unsupported(
                "checked dynamic calls do not map one-to-one onto Terminal operations",
            );
        }
    }
    // The admitted lanes are the only producers of descriptor-table
    // dispatches: any row left unrejoined means a source occurrence was
    // skipped or went stale, so the roster would be incomplete rather than
    // merely smaller.
    if module
        .dynamic_dispatch
        .indirect_dispatches
        .iter()
        .any(|dispatch| !joined.contains(&(dispatch.owner, dispatch.operation)))
    {
        return unsupported("Terminal indirect dispatch does not rejoin one checked rebound call");
    }
    if module
        .dynamic_dispatch
        .stored_dispatches
        .iter()
        .any(|dispatch| !joined.contains(&(dispatch.owner, dispatch.operation)))
    {
        return unsupported("Terminal stored dispatch does not rejoin one checked stored call");
    }
    if module
        .dynamic_dispatch
        .parameter_dispatches
        .iter()
        .any(|dispatch| !joined.contains(&(dispatch.owner, dispatch.operation)))
    {
        return unsupported(
            "Terminal parameter dispatch does not rejoin one checked forwarded call",
        );
    }
    // Every descriptor parameter and argument row in the catalog exists only
    // to carry one forwarded chain: an unrejoined row means an emitted hop
    // was skipped or went stale, not that the roster is merely smaller.
    if module
        .dynamic_dispatch
        .parameters
        .iter()
        .any(|parameter| !claimed_parameters.contains(&(parameter.owner, parameter.ordinal)))
    {
        return unsupported(
            "Terminal dynamic parameter does not rejoin one checked forwarded call",
        );
    }
    if module
        .dynamic_dispatch
        .arguments
        .iter()
        .any(|argument| !claimed_arguments.contains(&(argument.owner, argument.operation)))
    {
        return unsupported("Terminal dynamic argument does not rejoin one checked forwarded call");
    }
    // Every emitted dynamic dispatch operation is one admitted lane's
    // occurrence: a dynamic call operation whose row was lost leaves the
    // emitted dispatch itself unrejoined.
    if module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .map(move |operation| (machine.id, operation))
        })
        .any(|(owner, operation)| {
            matches!(
                operation.kind,
                OperationKind::CallDynamicScalar { .. }
                    | OperationKind::CallDynamicUnit { .. }
                    | OperationKind::CallDynamicParameterScalar { .. }
                    | OperationKind::CallDynamicParameterUnit { .. }
            ) && !joined.contains(&(owner, operation.id))
        })
    {
        return unsupported(
            "Terminal dynamic call operation does not rejoin one checked dynamic call",
        );
    }
    // Every rebound descriptor is consumed by exactly one row: an indirect
    // dispatch for the rebound lanes or a forwarded descriptor argument for
    // the forwarded lanes. An unconsumed row is dead establishment custody,
    // and one descriptor backing two consumers would share the per-call
    // custody checked plans keep distinct.
    for descriptor in &module.dynamic_dispatch.rebound_descriptors {
        let consumers = module
            .dynamic_dispatch
            .indirect_dispatches
            .iter()
            .filter(|dispatch| {
                dispatch.owner == descriptor.owner
                    && dispatch.descriptor_ordinal == descriptor.ordinal
            })
            .count()
            + module
                .dynamic_dispatch
                .arguments
                .iter()
                .filter(|argument| {
                    argument.owner == descriptor.owner
                        && argument.source
                            == (TerminalDynamicDescriptorSource::ReboundDescriptor {
                                ordinal: descriptor.ordinal,
                            })
                })
                .count();
        if consumers != 1 {
            return unsupported(
                "Terminal rebound descriptor does not rejoin exactly one checked dynamic call",
            );
        }
    }
    // Every stored descriptor is consumed by exactly one joined dispatch: an
    // unconsumed row is dead establishment custody, and one descriptor
    // backing two dispatches would share the per-call reload checked plans
    // keep distinct.
    for descriptor in &module.dynamic_dispatch.stored_descriptors {
        let consumers = module
            .dynamic_dispatch
            .stored_dispatches
            .iter()
            .filter(|dispatch| {
                dispatch.owner == descriptor.owner
                    && dispatch.descriptor_ordinal == descriptor.ordinal
            })
            .count();
        if consumers != 1 {
            return unsupported(
                "Terminal stored descriptor does not rejoin exactly one checked stored call",
            );
        }
    }
    Ok(occurrences)
}

/// The exact `CallDynamic*` operation one checked local dispatch produced:
/// its source call occurrence names it, its owning machine contains it and
/// its result shape matches the lane. `None` when the plan's machine stayed
/// outside the selected composition.
fn join_call_operation<'a>(
    module: &'a TerminalModule,
    source_calls: &[LoweredSourceCallOccurrence],
    latest: &DispatchView<'_>,
    lane: CheckedDynamicCallLane,
) -> Result<Option<(&'a TerminalMachine, OperationId, u32)>, &'static str> {
    let family = DispatchFamily::of(lane);
    let statement_index = usize::try_from(latest.coordinate.statement_index).map_err(|_| {
        family.message(
            "rebound dynamic call statement coordinate exceeds usize",
            "stored dynamic call statement coordinate exceeds usize",
            "forwarded dynamic call statement coordinate exceeds usize",
        )
    })?;
    let call_ordinal = usize::try_from(latest.coordinate.call_ordinal).map_err(|_| {
        family.message(
            "rebound dynamic call ordinal exceeds usize",
            "stored dynamic call ordinal exceeds usize",
            "forwarded dynamic call ordinal exceeds usize",
        )
    })?;
    let matching = source_calls
        .iter()
        .filter(|occurrence| {
            occurrence.source_state == latest.caller_state
                && occurrence.statement_index == statement_index
                && occurrence.call_ordinal == call_ordinal
                && occurrence.source_target == latest.requirement
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        // The checked roster spans every dispatch-eligible machine; a plan
        // outside the selected composition legitimately has no occurrence.
        return Ok(None);
    }
    let [occurrence] = matching.as_slice() else {
        return unsupported(family.message(
            "rebound dynamic call maps to duplicate Terminal occurrences",
            "stored dynamic call maps to duplicate Terminal occurrences",
            "forwarded dynamic call maps to duplicate Terminal occurrences",
        ));
    };
    let terminal_operation = occurrence.terminal_operation;
    let owners = module
        .machines
        .iter()
        .filter_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == terminal_operation)
                .map(|operation| (machine, operation))
        })
        .collect::<Vec<_>>();
    let [(owner, operation)] = owners.as_slice() else {
        return unsupported(family.message(
            "rebound dynamic call does not name one exact Terminal operation",
            "stored dynamic call does not name one exact Terminal operation",
            "forwarded dynamic call does not name one exact Terminal operation",
        ));
    };
    let descriptor_ordinal = match (&operation.kind, lane) {
        (
            OperationKind::CallDynamicScalar {
                descriptor_ordinal, ..
            },
            CheckedDynamicCallLane::ReboundScalar | CheckedDynamicCallLane::StoredScalar,
        )
        | (
            OperationKind::CallDynamicUnit {
                descriptor_ordinal, ..
            },
            CheckedDynamicCallLane::ReboundUnit,
        ) => *descriptor_ordinal,
        _ => {
            return unsupported(family.message(
                "rebound dynamic call does not name one exact Terminal operation",
                "stored dynamic call does not name one exact Terminal operation",
                "forwarded dynamic call does not name one exact Terminal operation",
            ));
        }
    };
    Ok(Some((owner, terminal_operation, descriptor_ordinal)))
}

fn replay_rebound(
    checked: &CheckedTrees,
    module: &TerminalModule,
    source_calls: &[LoweredSourceCallOccurrence],
    latest: DispatchView<'_>,
    initial: &CheckedDynamicSelectionPlan,
    plan_index: usize,
    lane: CheckedDynamicCallLane,
) -> Result<Option<CheckedDynamicCallOccurrence>, &'static str> {
    let Some((owner, terminal_operation, descriptor_ordinal)) =
        join_call_operation(module, source_calls, &latest, lane)?
    else {
        return Ok(None);
    };
    let dispatches = module
        .dynamic_dispatch
        .indirect_dispatches
        .iter()
        .filter(|dispatch| dispatch.owner == owner.id && dispatch.operation == terminal_operation)
        .collect::<Vec<_>>();
    let [dispatch] = dispatches.as_slice() else {
        return unsupported(
            "rebound dynamic call does not rejoin one exact Terminal indirect dispatch",
        );
    };
    let dispatch_row = DispatchRow::from(*dispatch);
    verify_dispatch_row(
        checked,
        module,
        DispatchFamily::Rebound,
        &latest,
        descriptor_ordinal,
        &dispatch_row,
    )?;
    // The latest selection supplies the dispatch's table identity; its
    // application must carry the checked conformance's exact row roster and
    // the callable the dispatch names.
    let application = verify_rebound_descriptor(
        checked,
        module,
        owner,
        dispatch.descriptor_ordinal,
        &latest,
        initial,
        DispatchFamily::Rebound,
    )?;
    verify_selected_row(
        checked,
        DispatchFamily::Rebound,
        &latest,
        application,
        &dispatch_row,
    )?;
    Ok(Some(CheckedDynamicCallOccurrence::new(
        lane,
        plan_index,
        owner.id,
        terminal_operation,
        dispatch.descriptor_ordinal,
    )))
}

/// The rebound descriptor an indirect dispatch or a forwarded descriptor
/// argument consumes: two distinct selections, the latest naming the checked
/// selection's exact application and the initial either sharing that
/// application or rejoining its own changed one. Returns the latest
/// selection's application, the table identity the call dispatches through.
fn verify_rebound_descriptor<'a>(
    checked: &CheckedTrees,
    module: &'a TerminalModule,
    owner: &TerminalMachine,
    descriptor_ordinal: u32,
    latest: &DispatchView<'_>,
    initial: &CheckedDynamicSelectionPlan,
    family: DispatchFamily,
) -> Result<&'a ClosedConformanceApplication, &'static str> {
    let descriptors = module
        .dynamic_dispatch
        .rebound_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == owner.id && descriptor.ordinal == descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return unsupported("rebound dynamic descriptor is absent or ambiguous");
    };
    if descriptor.initial_selection_ordinal == descriptor.rebound_selection_ordinal {
        return unsupported(
            "rebound dynamic descriptor does not rejoin two distinct conformance selections",
        );
    }
    let latest_selection = conformance_selection(
        module,
        owner.id,
        descriptor.rebound_selection_ordinal,
        family,
    )?;
    verify_source(
        owner,
        &latest_selection.source,
        latest.source_path,
        latest.source_access,
        latest.source_parameter_position,
        family,
    )?;
    let application = verify_application(
        checked,
        module,
        owner.id,
        latest_selection,
        latest.selected_conformance,
        latest.target_trait,
        latest.source_type_identity,
        &latest.selection.rows,
        family,
    )?;
    // The initial selection retains the descriptor's first source version. It
    // names the same application only when the checked conformance and row
    // roster are unchanged; a distinct application must rejoin the initial
    // selection's own checked custody.
    let initial_selection = conformance_selection(
        module,
        owner.id,
        descriptor.initial_selection_ordinal,
        family,
    )?;
    verify_source(
        owner,
        &initial_selection.source,
        &initial.path,
        latest.source_access,
        latest.source_parameter_position,
        family,
    )?;
    if initial.fact.conformance == latest.selection.conformance
        && initial.fact.rows == latest.selection.rows
    {
        if initial_selection.conformance_application_commitment
            != latest_selection.conformance_application_commitment
            || initial_selection.conformance_application_report_fingerprint
                != latest_selection.conformance_application_report_fingerprint
        {
            return unsupported(
                "rebound dynamic selections disagree about a shared conformance application",
            );
        }
    } else {
        let conformance = initial
            .fact
            .conformance
            .ok_or("rebound dynamic initial selection has no named conformance")?;
        verify_application(
            checked,
            module,
            owner.id,
            initial_selection,
            conformance,
            initial.fact.target_trait,
            &initial.type_identity,
            &initial.fact.rows,
            family,
        )?;
    }
    Ok(application)
}

/// The final helper's checked origin: the source machine and state the
/// dispatching helper replaces, its call coordinate, and its descriptor
/// parameter symbol.
struct ForwardedOrigin {
    machine: SymbolHandle,
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameter: SymbolHandle,
}

/// One root caller of a forwarded chain: its shared dispatch view and, for a
/// rebound binding, the initial selection the descriptor argument must
/// consume.
struct ForwardedRoot<'a> {
    view: DispatchView<'a>,
    initial: Option<&'a CheckedDynamicSelectionPlan>,
}

/// Whether a joined forwarded operation forwards the descriptor to the next
/// helper or dispatches through it. The two roles admit different operation
/// kinds and different descriptor-argument sources.
#[derive(Clone, Copy)]
enum ForwardedRole {
    Transfer,
    Dispatch,
}

/// The checked helper bodies name their replaced source machine and state
/// identically in either lane.
trait ForwardedHelperSource {
    fn source(&self) -> (SymbolHandle, SymbolHandle);
}

impl ForwardedHelperSource for checked_trees::CheckedDynamicScalarHelperPlan {
    fn source(&self) -> (SymbolHandle, SymbolHandle) {
        (self.machine, self.state)
    }
}

impl ForwardedHelperSource for checked_trees::CheckedDynamicUnitHelperPlan {
    fn source(&self) -> (SymbolHandle, SymbolHandle) {
        (self.machine, self.state)
    }
}

fn helper_sources<H: ForwardedHelperSource>(helpers: &[H]) -> Vec<(SymbolHandle, SymbolHandle)> {
    helpers.iter().map(ForwardedHelperSource::source).collect()
}

/// The terminal scalar type one checked primitive lowers to, mirroring the
/// lowering stage's exact mapping so a drifted helper result rejects.
fn primitive_scalar_type(primitive: PrimitiveType) -> Option<ScalarType> {
    let (sign, bits) = match primitive {
        PrimitiveType::Bool => return Some(ScalarType::Boolean),
        PrimitiveType::F32 => return Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
        PrimitiveType::F64 => return Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Addr => return IntegerType::address(64).ok().map(ScalarType::Integer),
    };
    IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
}

/// Both branches of a checked join forward through one shared chain, so
/// their origins must be one exact forwarded route. Two local branches emit
/// no chain, and mixed branches are drift, not a local plan.
fn joined_scalar_origin(
    when_true: &CheckedDynamicJoinBranchPlan<CheckedDynamicScalarCallPlan>,
    when_false: &CheckedDynamicJoinBranchPlan<CheckedDynamicScalarCallPlan>,
) -> Result<Option<ForwardedOrigin>, &'static str> {
    let forwarded = |origin: CheckedDynamicScalarCallOrigin| match origin {
        CheckedDynamicScalarCallOrigin::Forwarded {
            machine,
            state,
            coordinate,
            parameter,
        } => Some((machine, state, coordinate, parameter)),
        CheckedDynamicScalarCallOrigin::Local => None,
    };
    match (
        forwarded(when_true.call.origin),
        forwarded(when_false.call.origin),
    ) {
        (Some(origin), Some(other)) if origin == other => Ok(Some(ForwardedOrigin {
            machine: origin.0,
            state: origin.1,
            coordinate: origin.2,
            parameter: origin.3,
        })),
        (None, None) => Ok(None),
        _ => unsupported("joined dynamic branches disagree on forwarded custody"),
    }
}

fn joined_unit_origin(
    when_true: &CheckedDynamicJoinBranchPlan<CheckedDynamicUnitCallPlan>,
    when_false: &CheckedDynamicJoinBranchPlan<CheckedDynamicUnitCallPlan>,
) -> Result<Option<ForwardedOrigin>, &'static str> {
    let forwarded = |origin: CheckedDynamicUnitCallOrigin| match origin {
        CheckedDynamicUnitCallOrigin::Forwarded {
            machine,
            state,
            coordinate,
            parameter,
        } => Some((machine, state, coordinate, parameter)),
        CheckedDynamicUnitCallOrigin::Local => None,
    };
    match (
        forwarded(when_true.call.origin),
        forwarded(when_false.call.origin),
    ) {
        (Some(origin), Some(other)) if origin == other => Ok(Some(ForwardedOrigin {
            machine: origin.0,
            state: origin.1,
            coordinate: origin.2,
            parameter: origin.3,
        })),
        (None, None) => Ok(None),
        _ => unsupported("joined dynamic branches disagree on forwarded custody"),
    }
}

/// One forwarded chain's Terminal custody: every root's ordinary call into
/// the first generated helper, one ordinary forwarding call per checked
/// transfer inside each helper, and the final helper's parameter dispatch.
/// The chain is register-indirect: each generated machine carries one shared
/// descriptor-parameter interface, each hop's argument row names the exact
/// descriptor it receives, and only the last operation dispatches through
/// the parameter.
#[allow(clippy::too_many_arguments)]
fn replay_forwarded(
    checked: &CheckedTrees,
    module: &TerminalModule,
    source_calls: &[LoweredSourceCallOccurrence],
    plans: &checked_trees::CheckedDynamicDispatchPlans,
    roots: &[ForwardedRoot<'_>],
    origin: ForwardedOrigin,
    transfers: &[CheckedDynamicDescriptorTransferPlan],
    helpers: &[(SymbolHandle, SymbolHandle)],
    result: Option<PrimitiveType>,
    plan_index: usize,
    lane: CheckedDynamicCallLane,
    claimed_parameters: &mut BTreeSet<(MachineId, u32)>,
    claimed_arguments: &mut BTreeSet<(MachineId, OperationId)>,
) -> Result<Option<CheckedDynamicCallOccurrence>, &'static str> {
    let [first_root, extra_roots @ ..] = roots else {
        return unsupported("forwarded dynamic call has no checked root caller");
    };
    if extra_roots.len() > 1 || helpers.len() != transfers.len() + 1 {
        return unsupported("forwarded dynamic helper chain drifted from checked custody");
    }
    let scalar_type = result.map(|primitive| {
        primitive_scalar_type(primitive)
            .ok_or("forwarded dynamic call retained an unrepresentable scalar result")
    });
    let scalar_type = match scalar_type {
        Some(scalar_type) => Some(scalar_type?),
        None => None,
    };
    // Every hop draws the descriptor from the parameter its caller received,
    // sits in the checked transfer roster, and targets the next hop's caller
    // exactly; the first hop's predecessors are this plan's roots.
    let first_hop = |transfer: Option<&CheckedDynamicDescriptorTransferPlan>| {
        transfer.map_or((origin.machine, origin.state, origin.parameter), |first| {
            (
                first.caller_machine,
                first.caller_state,
                first.source_binding,
            )
        })
    };
    let (first_machine, first_state, first_binding) = first_hop(transfers.first());
    for (index, transfer) in transfers.iter().enumerate() {
        let expected_predecessors = if index == 0 { roots.len() } else { 1 };
        if transfer.source
            != (CheckedDynamicDescriptorTransferSource::Parameter {
                parameter_position: 0,
            })
            || transfer.parameter_position != 0
            || transfer.target_trait != first_root.view.target_trait
            || usize::try_from(transfer.source_predecessor_count) != Ok(expected_predecessors)
            || transfer.source_paths.len() != roots.len()
            || !plans.transfers.contains(transfer)
            || !transfer.has_complete_source_custody(&plans.transfers)
        {
            return unsupported("forwarded dynamic transfer drifted from its checked custody");
        }
        let (next_machine, next_state, next_binding) = first_hop(transfers.get(index + 1));
        if transfer.target_machine != next_machine
            || transfer.target_state != next_state
            || transfer.parameter != next_binding
        {
            return unsupported("forwarded dynamic chain does not link its checked hops");
        }
    }
    // The checked helper bodies cover every hop in chain order: each
    // transfer's caller, then the dispatching helper's source.
    for (index, (machine, state)) in helpers.iter().enumerate() {
        let (expected_machine, expected_state) = transfers
            .get(index)
            .map_or((origin.machine, origin.state), |transfer| {
                (transfer.caller_machine, transfer.caller_state)
            });
        if *machine != expected_machine || *state != expected_state {
            return unsupported("forwarded dynamic helper body drifted from its checked chain");
        }
    }
    // Each root's descriptor arrives through one owner-local selection
    // transfer in the checked roster; the roots of a join are distinct edges
    // into the same first hop, and no third caller may feed it.
    let mut root_edges = BTreeSet::new();
    for root in roots {
        let candidates = plans
            .transfers
            .iter()
            .filter(|transfer| {
                transfer.caller_machine == root.view.caller_machine
                    && transfer.caller_state == root.view.caller_state
                    && transfer.coordinate == root.view.coordinate
                    && transfer.target_machine == first_machine
                    && transfer.target_state == first_state
                    && transfer.parameter_position == 0
                    && transfer.parameter == first_binding
                    && transfer.target_trait == root.view.target_trait
                    && transfer.source_binding == root.view.receiver_binding
                    && transfer.source == CheckedDynamicDescriptorTransferSource::Selection
                    && transfer.source_predecessor_count == 0
                    && transfer.sole_selection() == Some(root.view.selection)
                    && transfer.has_complete_source_custody(&plans.transfers)
            })
            .collect::<Vec<_>>();
        let [root_transfer] = candidates.as_slice() else {
            return unsupported("forwarded dynamic call lost its checked root transfer");
        };
        if !root_edges.insert(root_transfer.edge().canonical_order_key()) {
            return unsupported("forwarded dynamic call's roots are not distinct checked edges");
        }
    }
    if plans
        .transfers
        .iter()
        .filter(|transfer| {
            transfer.target_machine == first_machine
                && transfer.target_state == first_state
                && transfer.parameter == first_binding
                && transfer.target_trait == first_root.view.target_trait
        })
        .count()
        != roots.len()
    {
        return unsupported("forwarded dynamic chain gained or lost a checked root transfer");
    }
    // Every root's ordinary call into the first helper names the same
    // callee; each root's argument row supplies the descriptor its own
    // selection or rebound descriptor materializes. Descriptor-source
    // ordinals are dense per owning caller, so a joined caller's roots take
    // them in root order while roots in separate callers each start at zero.
    let mut first_helper = None;
    let mut interfaces = Vec::new();
    let mut prior_owners = Vec::new();
    for root in roots {
        let joined = join_forwarded_operation(
            module,
            source_calls,
            root.view.caller_state,
            root.view.coordinate,
            lane,
            ForwardedRole::Transfer,
        )?;
        let Some((owner, operation, occurrence)) = joined else {
            if prior_owners.is_empty() {
                // The checked roster spans every dispatch-eligible machine; a
                // plan outside the selected composition legitimately has no
                // occurrence, and the completeness checks below still refuse
                // an orphaned emitted chain.
                return Ok(None);
            }
            return unsupported("forwarded dynamic call lost one joined root occurrence");
        };
        if occurrence.source_target != first_state {
            return unsupported("forwarded dynamic root call drifted from its checked target");
        }
        verify_forwarded_result(operation, scalar_type)?;
        let (OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallUnit { callee, .. }) = &operation.kind
        else {
            return unsupported(
                "forwarded dynamic call does not name one exact Terminal operation",
            );
        };
        match first_helper {
            Some(earlier) if earlier != *callee => {
                return unsupported("forwarded dynamic roots disagree on their first helper");
            }
            Some(_) => {}
            None => first_helper = Some(*callee),
        }
        let expected_ordinal = u32::try_from(
            prior_owners
                .iter()
                .filter(|prior| **prior == owner.id)
                .count(),
        )
        .map_err(|_| "forwarded dynamic root ordinal exceeds u32")?;
        prior_owners.push(owner.id);
        let argument = forwarded_argument(module, owner.id, operation.id, claimed_arguments)?;
        let application = match (argument.source, root.initial) {
            (TerminalDynamicDescriptorSource::Selection { ordinal }, None)
                if ordinal == expected_ordinal =>
            {
                let selection =
                    conformance_selection(module, owner.id, ordinal, DispatchFamily::Forwarded)?;
                verify_source(
                    owner,
                    &selection.source,
                    root.view.source_path,
                    root.view.source_access,
                    root.view.source_parameter_position,
                    DispatchFamily::Forwarded,
                )?;
                verify_application(
                    checked,
                    module,
                    owner.id,
                    selection,
                    root.view.selected_conformance,
                    root.view.target_trait,
                    root.view.source_type_identity,
                    &root.view.selection.rows,
                    DispatchFamily::Forwarded,
                )?
            }
            (TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal }, Some(initial))
                if ordinal == expected_ordinal =>
            {
                verify_rebound_descriptor(
                    checked,
                    module,
                    owner,
                    ordinal,
                    &root.view,
                    initial,
                    DispatchFamily::Forwarded,
                )?
            }
            _ => {
                return unsupported(
                    "forwarded dynamic argument drifted from its checked descriptor source",
                );
            }
        };
        let selected_row =
            selected_application_row(checked, DispatchFamily::Forwarded, &root.view, application)?;
        interfaces.push((
            application.trait_identity.as_str(),
            expected_interface(application, selected_row)?,
        ));
    }
    // A join's branches must expose one exact interface; a single root
    // trivially agrees with itself.
    let Some((trait_identity, (requirements, requirement_slot))) =
        interfaces
            .first()
            .map(|(trait_identity, (requirements, requirement_slot))| {
                (
                    *trait_identity,
                    (requirements.as_slice(), *requirement_slot),
                )
            })
    else {
        return unsupported("forwarded dynamic call lost its parameter interface");
    };
    if interfaces[1..]
        .iter()
        .any(|interface| interface != &interfaces[0])
    {
        return unsupported("joined dynamic conformances do not expose one exact interface");
    }
    let selected_requirement = requirements
        .iter()
        .find(|requirement| requirement.slot == requirement_slot)
        .ok_or("forwarded dynamic interface lost its selected requirement")?;
    if !forwarded_result_matches(selected_requirement.result, result) {
        return unsupported("forwarded dynamic dispatch drifted from its checked result shape");
    }
    let access = forwarded_access(first_root.view.source_access)?;
    // The helper chain in chain order: each transfer's occurrence names its
    // helper's forwarding call, whose callee is the next helper, and the
    // final occurrence names the parameter dispatch in the last one.
    let mut expected_helper = first_helper.ok_or("forwarded dynamic call has no first helper")?;
    for transfer in transfers {
        let Some((owner, operation, occurrence)) = join_forwarded_operation(
            module,
            source_calls,
            transfer.caller_state,
            transfer.coordinate,
            lane,
            ForwardedRole::Transfer,
        )?
        else {
            return unsupported("forwarded dynamic call lost a helper's transfer occurrence");
        };
        if owner.id != expected_helper {
            return unsupported("forwarded dynamic helper chain ordered its machines wrong");
        }
        if occurrence.source_target != transfer.target_state {
            return unsupported("forwarded dynamic transfer call drifted from its checked target");
        }
        verify_forwarded_result(operation, scalar_type)?;
        let (OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallUnit { callee, .. }) = &operation.kind
        else {
            return unsupported(
                "forwarded dynamic call does not name one exact Terminal operation",
            );
        };
        let argument = forwarded_argument(module, owner.id, operation.id, claimed_arguments)?;
        if argument.parameter_ordinal != 0
            || argument.source != (TerminalDynamicDescriptorSource::Parameter { ordinal: 0 })
        {
            return unsupported(
                "forwarded dynamic argument drifted from its checked parameter source",
            );
        }
        verify_forwarded_machine_result(owner, scalar_type)?;
        verify_parameter_interface(
            module,
            owner.id,
            trait_identity,
            access,
            requirements,
            claimed_parameters,
        )?;
        expected_helper = *callee;
    }
    let Some((owner, operation, occurrence)) = join_forwarded_operation(
        module,
        source_calls,
        origin.state,
        origin.coordinate,
        lane,
        ForwardedRole::Dispatch,
    )?
    else {
        return unsupported("forwarded dynamic call lost its final helper occurrence");
    };
    if owner.id != expected_helper {
        return unsupported("forwarded dynamic helper chain ordered its machines wrong");
    }
    if occurrence.source_target != first_root.view.requirement {
        return unsupported("forwarded dynamic dispatch drifted from its checked requirement");
    }
    verify_forwarded_result(operation, scalar_type)?;
    verify_forwarded_machine_result(owner, scalar_type)?;
    verify_parameter_interface(
        module,
        owner.id,
        trait_identity,
        access,
        requirements,
        claimed_parameters,
    )?;
    let (kind_ordinal, kind_slot) = match &operation.kind {
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal,
            requirement_slot: slot,
            ..
        }
        | OperationKind::CallDynamicParameterUnit {
            parameter_ordinal,
            requirement_slot: slot,
            ..
        } => (*parameter_ordinal, *slot),
        _ => {
            return unsupported(
                "forwarded dynamic call does not name one exact Terminal operation",
            );
        }
    };
    if kind_ordinal != 0 || kind_slot != requirement_slot {
        return unsupported("forwarded dynamic dispatch drifted from its checked interface");
    }
    let dispatches = module
        .dynamic_dispatch
        .parameter_dispatches
        .iter()
        .filter(|dispatch| dispatch.owner == owner.id && dispatch.operation == operation.id)
        .collect::<Vec<_>>();
    let [dispatch] = dispatches.as_slice() else {
        return unsupported(
            "forwarded dynamic call does not rejoin one exact Terminal parameter dispatch",
        );
    };
    if dispatch.parameter_ordinal != kind_ordinal || dispatch.requirement_slot != kind_slot {
        return unsupported("forwarded dynamic dispatch drifted from its checked interface");
    }
    Ok(Some(CheckedDynamicCallOccurrence::new(
        lane,
        plan_index,
        owner.id,
        operation.id,
        kind_ordinal,
    )))
}

/// The shared source-coordinate join for forwarded hops: exactly one
/// occurrence, exactly one owning machine, and the operation kind the hop's
/// role admits.
fn join_forwarded_operation<'a>(
    module: &'a TerminalModule,
    source_calls: &'a [LoweredSourceCallOccurrence],
    source_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    lane: CheckedDynamicCallLane,
    role: ForwardedRole,
) -> Result<
    Option<(
        &'a TerminalMachine,
        &'a Operation,
        &'a LoweredSourceCallOccurrence,
    )>,
    &'static str,
> {
    let statement_index = usize::try_from(coordinate.statement_index)
        .map_err(|_| "forwarded dynamic call statement coordinate exceeds usize")?;
    let call_ordinal = usize::try_from(coordinate.call_ordinal)
        .map_err(|_| "forwarded dynamic call ordinal exceeds usize")?;
    let matching = source_calls
        .iter()
        .filter(|occurrence| {
            occurrence.source_state == source_state
                && occurrence.statement_index == statement_index
                && occurrence.call_ordinal == call_ordinal
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Ok(None);
    }
    let [occurrence] = matching.as_slice() else {
        return unsupported("forwarded dynamic call maps to duplicate Terminal occurrences");
    };
    let owners = module
        .machines
        .iter()
        .filter_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == occurrence.terminal_operation)
                .map(|operation| (machine, operation))
        })
        .collect::<Vec<_>>();
    let [(owner, operation)] = owners.as_slice() else {
        return unsupported("forwarded dynamic call does not name one exact Terminal operation");
    };
    let admitted = match (role, lane, &operation.kind) {
        (
            ForwardedRole::Transfer,
            CheckedDynamicCallLane::ForwardedScalar,
            OperationKind::CallStructuralScalar { .. },
        )
        | (
            ForwardedRole::Transfer,
            CheckedDynamicCallLane::ForwardedUnit,
            OperationKind::CallUnit { .. },
        )
        | (
            ForwardedRole::Dispatch,
            CheckedDynamicCallLane::ForwardedScalar,
            OperationKind::CallDynamicParameterScalar { .. },
        )
        | (
            ForwardedRole::Dispatch,
            CheckedDynamicCallLane::ForwardedUnit,
            OperationKind::CallDynamicParameterUnit { .. },
        ) => true,
        _ => false,
    };
    if !admitted {
        return unsupported("forwarded dynamic call does not name one exact Terminal operation");
    }
    Ok(Some((owner, operation, occurrence)))
}

/// A forwarded call's result must be the lane's own shape: the checked
/// scalar's exact terminal type, or Unit.
fn verify_forwarded_result(
    operation: &Operation,
    scalar_type: Option<ScalarType>,
) -> Result<(), &'static str> {
    match (scalar_type, &operation.result) {
        (Some(expected), OperationResult::Scalar(declaration))
            if declaration.scalar_type == expected =>
        {
            Ok(())
        }
        (None, OperationResult::Unit) => Ok(()),
        _ => unsupported("forwarded dynamic call result drifted from its checked shape"),
    }
}

/// Every generated helper returns the lane's result: the checked scalar's
/// exact terminal type, or Unit.
fn verify_forwarded_machine_result(
    owner: &TerminalMachine,
    scalar_type: Option<ScalarType>,
) -> Result<(), &'static str> {
    match (scalar_type, &owner.result) {
        (Some(expected), TerminalMachineResult::Scalar(declaration))
            if declaration.scalar_type == expected =>
        {
            Ok(())
        }
        (None, TerminalMachineResult::Unit) => Ok(()),
        _ => unsupported("forwarded dynamic helper result drifted from its checked shape"),
    }
}

/// The one descriptor-argument row a forwarded hop carries.
fn forwarded_argument<'a>(
    module: &'a TerminalModule,
    owner: MachineId,
    operation: OperationId,
    claimed_arguments: &mut BTreeSet<(MachineId, OperationId)>,
) -> Result<&'a terminal_psi::TerminalDynamicDescriptorArgument, &'static str> {
    let arguments = module
        .dynamic_dispatch
        .arguments
        .iter()
        .filter(|argument| argument.owner == owner && argument.operation == operation)
        .collect::<Vec<_>>();
    let [argument] = arguments.as_slice() else {
        return unsupported("forwarded dynamic argument is absent or ambiguous");
    };
    if argument.parameter_ordinal != 0 || !claimed_arguments.insert((owner, operation)) {
        return unsupported("forwarded dynamic argument is shared by two checked chains");
    }
    Ok(argument)
}

/// One helper's descriptor-parameter row: the shared interface in ordinal
/// zero, checked coordinates only — no realization identity, which the
/// caller's descriptor supplies at each invocation.
fn verify_parameter_interface(
    module: &TerminalModule,
    owner: MachineId,
    trait_identity: &str,
    access: StructuralAccess,
    requirements: &[TerminalDynamicRequirement],
    claimed_parameters: &mut BTreeSet<(MachineId, u32)>,
) -> Result<(), &'static str> {
    let parameters = module
        .dynamic_dispatch
        .parameters
        .iter()
        .filter(|parameter| parameter.owner == owner && parameter.ordinal == 0)
        .collect::<Vec<_>>();
    let [parameter] = parameters.as_slice() else {
        return unsupported("forwarded dynamic parameter interface is absent or ambiguous");
    };
    if parameter.source_position != 0
        || parameter.trait_identity != trait_identity
        || parameter.access != access
        || parameter.requirements[..] != *requirements
    {
        return unsupported(
            "forwarded dynamic parameter interface drifted from its checked custody",
        );
    }
    if !claimed_parameters.insert((owner, parameter.ordinal)) {
        return unsupported("forwarded dynamic parameter is shared by two checked chains");
    }
    Ok(())
}

/// The application row a forwarded call selects, by its checked slot
/// coordinates — the same replay `verify_selected_row` performs before the
/// dispatch-row comparison it additionally owns.
fn selected_application_row<'a>(
    checked: &CheckedTrees,
    family: DispatchFamily,
    latest: &DispatchView<'_>,
    application: &'a ClosedConformanceApplication,
) -> Result<&'a ClosedConformanceRow, &'static str> {
    let selected_rows = application
        .rows
        .iter()
        .filter(|application_row| {
            application_row.declaring_trait_identity
                == checked.symbols.display_path(latest.declaring_trait, "::")
                && application_row.public_requirement_identity == latest.requirement_identity
                && application_row.family_tuple[..] == *latest.family_tuple
                && application_row.requirement_identity
                    == checked.symbols.display_path(latest.requirement, "::")
                && application_row.realization_identity
                    == checked.symbols.display_path(latest.realization_state, "::")
        })
        .collect::<Vec<_>>();
    let [selected_row] = selected_rows.as_slice() else {
        return unsupported(family.message(
            "rebound dynamic dispatch does not rejoin its selected conformance row",
            "stored dynamic dispatch does not rejoin its selected conformance row",
            "forwarded dynamic dispatch does not rejoin its selected conformance row",
        ));
    };
    Ok(selected_row)
}

/// The existential interface and selected slot one checked selection's
/// application exposes: every application row contributes one callable slot
/// in order, and the selected row's index is the requirement slot the
/// parameter dispatch names.
fn expected_interface(
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
) -> Result<(Vec<TerminalDynamicRequirement>, u32), &'static str> {
    let mut selected_slot = None;
    let mut requirements = Vec::with_capacity(application.rows.len());
    for (ordinal, row) in application.rows.iter().enumerate() {
        let slot = u32::try_from(ordinal)
            .map_err(|_| "forwarded dynamic parameter requirement ordinal exceeds u32")?;
        if row == selected_row && selected_slot.replace(slot).is_some() {
            return unsupported("forwarded dynamic selected requirement is duplicated");
        }
        let callable_identity = row
            .realization_callable_identity
            .as_deref()
            .ok_or("forwarded dynamic application row has no realization callable")?;
        let matching = application
            .realization_callables
            .iter()
            .filter(|callable| callable.source_callable_identity == callable_identity)
            .collect::<Vec<_>>();
        let [callable] = matching.as_slice() else {
            return unsupported(
                "forwarded dynamic application row callable is absent or ambiguous",
            );
        };
        requirements.push(TerminalDynamicRequirement {
            slot,
            declaring_trait_identity: row.declaring_trait_identity.clone(),
            public_requirement_identity: row.public_requirement_identity.clone(),
            family_tuple: row.family_tuple.clone(),
            result: callable.result,
        });
    }
    let slot = selected_slot.ok_or("forwarded dynamic interface lost its selected requirement")?;
    Ok((requirements, slot))
}

/// The parameter interface's callable-result vocabulary is narrower than
/// the primitive roster: a forwarded call admits only the results it can
/// represent, and each must be the checked lane's exact shape.
fn forwarded_result_matches(
    result: ClosedConformanceCallableResult,
    scalar: Option<PrimitiveType>,
) -> bool {
    match (result, scalar) {
        (ClosedConformanceCallableResult::Unit, None) => true,
        (ClosedConformanceCallableResult::I32, Some(PrimitiveType::I32))
        | (ClosedConformanceCallableResult::Bool, Some(PrimitiveType::Bool)) => true,
        _ => false,
    }
}

/// The parameter interface retains the descriptor argument's borrow access;
/// any other checked access cannot form a descriptor argument.
fn forwarded_access(access: CheckedStructuralAccess) -> Result<StructuralAccess, &'static str> {
    match access {
        CheckedStructuralAccess::SharedBorrow => Ok(StructuralAccess::SharedBorrow),
        CheckedStructuralAccess::MutableBorrow => Ok(StructuralAccess::MutableBorrow),
        _ => unsupported("forwarded dynamic descriptor source drifted from its checked selection"),
    }
}

/// One checked stored scalar call's Terminal custody: the `CallDynamicScalar`
/// operation, the stored-dispatch row carrying its selected conformance
/// coordinates, the stored descriptor whose `StoreDynamicDescriptor`
/// establishment and aggregate field custody match the checked storage plan,
/// and the single conformance selection the reload consumes.
fn replay_stored(
    checked: &CheckedTrees,
    module: &TerminalModule,
    source_calls: &[LoweredSourceCallOccurrence],
    latest: DispatchView<'_>,
    descriptor_plan: &CheckedDynamicStoredDescriptorPlan,
    plan_index: usize,
) -> Result<Option<CheckedDynamicCallOccurrence>, &'static str> {
    let Some((owner, terminal_operation, descriptor_ordinal)) = join_call_operation(
        module,
        source_calls,
        &latest,
        CheckedDynamicCallLane::StoredScalar,
    )?
    else {
        return Ok(None);
    };
    let dispatches = module
        .dynamic_dispatch
        .stored_dispatches
        .iter()
        .filter(|dispatch| dispatch.owner == owner.id && dispatch.operation == terminal_operation)
        .collect::<Vec<_>>();
    let [dispatch] = dispatches.as_slice() else {
        return unsupported(
            "stored dynamic call does not rejoin one exact Terminal stored dispatch",
        );
    };
    let dispatch_row = DispatchRow::from(*dispatch);
    verify_dispatch_row(
        checked,
        module,
        DispatchFamily::Stored,
        &latest,
        descriptor_ordinal,
        &dispatch_row,
    )?;
    let descriptors = module
        .dynamic_dispatch
        .stored_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == owner.id && descriptor.ordinal == dispatch.descriptor_ordinal
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return unsupported("stored dynamic descriptor is absent or ambiguous");
    };
    if descriptor.aggregate_type_identity != descriptor_plan.destination_type_identity
        || descriptor.field_identity != descriptor_plan.destination_field_identity
    {
        return unsupported("stored dynamic descriptor drifted from its checked aggregate custody");
    }
    // The descriptor's establishment must be an exact `StoreDynamicDescriptor`
    // in the owning machine naming this descriptor's ordinal.
    let establishments = owner
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == descriptor.establishment_operation)
        .collect::<Vec<_>>();
    let [establishment] = establishments.as_slice() else {
        return unsupported("stored dynamic descriptor names an absent establishment operation");
    };
    if !matches!(
        establishment.kind,
        OperationKind::StoreDynamicDescriptor {
            descriptor_ordinal: ordinal
        } if ordinal == descriptor.ordinal
    ) {
        return unsupported("stored dynamic descriptor names an unrelated establishment operation");
    }
    // The stored descriptor reloads the call's own latest selection: it must
    // carry the checked structural source and the exact conformance
    // application the call's retained row roster names.
    let selection = conformance_selection(
        module,
        owner.id,
        descriptor.selection_ordinal,
        DispatchFamily::Stored,
    )?;
    verify_source(
        owner,
        &selection.source,
        latest.source_path,
        latest.source_access,
        latest.source_parameter_position,
        DispatchFamily::Stored,
    )?;
    let application = verify_application(
        checked,
        module,
        owner.id,
        selection,
        latest.selected_conformance,
        latest.target_trait,
        latest.source_type_identity,
        &latest.selection.rows,
        DispatchFamily::Stored,
    )?;
    verify_selected_row(
        checked,
        DispatchFamily::Stored,
        &latest,
        application,
        &dispatch_row,
    )?;
    Ok(Some(CheckedDynamicCallOccurrence::new(
        CheckedDynamicCallLane::StoredScalar,
        plan_index,
        owner.id,
        terminal_operation,
        dispatch.descriptor_ordinal,
    )))
}

/// The dispatch row must consume the operation's own descriptor ordinal,
/// repeat the checked call's exact selected-row coordinates, and name an
/// emitted realization machine in the same module.
fn verify_dispatch_row(
    checked: &CheckedTrees,
    module: &TerminalModule,
    family: DispatchFamily,
    latest: &DispatchView<'_>,
    operation_descriptor_ordinal: u32,
    row: &DispatchRow<'_>,
) -> Result<(), &'static str> {
    if row.descriptor_ordinal != operation_descriptor_ordinal
        || row.declaring_trait_identity
            != checked.symbols.display_path(latest.declaring_trait, "::")
        || row.public_requirement_identity != latest.requirement_identity
        || *row.family_tuple != *latest.family_tuple
        || row.requirement_identity != checked.symbols.display_path(latest.requirement, "::")
        || row.realization_identity != checked.symbols.display_path(latest.realization_state, "::")
    {
        return unsupported(family.message(
            "rebound dynamic dispatch drifted from its checked selection",
            "stored dynamic dispatch drifted from its checked selection",
            "forwarded dynamic dispatch drifted from its checked selection",
        ));
    }
    if !module
        .machines
        .iter()
        .any(|machine| machine.id == row.realization)
    {
        return unsupported(family.message(
            "rebound dynamic dispatch names an absent Terminal realization",
            "stored dynamic dispatch names an absent Terminal realization",
            "forwarded dynamic dispatch names an absent Terminal realization",
        ));
    }
    Ok(())
}

/// The dispatch row's coordinates must select exactly one application row,
/// and the callable it names must be the one realization callable that row
/// retains.
fn verify_selected_row(
    checked: &CheckedTrees,
    family: DispatchFamily,
    latest: &DispatchView<'_>,
    application: &ClosedConformanceApplication,
    row: &DispatchRow<'_>,
) -> Result<(), &'static str> {
    let selected_row = selected_application_row(checked, family, latest, application)?;
    if selected_row.realization_callable_identity.as_deref()
        != Some(row.realization_callable_identity)
        || application
            .realization_callables
            .iter()
            .filter(|callable| {
                callable.source_callable_identity == row.realization_callable_identity
                    && callable.machine == row.realization
            })
            .count()
            != 1
    {
        return unsupported(family.message(
            "rebound dynamic dispatch names a callable outside its conformance application",
            "stored dynamic dispatch names a callable outside its conformance application",
            "forwarded dynamic dispatch names a callable outside its conformance application",
        ));
    }
    Ok(())
}

fn conformance_selection(
    module: &TerminalModule,
    owner: MachineId,
    ordinal: u32,
    family: DispatchFamily,
) -> Result<&TerminalDynamicConformanceSelection, &'static str> {
    let selections = module
        .dynamic_dispatch
        .selections
        .iter()
        .filter(|selection| selection.owner == owner && selection.ordinal == ordinal)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return unsupported(family.message(
            "rebound dynamic conformance selection is absent or ambiguous",
            "stored dynamic conformance selection is absent or ambiguous",
            "forwarded dynamic conformance selection is absent or ambiguous",
        ));
    };
    Ok(selection)
}

/// The selection's recorded structural source must be the checked path and
/// borrow beneath the owning machine's exact `self` parameter.
fn verify_source(
    owner: &TerminalMachine,
    source: &StructuralArgument,
    path: &[CheckedUnitStructuralPathSegment],
    access: CheckedStructuralAccess,
    source_parameter_position: u32,
    family: DispatchFamily,
) -> Result<(), &'static str> {
    let expected_access = match access {
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => {
            return unsupported(family.message(
                "rebound dynamic descriptor source drifted from its checked selection",
                "stored dynamic descriptor source drifted from its checked selection",
                "forwarded dynamic descriptor source drifted from its checked selection",
            ));
        }
    };
    // A dynamic descriptor lives at one exact place: its selection source is
    // an unconditional static projection. Neither a runtime-selected element
    // nor a projection requiring live case membership matches that contract.
    let expected_path = path
        .iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                Some(StructuralPathSegment::Field(identity.clone()))
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                Some(StructuralPathSegment::FixedIndex(*index))
            }
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                Some(StructuralPathSegment::FixedByteRange {
                    start: *start,
                    end: *end,
                })
            }
            CheckedUnitStructuralPathSegment::Case(_)
            | CheckedUnitStructuralPathSegment::RuntimeIndex { .. } => None,
            CheckedUnitStructuralPathSegment::Referent => Some(StructuralPathSegment::Referent),
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(family.message(
            "rebound dynamic descriptor source drifted from its checked selection",
            "stored dynamic descriptor source drifted from its checked selection",
            "forwarded dynamic descriptor source drifted from its checked selection",
        ))?;
    let self_parameters = owner
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    let [self_parameter] = self_parameters.as_slice() else {
        return unsupported(family.message(
            "rebound dynamic source lost its exact caller self parameter",
            "stored dynamic source lost its exact caller self parameter",
            "forwarded dynamic source lost its exact caller self parameter",
        ));
    };
    if self_parameter.position != source_parameter_position
        || source.place != self_parameter.place
        || source.path != expected_path
        || source.access != expected_access
    {
        return unsupported(family.message(
            "rebound dynamic descriptor source drifted from its checked selection",
            "stored dynamic descriptor source drifted from its checked selection",
            "forwarded dynamic descriptor source drifted from its checked selection",
        ));
    }
    Ok(())
}

/// The selection's commitment must name exactly one closed conformance
/// application in the owning machine, and that application's rows must replay
/// the checked selection's retained roster slot by slot. Family-expanded
/// generic rows share their retained row's slot triple; a nongeneric row
/// degenerates to the retained realization state itself.
fn verify_application<'a>(
    checked: &CheckedTrees,
    module: &'a TerminalModule,
    owner: MachineId,
    selection: &TerminalDynamicConformanceSelection,
    conformance: SymbolHandle,
    target_trait: SymbolHandle,
    subject_identity: &str,
    retained_rows: &[DynamicConformanceRowFact],
    family: DispatchFamily,
) -> Result<&'a ClosedConformanceApplication, &'static str> {
    let applications = module
        .closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == owner
                && application.commitment == selection.conformance_application_commitment
                && application.report_fingerprint
                    == selection.conformance_application_report_fingerprint
        })
        .collect::<Vec<_>>();
    let [application] = applications.as_slice() else {
        return unsupported(family.message(
            "rebound dynamic selection names an absent conformance application",
            "stored dynamic selection names an absent conformance application",
            "forwarded dynamic selection names an absent conformance application",
        ));
    };
    let drifted = family.message(
        "rebound dynamic conformance application drifted from its checked selection",
        "stored dynamic conformance application drifted from its checked selection",
        "forwarded dynamic conformance application drifted from its checked selection",
    );
    if application.declaration_identity != checked.symbols.display_path(conformance, "::")
        || application.trait_identity != checked.symbols.display_path(target_trait, "::")
        || application.subject_identity.as_deref() != Some(subject_identity)
        || !application.telescope.is_empty()
        || !application.trait_lifetime_arguments.is_empty()
        || !application.trait_arguments.is_empty()
    {
        return unsupported(drifted);
    }
    // Rows are emitted grouped by retained checked row: each group shares one
    // slot triple and expands the requirement's roster tuples in order.
    let mut groups: Vec<((&str, &str, &str), usize, usize)> = Vec::new();
    for (index, row) in application.rows.iter().enumerate() {
        let key = (
            row.declaring_trait_identity.as_str(),
            row.public_requirement_identity.as_str(),
            row.requirement_identity.as_str(),
        );
        match groups.last_mut() {
            Some((last_key, _, end)) if *last_key == key => *end += 1,
            _ => groups.push((key, index, index + 1)),
        }
    }
    if groups.len() != retained_rows.len() {
        return unsupported(drifted);
    }
    for ((key, start, end), retained) in groups.iter().zip(retained_rows) {
        if *key
            != (
                checked
                    .symbols
                    .display_path(retained.declaring_trait, "::")
                    .as_str(),
                retained.requirement_identity.as_str(),
                checked
                    .symbols
                    .display_path(retained.requirement, "::")
                    .as_str(),
            )
        {
            return unsupported(drifted);
        }
        let group = &application.rows[*start..*end];
        let mut tuples = BTreeSet::new();
        let mut nongeneric = false;
        for row in group {
            if !tuples.insert(&row.family_tuple) {
                return unsupported(family.message(
                    "rebound dynamic conformance application repeats a family tuple",
                    "stored dynamic conformance application repeats a family tuple",
                    "forwarded dynamic conformance application repeats a family tuple",
                ));
            }
            nongeneric |= row.family_tuple.is_empty();
        }
        if nongeneric {
            // A nongeneric requirement expands to exactly its own empty-tuple
            // row, whose realization is the retained row's state itself.
            let [row] = group else {
                return unsupported(drifted);
            };
            if !row.family_tuple.is_empty()
                || row.realization_identity
                    != checked
                        .symbols
                        .display_path(retained.realization_state, "::")
            {
                return unsupported(drifted);
            }
        }
    }
    Ok(application)
}

#[cfg(test)]
mod tests {
    //! End-to-end replay checks for the stored and forwarded descriptor
    //! lanes: the checked plan, its establishment and descriptor custody, its
    //! dispatch rows and its conformance selections must all rejoin one exact
    //! occurrence, and drifted or stale custody rejects. The forwarded lanes
    //! additionally replay the generated helper chain hop by hop.
    use crate::{CheckedDynamicCallLane, checked_boundary_operator_scope};
    use checked_trees::{
        CheckedDynamicBinding, CheckedDynamicDispatchPlan, CheckedDynamicScalarCallPlan,
        CheckedTrees,
    };
    use checked_trees_to_lowered_psi::TerminalMachineSelection;
    use terminal_production::{
        ProducedTerminalArtifact, TerminalProductionCustody, TerminalProductionRequest,
        TerminalProductionTimings,
    };
    use terminal_psi::{OperationKind, TerminalDynamicDescriptorSource};

    fn check_source(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap()
    }

    /// One stored descriptor-table call: the `&'item dyn Measure` selection
    /// is established in `holder.handler` and the call reloads it.
    const STORED_DYNAMIC_SOURCE: &str = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item [copy] {
            value: i32;
        }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Holder<'item> {
            handler: &'item dyn Measure;
        }

        data Main [copy] {
            item: Item;
        }

        machine Main::run<'item>(&self) {
            let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
            let holder: Holder<'item> = Holder { handler: erased };
            let result: i32 = holder.handler.measure();
        }
    "#;

    fn stored_dynamic_produced(source: &str) -> (CheckedTrees, ProducedTerminalArtifact<()>) {
        let checked = check_source(source);
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: true,
                    entry_identity: None,
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .expect("stored dynamic production publishes");
        (checked, produced)
    }

    /// The checked stored plan inside the roster, for drift mutations.
    fn stored_scalar_mut(checked: &mut CheckedTrees) -> &mut CheckedDynamicScalarCallPlan {
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .calls
            .iter_mut()
            .find_map(|plan| match plan {
                CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Stored {
                    call, ..
                }) => Some(call),
                _ => None,
            })
            .expect("one stored scalar dynamic plan")
    }

    #[test]
    fn a_stored_dynamic_call_rejoins_its_exact_descriptor_table_occurrence() {
        let (checked, produced) = stored_dynamic_produced(STORED_DYNAMIC_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        let catalog = &module.dynamic_dispatch;
        let [dispatch] = catalog.stored_dispatches.as_slice() else {
            panic!("one stored dispatch row");
        };
        let [descriptor] = catalog.stored_descriptors.as_slice() else {
            panic!("one stored descriptor");
        };
        assert!(catalog.indirect_dispatches.is_empty());
        assert!(catalog.direct_dispatches.is_empty());
        assert!(catalog.parameter_dispatches.is_empty());
        assert_eq!(catalog.selections.len(), 1);
        assert_eq!(dispatch.descriptor_ordinal, descriptor.ordinal);
        // The establishment operation names the emitted StoreDynamicDescriptor
        // in the same owner.
        let owner = module
            .machines
            .iter()
            .find(|machine| machine.id == dispatch.owner)
            .expect("stored dispatch owner machine");
        let establishment = owner
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == descriptor.establishment_operation)
            .expect("the descriptor names an emitted establishment operation");
        assert!(matches!(
            establishment.kind,
            terminal_psi::OperationKind::StoreDynamicDescriptor { descriptor_ordinal }
                if descriptor_ordinal == descriptor.ordinal
        ));
        let scope = checked_boundary_operator_scope(
            &checked,
            produced.artifact(),
            produced
                .unoptimized()
                .expect("custody retained the lowered module"),
        )
        .expect("the recorded roster rejoins its checked stored plan");
        let [occurrence] = scope.dynamic_call_occurrences() else {
            panic!("one replayed dynamic call occurrence");
        };
        assert_eq!(occurrence.lane(), CheckedDynamicCallLane::StoredScalar);
        assert_eq!(occurrence.plan_index(), 0);
        assert_eq!(occurrence.terminal_machine(), dispatch.owner);
        assert_eq!(occurrence.terminal_operation(), dispatch.operation);
        assert_eq!(occurrence.descriptor_ordinal(), descriptor.ordinal);
        assert_eq!(
            produced.source_call_occurrences()[0].terminal_operation,
            dispatch.operation
        );
        // The scope production retained belongs to the dev-dependency's own
        // build of this crate, so the rosters compare by representation.
        assert_eq!(
            format!("{:?}", scope.dynamic_call_occurrences()),
            format!(
                "{:?}",
                produced
                    .boundary_operator_scope()
                    .dynamic_call_occurrences()
            ),
        );
    }

    #[test]
    fn a_stale_or_duplicated_stored_occurrence_rejects() {
        use semantic_vocabulary::OperationId;
        let (checked, produced) = stored_dynamic_produced(STORED_DYNAMIC_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay = |corrupted: &lowered_psi::LoweredPsi| {
            checked_boundary_operator_scope(&checked, produced.artifact(), corrupted).unwrap_err()
        };
        // A row naming an operation the module no longer carries.
        let mut corrupted = lowered.clone();
        corrupted.source_call_occurrences[0].terminal_operation =
            OperationId::new(u64::MAX).expect("unused operation identity");
        assert_eq!(
            replay(&corrupted),
            "stored dynamic call does not name one exact Terminal operation"
        );
        // A duplicated occurrence row.
        let mut corrupted = lowered.clone();
        let duplicate = corrupted.source_call_occurrences[0].clone();
        corrupted.source_call_occurrences.push(duplicate);
        assert_eq!(
            replay(&corrupted),
            "stored dynamic call maps to duplicate Terminal occurrences"
        );
        // A stale coordinate leaves the surviving stored dispatch unchecked.
        let mut corrupted = lowered.clone();
        corrupted.source_call_occurrences[0].call_ordinal += 1;
        assert_eq!(
            replay(&corrupted),
            "Terminal stored dispatch does not rejoin one checked stored call"
        );
    }

    #[test]
    fn a_substituted_stored_selection_identity_rejects() {
        let (checked, produced) = stored_dynamic_produced(STORED_DYNAMIC_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay = |corrupted: &CheckedTrees| {
            checked_boundary_operator_scope(corrupted, produced.artifact(), &lowered).unwrap_err()
        };
        let drifted = "stored dynamic dispatch drifted from its checked selection";
        // A checked plan whose requirement identity no longer matches the
        // emitted dispatch row.
        let mut corrupted = checked.clone();
        stored_scalar_mut(&mut corrupted).requirement_identity = "foreign".to_owned();
        assert_eq!(replay(&corrupted), drifted);
        // A checked plan whose family tuple is substituted.
        let mut corrupted = checked.clone();
        stored_scalar_mut(&mut corrupted).family_tuple = Box::from(["foreign".to_owned()]);
        assert_eq!(replay(&corrupted), drifted);
        // A stale checked coordinate leaves the stored dispatch unrejoined.
        let mut corrupted = checked.clone();
        stored_scalar_mut(&mut corrupted).coordinate.statement_index += 1;
        assert_eq!(
            replay(&corrupted),
            "Terminal stored dispatch does not rejoin one checked stored call"
        );
        // A retained conformance row whose requirement identity drifted.
        let mut corrupted = checked.clone();
        stored_scalar_mut(&mut corrupted).selection.rows[0].requirement_identity =
            "foreign".to_owned();
        assert_eq!(
            replay(&corrupted),
            "stored dynamic conformance application drifted from its checked selection"
        );
        // Drifted destination custody no longer names the emitted descriptor.
        let mut corrupted = checked.clone();
        let CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Stored {
            descriptor, ..
        }) = &mut corrupted
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .calls[0]
        else {
            unreachable!("checked above")
        };
        descriptor.destination_field_identity = "other".into();
        assert_eq!(
            replay(&corrupted),
            "stored dynamic descriptor drifted from its checked aggregate custody"
        );
    }

    /// The artifact wrapper seals `semantic_module` by identity, so catalog
    /// mutations exercise the replay body directly rather than the scope's
    /// artifact comparison.
    #[test]
    fn a_stored_dynamic_descriptor_requires_its_exact_establishment() {
        use semantic_vocabulary::OperationId;
        let (checked, produced) = stored_dynamic_produced(STORED_DYNAMIC_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay =
            |corrupted: &lowered_psi::LoweredPsi| super::replay(&checked, corrupted).unwrap_err();
        // An establishment identity no operation carries is absent.
        let mut corrupted = lowered.clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .stored_descriptors[0]
            .establishment_operation = OperationId::new(u64::MAX).unwrap();
        assert_eq!(
            replay(&corrupted),
            "stored dynamic descriptor names an absent establishment operation"
        );
        // Swapping in the dispatch operation exchanges the roles the rows
        // name: the call operation is not a `StoreDynamicDescriptor`.
        let mut corrupted = lowered.clone();
        let dispatch_operation =
            corrupted.semantic_module.dynamic_dispatch.stored_dispatches[0].operation;
        corrupted
            .semantic_module
            .dynamic_dispatch
            .stored_descriptors[0]
            .establishment_operation = dispatch_operation;
        assert_eq!(
            replay(&corrupted),
            "stored dynamic descriptor names an unrelated establishment operation"
        );
        // A duplicated stored descriptor makes the dispatch's row ambiguous.
        let mut corrupted = lowered.clone();
        let duplicate = corrupted
            .semantic_module
            .dynamic_dispatch
            .stored_descriptors[0]
            .clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .stored_descriptors
            .push(duplicate);
        assert_eq!(
            replay(&corrupted),
            "stored dynamic descriptor is absent or ambiguous"
        );
        // An orphan stored descriptor has no consuming dispatch.
        let mut corrupted = lowered.clone();
        let mut orphan = corrupted
            .semantic_module
            .dynamic_dispatch
            .stored_descriptors[0]
            .clone();
        orphan.ordinal = 1;
        corrupted
            .semantic_module
            .dynamic_dispatch
            .stored_descriptors
            .push(orphan);
        assert_eq!(
            replay(&corrupted),
            "Terminal stored descriptor does not rejoin exactly one checked stored call"
        );
        // A dropped stored dispatch leaves its row unrejoined.
        let mut corrupted = lowered.clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .stored_dispatches
            .clear();
        assert_eq!(
            replay(&corrupted),
            "stored dynamic call does not rejoin one exact Terminal stored dispatch"
        );
        // A selection row renumbered away leaves the descriptor's selection
        // absent.
        let mut corrupted = lowered.clone();
        corrupted.semantic_module.dynamic_dispatch.selections[0].ordinal = 9;
        assert_eq!(
            replay(&corrupted),
            "stored dynamic conformance selection is absent or ambiguous"
        );
    }

    #[test]
    fn a_foreign_stored_dynamic_scope_rejects() {
        let (_checked, produced) = stored_dynamic_produced(STORED_DYNAMIC_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        // A program whose stored plan names a different requirement symbol:
        // replaying the published roster under foreign checked custody either
        // drifts on the row identities or leaves the dispatch unrejoined.
        let foreign = check_source(&STORED_DYNAMIC_SOURCE.replace("measure", "gauge"));
        let error =
            checked_boundary_operator_scope(&foreign, produced.artifact(), &lowered).unwrap_err();
        assert!(
            [
                "stored dynamic dispatch drifted from its checked selection",
                "Terminal stored dispatch does not rejoin one checked stored call",
            ]
            .contains(&error),
            "unexpected replay failure: {error}"
        );
    }

    /// One forwarded scalar call on a direct selection: `Main::run` supplies
    /// the descriptor and the generated `forward` helper dispatches through
    /// its existential parameter.
    const FORWARDED_DYNAMIC_INTEGER_SOURCE: &str = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item [copy] {
            value: i32;
        }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main [copy] {
            selected: Item;
        }

        machine Main::run(&self) {
            let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;

    /// One forwarded scalar call on a rebound descriptor: the caller
    /// materializes the two-selection descriptor and passes it as the helper
    /// argument.
    const FORWARDED_REBOUND_DYNAMIC_INTEGER_SOURCE: &str = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;

    /// One forwarded Unit call on a direct selection.
    const FORWARDED_DYNAMIC_UNIT_SOURCE: &str = r#"
        trait Touch {
            machine touch(&self);
        }

        data Item { value: i32; }

        Primary: Item satisfies Touch {
            machine touch(&self) {}
        }

        data Main { selected: Item; }

        machine Main::run(&mut self) {
            let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            erased.touch();
        }
    "#;

    /// One forwarded Unit call on a rebound descriptor whose two selections
    /// name different checked conformances, so the descriptor's argument
    /// consumes two distinct closed applications.
    const FORWARDED_CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE: &str = r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }

        Primary: Item satisfies Touch { machine touch(&self) {} }
        Secondary: Item satisfies Touch { machine touch(&self) {} }

        data Main { decoy: Item; selected: Item; }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Secondary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) { erased.touch(); }
    "#;

    /// One forwarded scalar call through a two-helper chain: `forward`
    /// forwards the parameter to `finish`, which dispatches.
    const MULTI_HOP_DYNAMIC_INTEGER_SOURCE: &str = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item [copy] { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main { selected: Item; }

        machine Main::run(&self) {
            let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = finish(erased);
            transition { _ -> result }
        }

        machine finish(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;

    /// A checked two-predecessor join feeding a three-helper scalar chain:
    /// each branch selects its own descriptor into `forward`, which forwards
    /// through `relay` to the dispatching `finish`.
    const JOINED_DYNAMIC_BOOLEAN_FORWARD_SOURCE: &str = r#"
        trait Measure {
            machine measure(&self) -> bool;
        }

        data Item [copy] { marker: bool; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> bool { transition { _ -> self.marker } }
        }

        Secondary: Item satisfies Measure {
            machine measure(&self) -> bool { transition { _ -> self.marker } }
        }

        data Main [copy] { first: Item; second: Item; }

        machine Main::run(&self, choose_first: bool) {
            transition choose_first {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Measure = &self.first as &dyn Item::Primary;
                let result: bool = forward(selected);
            }

            state take_second(&self) {
                let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
                let result: bool = forward(selected);
            }
        }

        machine forward(erased: &dyn Measure) -> bool {
            let result: bool = relay(erased);
            transition { _ -> result }
        }

        machine relay(erased: &dyn Measure) -> bool {
            let result: bool = finish(erased);
            transition { _ -> result }
        }

        machine finish(erased: &dyn Measure) -> bool {
            let result: bool = erased.measure();
            transition { _ -> result }
        }
    "#;

    /// The same joined shape on the Unit-result lane.
    const JOINED_DYNAMIC_UNIT_FORWARD_SOURCE: &str = r#"
        trait Touch { machine touch(&self); }

        data Item { value: i32; }

        Primary: Item satisfies Touch { machine touch(&self) {} }
        Secondary: Item satisfies Touch { machine touch(&self) {} }

        data Main { first: Item; second: Item; }

        machine Main::run(&self, choose_first: bool) {
            transition choose_first {
                true -> take_first()
                _ -> take_second()
            }

            state take_first(&self) {
                let selected: &dyn Touch = &self.first as &dyn Item::Primary;
                forward(selected);
            }

            state take_second(&self) {
                let selected: &dyn Touch = &self.second as &dyn Item::Secondary;
                forward(selected);
            }
        }

        machine forward(erased: &dyn Touch) { relay(erased); }
        machine relay(erased: &dyn Touch) { finish(erased); }
        machine finish(erased: &dyn Touch) { erased.touch(); }
    "#;

    fn forwarded_dynamic_produced(source: &str) -> (CheckedTrees, ProducedTerminalArtifact<()>) {
        let checked = check_source(source);
        let produced =
            TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::run"))
                .produce(TerminalProductionCustody {
                    retain_unoptimized: true,
                    entry_identity: None,
                    callback_custody: (),
                    timings: &mut TerminalProductionTimings::default(),
                })
                .expect("forwarded dynamic production publishes");
        (checked, produced)
    }

    /// The checked call plan inside the roster for forwarded-plan drift
    /// mutations, whichever binding wraps it.
    fn forwarded_scalar_mut(checked: &mut CheckedTrees) -> &mut CheckedDynamicScalarCallPlan {
        checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .calls
            .iter_mut()
            .find_map(|plan| match plan {
                CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Direct(call))
                    if matches!(
                        call.origin,
                        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { .. }
                    ) =>
                {
                    Some(call)
                }
                CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Rebound {
                    latest,
                    ..
                }) if matches!(
                    latest.origin,
                    checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { .. }
                ) =>
                {
                    Some(latest)
                }
                _ => None,
            })
            .expect("one forwarded scalar dynamic plan")
    }

    /// One forwarded chain's emitted shape: the sole parameter dispatch, the
    /// shared descriptor-parameter interface per helper, and one argument
    /// row per root plus one per transfer hop.
    fn forwarded_catalog_counts(
        module: &terminal_psi::TerminalModule,
        roots: usize,
        helpers: usize,
        selections: usize,
    ) {
        let catalog = &module.dynamic_dispatch;
        assert_eq!(catalog.parameter_dispatches.len(), 1);
        assert_eq!(catalog.parameters.len(), helpers);
        assert_eq!(catalog.arguments.len(), roots + helpers - 1);
        assert_eq!(catalog.selections.len(), selections);
        assert!(catalog.direct_dispatches.is_empty());
        assert!(catalog.indirect_dispatches.is_empty());
        assert!(catalog.stored_dispatches.is_empty());
        assert!(catalog.stored_descriptors.is_empty());
    }

    /// The one replayed forwarded occurrence binds the final helper's
    /// parameter dispatch.
    fn assert_forwarded_occurrence(
        checked: &CheckedTrees,
        produced: &ProducedTerminalArtifact<()>,
        module: &terminal_psi::TerminalModule,
        lane: CheckedDynamicCallLane,
    ) {
        let catalog = &module.dynamic_dispatch;
        let dispatch = catalog
            .parameter_dispatches
            .first()
            .expect("one parameter dispatch row");
        let operation = module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .find(|operation| operation.id == dispatch.operation)
            .expect("the dispatch names an emitted operation");
        let expected_kind = match lane {
            CheckedDynamicCallLane::ForwardedScalar => {
                matches!(
                    operation.kind,
                    OperationKind::CallDynamicParameterScalar { .. }
                )
            }
            CheckedDynamicCallLane::ForwardedUnit => {
                matches!(
                    operation.kind,
                    OperationKind::CallDynamicParameterUnit { .. }
                )
            }
            _ => panic!("forwarded lane expected"),
        };
        assert!(expected_kind, "the final helper owns a parameter dispatch");
        let scope = checked_boundary_operator_scope(
            checked,
            produced.artifact(),
            produced
                .unoptimized()
                .expect("custody retained the lowered module"),
        )
        .expect("the recorded roster rejoins its checked forwarded plan");
        let [occurrence] = scope.dynamic_call_occurrences() else {
            panic!("one replayed dynamic call occurrence");
        };
        assert_eq!(occurrence.lane(), lane);
        assert_eq!(occurrence.plan_index(), 0);
        assert_eq!(occurrence.terminal_machine(), dispatch.owner);
        assert_eq!(occurrence.terminal_operation(), dispatch.operation);
        assert_eq!(occurrence.descriptor_ordinal(), dispatch.parameter_ordinal);
        // The same roster replays directly against the published artifact.
        let replayed = checked_boundary_operator_scope(
            checked,
            produced.artifact(),
            produced
                .unoptimized()
                .expect("custody retained the lowered module"),
        )
        .expect("the recorded roster rejoins its checked forwarded plan");
        assert_eq!(
            replayed.dynamic_call_occurrences(),
            scope.dynamic_call_occurrences()
        );
    }

    #[test]
    fn a_forwarded_dynamic_scalar_call_rejoins_its_parameter_dispatch() {
        let (checked, produced) = forwarded_dynamic_produced(FORWARDED_DYNAMIC_INTEGER_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        forwarded_catalog_counts(&module, 1, 1, 1);
        let catalog = &module.dynamic_dispatch;
        assert!(catalog.rebound_descriptors.is_empty());
        assert_eq!(
            catalog.arguments[0].source,
            TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
        );
        // Two source-call occurrences: the root call into the helper and the
        // helper's own dispatch.
        assert_eq!(produced.source_call_occurrences().len(), 2);
        assert_forwarded_occurrence(
            &checked,
            &produced,
            &module,
            CheckedDynamicCallLane::ForwardedScalar,
        );
    }

    #[test]
    fn a_forwarded_rebound_descriptor_argument_rejoins_its_parameter_dispatch() {
        let (checked, produced) =
            forwarded_dynamic_produced(FORWARDED_REBOUND_DYNAMIC_INTEGER_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        forwarded_catalog_counts(&module, 1, 1, 2);
        let catalog = &module.dynamic_dispatch;
        let [descriptor] = catalog.rebound_descriptors.as_slice() else {
            panic!("one rebound descriptor feeds the forwarded argument")
        };
        assert_eq!(
            catalog.arguments[0].source,
            TerminalDynamicDescriptorSource::ReboundDescriptor {
                ordinal: descriptor.ordinal
            }
        );
        assert_forwarded_occurrence(
            &checked,
            &produced,
            &module,
            CheckedDynamicCallLane::ForwardedScalar,
        );
    }

    #[test]
    fn a_forwarded_dynamic_unit_call_rejoins_its_parameter_dispatch() {
        let (checked, produced) = forwarded_dynamic_produced(FORWARDED_DYNAMIC_UNIT_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        forwarded_catalog_counts(&module, 1, 1, 1);
        assert!(module.dynamic_dispatch.rebound_descriptors.is_empty());
        assert_forwarded_occurrence(
            &checked,
            &produced,
            &module,
            CheckedDynamicCallLane::ForwardedUnit,
        );
    }

    #[test]
    fn a_changed_conformance_forwarded_rebound_rejoins_two_applications() {
        let (checked, produced) =
            forwarded_dynamic_produced(FORWARDED_CHANGED_CONFORMANCE_DYNAMIC_UNIT_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        forwarded_catalog_counts(&module, 1, 1, 2);
        let catalog = &module.dynamic_dispatch;
        assert_eq!(catalog.rebound_descriptors.len(), 1);
        assert_ne!(
            catalog.selections[0].conformance_application_commitment,
            catalog.selections[1].conformance_application_commitment,
            "the changed conformance keeps two distinct closed applications",
        );
        assert_forwarded_occurrence(
            &checked,
            &produced,
            &module,
            CheckedDynamicCallLane::ForwardedUnit,
        );
    }

    #[test]
    fn a_multi_hop_forwarded_chain_rejoins_every_helper_hop() {
        let (checked, produced) = forwarded_dynamic_produced(MULTI_HOP_DYNAMIC_INTEGER_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        forwarded_catalog_counts(&module, 1, 2, 1);
        let catalog = &module.dynamic_dispatch;
        // The transfer hop's argument draws the descriptor back out of the
        // shared parameter interface.
        let transfer_argument = catalog
            .arguments
            .iter()
            .find(|argument| {
                argument.source == (TerminalDynamicDescriptorSource::Parameter { ordinal: 0 })
            })
            .expect("one parameter-sourced transfer argument");
        let first_helper = catalog
            .arguments
            .iter()
            .find(|argument| {
                matches!(
                    argument.source,
                    TerminalDynamicDescriptorSource::Selection { .. }
                )
            })
            .and_then(|root| {
                module
                    .machines
                    .iter()
                    .flat_map(|machine| machine.blocks.iter())
                    .flat_map(|block| block.operations.iter())
                    .find_map(|operation| match operation.kind {
                        OperationKind::CallStructuralScalar { callee, .. }
                            if operation.id == root.operation =>
                        {
                            Some(callee)
                        }
                        _ => None,
                    })
            })
            .expect("the root call names the first helper");
        assert_eq!(transfer_argument.owner, first_helper);
        // Three source-call occurrences: root, transfer hop, final dispatch.
        assert_eq!(produced.source_call_occurrences().len(), 3);
        assert_forwarded_occurrence(
            &checked,
            &produced,
            &module,
            CheckedDynamicCallLane::ForwardedScalar,
        );
    }

    #[test]
    fn a_joined_forwarded_call_rejoins_both_branch_roots() {
        let (checked, produced) = forwarded_dynamic_produced(JOINED_DYNAMIC_BOOLEAN_FORWARD_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        forwarded_catalog_counts(&module, 2, 3, 2);
        let catalog = &module.dynamic_dispatch;
        // Each branch root supplies its own selection in caller order.
        let mut selection_sources = catalog
            .arguments
            .iter()
            .filter_map(|argument| match argument.source {
                TerminalDynamicDescriptorSource::Selection { ordinal } => Some(ordinal),
                _ => None,
            })
            .collect::<Vec<_>>();
        selection_sources.sort_unstable();
        assert_eq!(selection_sources, [0, 1]);
        // Five source-call occurrences: two roots, two transfer hops and the
        // final dispatch.
        assert_eq!(produced.source_call_occurrences().len(), 5);
        assert_forwarded_occurrence(
            &checked,
            &produced,
            &module,
            CheckedDynamicCallLane::ForwardedScalar,
        );
    }

    #[test]
    fn a_joined_forwarded_unit_call_rejoins_both_branch_roots() {
        let (checked, produced) = forwarded_dynamic_produced(JOINED_DYNAMIC_UNIT_FORWARD_SOURCE);
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
            .expect("published Terminal module decodes");
        forwarded_catalog_counts(&module, 2, 3, 2);
        assert_eq!(produced.source_call_occurrences().len(), 5);
        assert_forwarded_occurrence(
            &checked,
            &produced,
            &module,
            CheckedDynamicCallLane::ForwardedUnit,
        );
    }

    #[test]
    fn a_stale_or_duplicated_forwarded_occurrence_rejects() {
        use semantic_vocabulary::OperationId;
        let (checked, produced) = forwarded_dynamic_produced(MULTI_HOP_DYNAMIC_INTEGER_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay = |corrupted: &lowered_psi::LoweredPsi| {
            checked_boundary_operator_scope(&checked, produced.artifact(), corrupted).unwrap_err()
        };
        // A root row naming an operation the module no longer carries.
        let mut corrupted = lowered.clone();
        corrupted.source_call_occurrences[0].terminal_operation =
            OperationId::new(u64::MAX).expect("unused operation identity");
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call does not name one exact Terminal operation"
        );
        // A final-dispatch row naming an operation the module no longer
        // carries.
        let mut corrupted = lowered.clone();
        let last = corrupted.source_call_occurrences.len() - 1;
        corrupted.source_call_occurrences[last].terminal_operation =
            OperationId::new(u64::MAX).expect("unused operation identity");
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call does not name one exact Terminal operation"
        );
        // A duplicated occurrence row.
        let mut corrupted = lowered.clone();
        let duplicate = corrupted.source_call_occurrences[0].clone();
        corrupted.source_call_occurrences.push(duplicate);
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call maps to duplicate Terminal occurrences"
        );
        // A stale root coordinate leaves the surviving parameter dispatch
        // unchecked.
        let mut corrupted = lowered.clone();
        corrupted.source_call_occurrences[0].call_ordinal += 1;
        assert_eq!(
            replay(&corrupted),
            "Terminal parameter dispatch does not rejoin one checked forwarded call"
        );
        // A stale final-dispatch coordinate loses the helper's dispatch
        // occurrence before the roster completes.
        let mut corrupted = lowered.clone();
        let last = corrupted.source_call_occurrences.len() - 1;
        corrupted.source_call_occurrences[last].call_ordinal += 1;
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call lost its final helper occurrence"
        );
    }

    #[test]
    fn a_substituted_forwarded_selection_identity_rejects() {
        let (checked, produced) = forwarded_dynamic_produced(FORWARDED_DYNAMIC_INTEGER_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay = |corrupted: &CheckedTrees| {
            checked_boundary_operator_scope(corrupted, produced.artifact(), &lowered).unwrap_err()
        };
        // A checked plan whose requirement identity no longer selects the
        // emitted conformance row.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted).requirement_identity = "foreign".to_owned();
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic dispatch does not rejoin its selected conformance row"
        );
        // A checked plan whose family tuple is substituted.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted).family_tuple = Box::from(["foreign".to_owned()]);
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic dispatch does not rejoin its selected conformance row"
        );
        // A stale checked coordinate drops the root's own transfer edge out
        // of the checked roster before the occurrence lookup runs.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted)
            .coordinate
            .statement_index += 1;
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call lost its checked root transfer"
        );
        // A retained conformance row whose requirement identity drifted no
        // longer matches the root transfer's recorded path selection.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted).selection.rows[0].requirement_identity =
            "foreign".to_owned();
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call lost its checked root transfer"
        );
        // A drifted structural source no longer matches the emitted
        // selection's recorded argument.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted).source_path.push(
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0),
        );
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic descriptor source drifted from its checked selection"
        );
    }

    #[test]
    fn a_drifted_forwarded_helper_chain_rejects() {
        let (checked, produced) = forwarded_dynamic_produced(MULTI_HOP_DYNAMIC_INTEGER_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay = |corrupted: &CheckedTrees| {
            checked_boundary_operator_scope(corrupted, produced.artifact(), &lowered).unwrap_err()
        };
        // A retained transfer dropped from the checked chain.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted)
            .forwarding_transfers
            .pop();
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic helper chain drifted from checked custody"
        );
        // A retained helper body dropped from the checked chain.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted)
            .forwarding_helpers
            .pop();
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic helper chain drifted from checked custody"
        );
        // A transfer whose coordinate drifted out of the checked roster.
        let mut corrupted = checked.clone();
        forwarded_scalar_mut(&mut corrupted).forwarding_transfers[0]
            .coordinate
            .statement_index += 1;
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic transfer drifted from its checked custody"
        );
    }

    /// The artifact wrapper seals `semantic_module` by identity, so catalog
    /// and operation mutations exercise the replay body directly.
    #[test]
    fn a_missing_or_duplicated_forwarded_catalog_row_rejects() {
        use semantic_vocabulary::{MachineId, OperationId};
        let (checked, produced) = forwarded_dynamic_produced(MULTI_HOP_DYNAMIC_INTEGER_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay =
            |corrupted: &lowered_psi::LoweredPsi| super::replay(&checked, corrupted).unwrap_err();
        // A dropped root argument row.
        let mut corrupted = lowered.clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .arguments
            .retain(|argument| {
                matches!(
                    argument.source,
                    TerminalDynamicDescriptorSource::Parameter { .. }
                )
            });
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic argument is absent or ambiguous"
        );
        // A duplicated helper parameter row.
        let mut corrupted = lowered.clone();
        let duplicate = corrupted.semantic_module.dynamic_dispatch.parameters[0].clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .parameters
            .push(duplicate);
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic parameter interface is absent or ambiguous"
        );
        // A dropped parameter-dispatch row.
        let mut corrupted = lowered.clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .parameter_dispatches
            .clear();
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call does not rejoin one exact Terminal parameter dispatch"
        );
        // An orphan parameter row no checked chain claims.
        let mut corrupted = lowered.clone();
        let mut orphan = corrupted.semantic_module.dynamic_dispatch.parameters[0].clone();
        orphan.owner = MachineId::new(u64::MAX).expect("unused machine identity");
        corrupted
            .semantic_module
            .dynamic_dispatch
            .parameters
            .push(orphan);
        assert_eq!(
            replay(&corrupted),
            "Terminal dynamic parameter does not rejoin one checked forwarded call"
        );
        // An orphan argument row no forwarded hop claims.
        let mut corrupted = lowered.clone();
        let mut orphan = corrupted.semantic_module.dynamic_dispatch.arguments[0].clone();
        orphan.operation = OperationId::new(u64::MAX).expect("unused operation identity");
        corrupted
            .semantic_module
            .dynamic_dispatch
            .arguments
            .push(orphan);
        assert_eq!(
            replay(&corrupted),
            "Terminal dynamic argument does not rejoin one checked forwarded call"
        );
        // An orphan parameter-dispatch row no forwarded plan claims.
        let mut corrupted = lowered.clone();
        let mut orphan = corrupted
            .semantic_module
            .dynamic_dispatch
            .parameter_dispatches[0]
            .clone();
        orphan.operation = OperationId::new(u64::MAX).expect("unused operation identity");
        corrupted
            .semantic_module
            .dynamic_dispatch
            .parameter_dispatches
            .push(orphan);
        assert_eq!(
            replay(&corrupted),
            "Terminal parameter dispatch does not rejoin one checked forwarded call"
        );
        // A conformance-application row whose identity drifted from the
        // checked selection's retained roster.
        let mut corrupted = lowered.clone();
        corrupted.semantic_module.closed_conformance_applications[0].rows[0].requirement_identity =
            "foreign".to_owned();
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic conformance application drifted from its checked selection"
        );
    }

    #[test]
    fn a_substituted_forwarded_helper_operation_rejects() {
        use semantic_vocabulary::{MachineId, OperationId};
        let (checked, produced) = forwarded_dynamic_produced(MULTI_HOP_DYNAMIC_INTEGER_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        let replay =
            |corrupted: &lowered_psi::LoweredPsi| super::replay(&checked, corrupted).unwrap_err();
        // A helper whose forwarding call targets a different next machine
        // breaks the chain order the checked transfers keep.
        let mut corrupted = lowered.clone();
        let transfer_operation = corrupted
            .semantic_module
            .dynamic_dispatch
            .arguments
            .iter()
            .find(|argument| {
                argument.source == (TerminalDynamicDescriptorSource::Parameter { ordinal: 0 })
            })
            .expect("one transfer argument")
            .operation;
        let substitute = corrupted
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| machine.blocks.iter_mut())
            .flat_map(|block| block.operations.iter_mut())
            .find_map(|operation| match &mut operation.kind {
                kind @ OperationKind::CallStructuralScalar { .. }
                    if operation.id == transfer_operation =>
                {
                    Some(kind)
                }
                _ => None,
            })
            .expect("the transfer hop is an ordinary scalar call");
        let OperationKind::CallStructuralScalar { callee, .. } = substitute else {
            unreachable!("matched above")
        };
        *callee = MachineId::new(u64::MAX).expect("unused machine identity");
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic helper chain ordered its machines wrong"
        );
        // A final scalar dispatch op substituted by the wrong dynamic kind.
        let mut corrupted = lowered.clone();
        let dispatch_operation = corrupted
            .semantic_module
            .dynamic_dispatch
            .parameter_dispatches[0]
            .operation;
        let operation = corrupted
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| machine.blocks.iter_mut())
            .flat_map(|block| block.operations.iter_mut())
            .find(|operation| operation.id == dispatch_operation)
            .expect("the parameter dispatch names an emitted operation");
        let OperationKind::CallDynamicParameterScalar {
            requirement_obligations,
            crash_continuations,
            ..
        } = &operation.kind
        else {
            unreachable!("checked above")
        };
        operation.kind = OperationKind::CallDynamicParameterUnit {
            parameter_ordinal: 0,
            requirement_slot: 0,
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        };
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic call does not name one exact Terminal operation"
        );
        // A parameter dispatch row naming a different requirement slot.
        let mut corrupted = lowered.clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .parameter_dispatches[0]
            .requirement_slot += 1;
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic dispatch drifted from its checked interface"
        );
        // A parameter dispatch row naming a different parameter ordinal.
        let mut corrupted = lowered.clone();
        corrupted
            .semantic_module
            .dynamic_dispatch
            .parameter_dispatches[0]
            .parameter_ordinal = 1;
        assert_eq!(
            replay(&corrupted),
            "forwarded dynamic dispatch drifted from its checked interface"
        );
        // An emitted dynamic operation no checked plan rejoins: a fresh
        // identity keeps the row join exact but the extra operation is stray
        // emitted custody.
        let mut corrupted = lowered.clone();
        let mut operation = corrupted
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .find(|operation| operation.id == dispatch_operation)
            .expect("the parameter dispatch names an emitted operation")
            .clone();
        operation.id = OperationId::new(u64::MAX).expect("unused operation identity");
        corrupted.semantic_module.machines[0].blocks[0]
            .operations
            .push(operation);
        assert_eq!(
            replay(&corrupted),
            "Terminal dynamic call operation does not rejoin one checked dynamic call"
        );
    }

    #[test]
    fn a_foreign_forwarded_dynamic_scope_rejects() {
        let (_checked, produced) = forwarded_dynamic_produced(FORWARDED_DYNAMIC_INTEGER_SOURCE);
        let lowered = produced.unoptimized().unwrap().clone();
        // A program whose forwarded plan names a different requirement
        // symbol: replaying the published roster under foreign checked
        // custody either drifts on the row identities or leaves the dispatch
        // unrejoined.
        let foreign = check_source(&FORWARDED_DYNAMIC_INTEGER_SOURCE.replace("measure", "gauge"));
        let error =
            checked_boundary_operator_scope(&foreign, produced.artifact(), &lowered).unwrap_err();
        assert!(
            [
                "forwarded dynamic dispatch does not rejoin its selected conformance row",
                "forwarded dynamic conformance application drifted from its checked selection",
                "forwarded dynamic dispatch drifted from its checked requirement",
                "Terminal parameter dispatch does not rejoin one checked forwarded call",
            ]
            .contains(&error),
            "unexpected replay failure: {error}"
        );
    }
}

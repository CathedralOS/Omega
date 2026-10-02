//! Rejoin checked rebound and stored dynamic-call plans to their exact
//! Terminal descriptor-table occurrences.
//!
//! The admitted families are the local descriptor-table materializations. One
//! checked rebound plan emits one `CallDynamicScalar`/`CallDynamicUnit`
//! operation, one indirect-dispatch row, one rebound descriptor, and two
//! conformance selections naming closed applications. One checked stored
//! scalar plan emits one `CallDynamicScalar` operation, one stored-dispatch
//! row, one stored descriptor established by an exact `StoreDynamicDescriptor`
//! operation, and one conformance selection the reload consumes; a stored
//! Unit call is refused at lowering, so the stored family is scalar-only.
//! Forwarded plans dispatch through a generated helper's existential
//! parameter; that register-indirect family stays unadmitted here and is
//! skipped rather than silently rejoined. Direct and joined plans emit
//! different catalog species and never enter this roster.

use super::{CheckedDynamicCallLane, CheckedDynamicCallOccurrence, unsupported};
use checked_trees::{
    CheckedDynamicBinding, CheckedDynamicDispatchPlan, CheckedDynamicScalarCallOrigin,
    CheckedDynamicSelectionPlan, CheckedDynamicStoredDescriptorPlan, CheckedDynamicUnitCallOrigin,
    CheckedStructuralAccess, CheckedTrees, CheckedUnitCallCoordinate,
    CheckedUnitStructuralPathSegment, DynamicConformanceRowFact,
};
use lowered_psi::{LoweredPsi, LoweredSourceCallOccurrence};
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::BTreeSet;
use symbols::SymbolHandle;
use terminal_psi::{
    ClosedConformanceApplication, OperationKind, StructuralAccess, StructuralArgument,
    StructuralPathSegment, TerminalDynamicConformanceSelection, TerminalIndirectDynamicDispatch,
    TerminalMachine, TerminalModule, TerminalStoredDynamicDispatch,
};

/// Borrowed view over the dispatch coordinates shared by the scalar and Unit
/// plan types.
struct DispatchView<'a> {
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
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

/// Which local descriptor-table species a replayed dispatch belongs to. Error
/// messages keep naming the checked family a drifted row belongs to, so both
/// families share one join without conflating their custody.
#[derive(Clone, Copy)]
enum DispatchFamily {
    Rebound,
    Stored,
}

impl DispatchFamily {
    const fn of(lane: CheckedDynamicCallLane) -> Self {
        match lane {
            CheckedDynamicCallLane::ReboundScalar | CheckedDynamicCallLane::ReboundUnit => {
                Self::Rebound
            }
            CheckedDynamicCallLane::StoredScalar => Self::Stored,
        }
    }

    const fn message(self, rebound: &'static str, stored: &'static str) -> &'static str {
        match self {
            Self::Rebound => rebound,
            Self::Stored => stored,
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
    let mut occurrences = Vec::new();
    for (plan_index, plan) in plans.calls.iter().enumerate() {
        let occurrence = match plan {
            CheckedDynamicDispatchPlan::Scalar(CheckedDynamicBinding::Rebound {
                initial,
                latest,
            }) => {
                if !matches!(latest.origin, CheckedDynamicScalarCallOrigin::Local) {
                    continue;
                }
                replay_rebound(
                    checked,
                    module,
                    &lowered.source_call_occurrences,
                    DispatchView {
                        caller_state: latest.caller_state,
                        coordinate: latest.coordinate,
                        requirement: latest.requirement,
                        requirement_identity: &latest.requirement_identity,
                        declaring_trait: latest.declaring_trait,
                        realization_state: latest.realization_state,
                        target_trait: latest.target_trait,
                        selected_conformance: latest.selected_conformance,
                        family_tuple: &latest.family_tuple,
                        selection: &latest.selection,
                        source_path: &latest.source_path,
                        source_type_identity: &latest.source_type_identity,
                        source_parameter_position: latest.source_parameter_position,
                        source_access: latest.source_access,
                    },
                    initial,
                    plan_index,
                    CheckedDynamicCallLane::ReboundScalar,
                )?
            }
            CheckedDynamicDispatchPlan::Unit(CheckedDynamicBinding::Rebound {
                initial,
                latest,
            }) => {
                if !matches!(latest.origin, CheckedDynamicUnitCallOrigin::Local) {
                    continue;
                }
                replay_rebound(
                    checked,
                    module,
                    &lowered.source_call_occurrences,
                    DispatchView {
                        caller_state: latest.caller_state,
                        coordinate: latest.coordinate,
                        requirement: latest.requirement,
                        requirement_identity: &latest.requirement_identity,
                        declaring_trait: latest.declaring_trait,
                        realization_state: latest.realization_state,
                        target_trait: latest.target_trait,
                        selected_conformance: latest.selected_conformance,
                        family_tuple: &latest.family_tuple,
                        selection: &latest.selection,
                        source_path: &latest.source_path,
                        source_type_identity: &latest.source_type_identity,
                        source_parameter_position: latest.source_parameter_position,
                        source_access: latest.source_access,
                    },
                    initial,
                    plan_index,
                    CheckedDynamicCallLane::ReboundUnit,
                )?
            }
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
                    &lowered.source_call_occurrences,
                    DispatchView {
                        caller_state: call.caller_state,
                        coordinate: call.coordinate,
                        requirement: call.requirement,
                        requirement_identity: &call.requirement_identity,
                        declaring_trait: call.declaring_trait,
                        realization_state: call.realization_state,
                        target_trait: call.target_trait,
                        selected_conformance: call.selected_conformance,
                        family_tuple: &call.family_tuple,
                        selection: &call.selection,
                        source_path: &call.source_path,
                        source_type_identity: &call.source_type_identity,
                        source_parameter_position: call.source_parameter_position,
                        source_access: call.source_access,
                    },
                    descriptor,
                    plan_index,
                )?
            }
            // A stored Unit call is refused at lowering, and direct, joined
            // and forwarded plans emit different catalog species: none enter
            // this roster.
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
        )
    })?;
    let call_ordinal = usize::try_from(latest.coordinate.call_ordinal).map_err(|_| {
        family.message(
            "rebound dynamic call ordinal exceeds usize",
            "stored dynamic call ordinal exceeds usize",
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
    let descriptors = module
        .dynamic_dispatch
        .rebound_descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.owner == owner.id && descriptor.ordinal == dispatch.descriptor_ordinal
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
    // The latest selection supplies the dispatch's table identity; its
    // application must carry the checked conformance's exact row roster and
    // the callable the dispatch names.
    let latest_selection = conformance_selection(
        module,
        owner.id,
        descriptor.rebound_selection_ordinal,
        DispatchFamily::Rebound,
    )?;
    verify_source(
        owner,
        &latest_selection.source,
        latest.source_path,
        latest.source_access,
        latest.source_parameter_position,
        DispatchFamily::Rebound,
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
        DispatchFamily::Rebound,
    )?;
    verify_selected_row(
        checked,
        DispatchFamily::Rebound,
        &latest,
        application,
        &dispatch_row,
    )?;
    // The initial selection retains the descriptor's first source version. It
    // names the same application only when the checked conformance and row
    // roster are unchanged; a distinct application must rejoin the initial
    // selection's own checked custody.
    let initial_selection = conformance_selection(
        module,
        owner.id,
        descriptor.initial_selection_ordinal,
        DispatchFamily::Rebound,
    )?;
    verify_source(
        owner,
        &initial_selection.source,
        &initial.path,
        latest.source_access,
        latest.source_parameter_position,
        DispatchFamily::Rebound,
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
            DispatchFamily::Rebound,
        )?;
    }
    Ok(Some(CheckedDynamicCallOccurrence::new(
        lane,
        plan_index,
        owner.id,
        terminal_operation,
        dispatch.descriptor_ordinal,
    )))
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
        ));
    };
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
        ));
    };
    let drifted = family.message(
        "rebound dynamic conformance application drifted from its checked selection",
        "stored dynamic conformance application drifted from its checked selection",
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
    //! End-to-end replay checks for the stored descriptor-table lane: the
    //! checked plan, its `StoreDynamicDescriptor` establishment, its stored
    //! descriptor and dispatch rows and its conformance selection must all
    //! rejoin one exact occurrence, and drifted or stale custody rejects.
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

    fn check_source(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
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
}

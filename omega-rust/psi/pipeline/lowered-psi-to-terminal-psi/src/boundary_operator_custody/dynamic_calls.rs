//! Rejoin checked rebound dynamic-call plans to their exact Terminal
//! descriptor-table occurrences.
//!
//! The admitted family is local descriptor-table materialization: one checked
//! rebound plan emits one `CallDynamicScalar`/`CallDynamicUnit` operation, one
//! indirect-dispatch row, one rebound descriptor, and two conformance
//! selections naming closed applications. Forwarded rebound plans dispatch
//! through a generated helper's existential parameter; that register-indirect
//! family stays unadmitted here and is skipped rather than silently rejoined.
//! Stored and direct plans emit different catalog species and never enter this
//! roster.

use super::{CheckedDynamicCallLane, CheckedDynamicCallOccurrence, unsupported};
use checked_trees::{
    CheckedDynamicScalarCallOrigin, CheckedDynamicSelectionPlan, CheckedDynamicUnitCallOrigin,
    CheckedStructuralAccess, CheckedTrees, CheckedUnitCallCoordinate,
    CheckedUnitStructuralPathSegment, DynamicConformanceRowFact,
};
use lowered_psi::{LoweredPsi, LoweredSourceCallOccurrence};
use semantic_vocabulary::MachineId;
use std::collections::BTreeSet;
use symbols::SymbolHandle;
use terminal_psi::{
    ClosedConformanceApplication, OperationKind, StructuralAccess, StructuralArgument,
    StructuralPathSegment, TerminalDynamicConformanceSelection, TerminalMachine, TerminalModule,
};

/// Borrowed view over the dispatch coordinates shared by the scalar and Unit
/// rebound plan types.
struct ReboundDispatchView<'a> {
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

pub(super) fn replay(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
) -> Result<Vec<CheckedDynamicCallOccurrence>, &'static str> {
    let plans = &checked.facts.flow.terminal_unit_effects.dynamic_dispatch;
    let module = &lowered.semantic_module;
    let mut occurrences = Vec::new();
    for (plan_index, rebound) in plans.rebound_scalar_calls.iter().enumerate() {
        let plan = &rebound.latest;
        if !matches!(plan.origin, CheckedDynamicScalarCallOrigin::Local) {
            continue;
        }
        if let Some(occurrence) = replay_rebound(
            checked,
            module,
            &lowered.source_call_occurrences,
            ReboundDispatchView {
                caller_state: plan.caller_state,
                coordinate: plan.coordinate,
                requirement: plan.requirement,
                requirement_identity: &plan.requirement_identity,
                declaring_trait: plan.declaring_trait,
                realization_state: plan.realization_state,
                target_trait: plan.target_trait,
                selected_conformance: plan.selected_conformance,
                family_tuple: &plan.family_tuple,
                selection: &plan.selection,
                source_path: &plan.source_path,
                source_type_identity: &plan.source_type_identity,
                source_parameter_position: plan.source_parameter_position,
                source_access: plan.source_access,
            },
            &rebound.initial,
            plan_index,
            CheckedDynamicCallLane::ReboundScalar,
        )? {
            occurrences.push(occurrence);
        }
    }
    for (plan_index, rebound) in plans.rebound_unit_calls.iter().enumerate() {
        let plan = &rebound.latest;
        if !matches!(plan.origin, CheckedDynamicUnitCallOrigin::Local) {
            continue;
        }
        if let Some(occurrence) = replay_rebound(
            checked,
            module,
            &lowered.source_call_occurrences,
            ReboundDispatchView {
                caller_state: plan.caller_state,
                coordinate: plan.coordinate,
                requirement: plan.requirement,
                requirement_identity: &plan.requirement_identity,
                declaring_trait: plan.declaring_trait,
                realization_state: plan.realization_state,
                target_trait: plan.target_trait,
                selected_conformance: plan.selected_conformance,
                family_tuple: &plan.family_tuple,
                selection: &plan.selection,
                source_path: &plan.source_path,
                source_type_identity: &plan.source_type_identity,
                source_parameter_position: plan.source_parameter_position,
                source_access: plan.source_access,
            },
            &rebound.initial,
            plan_index,
            CheckedDynamicCallLane::ReboundUnit,
        )? {
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
                "rebound dynamic calls do not map one-to-one onto Terminal operations",
            );
        }
    }
    // The admitted lane is the only producer of indirect dispatches: any row
    // left unrejoined means a source occurrence was skipped or went stale, so
    // the roster would be incomplete rather than merely smaller.
    if module
        .dynamic_dispatch
        .indirect_dispatches
        .iter()
        .any(|dispatch| !joined.contains(&(dispatch.owner, dispatch.operation)))
    {
        return unsupported("Terminal indirect dispatch does not rejoin one checked rebound call");
    }
    Ok(occurrences)
}

fn replay_rebound(
    checked: &CheckedTrees,
    module: &TerminalModule,
    source_calls: &[LoweredSourceCallOccurrence],
    latest: ReboundDispatchView<'_>,
    initial: &CheckedDynamicSelectionPlan,
    plan_index: usize,
    lane: CheckedDynamicCallLane,
) -> Result<Option<CheckedDynamicCallOccurrence>, &'static str> {
    let statement_index = usize::try_from(latest.coordinate.statement_index)
        .map_err(|_| "rebound dynamic call statement coordinate exceeds usize")?;
    let call_ordinal = usize::try_from(latest.coordinate.call_ordinal)
        .map_err(|_| "rebound dynamic call ordinal exceeds usize")?;
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
        return unsupported("rebound dynamic call maps to duplicate Terminal occurrences");
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
        return unsupported("rebound dynamic call does not name one exact Terminal operation");
    };
    let descriptor_ordinal = match (&operation.kind, lane) {
        (
            OperationKind::CallDynamicScalar {
                descriptor_ordinal, ..
            },
            CheckedDynamicCallLane::ReboundScalar,
        )
        | (
            OperationKind::CallDynamicUnit {
                descriptor_ordinal, ..
            },
            CheckedDynamicCallLane::ReboundUnit,
        ) => *descriptor_ordinal,
        _ => {
            return unsupported("rebound dynamic call does not name one exact Terminal operation");
        }
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
    if dispatch.descriptor_ordinal != descriptor_ordinal
        || dispatch.declaring_trait_identity
            != checked.symbols.display_path(latest.declaring_trait, "::")
        || dispatch.public_requirement_identity != latest.requirement_identity
        || dispatch.family_tuple[..] != *latest.family_tuple
        || dispatch.requirement_identity != checked.symbols.display_path(latest.requirement, "::")
        || dispatch.realization_identity
            != checked.symbols.display_path(latest.realization_state, "::")
    {
        return unsupported("rebound dynamic dispatch drifted from its checked selection");
    }
    if !module
        .machines
        .iter()
        .any(|machine| machine.id == dispatch.realization)
    {
        return unsupported("rebound dynamic dispatch names an absent Terminal realization");
    }
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
    let latest_selection =
        conformance_selection(module, owner.id, descriptor.rebound_selection_ordinal)?;
    verify_source(
        owner,
        &latest_selection.source,
        latest.source_path,
        latest.source_access,
        latest.source_parameter_position,
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
    )?;
    let selected_rows = application
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait_identity
                == checked.symbols.display_path(latest.declaring_trait, "::")
                && row.public_requirement_identity == latest.requirement_identity
                && row.family_tuple[..] == *latest.family_tuple
                && row.requirement_identity
                    == checked.symbols.display_path(latest.requirement, "::")
                && row.realization_identity
                    == checked.symbols.display_path(latest.realization_state, "::")
        })
        .collect::<Vec<_>>();
    let [selected_row] = selected_rows.as_slice() else {
        return unsupported(
            "rebound dynamic dispatch does not rejoin its selected conformance row",
        );
    };
    if selected_row.realization_callable_identity.as_deref()
        != Some(dispatch.realization_callable_identity.as_str())
        || application
            .realization_callables
            .iter()
            .filter(|callable| {
                callable.source_callable_identity == dispatch.realization_callable_identity
                    && callable.machine == dispatch.realization
            })
            .count()
            != 1
    {
        return unsupported(
            "rebound dynamic dispatch names a callable outside its conformance application",
        );
    }
    // The initial selection retains the descriptor's first source version. It
    // names the same application only when the checked conformance and row
    // roster are unchanged; a distinct application must rejoin the initial
    // selection's own checked custody.
    let initial_selection =
        conformance_selection(module, owner.id, descriptor.initial_selection_ordinal)?;
    verify_source(
        owner,
        &initial_selection.source,
        &initial.path,
        latest.source_access,
        latest.source_parameter_position,
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

fn conformance_selection(
    module: &TerminalModule,
    owner: MachineId,
    ordinal: u32,
) -> Result<&TerminalDynamicConformanceSelection, &'static str> {
    let selections = module
        .dynamic_dispatch
        .selections
        .iter()
        .filter(|selection| selection.owner == owner && selection.ordinal == ordinal)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return unsupported("rebound dynamic conformance selection is absent or ambiguous");
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
) -> Result<(), &'static str> {
    let expected_access = match access {
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => {
            return unsupported(
                "rebound dynamic descriptor source drifted from its checked selection",
            );
        }
    };
    let expected_path = path
        .iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                StructuralPathSegment::Field(identity.clone())
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                StructuralPathSegment::FixedIndex(*index)
            }
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                StructuralPathSegment::FixedByteRange {
                    start: *start,
                    end: *end,
                }
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex {
                selector,
                minimum,
                maximum,
            } => StructuralPathSegment::RuntimeIndex {
                selector: *selector,
                minimum: *minimum,
                maximum: *maximum,
            },
            CheckedUnitStructuralPathSegment::Referent => StructuralPathSegment::Referent,
        })
        .collect::<Vec<_>>();
    let self_parameters = owner
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    let [self_parameter] = self_parameters.as_slice() else {
        return unsupported("rebound dynamic source lost its exact caller self parameter");
    };
    if self_parameter.position != source_parameter_position
        || source.place != self_parameter.place
        || source.path != expected_path
        || source.access != expected_access
    {
        return unsupported("rebound dynamic descriptor source drifted from its checked selection");
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
        return unsupported("rebound dynamic selection names an absent conformance application");
    };
    if application.declaration_identity != checked.symbols.display_path(conformance, "::")
        || application.trait_identity != checked.symbols.display_path(target_trait, "::")
        || application.subject_identity.as_deref() != Some(subject_identity)
        || !application.telescope.is_empty()
        || !application.trait_lifetime_arguments.is_empty()
        || !application.trait_arguments.is_empty()
    {
        return unsupported(
            "rebound dynamic conformance application drifted from its checked selection",
        );
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
        return unsupported(
            "rebound dynamic conformance application drifted from its checked selection",
        );
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
            return unsupported(
                "rebound dynamic conformance application drifted from its checked selection",
            );
        }
        let group = &application.rows[*start..*end];
        let mut tuples = BTreeSet::new();
        let mut nongeneric = false;
        for row in group {
            if !tuples.insert(&row.family_tuple) {
                return unsupported(
                    "rebound dynamic conformance application repeats a family tuple",
                );
            }
            nongeneric |= row.family_tuple.is_empty();
        }
        if nongeneric {
            // A nongeneric requirement expands to exactly its own empty-tuple
            // row, whose realization is the retained row's state itself.
            let [row] = group else {
                return unsupported(
                    "rebound dynamic conformance application drifted from its checked selection",
                );
            };
            if !row.family_tuple.is_empty()
                || row.realization_identity
                    != checked
                        .symbols
                        .display_path(retained.realization_state, "::")
            {
                return unsupported(
                    "rebound dynamic conformance application drifted from its checked selection",
                );
            }
        }
    }
    Ok(application)
}

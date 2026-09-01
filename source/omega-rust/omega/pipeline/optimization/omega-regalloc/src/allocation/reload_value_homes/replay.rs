//! Independent reconstruction as a point-indexed allocation event timeline.

use std::collections::BTreeMap;

mod mechanics;

use omega_optimization_core::OptimizationWorkBudget;
use omega_register_model::{
    RegisterClassId, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use omega_selected_instructions::{SelectedBlockId, VirtualRegisterId};

use crate::{
    AbstractSpillInsertionAction, FunctionReloadValueHomes, LiveRangePoint, ReloadCoexistingHome,
    ReloadValueHomeAssignment, ReloadValueHomeError, ReloadValueHomePlan, ReloadValueHomePolicy,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLogicalSpillOperations, VirtualInterference,
};
use mechanics::{contains_interference, reconstruct_usage, views_overlap};

#[derive(Clone, Copy)]
struct OriginalEvent<'a> {
    legality: &'a crate::VirtualRegisterAllocationLegality,
    exclusive_end: LiveRangePoint,
}

#[derive(Clone, Copy)]
enum Occupant {
    Original {
        register: VirtualRegisterId,
        class: RegisterClassId,
        exclusive_end: LiveRangePoint,
        view: RegisterViewId,
    },
    Reload {
        exclusive_end: LiveRangePoint,
        view: RegisterViewId,
    },
}

impl Occupant {
    const fn end(self) -> LiveRangePoint {
        match self {
            Self::Original { exclusive_end, .. } | Self::Reload { exclusive_end, .. } => {
                exclusive_end
            }
        }
    }

    const fn view(self) -> RegisterViewId {
        match self {
            Self::Original { view, .. } | Self::Reload { view, .. } => view,
        }
    }
}

struct ReloadSpec {
    block: SelectedBlockId,
    start: LiveRangePoint,
    exclusive_end: LiveRangePoint,
    candidates: Vec<RegisterViewId>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: ReloadValueHomePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ReloadValueHomePlan, ReloadValueHomeError> {
    validate_source_chain(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    if policy != ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1 {
        return Err(ReloadValueHomeError::UnsupportedPolicy);
    }
    let mut functions = Vec::with_capacity(insertion.plan().functions.len());
    for function in 0..insertion.plan().functions.len() {
        functions.push(reconstruct_function(
            function,
            &insertion.plan().functions[function],
            &legality.plan().functions[function],
            &ranges.plan().functions[function],
            physical,
        )?);
    }
    let usage = reconstruct_usage(&functions)?;
    if !usage.within(budget) {
        return Err(ReloadValueHomeError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let logical_receipt = logical.receipt();
    Ok(ReloadValueHomePlan {
        abstract_spill_insertion: insertion.receipt().identity(),
        logical_spill_operations: logical_receipt.identity(),
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment: logical_receipt.register_environment(),
        allocator_availability: logical_receipt.allocator_availability(),
        policy,
        budget,
        usage,
        functions,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_source_chain(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), ReloadValueHomeError> {
    let logical_receipt = logical.receipt();
    let derived_environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    let counts = [
        insertion.plan().functions.len(),
        logical.plan().functions.len(),
        legality.plan().functions.len(),
        ranges.plan().functions.len(),
    ];
    let identities_match = insertion.receipt().logical_spill_operations()
        == logical_receipt.identity()
        && logical_receipt.legality() == legality.receipt().identity()
        && logical_receipt.ranges() == ranges.receipt().identity()
        && legality.receipt().ranges() == ranges.receipt().identity()
        && logical_receipt.allocator_availability() == legality.receipt().allocator_availability();
    let environment_matches = derived_environment == logical_receipt.register_environment()
        && legality.receipt().register_environment() == derived_environment
        && constraints.physical_identity() == physical.identity()
        && reservations.physical_identity() == physical.identity()
        && reservations.target() == ranges.plan().target;
    if !counts.iter().all(|count| *count == counts[0]) || !identities_match || !environment_matches
    {
        return Err(ReloadValueHomeError::RootMismatch);
    }
    Ok(())
}

fn reconstruct_function(
    function: usize,
    insertion: &crate::FunctionAbstractSpillInsertion,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionReloadValueHomes, ReloadValueHomeError> {
    if [legality.machine, ranges.machine]
        .into_iter()
        .any(|machine| machine != insertion.machine)
    {
        return Err(ReloadValueHomeError::FunctionMismatch { function });
    }
    if !(ranges.tied_pairs.is_empty() && ranges.early_clobbers.is_empty()) {
        return Err(ReloadValueHomeError::UnsupportedConstraintTopology { function });
    }
    let assignment = insertion
        .action
        .as_ref()
        .map(|action| reconstruct_assignment(function, action, legality, ranges, physical))
        .transpose()?;
    Ok(FunctionReloadValueHomes {
        machine: insertion.machine,
        assignment,
    })
}

fn reconstruct_assignment(
    function: usize,
    action: &AbstractSpillInsertionAction,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<ReloadValueHomeAssignment, ReloadValueHomeError> {
    let spec = reconstruct_reload_spec(function, action, legality)?;
    let events = original_events(function, legality)?;
    validate_overlapping_shapes(function, action, &spec, &events, ranges)?;

    let mut occupants = Vec::<Occupant>::new();
    let mut coexisting = BTreeMap::<VirtualRegisterId, ReloadCoexistingHome>::new();
    let mut reload_view = None;
    for (point, starting) in events.range(..spec.exclusive_end) {
        if reload_view.is_none() && spec.start <= *point {
            reload_view = Some(place_reload(
                function,
                action,
                &spec,
                &mut occupants,
                &mut coexisting,
                physical,
            )?);
        }
        occupants.retain(|occupant| occupant.end() > *point);
        for event in starting {
            let view = place_original(
                function,
                *point,
                *event,
                action,
                ranges,
                &mut occupants,
                physical,
            )?;
            if reload_view.is_some() && event.exclusive_end > spec.start {
                coexisting.insert(
                    event.legality.virtual_register,
                    ReloadCoexistingHome {
                        virtual_register: event.legality.virtual_register,
                        class: event.legality.class,
                        view,
                    },
                );
            }
        }
    }
    let view = match reload_view {
        Some(view) => view,
        None => place_reload(
            function,
            action,
            &spec,
            &mut occupants,
            &mut coexisting,
            physical,
        )?,
    };
    Ok(ReloadValueHomeAssignment {
        result: action.reload.result,
        block: spec.block,
        start: spec.start,
        exclusive_end: spec.exclusive_end,
        class: action.reload.destination_class,
        candidates: spec.candidates,
        view,
        coexisting_homes: coexisting.into_values().collect(),
    })
}

fn reconstruct_reload_spec(
    function: usize,
    action: &AbstractSpillInsertionAction,
    legality: &crate::FunctionAllocationLegality,
) -> Result<ReloadSpec, ReloadValueHomeError> {
    let first = action
        .rewrites
        .first()
        .ok_or(ReloadValueHomeError::UnsupportedReloadShape { function })?;
    let last = action
        .rewrites
        .last()
        .ok_or(ReloadValueHomeError::UnsupportedReloadShape { function })?;
    let exclusive_end = LiveRangePoint(last.point.0.checked_add(1).ok_or(
        ReloadValueHomeError::IntervalOverflow {
            function,
            register: action.victim.0,
        },
    )?);
    let shape_matches = action.reload.before_instruction == first.instruction
        && action.rewrites.windows(2).all(|pair| pair[0] < pair[1])
        && action
            .rewrites
            .iter()
            .all(|rewrite| rewrite.block == first.block && rewrite.result == action.reload.result);
    if !shape_matches {
        return Err(ReloadValueHomeError::UnsupportedReloadShape { function });
    }
    let victim = find_legality(function, legality, action.victim)?;
    if victim.class != action.reload.destination_class {
        return Err(ReloadValueHomeError::UnsupportedReloadShape { function });
    }
    let mut candidates = None::<Vec<RegisterViewId>>;
    for raw_point in first.point.0..exclusive_end.0 {
        let point = LiveRangePoint(raw_point);
        let row = victim
            .points
            .iter()
            .find(|row| row.block == first.block && row.point == point)
            .ok_or(ReloadValueHomeError::UnsupportedReloadShape { function })?;
        match &mut candidates {
            None => candidates = Some(row.candidates.clone()),
            Some(shared) => shared.retain(|view| row.candidates.binary_search(view).is_ok()),
        }
    }
    let candidates = candidates.filter(|views| !views.is_empty()).ok_or(
        ReloadValueHomeError::NoCommonCandidate {
            function,
            register: action.victim.0,
        },
    )?;
    Ok(ReloadSpec {
        block: first.block,
        start: first.point,
        exclusive_end,
        candidates,
    })
}

fn original_events<'a>(
    function: usize,
    legality: &'a crate::FunctionAllocationLegality,
) -> Result<BTreeMap<LiveRangePoint, Vec<OriginalEvent<'a>>>, ReloadValueHomeError> {
    let mut events = BTreeMap::<LiveRangePoint, Vec<OriginalEvent<'a>>>::new();
    for register in &legality.virtual_registers {
        let first = register
            .points
            .first()
            .ok_or(ReloadValueHomeError::NoLivePoints {
                function,
                register: register.virtual_register.0,
            })?;
        let last = register.points.last().expect("nonempty point roster");
        let exclusive_end = LiveRangePoint(last.point.0.checked_add(1).ok_or(
            ReloadValueHomeError::IntervalOverflow {
                function,
                register: register.virtual_register.0,
            },
        )?);
        events.entry(first.point).or_default().push(OriginalEvent {
            legality: register,
            exclusive_end,
        });
    }
    for starting in events.values_mut() {
        starting.sort_by_key(|event| event.legality.virtual_register);
    }
    Ok(events)
}

fn validate_overlapping_shapes(
    function: usize,
    action: &AbstractSpillInsertionAction,
    spec: &ReloadSpec,
    events: &BTreeMap<LiveRangePoint, Vec<OriginalEvent<'_>>>,
    ranges: &crate::FunctionLiveRanges,
) -> Result<(), ReloadValueHomeError> {
    for (start, starting) in events {
        for event in starting {
            if event.exclusive_end <= action.pressure_point || *start >= spec.exclusive_end {
                continue;
            }
            let range = ranges
                .virtual_registers
                .iter()
                .find(|range| range.virtual_register == event.legality.virtual_register)
                .ok_or(ReloadValueHomeError::VirtualRegisterMismatch {
                    function,
                    register: event.legality.virtual_register.0,
                })?;
            let local = range.fragments.as_slice()
                == [crate::LiveRangeFragment {
                    block: spec.block,
                    start: *start,
                    end: event.exclusive_end,
                }];
            if !local || !range.edge_connectors.is_empty() {
                return Err(ReloadValueHomeError::UnsupportedReloadShape { function });
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn place_original(
    function: usize,
    point: LiveRangePoint,
    event: OriginalEvent<'_>,
    action: &AbstractSpillInsertionAction,
    ranges: &crate::FunctionLiveRanges,
    occupants: &mut Vec<Occupant>,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<RegisterViewId, ReloadValueHomeError> {
    let register = event.legality.virtual_register;
    let candidates = intersect_original_domain(function, event.legality)?;
    let view = if register == action.incoming {
        let all_blocked = candidates.iter().all(|candidate| {
            original_conflicts(
                register,
                *candidate,
                occupants,
                &ranges.interference,
                physical,
            )
        });
        let victim_matches = occupants.iter().any(|occupant| {
            matches!(occupant, Occupant::Original { register, view, .. }
                if *register == action.victim && *view == action.victim_view)
        });
        if point != action.pressure_point || !all_blocked || !victim_matches {
            return Err(ReloadValueHomeError::PrefixMismatch { function });
        }
        occupants.retain(|occupant| {
            !matches!(occupant, Occupant::Original { register, .. } if *register == action.victim)
        });
        let recovered = candidates.iter().copied().find(|candidate| {
            !original_conflicts(
                register,
                *candidate,
                occupants,
                &ranges.interference,
                physical,
            )
        });
        if recovered != Some(action.incoming_view) {
            return Err(ReloadValueHomeError::PrefixMismatch { function });
        }
        action.incoming_view
    } else {
        candidates
            .iter()
            .copied()
            .find(|candidate| {
                !original_conflicts(
                    register,
                    *candidate,
                    occupants,
                    &ranges.interference,
                    physical,
                )
            })
            .ok_or_else(|| {
                if point <= action.pressure_point {
                    ReloadValueHomeError::PrefixMismatch { function }
                } else {
                    ReloadValueHomeError::SecondaryPressure {
                        function,
                        register: register.0,
                    }
                }
            })?
    };
    occupants.push(Occupant::Original {
        register,
        class: event.legality.class,
        exclusive_end: event.exclusive_end,
        view,
    });
    Ok(view)
}

fn place_reload(
    function: usize,
    action: &AbstractSpillInsertionAction,
    spec: &ReloadSpec,
    occupants: &mut Vec<Occupant>,
    coexisting: &mut BTreeMap<VirtualRegisterId, ReloadCoexistingHome>,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<RegisterViewId, ReloadValueHomeError> {
    occupants.retain(|occupant| occupant.end() > spec.start);
    let view = spec
        .candidates
        .iter()
        .copied()
        .find(|candidate| {
            occupants
                .iter()
                .all(|occupant| !views_overlap(*candidate, occupant.view(), physical))
        })
        .ok_or(ReloadValueHomeError::ReloadPressure {
            function,
            result: action.reload.result.0,
        })?;
    for occupant in occupants.iter().copied() {
        if let Occupant::Original {
            register,
            class,
            view,
            ..
        } = occupant
        {
            coexisting.insert(
                register,
                ReloadCoexistingHome {
                    virtual_register: register,
                    class,
                    view,
                },
            );
        }
    }
    occupants.push(Occupant::Reload {
        exclusive_end: spec.exclusive_end,
        view,
    });
    Ok(view)
}

fn find_legality<'a>(
    function: usize,
    legality: &'a crate::FunctionAllocationLegality,
    register: VirtualRegisterId,
) -> Result<&'a crate::VirtualRegisterAllocationLegality, ReloadValueHomeError> {
    legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == register)
        .ok_or(ReloadValueHomeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        })
}

fn intersect_original_domain(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<Vec<RegisterViewId>, ReloadValueHomeError> {
    let mut rows = register.points.iter();
    let first = rows.next().ok_or(ReloadValueHomeError::NoLivePoints {
        function,
        register: register.virtual_register.0,
    })?;
    let mut shared = first.candidates.clone();
    for row in rows {
        shared.retain(|candidate| row.candidates.binary_search(candidate).is_ok());
    }
    if shared.is_empty() {
        return Err(ReloadValueHomeError::NoCommonCandidate {
            function,
            register: register.virtual_register.0,
        });
    }
    Ok(shared)
}

fn original_conflicts(
    register: VirtualRegisterId,
    candidate: RegisterViewId,
    occupants: &[Occupant],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    occupants.iter().any(|occupant| {
        let semantic_overlap = match *occupant {
            Occupant::Reload { .. } => true,
            Occupant::Original {
                register: other, ..
            } => contains_interference(register, other, interference),
        };
        semantic_overlap && views_overlap(candidate, occupant.view(), physical)
    })
}

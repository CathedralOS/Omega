use std::collections::BTreeSet;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{
    AllocationLegalityError, AllocationLegalityPlan, AllocationLegalityValidationReceipt,
    EntryFixedViewTransition, LiveRangePoint, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, VirtualEarlyClobberPointLegality,
    VirtualFixedConstraintSite, VirtualLiveRange, VirtualPointLegality,
    VirtualRegisterAllocationLegality, allocation_legality_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    plan: AllocationLegalityPlan,
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    let target = ranges.plan().target;
    if target.architecture != physical.model().architecture
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != target
        || plan.ranges != ranges.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != availability.receipt().identity()
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical.identity()
        || register_environment
            != target_register_environment_identity(
                target,
                physical,
                constraints,
                reservations,
                selected_keys,
            )
    {
        return Err(AllocationLegalityError::RootMismatch);
    }
    if plan.functions.len() != ranges.plan().functions.len() {
        return Err(AllocationLegalityError::FunctionMismatch {
            function: plan.functions.len().min(ranges.plan().functions.len()),
        });
    }
    if plan.structural_unit_functions.len() != ranges.plan().structural_unit_functions.len() {
        return Err(AllocationLegalityError::FunctionMismatch {
            function: plan
                .structural_unit_functions
                .len()
                .min(ranges.plan().structural_unit_functions.len()),
        });
    }
    for (function_index, (actual, source)) in plan
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .enumerate()
    {
        if actual.machine != source.machine
            || actual.virtual_registers.len() != source.virtual_registers.len()
        {
            return Err(AllocationLegalityError::FunctionMismatch {
                function: function_index,
            });
        }
        for (actual, source_register) in actual
            .virtual_registers
            .iter()
            .zip(&source.virtual_registers)
        {
            let expected = replay_register(
                function_index,
                source,
                source_register,
                availability,
                physical,
                reservations,
            )?;
            if actual != &expected {
                return Err(AllocationLegalityError::VirtualRegisterMismatch {
                    function: function_index,
                    register: source_register.virtual_register.0,
                });
            }
            validate_canonical(function_index, actual)?;
        }
    }
    for (function_index, (actual, source)) in plan
        .structural_unit_functions
        .iter()
        .zip(&ranges.plan().structural_unit_functions)
        .enumerate()
    {
        if actual.machine != source.machine
            || !actual.virtual_registers.is_empty()
            || !source.virtual_registers.is_empty()
            || !source.tied_pairs.is_empty()
            || !source.early_clobbers.is_empty()
            || !source.interference.is_empty()
        {
            return Err(AllocationLegalityError::FunctionMismatch {
                function: function_index,
            });
        }
    }
    let identity = allocation_legality_identity(&plan);
    let receipt = AllocationLegalityValidationReceipt {
        identity,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        function_count: plan.functions.len(),
        structural_unit_function_count: plan.structural_unit_functions.len(),
        virtual_register_count: plan
            .functions
            .iter()
            .map(|function| function.virtual_registers.len())
            .sum(),
        point_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.virtual_registers)
            .map(|register| register.points.len())
            .sum(),
        candidate_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.virtual_registers)
            .flat_map(|register| &register.points)
            .map(|point| point.candidates.len())
            .sum(),
        early_clobber_point_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.virtual_registers)
            .map(|register| register.early_clobber_points.len())
            .sum(),
        early_clobber_candidate_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.virtual_registers)
            .flat_map(|register| &register.early_clobber_points)
            .map(|point| point.candidates.len())
            .sum(),
        entry_transition_count: plan
            .functions
            .iter()
            .flat_map(|function| &function.virtual_registers)
            .map(|register| register.entry_transitions.len())
            .sum(),
    };
    Ok(ValidatedAllocationLegality {
        plan: plan.into(),
        receipt,
    })
}

fn replay_register(
    function_index: usize,
    function: &crate::FunctionLiveRanges,
    register: &VirtualLiveRange,
    availability: &ValidatedAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<VirtualRegisterAllocationLegality, AllocationLegalityError> {
    let class = physical
        .model()
        .classes
        .iter()
        .find(|class| class.id == register.class)
        .ok_or(AllocationLegalityError::UnknownClass {
            function: function_index,
            register: register.virtual_register.0,
            class: register.class.0,
        })?;
    let entry_location = register
        .fragments
        .first()
        .map(|fragment| (fragment.block, fragment.start));
    let available = availability.unconstrained_views(register.class).ok_or(
        AllocationLegalityError::UnknownClass {
            function: function_index,
            register: register.virtual_register.0,
            class: register.class.0,
        },
    )?;
    let mut points = Vec::new();
    for fragment in &register.fragments {
        let mut point = fragment.start;
        while point < fragment.end {
            let fixed = register
                .fixed_constraints
                .iter()
                .filter_map(|constraint| {
                    let applies = match constraint.site {
                        VirtualFixedConstraintSite::Entry => {
                            entry_location == Some((fragment.block, point))
                        }
                        VirtualFixedConstraintSite::Operand {
                            point: fixed_point, ..
                        } => fixed_point == point,
                    };
                    applies.then_some(constraint.view)
                })
                .collect::<BTreeSet<_>>();
            if fixed.len() > 1 {
                return Err(AllocationLegalityError::IllegalFixedView {
                    function: function_index,
                    register: register.virtual_register.0,
                    view: fixed.last().expect("two fixed views exist").0,
                });
            }
            let fixed = fixed.into_iter().next();
            let occupied = occupied_units(function, fragment.block, point, reservations);
            let mut candidates = class
                .views
                .iter()
                .filter(|view_id| available.binary_search(view_id).is_ok())
                .filter_map(|view_id| {
                    let view = physical
                        .model()
                        .views
                        .iter()
                        .find(|view| view.id == *view_id)?;
                    (view.allocatable
                        && view
                            .units
                            .iter()
                            .chain(&view.write_units)
                            .all(|unit| !occupied.contains(unit)))
                    .then_some(*view_id)
                })
                .collect::<Vec<_>>();
            if let Some(fixed) = fixed {
                let fixed_view = physical
                    .model()
                    .views
                    .iter()
                    .find(|view| view.id == fixed)
                    .ok_or(AllocationLegalityError::UnknownFixedView {
                        function: function_index,
                        register: register.virtual_register.0,
                        view: fixed.0,
                    })?;
                if fixed_view.class != register.class
                    || fixed_view
                        .units
                        .iter()
                        .chain(&fixed_view.write_units)
                        .any(|unit| occupied.contains(unit))
                {
                    return Err(AllocationLegalityError::IllegalFixedView {
                        function: function_index,
                        register: register.virtual_register.0,
                        view: fixed.0,
                    });
                }
                candidates = vec![fixed];
            }
            if candidates.is_empty() {
                return Err(AllocationLegalityError::NoCandidateViews {
                    function: function_index,
                    register: register.virtual_register.0,
                    block: fragment.block.0,
                    point: point.0,
                });
            }
            points.push(VirtualPointLegality {
                block: fragment.block,
                point,
                candidates,
            });
            point = LiveRangePoint(point.0.checked_add(1).ok_or(
                AllocationLegalityError::PointOverflow {
                    function: function_index,
                },
            )?);
        }
    }
    let mut early_clobber_points = Vec::new();
    for early in function
        .early_clobbers
        .iter()
        .filter(|early| early.def_virtual_register == register.virtual_register)
    {
        if early.def_class != register.class {
            return Err(AllocationLegalityError::UnknownClass {
                function: function_index,
                register: register.virtual_register.0,
                class: early.def_class.0,
            });
        }
        let fixed = register
            .fixed_constraints
            .iter()
            .filter_map(|constraint| match constraint.site {
                VirtualFixedConstraintSite::Operand {
                    position,
                    instruction,
                    operand,
                    access: register_model::RegisterOperandAccess::Def,
                    ..
                } if position == early.position
                    && instruction == early.instruction
                    && operand == early.def_operand =>
                {
                    Some(constraint.view)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if fixed.len() > 1 {
            return Err(AllocationLegalityError::IllegalFixedView {
                function: function_index,
                register: register.virtual_register.0,
                view: fixed.last().expect("two fixed views exist").0,
            });
        }
        let occupied = occupied_units(function, early.block, early.early_point, reservations);
        let mut candidates = class
            .views
            .iter()
            .filter(|view_id| available.binary_search(view_id).is_ok())
            .filter_map(|view_id| {
                let view = physical
                    .model()
                    .views
                    .iter()
                    .find(|view| view.id == *view_id)?;
                (view.allocatable
                    && view
                        .units
                        .iter()
                        .chain(&view.write_units)
                        .all(|unit| !occupied.contains(unit)))
                .then_some(*view_id)
            })
            .collect::<Vec<_>>();
        if let Some(fixed) = fixed.into_iter().next() {
            let fixed_view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == fixed)
                .ok_or(AllocationLegalityError::UnknownFixedView {
                    function: function_index,
                    register: register.virtual_register.0,
                    view: fixed.0,
                })?;
            if fixed_view.class != register.class
                || fixed_view
                    .units
                    .iter()
                    .chain(&fixed_view.write_units)
                    .any(|unit| occupied.contains(unit))
            {
                return Err(AllocationLegalityError::IllegalFixedView {
                    function: function_index,
                    register: register.virtual_register.0,
                    view: fixed.0,
                });
            }
            candidates = vec![fixed];
        }
        if candidates.is_empty() {
            return Err(AllocationLegalityError::NoCandidateViews {
                function: function_index,
                register: register.virtual_register.0,
                block: early.block.0,
                point: early.early_point.0,
            });
        }
        early_clobber_points.push(VirtualEarlyClobberPointLegality {
            block: early.block,
            position: early.position,
            instruction: early.instruction,
            operand: early.def_operand,
            point: early.early_point,
            candidates,
        });
    }
    let entry = register.fixed_constraints.iter().find_map(|constraint| {
        matches!(constraint.site, VirtualFixedConstraintSite::Entry).then_some(constraint.view)
    });
    let entry_transitions = entry
        .map(|entry| {
            register
                .fixed_constraints
                .iter()
                .filter_map(|constraint| match constraint.site {
                    VirtualFixedConstraintSite::Entry => None,
                    site if constraint.view != entry => Some(EntryFixedViewTransition {
                        from_view: entry,
                        to_site: site,
                        to_view: constraint.view,
                    }),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(VirtualRegisterAllocationLegality {
        virtual_register: register.virtual_register,
        class: register.class,
        points,
        early_clobber_points,
        entry_transitions,
    })
}

#[cfg(test)]
pub(crate) fn replay_register_for_test(
    function_index: usize,
    function: &crate::FunctionLiveRanges,
    register: &VirtualLiveRange,
    availability: &ValidatedAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<VirtualRegisterAllocationLegality, AllocationLegalityError> {
    replay_register(
        function_index,
        function,
        register,
        availability,
        physical,
        reservations,
    )
}

fn occupied_units(
    function: &crate::FunctionLiveRanges,
    block: selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    reservations: &ValidatedRegisterReservationProfile,
) -> BTreeSet<register_model::RegisterUnitId> {
    let mut occupied = reservations
        .reserved_units()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for row in &function.architectural_units {
        if row.fragments.iter().any(|fragment| {
            fragment.block == block && fragment.start <= point && point < fragment.end
        }) || row
            .actions
            .iter()
            .any(|action| action.block == block && action.point == point)
        {
            occupied.insert(row.unit);
        }
    }
    occupied
}

fn validate_canonical(
    function_index: usize,
    register: &VirtualRegisterAllocationLegality,
) -> Result<(), AllocationLegalityError> {
    if register
        .points
        .windows(2)
        .any(|pair| (pair[0].block, pair[0].point) >= (pair[1].block, pair[1].point))
        || register.points.iter().any(|point| {
            point.candidates.is_empty()
                || point.candidates.windows(2).any(|pair| pair[0] >= pair[1])
        })
        || register.early_clobber_points.windows(2).any(|pair| {
            (
                pair[0].block,
                pair[0].point,
                pair[0].instruction,
                pair[0].operand,
            ) >= (
                pair[1].block,
                pair[1].point,
                pair[1].instruction,
                pair[1].operand,
            )
        })
        || register.early_clobber_points.iter().any(|point| {
            point.candidates.is_empty()
                || point.candidates.windows(2).any(|pair| pair[0] >= pair[1])
        })
    {
        return Err(AllocationLegalityError::NonCanonicalRows {
            function: function_index,
            register: register.virtual_register.0,
        });
    }
    Ok(())
}

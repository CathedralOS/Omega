use std::collections::BTreeSet;

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{
    TerminalAllocationLegalityError, TerminalAllocationLegalityPlan,
    TerminalAllocationLegalityValidationReceipt, TerminalEntryFixedViewTransition,
    TerminalLiveRangePoint, TerminalVirtualEarlyClobberPointLegality,
    TerminalVirtualFixedConstraintSite, TerminalVirtualLiveRange, TerminalVirtualPointLegality,
    TerminalVirtualRegisterAllocationLegality, ValidatedTerminalAllocationLegality,
    ValidatedTerminalAllocatorAvailability, ValidatedTerminalLiveRanges,
    terminal_allocation_legality_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_allocation_legality(
    ranges: &ValidatedTerminalLiveRanges,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: TerminalAllocationLegalityPlan,
) -> Result<ValidatedTerminalAllocationLegality, TerminalAllocationLegalityError> {
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
        return Err(TerminalAllocationLegalityError::RootMismatch);
    }
    if plan.functions.len() != ranges.plan().functions.len() {
        return Err(TerminalAllocationLegalityError::FunctionMismatch {
            function: plan.functions.len().min(ranges.plan().functions.len()),
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
            return Err(TerminalAllocationLegalityError::FunctionMismatch {
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
                return Err(TerminalAllocationLegalityError::VirtualRegisterMismatch {
                    function: function_index,
                    register: source_register.virtual_register.0,
                });
            }
            validate_canonical(function_index, actual)?;
        }
    }
    let identity = terminal_allocation_legality_identity(&plan);
    let receipt = TerminalAllocationLegalityValidationReceipt {
        identity,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        function_count: plan.functions.len(),
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
    Ok(ValidatedTerminalAllocationLegality { plan, receipt })
}

fn replay_register(
    function_index: usize,
    function: &crate::TerminalFunctionLiveRanges,
    register: &TerminalVirtualLiveRange,
    availability: &ValidatedTerminalAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<TerminalVirtualRegisterAllocationLegality, TerminalAllocationLegalityError> {
    let class = physical
        .model()
        .classes
        .iter()
        .find(|class| class.id == register.class)
        .ok_or(TerminalAllocationLegalityError::UnknownClass {
            function: function_index,
            register: register.virtual_register.0,
            class: register.class.0,
        })?;
    let entry_location = register
        .fragments
        .first()
        .map(|fragment| (fragment.block, fragment.start));
    let available = availability.unconstrained_views(register.class).ok_or(
        TerminalAllocationLegalityError::UnknownClass {
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
                        TerminalVirtualFixedConstraintSite::Entry => {
                            entry_location == Some((fragment.block, point))
                        }
                        TerminalVirtualFixedConstraintSite::Operand {
                            point: fixed_point, ..
                        } => fixed_point == point,
                    };
                    applies.then_some(constraint.view)
                })
                .collect::<BTreeSet<_>>();
            if fixed.len() > 1 {
                return Err(TerminalAllocationLegalityError::IllegalFixedView {
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
                    .ok_or(TerminalAllocationLegalityError::UnknownFixedView {
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
                    return Err(TerminalAllocationLegalityError::IllegalFixedView {
                        function: function_index,
                        register: register.virtual_register.0,
                        view: fixed.0,
                    });
                }
                candidates = vec![fixed];
            }
            if candidates.is_empty() {
                return Err(TerminalAllocationLegalityError::NoCandidateViews {
                    function: function_index,
                    register: register.virtual_register.0,
                    block: fragment.block.0,
                    point: point.0,
                });
            }
            points.push(TerminalVirtualPointLegality {
                block: fragment.block,
                point,
                candidates,
            });
            point = TerminalLiveRangePoint(point.0.checked_add(1).ok_or(
                TerminalAllocationLegalityError::PointOverflow {
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
            return Err(TerminalAllocationLegalityError::UnknownClass {
                function: function_index,
                register: register.virtual_register.0,
                class: early.def_class.0,
            });
        }
        let fixed = register
            .fixed_constraints
            .iter()
            .filter_map(|constraint| match constraint.site {
                TerminalVirtualFixedConstraintSite::Operand {
                    position,
                    instruction,
                    operand,
                    access: omega_register_model::RegisterOperandAccess::Def,
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
            return Err(TerminalAllocationLegalityError::IllegalFixedView {
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
                .ok_or(TerminalAllocationLegalityError::UnknownFixedView {
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
                return Err(TerminalAllocationLegalityError::IllegalFixedView {
                    function: function_index,
                    register: register.virtual_register.0,
                    view: fixed.0,
                });
            }
            candidates = vec![fixed];
        }
        if candidates.is_empty() {
            return Err(TerminalAllocationLegalityError::NoCandidateViews {
                function: function_index,
                register: register.virtual_register.0,
                block: early.block.0,
                point: early.early_point.0,
            });
        }
        early_clobber_points.push(TerminalVirtualEarlyClobberPointLegality {
            block: early.block,
            position: early.position,
            instruction: early.instruction,
            operand: early.def_operand,
            point: early.early_point,
            candidates,
        });
    }
    let entry = register.fixed_constraints.iter().find_map(|constraint| {
        matches!(constraint.site, TerminalVirtualFixedConstraintSite::Entry)
            .then_some(constraint.view)
    });
    let entry_transitions = entry
        .map(|entry| {
            register
                .fixed_constraints
                .iter()
                .filter_map(|constraint| match constraint.site {
                    TerminalVirtualFixedConstraintSite::Entry => None,
                    site if constraint.view != entry => Some(TerminalEntryFixedViewTransition {
                        from_view: entry,
                        to_site: site,
                        to_view: constraint.view,
                    }),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(TerminalVirtualRegisterAllocationLegality {
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
    function: &crate::TerminalFunctionLiveRanges,
    register: &TerminalVirtualLiveRange,
    availability: &ValidatedTerminalAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<TerminalVirtualRegisterAllocationLegality, TerminalAllocationLegalityError> {
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
    function: &crate::TerminalFunctionLiveRanges,
    block: omega_terminal_selected_instructions::TerminalSelectedBlockId,
    point: TerminalLiveRangePoint,
    reservations: &ValidatedRegisterReservationProfile,
) -> BTreeSet<omega_register_model::RegisterUnitId> {
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
    register: &TerminalVirtualRegisterAllocationLegality,
) -> Result<(), TerminalAllocationLegalityError> {
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
        return Err(TerminalAllocationLegalityError::NonCanonicalRows {
            function: function_index,
            register: register.virtual_register.0,
        });
    }
    Ok(())
}

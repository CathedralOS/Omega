use std::collections::BTreeSet;

use omega_register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};

use crate::{
    TerminalAllocationLegalityError, TerminalAllocationLegalityPlan,
    TerminalEntryFixedViewTransition, TerminalFunctionAllocationLegality,
    TerminalFunctionLiveRanges, TerminalLiveRangePoint, TerminalVirtualEarlyClobberPointLegality,
    TerminalVirtualFixedConstraintSite, TerminalVirtualPointLegality,
    TerminalVirtualRegisterAllocationLegality, ValidatedTerminalAllocatorAvailability,
    ValidatedTerminalLiveRanges,
};

pub(crate) fn compute_terminal_allocation_legality(
    ranges: &ValidatedTerminalLiveRanges,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<TerminalAllocationLegalityPlan, TerminalAllocationLegalityError> {
    validate_roots(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
    )?;
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    if environment != register_environment {
        return Err(TerminalAllocationLegalityError::RootMismatch);
    }
    let functions = ranges
        .plan()
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            compute_function(
                function_index,
                function,
                availability,
                physical,
                reservations,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalAllocationLegalityPlan {
        ranges: ranges.receipt().identity(),
        register_environment,
        allocator_availability: availability.receipt().identity(),
        functions,
    })
}

fn validate_roots(
    ranges: &ValidatedTerminalLiveRanges,
    availability: &ValidatedTerminalAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<(), TerminalAllocationLegalityError> {
    let physical_identity = physical.identity();
    if ranges.plan().target.architecture != physical.model().architecture
        || constraints.physical_identity() != physical_identity
        || reservations.physical_identity() != physical_identity
        || reservations.target() != ranges.plan().target
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical_identity
    {
        return Err(TerminalAllocationLegalityError::RootMismatch);
    }
    Ok(())
}

fn compute_function(
    function_index: usize,
    function: &TerminalFunctionLiveRanges,
    availability: &ValidatedTerminalAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<TerminalFunctionAllocationLegality, TerminalAllocationLegalityError> {
    let virtual_registers = function
        .virtual_registers
        .iter()
        .map(|register| {
            let class = physical
                .model()
                .classes
                .get(usize::from(register.class.0))
                .filter(|class| class.id == register.class)
                .ok_or(TerminalAllocationLegalityError::UnknownClass {
                    function: function_index,
                    register: register.virtual_register.0,
                    class: register.class.0,
                })?;
            let entry_point = register
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
                for raw_point in fragment.start.0..fragment.end.0 {
                    let point = TerminalLiveRangePoint(raw_point);
                    let fixed = fixed_view_at_point(
                        function_index,
                        register,
                        fragment.block,
                        point,
                        entry_point,
                    )?;
                    let mut candidates = class
                        .views
                        .iter()
                        .copied()
                        .filter(|view_id| available.binary_search(view_id).is_ok())
                        .filter(|view_id| {
                            physical
                                .model()
                                .views
                                .get(usize::from(view_id.0))
                                .is_some_and(|view| {
                                    view.id == *view_id
                                        && view.allocatable
                                        && !view_conflicts(
                                            view,
                                            fragment.block,
                                            point,
                                            function,
                                            reservations,
                                        )
                                })
                        })
                        .collect::<Vec<_>>();
                    if let Some(fixed) = fixed {
                        let Some(view) = physical.model().views.get(usize::from(fixed.0)) else {
                            return Err(TerminalAllocationLegalityError::UnknownFixedView {
                                function: function_index,
                                register: register.virtual_register.0,
                                view: fixed.0,
                            });
                        };
                        if view.id != fixed || view.class != register.class {
                            return Err(TerminalAllocationLegalityError::UnknownFixedView {
                                function: function_index,
                                register: register.virtual_register.0,
                                view: fixed.0,
                            });
                        }
                        if view_conflicts(view, fragment.block, point, function, reservations) {
                            return Err(TerminalAllocationLegalityError::IllegalFixedView {
                                function: function_index,
                                register: register.virtual_register.0,
                                view: fixed.0,
                            });
                        }
                        candidates.clear();
                        candidates.push(fixed);
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
                let fixed = fixed_view_for_early_clobber(function_index, register, early)?;
                let mut candidates = class
                    .views
                    .iter()
                    .copied()
                    .filter(|view_id| available.binary_search(view_id).is_ok())
                    .filter(|view_id| {
                        physical
                            .model()
                            .views
                            .get(usize::from(view_id.0))
                            .is_some_and(|view| {
                                view.id == *view_id
                                    && view.allocatable
                                    && !view_conflicts(
                                        view,
                                        early.block,
                                        early.early_point,
                                        function,
                                        reservations,
                                    )
                            })
                    })
                    .collect::<Vec<_>>();
                if let Some(fixed) = fixed {
                    let Some(view) = physical.model().views.get(usize::from(fixed.0)) else {
                        return Err(TerminalAllocationLegalityError::UnknownFixedView {
                            function: function_index,
                            register: register.virtual_register.0,
                            view: fixed.0,
                        });
                    };
                    if view.id != fixed || view.class != register.class {
                        return Err(TerminalAllocationLegalityError::UnknownFixedView {
                            function: function_index,
                            register: register.virtual_register.0,
                            view: fixed.0,
                        });
                    }
                    if view_conflicts(view, early.block, early.early_point, function, reservations)
                    {
                        return Err(TerminalAllocationLegalityError::IllegalFixedView {
                            function: function_index,
                            register: register.virtual_register.0,
                            view: fixed.0,
                        });
                    }
                    candidates.clear();
                    candidates.push(fixed);
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
            let entry_transitions = entry_transitions(register);
            Ok(TerminalVirtualRegisterAllocationLegality {
                virtual_register: register.virtual_register,
                class: register.class,
                points,
                early_clobber_points,
                entry_transitions,
            })
        })
        .collect::<Result<Vec<_>, TerminalAllocationLegalityError>>()?;
    Ok(TerminalFunctionAllocationLegality {
        machine: function.machine,
        virtual_registers,
    })
}

fn fixed_view_for_early_clobber(
    function_index: usize,
    register: &crate::TerminalVirtualLiveRange,
    early: &crate::TerminalEarlyClobberConstraint,
) -> Result<Option<RegisterViewId>, TerminalAllocationLegalityError> {
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
    Ok(fixed.into_iter().next())
}

fn fixed_view_at_point(
    function_index: usize,
    register: &crate::TerminalVirtualLiveRange,
    block: omega_terminal_selected_instructions::TerminalSelectedBlockId,
    point: TerminalLiveRangePoint,
    entry_point: Option<(
        omega_terminal_selected_instructions::TerminalSelectedBlockId,
        TerminalLiveRangePoint,
    )>,
) -> Result<Option<RegisterViewId>, TerminalAllocationLegalityError> {
    let mut fixed = BTreeSet::new();
    for constraint in &register.fixed_constraints {
        let applies = match constraint.site {
            TerminalVirtualFixedConstraintSite::Entry => entry_point == Some((block, point)),
            TerminalVirtualFixedConstraintSite::Operand {
                point: constraint_point,
                ..
            } => constraint_point == point,
        };
        if applies {
            fixed.insert(constraint.view);
        }
    }
    if fixed.len() > 1 {
        return Err(TerminalAllocationLegalityError::IllegalFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.last().expect("two fixed views exist").0,
        });
    }
    Ok(fixed.into_iter().next())
}

fn entry_transitions(
    register: &crate::TerminalVirtualLiveRange,
) -> Vec<TerminalEntryFixedViewTransition> {
    let Some(entry) = register
        .fixed_constraints
        .iter()
        .find(|constraint| matches!(constraint.site, TerminalVirtualFixedConstraintSite::Entry))
    else {
        return Vec::new();
    };
    register
        .fixed_constraints
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.site,
                TerminalVirtualFixedConstraintSite::Operand { .. }
            )
        })
        .filter(|constraint| constraint.view != entry.view)
        .map(|constraint| TerminalEntryFixedViewTransition {
            from_view: entry.view,
            to_site: constraint.site,
            to_view: constraint.view,
        })
        .collect()
}

fn view_conflicts(
    view: &RegisterView,
    block: omega_terminal_selected_instructions::TerminalSelectedBlockId,
    point: TerminalLiveRangePoint,
    function: &TerminalFunctionLiveRanges,
    reservations: &ValidatedRegisterReservationProfile,
) -> bool {
    view.units.iter().chain(&view.write_units).any(|unit| {
        reservations.reserved_units().binary_search(unit).is_ok()
            || function
                .architectural_units
                .iter()
                .find(|row| row.unit == *unit)
                .is_some_and(|row| {
                    row.fragments.iter().any(|fragment| {
                        fragment.block == block && fragment.start <= point && point < fragment.end
                    }) || row
                        .actions
                        .iter()
                        .any(|action| action.block == block && action.point == point)
                })
    })
}

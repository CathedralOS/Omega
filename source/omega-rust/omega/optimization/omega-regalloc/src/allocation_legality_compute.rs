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

#[cfg(test)]
mod tests {
    use omega_register_model::{
        PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterReservationProfile,
        RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId,
        RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
        validate_physical_register_model, validate_register_reservation_profile,
    };
    use omega_target::{Architecture, NativeTarget, ObjectFormat};
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalVirtualRegisterId,
    };
    use psi_core::MachineId;

    use super::compute_function;
    use crate::{
        TerminalAllocatorAvailabilityPlan, TerminalAllocatorAvailabilityPolicy,
        TerminalAllocatorAvailabilityValidationReceipt, TerminalDistinctUseDefTie,
        TerminalEarlyClobberConstraint, TerminalEarlyClobberUse, TerminalFunctionLiveRanges,
        TerminalLiveRangeFragment, TerminalLiveRangePoint, TerminalLivenessPosition,
        TerminalRegisterClassAvailability, TerminalVirtualLiveRange,
        ValidatedTerminalAllocatorAvailability, terminal_allocator_availability_identity,
    };

    fn physical() -> omega_register_model::ValidatedPhysicalRegisterModel {
        validate_physical_register_model(PhysicalRegisterModel {
            architecture: Architecture::X86_64,
            units: (0..2)
                .map(|id| RegisterUnit {
                    id: RegisterUnitId(id),
                    name: format!("r{id}.storage"),
                    bits: 64,
                    kind: RegisterUnitKind::IntegerLane,
                })
                .collect(),
            views: (0..2)
                .map(|id| RegisterView {
                    id: RegisterViewId(id),
                    name: format!("r{id}"),
                    class: RegisterClassId(0),
                    units: vec![RegisterUnitId(id)],
                    write_units: vec![RegisterUnitId(id)],
                    bits: 64,
                    write_semantics: RegisterWriteSemantics::ExactView,
                    allocatable: true,
                })
                .collect(),
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "integer".into(),
                views: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
            conventions: Vec::new(),
            reservations: Vec::new(),
        })
        .unwrap()
    }

    fn availability(
        physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    ) -> ValidatedTerminalAllocatorAvailability {
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes([1; 32]);
        let plan = TerminalAllocatorAvailabilityPlan {
            register_environment,
            physical: physical.identity(),
            policy: TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
            classes: vec![TerminalRegisterClassAvailability {
                class: RegisterClassId(0),
                unconstrained_views: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
        };
        let receipt = TerminalAllocatorAvailabilityValidationReceipt {
            identity: terminal_allocator_availability_identity(&plan),
            register_environment,
            physical: physical.identity(),
            class_count: 1,
            unconstrained_view_count: 2,
        };
        ValidatedTerminalAllocatorAvailability { plan, receipt }
    }

    fn range(register: u32, start: u32, end: u32) -> TerminalVirtualLiveRange {
        TerminalVirtualLiveRange {
            virtual_register: TerminalVirtualRegisterId(register),
            class: RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments: vec![TerminalLiveRangeFragment {
                block: TerminalSelectedBlockId(0),
                start: TerminalLiveRangePoint(start),
                end: TerminalLiveRangePoint(end),
            }],
            edge_connectors: Vec::new(),
        }
    }

    fn early(position: u32, used: u32, defined: u32) -> TerminalEarlyClobberConstraint {
        TerminalEarlyClobberConstraint {
            block: TerminalSelectedBlockId(0),
            position: TerminalLivenessPosition(position),
            instruction: TerminalSelectedInstructionId(position),
            early_point: TerminalLiveRangePoint(position * 2),
            def_operand: 1,
            def_virtual_register: TerminalVirtualRegisterId(defined),
            def_class: RegisterClassId(0),
            def_point: TerminalLiveRangePoint(position * 2 + 1),
            uses: vec![TerminalEarlyClobberUse {
                operand: 0,
                virtual_register: TerminalVirtualRegisterId(used),
                class: RegisterClassId(0),
            }],
        }
    }

    #[test]
    fn computes_before_phase_candidates_for_each_early_clobber_row() {
        let physical = physical();
        let target = NativeTarget {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        let reservations = validate_register_reservation_profile(
            RegisterReservationProfile {
                name: "none".into(),
                active_overlays: Vec::new(),
            },
            target,
            &physical,
        )
        .unwrap();
        let availability = availability(&physical);
        let ranges = TerminalFunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: vec![range(0, 0, 1), range(1, 1, 3), range(2, 3, 4)],
            tied_pairs: Vec::<TerminalDistinctUseDefTie>::new(),
            early_clobbers: vec![early(0, 0, 1), early(1, 1, 2)],
            architectural_units: Vec::new(),
            interference: Vec::new(),
        };
        let legality =
            compute_function(0, &ranges, &availability, &physical, &reservations).unwrap();
        let replayed = ranges
            .virtual_registers
            .iter()
            .map(|register| {
                crate::allocation_legality_validate::replay_register_for_test(
                    0,
                    &ranges,
                    register,
                    &availability,
                    &physical,
                    &reservations,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(legality.virtual_registers, replayed);
        assert_eq!(legality.virtual_registers[1].early_clobber_points.len(), 1);
        assert_eq!(legality.virtual_registers[2].early_clobber_points.len(), 1);
        assert_eq!(
            legality.virtual_registers[1].early_clobber_points[0].point,
            TerminalLiveRangePoint(0)
        );
        assert_eq!(
            legality.virtual_registers[2].early_clobber_points[0].point,
            TerminalLiveRangePoint(2)
        );
        assert_eq!(
            legality.virtual_registers[2].early_clobber_points[0].candidates,
            vec![RegisterViewId(0), RegisterViewId(1)]
        );
    }

    #[test]
    fn computes_before_phase_legality_for_isolated_tied_early_definition() {
        let physical = physical();
        let target = NativeTarget {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        let reservations = validate_register_reservation_profile(
            RegisterReservationProfile {
                name: "none".into(),
                active_overlays: Vec::new(),
            },
            target,
            &physical,
        )
        .unwrap();
        let availability = availability(&physical);
        let ranges = TerminalFunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: vec![range(0, 0, 1), range(1, 0, 1), range(2, 1, 2)],
            tied_pairs: vec![TerminalDistinctUseDefTie {
                block: TerminalSelectedBlockId(0),
                position: TerminalLivenessPosition(0),
                instruction: TerminalSelectedInstructionId(0),
                use_operand: 0,
                use_virtual_register: TerminalVirtualRegisterId(0),
                use_point: TerminalLiveRangePoint(0),
                def_operand: 2,
                def_virtual_register: TerminalVirtualRegisterId(2),
                def_point: TerminalLiveRangePoint(1),
                class: RegisterClassId(0),
            }],
            early_clobbers: vec![TerminalEarlyClobberConstraint {
                block: TerminalSelectedBlockId(0),
                position: TerminalLivenessPosition(0),
                instruction: TerminalSelectedInstructionId(0),
                early_point: TerminalLiveRangePoint(0),
                def_operand: 2,
                def_virtual_register: TerminalVirtualRegisterId(2),
                def_class: RegisterClassId(0),
                def_point: TerminalLiveRangePoint(1),
                uses: vec![TerminalEarlyClobberUse {
                    operand: 1,
                    virtual_register: TerminalVirtualRegisterId(1),
                    class: RegisterClassId(0),
                }],
            }],
            architectural_units: Vec::new(),
            interference: Vec::new(),
        };
        let legality =
            compute_function(0, &ranges, &availability, &physical, &reservations).unwrap();
        let replayed = crate::allocation_legality_validate::replay_register_for_test(
            0,
            &ranges,
            &ranges.virtual_registers[2],
            &availability,
            &physical,
            &reservations,
        )
        .unwrap();
        assert_eq!(legality.virtual_registers[2], replayed);
        assert_eq!(
            legality.virtual_registers[2].early_clobber_points[0].candidates,
            vec![RegisterViewId(0), RegisterViewId(1)]
        );
    }
}

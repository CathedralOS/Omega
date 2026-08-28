use std::collections::BTreeSet;

use omega_register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};

use crate::{
    AllocationLegalityError, AllocationLegalityPlan, EntryFixedViewTransition,
    FunctionAllocationLegality, FunctionLiveRanges, LiveRangePoint, ValidatedAllocatorAvailability,
    ValidatedLiveRanges, VirtualEarlyClobberPointLegality, VirtualFixedConstraintSite,
    VirtualPointLegality, VirtualRegisterAllocationLegality,
};

pub(crate) fn compute_terminal_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<AllocationLegalityPlan, AllocationLegalityError> {
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
        return Err(AllocationLegalityError::RootMismatch);
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
    let structural_unit_functions = ranges
        .plan()
        .structural_unit_functions
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
    Ok(AllocationLegalityPlan {
        ranges: ranges.receipt().identity(),
        register_environment,
        allocator_availability: availability.receipt().identity(),
        functions,
        structural_unit_functions,
    })
}

fn validate_roots(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<(), AllocationLegalityError> {
    let physical_identity = physical.identity();
    if ranges.plan().target.architecture != physical.model().architecture
        || constraints.physical_identity() != physical_identity
        || reservations.physical_identity() != physical_identity
        || reservations.target() != ranges.plan().target
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical_identity
    {
        return Err(AllocationLegalityError::RootMismatch);
    }
    Ok(())
}

fn compute_function(
    function_index: usize,
    function: &FunctionLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<FunctionAllocationLegality, AllocationLegalityError> {
    let virtual_registers = function
        .virtual_registers
        .iter()
        .map(|register| {
            let class = physical
                .model()
                .classes
                .get(usize::from(register.class.0))
                .filter(|class| class.id == register.class)
                .ok_or(AllocationLegalityError::UnknownClass {
                    function: function_index,
                    register: register.virtual_register.0,
                    class: register.class.0,
                })?;
            let entry_point = register
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
                for raw_point in fragment.start.0..fragment.end.0 {
                    let point = LiveRangePoint(raw_point);
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
                            return Err(AllocationLegalityError::UnknownFixedView {
                                function: function_index,
                                register: register.virtual_register.0,
                                view: fixed.0,
                            });
                        };
                        if view.id != fixed || view.class != register.class {
                            return Err(AllocationLegalityError::UnknownFixedView {
                                function: function_index,
                                register: register.virtual_register.0,
                                view: fixed.0,
                            });
                        }
                        if view_conflicts(view, fragment.block, point, function, reservations) {
                            return Err(AllocationLegalityError::IllegalFixedView {
                                function: function_index,
                                register: register.virtual_register.0,
                                view: fixed.0,
                            });
                        }
                        candidates.clear();
                        candidates.push(fixed);
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
                        return Err(AllocationLegalityError::UnknownFixedView {
                            function: function_index,
                            register: register.virtual_register.0,
                            view: fixed.0,
                        });
                    };
                    if view.id != fixed || view.class != register.class {
                        return Err(AllocationLegalityError::UnknownFixedView {
                            function: function_index,
                            register: register.virtual_register.0,
                            view: fixed.0,
                        });
                    }
                    if view_conflicts(view, early.block, early.early_point, function, reservations)
                    {
                        return Err(AllocationLegalityError::IllegalFixedView {
                            function: function_index,
                            register: register.virtual_register.0,
                            view: fixed.0,
                        });
                    }
                    candidates.clear();
                    candidates.push(fixed);
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
            let entry_transitions = entry_transitions(register);
            Ok(VirtualRegisterAllocationLegality {
                virtual_register: register.virtual_register,
                class: register.class,
                points,
                early_clobber_points,
                entry_transitions,
            })
        })
        .collect::<Result<Vec<_>, AllocationLegalityError>>()?;
    Ok(FunctionAllocationLegality {
        machine: function.machine,
        virtual_registers,
    })
}

fn fixed_view_for_early_clobber(
    function_index: usize,
    register: &crate::VirtualLiveRange,
    early: &crate::EarlyClobberConstraint,
) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
    let fixed = register
        .fixed_constraints
        .iter()
        .filter_map(|constraint| match constraint.site {
            VirtualFixedConstraintSite::Operand {
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
        return Err(AllocationLegalityError::IllegalFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.last().expect("two fixed views exist").0,
        });
    }
    Ok(fixed.into_iter().next())
}

fn fixed_view_at_point(
    function_index: usize,
    register: &crate::VirtualLiveRange,
    block: omega_selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    entry_point: Option<(omega_selected_instructions::SelectedBlockId, LiveRangePoint)>,
) -> Result<Option<RegisterViewId>, AllocationLegalityError> {
    let mut fixed = BTreeSet::new();
    for constraint in &register.fixed_constraints {
        let applies = match constraint.site {
            VirtualFixedConstraintSite::Entry => entry_point == Some((block, point)),
            VirtualFixedConstraintSite::Operand {
                point: constraint_point,
                ..
            } => constraint_point == point,
        };
        if applies {
            fixed.insert(constraint.view);
        }
    }
    if fixed.len() > 1 {
        return Err(AllocationLegalityError::IllegalFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.last().expect("two fixed views exist").0,
        });
    }
    Ok(fixed.into_iter().next())
}

fn entry_transitions(register: &crate::VirtualLiveRange) -> Vec<EntryFixedViewTransition> {
    let Some(entry) = register
        .fixed_constraints
        .iter()
        .find(|constraint| matches!(constraint.site, VirtualFixedConstraintSite::Entry))
    else {
        return Vec::new();
    };
    register
        .fixed_constraints
        .iter()
        .filter(|constraint| matches!(constraint.site, VirtualFixedConstraintSite::Operand { .. }))
        .filter(|constraint| constraint.view != entry.view)
        .map(|constraint| EntryFixedViewTransition {
            from_view: entry.view,
            to_site: constraint.site,
            to_view: constraint.view,
        })
        .collect()
}

fn view_conflicts(
    view: &RegisterView,
    block: omega_selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    function: &FunctionLiveRanges,
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
    use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
    use omega_target::{Architecture, NativeTarget, ObjectFormat};
    use psi_core::MachineId;

    use super::compute_function;
    use crate::{
        AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
        AllocatorAvailabilityValidationReceipt, DistinctUseDefTie, EarlyClobberConstraint,
        EarlyClobberUse, FunctionLiveRanges, LiveRangeFragment, LiveRangePoint, LivenessPosition,
        RegisterClassAvailability, ValidatedAllocatorAvailability, VirtualLiveRange,
        allocator_availability_identity,
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
    ) -> ValidatedAllocatorAvailability {
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes([1; 32]);
        let plan = AllocatorAvailabilityPlan {
            register_environment,
            physical: physical.identity(),
            policy: AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
            classes: vec![RegisterClassAvailability {
                class: RegisterClassId(0),
                unconstrained_views: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
        };
        let receipt = AllocatorAvailabilityValidationReceipt {
            identity: allocator_availability_identity(&plan),
            register_environment,
            physical: physical.identity(),
            class_count: 1,
            unconstrained_view_count: 2,
        };
        ValidatedAllocatorAvailability { plan, receipt }
    }

    fn range(register: u32, start: u32, end: u32) -> VirtualLiveRange {
        VirtualLiveRange {
            virtual_register: VirtualRegisterId(register),
            class: RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments: vec![LiveRangeFragment {
                block: SelectedBlockId(0),
                start: LiveRangePoint(start),
                end: LiveRangePoint(end),
            }],
            edge_connectors: Vec::new(),
        }
    }

    fn early(position: u32, used: u32, defined: u32) -> EarlyClobberConstraint {
        EarlyClobberConstraint {
            block: SelectedBlockId(0),
            position: LivenessPosition(position),
            instruction: SelectedInstructionId(position),
            early_point: LiveRangePoint(position * 2),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(defined),
            def_class: RegisterClassId(0),
            def_point: LiveRangePoint(position * 2 + 1),
            uses: vec![EarlyClobberUse {
                operand: 0,
                virtual_register: VirtualRegisterId(used),
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
        let ranges = FunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: vec![range(0, 0, 1), range(1, 1, 3), range(2, 3, 4)],
            tied_pairs: Vec::<DistinctUseDefTie>::new(),
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
            LiveRangePoint(0)
        );
        assert_eq!(
            legality.virtual_registers[2].early_clobber_points[0].point,
            LiveRangePoint(2)
        );
        assert_eq!(
            legality.virtual_registers[2].early_clobber_points[0].candidates,
            vec![RegisterViewId(0), RegisterViewId(1)]
        );
    }

    #[test]
    fn computes_before_phase_legality_for_early_definition_in_tied_component() {
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
        let ranges = FunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: vec![
                range(0, 0, 1),
                range(1, 1, 3),
                range(2, 2, 3),
                range(3, 3, 4),
            ],
            tied_pairs: vec![
                DistinctUseDefTie {
                    block: SelectedBlockId(0),
                    position: LivenessPosition(0),
                    instruction: SelectedInstructionId(0),
                    use_operand: 0,
                    use_virtual_register: VirtualRegisterId(0),
                    use_point: LiveRangePoint(0),
                    def_operand: 1,
                    def_virtual_register: VirtualRegisterId(1),
                    def_point: LiveRangePoint(1),
                    class: RegisterClassId(0),
                },
                DistinctUseDefTie {
                    block: SelectedBlockId(0),
                    position: LivenessPosition(1),
                    instruction: SelectedInstructionId(1),
                    use_operand: 0,
                    use_virtual_register: VirtualRegisterId(1),
                    use_point: LiveRangePoint(2),
                    def_operand: 2,
                    def_virtual_register: VirtualRegisterId(3),
                    def_point: LiveRangePoint(3),
                    class: RegisterClassId(0),
                },
            ],
            early_clobbers: vec![EarlyClobberConstraint {
                block: SelectedBlockId(0),
                position: LivenessPosition(1),
                instruction: SelectedInstructionId(1),
                early_point: LiveRangePoint(2),
                def_operand: 2,
                def_virtual_register: VirtualRegisterId(3),
                def_class: RegisterClassId(0),
                def_point: LiveRangePoint(3),
                uses: vec![EarlyClobberUse {
                    operand: 1,
                    virtual_register: VirtualRegisterId(2),
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
            &ranges.virtual_registers[3],
            &availability,
            &physical,
            &reservations,
        )
        .unwrap();
        assert_eq!(legality.virtual_registers[3], replayed);
        assert_eq!(
            legality.virtual_registers[3].early_clobber_points[0].candidates,
            vec![RegisterViewId(0), RegisterViewId(1)]
        );
    }
}

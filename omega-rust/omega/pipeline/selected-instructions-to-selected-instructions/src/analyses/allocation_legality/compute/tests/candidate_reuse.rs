use super::*;
use crate::analyses::allocation_legality::validate::{
    replay_function_for_test, replay_register_for_test,
};
use crate::{
    AllocationLegalityError, ArchitecturalUnitAction, ArchitecturalUnitActionKind,
    ArchitecturalUnitLiveRange, FunctionAllocationLegality, VirtualFixedConstraint,
    VirtualFixedConstraintSite,
};
use register_model::{RegisterReservationOverlay, ReservationReason};

fn environment() -> register_model::ValidatedPhysicalRegisterModel {
    let mut model = physical().model().clone();
    for unit in 2..4 {
        model.units.push(RegisterUnit {
            id: RegisterUnitId(unit),
            name: format!("extra{unit}"),
            bits: 64,
            kind: RegisterUnitKind::IntegerLane,
        });
    }
    model.views[0].write_units.push(RegisterUnitId(2));
    model.views[0].write_semantics = RegisterWriteSemantics::InstructionDefined;
    for (view, class, unit, allocatable) in [(2, 0, 0, false), (3, 1, 3, true), (4, 0, 0, true)] {
        model.views.push(RegisterView {
            id: RegisterViewId(view),
            name: format!("extra{view}"),
            class: RegisterClassId(class),
            units: vec![RegisterUnitId(unit)],
            write_units: vec![RegisterUnitId(unit)],
            bits: 64,
            write_semantics: RegisterWriteSemantics::ExactView,
            allocatable,
        });
    }
    model.classes[0]
        .views
        .extend([RegisterViewId(2), RegisterViewId(4)]);
    model.classes.push(RegisterClass {
        id: RegisterClassId(1),
        name: "other".into(),
        views: vec![RegisterViewId(3)],
    });
    model.reservations.push(RegisterReservationOverlay {
        name: "reserve-one".into(),
        reason: ReservationReason::Backend,
        units: vec![RegisterUnitId(1)],
    });
    validate_physical_register_model(model).unwrap()
}

fn available(
    physical: &register_model::ValidatedPhysicalRegisterModel,
) -> ValidatedAllocatorAvailability {
    let mut available = availability(physical);
    available.plan.classes.push(RegisterClassAvailability {
        class: RegisterClassId(1),
        unconstrained_views: vec![RegisterViewId(3)],
    });
    available.receipt.identity = allocator_availability_identity(&available.plan);
    available.receipt.class_count = 2;
    available.receipt.unconstrained_view_count = 3;
    available
}

fn reservations(
    physical: &register_model::ValidatedPhysicalRegisterModel,
    reserve: bool,
) -> register_model::ValidatedRegisterReservationProfile {
    validate_register_reservation_profile(
        RegisterReservationProfile {
            name: "test".into(),
            active_overlays: if reserve {
                vec!["reserve-one".into()]
            } else {
                Vec::new()
            },
        },
        NativeTarget {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        },
        physical,
    )
    .unwrap()
}

fn function_ranges() -> FunctionLiveRanges {
    let mut disjoint_block = range(2, 0, 2);
    disjoint_block.fragments[0].block = SelectedBlockId(7);
    disjoint_block.fragments.push(LiveRangeFragment {
        block: SelectedBlockId(7),
        start: LiveRangePoint(4),
        end: LiveRangePoint(6),
    });
    let mut other_class = range(3, 0, 4);
    other_class.class = RegisterClassId(1);
    let mut empty = range(4, 0, 0);
    empty.fragments.clear();
    FunctionLiveRanges {
        machine: MachineId::new(1).unwrap(),
        // These kernel fixtures intentionally have no precomputed point domains.
        block_domains: Vec::new(),
        virtual_registers: vec![
            range(0, 0, 4),
            range(1, 1, 3),
            disjoint_block,
            other_class,
            empty,
            range(5, 1, 2),
        ],
        tied_pairs: Vec::new(),
        edge_transfers: Vec::new(),
        copy_affinities: Vec::new(),
        early_clobbers: vec![early(0, 0, 5)],
        architectural_units: Vec::new(),
        interference: Vec::new(),
    }
}

fn occupy(
    function: &mut FunctionLiveRanges,
    unit: u16,
    block: u32,
    start: u32,
    end: u32,
    action: bool,
) {
    let mut row = ArchitecturalUnitLiveRange {
        unit: RegisterUnitId(unit),
        actions: Vec::new(),
        fragments: Vec::new(),
        edge_connectors: Vec::new(),
    };
    if action {
        row.actions.push(ArchitecturalUnitAction {
            block: SelectedBlockId(block),
            position: LivenessPosition(start / 2),
            point: LiveRangePoint(start),
            instruction: SelectedInstructionId(start / 2),
            kind: ArchitecturalUnitActionKind::Clobber,
        });
    } else {
        row.fragments.push(LiveRangeFragment {
            block: SelectedBlockId(block),
            start: LiveRangePoint(start),
            end: LiveRangePoint(end),
        });
    }
    function.architectural_units.push(row);
}

fn fixed(view: u16) -> VirtualFixedConstraint {
    VirtualFixedConstraint {
        site: VirtualFixedConstraintSite::Entry,
        view: RegisterViewId(view),
    }
}

fn compare(
    function: &FunctionLiveRanges,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    available: &ValidatedAllocatorAvailability,
    reservations: &register_model::ValidatedRegisterReservationProfile,
) -> Result<FunctionAllocationLegality, AllocationLegalityError> {
    let expected = function
        .virtual_registers
        .iter()
        .map(|register| {
            replay_register_for_test(0, function, register, available, physical, reservations)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|virtual_registers| FunctionAllocationLegality {
            machine: function.machine,
            virtual_registers,
        });
    assert_eq!(
        replay_function_for_test(0, function, available, physical, reservations),
        expected
    );
    assert_eq!(
        super::function::compute(0, function, available, physical, reservations),
        expected
    );
    expected
}

#[test]
fn reused_candidates_match_uncached_scan_across_locations_and_constraints() {
    let physical = environment();
    let available = available(&physical);
    for occupancy in 0..8 {
        for reserve in [false, true] {
            for fixed_view in [None, Some(1), Some(2), Some(4), Some(99)] {
                let mut function = function_ranges();
                if occupancy & 1 != 0 {
                    occupy(&mut function, 0, 0, 1, 3, false);
                }
                if occupancy & 2 != 0 {
                    occupy(&mut function, 2, 0, 0, 1, true);
                }
                if occupancy & 4 != 0 {
                    occupy(&mut function, 1, 7, 0, 1, true);
                }
                if let Some(view) = fixed_view {
                    function.virtual_registers[1]
                        .fixed_constraints
                        .push(fixed(view));
                }
                let _ = compare(
                    &function,
                    &physical,
                    &available,
                    &reservations(&physical, reserve),
                );
            }
        }
    }
}

#[test]
fn occupancy_keeps_half_open_boundaries_write_aliases_and_block_identity() {
    let physical = environment();
    let available = available(&physical);
    let mut function = function_ranges();
    occupy(&mut function, 0, 0, 1, 3, false);
    occupy(&mut function, 2, 0, 0, 1, true);
    let result = compare(
        &function,
        &physical,
        &available,
        &reservations(&physical, false),
    )
    .unwrap();
    let points = &result.virtual_registers[0].points;
    assert_eq!(points[0].candidates, vec![RegisterViewId(1)]);
    assert_eq!(points[1].candidates, vec![RegisterViewId(1)]);
    assert_eq!(points[2].candidates, vec![RegisterViewId(1)]);
    assert_eq!(
        points[3].candidates,
        vec![RegisterViewId(0), RegisterViewId(1)]
    );
    assert_eq!(
        result.virtual_registers[2].points[0].candidates,
        vec![RegisterViewId(0), RegisterViewId(1)]
    );
    assert_eq!(
        result.virtual_registers[3].points[0].candidates,
        vec![RegisterViewId(3)]
    );
    assert!(result.virtual_registers[4].points.is_empty());
    assert_eq!(
        result.virtual_registers[5].early_clobber_points[0].candidates,
        vec![RegisterViewId(1)]
    );
}

#[test]
fn special_fixed_views_and_competing_errors_keep_occurrence_order() {
    let physical = environment();
    let available = available(&physical);
    let reservations = reservations(&physical, false);
    for view in [2, 4] {
        let mut function = function_ranges();
        function.virtual_registers[1]
            .fixed_constraints
            .push(fixed(view));
        let result = compare(&function, &physical, &available, &reservations).unwrap();
        assert_eq!(
            result.virtual_registers[1].points[0].candidates,
            vec![RegisterViewId(view)]
        );
    }
    let mut function = function_ranges();
    function.virtual_registers[1]
        .fixed_constraints
        .push(fixed(99));
    function.virtual_registers[3].class = RegisterClassId(99);
    assert_eq!(
        compare(&function, &physical, &available, &reservations),
        Err(AllocationLegalityError::UnknownFixedView {
            function: 0,
            register: 1,
            view: 99
        })
    );
    function.virtual_registers.swap(1, 3);
    assert_eq!(
        compare(&function, &physical, &available, &reservations),
        Err(AllocationLegalityError::UnknownClass {
            function: 0,
            register: 3,
            class: 99
        })
    );
    let mut function = function_ranges();
    function.virtual_registers[1]
        .fixed_constraints
        .extend([fixed(0), fixed(1)]);
    assert_eq!(
        compare(&function, &physical, &available, &reservations),
        Err(AllocationLegalityError::IllegalFixedView {
            function: 0,
            register: 1,
            view: 1
        })
    );
}

#[test]
fn wrong_fixed_class_retains_the_existing_producer_and_replay_errors() {
    let physical = environment();
    let available = available(&physical);
    let reservations = reservations(&physical, false);
    let mut function = function_ranges();
    function.virtual_registers[1]
        .fixed_constraints
        .push(fixed(3));
    let expected = AllocationLegalityError::IllegalFixedView {
        function: 0,
        register: 1,
        view: 3,
    };
    assert_eq!(
        replay_register_for_test(
            0,
            &function,
            &function.virtual_registers[1],
            &available,
            &physical,
            &reservations
        )
        .unwrap_err(),
        expected
    );
    assert_eq!(
        replay_function_for_test(0, &function, &available, &physical, &reservations).unwrap_err(),
        expected
    );
    assert_eq!(
        super::function::compute(0, &function, &available, &physical, &reservations),
        Err(AllocationLegalityError::UnknownFixedView {
            function: 0,
            register: 1,
            view: 3
        })
    );
}

#[test]
fn early_fixed_views_recheck_occupancy_outside_general_candidates() {
    let physical = environment();
    let available = available(&physical);
    let reservations = reservations(&physical, false);
    for view in [2, 4] {
        let mut function = function_ranges();
        function.virtual_registers[5]
            .fixed_constraints
            .push(VirtualFixedConstraint {
                site: VirtualFixedConstraintSite::Operand {
                    position: LivenessPosition(0),
                    point: LiveRangePoint(1),
                    instruction: SelectedInstructionId(0),
                    operand: 1,
                    access: register_model::RegisterOperandAccess::Def,
                },
                view: RegisterViewId(view),
            });
        let result = compare(&function, &physical, &available, &reservations).unwrap();
        assert_eq!(
            result.virtual_registers[5].early_clobber_points[0].candidates,
            vec![RegisterViewId(view)]
        );
        // The ordinary after point remains free, but the early before point is occupied.
        occupy(&mut function, 0, 0, 0, 1, true);
        assert_eq!(
            compare(&function, &physical, &available, &reservations),
            Err(AllocationLegalityError::IllegalFixedView {
                function: 0,
                register: 5,
                view
            })
        );
    }
}

#[test]
fn prepared_rows_follow_unique_class_locations_not_register_visits() {
    let physical = environment();
    let available = available(&physical);
    let reservations = reservations(&physical, false);
    let function = function_ranges();
    let mut prepared =
        super::super::view_candidates::CandidateViews::new(&function, &physical, &reservations);
    for _visit in 0..32 {
        for (class, block, point) in [(0, 0, 0), (0, 0, 1), (0, 7, 0), (1, 0, 0)] {
            let _ = prepared.unconstrained(
                &physical.model().classes[class],
                available
                    .unconstrained_views(RegisterClassId(class as u16))
                    .unwrap(),
                SelectedBlockId(block),
                LiveRangePoint(point),
            );
        }
    }
    assert_eq!(prepared.prepared_row_count(), 4);
}

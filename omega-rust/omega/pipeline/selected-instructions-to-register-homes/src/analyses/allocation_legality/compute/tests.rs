use register_model::{
    PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterReservationProfile,
    RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId,
    RegisterWriteSemantics, TargetRegisterEnvironmentIdentity, validate_physical_register_model,
    validate_register_reservation_profile,
};
use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget, ObjectFormat};

use super::function;
use crate::{
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, AllocatorAvailabilityValidationReceipt,
    DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, FunctionLiveRanges,
    LiveRangeFragment, LiveRangePoint, LivenessPosition, RegisterClassAvailability,
    ValidatedAllocatorAvailability, VirtualLiveRange, allocator_availability_identity,
};

fn physical() -> register_model::ValidatedPhysicalRegisterModel {
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
    physical: &register_model::ValidatedPhysicalRegisterModel,
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
    let legality = function::compute(0, &ranges, &availability, &physical, &reservations).unwrap();
    let replayed = ranges
        .virtual_registers
        .iter()
        .map(|register| {
            crate::analyses::allocation_legality::validate::replay_register_for_test(
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
    let legality = function::compute(0, &ranges, &availability, &physical, &reservations).unwrap();
    let replayed = crate::analyses::allocation_legality::validate::replay_register_for_test(
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

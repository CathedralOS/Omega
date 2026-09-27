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
use crate::{AllocatorAvailabilityValidationReceipt, ValidatedAllocatorAvailability};
use register_homes::{
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, RegisterClassAvailability,
    allocator_availability_identity,
};
use selected_instructions::{
    EarlyClobberConstraint, EarlyClobberUse, FunctionLiveRanges, LiveRangeFragment, LiveRangePoint,
    LivenessPosition, VirtualLiveRange,
};

mod candidate_reuse;

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

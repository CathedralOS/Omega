use register_model::{
    PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId,
    RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use semantic_vocabulary::MachineId;

use crate::{
    DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, FunctionAllocationLegality,
    FunctionLiveRanges, LiveRangePoint, LivenessPosition, VirtualLiveRange, VirtualPointLegality,
    VirtualRegisterAllocationLegality,
};

pub(super) fn physical() -> ValidatedPhysicalRegisterModel {
    physical_with_views(vec![register_view(0, 0, "r0"), register_view(1, 1, "r1")])
}

pub(super) fn aliased_physical() -> ValidatedPhysicalRegisterModel {
    physical_with_views(vec![
        register_view(0, 0, "r0"),
        register_view(1, 0, "r0.alias"),
        register_view(2, 1, "r1"),
    ])
}

fn physical_with_views(views: Vec<RegisterView>) -> ValidatedPhysicalRegisterModel {
    let class_views = views.iter().map(|view| view.id).collect();
    validate_physical_register_model(PhysicalRegisterModel {
        architecture: target::Architecture::X86_64,
        units: (0..2)
            .map(|index| RegisterUnit {
                id: RegisterUnitId(index),
                name: format!("r{index}.storage"),
                bits: 64,
                kind: RegisterUnitKind::IntegerLane,
            })
            .collect(),
        views,
        classes: vec![RegisterClass {
            id: RegisterClassId(0),
            name: "integer".into(),
            views: class_views,
        }],
        conventions: Vec::new(),
        reservations: Vec::new(),
    })
    .unwrap()
}

fn register_view(id: u16, unit: u16, name: &str) -> RegisterView {
    RegisterView {
        id: RegisterViewId(id),
        name: name.into(),
        class: RegisterClassId(0),
        units: vec![RegisterUnitId(unit)],
        write_units: vec![RegisterUnitId(unit)],
        bits: 64,
        write_semantics: RegisterWriteSemantics::ExactView,
        allocatable: true,
    }
}

pub(super) fn legality(points: &[(u32, u32)]) -> FunctionAllocationLegality {
    FunctionAllocationLegality {
        machine: MachineId::new(1).unwrap(),
        virtual_registers: points
            .iter()
            .enumerate()
            .map(
                |(register, (start, end))| VirtualRegisterAllocationLegality {
                    virtual_register: VirtualRegisterId(register as u32),
                    class: RegisterClassId(0),
                    points: (*start..=*end)
                        .map(|point| VirtualPointLegality {
                            block: SelectedBlockId(0),
                            point: LiveRangePoint(point),
                            candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                        })
                        .collect(),
                    early_clobber_points: Vec::new(),
                    entry_transitions: Vec::new(),
                },
            )
            .collect(),
    }
}

pub(super) fn set_candidates(
    legality: &mut FunctionAllocationLegality,
    register: usize,
    candidates: &[u16],
) {
    let candidates = candidates
        .iter()
        .copied()
        .map(RegisterViewId)
        .collect::<Vec<_>>();
    for point in &mut legality.virtual_registers[register].points {
        point.candidates.clone_from(&candidates);
    }
    for point in &mut legality.virtual_registers[register].early_clobber_points {
        point.candidates.clone_from(&candidates);
    }
}

pub(super) fn ranges(register_count: u32, interference: &[(u32, u32)]) -> FunctionLiveRanges {
    FunctionLiveRanges {
        machine: MachineId::new(1).unwrap(),
        block_domains: Vec::new(),
        virtual_registers: (0..register_count)
            .map(|register| VirtualLiveRange {
                virtual_register: VirtualRegisterId(register),
                class: RegisterClassId(0),
                occurrences: Vec::new(),
                fixed_constraints: Vec::new(),
                fragments: Vec::new(),
                edge_connectors: Vec::new(),
            })
            .collect(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units: Vec::new(),
        interference: interference
            .iter()
            .map(|(lower, higher)| crate::VirtualInterference {
                lower: VirtualRegisterId(*lower),
                higher: VirtualRegisterId(*higher),
            })
            .collect(),
    }
}

pub(super) fn tied_ranges(interference: &[(u32, u32)]) -> FunctionLiveRanges {
    let mut ranges = ranges(3, interference);
    ranges.tied_pairs.push(DistinctUseDefTie {
        block: SelectedBlockId(0),
        position: LivenessPosition(1),
        instruction: SelectedInstructionId(1),
        use_operand: 0,
        use_virtual_register: VirtualRegisterId(0),
        use_point: LiveRangePoint(2),
        def_operand: 1,
        def_virtual_register: VirtualRegisterId(1),
        def_point: LiveRangePoint(3),
        class: RegisterClassId(0),
    });
    ranges
}

pub(super) fn tied_component_ranges(interference: &[(u32, u32)]) -> FunctionLiveRanges {
    let mut ranges = tied_ranges(interference);
    ranges.tied_pairs.push(DistinctUseDefTie {
        block: SelectedBlockId(0),
        position: LivenessPosition(2),
        instruction: SelectedInstructionId(2),
        use_operand: 0,
        use_virtual_register: VirtualRegisterId(1),
        use_point: LiveRangePoint(4),
        def_operand: 1,
        def_virtual_register: VirtualRegisterId(2),
        def_point: LiveRangePoint(5),
        class: RegisterClassId(0),
    });
    ranges
}

pub(super) fn early_clobber_ranges() -> FunctionLiveRanges {
    let mut ranges = ranges(3, &[]);
    ranges.early_clobbers.push(EarlyClobberConstraint {
        block: SelectedBlockId(0),
        position: LivenessPosition(1),
        instruction: SelectedInstructionId(1),
        early_point: LiveRangePoint(2),
        def_operand: 2,
        def_virtual_register: VirtualRegisterId(2),
        def_class: RegisterClassId(0),
        def_point: LiveRangePoint(3),
        uses: vec![
            EarlyClobberUse {
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                class: RegisterClassId(0),
            },
            EarlyClobberUse {
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                class: RegisterClassId(0),
            },
        ],
    });
    ranges
}

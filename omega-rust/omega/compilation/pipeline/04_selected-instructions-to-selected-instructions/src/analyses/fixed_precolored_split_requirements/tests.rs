//! Split-requirement fixtures for the early-clobber admission boundary.
//!
//! The fixture mirrors the x86 divide-into-shift-count customer: one register
//! is defined through a pinned view (the remainder's `rax` result), then used
//! as the pinned `rcx` shift count, while a sibling register is the shift's
//! early-clobber definition. Both registers previously died at admission as
//! early-clobber participants even though no `early_clobber_points` candidate
//! set enters a source domain.
use super::{FixedPrecoloredSplitRequirementError, analyze_fixed_precolored_split_requirements};
use crate::rewrites::test_support::budget;
use crate::{
    AllocationLegalityValidationReceipt, FixedPrecoloredIntervalValidationReceipt,
    LiveRangeValidationReceipt, ValidatedAllocationLegality, ValidatedFixedPrecoloredIntervals,
    ValidatedLiveRanges,
};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityPlan, AllocatorAvailabilityIdentity, EntryFixedViewTransition,
    FixedPrecoloredInterval, FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPolicy,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPolicy,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals, VirtualEarlyClobberPointLegality,
    VirtualPointLegality, VirtualRegisterAllocationLegality, allocation_legality_identity,
    fixed_precolored_interval_plan_identity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterViewId};
use selected_instructions::{
    DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, FunctionLiveRanges, LiveRangePlan,
    LiveRangePoint, LivenessIdentity, LivenessPosition, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, VirtualFixedConstraint, VirtualFixedConstraintSite,
    VirtualLiveRange, VirtualOccurrence, VirtualRegisterId, live_range_identity,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use std::sync::Arc;
use target::NativeTarget;

fn machine() -> MachineId {
    MachineId::new(1).unwrap()
}
const CLASS: RegisterClassId = RegisterClassId(0);
/// The pinned source view (the divide/remainder `rax` result home).
const SOURCE_VIEW: RegisterViewId = RegisterViewId(0);
/// The pinned destination view (the `rcx` shift-count operand).
const DESTINATION_VIEW: RegisterViewId = RegisterViewId(1);
const DIVIDE_RESULT: VirtualRegisterId = VirtualRegisterId(0);
const SHIFT_RESULT: VirtualRegisterId = VirtualRegisterId(1);
const SHIFT_VALUE: VirtualRegisterId = VirtualRegisterId(2);

fn divide_definition_site() -> VirtualFixedConstraintSite {
    VirtualFixedConstraintSite::Operand {
        position: LivenessPosition(3),
        point: LiveRangePoint(2),
        instruction: SelectedInstructionId(0),
        operand: 2,
        access: RegisterOperandAccess::Def,
    }
}

fn shift_count_site() -> VirtualFixedConstraintSite {
    VirtualFixedConstraintSite::Operand {
        position: LivenessPosition(6),
        point: LiveRangePoint(6),
        instruction: SelectedInstructionId(1),
        operand: 1,
        access: RegisterOperandAccess::Use,
    }
}

fn point(point: u32, candidates: &[u16]) -> VirtualPointLegality {
    VirtualPointLegality {
        block: SelectedBlockId(0),
        point: LiveRangePoint(point),
        candidates: candidates
            .iter()
            .map(|&view| RegisterViewId(view))
            .collect(),
    }
}

fn ranges() -> FunctionLiveRanges {
    FunctionLiveRanges {
        machine: machine(),
        block_domains: Vec::new(),
        virtual_registers: vec![
            VirtualLiveRange {
                virtual_register: DIVIDE_RESULT,
                class: CLASS,
                occurrences: vec![
                    VirtualOccurrence {
                        position: LivenessPosition(3),
                        point: LiveRangePoint(2),
                        instruction: SelectedInstructionId(0),
                        operand: 2,
                        access: RegisterOperandAccess::Def,
                    },
                    VirtualOccurrence {
                        position: LivenessPosition(6),
                        point: LiveRangePoint(6),
                        instruction: SelectedInstructionId(1),
                        operand: 1,
                        access: RegisterOperandAccess::Use,
                    },
                ],
                fixed_constraints: vec![
                    VirtualFixedConstraint {
                        site: divide_definition_site(),
                        view: SOURCE_VIEW,
                    },
                    VirtualFixedConstraint {
                        site: shift_count_site(),
                        view: DESTINATION_VIEW,
                    },
                ],
                fragments: vec![selected_instructions::LiveRangeFragment {
                    block: SelectedBlockId(0),
                    start: LiveRangePoint(2),
                    end: LiveRangePoint(7),
                }],
                edge_connectors: Vec::new(),
            },
            VirtualLiveRange {
                virtual_register: SHIFT_RESULT,
                class: CLASS,
                occurrences: vec![VirtualOccurrence {
                    position: LivenessPosition(7),
                    point: LiveRangePoint(8),
                    instruction: SelectedInstructionId(1),
                    operand: 2,
                    access: RegisterOperandAccess::Def,
                }],
                fixed_constraints: Vec::new(),
                fragments: vec![selected_instructions::LiveRangeFragment {
                    block: SelectedBlockId(0),
                    start: LiveRangePoint(8),
                    end: LiveRangePoint(10),
                }],
                edge_connectors: Vec::new(),
            },
            VirtualLiveRange {
                virtual_register: SHIFT_VALUE,
                class: CLASS,
                occurrences: vec![VirtualOccurrence {
                    position: LivenessPosition(6),
                    point: LiveRangePoint(6),
                    instruction: SelectedInstructionId(1),
                    operand: 0,
                    access: RegisterOperandAccess::Use,
                }],
                fixed_constraints: Vec::new(),
                fragments: vec![selected_instructions::LiveRangeFragment {
                    block: SelectedBlockId(0),
                    start: LiveRangePoint(4),
                    end: LiveRangePoint(10),
                }],
                edge_connectors: Vec::new(),
            },
        ],
        tied_pairs: Vec::new(),
        edge_transfers: Vec::new(),
        copy_affinities: Vec::new(),
        early_clobbers: vec![EarlyClobberConstraint {
            block: SelectedBlockId(0),
            position: LivenessPosition(6),
            instruction: SelectedInstructionId(1),
            early_point: LiveRangePoint(6),
            def_operand: 2,
            def_virtual_register: SHIFT_RESULT,
            def_class: CLASS,
            def_point: LiveRangePoint(8),
            uses: vec![
                EarlyClobberUse {
                    operand: 0,
                    virtual_register: SHIFT_VALUE,
                    class: CLASS,
                },
                EarlyClobberUse {
                    operand: 1,
                    virtual_register: DIVIDE_RESULT,
                    class: CLASS,
                },
            ],
        }],
        architectural_units: Vec::new(),
        interference: Vec::new(),
    }
}

fn legality() -> FunctionAllocationLegality {
    FunctionAllocationLegality {
        machine: machine(),
        virtual_registers: vec![
            VirtualRegisterAllocationLegality {
                virtual_register: DIVIDE_RESULT,
                class: CLASS,
                // The result stays in the pinned divide view until the shift
                // count's point domain forces the `rcx` view alone.
                points: vec![
                    point(2, &[0]),
                    point(3, &[0]),
                    point(4, &[0]),
                    point(5, &[0]),
                    point(6, &[1]),
                ],
                early_clobber_points: vec![VirtualEarlyClobberPointLegality {
                    block: SelectedBlockId(0),
                    position: LivenessPosition(6),
                    instruction: SelectedInstructionId(1),
                    operand: 1,
                    point: LiveRangePoint(6),
                    candidates: vec![DESTINATION_VIEW],
                }],
                entry_transitions: vec![EntryFixedViewTransition {
                    from_view: SOURCE_VIEW,
                    to_site: shift_count_site(),
                    to_view: DESTINATION_VIEW,
                }],
            },
            VirtualRegisterAllocationLegality {
                virtual_register: SHIFT_RESULT,
                class: CLASS,
                points: vec![point(8, &[0, 1]), point(9, &[0, 1])],
                early_clobber_points: vec![VirtualEarlyClobberPointLegality {
                    block: SelectedBlockId(0),
                    position: LivenessPosition(6),
                    instruction: SelectedInstructionId(1),
                    operand: 2,
                    point: LiveRangePoint(6),
                    candidates: vec![SOURCE_VIEW, DESTINATION_VIEW],
                }],
                entry_transitions: Vec::new(),
            },
            VirtualRegisterAllocationLegality {
                virtual_register: SHIFT_VALUE,
                class: CLASS,
                points: vec![
                    point(4, &[0, 1]),
                    point(5, &[0, 1]),
                    point(6, &[0, 1]),
                    point(7, &[0, 1]),
                    point(8, &[0, 1]),
                    point(9, &[0, 1]),
                ],
                early_clobber_points: Vec::new(),
                entry_transitions: Vec::new(),
            },
        ],
    }
}

fn fixed() -> FunctionFixedPrecoloredIntervals {
    FunctionFixedPrecoloredIntervals {
        machine: machine(),
        intervals: vec![
            FixedPrecoloredInterval {
                virtual_register: DIVIDE_RESULT,
                class: CLASS,
                site: divide_definition_site(),
                block: SelectedBlockId(0),
                start: LiveRangePoint(2),
                end: LiveRangePoint(3),
                view: SOURCE_VIEW,
            },
            FixedPrecoloredInterval {
                virtual_register: DIVIDE_RESULT,
                class: CLASS,
                site: shift_count_site(),
                block: SelectedBlockId(0),
                start: LiveRangePoint(6),
                end: LiveRangePoint(7),
                view: DESTINATION_VIEW,
            },
        ],
    }
}

fn validated(
    ranges: FunctionLiveRanges,
    legality: FunctionAllocationLegality,
    fixed: FunctionFixedPrecoloredIntervals,
) -> (
    ValidatedLiveRanges,
    ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals,
) {
    let selected = SelectedInstructionPlanIdentity::from_bytes([7; 32]);
    let liveness = LivenessIdentity::from_bytes([8; 32]);
    let optimization_unit = OptimizationUnitIdentity::from_bytes([9; 32]);
    let fuel_schedule = FuelScheduleIdentity::new(1).unwrap();
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes([3; 32]);
    let allocator_availability = AllocatorAvailabilityIdentity::from_bytes([4; 32]);
    let ranges_plan = LiveRangePlan {
        selected,
        liveness,
        optimization_unit,
        fuel_schedule,
        target: NativeTarget::linux_x64(),
        functions: vec![ranges],
    };
    let ranges_id = live_range_identity(&ranges_plan);
    let legality_plan = AllocationLegalityPlan {
        ranges: ranges_id,
        register_environment,
        allocator_availability,
        functions: vec![legality],
    };
    let legality_id = allocation_legality_identity(&legality_plan);
    let fixed_plan = FixedPrecoloredIntervalPlan {
        ranges: ranges_id,
        legality: legality_id,
        register_environment,
        allocator_availability,
        optimization_unit,
        fuel_schedule,
        policy: FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
        budget: budget(),
        usage: OptimizationWorkUsage::default(),
        functions: vec![fixed],
    };
    let fixed_id = fixed_precolored_interval_plan_identity(&fixed_plan);
    (
        ValidatedLiveRanges {
            plan: Arc::new(ranges_plan),
            receipt: LiveRangeValidationReceipt {
                identity: ranges_id,
                selected,
                liveness,
                optimization_unit,
                fuel_schedule,
                function_count: 1,
                block_count: 1,
                virtual_register_count: 3,
                virtual_occurrence_count: 4,
                fixed_constraint_count: 2,
                virtual_fragment_count: 3,
                architectural_unit_count: 0,
                architectural_action_count: 0,
                architectural_fragment_count: 0,
                virtual_edge_connector_count: 0,
                architectural_edge_connector_count: 0,
                interference_count: 0,
                tied_pair_count: 0,
                copy_affinity_count: 0,
                tied_component_count: 0,
                early_clobber_count: 1,
                early_clobber_use_count: 2,
            },
        },
        ValidatedAllocationLegality {
            plan: Arc::new(legality_plan),
            receipt: AllocationLegalityValidationReceipt {
                identity: legality_id,
                ranges: ranges_id,
                register_environment,
                allocator_availability,
                function_count: 1,
                virtual_register_count: 3,
                point_count: 13,
                candidate_count: 21,
                early_clobber_point_count: 2,
                early_clobber_candidate_count: 3,
                entry_transition_count: 1,
            },
        },
        ValidatedFixedPrecoloredIntervals {
            plan: fixed_plan,
            receipt: FixedPrecoloredIntervalValidationReceipt {
                identity: fixed_id,
                ranges: ranges_id,
                legality: legality_id,
                register_environment,
                allocator_availability,
                optimization_unit,
                fuel_schedule,
                policy: FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1,
                usage: OptimizationWorkUsage::default(),
                function_count: 1,
                inspected_register_count: 3,
                interval_count: 2,
                entry_interval_count: 0,
                operand_interval_count: 2,
            },
        },
    )
}

/// Both early-clobber participants — the pinned-view use register and the
/// early-clobber definition — now partition like any other register. The
/// analysis entry runs the independent keyed replay internally, so a
/// successful result is also the compute/replay plan-equality and
/// usage-equality proof; the hazard stays enforced downstream because the
/// inserted copy is re-derived under reanalysis and assignment.
#[test]
fn early_clobber_participants_partition_with_identical_replay() {
    let (ranges, legality, fixed) = validated(ranges(), legality(), fixed());
    let requirements = analyze_fixed_precolored_split_requirements(
        &ranges,
        &legality,
        &fixed,
        FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
        budget(),
    )
    .expect("early-clobber participants are admitted");
    let function = &requirements.plan().functions[0];
    assert_eq!(function.machine, machine());
    assert_eq!(function.registers.len(), 3);
    // The divide result keeps `rax` until the shift count's `rcx` domain
    // leaves an empty intersection: two segments, the second opening on the
    // authenticated incompatible fixed-use boundary at the count operand.
    let fragments = &function.registers[0].fragments;
    assert_eq!(fragments.len(), 1);
    let segments = &fragments[0].segments;
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].candidates, [SOURCE_VIEW]);
    assert_eq!(
        segments[0].opening,
        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1
    );
    assert_eq!(segments[1].candidates, [DESTINATION_VIEW]);
    assert_eq!(segments[1].start, LiveRangePoint(6));
    assert_eq!(segments[1].end, LiveRangePoint(7));
    assert_eq!(
        segments[1].opening,
        FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
            incoming: None,
            site: shift_count_site(),
            destination_view: DESTINATION_VIEW,
        }
    );
    // The early-clobber definition partitions into one unremarkable segment.
    let segments = &function.registers[1].fragments[0].segments;
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].candidates, [SOURCE_VIEW, DESTINATION_VIEW]);
    let usage = requirements.receipt().usage();
    // validation_steps = 1 base + (1 + 0 tied + 1 early row + 2 early uses)
    // + register rows (4 + 1 + 1) + point rows (10 + 6 + 18) = 45; the
    // early-clobber row and use counts still price the constraint evidence.
    assert_eq!(usage.validation_steps, 45);
    assert_eq!(usage.commits, 5);
}

/// Lifting the early-clobber veto does not relax the other admission walls:
/// a register appearing in a tied use/def pair still rejects.
#[test]
fn tied_register_still_rejects() {
    let mut ranges = ranges();
    ranges.tied_pairs.push(DistinctUseDefTie {
        block: SelectedBlockId(0),
        position: LivenessPosition(6),
        instruction: SelectedInstructionId(1),
        use_operand: 1,
        use_virtual_register: DIVIDE_RESULT,
        use_point: LiveRangePoint(6),
        def_operand: 2,
        def_virtual_register: SHIFT_RESULT,
        def_point: LiveRangePoint(8),
        class: CLASS,
    });
    let (ranges, legality, fixed) = validated(ranges, legality(), fixed());
    assert_eq!(
        analyze_fixed_precolored_split_requirements(
            &ranges,
            &legality,
            &fixed,
            FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1,
            budget(),
        ),
        Err(
            FixedPrecoloredSplitRequirementError::UnsupportedTiedRegister {
                function: 0,
                register: 0,
            }
        )
    );
}

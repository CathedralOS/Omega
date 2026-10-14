//! The declared-form rewrite and its unit-liveness guard over hand-built
//! post-allocation rosters, driven through the xor-zero rule's predicate.

use optimization_core::{Optimization, OptimizationWorkUsage};
use physical_instructions::{
    PhysicalOperandFootprint, PostAllocationMachineBlock, PostAllocationMachineFunction,
    PostAllocationMachineInstruction,
};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    RegisterUnitId, RegisterViewId,
};
use selected_instructions::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, MachineAlternative,
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEncodedEffects,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
    VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, MachineId};

use super::rules::rewrite_declared_forms;
use super::unit_liveness::UnitLivenessTable;

const RULE: Optimization = Optimization::X86SelectXorZeroI64MaterializationV1;
const FLAGS: RegisterUnitId = RegisterUnitId(99);
const DESTINATION_VIEW: RegisterViewId = RegisterViewId(7);
const DESTINATION_UNIT: RegisterUnitId = RegisterUnitId(7);

fn baseline_materialization() -> MachineAlternative {
    MachineAlternative {
        key: MachineAlternativeKey {
            family: MachineAlternativeFamily::MaterializeI64,
            variant: 0,
        },
        applicability: MachineAlternativeApplicability::Always,
        size: MachineSizeKnowledge::ExactBytes(10),
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
    }
}

fn xor_zero_form() -> MachineAlternative {
    let mut encoded = MachineEncodedEffects::fallthrough_v1(vec![], vec![0]);
    encoded.implicit_unit_clobbers = vec![FLAGS];
    MachineAlternative {
        key: MachineAlternativeKey {
            family: MachineAlternativeFamily::MaterializeI64,
            variant: 1,
        },
        applicability: MachineAlternativeApplicability::OptimizationSelected { optimization: RULE },
        size: MachineSizeKnowledge::ExactBytes(3),
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded,
    }
}

/// A post-allocation row with explicit unit actions and a baseline
/// materialization alternative; the kind lives on the effect row.
fn instruction(
    id: u32,
    uses: &[RegisterUnitId],
    defs: &[RegisterUnitId],
) -> PostAllocationMachineInstruction {
    PostAllocationMachineInstruction {
        instruction: SelectedInstructionId(id),
        alternative: baseline_materialization(),
        operands: vec![PhysicalOperandFootprint {
            operand: 0,
            virtual_register: VirtualRegisterId(id),
            class: RegisterClassId(0),
            view: DESTINATION_VIEW,
            access: RegisterOperandAccess::Def,
            storage_units: vec![DESTINATION_UNIT],
            read_units: vec![],
            write_units: vec![DESTINATION_UNIT],
            write_semantics: None,
        }],
        address: None,
        implicit_unit_uses: uses.to_vec(),
        implicit_unit_defs: defs.to_vec(),
        implicit_unit_clobbers: vec![],
        unit_uses: uses.to_vec(),
        unit_defs: defs.to_vec(),
        unit_clobbers: vec![],
    }
}

fn effects(id: u32, kind: SelectedInstructionKind, declared: bool) -> InstructionMachineEffects {
    let mut alternatives = vec![baseline_materialization()];
    if declared {
        alternatives.push(xor_zero_form());
    }
    InstructionMachineEffects {
        instruction: SelectedInstructionId(id),
        kind,
        constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 0,
        },
        unit_uses: vec![],
        unit_defs: vec![],
        unit_clobbers: vec![],
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::None,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        provenance: SelectedInstructionProvenance {
            operations: vec![],
            values: vec![],
            edges: vec![],
            obligations: vec![],
            fuel: vec![],
        },
        alternatives,
    }
}

fn zero() -> SelectedInstructionKind {
    SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Signed(0),
    }
}

fn seven() -> SelectedInstructionKind {
    SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(7),
    }
}

/// One function whose blocks are given as (instructions, effect rows); the
/// successor table is supplied beside it.
fn roster(
    blocks: Vec<(
        Vec<PostAllocationMachineInstruction>,
        Vec<InstructionMachineEffects>,
    )>,
) -> (PostAllocationMachineFunction, FunctionMachineEffects) {
    let machine = MachineId::new(1).unwrap();
    let mut function = PostAllocationMachineFunction {
        machine,
        local_storage_slots: vec![],
        outgoing_arguments: vec![],
        blocks: vec![],
    };
    let mut effect_function = FunctionMachineEffects {
        machine,
        blocks: vec![],
    };
    for (index, (instructions, rows)) in blocks.into_iter().enumerate() {
        function.blocks.push(PostAllocationMachineBlock {
            block: SelectedBlockId(index as u32),
            instructions,
        });
        effect_function.blocks.push(BlockMachineEffects {
            block: SelectedBlockId(index as u32),
            instructions: rows,
        });
    }
    (function, effect_function)
}

fn admits_zero(kind: SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Signed(0) | IntegerValue::Unsigned(0)
        }
    )
}

fn run(
    function: PostAllocationMachineFunction,
    effect_function: FunctionMachineEffects,
    successors: Vec<Vec<usize>>,
) -> (
    Result<
        super::PostAllocationMachineRuleExecution,
        super::PostAllocationMachineOptimizationError,
    >,
    PostAllocationMachineFunction,
    OptimizationWorkUsage,
) {
    let mut functions = vec![function];
    let mut usage = OptimizationWorkUsage::default();
    let result = rewrite_declared_forms(
        RULE,
        &mut functions,
        &[effect_function],
        &[successors],
        &mut usage,
        admits_zero,
    );
    (result, functions.remove(0), usage)
}

#[test]
fn zero_materialization_with_dead_flags_takes_the_declared_form() {
    // materialize 0 -> return (the terminator reads nothing of the flags).
    let (function, rows) = roster(vec![(
        vec![instruction(1, &[], &[]), instruction(2, &[], &[])],
        vec![effects(1, zero(), true), effects(2, seven(), true)],
    )]);
    let (result, function, usage) = run(function, rows, vec![vec![]]);
    let execution = result.unwrap();
    assert_eq!(execution.candidates(), 1);
    assert_eq!(execution.actions().len(), 1);
    assert_eq!(execution.actions()[0].instruction, SelectedInstructionId(1));
    assert_eq!(execution.actions()[0].selected.variant, 1);
    let rewritten = &function.blocks[0].instructions[0];
    assert_eq!(rewritten.alternative, xor_zero_form());
    assert_eq!(rewritten.unit_clobbers, vec![FLAGS]);
    assert_eq!(rewritten.implicit_unit_clobbers, vec![FLAGS]);
    // The nonzero materialization is not a candidate and keeps its baseline.
    assert_eq!(
        function.blocks[0].instructions[1].alternative.key.variant,
        0
    );
    assert_eq!(usage.candidates, 1);
    assert_eq!(usage.commits, 1);
}

#[test]
fn a_later_flag_reader_in_the_block_keeps_the_baseline() {
    // materialize 0 -> consume flags (a conditional materialization).
    let (function, rows) = roster(vec![(
        vec![instruction(1, &[], &[]), instruction(2, &[FLAGS], &[])],
        vec![effects(1, zero(), true), effects(2, seven(), true)],
    )]);
    let (result, function, usage) = run(function, rows, vec![vec![]]);
    let execution = result.unwrap();
    assert_eq!(execution.candidates(), 1);
    assert!(execution.actions().is_empty());
    assert_eq!(
        function.blocks[0].instructions[0].alternative.key.variant,
        0
    );
    assert_eq!(usage.commits, 0);
    assert_eq!(usage.validation_steps, 1);
}

#[test]
fn a_flag_definition_before_the_reader_frees_the_candidate() {
    // materialize 0 -> compare (defines flags) -> consume flags.
    let (function, rows) = roster(vec![(
        vec![
            instruction(1, &[], &[]),
            instruction(2, &[], &[FLAGS]),
            instruction(3, &[FLAGS], &[]),
        ],
        vec![
            effects(1, zero(), true),
            effects(2, seven(), true),
            effects(3, seven(), true),
        ],
    )]);
    let (result, _, _) = run(function, rows, vec![vec![]]);
    assert_eq!(result.unwrap().actions().len(), 1);
}

#[test]
fn flags_live_into_a_successor_block_keep_the_baseline() {
    // Block 0: materialize 0, then jump. Block 1: read flags first.
    let (function, rows) = roster(vec![
        (
            vec![instruction(1, &[], &[]), instruction(2, &[], &[])],
            vec![effects(1, zero(), true), effects(2, seven(), false)],
        ),
        (
            vec![instruction(3, &[FLAGS], &[])],
            vec![effects(3, seven(), false)],
        ),
    ]);
    let (result, _, _) = run(function, rows, vec![vec![1], vec![]]);
    assert!(result.unwrap().actions().is_empty());
}

#[test]
fn flags_dead_through_a_loop_back_edge_take_the_declared_form() {
    // Block 0: materialize 0, jump to 1. Block 1: define flags, read them,
    // branch back to 0 or fall through to 2. Block 2: return.
    let (function, rows) = roster(vec![
        (
            vec![instruction(1, &[], &[]), instruction(2, &[], &[])],
            vec![effects(1, zero(), true), effects(2, seven(), false)],
        ),
        (
            vec![
                instruction(3, &[], &[FLAGS]),
                instruction(4, &[FLAGS], &[]),
                instruction(5, &[FLAGS], &[]),
            ],
            vec![
                effects(3, seven(), false),
                effects(4, seven(), false),
                effects(5, seven(), false),
            ],
        ),
        (
            vec![instruction(6, &[], &[])],
            vec![effects(6, seven(), false)],
        ),
    ]);
    let (result, _, _) = run(function, rows, vec![vec![1], vec![0, 2], vec![]]);
    assert_eq!(result.unwrap().actions().len(), 1);
}

#[test]
fn a_candidate_without_a_declared_form_rejects_by_rule_and_site() {
    let (function, rows) = roster(vec![(
        vec![instruction(1, &[], &[])],
        vec![effects(1, zero(), false)],
    )]);
    let (result, _, _) = run(function, rows, vec![vec![]]);
    assert_eq!(
        result,
        Err(
            super::PostAllocationMachineOptimizationError::MissingDeclaredForm {
                optimization: RULE,
                function: 0,
                instruction: 1,
            }
        )
    );
}

#[test]
fn an_already_selected_form_is_not_rewritten_again() {
    let (mut function, rows) = roster(vec![(
        vec![instruction(1, &[], &[])],
        vec![effects(1, zero(), true)],
    )]);
    function.blocks[0].instructions[0].alternative = xor_zero_form();
    let (result, _, usage) = run(function, rows, vec![vec![]]);
    let execution = result.unwrap();
    assert_eq!(execution.candidates(), 1);
    assert!(execution.actions().is_empty());
    assert_eq!(usage.commits, 0);
}

#[test]
fn unit_liveness_reads_block_locally_then_through_successors() {
    let (function, _) = roster(vec![
        (
            vec![instruction(1, &[], &[]), instruction(2, &[], &[FLAGS])],
            vec![],
        ),
        (vec![instruction(3, &[FLAGS], &[])], vec![]),
    ]);
    let successors = vec![vec![1], vec![]];
    let table = UnitLivenessTable::compute(&function, &successors, &[FLAGS]);
    // After instruction 1 the block itself rewrites the flags: dead.
    assert!(!table.live_after(&function, &successors, 0, 0, &[FLAGS]).0);
    // After instruction 2 the block is finished and block 1 reads: live.
    assert!(table.live_after(&function, &successors, 0, 1, &[FLAGS]).0);
    // After the last instruction of block 1 nothing follows: dead.
    assert!(!table.live_after(&function, &successors, 1, 0, &[FLAGS]).0);
}

#[test]
fn the_rule_table_executes_only_rules_it_names() {
    assert!(super::rules::executor(RULE).is_some());
    assert!(
        super::rules::executor(Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1).is_none()
    );
    assert!(super::rules::executor(Optimization::CopyPropagation).is_none());
}

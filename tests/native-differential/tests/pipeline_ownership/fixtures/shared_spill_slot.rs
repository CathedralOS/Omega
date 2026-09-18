//! Two sequential pressure waves in one acyclic caller. Each wave keeps more
//! simultaneously live non-materialization values than the explicit
//! physical-view allowlist admits, and every wave value dies as a pinned
//! scalar call argument: the fixed argument views stay inside the allowlist,
//! keep each spill's reload pairs placeable, and the second call of each
//! wave forces part of the wave across a unit-writing instruction — the
//! clobber-set-reduced span the composing recovery already supports.
//!
//! The second wave is born only after the first wave's calls: its chained
//! xor run is seeded by constants defined between the call pairs, so every
//! wave-two access — the victim's definition store and its reload — sits
//! strictly after the wave-one victims' spill windows closed. The rewrite's
//! last-writer slot check therefore reuses an already-declared `Spill` slot
//! instead of appending another: the retained program declares fewer slots
//! than the ledger's committed steps and the shared bytes enter frame demand
//! exactly once. A stranded seed constant itself resolves by
//! rematerialization, which declares no slot.
//!
//! The caller returns unit so the only fixed-use boundaries are the pinned
//! entry parameters' operand sites and the call arguments — transition
//! shapes the recorded policies already admit.

use crate::tests::{
    AdmissionProfile, AllocatorAvailabilityPolicy, Block, BlockId, ContractId, EdgeId,
    ExplicitOptimizationRequest, IntegerSign, IntegerType, IntegerValue, MachineContract,
    MachineId, NativeTarget, Operation, OperationId, OperationKind, OperationResult, Optimization,
    OptimizationSelections, OptimizationWorkBudget, OptimizedTargetLoweringRequest, ScalarType,
    StagedOptimizedAllocationLegality, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, ValueId, conditional_immediate_module,
    lower_optimized_to_target_operations, materialize_allocator_availability,
    operation_proof_bundle, optimize_artifact_sections,
    stage_optimized_allocation_legality_with_availability, stage_optimized_instruction_selection,
    stage_optimized_live_ranges, stage_optimized_liveness,
};
use target::{Architecture, ObjectFormat};

pub(crate) const SHARED_SPILL_SLOT_CALLER: u64 = 25_400;
const SHARED_SPILL_SLOT_ENTRY: u64 = 25_401;
const SHARED_SPILL_SLOT_CALLEE_QUAD: u64 = 25_402;
const SHARED_SPILL_SLOT_CALLEE_QUAD_BASE: u64 = 25_403;
const SHARED_SPILL_SLOT_CALLEE_DUO: u64 = 25_405;
// ValueIds are unique across the whole module, so the duo base must clear the
// quad callee's four parameter ids (25_403..=25_406).
const SHARED_SPILL_SLOT_CALLEE_DUO_BASE: u64 = 25_407;
const SHARED_SPILL_SLOT_PARAM_BASE: u64 = 25_410;
const SHARED_SPILL_SLOT_WAVE_ONE_BASE: u64 = 25_420;
const SHARED_SPILL_SLOT_SEED_BASE: u64 = 25_430;
const SHARED_SPILL_SLOT_WAVE_TWO_BASE: u64 = 25_440;
const SHARED_SPILL_SLOT_RETURN_EDGE: u64 = 25_470;
const SHARED_SPILL_SLOT_CONTRACT: u64 = 25_480;
const SHARED_SPILL_SLOT_OP_BASE: u64 = 25_500;

/// Six simultaneous values per wave against five unconstrained views: the
/// pinned parameter copies die inside the first wave, so each wave strands
/// at least one victim no matter which pinned views the two entry parameters
/// hold.
const SHARED_SPILL_SLOT_WAVE: u64 = 6;

/// The first call of each wave consumes the oldest four values; the pinned
/// argument views for a four-argument scalar call stay inside the allowlist
/// on every supported architecture.
const SHARED_SPILL_SLOT_FIRST_CALL_ARGS: usize = 4;

pub(crate) fn shared_spill_slot_caller() -> MachineId {
    MachineId::new(SHARED_SPILL_SLOT_CALLER).unwrap()
}

/// Views an unconstrained home may occupy. The choice matches the other
/// pressure fixtures: five caller-visible views, two of which pin the
/// incoming parameters on each architecture, and all pinned argument views
/// used below stay inside it. Windows and UEFI pin their four integer
/// argument views at `rcx`/`rdx`/`r8`/`r9`, so the System V views `rsi`/`rdi`
/// trade places with them there.
fn shared_spill_slot_allowlist(target: NativeTarget) -> &'static [&'static str] {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => &["rbx", "rcx", "rdx", "rsi", "rdi"],
        (Architecture::X86_64, _) => &["rbx", "rcx", "rdx", "r8", "r9"],
        (Architecture::Aarch64, _) => &["x0", "x1", "x2", "x3", "x19"],
    }
}

fn unit_callee(machine: u64, base: u64, arity: u64) -> TerminalMachine {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(machine).unwrap(),
        attachment: None,
        parameters: (0..arity)
            .map(|index| declaration(ValueId::new(base + index).unwrap()))
            .collect(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(base + 100).unwrap(),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(base + 100).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(base + 200).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(base + 300).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn shared_spill_slot_module() -> TerminalModule {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let value = |id: u64| ValueId::new(id).unwrap();
    let xor = |op: u64, left: ValueId, right: ValueId, result: u64| Operation {
        static_reach_binding: None,
        id: OperationId::new(op).unwrap(),
        result: OperationResult::Scalar(declaration(value(result))),
        kind: OperationKind::IntegerBitwiseXor { left, right },
    };
    let constant = |op: u64, result: u64, n: u64| Operation {
        static_reach_binding: None,
        id: OperationId::new(op).unwrap(),
        result: OperationResult::Scalar(declaration(value(result))),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(n.into()),
        },
    };
    let call = |op: u64, callee: u64, arguments: Vec<ValueId>| Operation {
        static_reach_binding: None,
        id: OperationId::new(op).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: MachineId::new(callee).unwrap(),
            arguments,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    let parameter = |index: u64| value(SHARED_SPILL_SLOT_PARAM_BASE + index);
    let seed = |index: u64| value(SHARED_SPILL_SLOT_SEED_BASE + index);
    let mut operations = Vec::new();
    let mut next_op = SHARED_SPILL_SLOT_OP_BASE;
    // Each wave: a chained xor run whose six results all stay live until the
    // wave's two calls consume them as pinned arguments — four first, then
    // the pair that had to cross the first call. Every definition is a real
    // computation, never a materialization rematerialization could take.
    // `operations` and `next_op` arrive as parameters rather than captures so
    // the seed constants between the two waves can mutate them directly.
    let wave = |operations: &mut Vec<Operation>,
                next_op: &mut u64,
                seeds: [ValueId; 2],
                wave_base: u64|
     -> Vec<u64> {
        let mut wave = Vec::new();
        wave.push(wave_base);
        operations.push(xor(*next_op, seeds[0], seeds[1], wave_base));
        *next_op += 1;
        operations.push(xor(*next_op, value(wave_base), seeds[0], wave_base + 1));
        *next_op += 1;
        wave.push(wave_base + 1);
        // The chained results mix the two preceding wave values, so the seeds
        // die at the second xor and the simultaneous live set stays exactly
        // the six wave values — one victim per wave, never a pile of spilled
        // reloads contending at one call.
        for index in 2..SHARED_SPILL_SLOT_WAVE {
            let result = wave_base + index;
            operations.push(xor(
                *next_op,
                value(wave[index as usize - 1]),
                value(wave[index as usize - 2]),
                result,
            ));
            wave.push(result);
            *next_op += 1;
        }
        operations.push(call(
            *next_op,
            SHARED_SPILL_SLOT_CALLEE_QUAD,
            wave[..SHARED_SPILL_SLOT_FIRST_CALL_ARGS]
                .iter()
                .map(|&result| value(result))
                .collect(),
        ));
        *next_op += 1;
        operations.push(call(
            *next_op,
            SHARED_SPILL_SLOT_CALLEE_DUO,
            wave[SHARED_SPILL_SLOT_FIRST_CALL_ARGS..]
                .iter()
                .map(|&result| value(result))
                .collect(),
        ));
        *next_op += 1;
        wave
    };
    wave(
        &mut operations,
        &mut next_op,
        [parameter(0), parameter(1)],
        SHARED_SPILL_SLOT_WAVE_ONE_BASE,
    );
    // Wave two is seeded by constants born between the call pairs: the seed
    // chain cannot carry a wave-one range across the first wave's calls, so
    // every wave-two access follows the first spill's closed window in block
    // order. A stranded seed rematerializes rather than taking a slot.
    operations.push(constant(next_op, SHARED_SPILL_SLOT_SEED_BASE, 3));
    next_op += 1;
    operations.push(constant(next_op, SHARED_SPILL_SLOT_SEED_BASE + 1, 5));
    next_op += 1;
    wave(
        &mut operations,
        &mut next_op,
        [seed(0), seed(1)],
        SHARED_SPILL_SLOT_WAVE_TWO_BASE,
    );
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: shared_spill_slot_caller(),
        attachment: None,
        parameters: (0..2_u64)
            .map(|index| declaration(parameter(index)))
            .collect(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(SHARED_SPILL_SLOT_ENTRY).unwrap(),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(SHARED_SPILL_SLOT_ENTRY).unwrap(),
            parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(SHARED_SPILL_SLOT_RETURN_EDGE).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(SHARED_SPILL_SLOT_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    // The callees' only job is to pin each wave value to an argument view at
    // the call site; their unit returns keep their own allocation trivial.
    let callee_quad = unit_callee(
        SHARED_SPILL_SLOT_CALLEE_QUAD,
        SHARED_SPILL_SLOT_CALLEE_QUAD_BASE,
        4,
    );
    let callee_duo = unit_callee(
        SHARED_SPILL_SLOT_CALLEE_DUO,
        SHARED_SPILL_SLOT_CALLEE_DUO_BASE,
        2,
    );
    conditional_immediate_module(
        shared_spill_slot_caller(),
        vec![caller, callee_quad, callee_duo],
    )
}

/// Selection through allocation legality with the unconstrained homes reduced
/// to the explicit allowlist: each wave's simultaneous live set exceeds it,
/// so the segment-home probe reports a capacity decline and the declared
/// shared-entry route hands custody to runtime-spill recovery, which commits
/// `RuntimeSpill` steps whose disjoint windows share declared slots.
pub(crate) fn staged_shared_spill_slot_legality(
    target: NativeTarget,
) -> StagedOptimizedAllocationLegality {
    let module = shared_spill_slot_module();
    let proof = operation_proof_bundle(&module);
    let (semantic, proof) = (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    );
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(
            OptimizationSelections::new([
                Optimization::CopyPropagation,
                // The declared shared-entry selection is what lets the
                // composing route answer the segment-home probe's capacity
                // decline with runtime-spill recovery: the six-value waves
                // exceed what segment homes can serve, and the declined
                // probe hands the still-owned legality to the spill steps
                // under test.
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            ])
            .unwrap(),
            // Both waves' values must clear selection, legality, and the
            // fixed-segment split analysis in one pass: the shared per-stage
            // capacity other pressure fixtures carry.
            OptimizationWorkBudget::new(4096, 8192, 8192, 4096, 4096).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let lowered = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    let staged = stage_optimized_instruction_selection(lowered).unwrap();
    let ranges = stage_optimized_live_ranges(stage_optimized_liveness(staged).unwrap()).unwrap();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let mut views = shared_spill_slot_allowlist(target)
        .iter()
        .map(|name| environment.physical().model().view_named(name).unwrap().id)
        .collect::<Vec<_>>();
    views.sort();
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views },
    )
    .unwrap();
    stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap()
}

//! Unit-call chain whose spill victim's flexible uses straddle an intervening
//! call.
//!
//! The caller materializes enough live constants to exhaust an explicit
//! physical-view allowlist at the first `CallUnit`, and holds one filler live
//! across that call so the single callee-saved survivor is already occupied,
//! leaving the chosen spill victim without a home. A `CallUnit` clobbers
//! every caller-saved unit, including the ABI result register, which leaves
//! the callee-saved allowlist member as the filler's only candidate — and
//! closes any still-open spill reload, so the victim's uses on either side of
//! a later `CallUnit` each take a private reload pair whose interval never
//! demands a cross-call home.

use crate::tests::{
    AdmissionProfile, AllocatorAvailabilityPolicy, Block, BlockId, ContractId, EdgeId,
    ExplicitOptimizationRequest, IntegerSign, IntegerType, IntegerValue, MachineContract,
    MachineId, NativeTarget, Operation, OperationId, OperationKind, OperationResult, Optimization,
    OptimizationSelections, OptimizedTargetLoweringRequest, ScalarType,
    StagedOptimizedAllocationLegality, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, ValueId, conditional_immediate_module,
    lower_optimized_to_target_operations, materialize_allocator_availability,
    operation_proof_bundle, optimize_artifact_sections, request,
    stage_optimized_allocation_legality_with_availability, stage_optimized_instruction_selection,
    stage_optimized_live_ranges, stage_optimized_liveness,
};
use target::{Architecture, ObjectFormat};

pub(crate) const CALL_SPANNING_RELOAD_CALLER: u64 = 23_400;
const CALL_SPANNING_RELOAD_ENTRY: u64 = 23_401;
const CALL_SPANNING_RELOAD_VICTIM: u64 = 23_410;
const CALL_SPANNING_RELOAD_FILLER_BASE: u64 = 23_411;
const CALL_SPANNING_RELOAD_INCOMING: u64 = 23_415;
const CALL_SPANNING_RELOAD_FIRST_CALL: u64 = 23_510;
const CALL_SPANNING_RELOAD_RETURN_EDGE: u64 = 23_520;
const CALL_SPANNING_RELOAD_CONTRACT: u64 = 23_530;
const CALL_SPANNING_RELOAD_WIDE_CALLEE: u64 = 23_100;
const CALL_SPANNING_RELOAD_WIDE_CALLEE_BASE: u64 = 23_110;
const CALL_SPANNING_RELOAD_NARROW_CALLEE: u64 = 23_200;
const CALL_SPANNING_RELOAD_NARROW_CALLEE_BASE: u64 = 23_210;

pub(crate) fn call_spanning_reload_caller() -> MachineId {
    MachineId::new(CALL_SPANNING_RELOAD_CALLER).unwrap()
}

/// The source value the reduced allowlist forces onto runtime-value storage.
pub(crate) fn call_spanning_reload_victim() -> ValueId {
    ValueId::new(CALL_SPANNING_RELOAD_VICTIM).unwrap()
}

/// The filler held live across the first call, so its interval occupies the
/// callee-saved survivor exactly when the victim would need it.
pub(crate) fn call_spanning_reload_incoming() -> ValueId {
    ValueId::new(CALL_SPANNING_RELOAD_INCOMING).unwrap()
}

/// Views an unconstrained home may occupy: the callee-saved survivor plus the
/// exact argument-pin views the calls fix their use operands to.
pub(crate) fn call_spanning_reload_allowlist(target: NativeTarget) -> &'static [&'static str] {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => &["rbx", "rcx", "rdx", "rsi", "rdi"],
        (Architecture::X86_64, _) => &["rbx", "rcx", "rdx", "r8", "r9"],
        (Architecture::Aarch64, _) => &["x0", "x1", "x2", "x3", "x19"],
    }
}

/// The only allowlisted view whose units survive every `CallUnit` clobber set.
pub(crate) fn call_spanning_reload_surviving_view(target: NativeTarget) -> &'static str {
    match target.architecture {
        Architecture::X86_64 => "rbx",
        Architecture::Aarch64 => "x19",
    }
}

fn unit_callee(machine: u64, base: u64, arity: usize) -> TerminalMachine {
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
            .map(|index| declaration(ValueId::new(base + index as u64).unwrap()))
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
            erased_scalar_formals: Vec::new(),
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
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(base + 300).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn call_spanning_reload_module(target: NativeTarget) -> TerminalModule {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let declaration = |id| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let victim = ValueId::new(CALL_SPANNING_RELOAD_VICTIM).unwrap();
    let fillers = (0..4_u64)
        .map(|index| ValueId::new(CALL_SPANNING_RELOAD_FILLER_BASE + index).unwrap())
        .collect::<Vec<_>>();
    let incoming = ValueId::new(CALL_SPANNING_RELOAD_INCOMING).unwrap();
    let constant = |id, result, n: u64| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(declaration(result)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(n.into()),
        },
    };
    let unit_call = |id, callee, arguments| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            erased_arguments: Vec::new(),
            callee,
            arguments,
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    let wide = unit_callee(
        CALL_SPANNING_RELOAD_WIDE_CALLEE,
        CALL_SPANNING_RELOAD_WIDE_CALLEE_BASE,
        4,
    );
    let narrow = unit_callee(
        CALL_SPANNING_RELOAD_NARROW_CALLEE,
        CALL_SPANNING_RELOAD_NARROW_CALLEE_BASE,
        1,
    );
    // The constant emission order and the first call's argument order differ per
    // ABI so each filler holds the pinned argument view it will die into in pin
    // order; the victim is materialized where the pressure point spills it and
    // its two later uses straddle an intervening `CallUnit`.
    let (const_order, call_arguments): ([usize; 6], Vec<ValueId>) =
        match (target.architecture, target.object_format) {
            (Architecture::X86_64, ObjectFormat::Elf) => (
                [0, 1, 2, 3, 4, 5],
                vec![fillers[3], fillers[2], fillers[1], fillers[0]],
            ),
            (Architecture::X86_64, _) => (
                [0, 1, 2, 3, 4, 5],
                vec![fillers[0], fillers[1], fillers[2], fillers[3]],
            ),
            (Architecture::Aarch64, _) => (
                [1, 2, 3, 4, 0, 5],
                vec![fillers[0], fillers[1], fillers[2], fillers[3]],
            ),
        };
    let consts = [
        victim, fillers[0], fillers[1], fillers[2], fillers[3], incoming,
    ];
    let mut operations = Vec::new();
    for (index, position) in const_order.iter().enumerate() {
        operations.push(constant(
            CALL_SPANNING_RELOAD_FIRST_CALL - 10 + index as u64,
            consts[*position],
            index as u64,
        ));
    }
    operations.push(unit_call(
        CALL_SPANNING_RELOAD_FIRST_CALL,
        wide.id,
        call_arguments,
    ));
    operations.push(unit_call(
        CALL_SPANNING_RELOAD_FIRST_CALL + 1,
        narrow.id,
        vec![incoming],
    ));
    operations.push(unit_call(
        CALL_SPANNING_RELOAD_FIRST_CALL + 2,
        narrow.id,
        vec![victim],
    ));
    operations.push(unit_call(
        CALL_SPANNING_RELOAD_FIRST_CALL + 3,
        narrow.id,
        vec![victim],
    ));
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: call_spanning_reload_caller(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(CALL_SPANNING_RELOAD_ENTRY).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(CALL_SPANNING_RELOAD_ENTRY).unwrap(),
            parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(CALL_SPANNING_RELOAD_RETURN_EDGE).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(CALL_SPANNING_RELOAD_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    conditional_immediate_module(call_spanning_reload_caller(), vec![wide, narrow, caller])
}

/// Selection through allocation legality with the unconstrained homes reduced
/// to the explicit allowlist, leaving the calls' argument pins plus exactly one
/// callee-saved view. Caller-chosen optimization selections ride the same
/// artifact so recovery-composition coverage can declare the selection the
/// retained route must bind.
fn staged_call_spanning_legality(
    target: NativeTarget,
    request: ExplicitOptimizationRequest,
) -> StagedOptimizedAllocationLegality {
    let module = call_spanning_reload_module(target);
    let proof = operation_proof_bundle(&module);
    let (semantic, proof) = (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_section(&module, &proof).unwrap(),
    );
    let optimized =
        optimize_artifact_sections(&semantic, &proof, &AdmissionProfile::default(), request)
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
    let mut views = call_spanning_reload_allowlist(target)
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

pub(crate) fn staged_call_spanning_reload_legality(
    target: NativeTarget,
) -> StagedOptimizedAllocationLegality {
    staged_call_spanning_legality(
        target,
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
}

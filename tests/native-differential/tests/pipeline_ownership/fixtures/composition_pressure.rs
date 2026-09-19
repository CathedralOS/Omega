//! Divergent-pressure caller: fixed/precolored segment homes succeed while
//! post-copy register-home assignment still reports `NoCompatibleHome`.
//!
//! The caller splits into leaf calls after a branch. Flexible killer values
//! each die as the last argument of their own leaf call, so the earlier
//! argument pins plus a call-crossing filler leave a small viable set. A
//! further flexible value `X` holds the earliest live point and two viable
//! views: segment homes take the lower one; post-copy register homes prefer
//! `X`'s copy-affinity view, which is higher. The register preference removes
//! one killer view too many — one killer loses its last viable home — while
//! the lower segment choice leaves each of them a home, so the segment stage
//! succeeds and post-copy assignment still fails.
//!
//! x86-64: `X` dies as the fourth argument of its leaf call, keeping
//! `{rcx, rbx}` with an affinity to `rcx`; the two killers die as third
//! arguments, each keeping `{rcx, rdx}` — register homes take `rcx` and
//! strand a killer while segment homes take `rbx` and let both place.
//!
//! AArch64: `X` is `param1`'s forwarded value. Instruction selection
//! normalizes each entry parameter through a pinned copy in parameter order,
//! so `X` is born at the second copy — after `x0`/`x1`'s pinned domains die
//! but while `x2`/`x3`'s are still live. A call-crossing filler removes
//! `x19`, leaving `X` with `{x0, x1}` and an affinity to `param1`'s `x1`.
//! The three killers die as second leaf arguments, keeping `{x1, x2, x3}`:
//! register homes take `x1` for `X` and strand the last killer while segment
//! homes take `x0` and let all three place.
//!
//! Two killer shapes share that pressure. `Immediate` defines every killer by
//! a bare literal, so the stranded register's only definition is a
//! materialization and recovery rematerializes it — every runtime step in
//! the retained ledger is a `RuntimeRematerialization`. `Computed` defines
//! each killer by an add over an already-live operand plus a fresh literal —
//! an exact add whose known operands keep the certificate provable on
//! x86-64, a wrapping add over `X`'s forwarded parameter on AArch64 — so the
//! failed register's definition is never a materialization. Recovery's
//! rematerialization-first cost decision declines it and the same victim
//! commits a genuine `RuntimeSpill` step instead: a private `{Spill, 8, 8}`
//! slot, a store at the add, and reload pairs at the leaf uses, all retained
//! for replay under the same divergent pressure.

use crate::tests::{
    AdmissionProfile, AllocatorAvailabilityPolicy, Block, BlockId, ContractId, EdgeId,
    ExplicitOptimizationRequest, IntegerSign, IntegerType, IntegerValue, MachineContract,
    MachineId, NativeTarget, ObligationId, Operation, OperationId, OperationKind, OperationResult,
    OptimizationSelections, OptimizationWorkBudget, OptimizedTargetLoweringRequest, ScalarType,
    StagedOptimizedAllocationLegality, SuccessorEdge, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, ValueId, conditional_immediate_module,
    lower_optimized_to_target_operations, materialize_allocator_availability,
    operation_proof_bundle, optimize_artifact_sections,
    stage_optimized_allocation_legality_with_availability, stage_optimized_instruction_selection,
    stage_optimized_live_ranges, stage_optimized_liveness,
};
use target::Architecture;

/// How the pressured leaf victims are defined. `Immediate` keeps each killer
/// a bare literal so recovery rematerializes it; `Computed` defines each by
/// an add whose result is not a materialization, so rematerialization
/// declines and the same victim commits a real spill step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositionPressureKillers {
    Immediate,
    Computed,
}

const COMPOSITION_PRESSURE_CALLER: u64 = 24_400;
const COMPOSITION_PRESSURE_ENTRY: u64 = 24_401;
const COMPOSITION_PRESSURE_X: u64 = 24_410;
const COMPOSITION_PRESSURE_W: u64 = 24_411;
const COMPOSITION_PRESSURE_Z: u64 = 24_412;
const COMPOSITION_PRESSURE_W2: u64 = 24_413;
const COMPOSITION_PRESSURE_COND_ENTRY: u64 = 24_414;
const COMPOSITION_PRESSURE_COND_LEAF: u64 = 24_415;
const COMPOSITION_PRESSURE_COND_MID: u64 = 24_416;
const COMPOSITION_PRESSURE_PARAM_BASE: u64 = 24_430;
const COMPOSITION_PRESSURE_LEAF: u64 = 24_440;
const COMPOSITION_PRESSURE_JUMP: u64 = 24_441;
const COMPOSITION_PRESSURE_Z_LEAF: u64 = 24_442;
const COMPOSITION_PRESSURE_W_LEAF: u64 = 24_443;
const COMPOSITION_PRESSURE_X_LEAF: u64 = 24_444;
const COMPOSITION_PRESSURE_MID: u64 = 24_445;
const COMPOSITION_PRESSURE_W2_LEAF: u64 = 24_446;
const COMPOSITION_PRESSURE_FILLER_BASE: u64 = 24_450;
const COMPOSITION_PRESSURE_EDGE_BASE: u64 = 24_460;
const COMPOSITION_PRESSURE_OPERAND_BASE: u64 = 24_700;
const COMPOSITION_PRESSURE_OBLIGATION_BASE: u64 = 24_720;
const COMPOSITION_PRESSURE_OP_BASE: u64 = 24_500;
const COMPOSITION_PRESSURE_CONTRACT: u64 = 24_530;
const COMPOSITION_PRESSURE_TRIO_CALLEE: u64 = 24_100;
const COMPOSITION_PRESSURE_TRIO_CALLEE_BASE: u64 = 24_110;
const COMPOSITION_PRESSURE_QUAD_CALLEE: u64 = 24_200;
const COMPOSITION_PRESSURE_QUAD_CALLEE_BASE: u64 = 24_210;
const COMPOSITION_PRESSURE_NARROW_CALLEE: u64 = 24_300;
const COMPOSITION_PRESSURE_NARROW_CALLEE_BASE: u64 = 24_310;
const COMPOSITION_PRESSURE_DUO_CALLEE: u64 = 24_350;
const COMPOSITION_PRESSURE_DUO_CALLEE_BASE: u64 = 24_360;

/// Views an unconstrained home may occupy: the argument-pin views the leaf
/// calls fix their operands to plus the single view surviving a call.
fn composition_pressure_allowlist(target: NativeTarget) -> &'static [&'static str] {
    match target.architecture {
        Architecture::X86_64 => &["rbx", "rcx", "rdx", "rsi", "rdi"],
        Architecture::Aarch64 => &["x0", "x1", "x2", "x3", "x19"],
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

fn composition_pressure_module(
    target: NativeTarget,
    killers: CompositionPressureKillers,
) -> TerminalModule {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let boolean_type = ScalarType::Boolean;
    let declaration = |id, scalar_type| ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    };
    let scalar = |id| declaration(id, scalar_type);
    let boolean = |id| declaration(id, boolean_type);
    let value = |id: u64| ValueId::new(id).unwrap();
    let x = value(COMPOSITION_PRESSURE_X);
    let w = value(COMPOSITION_PRESSURE_W);
    let z = value(COMPOSITION_PRESSURE_Z);
    let param = |index: u64| value(COMPOSITION_PRESSURE_PARAM_BASE + index);
    let filler = |index: u64| value(COMPOSITION_PRESSURE_FILLER_BASE + index);
    let operand = |index: u64| value(COMPOSITION_PRESSURE_OPERAND_BASE + index);
    let obligation =
        |index: u64| ObligationId::new(COMPOSITION_PRESSURE_OBLIGATION_BASE + index).unwrap();
    let constant = |id: u64, result, n: u64| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(scalar(result)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(n.into()),
        },
    };
    let exact_add = |id: u64, left, right, result, obligation: ObligationId| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(scalar(result)),
        kind: OperationKind::ExactIntegerAdd {
            left,
            right,
            obligation,
        },
    };
    let wrapping_add = |id: u64, left, right, result| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(scalar(result)),
        kind: OperationKind::WrappingIntegerAdd { left, right },
    };
    let boolean_constant = |id: u64, result| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(boolean(result)),
        kind: OperationKind::BooleanConstant { value: true },
    };
    let less_than = |id: u64, left, right, result| Operation {
        static_reach_binding: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(boolean(result)),
        kind: OperationKind::IntegerLessThan { left, right },
    };
    let unit_call = |id: u64, callee, arguments| Operation {
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
    let edge = |id: u64, target: BlockId| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: EdgeId::new(id).unwrap(),
        target,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let return_unit = |id: u64| Terminator::ReturnUnit {
        edge: EdgeId::new(id).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    let trio = unit_callee(
        COMPOSITION_PRESSURE_TRIO_CALLEE,
        COMPOSITION_PRESSURE_TRIO_CALLEE_BASE,
        3,
    );
    let quad = unit_callee(
        COMPOSITION_PRESSURE_QUAD_CALLEE,
        COMPOSITION_PRESSURE_QUAD_CALLEE_BASE,
        4,
    );
    let duo = unit_callee(
        COMPOSITION_PRESSURE_DUO_CALLEE,
        COMPOSITION_PRESSURE_DUO_CALLEE_BASE,
        2,
    );
    let narrow = unit_callee(
        COMPOSITION_PRESSURE_NARROW_CALLEE,
        COMPOSITION_PRESSURE_NARROW_CALLEE_BASE,
        1,
    );
    let leaf = BlockId::new(COMPOSITION_PRESSURE_LEAF).unwrap();
    let jump = BlockId::new(COMPOSITION_PRESSURE_JUMP).unwrap();
    let mid = BlockId::new(COMPOSITION_PRESSURE_MID).unwrap();
    let z_leaf = BlockId::new(COMPOSITION_PRESSURE_Z_LEAF).unwrap();
    let w_leaf = BlockId::new(COMPOSITION_PRESSURE_W_LEAF).unwrap();
    let w2_leaf = BlockId::new(COMPOSITION_PRESSURE_W2_LEAF).unwrap();
    let x_leaf = BlockId::new(COMPOSITION_PRESSURE_X_LEAF).unwrap();
    let cond_entry = value(COMPOSITION_PRESSURE_COND_ENTRY);
    let cond_leaf = value(COMPOSITION_PRESSURE_COND_LEAF);
    let cond_mid = value(COMPOSITION_PRESSURE_COND_MID);
    // `W`/`Z` die as each leaf call's last argument: the earlier argument
    // pins and a call-crossing filler leave exactly the two-view sets each
    // architecture needs.
    let pressure_leaf = |block: BlockId, victim, op_base: u64, filler_base: u64| Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: block,
        parameters: Vec::new(),
        operations: match target.architecture {
            // x86-64: `victim` dies at the third argument copy, so the
            // first two argument pins (`rdi`/`rsi`) and the call-crossing
            // filler (`rbx`) leave `{rcx, rdx}`.
            Architecture::X86_64 => vec![
                constant(op_base, filler(filler_base), 0),
                constant(op_base + 1, filler(filler_base + 1), 1),
                constant(op_base + 2, filler(filler_base + 2), 2),
                unit_call(
                    op_base + 3,
                    trio.id,
                    vec![filler(filler_base), filler(filler_base + 1), victim],
                ),
                unit_call(op_base + 4, narrow.id, vec![filler(filler_base + 2)]),
            ],
            // AArch64: `victim` dies at the second argument copy, so the
            // first argument pin (`x0`) and the call-crossing filler (`x19`)
            // leave `{x1, x2, x3}`.
            Architecture::Aarch64 => vec![
                constant(op_base, filler(filler_base), 0),
                constant(op_base + 1, filler(filler_base + 1), 1),
                unit_call(op_base + 2, duo.id, vec![filler(filler_base), victim]),
                unit_call(op_base + 3, narrow.id, vec![filler(filler_base + 1)]),
            ],
        },
        terminator: return_unit(op_base + 5),
    };
    let (parameters, entry_operations, caller_blocks) = match target.architecture {
        Architecture::X86_64 => (
            Vec::new(),
            // The entry block defines `X` first so it holds the earliest live
            // point of the pressure trio. Under `Computed` the killers are
            // exact adds over the still-live `X` and a fresh literal: the
            // result's definition is an `ExactAddI64`, not a materialization,
            // while the known-zero left operand keeps the certificate
            // provable. The added literal's interval is one instruction, so
            // the killers' live ranges — and the divergent pressure — are
            // unchanged.
            match killers {
                CompositionPressureKillers::Immediate => vec![
                    constant(COMPOSITION_PRESSURE_OP_BASE, x, 0),
                    boolean_constant(COMPOSITION_PRESSURE_OP_BASE + 1, cond_entry),
                    constant(COMPOSITION_PRESSURE_OP_BASE + 2, w, 1),
                    constant(COMPOSITION_PRESSURE_OP_BASE + 3, z, 2),
                ],
                CompositionPressureKillers::Computed => vec![
                    constant(COMPOSITION_PRESSURE_OP_BASE, x, 0),
                    boolean_constant(COMPOSITION_PRESSURE_OP_BASE + 1, cond_entry),
                    constant(COMPOSITION_PRESSURE_OP_BASE + 2, operand(0), 1),
                    exact_add(
                        COMPOSITION_PRESSURE_OP_BASE + 3,
                        x,
                        operand(0),
                        w,
                        obligation(0),
                    ),
                    constant(COMPOSITION_PRESSURE_OP_BASE + 4, operand(1), 2),
                    exact_add(
                        COMPOSITION_PRESSURE_OP_BASE + 5,
                        x,
                        operand(1),
                        z,
                        obligation(1),
                    ),
                ],
            },
            vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: leaf,
                    parameters: Vec::new(),
                    operations: vec![boolean_constant(
                        COMPOSITION_PRESSURE_OP_BASE + 30,
                        cond_leaf,
                    )],
                    terminator: Terminator::Conditional {
                        condition: cond_leaf,
                        when_true: edge(COMPOSITION_PRESSURE_EDGE_BASE + 2, z_leaf),
                        when_false: edge(COMPOSITION_PRESSURE_EDGE_BASE + 3, w_leaf),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: jump,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        edge: EdgeId::new(COMPOSITION_PRESSURE_EDGE_BASE + 4).unwrap(),
                        target: x_leaf,
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                pressure_leaf(z_leaf, z, COMPOSITION_PRESSURE_OP_BASE + 40, 0),
                pressure_leaf(w_leaf, w, COMPOSITION_PRESSURE_OP_BASE + 50, 10),
                // `X` dies as the fourth argument, keeping `{rcx, rbx}` with
                // an affinity to `rcx`: register homes take `rcx` and
                // strand a victim while segment homes take `rbx` and let
                // both survivors place.
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: x_leaf,
                    parameters: Vec::new(),
                    operations: vec![
                        constant(COMPOSITION_PRESSURE_OP_BASE + 20, filler(20), 0),
                        constant(COMPOSITION_PRESSURE_OP_BASE + 21, filler(21), 1),
                        constant(COMPOSITION_PRESSURE_OP_BASE + 22, filler(22), 2),
                        unit_call(
                            COMPOSITION_PRESSURE_OP_BASE + 23,
                            quad.id,
                            vec![filler(20), filler(21), filler(22), x],
                        ),
                    ],
                    terminator: return_unit(COMPOSITION_PRESSURE_EDGE_BASE + 5),
                },
            ],
        ),
        Architecture::Aarch64 => (
            // `param1` is the pressure value `X`: instruction selection
            // normalizes each used entry parameter through a pinned copy in
            // parameter order, so `X` is born at the first copy — while
            // `param2`/`param3`'s pinned `x2`/`x3` domains are still live —
            // and the `x19` crosser in its leaf removes the remaining call
            // survivor, leaving `X` with `{x0, x1}` and an affinity to
            // `param1`'s pinned `x1`.
            (0..4_u64)
                .map(|index| scalar(param(index)))
                .collect::<Vec<_>>(),
            // `param2`/`param3` are consumed early so their transport
            // registers die before the killers are born; the compare also
            // supplies the shared branch condition for every dispatcher.
            // Under `Computed` each killer is a wrapping add over `X`'s
            // forwarded parameter and one shared literal — the parameter's
            // register is already live to `x_leaf`, so the killers' ranges
            // and the divergent pressure are unchanged while their
            // definitions become adds, not materializations.
            match killers {
                CompositionPressureKillers::Immediate => vec![
                    less_than(COMPOSITION_PRESSURE_OP_BASE, param(2), param(3), cond_entry),
                    constant(COMPOSITION_PRESSURE_OP_BASE + 1, w, 1),
                    constant(
                        COMPOSITION_PRESSURE_OP_BASE + 2,
                        value(COMPOSITION_PRESSURE_W2),
                        2,
                    ),
                    constant(COMPOSITION_PRESSURE_OP_BASE + 3, z, 3),
                ],
                CompositionPressureKillers::Computed => vec![
                    less_than(COMPOSITION_PRESSURE_OP_BASE, param(2), param(3), cond_entry),
                    constant(COMPOSITION_PRESSURE_OP_BASE + 1, operand(0), 1),
                    wrapping_add(COMPOSITION_PRESSURE_OP_BASE + 2, param(1), operand(0), w),
                    wrapping_add(
                        COMPOSITION_PRESSURE_OP_BASE + 3,
                        param(1),
                        operand(0),
                        value(COMPOSITION_PRESSURE_W2),
                    ),
                    wrapping_add(COMPOSITION_PRESSURE_OP_BASE + 4, param(1), operand(0), z),
                ],
            },
            vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: leaf,
                    parameters: Vec::new(),
                    operations: vec![boolean_constant(
                        COMPOSITION_PRESSURE_OP_BASE + 30,
                        cond_leaf,
                    )],
                    terminator: Terminator::Conditional {
                        condition: cond_leaf,
                        when_true: edge(COMPOSITION_PRESSURE_EDGE_BASE + 2, z_leaf),
                        when_false: edge(COMPOSITION_PRESSURE_EDGE_BASE + 3, mid),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: jump,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        edge: EdgeId::new(COMPOSITION_PRESSURE_EDGE_BASE + 4).unwrap(),
                        target: x_leaf,
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                pressure_leaf(z_leaf, z, COMPOSITION_PRESSURE_OP_BASE + 40, 0),
                pressure_leaf(w_leaf, w, COMPOSITION_PRESSURE_OP_BASE + 50, 10),
                // `X` dies at a compare ahead of the leaf calls; the filler
                // crossing the first call removes `x19` from `X`'s fragment,
                // leaving `{x0, x1}`.
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: x_leaf,
                    parameters: Vec::new(),
                    operations: vec![
                        constant(COMPOSITION_PRESSURE_OP_BASE + 20, filler(30), 0),
                        constant(COMPOSITION_PRESSURE_OP_BASE + 21, filler(31), 1),
                        constant(COMPOSITION_PRESSURE_OP_BASE + 22, filler(32), 2),
                        less_than(
                            COMPOSITION_PRESSURE_OP_BASE + 23,
                            param(1),
                            filler(31),
                            cond_mid,
                        ),
                        unit_call(
                            COMPOSITION_PRESSURE_OP_BASE + 24,
                            narrow.id,
                            vec![filler(32)],
                        ),
                        unit_call(
                            COMPOSITION_PRESSURE_OP_BASE + 25,
                            narrow.id,
                            vec![filler(30)],
                        ),
                    ],
                    terminator: return_unit(COMPOSITION_PRESSURE_EDGE_BASE + 5),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: mid,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: cond_entry,
                        when_true: edge(COMPOSITION_PRESSURE_EDGE_BASE + 6, w_leaf),
                        when_false: edge(COMPOSITION_PRESSURE_EDGE_BASE + 7, w2_leaf),
                    },
                },
                pressure_leaf(
                    w2_leaf,
                    value(COMPOSITION_PRESSURE_W2),
                    COMPOSITION_PRESSURE_OP_BASE + 60,
                    20,
                ),
            ],
        ),
    };
    let caller = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(COMPOSITION_PRESSURE_CALLER).unwrap(),
        attachment: None,
        parameters,
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(COMPOSITION_PRESSURE_ENTRY).unwrap(),
        blocks: {
            let mut blocks = vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(COMPOSITION_PRESSURE_ENTRY).unwrap(),
                parameters: Vec::new(),
                operations: entry_operations,
                terminator: Terminator::Conditional {
                    condition: cond_entry,
                    when_true: edge(COMPOSITION_PRESSURE_EDGE_BASE, leaf),
                    when_false: edge(COMPOSITION_PRESSURE_EDGE_BASE + 1, jump),
                },
            }];
            blocks.extend(caller_blocks);
            blocks
        },
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(COMPOSITION_PRESSURE_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    conditional_immediate_module(
        MachineId::new(COMPOSITION_PRESSURE_CALLER).unwrap(),
        vec![trio, quad, narrow, duo, caller],
    )
}

/// Selection through allocation legality with the unconstrained homes reduced
/// to the explicit allowlist. The caller's structure is chosen so the same
/// legality succeeds under fixed/precolored segment homes yet still reports
/// `NoCompatibleHome` under post-copy register-home assignment, which is the
/// pressure the runtime-spill composition must absorb.
fn staged_composition_pressure_legality(
    target: NativeTarget,
    request: ExplicitOptimizationRequest,
    killers: CompositionPressureKillers,
) -> StagedOptimizedAllocationLegality {
    let module = composition_pressure_module(target, killers);
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
    let mut views = composition_pressure_allowlist(target)
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

/// Composition-coverage legality: the divergent-pressure caller under
/// caller-chosen optimization selections, its killers defined by bare
/// literals.
pub(crate) fn staged_composition_pressure_module_legality(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> StagedOptimizedAllocationLegality {
    staged_composition_pressure_legality(
        target,
        ExplicitOptimizationRequest::new(
            selections,
            OptimizationWorkBudget::new(4096, 8192, 8192, 4096, 4096).unwrap(),
        )
        .unwrap(),
        CompositionPressureKillers::Immediate,
    )
}

/// The same divergent pressure with each killer defined by an add instead of
/// a literal, so the stranded register fails rematerialization admission and
/// recovery must commit a genuine `RuntimeSpill` ledger step.
pub(crate) fn staged_composition_pressure_computed_killer_legality(
    target: NativeTarget,
    selections: OptimizationSelections,
) -> StagedOptimizedAllocationLegality {
    staged_composition_pressure_legality(
        target,
        ExplicitOptimizationRequest::new(
            selections,
            OptimizationWorkBudget::new(4096, 8192, 8192, 4096, 4096).unwrap(),
        )
        .unwrap(),
        CompositionPressureKillers::Computed,
    )
}

//! Shared hand-built `LoweredPsi` fixtures for stage-level coverage.
//!
//! These fixtures construct the complete unsealed carrier directly rather than
//! lowering authored source: the contract under test is the public
//! `run_psi_optimization` entrance, and a hand-built module crosses the same
//! `validate_module_for_optimization` admission gate every real input takes.
//!
//! Each test binary compiles this module separately; not every binary uses
//! every helper.
#![allow(dead_code)]

use lowered_psi::LoweredPsi;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::{
    Block, DebugFileId, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin,
    DebugSourceSpan, DebugSubject, MachineContract, Operation, OperationKind, OperationResult,
    ProofBundle, SuccessorEdge, TerminalDebugMap, TerminalMachine, TerminalMachineResult,
    Terminator, ValueDeclaration, VocabularyMarker,
};

pub fn i32_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
}

pub fn declaration(ordinal: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(ordinal).unwrap(),
        scalar_type,
    }
}

pub fn boolean(ordinal: u64) -> ValueDeclaration {
    declaration(ordinal, ScalarType::Boolean)
}

pub fn i32(ordinal: u64) -> ValueDeclaration {
    declaration(ordinal, i32_type())
}

pub fn operation(ordinal: u64, result: ValueDeclaration, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        id: OperationId::new(ordinal).unwrap(),
        result: OperationResult::Scalar(result),
        kind,
    }
}

pub fn integer_constant(ordinal: u64, result: ValueDeclaration, value: i128) -> Operation {
    operation(
        ordinal,
        result,
        OperationKind::IntegerConstant {
            value: IntegerValue::Signed(value),
        },
    )
}

pub fn contract(ordinal: u64) -> MachineContract {
    MachineContract {
        id: ContractId::new(ordinal).unwrap(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

pub fn lowered(machines: Vec<TerminalMachine>) -> LoweredPsi {
    let entry = machines[0].id;
    LoweredPsi {
        selected_ieee_float_comparison_occurrences: Vec::new(),
        semantic_module: terminal_psi::TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry,
            scalar_qualifications: Default::default(),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            scalar_block_invariants: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
    }
}

pub fn machine(
    ordinal: u64,
    parameters: Vec<ValueDeclaration>,
    result: TerminalMachineResult,
    entry: BlockId,
    blocks: Vec<Block>,
) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(ordinal).unwrap(),
        attachment: None,
        parameters,
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks,
        contract: contract(ordinal),
    }
}

pub fn block(
    ordinal: u64,
    parameters: Vec<ValueDeclaration>,
    operations: Vec<Operation>,
    terminator: Terminator,
) -> Block {
    Block {
        structural_parameters: Vec::new(),
        id: BlockId::new(ordinal).unwrap(),
        parameters,
        operations,
        terminator,
    }
}

pub fn successor(ordinal: u64, target: BlockId, arguments: Vec<ValueId>) -> SuccessorEdge {
    SuccessorEdge {
        edge: EdgeId::new(ordinal).unwrap(),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

pub fn conditional(
    condition: ValueId,
    when_true: SuccessorEdge,
    when_false: SuccessorEdge,
) -> Terminator {
    Terminator::Conditional {
        condition,
        when_true,
        when_false,
    }
}

pub fn jump(ordinal: u64, target: BlockId, arguments: Vec<ValueId>) -> Terminator {
    Terminator::Jump {
        edge: EdgeId::new(ordinal).unwrap(),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

pub fn return_value(ordinal: u64, value: ValueId) -> Terminator {
    Terminator::Return {
        edge: EdgeId::new(ordinal).unwrap(),
        value,
        cleanup_actions: Vec::new(),
    }
}

pub fn edge(ordinal: u64) -> EdgeId {
    EdgeId::new(ordinal).unwrap()
}

pub fn block_id(ordinal: u64) -> BlockId {
    BlockId::new(ordinal).unwrap()
}

pub fn machine_id(ordinal: u64) -> MachineId {
    MachineId::new(ordinal).unwrap()
}

pub fn operation_id(ordinal: u64) -> OperationId {
    OperationId::new(ordinal).unwrap()
}

pub fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

/// Attach a valid debug map covering `subjects` so rewrite tests can observe
/// sidecar pruning. The map validates because `semantic` is recomputed from
/// the module and every subject exists in it.
pub fn with_debug_sites(lowered: &mut LoweredPsi, subjects: &[DebugSubject]) {
    let file = DebugFileId::new(1).unwrap();
    let mut sites = subjects
        .iter()
        .map(|subject| DebugSite {
            subject: *subject,
            span: DebugSourceSpan {
                file,
                start: 0,
                end: 1,
            },
        })
        .collect::<Vec<_>>();
    sites.sort_by_key(|site| site.subject);
    lowered.debug_map = Some(TerminalDebugMap {
        semantic: terminal_codec::terminal_psi_identity(&lowered.semantic_module).unwrap(),
        files: vec![DebugSourceFile {
            id: file,
            origin: DebugSourceOrigin::User,
            byte_len: 1,
            digest: DebugSourceDigest::from_bytes([7; 32]),
            path: "fixture.omg".to_string(),
        }],
        sites,
    });
}

/// Minimal valid carrier: one Unit machine whose entry block returns
/// immediately. The stage must accept it and return it unchanged under every
/// executable selection.
pub fn minimal_unit_lowered() -> LoweredPsi {
    let entry = block_id(1);
    lowered(vec![machine(
        1,
        Vec::new(),
        TerminalMachineResult::Unit,
        entry,
        vec![block(
            1,
            Vec::new(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: edge(1),
                trivial_affine_discards: Vec::new(),
            },
        )],
    )])
}

/// Copy-propagation fixture. One machine, one diamond, one merge chain:
///
/// ```text
/// b1 (entry, cond v1) ──true──▶ b2 ──[v2, v20]──▶ ┐
///                   └──false─▶ b3 ──[v2, v30]──▶ b4(v41, v42) ──[v44]──▶ b5(v51) → return
/// ```
///
/// `v41` binds `v2` on every incoming edge (a copy); `v42` binds `v20`/`v30`
/// (different values, not a copy); `v51` binds only `v44`, the add result of
/// `v41 + v42` (a copy through the single-edge inventory). `v20`/`v30` are
/// used only by the live `v42`, so dead-scalar elimination changes nothing.
pub fn copy_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4, b5) = (
        block_id(1),
        block_id(2),
        block_id(3),
        block_id(4),
        block_id(5),
    );
    let (v1, v2) = (value(1), value(2));
    let (v20, v30) = (value(20), value(30));
    let (v41, v42, v44, v51) = (value(41), value(42), value(44), value(51));
    lowered(vec![machine(
        1,
        vec![boolean(1), i32(2), i32(3)],
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                Vec::new(),
                conditional(v1, successor(2, b2, vec![]), successor(3, b3, vec![])),
            ),
            block(
                2,
                Vec::new(),
                vec![integer_constant(20, i32(20), 1)],
                jump(4, b4, vec![v2, v20]),
            ),
            block(
                3,
                Vec::new(),
                vec![integer_constant(30, i32(30), 1)],
                jump(5, b4, vec![v2, v30]),
            ),
            block(
                4,
                vec![i32(41), i32(42)],
                vec![operation(
                    40,
                    i32(44),
                    OperationKind::WrappingIntegerAdd {
                        left: v41,
                        right: v42,
                    },
                )],
                jump(6, b5, vec![v44]),
            ),
            block(5, vec![i32(51)], Vec::new(), return_value(7, v51)),
        ],
    )])
}

/// Sparse-conditional-constant-propagation fixture. One machine, one diamond:
///
/// ```text
/// b1 (entry): v10 = 2; v11 = 3; v12 = v10 + v11; v13 = v12 * v11;
///             v14 = v12 < v13; v15 = !v14; cond v15 ──▶ b2 / b3
/// b2: v20 = v1 + v12 ──[v20]──▶ ┐
/// b3: v30 = v1 + v13 ──[v30]──▶ b4(v41) → return
/// ```
///
/// `v10`/`v11` seed the literal map, so `v12` folds to 5, `v13` folds
/// transitively to 15 through the folded `v12`, `v14` folds to `true`, and
/// `v15` folds to `false` — yet the conditional keeps reading `v15`: branch
/// resolution belongs to the control-flow rule, not this one. `v20`/`v30` mix
/// the literal `v12`/`v13` with the opaque machine parameter `v1` and survive
/// unfolded. Dead producers a fold leaves unreferenced survive as well: their
/// removal belongs to the dead-scalar rule.
pub fn sccp_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4) = (block_id(1), block_id(2), block_id(3), block_id(4));
    let v1 = value(1);
    let (v10, v11, v12, v13, v14, v15) = (
        value(10),
        value(11),
        value(12),
        value(13),
        value(14),
        value(15),
    );
    let (v20, v30, v41) = (value(20), value(30), value(41));
    lowered(vec![machine(
        1,
        vec![i32(1)],
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                vec![
                    integer_constant(10, i32(10), 2),
                    integer_constant(11, i32(11), 3),
                    operation(
                        12,
                        i32(12),
                        OperationKind::WrappingIntegerAdd {
                            left: v10,
                            right: v11,
                        },
                    ),
                    operation(
                        13,
                        i32(13),
                        OperationKind::WrappingIntegerMultiply {
                            left: v12,
                            right: v11,
                        },
                    ),
                    operation(
                        14,
                        boolean(14),
                        OperationKind::IntegerLessThan {
                            left: v12,
                            right: v13,
                        },
                    ),
                    operation(15, boolean(15), OperationKind::BooleanNot { operand: v14 }),
                ],
                conditional(v15, successor(2, b2, vec![]), successor(3, b3, vec![])),
            ),
            block(
                2,
                Vec::new(),
                vec![operation(
                    20,
                    i32(20),
                    OperationKind::WrappingIntegerAdd {
                        left: v1,
                        right: v12,
                    },
                )],
                jump(4, b4, vec![v20]),
            ),
            block(
                3,
                Vec::new(),
                vec![operation(
                    30,
                    i32(30),
                    OperationKind::WrappingIntegerAdd {
                        left: v1,
                        right: v13,
                    },
                )],
                jump(5, b4, vec![v30]),
            ),
            block(4, vec![i32(41)], Vec::new(), return_value(6, v41)),
        ],
    )])
}

/// Dead-scalar fixture sharing the copy fixture's diamond shape:
///
/// ```text
/// b1 (entry): v10 = true (dead); cond v1 ──▶ b2 / b3
/// b2: v20, v21 constants ──[v2, v20, v21]──▶ ┐
/// b3: v30, v31 constants ──[v2, v30, v31]──▶ b4(v41, v42, v43) → add(v41, v42) → return
/// ```
///
/// `v43` is never used, so its incoming values `v21`/`v31` and their producers
/// die transitively; `v41` binds `v2` on every edge (a copy a selected
/// `CopyPropagation` collapses); `v10` is an unused total constant.
pub fn dead_scalar_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4) = (block_id(1), block_id(2), block_id(3), block_id(4));
    let (v1, v2) = (value(1), value(2));
    let (v20, v21, v30, v31) = (value(20), value(21), value(30), value(31));
    let (v41, v42, v44) = (value(41), value(42), value(44));
    lowered(vec![machine(
        1,
        vec![boolean(1), i32(2)],
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                vec![operation(
                    10,
                    boolean(10),
                    OperationKind::BooleanConstant { value: true },
                )],
                conditional(v1, successor(2, b2, vec![]), successor(3, b3, vec![])),
            ),
            block(
                2,
                Vec::new(),
                vec![
                    integer_constant(20, i32(20), 2),
                    integer_constant(21, i32(21), 3),
                ],
                jump(4, b4, vec![v2, v20, v21]),
            ),
            block(
                3,
                Vec::new(),
                vec![
                    integer_constant(30, i32(30), 4),
                    integer_constant(31, i32(31), 5),
                ],
                jump(5, b4, vec![v2, v30, v31]),
            ),
            block(
                4,
                vec![i32(41), i32(42), i32(43)],
                vec![operation(
                    40,
                    i32(44),
                    OperationKind::WrappingIntegerAdd {
                        left: v41,
                        right: v42,
                    },
                )],
                return_value(6, v44),
            ),
        ],
    )])
}

/// Control-flow-cleanup fixture. One machine with a literal-selected branch
/// whose untaken arm owns a deeper stranded region, plus one trivially
/// conditional branch whose arms agree:
///
/// ```text
/// b1 (entry): v10 = true; v11 = true; cond v10 ──e2:t──▶ b2 ──e3:f──▶ b3
/// b2: v20 = 1; cond v11 ──e4:t──▶ b4 ──e5:f──▶ b4
/// b3: v30 = 2 ──e6──▶ b5
/// b5: v50 = 3 ──e7──▶ b4
/// b4: v40 = 9; return v40 ──e8
/// ```
///
/// The `v10` fold keeps edge `e2`, strands `b3` and its sole successor `b5`,
/// and removes both. The `v11` fold keeps edge `e4` and strands nothing:
/// every path already reached `b4`. Dead scalar rows the cleanup leaves behind
/// — `v10`, `v11`, `v20` — stay for the dead-scalar rule.
pub fn control_flow_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4, b5) = (
        block_id(1),
        block_id(2),
        block_id(3),
        block_id(4),
        block_id(5),
    );
    let (v10, v11) = (value(10), value(11));
    lowered(vec![machine(
        1,
        Vec::new(),
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                vec![
                    operation(
                        10,
                        boolean(10),
                        OperationKind::BooleanConstant { value: true },
                    ),
                    operation(
                        11,
                        boolean(11),
                        OperationKind::BooleanConstant { value: true },
                    ),
                ],
                conditional(v10, successor(2, b2, vec![]), successor(3, b3, vec![])),
            ),
            block(
                2,
                Vec::new(),
                vec![integer_constant(20, i32(20), 1)],
                conditional(v11, successor(4, b4, vec![]), successor(5, b4, vec![])),
            ),
            block(
                3,
                Vec::new(),
                vec![integer_constant(30, i32(30), 2)],
                jump(6, b5, vec![]),
            ),
            block(
                4,
                Vec::new(),
                vec![integer_constant(40, i32(40), 9)],
                return_value(8, value(40)),
            ),
            block(
                5,
                Vec::new(),
                vec![integer_constant(50, i32(50), 3)],
                jump(7, b4, vec![]),
            ),
        ],
    )])
}

/// A conditional whose false arm carries a qualification coercion row on the
/// exact untaken edge. Dropping `e3` would orphan the coercion, so cleanup
/// keeps the conditional — the edge identity is evidence, not just a choice.
///
/// ```text
/// b1 (entry): v10 = true; cond v10 ──e2:t──▶ b2 ──e3:f──▶ b3(v31)
///             coercion (e3, arg 0): v5{i32,d1} → v31{i32,∅}
/// b2: ──e4──▶ b4
/// b3: ──e6──▶ b4
/// b4: v40 = 9; return v40
/// ```
pub fn coercion_edge_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4) = (block_id(1), block_id(2), block_id(3), block_id(4));
    let (v5, v10, v31) = (value(5), value(10), value(31));
    let mut lowered = lowered(vec![machine(
        1,
        vec![qualified_i32(5, 1)],
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                vec![operation(
                    10,
                    boolean(10),
                    OperationKind::BooleanConstant { value: true },
                )],
                conditional(v10, successor(2, b2, vec![]), successor(3, b3, vec![v5])),
            ),
            block(2, Vec::new(), Vec::new(), jump(4, b4, vec![])),
            block(3, vec![i32(31)], Vec::new(), jump(6, b4, vec![])),
            block(
                4,
                Vec::new(),
                vec![integer_constant(40, i32(40), 9)],
                return_value(8, value(40)),
            ),
        ],
    )]);
    lowered.semantic_module.scalar_qualifications = terminal_psi::ScalarQualificationCatalog {
        domains: vec![terminal_psi::ScalarDomainDeclaration {
            id: semantic_vocabulary::ScalarDomainId::new(1).unwrap(),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: "d1".to_string(),
            carrier: i32_type(),
        }],
        sets: vec![terminal_psi::ScalarQualificationSet {
            id: semantic_vocabulary::ScalarQualificationSetId::new(1),
            domains: vec![semantic_vocabulary::ScalarDomainId::new(1).unwrap()],
        }],
        coercions: vec![terminal_psi::ScalarQualificationCoercion {
            machine: machine_id(1),
            edge: edge(3),
            argument_ordinal: 0,
            source: v5,
            destination: v31,
        }],
    };
    lowered
}

/// The untaken edge is clean, but a coercion inside the stranded region keeps
/// `b3`'s outgoing edge alive: removing `b3` and `b5` would orphan it, so the
/// whole conditional survives.
///
/// ```text
/// b1 (entry): v10 = true; cond v10 ──e2:t──▶ b2 ──e3:f──▶ b3
/// b2: ──e4──▶ b4
/// b3: v30 = 2 ──e6──▶ b5(v51)   coercion (e6, arg 0): v30{i32,∅} → v51{i32,d1}
/// b5: ──e7──▶ b4
/// b4: v40 = 9; return v40
/// ```
pub fn coercion_region_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4, b5) = (
        block_id(1),
        block_id(2),
        block_id(3),
        block_id(4),
        block_id(5),
    );
    let (v10, v30, v51) = (value(10), value(30), value(51));
    let mut lowered = lowered(vec![machine(
        1,
        Vec::new(),
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                vec![operation(
                    10,
                    boolean(10),
                    OperationKind::BooleanConstant { value: true },
                )],
                conditional(v10, successor(2, b2, vec![]), successor(3, b3, vec![])),
            ),
            block(2, Vec::new(), Vec::new(), jump(4, b4, vec![])),
            block(
                3,
                Vec::new(),
                vec![integer_constant(30, i32(30), 2)],
                jump(6, b5, vec![v30]),
            ),
            block(
                4,
                Vec::new(),
                vec![integer_constant(40, i32(40), 9)],
                return_value(8, value(40)),
            ),
            block(
                5,
                vec![qualified_i32(51, 1)],
                Vec::new(),
                jump(7, b4, vec![]),
            ),
        ],
    )]);
    lowered.semantic_module.scalar_qualifications = terminal_psi::ScalarQualificationCatalog {
        domains: vec![terminal_psi::ScalarDomainDeclaration {
            id: semantic_vocabulary::ScalarDomainId::new(1).unwrap(),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: "d1".to_string(),
            carrier: i32_type(),
        }],
        sets: vec![terminal_psi::ScalarQualificationSet {
            id: semantic_vocabulary::ScalarQualificationSetId::new(1),
            domains: vec![semantic_vocabulary::ScalarDomainId::new(1).unwrap()],
        }],
        coercions: vec![terminal_psi::ScalarQualificationCoercion {
            machine: machine_id(1),
            edge: edge(6),
            argument_ordinal: 0,
            source: v30,
            destination: v51,
        }],
    };
    lowered
}

fn qualified_i32(ordinal: u64, set: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: semantic_vocabulary::ScalarQualificationSetId::new(set),
        id: ValueId::new(ordinal).unwrap(),
        scalar_type: i32_type(),
    }
}

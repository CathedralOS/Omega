//! State-specialization verified fixtures: a dispatch state whose incoming
//! edges supply a mixed constant/non-constant state argument, and a dispatch
//! whose every incoming edge is constant and must decline.

use super::super::VerifiedPsiOptimizationUnit;
use super::admission::verified_unit;
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    ValueId,
};
use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, SuccessorEdge, TerminalMachineResult,
    Terminator, ValueDeclaration,
};

fn empty_contract(id: u64) -> terminal_psi::MachineContract {
    use semantic_vocabulary::ContractId;
    terminal_psi::MachineContract {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        id: ContractId::new(id).unwrap(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn module(machine: terminal_psi::TerminalMachine) -> terminal_psi::TerminalModule {
    use terminal_psi::VocabularyMarker;
    terminal_psi::TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    }
}

fn boolean(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Boolean,
    }
}

fn unsigned64(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    }
}

fn return_unit(edge: u64) -> Terminator {
    Terminator::ReturnUnit {
        edge: EdgeId::new(edge).unwrap(),
        trivial_affine_discards: Vec::new(),
    }
}

/// A single-`Conditional` dispatch block parameter is bound by three incoming
/// edges across both admitted predecessor shapes: `pred_true`'s unconditional
/// jump and `pred_cond`'s `when_true` conditional arm each supply the proven
/// literal `flag`, while `pred_false` supplies the machine parameter `seed`,
/// which no sparse constant lattice can prove. Exactly the two
/// constant-supplied edges specialize; the conditional's sibling arm keeps
/// the still-variable route through `pred_false` untouched.
///
/// A second machine carries the non-Boolean family member: `idispatch` reads
/// its own `u64` state parameter through an in-block `istate < 4` comparison,
/// with `ipred`'s jump binding it to the proven literal `3` and `ientry`'s
/// `when_false` arm binding it to the still-variable machine parameter
/// `ivar`. The constant edge resolves the comparison's `when_true` arm while
/// the variable arm keeps `idispatch` reachable.
///
/// `entry -[seed]-> {pred_true, pred_cond} ; pred_cond -[flag]-> dispatch
/// and `-[ ]-> pred_false ; {pred_true, pred_false} -[flag|seed]-> dispatch
/// -> {yes, no}`.
/// `ientry -[iseed]-> {ipred | idispatch[ivar]} ; ipred -[3]-> idispatch
/// -> {iyes, ino}`.
///
/// A third pair carries the result specialization: `caller`'s `ccall_pred`
/// block executes `Call{leaf}` and binds `cstate` to the call's scalar
/// result — `leaf` carries exactly one `Return` over a proven `true`
/// constant, so the delivered argument is a proven constant even though the
/// sparse lattice leaves every call result overdefined. `cvar_pred` keeps
/// the still-variable `cseed` and the dispatch reachable.
/// `centry -[cseed]-> {ccall_pred | cdispatch[cseed]} ; ccall_pred -[leaf()]->
/// cdispatch -> {cyes, cno}`.
pub(in crate::pass_manager::tests) fn verified_dispatch_specialization_unit()
-> VerifiedPsiOptimizationUnit {
    let (entry, pred_true, pred_cond, pred_false, dispatch, yes, no) = (
        BlockId::new(5_602).unwrap(),
        BlockId::new(5_603).unwrap(),
        BlockId::new(5_604).unwrap(),
        BlockId::new(5_605).unwrap(),
        BlockId::new(5_606).unwrap(),
        BlockId::new(5_607).unwrap(),
        BlockId::new(5_608).unwrap(),
    );
    let seed = ValueId::new(5_609).unwrap();
    let flag = ValueId::new(5_610).unwrap();
    let state = ValueId::new(5_612).unwrap();
    let gate = ValueId::new(5_611).unwrap();
    let successor = |edge: u64, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let jump = |edge: u64, arguments| Terminator::Jump {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target: dispatch,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let boolean_machine = terminal_psi::TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(5_601).unwrap(),
        attachment: None,
        parameters: vec![boolean(5_609)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        contract: empty_contract(5_699),
        blocks: vec![
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(5_613).unwrap(),
                    result: OperationResult::Scalar(boolean(5_610)),
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Conditional {
                    condition: seed,
                    when_true: successor(5_614, pred_true, Vec::new()),
                    when_false: successor(5_615, pred_cond, vec![seed]),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: pred_true,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: jump(5_616, vec![flag]),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: pred_cond,
                parameters: vec![boolean(5_611)],
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition: gate,
                    when_true: successor(5_617, dispatch, vec![flag]),
                    when_false: successor(5_618, pred_false, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: pred_false,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: jump(5_619, vec![seed]),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: dispatch,
                parameters: vec![boolean(5_612)],
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition: state,
                    when_true: successor(5_620, yes, Vec::new()),
                    when_false: successor(5_621, no, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: yes,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(5_622),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: no,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(5_623),
            },
        ],
    };
    let mut module = module(boolean_machine);
    module.machines.push(integer_comparison_machine());
    let (caller, leaf) = call_result_pair();
    module.machines.push(caller);
    module.machines.push(leaf);
    verified_unit(&module, &terminal_verifier::ProofBundle::default())
}

/// The result-specialization family member: `caller`'s `ccall_pred` block
/// executes `Call{leaf}` producing the scalar result `cres`, then jumps to
/// `cdispatch` binding `cstate` to that result. `leaf` contains exactly one
/// `Return` whose value is a proven `true` constant, so the callee's own
/// lattice proves the delivered argument constant even though the caller's
/// lattice leaves `cres` overdefined. `cvar_pred`'s jump binds `cstate` to
/// the still-variable machine parameter `cseed`, keeping the dispatch
/// reachable — exactly one incoming edge specializes.
fn call_result_pair() -> (terminal_psi::TerminalMachine, terminal_psi::TerminalMachine) {
    let (centry, ccall_pred, cvar_pred, cdispatch, cyes, cno) = (
        BlockId::new(5_902).unwrap(),
        BlockId::new(5_903).unwrap(),
        BlockId::new(5_904).unwrap(),
        BlockId::new(5_905).unwrap(),
        BlockId::new(5_906).unwrap(),
        BlockId::new(5_907).unwrap(),
    );
    let (cseed, cres, cstate) = (
        ValueId::new(5_910).unwrap(),
        ValueId::new(5_911).unwrap(),
        ValueId::new(5_912).unwrap(),
    );
    let leaf_id = MachineId::new(5_950).unwrap();
    let successor = |edge: u64, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let jump = |edge: u64, arguments| Terminator::Jump {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target: cdispatch,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let caller = terminal_psi::TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(5_900).unwrap(),
        attachment: None,
        parameters: vec![boolean(5_910)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: centry,
        contract: empty_contract(5_998),
        blocks: vec![
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: centry,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition: cseed,
                    when_true: successor(5_913, ccall_pred, Vec::new()),
                    when_false: successor(5_914, cvar_pred, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: ccall_pred,
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(5_915).unwrap(),
                    result: OperationResult::Scalar(boolean(5_911)),
                    kind: OperationKind::Call {
                        callee: leaf_id,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                }],
                terminator: jump(5_916, vec![cres]),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: cvar_pred,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: jump(5_917, vec![cseed]),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: cdispatch,
                parameters: vec![boolean(5_912)],
                operations: Vec::new(),
                terminator: Terminator::Conditional {
                    condition: cstate,
                    when_true: successor(5_918, cyes, Vec::new()),
                    when_false: successor(5_919, cno, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: cyes,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(5_920),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: cno,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(5_921),
            },
        ],
    };
    let leaf = terminal_psi::TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: leaf_id,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(boolean(5_955)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(5_951).unwrap(),
        contract: empty_contract(5_999),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(5_951).unwrap(),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(5_952).unwrap(),
                result: OperationResult::Scalar(boolean(5_956)),
                kind: OperationKind::BooleanConstant { value: true },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(5_953).unwrap(),
                value: ValueId::new(5_956).unwrap(),
                cleanup_actions: Vec::new(),
            },
        }],
    };
    (caller, leaf)
}

/// The integer state-argument family member: `idispatch` reads its own `u64`
/// parameter `istate` through an in-block `istate < 4` comparison — the
/// `[IntegerConstant, IntegerLessThan, Conditional]` dispatch shape. `ipred`'s
/// unconditional jump binds `istate` to the proven literal `3` and resolves
/// the `when_true` arm; `ientry`'s `when_false` conditional arm binds it to
/// the machine parameter `ivar`, which no sparse constant lattice can prove,
/// so `idispatch` stays reachable.
fn integer_comparison_machine() -> terminal_psi::TerminalMachine {
    let (ientry, ipred, idispatch, iyes, ino) = (
        BlockId::new(5_801).unwrap(),
        BlockId::new(5_802).unwrap(),
        BlockId::new(5_803).unwrap(),
        BlockId::new(5_804).unwrap(),
        BlockId::new(5_805).unwrap(),
    );
    let (iseed, ivar, ilit, istate, ibound, icmp) = (
        ValueId::new(5_806).unwrap(),
        ValueId::new(5_807).unwrap(),
        ValueId::new(5_808).unwrap(),
        ValueId::new(5_809).unwrap(),
        ValueId::new(5_810).unwrap(),
        ValueId::new(5_811).unwrap(),
    );
    let successor = |edge: u64, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    terminal_psi::TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(5_800).unwrap(),
        attachment: None,
        parameters: vec![boolean(5_806), unsigned64(5_807)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: ientry,
        contract: empty_contract(5_898),
        blocks: vec![
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: ientry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(5_812).unwrap(),
                    result: OperationResult::Scalar(unsigned64(5_808)),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(3),
                    },
                }],
                terminator: Terminator::Conditional {
                    condition: iseed,
                    when_true: successor(5_815, ipred, Vec::new()),
                    when_false: successor(5_816, idispatch, vec![ivar]),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: ipred,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    edge: EdgeId::new(5_817).unwrap(),
                    target: idispatch,
                    arguments: vec![ilit],
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: idispatch,
                parameters: vec![unsigned64(5_809)],
                operations: vec![
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_813).unwrap(),
                        result: OperationResult::Scalar(unsigned64(5_810)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(4),
                        },
                    },
                    Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_814).unwrap(),
                        result: OperationResult::Scalar(boolean(5_811)),
                        kind: OperationKind::IntegerLessThan {
                            left: istate,
                            right: ibound,
                        },
                    },
                ],
                terminator: Terminator::Conditional {
                    condition: icmp,
                    when_true: successor(5_818, iyes, Vec::new()),
                    when_false: successor(5_819, ino, Vec::new()),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: iyes,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(5_820),
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: ino,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: return_unit(5_821),
            },
        ],
    }
}

/// Every incoming edge of `dispatch` supplies a proven Boolean literal, so the
/// specialization would orphan the state and the pass must decline — the
/// boundary leg at this family's exact admission edge.
pub(in crate::pass_manager::tests) fn verified_dispatch_all_constant_unit()
-> VerifiedPsiOptimizationUnit {
    let (entry, dispatch, yes, no) = (
        BlockId::new(5_702).unwrap(),
        BlockId::new(5_703).unwrap(),
        BlockId::new(5_704).unwrap(),
        BlockId::new(5_705).unwrap(),
    );
    let flag = ValueId::new(5_706).unwrap();
    let state = ValueId::new(5_707).unwrap();
    let successor = |edge: u64, target| SuccessorEdge {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge: EdgeId::new(edge).unwrap(),
        target,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    verified_unit(
        &module(terminal_psi::TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(5_701).unwrap(),
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
            entry,
            contract: empty_contract(5_799),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(5_708).unwrap(),
                        result: OperationResult::Scalar(boolean(5_706)),
                        kind: OperationKind::BooleanConstant { value: true },
                    }],
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        edge: EdgeId::new(5_709).unwrap(),
                        target: dispatch,
                        arguments: vec![flag],
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: dispatch,
                    parameters: vec![boolean(5_707)],
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: state,
                        when_true: successor(5_710, yes),
                        when_false: successor(5_711, no),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: yes,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_712),
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: no,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: return_unit(5_713),
                },
            ],
        }),
        &terminal_verifier::ProofBundle::default(),
    )
}

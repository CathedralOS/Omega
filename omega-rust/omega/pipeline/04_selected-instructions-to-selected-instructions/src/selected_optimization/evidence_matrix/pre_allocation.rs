//! Per-rule coverage matrix for the pre-allocation rules — every catalog
//! member exercises the stage's public entrance axes on a compiler-produced
//! staged input.

use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};

use terminal_psi::{
    Block, Operation, OperationKind, OperationResult, SuccessorEdge, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration,
};
use terminal_verifier::ProofBundle;

use optimization_core::Optimization;

use super::{
    CoverageCase, assert_disabled_leg, assert_downstream_replay_leg, assert_empty_selection_leg,
    assert_exact_selection_leg, assert_unsupported_composition_leg, machine, module,
    optimize_selected_instructions, returning_block, selections_of, staged,
};

fn unsigned(width: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, width).unwrap())
}

fn decl(id: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type,
        qualifications: Default::default(),
    }
}

fn u64_decl(id: u64) -> ValueDeclaration {
    decl(id, unsigned(64))
}

fn bool_decl(id: u64) -> ValueDeclaration {
    decl(id, ScalarType::Boolean)
}

fn scalar_op(id: u64, result: ValueDeclaration, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(result),
        kind,
    }
}

fn const_bool(id: u64, value: bool) -> Operation {
    scalar_op(
        id,
        bool_decl(id + 1_000),
        OperationKind::BooleanConstant { value },
    )
}

fn const_u64(id: u64, value: u128) -> Operation {
    scalar_op(
        id,
        u64_decl(id + 1_000),
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(value),
        },
    )
}

// -- positive fixtures -------------------------------------------------------

/// `if const { a } else { b }` — constant boolean feeding the conditional.
/// `if const { a } else { b }` — constant boolean feeding the conditional.
fn constant_branch(_pressure: usize) -> (TerminalModule, ProofBundle) {
    let condition = ValueId::new(10_202).unwrap();
    let machine = machine(
        9_200,
        Vec::new(),
        TerminalMachineResult::Scalar(u64_decl(9_211)),
        BlockId::new(9_201).unwrap(),
        vec![
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(9_201).unwrap(),
                parameters: Vec::new(),
                operations: vec![const_bool(9_202, true)],
                terminator: Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        edge: EdgeId::new(9_205).unwrap(),
                        target: BlockId::new(9_203).unwrap(),
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        edge: EdgeId::new(9_206).unwrap(),
                        target: BlockId::new(9_204).unwrap(),
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            },
            returning_block(
                9_203,
                9_207,
                vec![const_u64(9_208, 1)],
                ValueId::new(10_208).unwrap(),
            ),
            returning_block(
                9_204,
                9_212,
                vec![const_u64(9_213, 2)],
                ValueId::new(10_213).unwrap(),
            ),
        ],
        9_210,
        Vec::new(),
    );
    (module(machine), ProofBundle::default())
}

fn constant_boolean_branch(_pressure: usize) -> (TerminalModule, ProofBundle) {
    // `3 < 5` materialized as a boolean value — a `MaterializeBoolean*`
    // instruction whose two literal-register operands fold to a constant.
    let machine = machine(
        9_200,
        Vec::new(),
        TerminalMachineResult::Scalar(bool_decl(9_211)),
        BlockId::new(9_201).unwrap(),
        vec![returning_block(
            9_201,
            9_207,
            vec![
                const_u64(9_214, 3),
                const_u64(9_216, 5),
                scalar_op(
                    9_217,
                    bool_decl(9_218),
                    OperationKind::IntegerLessThan {
                        left: ValueId::new(10_214).unwrap(),
                        right: ValueId::new(10_216).unwrap(),
                    },
                ),
            ],
            ValueId::new(9_218).unwrap(),
        )],
        9_210,
        Vec::new(),
    );
    (module(machine), ProofBundle::default())
}

/// `widen(widen(x))` — U16 → U32 → U64 chained widenings.
fn redundant_extension(_pressure: usize) -> (TerminalModule, ProofBundle) {
    let parameter = decl(9_310, unsigned(16));
    let mid = ValueId::new(9_302).unwrap();
    let result = ValueId::new(9_303).unwrap();
    let machine = machine(
        9_300,
        vec![parameter],
        TerminalMachineResult::Scalar(u64_decl(9_311)),
        BlockId::new(9_301).unwrap(),
        vec![returning_block(
            9_301,
            9_305,
            vec![
                scalar_op(
                    9_302,
                    decl(9_302, unsigned(32)),
                    OperationKind::IntegerWiden {
                        operand: parameter.id,
                    },
                ),
                scalar_op(
                    9_303,
                    u64_decl(9_303),
                    OperationKind::IntegerWiden { operand: mid },
                ),
            ],
            result,
        )],
        9_304,
        Vec::new(),
    );
    (module(machine), ProofBundle::default())
}

/// `x == c` twice feeding `BooleanEqual` — the second compare is redundant.
fn redundant_compare(_pressure: usize) -> (TerminalModule, ProofBundle) {
    let parameter = u64_decl(9_410);
    let first = ValueId::new(9_402).unwrap();
    let second = ValueId::new(9_404).unwrap();
    let machine = machine(
        9_400,
        vec![parameter],
        TerminalMachineResult::Scalar(bool_decl(9_515)),
        BlockId::new(9_401).unwrap(),
        vec![returning_block(
            9_401,
            9_406,
            vec![
                const_u64(9_403, 5),
                scalar_op(
                    9_407,
                    bool_decl(9_402),
                    OperationKind::IntegerEqual {
                        left: parameter.id,
                        right: ValueId::new(10_403).unwrap(),
                    },
                ),
                scalar_op(
                    9_408,
                    bool_decl(9_404),
                    OperationKind::IntegerEqual {
                        left: parameter.id,
                        right: ValueId::new(10_403).unwrap(),
                    },
                ),
                scalar_op(
                    9_409,
                    bool_decl(9_405),
                    OperationKind::BooleanEqual {
                        left: first,
                        right: second,
                    },
                ),
            ],
            ValueId::new(9_405).unwrap(),
        )],
        9_411,
        Vec::new(),
    );
    (module(machine), ProofBundle::default())
}

/// `x < y` then `x < z` where `y` and `z` are two registers pinned to the
/// same literal — the second compare re-publishes flag state the shadow
/// already carried, so the equivalent arm removes it.
fn equivalent_compare(_pressure: usize) -> (TerminalModule, ProofBundle) {
    let parameter = u64_decl(9_510);
    let machine = machine(
        9_500,
        vec![parameter],
        TerminalMachineResult::Scalar(bool_decl(9_515)),
        BlockId::new(9_501).unwrap(),
        vec![returning_block(
            9_501,
            9_506,
            vec![
                const_u64(9_503, 5),
                const_u64(9_507, 5),
                scalar_op(
                    9_508,
                    bool_decl(9_502),
                    OperationKind::IntegerLessThan {
                        left: parameter.id,
                        right: ValueId::new(10_503).unwrap(),
                    },
                ),
                scalar_op(
                    9_509,
                    bool_decl(9_504),
                    OperationKind::IntegerLessThan {
                        left: parameter.id,
                        right: ValueId::new(10_507).unwrap(),
                    },
                ),
            ],
            ValueId::new(9_504).unwrap(),
        )],
        9_511,
        Vec::new(),
    );
    (module(machine), ProofBundle::default())
}

/// A param passed through a block-param edge then returned — the same-block
/// copy this rule removes.
fn same_block_copy(_pressure: usize) -> (TerminalModule, ProofBundle) {
    let parameter = u64_decl(9_610);
    let target = BlockId::new(9_603).unwrap();
    let machine = machine(
        9_600,
        vec![parameter],
        TerminalMachineResult::Scalar(u64_decl(9_611)),
        BlockId::new(9_601).unwrap(),
        vec![
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(9_601).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    edge: EdgeId::new(9_605).unwrap(),
                    target,
                    arguments: vec![parameter.id],
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    residual_affine_discards: Vec::new(),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: target,
                parameters: vec![u64_decl(9_606)],
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(9_607).unwrap(),
                    value: ValueId::new(9_606).unwrap(),
                },
            },
        ],
        9_612,
        Vec::new(),
    );
    (module(machine), ProofBundle::default())
}

/// `x < 5` whose bool is never consumed — the closest compiler-emitted
/// shape to a dead compare; the flag publication is still observed by the
/// bool materialization, so the rule declines.
fn dead_compare(_pressure: usize) -> (TerminalModule, ProofBundle) {
    let parameter = u64_decl(9_710);
    let machine = machine(
        9_700,
        vec![parameter],
        TerminalMachineResult::Scalar(u64_decl(9_711)),
        BlockId::new(9_701).unwrap(),
        vec![returning_block(
            9_701,
            9_706,
            vec![
                const_u64(9_713, 5),
                scalar_op(
                    9_717,
                    bool_decl(9_718),
                    OperationKind::IntegerLessThan {
                        left: parameter.id,
                        right: ValueId::new(10_713).unwrap(),
                    },
                ),
                const_u64(9_721, 7),
            ],
            ValueId::new(10_721).unwrap(),
        )],
        9_712,
        Vec::new(),
    );
    (module(machine), ProofBundle::default())
}

// -- the matrix --------------------------------------------------------------

macro_rules! pre_allocation_cases {
    ($($name:ident : $selection:expr, $sibling:expr, $positive:expr, $variant:ident ;)*) => {
        $(
            mod $name {
                use super::*;
                use crate::selected_optimization::SelectedInstructionOptimizationEvidence;
                use crate::ValidatedPreAllocationTransformation;

                const CASE: CoverageCase = CoverageCase {
                    selection: $selection,
                    sibling: $sibling,
                    positive: $positive,
                    target: target::NativeTarget::linux_x64,
                    pressure: 0,
                    commits,
                };

                fn commits(evidence: &SelectedInstructionOptimizationEvidence) -> bool {
                    match evidence {
                        SelectedInstructionOptimizationEvidence::PreAllocation(run) => run
                            .steps()
                            .iter()
                            .any(|step| matches!(
                                step.transformation(),
                                ValidatedPreAllocationTransformation::$variant(_)
                            )),
                        _ => false,
                    }
                }

                #[test]
                fn exact_selection() {
                    assert_exact_selection_leg(&CASE);
                }
                #[test]
                fn disabled() {
                    assert_disabled_leg(&CASE);
                }
                #[test]
                fn empty_selection() {
                    assert_empty_selection_leg(&CASE);
                }
                #[test]
                fn unsupported_composition() {
                    assert_unsupported_composition_leg(&CASE);
                }
                #[test]
                fn downstream_replay() {
                    assert_downstream_replay_leg(&CASE);
                }
            }
        )*
    };
}

pre_allocation_cases! {
    same_block_copy_removal:
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        Optimization::SelectedConstantBooleanFoldV1,
        same_block_copy,
        CopyRemoval;
    redundant_extension_removal:
        Optimization::SelectedRedundantExtensionRemovalV1,
        Optimization::SelectedConstantBooleanFoldV1,
        redundant_extension,
        RedundantExtension;
    constant_boolean_fold:
        Optimization::SelectedConstantBooleanFoldV1,
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        constant_boolean_branch,
        ConstantBoolean;
    constant_branch_fold:
        Optimization::SelectedConstantBranchFoldV1,
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        constant_branch,
        ConstantBranch;
    redundant_compare_removal:
        Optimization::SelectedRedundantCompareRemovalV1,
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        redundant_compare,
        RedundantCompare;
    equivalent_compare_removal:
        Optimization::SelectedEquivalentCompareRemovalV1,
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        equivalent_compare,
        EquivalentCompare;
}

/// `SelectedAddressOffsetFoldV1` cannot commit through the public stage
/// entrance. Every `AddressOffset` producer pairs its result with a consumer
/// the rule's admission table does not cover: byte-field reads and stores feed
/// `Load8Indexed`, `ByteViewAddress`, and `CopyBytes`; local-storage argument
/// copies and edge-transfer address bridges feed slot-form `Store64`; call
/// staging feeds `Call` operands. Every admitted consumer shape —
/// `Load8`/`Load16`/`Load32`/`Load64`, `Store`, `AddressOffset` — instead
/// reads a byte-0 place pointer or a frame address that is never an
/// `AddressOffset` result register, so the producer/consumer pair the rule
/// folds never appears in compiler-emitted plans. The positive and replay
/// axes are therefore pinned by this disposition rather than a commit.
mod address_offset_fold {
    use super::*;
    use crate::ValidatedPreAllocationTransformation;
    use crate::selected_optimization::SelectedInstructionOptimizationEvidence;

    const CASE: CoverageCase = CoverageCase {
        selection: Optimization::SelectedAddressOffsetFoldV1,
        sibling: Optimization::SelectedConstantBooleanFoldV1,
        positive: same_block_copy,
        target: target::NativeTarget::linux_x64,
        pressure: 0,
        commits,
    };

    fn commits(evidence: &SelectedInstructionOptimizationEvidence) -> bool {
        match evidence {
            SelectedInstructionOptimizationEvidence::PreAllocation(run) => {
                run.steps().iter().any(|step| {
                    matches!(
                        step.transformation(),
                        ValidatedPreAllocationTransformation::AddressFold(_)
                    )
                })
            }
            _ => false,
        }
    }

    #[test]
    fn disabled() {
        assert_disabled_leg(&CASE);
    }
    #[test]
    fn empty_selection() {
        assert_empty_selection_leg(&CASE);
    }
    #[test]
    fn unsupported_composition() {
        assert_unsupported_composition_leg(&CASE);
    }

    /// The rule runs and declines: the stage entrance succeeds but no
    /// `AddressFold` step can commit on a compiler-emitted plan.
    #[test]
    fn exact_selection_is_pinned_unreachable() {
        let (module, proof) = (CASE.positive)(CASE.pressure);
        let selections = selections_of(&[CASE.selection]);
        let output =
            optimize_selected_instructions(staged(&module, &proof, &selections, (CASE.target)()))
                .expect("the stage entrance must accept the workload");
        let evidence = output.into_replayed_evidence().expect("replay succeeds");
        assert!(
            !(CASE.commits)(&evidence),
            "SelectedAddressOffsetFoldV1 must not commit: no emitted consumer              reads an AddressOffset result through operand 0",
        );
    }

    /// Replayed evidence likewise carries no fold step.
    #[test]
    fn downstream_replay_is_pinned_unreachable() {
        let (module, proof) = (CASE.positive)(CASE.pressure);
        let selections = selections_of(&[CASE.selection]);
        let output =
            optimize_selected_instructions(staged(&module, &proof, &selections, (CASE.target)()))
                .expect("the stage entrance must accept the workload");
        let replayed = output
            .into_replayed_evidence()
            .expect("replay of an empty evidence run succeeds");
        assert!(
            !(CASE.commits)(&replayed),
            "replay cannot mint an AddressFold step the proposal never committed",
        );
    }
}

/// `SelectedDeadCompareRemovalV1` cannot commit through the public stage
/// entrance under solo selection. Every emitted compare is observed: a
/// bool-result compare publishes flags its own `MaterializeBoolean*` reads,
/// and a branch-fused compare publishes flags its `ConditionalBranch*`
/// terminator reads. The dead-flag shape the rule removes — a publication
/// whose every reader is gone — arises only after the `constant_boolean` /
/// `constant_branch` cluster folds those readers away, a composition the
/// exact-selection axis (this member alone) cannot express. The positive
/// and replay axes are therefore pinned by this disposition rather than a
/// commit.
mod dead_compare_removal {
    use super::*;
    use crate::ValidatedPreAllocationTransformation;
    use crate::selected_optimization::SelectedInstructionOptimizationEvidence;

    const CASE: CoverageCase = CoverageCase {
        selection: Optimization::SelectedDeadCompareRemovalV1,
        sibling: Optimization::SelectedSameBlockCopyI64RemovalV1,
        positive: dead_compare,
        target: target::NativeTarget::linux_x64,
        pressure: 0,
        commits,
    };

    fn commits(evidence: &SelectedInstructionOptimizationEvidence) -> bool {
        match evidence {
            SelectedInstructionOptimizationEvidence::PreAllocation(run) => {
                run.steps().iter().any(|step| {
                    matches!(
                        step.transformation(),
                        ValidatedPreAllocationTransformation::DeadCompare(_)
                    )
                })
            }
            _ => false,
        }
    }

    #[test]
    fn disabled() {
        assert_disabled_leg(&CASE);
    }
    #[test]
    fn empty_selection() {
        assert_empty_selection_leg(&CASE);
    }
    #[test]
    fn unsupported_composition() {
        assert_unsupported_composition_leg(&CASE);
    }

    /// The rule runs and declines: the compare's flags remain observed by
    /// its bool materialization, so no `DeadCompare` step commits.
    #[test]
    fn exact_selection_is_pinned_unreachable() {
        let (module, proof) = (CASE.positive)(CASE.pressure);
        let selections = selections_of(&[CASE.selection]);
        let output =
            optimize_selected_instructions(staged(&module, &proof, &selections, (CASE.target)()))
                .expect("the stage entrance must accept the workload");
        let evidence = output.into_replayed_evidence().expect("replay succeeds");
        assert!(
            !(CASE.commits)(&evidence),
            "SelectedDeadCompareRemovalV1 must not commit: the compare's flags              are still observed by its own materialization",
        );
    }

    /// Replayed evidence likewise carries no removal step.
    #[test]
    fn downstream_replay_is_pinned_unreachable() {
        let (module, proof) = (CASE.positive)(CASE.pressure);
        let selections = selections_of(&[CASE.selection]);
        let output =
            optimize_selected_instructions(staged(&module, &proof, &selections, (CASE.target)()))
                .expect("the stage entrance must accept the workload");
        let replayed = output
            .into_replayed_evidence()
            .expect("replay of an empty evidence run succeeds");
        assert!(
            !(CASE.commits)(&replayed),
            "replay cannot mint a DeadCompare step the proposal never committed",
        );
    }
}

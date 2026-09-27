//! Per-rule evidence matrices through the public stage entrance.
//!
//! `optimize_selected_instructions` is the stage's one public entrance: every
//! selectable rule the stage catalog executes — the selected-lowering and
//! pre-allocation slices — exercises each applicable axis here on a
//! compiler-admitted workload. A `TerminalModule` fixture plus its proof
//! bundle travel the real chain: `terminal_codec` encodes the module,
//! `terminal_psi_to_abstract_operations::lower_artifact` admits it,
//! `optimize_abstract_operations` publishes the optimized plan under the
//! case's exact selections, `lower_optimized_to_target_operations` and
//! `stage_optimized_instruction_selection` produce the staged input, and the
//! entrance under test consumes it. The staged input retains the authored
//! selections as stage-owned data, so the case's exact, sibling-disabled,
//! empty, and unsupported-composition legs all reach the same admission
//! logic production callers hit.
//!
//! Axes per case:
//! - exact selection: the rule alone is selected and must commit on the
//!   positive workload — literal-fold rules attribute through the run's
//!   resolved `LiteralFoldPolicy` (each catalog member owns one bit) plus a
//!   committed step; pre-allocation rules attribute through the committed
//!   step's `ValidatedPreAllocationTransformation` variant, which names the
//!   rule family directly.
//! - disabled sibling: a sibling selection that provably cannot fire on the
//!   workload runs instead; the positive workload publishes untransformed.
//! - empty selection: no selected member, so the entrance returns the
//!   identity evidence without consulting a rule.
//! - unsupported composition: the rule mixed with an executor-less catalog
//!   slice's member rejects before any rule runs.
//! - downstream replay: `into_replayed_evidence` independently replays the
//!   produced evidence and must re-derive it.
//!
//! Per-rule positive, negative, boundary, budget, corruption, and
//! fixed-point coverage stays in each family directory's own tests; this
//! matrix owns the legs that bind every executable rule to the public stage
//! entrance itself.

use optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    Proposition, ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::ProofBundle;

use crate::optimize_selected_instructions;
use crate::selected_optimization::SelectedInstructionOptimizationEvidence;
use target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions;

mod literal_folds;
mod pre_allocation;

/// One exact selection member's entrance-matrix case data.
pub(super) struct CoverageCase {
    /// The exact `Optimization` selection member under test.
    pub selection: Optimization,
    /// Workload where the selection commits: a module + its proof bundle,
    /// sized for `pressure` live values across the boundary under test
    /// (the literal's definition for literal-fold fixtures).
    pub positive: fn(usize) -> (TerminalModule, ProofBundle),
    /// The native target the workload is staged against — the register
    /// environment's home count decides where the allocator's first home
    /// failure lands, so cases pair `target` with a `pressure` sized for
    /// that environment.
    pub target: fn() -> target::NativeTarget,
    /// Pressure values the positive workload spans across its boundary —
    /// one fewer than the target's usable GPR homes so the boundary
    /// definition is the allocator's first home failure.
    pub pressure: usize,
    /// A sibling `Optimization` selection in the same executed phase that
    /// provably declines the positive workload, isolating this member's
    /// disable policy.
    pub sibling: Optimization,
    /// The rule family's executor-agnostic commit predicate: literal-fold
    /// cases check the resolved policy bit; pre-allocation cases check the
    /// committed transformation variant.
    pub commits: fn(&SelectedInstructionOptimizationEvidence) -> bool,
}

pub(super) fn selections_of(selections: &[Optimization]) -> OptimizationSelections {
    OptimizationSelections::new(selections.iter().copied()).unwrap()
}

/// The per-pass budget the matrix admits: generous enough that no axis tests
/// exhaustion; the per-family suites own measured budget legs.
pub(super) fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 4096, 100, 100).unwrap()
}

/// Encode a module + proof bundle, admit the artifact through the real
/// optimizer boundary, and return the verified input carrier — the same
/// admission the compiler performs on checked source.
pub(super) fn verified_input(
    module: &TerminalModule,
    proof: &ProofBundle,
) -> terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput {
    let semantic = terminal_codec::encode_module(module).unwrap();
    let proof = terminal_codec::encode_proof_section(module, proof).unwrap();
    terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap()
}

/// Drive a fabricated module through the real pipeline to the staged input
/// `optimize_selected_instructions` consumes.
pub(super) fn staged(
    module: &TerminalModule,
    proof: &ProofBundle,
    selections: &OptimizationSelections,
    target: target::NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let input = verified_input(module, proof);
    let plan = abstract_operations_to_abstract_operations::optimize_abstract_operations(
        input,
        selections,
        &selections.project_psi(),
        budget(),
    )
    .unwrap();
    let target_operations =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            plan,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(target),
        )
        .unwrap();
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    target_operations_to_selected_instructions::stage_optimized_instruction_selection(
        target_operations,
        environment,
    )
    .unwrap()
}

/// Exact selection: the member alone is selected and commits on the positive
/// workload through the public entrance; `into_replayed_evidence` also runs
/// the custody replay before the evidence is inspected.
pub(super) fn assert_exact_selection_leg(case: &CoverageCase) {
    let (module, proof) = (case.positive)(case.pressure);
    let selections = selections_of(&[case.selection]);
    let staged_input = staged(&module, &proof, &selections, (case.target)());
    let staged_dump = format!("{:#?}", staged_input);
    let output = match optimize_selected_instructions(staged_input) {
        Ok(output) => output,
        Err(error) => panic!(
            "optimize failed for {:?}: {error:?}\nstaged input:\n{}",
            case.selection, staged_dump
        ),
    };
    let program = format!("{:#?}", output.program());
    let evidence = output.into_replayed_evidence().unwrap();
    assert!(
        (case.commits)(&evidence),
        "exact selection {:?} did not commit on its positive workload\nprogram:\n{}",
        case.selection,
        program,
    );
}

/// Disabled policy: under a same-phase sibling selection the positive
/// workload publishes without this member's commit.
pub(super) fn assert_disabled_leg(case: &CoverageCase) {
    let (module, proof) = (case.positive)(case.pressure);
    let sibling = selections_of(&[case.sibling]);
    let staged = staged(&module, &proof, &sibling, (case.target)());
    let output = optimize_selected_instructions(staged).unwrap();
    assert!(
        !(case.commits)(&output.into_replayed_evidence().unwrap()),
        "disabled {:?} rewrote the positive workload",
        case.selection,
    );
}

/// Empty selection: no member selected, so the entrance returns identity
/// evidence on the positive workload — the rollback receipt's empty
/// effective set shares this route.
pub(super) fn assert_empty_selection_leg(case: &CoverageCase) {
    let (module, proof) = (case.positive)(case.pressure);
    let staged = staged(
        &module,
        &proof,
        &OptimizationSelections::default(),
        (case.target)(),
    );
    let output = optimize_selected_instructions(staged).unwrap();
    assert!(
        matches!(
            output.into_replayed_evidence().unwrap(),
            SelectedInstructionOptimizationEvidence::Identity(_)
        ),
        "empty selection produced non-identity evidence for {:?}",
        case.selection,
    );
}

/// Unsupported composition: the member mixed with an executor-less catalog
/// slice's rule rejects before any rule executes.
pub(super) fn assert_unsupported_composition_leg(case: &CoverageCase) {
    let (module, proof) = (case.positive)(case.pressure);
    let selections = selections_of(&[
        case.selection,
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
    ]);
    let staged = staged(&module, &proof, &selections, (case.target)());
    let result = optimize_selected_instructions(staged);
    assert!(
        matches!(
            result.unwrap_err(),
            super::SelectedInstructionOptimizationError::UnsupportedComposition
        ),
        "unsupported composition did not reject for {:?}",
        case.selection,
    );
}

/// Downstream replay: the independent replay of the produced evidence
/// re-derives it; the exact leg already exercises this route, and this leg
/// repeats it explicitly so a replay regression cannot hide inside the
/// positive-leg assertion.
pub(super) fn assert_downstream_replay_leg(case: &CoverageCase) {
    let (module, proof) = (case.positive)(case.pressure);
    let selections = selections_of(&[case.selection]);
    let output =
        optimize_selected_instructions(staged(&module, &proof, &selections, (case.target)()))
            .unwrap();
    let replayed = output.into_replayed_evidence().unwrap();
    assert!(
        (case.commits)(&replayed),
        "downstream replay did not re-derive the commit for {:?}",
        case.selection,
    );
}

// ---------------------------------------------------------------------------
// Fixture support: minimal TerminalModule builders mirroring the pass-manager
// evidence-matrix fixtures, admitted through the real artifact boundary.

pub(super) fn module(machine: TerminalMachine) -> TerminalModule {
    TerminalModule {
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

pub(super) fn machine(
    id: u64,
    parameters: Vec<terminal_psi::ValueDeclaration>,
    result: TerminalMachineResult,
    entry: BlockId,
    blocks: Vec<Block>,
    contract: u64,
    requires: Vec<Proposition>,
) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(id).unwrap(),
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
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: ContractId::new(contract).unwrap(),
            crash_routes: Vec::new(),
            requires,
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

/// One block of operations returning `value`.
pub(super) fn returning_block(
    id: u64,
    edge: u64,
    operations: Vec<Operation>,
    value: ValueId,
) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: BlockId::new(id).unwrap(),
        parameters: Vec::new(),
        operations,
        terminator: Terminator::Return {
            cleanup_actions: Vec::new(),
            edge: EdgeId::new(edge).unwrap(),
            value,
        },
    }
}

/// Literal-fold attribution: the run committed at least one fold under the
/// case's isolated selection, and replay re-validates the same evidence.
pub(super) fn literal_fold_commits(evidence: &SelectedInstructionOptimizationEvidence) -> bool {
    match evidence {
        SelectedInstructionOptimizationEvidence::LiteralFolds(run) => run
            .steps()
            .iter()
            .any(|step| step.fold().receipt().applied_count() > 0),
        _ => false,
    }
}

/// Number of live pressure values carried across the literal's definition —
/// `x` plus this many materialized constants seats every linux_x64 GPR home
/// at the literal's definition, so the literal's own definition is the
/// allocator's first home failure and its latest-ending interval makes it
/// the `Incoming` victim.
pub(super) const PRESSURE_PARAMS_X64: usize = 8;
/// Same on linux_arm64 — the aarch64 physical model exposes 19 usable GPR
/// homes in the baseline environment, so the pressure set is wider.
pub(super) const PRESSURE_PARAMS_AARCH64: usize = 18;

/// The operands a `pressure_machine` tail closure consumes: the entry
/// parameter and the materialized literal's value.
pub(super) struct PressureOperands {
    pub x: ValueId,
    pub literal: ValueId,
}

/// Pressure layout shared by every literal-fold positive fixture: `x` plus
/// `pressure` materialized constants whose intervals span the literal's
/// definition; the literal materializes once every home is taken, and the
/// tail decides where its interval ends — the caller's fold op, a boundary
/// copy, or another single use that lands last, so the allocator picks the
/// literal as the `Incoming` rematerialization victim. `s0 = c0 + c1`,
/// `s_i = s_(i-1) + c_i`, and the final sum consumes `x` so the parameter
/// stays live across the literal's definition even when the tail never
/// reads it.
///
/// Value-id plan: pressure constants are operations `base + 150 + i`
/// producing `base + 200 + i`; `x` is `base + 30`; the literal is the
/// caller's operation (conventionally `const_op(base + 40, ..)` producing
/// `base + 1040`); the sum chain is operations `base + 80 + i` producing
/// `base + 80 + i - 1` with the final `x`-consuming sum at `base + 130`
/// producing `base + 131`; the fold op is `base + 70` producing `base + 72`;
/// the tail may claim `base + 73+`. Block `base + 1`, return edge
/// `base + 5`, contract `base + 4`.
pub(super) fn pressure_machine(
    base: u64,
    pressure: usize,
    scalar_type: ScalarType,
    literal_operation: Operation,
    requires: Vec<Proposition>,
    tail: impl FnOnce(&mut Vec<Operation>, PressureOperands) -> (TerminalMachineResult, ValueId),
) -> TerminalModule {
    // One `x` parameter; pressure comes from materialized constants whose
    // intervals span the literal's definition and die in the sum chain.
    // Param entry registers would consume the single per-function spill
    // choice before the literal's definition is reached.
    let parameter_x = ValueDeclaration {
        id: ValueId::new(base + 30).unwrap(),
        scalar_type,
        qualifications: Default::default(),
    };
    let parameters = vec![parameter_x];
    let literal_value = scalar_result(&literal_operation);
    let mut operations = Vec::with_capacity(2 * pressure + 5);
    // c_i = distinct non-boundary materialized constants, all live until the
    // post-literal sum chain.
    let pressure_value = |index: usize| match scalar_type {
        ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed => {
            IntegerValue::Signed(3 + index as i128)
        }
        _ => IntegerValue::Unsigned(3 + index as u128),
    };
    for index in 0..pressure {
        let id = base + 150 + index as u64;
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(id).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(base + 200 + index as u64).unwrap(),
                scalar_type,
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: pressure_value(index),
            },
        });
    }
    // The literal materializes once every home is taken; where its interval
    // ends is the tail's business, but every tail lands it strictly after
    // the last pressure-constant use.
    operations.push(literal_operation);
    let mut accumulator = ValueId::new(base + 80).unwrap();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(base + 81).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: accumulator,
            scalar_type,
            qualifications: Default::default(),
        }),
        kind: OperationKind::WrappingIntegerAdd {
            left: ValueId::new(base + 200).unwrap(),
            right: ValueId::new(base + 201).unwrap(),
        },
    });
    for index in 2..pressure {
        let previous = accumulator;
        accumulator = ValueId::new(base + 80 + index as u64 - 1).unwrap();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(base + 80 + index as u64).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: accumulator,
                scalar_type,
                qualifications: Default::default(),
            }),
            kind: OperationKind::WrappingIntegerAdd {
                left: previous,
                right: ValueId::new(base + 200 + index as u64).unwrap(),
            },
        });
    }
    // The last sum consumes `x`, keeping the parameter's interval live
    // across the literal's definition even for tails that never read it.
    {
        let previous = accumulator;
        accumulator = ValueId::new(base + 131).unwrap();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(base + 130).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: accumulator,
                scalar_type,
                qualifications: Default::default(),
            }),
            kind: OperationKind::WrappingIntegerAdd {
                left: previous,
                right: parameter_x.id,
            },
        });
    }
    let (result, returned) = tail(
        &mut operations,
        PressureOperands {
            x: parameter_x.id,
            literal: literal_value,
        },
    );
    module(machine(
        base,
        parameters,
        result,
        BlockId::new(base + 1).unwrap(),
        vec![returning_block(base + 1, base + 5, operations, returned)],
        base + 4,
        requires,
    ))
}

/// The tail most literal-fold fixtures want: emit the caller's fold op
/// (`fold(x, literal)`, conventionally `pressure_fold_operation` so it is
/// operation `base + 70` producing `base + 72`) and return the folded value
/// through a scalar machine result declared at `base + 75`. The fold is the
/// final operation so the literal's interval ends strictly after every
/// pressure constant's: under any target's home count, the value whose
/// definition fails to seat is the latest-ending one, which keeps the
/// literal the allocator's `Incoming` victim.
pub(super) fn folded_sum_tail(
    base: u64,
    result_scalar: ScalarType,
    fold: impl FnOnce(ValueId, ValueId) -> Operation,
) -> impl FnOnce(&mut Vec<Operation>, PressureOperands) -> (TerminalMachineResult, ValueId) {
    move |operations, operands| {
        let fold = fold(operands.x, operands.literal);
        let fold_value = scalar_result(&fold);
        operations.push(fold);
        (
            TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(base + 75).unwrap(),
                scalar_type: result_scalar,
                qualifications: Default::default(),
            }),
            fold_value,
        )
    }
}

/// `x OP literal` under pressure — the shape every right-operand
/// literal-fold rule triggers on.
pub(super) fn pressure_binary_machine(
    base: u64,
    constant_operation: Operation,
    fold: impl FnOnce(ValueId, ValueId) -> Operation,
    requires: Vec<Proposition>,
    pressure: usize,
) -> TerminalModule {
    pressure_machine(
        base,
        pressure,
        unsigned64_scalar(),
        constant_operation,
        requires,
        folded_sum_tail(base, unsigned64_scalar(), fold),
    )
}

/// The fold operation produced by `pressure_binary_machine` — an operation
/// with id `base + 70` and scalar result `base + 72` of the given kind.
pub(super) fn pressure_fold_operation(
    base: u64,
    scalar_type: ScalarType,
    kind: OperationKind,
) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(base + 70).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(base + 72).unwrap(),
            scalar_type,
            qualifications: Default::default(),
        }),
        kind,
    }
}

pub(super) fn unsigned64_scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

pub(super) fn scalar_result(operation: &Operation) -> ValueId {
    match &operation.result {
        OperationResult::Scalar(declaration) => declaration.id,
        _ => unreachable!("literal-fold fixtures produce scalar results"),
    }
}

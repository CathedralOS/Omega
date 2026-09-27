//! Per-rule coverage matrix for the selected-lowering literal-fold rules —
//! every catalog member exercises the stage's public entrance axes on a
//! compiler-produced staged input.

use semantic_vocabulary::{
    BlockId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, ObligationId, OperationId,
    PlaceId, Proposition, ScalarTerm, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};

use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachineResult, TerminalModule,
    ValueDeclaration,
};
use terminal_verifier::{
    ObligationEvidence, ProofBundle, ReconstructedOperationObligation,
    ReconstructedTerminalObligationOwner,
};

use optimization_core::Optimization;
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};

use super::{
    CoverageCase, PRESSURE_PARAMS_AARCH64, PRESSURE_PARAMS_X64, assert_disabled_leg,
    assert_downstream_replay_leg, assert_empty_selection_leg, assert_exact_selection_leg,
    assert_unsupported_composition_leg, budget, folded_sum_tail, literal_fold_commits,
    optimize_selected_instructions, pressure_binary_machine, pressure_fold_operation,
    pressure_machine, scalar_result, selections_of, staged, verified_input,
};

fn signed(width: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, width).unwrap())
}

fn unsigned(width: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, width).unwrap())
}

fn u64_decl(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type: unsigned(64),
        qualifications: Default::default(),
    }
}

fn bool_decl(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    }
}

fn const_op(id: u64, value: IntegerValue) -> Operation {
    const_op_typed(id, unsigned(64), value)
}

fn const_op_typed(id: u64, scalar_type: ScalarType, value: IntegerValue) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(id + 1_000).unwrap(),
            scalar_type,
            qualifications: Default::default(),
        }),
        kind: OperationKind::IntegerConstant { value },
    }
}

// -- proof-bundle support ----------------------------------------------------

/// The reconstructed obligation row owned by `operation`, so a fixture's
/// certificate is built against exactly the goal proposition and axiom
/// ordering the verifier will later replay — nothing is hand-copied.
fn operation_obligation_row<'a>(
    rows: &'a [ReconstructedOperationObligation],
    operation: OperationId,
) -> &'a ReconstructedOperationObligation {
    rows.iter()
        .find(|row| {
            matches!(
                row.owner,
                ReconstructedTerminalObligationOwner::Operation { operation: owner, .. }
                    if owner == operation
            )
        })
        .unwrap_or_else(|| panic!("no reconstructed obligation for operation {operation:?}"))
}

fn axiom_index(row: &ReconstructedOperationObligation, axiom: &Proposition) -> usize {
    row.semantic_axioms
        .iter()
        .position(|candidate| candidate == axiom)
        .unwrap_or_else(|| {
            panic!(
                "missing axiom {axiom:?}; roster:\n{:?}",
                row.semantic_axioms
            )
        })
}

/// Replace every `ScalarTerm::Value` naming `from` by `to` inside one scalar
/// term — one level of exact-arithmetic composition is walked, matching how
/// canonical goals embed operand terms.
fn substitute_scalar_term(term: &ScalarTerm, from: ValueId, to: &ScalarTerm) -> ScalarTerm {
    if let ScalarTerm::Value { id, .. } = term
        && *id == from
    {
        return to.clone();
    }
    let rebuild = |kind: fn(
        IntegerType,
        ScalarTerm,
        ScalarTerm,
    ) -> Result<ScalarTerm, semantic_vocabulary::PropositionError>,
                   scalar_type: IntegerType,
                   left: &ScalarTerm,
                   right: &ScalarTerm| {
        kind(
            scalar_type,
            substitute_scalar_term(left, from, to),
            substitute_scalar_term(right, from, to),
        )
        .unwrap_or_else(|_| term.clone())
    };
    match term {
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => rebuild(ScalarTerm::exact_integer_add, *scalar_type, left, right),
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => rebuild(
            ScalarTerm::exact_integer_subtract,
            *scalar_type,
            left,
            right,
        ),
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => rebuild(
            ScalarTerm::exact_integer_multiply,
            *scalar_type,
            left,
            right,
        ),
        _ => term.clone(),
    }
}

/// Map every scalar-term leaf of `proposition` through `substitute`.
fn substitute_proposition(
    proposition: &Proposition,
    from: ValueId,
    to: &ScalarTerm,
) -> Proposition {
    let term = |term: &ScalarTerm| substitute_scalar_term(term, from, to);
    match proposition {
        Proposition::Equal(left, right) => Proposition::Equal(term(left), term(right)),
        Proposition::LessThan(left, right) => Proposition::LessThan(term(left), term(right)),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(term(left), term(right)),
        Proposition::Conjunction(parts) => Proposition::Conjunction(
            parts
                .iter()
                .map(|part| substitute_proposition(part, from, to))
                .collect(),
        ),
        Proposition::Disjunction(parts) => Proposition::Disjunction(
            parts
                .iter()
                .map(|part| substitute_proposition(part, from, to))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition(premise, from, to)),
            conclusion: Box::new(substitute_proposition(conclusion, from, to)),
        },
        _ => proposition.clone(),
    }
}

/// Certificate proving `goal` by transporting the literal's published result
/// equation into the goal's operand position, with the literal-substituted
/// proposition proved by the closed-integer-relation primitive. Works for
/// unsigned `1 <= divisor` goals where the divisor is a materialized
/// constant.
fn literal_divisor_certificate(
    row: &ReconstructedOperationObligation,
    scalar_type: ScalarType,
    constant: IntegerValue,
    literal: ValueId,
) -> ProofNode {
    let goal = row.obligation.proposition.clone();
    let integer = match scalar_type {
        ScalarType::Integer(integer) => integer,
        _ => unreachable!("literal-divisor fixtures are integers"),
    };
    let equation = Proposition::Equal(
        ScalarTerm::value(literal, scalar_type),
        ScalarTerm::integer(integer, constant).unwrap(),
    );
    let index = axiom_index(row, &equation);
    let premise = ProofNode {
        conclusion: substitute_proposition(
            &goal,
            literal,
            &ScalarTerm::integer(integer, constant).unwrap(),
        ),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    ProofNode {
        conclusion: goal,
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(premise),
            equalities: vec![ProofNode {
                conclusion: equation,
                rule: ProofRule::SemanticAxiom { index },
            }],
        },
    }
}

/// Signed `-1` divisor: the goal is the nonzero disjunction `[d <= -1, 1 <=
/// d]`; transport proves the negative branch from the closed `-1 <= -1`.
fn minus_one_divisor_certificate(
    row: &ReconstructedOperationObligation,
    scalar_type: ScalarType,
    literal: ValueId,
) -> ProofNode {
    let goal = row.obligation.proposition.clone();
    let integer = match scalar_type {
        ScalarType::Integer(integer) => integer,
        _ => unreachable!(),
    };
    let minus_one = ScalarTerm::integer(integer, IntegerValue::Signed(-1)).unwrap();
    let Proposition::Disjunction(disjuncts) = &goal else {
        panic!("signed divisor goal is a disjunction: {goal:?}")
    };
    let disjunct = disjuncts[0].clone();
    let equation = Proposition::Equal(ScalarTerm::value(literal, scalar_type), minus_one.clone());
    let index = axiom_index(row, &equation);
    ProofNode {
        conclusion: goal,
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(ProofNode {
                conclusion: disjunct.clone(),
                rule: ProofRule::ValueEqualityTransport {
                    premise: Box::new(ProofNode {
                        conclusion: substitute_proposition(&disjunct, literal, &minus_one),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                    }),
                    equalities: vec![ProofNode {
                        conclusion: equation,
                        rule: ProofRule::SemanticAxiom { index },
                    }],
                },
            }),
            index: 0,
        },
    }
}

/// Build the evidence row for the obligation owned by `operation`, with the
/// certificate built by `build` from the reconstructed row.
fn operation_evidence(
    module: &TerminalModule,
    operation: OperationId,
    evidence_id: u64,
    build: impl FnOnce(&ReconstructedOperationObligation) -> ProofNode,
) -> ProofBundle {
    let rows = terminal_verifier::reconstruct_operation_obligations(module).unwrap();
    let row = operation_obligation_row(&rows, operation);
    let evidence = ObligationEvidence {
        obligation: row.obligation.id,
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(evidence_id).unwrap(),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: build(row),
        }),
    };
    ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![evidence],
    }
}

/// Assumption certificate: the contract's `requires` carries exactly the
/// reconstructed goal, so `Assumption{index: 0}` discharges it.
fn assumed_goal_certificate(row: &ReconstructedOperationObligation) -> ProofNode {
    ProofNode {
        conclusion: row.obligation.proposition.clone(),
        rule: ProofRule::Assumption { index: 0 },
    }
}

// -- positive fixtures -------------------------------------------------------

/// `x OP literal` under pressure with an unguarded proof surface.
fn right_literal(
    base: u64,
    kind: impl FnOnce(ValueId, ValueId) -> OperationKind,
    constant: IntegerValue,
    pressure: usize,
) -> (TerminalModule, ProofBundle) {
    (
        pressure_machine(
            base,
            pressure,
            unsigned(64),
            const_op(base + 40, constant),
            Vec::new(),
            folded_sum_tail(base, unsigned(64), move |x, literal| {
                pressure_fold_operation(base, unsigned(64), kind(x, literal))
            }),
        ),
        ProofBundle::default(),
    )
}

/// `literal OP x` under pressure — left-operand rules (zero-minuend and
/// zero-dividend materializations).
fn left_literal(
    base: u64,
    kind: impl FnOnce(ValueId, ValueId) -> OperationKind,
    constant: IntegerValue,
    pressure: usize,
) -> (TerminalModule, ProofBundle) {
    (
        pressure_machine(
            base,
            pressure,
            unsigned(64),
            const_op(base + 40, constant),
            Vec::new(),
            folded_sum_tail(base, unsigned(64), move |x, literal| {
                pressure_fold_operation(base, unsigned(64), kind(literal, x))
            }),
        ),
        ProofBundle::default(),
    )
}

/// `x OP const` where the op carries an obligation discharged by a
/// literal-divisor transport certificate — the contract needs no requires.
fn obligated_right_literal(
    base: u64,
    kind: impl FnOnce(ValueId, ValueId, ObligationId) -> OperationKind,
    constant: IntegerValue,
    pressure: usize,
) -> (TerminalModule, ProofBundle) {
    let scalar_type = unsigned(64);
    let obligation = ObligationId::new(base + 6).unwrap();
    let module = pressure_machine(
        base,
        pressure,
        scalar_type,
        const_op(base + 40, constant),
        Vec::new(),
        folded_sum_tail(base, scalar_type, move |x, literal| {
            pressure_fold_operation(base, scalar_type, kind(x, literal, obligation))
        }),
    );
    let proof = operation_evidence(
        &module,
        OperationId::new(base + 70).unwrap(),
        base + 7,
        |row| {
            literal_divisor_certificate(
                row,
                scalar_type,
                constant,
                ValueId::new(base + 40 + 1_000).unwrap(),
            )
        },
    );
    (module, proof)
}

/// `const OP x` where the op carries an obligation discharged by assuming
/// the goal through the contract's `requires` — the divisor `x` is an
/// in-scope parameter.
fn obligated_left_literal(
    base: u64,
    kind: impl Fn(ValueId, ValueId, ObligationId) -> OperationKind,
    constant: IntegerValue,
    pressure: usize,
) -> (TerminalModule, ProofBundle) {
    let scalar_type = unsigned(64);
    let obligation = ObligationId::new(base + 6).unwrap();
    let fold = |x: ValueId, literal: ValueId| {
        pressure_fold_operation(base, scalar_type, kind(literal, x, obligation))
    };
    // Discover the canonical goal against a requires-free module, then bind
    // the same proposition as the contract's one requires clause.
    let probe = pressure_machine(
        base,
        pressure,
        scalar_type,
        const_op(base + 40, constant),
        Vec::new(),
        folded_sum_tail(base, scalar_type, fold),
    );
    let rows = terminal_verifier::reconstruct_operation_obligations(&probe).unwrap();
    let goal = operation_obligation_row(&rows, OperationId::new(base + 70).unwrap())
        .obligation
        .proposition
        .clone();
    let module = pressure_machine(
        base,
        pressure,
        scalar_type,
        const_op(base + 40, constant),
        vec![goal],
        folded_sum_tail(base, scalar_type, fold),
    );
    let proof = operation_evidence(
        &module,
        OperationId::new(base + 70).unwrap(),
        base + 7,
        assumed_goal_certificate,
    );
    (module, proof)
}

fn add_zero(pressure: usize) -> (TerminalModule, ProofBundle) {
    right_literal(
        7_100,
        |left, right| OperationKind::WrappingIntegerAdd { left, right },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn saturating_add_zero(pressure: usize) -> (TerminalModule, ProofBundle) {
    right_literal(
        7_200,
        |left, right| OperationKind::SaturatingIntegerAdd { left, right },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn saturating_subtract_zero(pressure: usize) -> (TerminalModule, ProofBundle) {
    right_literal(
        7_300,
        |left, right| OperationKind::SaturatingIntegerSubtract { left, right },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn saturating_divide_one(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_right_literal(
        7_400,
        |left, right, obligation| OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(1),
        pressure,
    )
}

fn saturating_divide_zero_dividend(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_left_literal(
        7_500,
        |left, right, obligation| OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn saturating_subtract_zero_minuend(pressure: usize) -> (TerminalModule, ProofBundle) {
    left_literal(
        7_600,
        |left, right| OperationKind::SaturatingIntegerSubtract { left, right },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn saturating_add_upper_bound(pressure: usize) -> (TerminalModule, ProofBundle) {
    right_literal(
        7_700,
        |left, right| OperationKind::SaturatingIntegerAdd { left, right },
        IntegerValue::Unsigned(u64::MAX as u128),
        pressure,
    )
}

fn saturating_subtract_upper_bound_subtrahend(pressure: usize) -> (TerminalModule, ProofBundle) {
    right_literal(
        7_800,
        |left, right| OperationKind::SaturatingIntegerSubtract { left, right },
        IntegerValue::Unsigned(u64::MAX as u128),
        pressure,
    )
}

fn exact_divide_identity(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_right_literal(
        7_900,
        |left, right, obligation| OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(1),
        pressure,
    )
}

fn exact_divide_zero_dividend(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_left_literal(
        8_000,
        |left, right, obligation| OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn wrapping_remainder_one(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_right_literal(
        8_100,
        |left, right, obligation| OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(1),
        pressure,
    )
}

fn wrapping_remainder_zero_dividend(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_left_literal(
        8_200,
        |left, right, obligation| OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn wrapping_remainder_minus_one(pressure: usize) -> (TerminalModule, ProofBundle) {
    // x % -1 over signed operands — the saturating divisor edge.
    let scalar_type = signed(64);
    let obligation = ObligationId::new(8_306).unwrap();
    let literal = const_op_typed(8_340, scalar_type, IntegerValue::Signed(-1));
    let module = pressure_machine(
        8_300,
        pressure,
        scalar_type,
        literal,
        Vec::new(),
        folded_sum_tail(8_300, scalar_type, move |x, k| {
            pressure_fold_operation(
                8_300,
                scalar_type,
                OperationKind::WrappingIntegerRemainder {
                    left: x,
                    right: k,
                    obligation,
                },
            )
        }),
    );
    let proof = operation_evidence(&module, OperationId::new(8_370).unwrap(), 8_307, |row| {
        minus_one_divisor_certificate(row, scalar_type, ValueId::new(8_340 + 1_000).unwrap())
    });
    (module, proof)
}

fn bitwise_and_zero(pressure: usize) -> (TerminalModule, ProofBundle) {
    (
        pressure_binary_machine(
            8_400,
            const_op(8_440, IntegerValue::Unsigned(0)),
            |x, constant| {
                pressure_fold_operation(
                    8_400,
                    unsigned(64),
                    OperationKind::IntegerBitwiseAnd {
                        left: x,
                        right: constant,
                    },
                )
            },
            Vec::new(),
            pressure,
        ),
        ProofBundle::default(),
    )
}

fn bitwise_and_ones(pressure: usize) -> (TerminalModule, ProofBundle) {
    right_literal(
        8_500,
        |left, right| OperationKind::IntegerBitwiseAnd { left, right },
        IntegerValue::Unsigned(u64::MAX as u128),
        pressure,
    )
}

fn bitwise_xor_zero(pressure: usize) -> (TerminalModule, ProofBundle) {
    right_literal(
        8_600,
        |left, right| OperationKind::IntegerBitwiseXor { left, right },
        IntegerValue::Unsigned(0),
        pressure,
    )
}

fn u12_exact_add_immediate(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_assumed(
        8_700,
        |left, right, obligation| OperationKind::ExactIntegerAdd {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(5),
        false,
        pressure,
    )
}

/// Obligated exact-arith fixture with a param operand: the canonical goal
/// names the materialized literal, which is outside contract scope, so
/// `requires` carries the equivalent bound on the parameter alone
/// (`x + c <= MAX` ↔ `x <= MAX - c`; `0 <= x - c` ↔ `c <= x`). The
/// certificate maps the parameter bound plus the literal's published
/// `== c` axiom through the direct `x +/- literal` affine form.
fn obligated_assumed(
    base: u64,
    kind: impl Fn(ValueId, ValueId, ObligationId) -> OperationKind,
    constant: IntegerValue,
    subtract: bool,
    pressure: usize,
) -> (TerminalModule, ProofBundle) {
    let scalar_type = unsigned(64);
    let integer = match scalar_type {
        ScalarType::Integer(integer) => integer,
        _ => unreachable!("obligated-assumed fixtures are integers"),
    };
    let obligation = ObligationId::new(base + 6).unwrap();
    let literal = ValueId::new(base + 40 + 1_000).unwrap();
    let literal_term = ScalarTerm::integer(integer, constant).unwrap();
    let fold = |x: ValueId, literal: ValueId| {
        pressure_fold_operation(base, scalar_type, kind(x, literal, obligation))
    };
    let IntegerValue::Unsigned(magnitude) = constant else {
        unreachable!("obligated-assumed fixtures use unsigned constants")
    };
    let x_term = ScalarTerm::value(ValueId::new(base + 30).unwrap(), scalar_type);
    let root_bound = if subtract {
        Proposition::LessOrEqual(literal_term.clone(), x_term.clone())
    } else {
        Proposition::LessOrEqual(
            x_term.clone(),
            ScalarTerm::integer(
                integer,
                IntegerValue::Unsigned(u64::MAX as u128 - magnitude),
            )
            .unwrap(),
        )
    };
    let module = pressure_machine(
        base,
        pressure,
        scalar_type,
        const_op(base + 40, constant),
        vec![root_bound.clone()],
        folded_sum_tail(base, scalar_type, fold),
    );
    let proof = operation_evidence(
        &module,
        OperationId::new(base + 70).unwrap(),
        base + 7,
        |row| {
            let literal_equation = Proposition::Equal(
                ScalarTerm::value(literal, scalar_type),
                literal_term.clone(),
            );
            ProofNode {
                conclusion: row.obligation.proposition.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(ProofNode {
                        conclusion: Proposition::Conjunction(vec![
                            root_bound.clone(),
                            literal_equation.clone(),
                        ]),
                        rule: ProofRule::ConjunctionIntroduction(vec![
                            ProofNode {
                                conclusion: root_bound.clone(),
                                rule: ProofRule::Assumption { index: 0 },
                            },
                            ProofNode {
                                conclusion: literal_equation.clone(),
                                rule: ProofRule::SemanticAxiom {
                                    index: axiom_index(row, &literal_equation),
                                },
                            },
                        ]),
                    }),
                    witness: terminal_psi::IntegerAffineWitness {
                        root: x_term.clone(),
                        target: if subtract {
                            ScalarTerm::exact_integer_subtract(
                                integer,
                                x_term,
                                ScalarTerm::value(literal, scalar_type),
                            )
                            .unwrap()
                        } else {
                            ScalarTerm::exact_integer_add(
                                integer,
                                x_term,
                                ScalarTerm::value(literal, scalar_type),
                            )
                            .unwrap()
                        },
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                },
            }
        },
    );
    (module, proof)
}

fn u12_exact_subtract_immediate(pressure: usize) -> (TerminalModule, ProofBundle) {
    obligated_assumed(
        8_800,
        |left, right, obligation| OperationKind::ExactIntegerSubtract {
            left,
            right,
            obligation,
        },
        IntegerValue::Unsigned(5),
        true,
        pressure,
    )
}

fn u12_compare_immediate(pressure: usize) -> (TerminalModule, ProofBundle) {
    // `x == literal` feeding a boolean tail — the literal sits at operand 1
    // of the selected compare; `BooleanNot` keeps the compare result live so
    // the machine result is a boolean.
    (
        pressure_machine(
            8_900,
            pressure,
            unsigned(64),
            const_op(8_940, IntegerValue::Unsigned(5)),
            Vec::new(),
            move |operations, operands| {
                let fold = pressure_fold_operation(
                    8_900,
                    ScalarType::Boolean,
                    OperationKind::IntegerEqual {
                        left: operands.x,
                        right: operands.literal,
                    },
                );
                let compared = scalar_result(&fold);
                operations.push(fold);
                let negated = ValueId::new(8_974).unwrap();
                operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(8_973).unwrap(),
                    result: OperationResult::Scalar(bool_decl(8_974)),
                    kind: OperationKind::BooleanNot { operand: compared },
                });
                (TerminalMachineResult::Scalar(bool_decl(8_975)), negated)
            },
        ),
        ProofBundle::default(),
    )
}

fn literal_copy_materialization(pressure: usize) -> (TerminalModule, ProofBundle) {
    // `return literal` under pressure — the boundary return mints the
    // CopyI64 consumer this rule folds.
    (
        pressure_machine(
            9_000,
            pressure,
            unsigned(64),
            const_op(9_040, IntegerValue::Unsigned(7)),
            Vec::new(),
            |_, operands| {
                (
                    TerminalMachineResult::Scalar(u64_decl(9_075)),
                    operands.literal,
                )
            },
        ),
        ProofBundle::default(),
    )
}

fn literal_extension_elimination(pressure: usize) -> (TerminalModule, ProofBundle) {
    // A u32 literal materializing under pressure, widened to u64 — the
    // selected ZeroExtendU32 is the unary consumer the fold eliminates.
    (
        pressure_machine(
            9_100,
            pressure,
            unsigned(64),
            const_op_typed(9_140, unsigned(32), IntegerValue::Unsigned(7)),
            Vec::new(),
            folded_sum_tail(9_100, unsigned(64), move |_, literal| {
                pressure_fold_operation(
                    9_100,
                    unsigned(64),
                    OperationKind::IntegerWiden { operand: literal },
                )
            }),
        ),
        ProofBundle::default(),
    )
}

fn u12_load8_indexed_offset(pressure: usize) -> (TerminalModule, ProofBundle) {
    // A literal index into an established byte-sequence literal: the place's
    // static extent emits `len == 8` as a semantic axiom, so `index < len`
    // discharges without a dominating guard and the literal's only use is
    // the Load8Indexed operand the u12 rule folds.
    let bytes_type = StructuralTypeId::new(9_203).unwrap();
    let view_place = PlaceId::new(9_204).unwrap();
    let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let u64_scalar = ScalarType::Integer(u64_integer);
    let literal_value = ValueId::new(10_340).unwrap();
    let length_value = ValueId::new(9_212).unwrap();
    let sum_result = ValueId::new(9_260).unwrap();
    let byte_value = ValueId::new(9_270).unwrap();
    let wide_value = ValueId::new(9_272).unwrap();
    let final_value = ValueId::new(9_274).unwrap();

    let parameter_x = u64_decl(9_600);
    let mut operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_210).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: view_place,
                bytes: vec![0; 8],
                qualifications: Vec::new(),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_211).unwrap(),
            result: OperationResult::Scalar(u64_decl(9_212)),
            kind: OperationKind::ByteSequenceLength { source: view_place },
        },
    ];
    let const_value = |index: usize| ValueId::new(9_220 + index as u64).unwrap();
    for index in 0..pressure {
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_300 + index as u64).unwrap(),
            result: OperationResult::Scalar(u64_decl(9_220 + index as u64)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(3 + index as u128),
            },
        });
    }
    operations.push(const_op(9_340, IntegerValue::Unsigned(5)));
    debug_assert_eq!(scalar_result(operations.last().unwrap()), literal_value);
    let mut accumulator = ValueId::new(9_350).unwrap();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_349).unwrap(),
        result: OperationResult::Scalar(u64_decl(9_350)),
        kind: OperationKind::WrappingIntegerAdd {
            left: const_value(0),
            right: const_value(1),
        },
    });
    for index in 2..pressure {
        let previous = accumulator;
        accumulator = ValueId::new(9_350 + index as u64 - 1).unwrap();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_350 + index as u64).unwrap(),
            result: OperationResult::Scalar(u64_decl(9_350 + index as u64 - 1)),
            kind: OperationKind::WrappingIntegerAdd {
                left: previous,
                right: const_value(index),
            },
        });
    }
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_598).unwrap(),
        result: OperationResult::Scalar(u64_decl(9_260)),
        kind: OperationKind::WrappingIntegerAdd {
            left: accumulator,
            right: parameter_x.id,
        },
    });
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_369).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: byte_value,
            scalar_type: unsigned(8),
            qualifications: Default::default(),
        }),
        kind: OperationKind::ByteSequenceRead {
            source: view_place,
            index: literal_value,
            length: length_value,
            obligation: ObligationId::new(9_380).unwrap(),
        },
    });
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_371).unwrap(),
        result: OperationResult::Scalar(u64_decl(9_272)),
        kind: OperationKind::IntegerWiden {
            operand: byte_value,
        },
    });
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_373).unwrap(),
        result: OperationResult::Scalar(u64_decl(9_274)),
        kind: OperationKind::WrappingIntegerAdd {
            left: wide_value,
            right: sum_result,
        },
    });

    let mut machine = super::machine(
        9_200,
        vec![parameter_x],
        TerminalMachineResult::Scalar(u64_decl(9_275)),
        BlockId::new(9_206).unwrap(),
        vec![super::returning_block(
            9_206,
            9_207,
            operations,
            final_value,
        )],
        9_205,
        Vec::new(),
    );
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: view_place,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: bytes_type,
        },
    }];
    let mut module = super::module(machine);
    module.structural_types = vec![StructuralTypeDeclaration {
        id: bytes_type,
        identity: "Bytes".to_string(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: None,
        }),
    }];

    let proof = operation_evidence(&module, OperationId::new(9_369).unwrap(), 9_381, |row| {
        let literal_term = ScalarTerm::value(literal_value, u64_scalar);
        let length_term = ScalarTerm::value(length_value, u64_scalar);
        let five = ScalarTerm::integer(u64_integer, IntegerValue::Unsigned(5)).unwrap();
        let eight = ScalarTerm::integer(u64_integer, IntegerValue::Unsigned(8)).unwrap();
        ProofNode {
            conclusion: row.obligation.proposition.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                endpoint: 0,
                equality: Box::new(ProofNode {
                    conclusion: Proposition::Equal(literal_term, five.clone()),
                    rule: ProofRule::SemanticAxiom {
                        index: axiom_index(
                            row,
                            &Proposition::Equal(
                                ScalarTerm::value(literal_value, u64_scalar),
                                five.clone(),
                            ),
                        ),
                    },
                }),
                relation: Box::new(ProofNode {
                    conclusion: Proposition::LessThan(five.clone(), length_term.clone()),
                    rule: ProofRule::IntegerOrderSubstitution {
                        endpoint: 1,
                        equality: Box::new(ProofNode {
                            conclusion: Proposition::Equal(length_term, eight.clone()),
                            rule: ProofRule::SemanticAxiom {
                                index: axiom_index(
                                    row,
                                    &Proposition::Equal(
                                        ScalarTerm::value(length_value, u64_scalar),
                                        eight.clone(),
                                    ),
                                ),
                            },
                        }),
                        relation: Box::new(ProofNode {
                            conclusion: Proposition::LessThan(five, eight),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                        }),
                    },
                }),
            },
        }
    });
    (module, proof)
}

fn u12_byte_view_address_offset(pressure: usize) -> (TerminalModule, ProofBundle) {
    // A literal `start` on a first-level subslice: the emit feeds `start`'s
    // register to both the ByteViewAddress operand and the view-length
    // ExactSubtractI64, so under spill pressure the action derivation sees
    // two uses and rejects with FutureUseMismatch — no single-use literal
    // operand for the u12 ByteViewAddress rule can exist through this route.
    let bytes_type = StructuralTypeId::new(9_403).unwrap();
    let source_place = PlaceId::new(9_404).unwrap();
    let subslice_place = PlaceId::new(9_405).unwrap();
    let u64_integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let u64_scalar = ScalarType::Integer(u64_integer);
    let length_value = ValueId::new(9_412).unwrap();
    let end_value = ValueId::new(10_440).unwrap();
    let start_value = ValueId::new(10_442).unwrap();

    let parameter_x = u64_decl(9_600);
    let mut operations = vec![
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_410).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: source_place,
                bytes: vec![0; 8],
                qualifications: Vec::new(),
            },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_411).unwrap(),
            result: OperationResult::Scalar(u64_decl(9_412)),
            kind: OperationKind::ByteSequenceLength {
                source: source_place,
            },
        },
        const_op(9_440, IntegerValue::Unsigned(8)),
    ];
    for index in 0..pressure {
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_450 + index as u64).unwrap(),
            result: OperationResult::Scalar(u64_decl(9_420 + index as u64)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(3 + index as u128),
            },
        });
    }
    // `start` materializes last so it is the highest-virtual-register
    // contender ending at the subslice — the spill victim.
    operations.push(const_op(9_442, IntegerValue::Unsigned(5)));
    debug_assert_eq!(scalar_result(operations.last().unwrap()), start_value);
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_469).unwrap(),
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: subslice_place,
            structural_type: bytes_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::ByteSequenceSubslice {
            source: source_place,
            start: start_value,
            end: end_value,
            length: length_value,
            obligation: ObligationId::new(9_480).unwrap(),
        },
    });
    // Consume the pressure constants + parameter after the subslice so every
    // contender's interval spans the literal's definition.
    let mut accumulator = ValueId::new(9_490).unwrap();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_489).unwrap(),
        result: OperationResult::Scalar(u64_decl(9_490)),
        kind: OperationKind::WrappingIntegerAdd {
            left: ValueId::new(9_420).unwrap(),
            right: ValueId::new(9_421).unwrap(),
        },
    });
    for index in 2..pressure {
        let previous = accumulator;
        accumulator = ValueId::new(9_490 + index as u64 - 1).unwrap();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(9_490 + index as u64).unwrap(),
            result: OperationResult::Scalar(u64_decl(9_490 + index as u64 - 1)),
            kind: OperationKind::WrappingIntegerAdd {
                left: previous,
                right: ValueId::new(9_420 + index as u64).unwrap(),
            },
        });
    }
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_515).unwrap(),
        result: OperationResult::Scalar(u64_decl(9_516)),
        kind: OperationKind::ByteSequenceLength {
            source: subslice_place,
        },
    });
    let final_value = ValueId::new(9_520).unwrap();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(9_519).unwrap(),
        result: OperationResult::Scalar(u64_decl(9_520)),
        kind: OperationKind::WrappingIntegerAdd {
            left: accumulator,
            right: ValueId::new(9_516).unwrap(),
        },
    });

    let mut machine = super::machine(
        9_400,
        vec![parameter_x],
        TerminalMachineResult::Scalar(u64_decl(9_521)),
        BlockId::new(9_406).unwrap(),
        vec![super::returning_block(
            9_406,
            9_407,
            operations,
            final_value,
        )],
        9_401,
        Vec::new(),
    );
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: source_place,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: bytes_type,
            },
        },
        StructuralPlaceDeclaration {
            id: subslice_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(9_469).unwrap(),
                structural_type: bytes_type,
            },
        },
    ];
    let mut module = super::module(machine);
    module.structural_types = vec![StructuralTypeDeclaration {
        id: bytes_type,
        identity: "Bytes".to_string(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: None,
        }),
    }];

    let proof = operation_evidence(&module, OperationId::new(9_469).unwrap(), 9_481, |row| {
        let Proposition::Conjunction(legs) = &row.obligation.proposition else {
            panic!("subslice obligation is a two-leg conjunction")
        };
        let start_term = ScalarTerm::value(start_value, u64_scalar);
        let end_term = ScalarTerm::value(end_value, u64_scalar);
        let length_term = ScalarTerm::value(length_value, u64_scalar);
        let five = ScalarTerm::integer(u64_integer, IntegerValue::Unsigned(5)).unwrap();
        let eight = ScalarTerm::integer(u64_integer, IntegerValue::Unsigned(8)).unwrap();
        let substitute_leg = |leg: usize,
                              endpoint: usize,
                              equality_axiom: &Proposition,
                              inner: Box<ProofNode>|
         -> ProofNode {
            ProofNode {
                conclusion: legs[leg].clone(),
                rule: ProofRule::IntegerOrderSubstitution {
                    endpoint,
                    equality: Box::new(ProofNode {
                        conclusion: equality_axiom.clone(),
                        rule: ProofRule::SemanticAxiom {
                            index: axiom_index(row, equality_axiom),
                        },
                    }),
                    relation: inner,
                },
            }
        };
        let leg0 = substitute_leg(
            0,
            0,
            &Proposition::Equal(start_term.clone(), five.clone()),
            Box::new(ProofNode {
                conclusion: Proposition::LessOrEqual(five.clone(), end_term.clone()),
                rule: ProofRule::IntegerOrderSubstitution {
                    endpoint: 1,
                    equality: Box::new(ProofNode {
                        conclusion: Proposition::Equal(end_term.clone(), eight.clone()),
                        rule: ProofRule::SemanticAxiom {
                            index: axiom_index(
                                row,
                                &Proposition::Equal(end_term.clone(), eight.clone()),
                            ),
                        },
                    }),
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::LessOrEqual(five.clone(), eight.clone()),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                    }),
                },
            }),
        );
        let leg1 = substitute_leg(
            1,
            0,
            &Proposition::Equal(end_term.clone(), eight.clone()),
            Box::new(ProofNode {
                conclusion: Proposition::LessOrEqual(eight.clone(), length_term.clone()),
                rule: ProofRule::IntegerOrderSubstitution {
                    endpoint: 1,
                    equality: Box::new(ProofNode {
                        conclusion: Proposition::Equal(length_term.clone(), eight.clone()),
                        rule: ProofRule::SemanticAxiom {
                            index: axiom_index(
                                row,
                                &Proposition::Equal(length_term, eight.clone()),
                            ),
                        },
                    }),
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::LessOrEqual(eight.clone(), eight),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                    }),
                },
            }),
        );
        ProofNode {
            conclusion: row.obligation.proposition.clone(),
            rule: ProofRule::ConjunctionIntroduction(vec![leg0, leg1]),
        }
    });
    (module, proof)
}

// -- the matrix --------------------------------------------------------------

macro_rules! literal_fold_cases {
    ($($name:ident : $selection:expr, $sibling:expr, $positive:path, $target:expr, $pressure:expr ;)*) => {
        $(
            mod $name {
                use super::*;

                const CASE: CoverageCase = CoverageCase {
                    selection: $selection,
                    sibling: $sibling,
                    positive: $positive,
                    target: $target,
                    pressure: $pressure,
                    commits: literal_fold_commits,
                };

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

literal_fold_cases! {
    wrapping_add_zero_identity_copy:
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        Optimization::SelectedIncomingSaturatingAddZeroIdentityCopy,
        add_zero, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
    saturating_add_zero_identity_copy:
        Optimization::SelectedIncomingSaturatingAddZeroIdentityCopy,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        saturating_add_zero, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    saturating_subtract_zero_identity_copy:
        Optimization::SelectedIncomingSaturatingSubtractZeroIdentityCopy,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        saturating_subtract_zero, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    saturating_divide_one_identity_copy:
        Optimization::SelectedIncomingSaturatingDivideOneIdentityCopy,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        saturating_divide_one, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    saturating_divide_zero_dividend_zero_materialization:
        Optimization::SelectedIncomingSaturatingDivideZeroDividendZeroMaterialization,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        saturating_divide_zero_dividend, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    saturating_subtract_zero_minuend_zero_materialization:
        Optimization::SelectedIncomingSaturatingSubtractZeroMinuendZeroMaterialization,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        saturating_subtract_zero_minuend, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    saturating_add_upper_bound_materialization:
        Optimization::SelectedIncomingSaturatingAddUpperBoundMaterialization,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        saturating_add_upper_bound, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    saturating_subtract_upper_bound_subtrahend_zero_materialization:
        Optimization::SelectedIncomingSaturatingSubtractUpperBoundSubtrahendZeroMaterialization,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        saturating_subtract_upper_bound_subtrahend, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    exact_divide_identity_copy:
        Optimization::SelectedIncomingExactDivideIdentityCopy,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        exact_divide_identity, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    exact_divide_zero_dividend_zero_materialization:
        Optimization::SelectedIncomingExactDivideZeroDividendZeroMaterialization,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        exact_divide_zero_dividend, target::NativeTarget::linux_arm64, PRESSURE_PARAMS_AARCH64;
    bitwise_and_zero_materialization:
        Optimization::SelectedIncomingBitwiseAndZeroMaterialization,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        bitwise_and_zero, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
    bitwise_and_ones_identity_copy:
        Optimization::SelectedIncomingBitwiseAndOnesIdentityCopy,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        bitwise_and_ones, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
    bitwise_xor_zero_identity_copy:
        Optimization::SelectedIncomingBitwiseXorZeroIdentityCopy,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        bitwise_xor_zero, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
    u12_exact_add_immediate:
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        u12_exact_add_immediate, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
    u12_exact_subtract_immediate:
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        u12_exact_subtract_immediate, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
    u12_compare_immediate:
        Optimization::SelectedIncomingU12CompareImmediate,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        u12_compare_immediate, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
    literal_copy_materialization:
        Optimization::SelectedIncomingLiteralCopyMaterialization,
        Optimization::SelectedIncomingWrappingAddZeroIdentityCopy,
        literal_copy_materialization, target::NativeTarget::linux_x64, PRESSURE_PARAMS_X64;
}

// -- explicitly justified absent axes ----------------------------------------
//
// `PER-RULE-COVERAGE`: "absent axes need the gate's explicit justified
// disposition." These rules have no executable positive workload through the
// public stage entrance; each disposition pins the refusing wall so a later
// surface change that opens the rule breaks loudly here.
mod unwitnessable {
    use super::*;

    /// The `WrappingRemainder*` folds are unreachable through the public stage
    /// on both ISAs. On aarch64 the remainder result's `Def` operand
    /// early-clobbers over the operand uses, so the literal's live range
    /// carries `EarlyClobberConstraint` and the spill chooser rejects the
    /// candidate with `UnsupportedEarlyClobber`. On x86-64 the dividend and
    /// divisor uses are `fixed_view`-pinned to rax/rdx, which `future_uses`
    /// skips — the literal's only real use is invisible and the candidate
    /// silently declines (no admitted recovery → publish with no fold). The
    /// disabled, empty-selection, unsupported-composition, and
    /// downstream-replay axes exist only around a commit this entrance never
    /// produces, so the whole coverage family is absent by construction.
    macro_rules! remainder_disposition {
        ($name:ident, $positive:path, $selection:expr) => {
            #[test]
            fn $name() {
                let (module, proof) = $positive(PRESSURE_PARAMS_AARCH64);
                let selections = selections_of(&[$selection]);
                let arm64 = optimize_selected_instructions(staged(
                    &module,
                    &proof,
                    &selections,
                    target::NativeTarget::linux_arm64(),
                ));
                assert!(
                    matches!(
                        &arm64,
                        Err(error)
                            if format!("{error:?}").contains("UnsupportedEarlyClobber")
                    ),
                    "arm64 staging must refuse the early-clobbered remainder literal: {arm64:?}",
                );
                let (module, proof) = $positive(PRESSURE_PARAMS_X64);
                let output = optimize_selected_instructions(staged(
                    &module,
                    &proof,
                    &selections,
                    target::NativeTarget::linux_x64(),
                ))
                .expect("x64 staging publishes without the fold");
                assert!(
                    !literal_fold_commits(&output.into_replayed_evidence().unwrap()),
                    "x64 must publish with no remainder fold",
                );
            }
        };
    }

    remainder_disposition!(
        wrapping_remainder_one_has_no_executable_workload,
        wrapping_remainder_one,
        Optimization::SelectedIncomingWrappingRemainderOneZeroMaterialization
    );
    remainder_disposition!(
        wrapping_remainder_zero_dividend_has_no_executable_workload,
        wrapping_remainder_zero_dividend,
        Optimization::SelectedIncomingWrappingRemainderZeroDividendZeroMaterialization
    );
    remainder_disposition!(
        wrapping_remainder_minus_one_has_no_executable_workload,
        wrapping_remainder_minus_one,
        Optimization::SelectedIncomingWrappingRemainderMinusOneZeroMaterialization
    );

    /// `LiteralExtensionElimination` is unreachable through the public stage:
    /// every `ZeroExtend*` consumer reads a narrow-typed operand, so the
    /// materialized literal's register is narrow-typed, while recovery
    /// classification admits only fixed-u64 victims (`is_fixed_unsigned_u64`)
    /// — the candidate declines `UnsupportedScalarType` and the stage
    /// publishes clean with no fold on either ISA.
    #[test]
    fn literal_extension_elimination_has_no_executable_workload() {
        let selections =
            selections_of(&[Optimization::SelectedIncomingLiteralExtensionElimination]);
        for (target, pressure) in [
            (target::NativeTarget::linux_x64(), PRESSURE_PARAMS_X64),
            (target::NativeTarget::linux_arm64(), PRESSURE_PARAMS_AARCH64),
        ] {
            let (module, proof) = literal_extension_elimination(pressure);
            let output =
                optimize_selected_instructions(staged(&module, &proof, &selections, target))
                    .expect("staging publishes without the fold");
            assert!(
                !literal_fold_commits(&output.into_replayed_evidence().unwrap()),
                "no extension fold can commit on {target:?}",
            );
        }
    }

    /// `U12Load8IndexedOffset` is unreachable through the public stage. Every
    /// `Load8Indexed` emitter pairs the index operand with the view's `length`
    /// operand, and the length's register outlives the literal index — the
    /// byte-view retention rows and the memory-access role both re-cite it
    /// after the load. The spill chooser picks the contender with the maximum
    /// exclusive end, so the literal is never the selected victim — the rule
    /// rejects the spill with `UnsupportedVictimRole` on both ISAs. The
    /// static-extent literal place discharges `index < len` without a guard,
    /// which would otherwise need a dominating compare — a second literal use
    /// and the same single-use wall.
    #[test]
    fn u12_load8_indexed_offset_never_victim_of_record() {
        let selections = selections_of(&[Optimization::SelectedIncomingU12Load8IndexedOffset]);
        for (target, pressure) in [
            (target::NativeTarget::linux_x64(), PRESSURE_PARAMS_X64),
            (target::NativeTarget::linux_arm64(), PRESSURE_PARAMS_AARCH64),
        ] {
            let (module, proof) = u12_load8_indexed_offset(pressure);
            let result =
                optimize_selected_instructions(staged(&module, &proof, &selections, target));
            assert!(
                matches!(
                    &result,
                    Err(error) if format!("{error:?}").contains("UnsupportedVictimRole")
                ),
                "the literal index must lose the victim pick on {target:?}: {result:?}",
            );
        }
    }

    /// `U12ByteViewAddressOffset` is unreachable through the public stage.
    /// The only emit path whose operand-1 register could carry a literal is a
    /// first-level `ByteSequenceSubslice`'s `start` — but the same emission
    /// also feeds `start` into the view-length `ExactSubtractI64`, giving the
    /// literal two uses, which `derive_action` rejects with
    /// `FutureUseMismatch`. Write-side emits share the bound-obligation wall:
    /// the index's `index < len` needs a dominating compare (a second use) or
    /// a `len == extent` equation no param-rooted place emits. Element and
    /// runtime-address emits bind computed `scaled`/`O + s` registers, never
    /// bare literals.
    #[test]
    fn u12_byte_view_address_offset_requires_a_second_use() {
        for (target, pressure) in [
            (target::NativeTarget::linux_x64(), PRESSURE_PARAMS_X64),
            (target::NativeTarget::linux_arm64(), PRESSURE_PARAMS_AARCH64),
        ] {
            let (module, proof) = u12_byte_view_address_offset(pressure);
            let selections =
                selections_of(&[Optimization::SelectedIncomingU12ByteViewAddressOffset]);
            let input = verified_input(&module, &proof);
            let plan = abstract_operations_to_abstract_operations::optimize_abstract_operations(
                input,
                &selections,
                &selections.project_psi(),
                budget(),
            )
            .expect("the subslice module optimizes legally");
            let target_operations =
                abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                    plan,
                    abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(
                        target,
                    ),
                )
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let result =
                target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                    target_operations,
                    environment,
                );
            // The scalar-graph custody roster has no arm for a byte-sequence
            // subslice producer at all — a literal-operand ByteViewAddress can
            // never be selected through the public entrance.
            assert!(
                result.is_err(),
                "the subslice route to ByteViewAddress must decline on {target:?}",
            );
        }
    }
}

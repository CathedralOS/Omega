/*
    Denotation and elaboration of bounded `ProofNode` certificates into the
    common mathematical core: propositional connectives, Boolean case analysis,
    value-equality transport, signed binary numerals, and table-driven integer
    arithmetic and order laws.

    Every covered certificate elaborates into a `Term` judgment and is
    re-decided through `verify_mathematical_certificate`.
*/

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use bignum::BigInt;
use mathematical_kernel::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, Level,
    MathematicalCertificate, Signature, Sort, Term, TermArena, TermHandle,
    certificate_assumption_closure, infer_sort, shift, substitute, verify_mathematical_certificate,
    weak_head_normalize_retaining,
};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentTerm, IntegerCarrier, IntegerMathLiteral,
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

use crate::integer_rules::fixed_integer::fixed_native_interval;
use crate::kernel::{KernelError, decide_primitive};
use crate::proof::integer_math_normalization::{
    lift_fixed_integer_relation, propositions_match_under_integer_math_normalization,
};
use crate::proof::{
    AcceptedPremise, AcceptedProofRule, MathematicalJudgmentReceipt, PremiseRoster, ProofError,
    ProofNode, ProofRule, add_order, equality_rules, integer_bound_rules, integer_order_rules,
    order_discreteness, strict_order_transitivity, subtract_antitone, subtract_order,
};
use crate::term_matchers::{
    TermMatcher, exact_integer_add, exact_integer_subtract, is_math_add, is_math_multiply,
    is_math_subtract,
};
use crate::{
    CheckedIntegerCastChain, CheckedIntegerCorrelatedForbiddenRoots, ClosedIntegerEvaluator,
    CorrelatedAffineBranchWitness, IntegerAffineWitness, IntegerCorrelatedForbiddenRootWitness,
};

const MAX_ELABORATION_NODES: u64 = 1 << 16;

mod data_subjects;
#[cfg(test)]
mod law_arithmetic;
mod theorems;
pub use data_subjects::{DataFieldShape, DataSubject, DataSubjectShape};
pub use theorems::{
    BoundedImplicationType, BoundedTheorem, BoundedTheoremError, denote_bounded_implication_type,
    verify_bounded_theorem,
};

pub struct BoundedDenotation {
    pub arena: TermArena,
    pub certificate: MathematicalCertificate,
    pub rules: Vec<AcceptedProofRule>,
    pub assumptions: Vec<AcceptedPremise>,
    pub semantic_axioms: Vec<AcceptedPremise>,
}

impl BoundedDenotation {
    pub fn receipt(&self) -> MathematicalJudgmentReceipt {
        MathematicalJudgmentReceipt::for_certificate(&self.arena, &self.certificate)
    }
}

impl MathematicalJudgmentReceipt {
    /// Emit a receipt only after the kernel checks the complete certificate.
    pub fn verify(
        arena: &mut TermArena,
        certificate: &MathematicalCertificate,
        budget: &mut Budget,
    ) -> Result<Self, CoreError> {
        verify_mathematical_certificate(arena, certificate, budget)?;
        Ok(Self::for_certificate(arena, certificate))
    }

    fn for_certificate(arena: &TermArena, certificate: &MathematicalCertificate) -> Self {
        let sat = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        MathematicalJudgmentReceipt {
            declarations: sat(certificate.signature.len()),
            assumption_closure: sat(certificate_assumption_closure(arena, certificate).len()),
            context_depth: sat(certificate.context.len()),
            arena_slots: sat(arena.len()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedDenotationError {
    Certificate(ProofError),
    Unsupported(&'static str),
    Kernel(CoreError),
    DepthLimitExceeded,
}

crate::debug_display!(BoundedDenotationError);
impl std::error::Error for BoundedDenotationError {}

pub fn denote_bounded_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    denote_bounded_certificate_with_machine_parameters(
        Denotation::new(),
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
        proof,
    )
}

/// Check a closed derivation over a theorem's subjects with each sum input
/// bound to its declaration, so that its case tests compute and a case
/// exhaustion is derived by the sum's recursor.
pub(crate) fn verify_bounded_certificate_over_data(
    context: &PropositionContext,
    goal: &Proposition,
    data_subjects: &[DataSubject],
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    let mut denotation = Denotation::new();
    for subject in data_subjects {
        denotation.declare_data_subject(subject)?;
    }
    let mut denoted = denote_bounded_certificate_with_machine_parameters(
        denotation,
        context,
        goal,
        &[],
        &[],
        &BTreeSet::new(),
        proof,
    )?;
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, budget)
        .map_err(BoundedDenotationError::Kernel)?;
    Ok(denoted)
}

fn denote_bounded_certificate_with_machine_parameters(
    denotation: Denotation,
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    #[cfg(test)]
    crate::bounded_observations::record_denotation();
    let mut elab = Elaboration::new(
        denotation,
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    )?;
    let term = elab.node(proof)?;
    if proof.conclusion != *goal {
        return Err(BoundedDenotationError::Certificate(
            ProofError::CertificateConclusionMismatch,
        ));
    }
    Ok(elab.finish(term))
}

pub fn verify_bounded_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    verify_bounded_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
        proof,
        budget,
    )
}

pub(crate) fn verify_bounded_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
    budget: &mut Budget,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    let mut denoted = denote_bounded_certificate_with_machine_parameters(
        Denotation::new(),
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        proof,
    )?;
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, budget)
        .map_err(BoundedDenotationError::Kernel)?;
    Ok(denoted)
}

// ============================================================================
// Vocabulary enums, table-driven CoreLaw specifications, and scalar helpers
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
}

impl BinaryOp {
    const fn slot(self) -> usize {
        self as usize
    }
    fn eval_big(self, l: &BigInt, r: &BigInt) -> BigInt {
        match self {
            Self::Add => l.add(r),
            Self::Sub => l.sub(r),
            Self::Mul => l.mul(r),
        }
    }
    fn eval_i128(self, l: i128, r: i128) -> Option<i128> {
        match self {
            Self::Add => l.checked_add(r),
            Self::Sub => l.checked_sub(r),
            Self::Mul => l.checked_mul(r),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NumeralCtor {
    Double,
    Odd,
    Negate,
}

impl NumeralCtor {
    const fn slot(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum UnaryOp {
    BitwiseNot,
    Widen(IntegerType),
    ExactCast(IntegerType),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ArithPolicy {
    Exact,
    Wrapping,
    Saturating,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ArithKind {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BitwiseOp {
    And,
    Or,
    Xor,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ShiftOp {
    WrapShl,
    WrapShr,
    ExactShl,
    ExactShr,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RelOp {
    Eq,
    Lt,
    Le,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IntegerOperation {
    Unary(UnaryOp, IntegerType),
    Bitwise(BitwiseOp, IntegerType),
    Shift(ShiftOp, IntegerType, IntegerType),
    Arith(ArithPolicy, ArithKind, IntegerType),
    Rel(RelOp, IntegerType),
}

impl IntegerOperation {
    const fn exact_divide(ty: IntegerType) -> Self {
        Self::Arith(ArithPolicy::Exact, ArithKind::Div, ty)
    }
    const fn arity(self) -> usize {
        if matches!(self, Self::Unary(..)) {
            1
        } else {
            2
        }
    }
    const fn returns_boolean(self) -> bool {
        matches!(self, Self::Rel(..))
    }

    fn from_scalar_term(term: &ScalarTerm) -> Option<(Self, &ScalarTerm, Option<&ScalarTerm>)> {
        use {
            ArithKind as K, ArithPolicy as P, BitwiseOp as B, ScalarTerm as S, ShiftOp as Sh,
            UnaryOp as U,
        };
        Some(match term {
            S::IntegerBitwiseNot {
                scalar_type: t,
                operand: o,
            } => (Self::Unary(U::BitwiseNot, *t), o, None),
            S::IntegerWiden {
                source_type: s,
                target_type: t,
                operand: o,
            } => (Self::Unary(U::Widen(*t), *s), o, None),
            S::IntegerExactCast {
                source_type: s,
                target_type: t,
                operand: o,
            } => (Self::Unary(U::ExactCast(*t), *s), o, None),
            S::IntegerBitwiseAnd {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Bitwise(B::And, *t), l, Some(r)),
            S::IntegerBitwiseOr {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Bitwise(B::Or, *t), l, Some(r)),
            S::IntegerBitwiseXor {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Bitwise(B::Xor, *t), l, Some(r)),
            S::WrappingIntegerShiftLeft {
                value_type: v,
                count_type: c,
                value: l,
                count: r,
            } => (Self::Shift(Sh::WrapShl, *v, *c), l, Some(r)),
            S::WrappingIntegerShiftRight {
                value_type: v,
                count_type: c,
                value: l,
                count: r,
            } => (Self::Shift(Sh::WrapShr, *v, *c), l, Some(r)),
            S::ExactIntegerShiftLeft {
                value_type: v,
                count_type: c,
                value: l,
                count: r,
            } => (Self::Shift(Sh::ExactShl, *v, *c), l, Some(r)),
            S::ExactIntegerShiftRight {
                value_type: v,
                count_type: c,
                value: l,
                count: r,
            } => (Self::Shift(Sh::ExactShr, *v, *c), l, Some(r)),
            S::ExactIntegerDivide {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Exact, K::Div, *t), l, Some(r)),
            S::ExactIntegerRemainder {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Exact, K::Rem, *t), l, Some(r)),
            S::WrappingIntegerAdd {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Wrapping, K::Add, *t), l, Some(r)),
            S::WrappingIntegerSubtract {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Wrapping, K::Sub, *t), l, Some(r)),
            S::WrappingIntegerMultiply {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Wrapping, K::Mul, *t), l, Some(r)),
            S::WrappingIntegerDivide {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Wrapping, K::Div, *t), l, Some(r)),
            S::WrappingIntegerRemainder {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Wrapping, K::Rem, *t), l, Some(r)),
            S::SaturatingIntegerAdd {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Saturating, K::Add, *t), l, Some(r)),
            S::SaturatingIntegerSubtract {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Saturating, K::Sub, *t), l, Some(r)),
            S::SaturatingIntegerMultiply {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Saturating, K::Mul, *t), l, Some(r)),
            S::SaturatingIntegerDivide {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Saturating, K::Div, *t), l, Some(r)),
            S::SaturatingIntegerRemainder {
                scalar_type: t,
                left: l,
                right: r,
            } => (Self::Arith(P::Saturating, K::Rem, *t), l, Some(r)),
            _ => return None,
        })
    }
}

/// Unified table of all bounded arithmetic, order, numeral, division, cast, and
/// case-split laws elaborated into the mathematical core signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CoreLaw {
    EqToLe,
    LtToLe,
    LeTrans,
    LtTrans,
    LtLeTrans,
    LeLtTrans,
    LtSubstLeft,
    LtSubstRight,
    LeSubstLeft,
    LeSubstRight,
    LtIrrefl,
    DoubleZero,
    NegateZero,
    EvenBeforeOdd,
    OddBeforeNextEven,
    NegateOrder,
    AddMonotone,
    AddCancelSub,
    AddMonotoneBoth,
    AddZeroRight,
    AddZeroLeft,
    SubZero,
    SubAntitone,
    SubSelfZero,
    SubLeAntitone,
    SubCancelAddLeft,
    SubCancelAddRight,
    SubMonotoneAntitone,
    MulDistributeAdd,
    MulMonotonePos,
    MulAntitoneNeg,
    DivBound {
        ty: IntegerType,
        lower: bool,
        positive: bool,
    },
    AddRightInv,
    SubRightInv,
    QuotBound {
        ty: IntegerType,
        upper: bool,
        positive: bool,
    },
    LeAntisym,
    DivisorCaseSplit,
    CastId(IntegerOperation),
}

/// Declarative first-order schema table for `CoreLaw`.
/// Syntax: variables `0..3`, numeral zero `z`, signed binary literals `#0`, `#1`, `#-1`,
/// unary numeral constructors `d(e)` / `o(e)` / `n(e)`, parenthesized binary `(a+b)` /
/// `(a-b)` / `(a*b)` / `(a/b)`, atomic relations `=`, `<`, `<=`, and arrows `->`.
const CORE_LAW_FORMULAS: [(CoreLaw, u8, &str); 33] = [
    (CoreLaw::EqToLe, 2, "0=1 -> 0<=1"),
    (CoreLaw::LtToLe, 2, "0<1 -> 0<=1"),
    (CoreLaw::LeTrans, 3, "0<=1 -> 1<=2 -> 0<=2"),
    (CoreLaw::LtTrans, 3, "0<1 -> 1<2 -> 0<2"),
    (CoreLaw::LtLeTrans, 3, "0<1 -> 1<=2 -> 0<2"),
    (CoreLaw::LeLtTrans, 3, "0<=1 -> 1<2 -> 0<2"),
    (CoreLaw::LtSubstLeft, 3, "0=1 -> 0<2 -> 1<2"),
    (CoreLaw::LtSubstRight, 3, "1=2 -> 0<1 -> 0<2"),
    (CoreLaw::LeSubstLeft, 3, "0=1 -> 0<=2 -> 1<=2"),
    (CoreLaw::LeSubstRight, 3, "1=2 -> 0<=1 -> 0<=2"),
    (CoreLaw::DoubleZero, 0, "d(z)=z"),
    (CoreLaw::NegateZero, 0, "n(z)=z"),
    (CoreLaw::EvenBeforeOdd, 1, "d(0)<o(0)"),
    (CoreLaw::OddBeforeNextEven, 2, "0<1 -> o(0)<d(1)"),
    (CoreLaw::NegateOrder, 2, "0<1 -> n(1)<n(0)"),
    (CoreLaw::AddMonotone, 3, "0<=1 -> (0+2)<=(1+2)"),
    (CoreLaw::AddMonotoneBoth, 4, "0<=1 -> 2<=3 -> (0+2)<=(1+3)"),
    (CoreLaw::AddCancelSub, 2, "((0-1)+1)=0"),
    (CoreLaw::AddZeroLeft, 1, "(#0+0)=0"),
    (CoreLaw::AddZeroRight, 1, "(0+#0)=0"),
    (CoreLaw::SubSelfZero, 1, "(0-0)=#0"),
    (CoreLaw::SubZero, 1, "(0-#0)=0"),
    (CoreLaw::SubAntitone, 3, "1<2 -> (0-2)<(0-1)"),
    (CoreLaw::SubLeAntitone, 3, "1<=2 -> (0-2)<=(0-1)"),
    (
        CoreLaw::SubMonotoneAntitone,
        4,
        "0<=1 -> 3<=2 -> (0-2)<=(1-3)",
    ),
    (CoreLaw::SubCancelAddLeft, 3, "(0+2)<=1 -> 0<=(1-2)"),
    (CoreLaw::SubCancelAddRight, 3, "1<=(0+2) -> (1-2)<=0"),
    (CoreLaw::MulDistributeAdd, 3, "(0*(1+2))=((0*1)+(0*2))"),
    (CoreLaw::MulMonotonePos, 3, "0<=1 -> #1<=2 -> (0*2)<=(1*2)"),
    (CoreLaw::MulAntitoneNeg, 3, "0<=1 -> 2<=#-1 -> (1*2)<=(0*2)"),
    (CoreLaw::AddRightInv, 3, "(0+1)=2 -> 0=(2-1)"),
    (CoreLaw::SubRightInv, 3, "(0-1)=2 -> 0=(2+1)"),
    (CoreLaw::LeAntisym, 2, "0<=1 -> 1<=0 -> 0=1"),
];

const DIV_BOUND_FORMULAS: [[&str; 2]; 2] = [
    [
        "#0<=0 -> 1<=#-1 -> ((0/1)*1)<=0",
        "#0<=0 -> #1<=1 -> ((0/1)*1)<=0",
    ],
    [
        "0<=#0 -> 1<=#-1 -> 0<=((0/1)*1)",
        "0<=#0 -> #1<=1 -> 0<=((0/1)*1)",
    ],
];

const QUOT_BOUND_FORMULAS: [[&str; 2]; 2] = [
    [
        "0<=#-1 -> (2*0)<=1 -> (1/0)<=2",
        "#1<=0 -> 1<=(2*0) -> (1/0)<=2",
    ],
    [
        "0<=#-1 -> 1<=(2*0) -> 2<=(1/0)",
        "#1<=0 -> (2*0)<=1 -> 2<=(1/0)",
    ],
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum MathTermKey {
    Closed(BigInt),
    Open(IntegerMathTerm),
}

#[derive(Debug, Clone, Copy)]
enum IntegerRelation {
    Equality { left: TermHandle, right: TermHandle },
    LessThan { left: TermHandle, right: TermHandle },
    LessOrEqual { left: TermHandle, right: TermHandle },
}

fn fixed_magnitude(value: &BigInt) -> Option<(bool, u128)> {
    if value.bit_length() > u128::BITS as usize {
        return None;
    }
    let (hi, lo) = value.abs().div_rem(&BigInt::from_u128(1_u128 << 64))?;
    Some((
        value.is_negative(),
        (u128::from(hi.to_u64()?) << 64) | u128::from(lo.to_u64()?),
    ))
}

fn integer_value_of(value: &BigInt) -> Option<IntegerValue> {
    let (neg, mag) = fixed_magnitude(value)?;
    Some(if neg && mag != 0 {
        IntegerValue::Signed(if mag == 1_u128 << 127 {
            i128::MIN
        } else {
            i128::try_from(mag).ok()?.checked_neg()?
        })
    } else {
        IntegerValue::Unsigned(mag)
    })
}

fn integer_parts(value: IntegerValue) -> (bool, u128) {
    match value {
        IntegerValue::Signed(v) => (v < 0, v.unsigned_abs()),
        IntegerValue::Unsigned(v) => (false, v),
    }
}

fn math_literal_value(lit: IntegerMathLiteral) -> Option<IntegerValue> {
    if lit.negative() {
        if lit.magnitude() == (i128::MAX as u128) + 1 {
            Some(IntegerValue::Signed(i128::MIN))
        } else {
            i128::try_from(lit.magnitude())
                .ok()
                .map(|m| IntegerValue::Signed(-m))
        }
    } else {
        Some(IntegerValue::Unsigned(lit.magnitude()))
    }
}

fn unbounded_integer(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Signed(v) => BigInt::from_i128(v),
        IntegerValue::Unsigned(v) => BigInt::from_u128(v),
    }
}

fn signed_literal(term: &ScalarTerm, ity: IntegerType) -> Option<i128> {
    match term.integer_value()? {
        (a, IntegerValue::Signed(v)) if a == ity => Some(v),
        _ => None,
    }
}

fn propositions_match(a: &Proposition, b: &Proposition) -> bool {
    propositions_match_under_integer_math_normalization(a, b)
}

fn math_less_or_equal_sides(
    conclusion: &Proposition,
    is_target: fn(&IntegerMathTerm) -> bool,
) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match conclusion {
        Proposition::IntegerMathLessOrEqual(b, op) if is_target(op) => Some((true, b, op)),
        Proposition::IntegerMathLessOrEqual(op, b) if is_target(op) => Some((false, b, op)),
        _ => None,
    }
}

fn affine_equality_for<'a>(
    defs: &'a [(Proposition, TermHandle)],
    subject: &ScalarTerm,
) -> Option<(&'a ScalarTerm, TermHandle, &'a Proposition)> {
    defs.iter().find_map(|(prop, ev)| {
        let (a, b) = prop.as_equal()?;
        if a == subject {
            Some((b, *ev, prop))
        } else if b == subject {
            Some((a, *ev, prop))
        } else {
            None
        }
    })
}

fn affine_witness_root_expression<'a>(
    w: &'a IntegerAffineWitness,
    defs: &'a [(Proposition, TermHandle)],
) -> Option<(&'a ScalarTerm, Option<(TermHandle, &'a Proposition)>)> {
    if w.definition_axioms.is_empty() && w.literal_axioms.is_empty() {
        Some((&w.root, None))
    } else if w.definition_axioms.len() == 1 {
        let (expr, ev, prop) = affine_equality_for(defs, &w.root)?;
        Some((expr, Some((ev, prop))))
    } else {
        None
    }
}

fn affine_root_premise(w: &IntegerAffineWitness, lower: bool, left: &ScalarTerm) -> Proposition {
    if lower {
        Proposition::LessOrEqual(w.root.clone(), left.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), w.root.clone())
    }
}

fn affine_carrier_endpoint_equality<'a>(
    ity: IntegerType,
    lower: bool,
    endpoint: &'a ScalarTerm,
    defs: &'a [(Proposition, TermHandle)],
) -> Option<(Option<(TermHandle, &'a Proposition)>, ScalarTerm)> {
    let val = if lower {
        ity.minimum_value()
    } else {
        ity.maximum_value()
    }?;
    let lit = ScalarTerm::integer(ity, val).expect("carrier endpoint is representable");
    if endpoint.integer_value() == Some((ity, val)) {
        return Some((None, lit));
    }
    let (other, ev, prop) = affine_equality_for(defs, endpoint)?;
    (other == &lit).then_some((Some((ev, prop)), lit))
}

fn eval_closed_math(term: &IntegerMathTerm) -> Result<Option<BigInt>, BoundedDenotationError> {
    ClosedIntegerEvaluator::default()
        .evaluate_closed(term)
        .map_err(|e| {
            BoundedDenotationError::Certificate(ProofError::PrimitiveJudgment(
                KernelError::ClosedIntegerEvaluation(e),
            ))
        })
}

// ============================================================================
// Iterative logical syntax table & proposition denotation
// ============================================================================

mod logical {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct SyntaxKey(usize);

    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum LogicalSyntax {
        Conjunction(Vec<SyntaxKey>),
        Disjunction(Vec<SyntaxKey>),
        Implication(SyntaxKey, SyntaxKey),
    }

    #[derive(Default)]
    pub(super) struct LogicalTable {
        atoms: BTreeMap<Proposition, SyntaxKey>,
        logical: BTreeMap<LogicalSyntax, SyntaxKey>,
        terms: Vec<Option<TermHandle>>,
    }

    impl LogicalTable {
        pub(super) fn interned_atom(&self, p: &Proposition) -> Option<TermHandle> {
            self.atoms.get(p).and_then(|k| self.terms[k.0])
        }
        fn alloc(&mut self) -> SyntaxKey {
            let k = SyntaxKey(self.terms.len());
            self.terms.push(None);
            k
        }
        fn intern_atom(&mut self, p: &Proposition) -> SyntaxKey {
            if let Some(&k) = self.atoms.get(p) {
                return k;
            }
            let k = self.alloc();
            self.atoms.insert(p.clone(), k);
            k
        }
        fn intern_logical(&mut self, s: LogicalSyntax) -> SyntaxKey {
            if let Some(&k) = self.logical.get(&s) {
                return k;
            }
            let k = self.alloc();
            self.logical.insert(s, k);
            k
        }
        fn get(&self, k: Option<SyntaxKey>) -> Option<TermHandle> {
            k.and_then(|k| self.terms[k.0])
        }
        fn publish(&mut self, k: Option<SyntaxKey>, term: TermHandle) {
            let slot = &mut self.terms[k.expect("valid arity").0];
            debug_assert!(slot.is_none() || *slot == Some(term));
            *slot = Some(term);
        }
    }

    struct Node<'a> {
        prop: &'a Proposition,
        key: Option<SyntaxKey>,
        children: Vec<usize>,
    }

    fn prepare<'a>(t: &mut LogicalTable, root: &'a Proposition) -> (Vec<Node<'a>>, usize) {
        enum Work<'a> {
            Enter(&'a Proposition),
            Close(&'a Proposition, usize),
        }
        let (mut nodes, mut done, mut stack) = (
            Vec::<Node<'a>>::new(),
            Vec::<usize>::new(),
            vec![Work::Enter(root)],
        );
        while let Some(w) = stack.pop() {
            let node = match w {
                Work::Enter(p) => match p {
                    Proposition::Conjunction(cs) | Proposition::Disjunction(cs)
                        if cs.len() >= 2 =>
                    {
                        stack.push(Work::Close(p, cs.len()));
                        stack.extend(cs.iter().rev().map(Work::Enter));
                        continue;
                    }
                    Proposition::Conjunction(_) | Proposition::Disjunction(_) => Node {
                        prop: p,
                        key: None,
                        children: Vec::new(),
                    },
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        stack.push(Work::Close(p, 2));
                        stack.push(Work::Enter(conclusion));
                        stack.push(Work::Enter(premise));
                        continue;
                    }
                    _ => Node {
                        prop: p,
                        key: Some(t.intern_atom(p)),
                        children: Vec::new(),
                    },
                },
                Work::Close(p, count) => {
                    let children = done.split_off(done.len() - count);
                    let key = children
                        .iter()
                        .map(|&c| nodes[c].key)
                        .collect::<Option<Vec<_>>>()
                        .map(|ks| {
                            let s = match p {
                                Proposition::Conjunction(_) => LogicalSyntax::Conjunction(ks),
                                Proposition::Disjunction(_) => LogicalSyntax::Disjunction(ks),
                                Proposition::Implication { .. } => {
                                    LogicalSyntax::Implication(ks[0], ks[1])
                                }
                                _ => unreachable!(),
                            };
                            t.intern_logical(s)
                        });
                    Node {
                        prop: p,
                        key,
                        children,
                    }
                }
            };
            done.push(nodes.len());
            nodes.push(node);
        }
        assert_eq!(done.len(), 1);
        (nodes, done[0])
    }

    pub(super) fn denote(
        d: &mut Denotation,
        prop: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(term) = d.denotations.interned_atom(prop) {
            return Ok(term);
        }
        let (nodes, root) = prepare(&mut d.denotations, prop);
        let mut pending: Vec<(usize, usize, Vec<TermHandle>)> = Vec::new();
        let mut cur = root;
        'enter: loop {
            let node = &nodes[cur];
            let mut val = if let Some(term) = d.denotations.get(node.key) {
                term
            } else {
                match node.prop {
                    Proposition::Conjunction(cs) if cs.len() < 2 => {
                        return Err(BoundedDenotationError::Certificate(
                            ProofError::RuleConclusionMismatch("conjunction arity"),
                        ));
                    }
                    Proposition::Disjunction(cs) if cs.len() < 2 => {
                        return Err(BoundedDenotationError::Certificate(
                            ProofError::RuleConclusionMismatch("disjunction arity"),
                        ));
                    }
                    Proposition::Conjunction(_)
                    | Proposition::Disjunction(_)
                    | Proposition::Implication { .. } => {
                        pending.push((cur, 1, Vec::with_capacity(node.children.len())));
                        cur = node.children[0];
                        continue 'enter;
                    }
                    _ => {
                        let term = d.denote_leaf(node.prop)?;
                        d.denotations.publish(node.key, term);
                        term
                    }
                }
            };
            loop {
                let Some((idx, mut next, mut terms)) = pending.pop() else {
                    return Ok(val);
                };
                terms.push(val);
                let node = &nodes[idx];
                if next < node.children.len() {
                    cur = node.children[next];
                    next += 1;
                    pending.push((idx, next, terms));
                    continue 'enter;
                }
                val = match node.prop {
                    Proposition::Conjunction(_) => d.conjunction_terms(terms),
                    Proposition::Disjunction(_) => d.disjunction_terms(terms),
                    Proposition::Implication { .. } => d.pi(terms[0], terms[1]),
                    _ => unreachable!(),
                };
                d.denotations.publish(node.key, val);
            }
        }
    }
}

// ============================================================================
// Core Denotation state, arena combinators, and table-driven CoreLaw builder
// ============================================================================

trait PositionRoster<K> {
    fn roster_position(&self, key: &K) -> Option<u32>;
    fn record_position(&mut self, key: K, position: u32);
}
impl<K: Ord> PositionRoster<K> for BTreeMap<K, u32> {
    fn roster_position(&self, k: &K) -> Option<u32> {
        self.get(k).copied()
    }
    fn record_position(&mut self, k: K, p: u32) {
        self.insert(k, p);
    }
}
impl<K: Eq + std::hash::Hash> PositionRoster<K> for HashMap<K, u32> {
    fn roster_position(&self, k: &K) -> Option<u32> {
        self.get(k).copied()
    }
    fn record_position(&mut self, k: K, p: u32) {
        self.insert(k, p);
    }
}

#[derive(Default)]
struct DenotationTables {
    atoms: BTreeMap<Proposition, u32>,
    carriers: BTreeMap<ScalarType, u32>,
    terms: BTreeMap<ScalarTerm, u32>,
    content_carriers: BTreeMap<ContentAlgebra, TermHandle>,
    content_families: [Option<u32>; 2],
    content_parameters: BTreeMap<String, u32>,
    content_terms: BTreeMap<(ContentAlgebra, ContentTerm), u32>,
    integer: Option<u32>,
    integer_less_than: Option<u32>,
    integer_less_or_equal: Option<u32>,
    binary_ops: [Option<u32>; 3],
    numeral_zero: Option<u32>,
    numeral_ctors: [Option<u32>; 3],
    numeral_prefixes: BTreeMap<(NumeralCtor, u32), u32>,
    laws: BTreeMap<CoreLaw, u32>,
    numeral_equations: BTreeMap<(BinaryOp, IntegerValue, IntegerValue, IntegerValue), u32>,
    numeral_quotients: BTreeMap<(IntegerType, IntegerValue, IntegerValue, IntegerValue), u32>,
    math_terms: BTreeMap<MathTermKey, u32>,
    closed_math_values: Vec<Option<BigInt>>,
    scalar_integer_terms: BTreeMap<ScalarTerm, u32>,
    integer_operations: BTreeMap<IntegerOperation, u32>,
    carrier_bounds: BTreeMap<(ScalarTerm, bool), u32>,
    rule_axioms: BTreeMap<(Vec<Proposition>, Proposition), u32>,
    constants: Vec<Option<TermHandle>>,
}

struct Denotation {
    arena: TermArena,
    type_zero: TermHandle,
    two: TermHandle,
    two_zero: TermHandle,
    two_one: TermHandle,
    boolean_equality: TermHandle,
    declarations: Vec<Declaration>,
    decisions: HashMap<TermHandle, u32>,
    tables: DenotationTables,
    denotations: logical::LogicalTable,
    data: data_subjects::DataSubjectTables,
}

impl Denotation {
    fn new() -> Self {
        Self::new_in(TermArena::new(), Vec::new())
    }

    fn new_in(mut arena: TermArena, declarations: Vec<Declaration>) -> Self {
        let type_zero = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let two = arena.insert(Term::Two);
        let two_zero = arena.insert(Term::TwoZero);
        let two_one = arena.insert(Term::TwoOne);
        Self {
            arena,
            type_zero,
            two,
            two_zero,
            two_one,
            boolean_equality: TermHandle::invalid(),
            declarations,
            decisions: HashMap::new(),
            tables: DenotationTables::default(),
            denotations: logical::LogicalTable::default(),
            data: data_subjects::DataSubjectTables::default(),
        }
    }

    fn var(&mut self, index: u32) -> TermHandle {
        self.arena.insert(Term::Variable(index))
    }
    fn pi(&mut self, domain: TermHandle, codomain: TermHandle) -> TermHandle {
        self.arena.insert(Term::Pi { domain, codomain })
    }
    fn lam(&mut self, domain: TermHandle, body: TermHandle) -> TermHandle {
        self.arena.insert(Term::Lambda { domain, body })
    }
    fn sigma(&mut self, domain: TermHandle, codomain: TermHandle) -> TermHandle {
        self.arena.insert(Term::Sigma { domain, codomain })
    }
    fn pair(&mut self, first: TermHandle, second: TermHandle) -> TermHandle {
        self.arena.insert(Term::Pair { first, second })
    }
    /// `(λ (x : ty). x) term`: the kernel checks `term` against `ty` and infers `ty`.
    fn ascribe(&mut self, ty: TermHandle, term: TermHandle) -> TermHandle {
        let bound = self.var(0);
        let identity = self.lam(ty, bound);
        self.app(identity, term)
    }
    fn fst(&mut self, pair: TermHandle) -> TermHandle {
        self.arena.insert(Term::Fst { pair })
    }
    fn snd(&mut self, pair: TermHandle) -> TermHandle {
        self.arena.insert(Term::Snd { pair })
    }
    fn app(&mut self, function: TermHandle, argument: TermHandle) -> TermHandle {
        self.arena.insert(Term::Apply { function, argument })
    }
    fn id(&mut self, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
        self.arena.insert(Term::Id { ty, left, right })
    }
    fn refl(&mut self, ty: TermHandle, value: TermHandle) -> TermHandle {
        self.arena.insert(Term::Refl { ty, value })
    }
    fn empty_elim(&mut self, ty: TermHandle, scrutinee: TermHandle) -> TermHandle {
        self.arena.insert(Term::EmptyElim { ty, scrutinee })
    }
    fn case_two(
        &mut self,
        motive: TermHandle,
        zero_branch: TermHandle,
        one_branch: TermHandle,
        scrutinee: TermHandle,
    ) -> TermHandle {
        self.arena.insert(Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        })
    }

    fn position(&self) -> Result<u32, BoundedDenotationError> {
        u32::try_from(self.declarations.len())
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)
    }

    fn constant(&mut self, decl: u32) -> TermHandle {
        let idx = decl as usize;
        if let Some(&Some(h)) = self.tables.constants.get(idx) {
            return h;
        }
        let h = self.arena.insert(Term::Constant {
            declaration: decl,
            levels: Vec::new(),
        });
        if self.tables.constants.len() <= idx {
            self.tables.constants.resize(idx + 1, None);
        }
        self.tables.constants[idx] = Some(h);
        h
    }

    fn record_closed_math_value(&mut self, pos: u32, value: BigInt) {
        let idx = pos as usize;
        if self.tables.closed_math_values.len() <= idx {
            self.tables.closed_math_values.resize(idx + 1, None);
        }
        self.tables.closed_math_values[idx] = Some(value);
    }

    fn closed_math_value(&self, term: TermHandle) -> Option<BigInt> {
        let Term::Constant { declaration, .. } = self.arena.get(term) else {
            return None;
        };
        self.tables
            .closed_math_values
            .get(declaration as usize)
            .and_then(Clone::clone)
    }

    fn interned_slot(
        &mut self,
        slot: impl Fn(&mut Self) -> &mut Option<u32>,
        ty_fn: impl FnOnce(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(p) = *slot(self) {
            return Ok(p);
        }
        let ty = ty_fn(self)?;
        let p = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        *slot(self) = Some(p);
        Ok(p)
    }

    fn interned_keyed<K, M: PositionRoster<K>>(
        &mut self,
        roster: impl Fn(&mut Self) -> &mut M,
        key: K,
        ty_fn: impl FnOnce(&mut Self) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(p) = roster(self).roster_position(&key) {
            return Ok(p);
        }
        let ty = ty_fn(self)?;
        let p = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        roster(self).record_position(key, p);
        Ok(p)
    }

    fn integer(&mut self) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(|s| &mut s.tables.integer, |s| Ok(s.type_zero))
    }
    fn integer_constant(&mut self) -> Result<TermHandle, BoundedDenotationError> {
        let p = self.integer()?;
        Ok(self.constant(p))
    }
    fn integer_order_type(&mut self) -> Result<TermHandle, BoundedDenotationError> {
        let int = self.integer_constant()?;
        let t = self.pi(int, self.type_zero);
        Ok(self.pi(int, t))
    }
    fn integer_less_than(&mut self) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(
            |s| &mut s.tables.integer_less_than,
            Self::integer_order_type,
        )
    }
    fn integer_less_or_equal(&mut self) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(
            |s| &mut s.tables.integer_less_or_equal,
            Self::integer_order_type,
        )
    }
    fn binary_op_position(&mut self, op: BinaryOp) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(
            |s| &mut s.tables.binary_ops[op.slot()],
            |s| {
                let int = s.integer_constant()?;
                let inner = s.pi(int, int);
                Ok(s.pi(int, inner))
            },
        )
    }
    fn binary_terms(
        &mut self,
        op: BinaryOp,
        l: TermHandle,
        r: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let head = self.binary_op_position(op)?;
        let h = self.constant(head);
        Ok(self.arena.apply_binary(h, l, r))
    }
    fn order_term(
        &mut self,
        strict: bool,
        l: TermHandle,
        r: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let p = if strict {
            self.integer_less_than()?
        } else {
            self.integer_less_or_equal()?
        };
        let h = self.constant(p);
        Ok(self.arena.apply_binary(h, l, r))
    }

    // --- Table-driven CoreLaw instantiation ---

    fn law(
        &mut self,
        law: CoreLaw,
        args: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let pos = if let Some(&p) = self.tables.laws.get(&law) {
            p
        } else {
            let ty = self.build_law_type(law)?;
            let p = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.tables.laws.insert(law, p);
            p
        };
        let head = self.constant(pos);
        Ok(self.arena.apply_arguments(head, args))
    }

    fn parse_law_expr(
        &mut self,
        src: &mut &[u8],
        depth: u32,
        div_ty: Option<IntegerType>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let b = src[0];
        *src = &src[1..];
        Ok(match b {
            b'0'..=b'3' => self.var(depth - 1 - u32::from(b - b'0')),
            b'z' => {
                let p = self.numeral_zero()?;
                self.constant(p)
            }
            b'#' => {
                let neg = src[0] == b'-';
                if neg {
                    *src = &src[1..];
                }
                let mag = u128::from(src[0] - b'0');
                *src = &src[1..];
                self.binary_term(neg, mag)?
            }
            b'd' | b'o' | b'n' => {
                let ctor = match b {
                    b'd' => NumeralCtor::Double,
                    b'o' => NumeralCtor::Odd,
                    _ => NumeralCtor::Negate,
                };
                debug_assert_eq!(src[0], b'(');
                *src = &src[1..];
                let inner = self.parse_law_expr(src, depth, div_ty)?;
                debug_assert_eq!(src[0], b')');
                *src = &src[1..];
                let f = self.numeral_ctor(ctor)?;
                let fh = self.constant(f);
                self.app(fh, inner)
            }
            b'(' => {
                let l = self.parse_law_expr(src, depth, div_ty)?;
                let op = src[0];
                *src = &src[1..];
                let r = self.parse_law_expr(src, depth, div_ty)?;
                debug_assert_eq!(src[0], b')');
                *src = &src[1..];
                match op {
                    b'+' => self.binary_terms(BinaryOp::Add, l, r)?,
                    b'-' => self.binary_terms(BinaryOp::Sub, l, r)?,
                    b'*' => self.binary_terms(BinaryOp::Mul, l, r)?,
                    b'/' => self.apply_integer_operation(
                        IntegerOperation::exact_divide(div_ty.expect("div ty")),
                        l,
                        Some(r),
                    )?,
                    _ => unreachable!("invalid law operator"),
                }
            }
            _ => unreachable!("invalid law expression"),
        })
    }

    fn build_law_formula(
        &mut self,
        arity: u8,
        formula: &str,
        div_ty: Option<IntegerType>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let parts: Vec<&str> = formula.split(" -> ").collect();
        let k = u32::from(arity);
        let int = self.integer_constant()?;
        let mut built = Vec::with_capacity(parts.len());
        for (j, raw) in parts.into_iter().enumerate() {
            let depth = k + j as u32;
            let (op, l_str, r_str) = if let Some((l, r)) = raw.split_once("<=") {
                self.integer_less_or_equal()?;
                (0u8, l, r)
            } else if let Some((l, r)) = raw.split_once('<') {
                self.integer_less_than()?;
                (1u8, l, r)
            } else {
                let (l, r) = raw.split_once('=').expect("law atom relation");
                (2u8, l, r)
            };
            let (mut lb, mut rb) = (l_str.as_bytes(), r_str.as_bytes());
            let (lh, rh) = (
                self.parse_law_expr(&mut lb, depth, div_ty)?,
                self.parse_law_expr(&mut rb, depth, div_ty)?,
            );
            debug_assert!(lb.is_empty() && rb.is_empty());
            built.push(match op {
                0 => self.order_term(false, lh, rh)?,
                1 => self.order_term(true, lh, rh)?,
                _ => self.id(int, lh, rh),
            });
        }
        let mut ty = built.pop().expect("law conclusion");
        for dom in built.into_iter().rev() {
            ty = self.pi(dom, ty);
        }
        for _ in 0..k {
            ty = self.pi(int, ty);
        }
        Ok(ty)
    }

    fn build_law_type(&mut self, law: CoreLaw) -> Result<TermHandle, BoundedDenotationError> {
        use CoreLaw as L;
        let int = self.integer_constant()?;
        match law {
            L::LtIrrefl => {
                let lt = self.integer_less_than()?;
                let (lth, x, empty) = (
                    self.constant(lt),
                    self.var(0),
                    self.arena.insert(Term::Empty),
                );
                let dom = self.arena.apply_binary(lth, x, x);
                let cod = self.pi(dom, empty);
                return Ok(self.pi(int, cod));
            }
            L::CastId(op) => {
                let p = self.integer_operation(op)?;
                let (fh, x) = (self.constant(p), self.var(0));
                let app = self.app(fh, x);
                let eq = self.id(int, app, x);
                return Ok(self.pi(int, eq));
            }
            L::EqToLe
            | L::LtToLe
            | L::LeTrans
            | L::LtTrans
            | L::LtLeTrans
            | L::LeLtTrans
            | L::LtSubstLeft
            | L::LtSubstRight
            | L::LeSubstLeft
            | L::LeSubstRight => {
                self.integer_less_than()?;
                self.integer_less_or_equal()?;
            }
            L::DoubleZero
            | L::NegateZero
            | L::EvenBeforeOdd
            | L::OddBeforeNextEven
            | L::NegateOrder => {
                self.integer_less_than()?;
                self.declare_numerals()?;
            }
            L::AddMonotone
            | L::AddCancelSub
            | L::AddMonotoneBoth
            | L::AddZeroRight
            | L::AddZeroLeft => {
                self.binary_op_position(BinaryOp::Add)?;
            }
            L::SubZero
            | L::SubAntitone
            | L::SubSelfZero
            | L::SubLeAntitone
            | L::SubCancelAddLeft
            | L::SubCancelAddRight
            | L::SubMonotoneAntitone => {
                self.binary_op_position(BinaryOp::Sub)?;
                self.integer_less_than()?;
                self.binary_integer(false, 0)?;
            }
            L::MulDistributeAdd | L::MulMonotonePos | L::MulAntitoneNeg | L::DivBound { .. } => {
                self.binary_op_position(BinaryOp::Mul)?;
                self.integer_less_or_equal()?;
                for (neg, mag) in [(false, 0), (false, 1), (true, 1)] {
                    self.binary_integer(neg, mag)?;
                }
            }
            L::AddRightInv
            | L::SubRightInv
            | L::QuotBound { .. }
            | L::LeAntisym
            | L::DivisorCaseSplit => {
                for op in [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul] {
                    self.binary_op_position(op)?;
                }
                self.integer_less_or_equal()?;
                for (neg, mag) in [(false, 0), (false, 1), (true, 1), (true, 2)] {
                    self.binary_integer(neg, mag)?;
                }
            }
        }
        if law == L::DivisorCaseSplit {
            let (z, one, m1, m2) = (
                self.binary_term(false, 0)?,
                self.binary_term(false, 1)?,
                self.binary_term(true, 1)?,
                self.binary_term(true, 2)?,
            );
            let (d1, d2, d3) = (self.var(1), self.var(2), self.var(3));
            let (c0, c1) = (self.id(int, d3, z), self.order_term(false, one, d3)?);
            let inner = self.tagged_sum(c0, c1);
            let cm1 = self.id(int, d2, m1);
            let mid = self.tagged_sum(cm1, inner);
            let cm2 = self.order_term(false, d1, m2)?;
            let top = self.tagged_sum(cm2, mid);
            return Ok(self.pi(int, top));
        }
        let (arity, formula, div_ty) = match law {
            L::DivBound {
                ty,
                lower,
                positive,
            } => (
                2,
                DIV_BOUND_FORMULAS[usize::from(lower)][usize::from(positive)],
                Some(ty),
            ),
            L::QuotBound {
                ty,
                upper,
                positive,
            } => (
                3,
                QUOT_BOUND_FORMULAS[usize::from(upper)][usize::from(positive)],
                Some(ty),
            ),
            _ => {
                let &(_, arity, formula) = CORE_LAW_FORMULAS
                    .iter()
                    .find(|&&(l, ..)| l == law)
                    .expect("law in table");
                (arity, formula, None)
            }
        };
        self.build_law_formula(arity, formula, div_ty)
    }

    // --- Shared proof combinators ---

    fn ord_subst(
        &mut self,
        strict: bool,
        left_side: bool,
        from: TermHandle,
        to: TermHandle,
        other: TermHandle,
        eq: TermHandle,
        order: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let law = match (strict, left_side) {
            (true, true) => CoreLaw::LtSubstLeft,
            (true, false) => CoreLaw::LtSubstRight,
            (false, true) => CoreLaw::LeSubstLeft,
            (false, false) => CoreLaw::LeSubstRight,
        };
        let args = if left_side {
            [from, to, other, eq, order]
        } else {
            [other, from, to, eq, order]
        };
        self.law(law, &args)
    }

    fn ord_trans(
        &mut self,
        s1: bool,
        s2: bool,
        a: TermHandle,
        b: TermHandle,
        c: TermHandle,
        first: TermHandle,
        second: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let law = match (s1, s2) {
            (false, false) => CoreLaw::LeTrans,
            (true, true) => CoreLaw::LtTrans,
            (true, false) => CoreLaw::LtLeTrans,
            (false, true) => CoreLaw::LeLtTrans,
        };
        self.law(law, &[a, b, c, first, second])
    }

    fn le_of_eq(
        &mut self,
        a: TermHandle,
        b: TermHandle,
        eq: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.law(CoreLaw::EqToLe, &[a, b, eq])
    }
    fn le_of_lt(
        &mut self,
        a: TermHandle,
        b: TermHandle,
        lt: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.law(CoreLaw::LtToLe, &[a, b, lt])
    }
    fn le_refl(&mut self, a: TermHandle) -> Result<TermHandle, BoundedDenotationError> {
        let int = self.integer_constant()?;
        let r = self.refl(int, a);
        self.le_of_eq(a, a, r)
    }
    fn irreflexive(
        &mut self,
        v: TermHandle,
        ev: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.law(CoreLaw::LtIrrefl, &[v, ev])
    }

    fn id_elim(
        &mut self,
        ty: TermHandle,
        from: TermHandle,
        endpoint: TermHandle,
        proof: TermHandle,
        base: TermHandle,
        body: TermHandle,
    ) -> TermHandle {
        let bound = self.var(0);
        let identity = self.id(ty, from, bound);
        let inner = self.lam(identity, body);
        let motive = self.lam(ty, inner);
        self.arena.insert(Term::IdElim {
            motive,
            base,
            endpoint,
            proof,
        })
    }

    fn symmetry(
        &mut self,
        ty: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
    ) -> TermHandle {
        let flipped = self.var(1);
        let body = self.id(ty, flipped, from);
        let base = self.refl(ty, from);
        self.id_elim(ty, from, to, proof, base, body)
    }

    fn transitivity(
        &mut self,
        ty: TermHandle,
        left: TermHandle,
        mid: TermHandle,
        right: TermHandle,
        first: TermHandle,
        second: TermHandle,
    ) -> TermHandle {
        let ep = self.var(1);
        let body = self.id(ty, left, ep);
        self.id_elim(ty, mid, right, second, first, body)
    }

    fn tagged_sum(&mut self, zero: TermHandle, one: TermHandle) -> TermHandle {
        let motive = self.lam(self.two, self.type_zero);
        let bound = self.var(0);
        let fam = self.case_two(motive, zero, one, bound);
        self.sigma(self.two, fam)
    }

    fn tagged_sum_elim_with(
        &mut self,
        zero: TermHandle,
        rest: TermHandle,
        tag: TermHandle,
        payload: TermHandle,
        zb: TermHandle,
        ob: TermHandle,
        goal: TermHandle,
    ) -> TermHandle {
        let im = self.lam(self.two, self.type_zero);
        let bound = self.var(0);
        let fam = self.case_two(im, zero, rest, bound);
        let body = self.pi(fam, goal);
        let motive = self.lam(self.two, body);
        let elim = self.case_two(motive, zb, ob, tag);
        self.app(elim, payload)
    }

    fn tagged_sum_elim(
        &mut self,
        zero: TermHandle,
        rest: TermHandle,
        scrutinee: TermHandle,
        zb: TermHandle,
        ob: TermHandle,
        goal: TermHandle,
    ) -> TermHandle {
        let (tag, payload) = (self.fst(scrutinee), self.snd(scrutinee));
        self.tagged_sum_elim_with(zero, rest, tag, payload, zb, ob, goal)
    }

    fn directed_equality(
        &mut self,
        s: TermHandle,
        t: TermHandle,
        old: TermHandle,
        new: TermHandle,
        ev: TermHandle,
    ) -> Option<TermHandle> {
        if self.arena.structurally_equal(s, old) && self.arena.structurally_equal(t, new) {
            Some(ev)
        } else if self.arena.structurally_equal(s, new) && self.arena.structurally_equal(t, old) {
            let int = self.integer_constant().ok()?;
            Some(self.symmetry(int, s, t, ev))
        } else {
            None
        }
    }

    fn directed_prop_eq(
        &mut self,
        prop: &Proposition,
        ev: TermHandle,
        old: TermHandle,
        new: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let denoted = self.denote(prop)?;
        let Some((_, from, to)) = self.identity_parts(denoted) else {
            return Ok(None);
        };
        Ok(self.directed_equality(from, to, old, new, ev))
    }

    fn identity_parts(&self, term: TermHandle) -> Option<(TermHandle, TermHandle, TermHandle)> {
        match self.arena.get(term) {
            Term::Id { ty, left, right } => Some((ty, left, right)),
            _ => None,
        }
    }

    fn integer_relation(&self, term: TermHandle) -> Option<IntegerRelation> {
        match self.arena.get(term) {
            Term::Id { ty, left, right } => match self.arena.get(ty) {
                Term::Constant { declaration, .. } if Some(declaration) == self.tables.integer => {
                    Some(IntegerRelation::Equality { left, right })
                }
                _ => None,
            },
            Term::Apply {
                function,
                argument: right,
            } => match self.arena.get(function) {
                Term::Apply {
                    function: rel,
                    argument: left,
                } => match self.arena.get(rel) {
                    Term::Constant { declaration, .. }
                        if Some(declaration) == self.tables.integer_less_than =>
                    {
                        Some(IntegerRelation::LessThan { left, right })
                    }
                    Term::Constant { declaration, .. }
                        if Some(declaration) == self.tables.integer_less_or_equal =>
                    {
                        Some(IntegerRelation::LessOrEqual { left, right })
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    fn occurs_free(&self, term: TermHandle, index: u32) -> bool {
        self.arena.contains_free_variable(term, index)
    }

    fn atom(&mut self, prop: &Proposition) -> Result<TermHandle, BoundedDenotationError> {
        let key = lift_fixed_integer_relation(prop).unwrap_or_else(|| prop.clone());
        let p = self.interned_keyed(|s| &mut s.tables.atoms, key, |s| Ok(s.type_zero))?;
        Ok(self.constant(p))
    }

    fn carrier(&mut self, ty: ScalarType) -> Result<TermHandle, BoundedDenotationError> {
        if ty == ScalarType::Boolean {
            return Ok(self.two);
        }
        let p = self.interned_keyed(|s| &mut s.tables.carriers, ty, |s| Ok(s.type_zero))?;
        Ok(self.constant(p))
    }

    fn content_carrier(
        &mut self,
        alg: &ContentAlgebra,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(&c) = self.tables.content_carriers.get(alg) {
            return Ok(c);
        }
        let p = self.interned_keyed(
            |s| &mut s.tables.content_parameters,
            alg.parameter.clone(),
            |s| Ok(s.type_zero),
        )?;
        let param = self.constant(p);
        let c = self.content_carrier_at(alg.kind, param, Level::Constant(0))?;
        self.tables.content_carriers.insert(alg.clone(), c);
        Ok(c)
    }

    fn content_carrier_at(
        &mut self,
        kind: ContentAlgebraKind,
        param: TermHandle,
        level: Level,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let slot = usize::from(matches!(kind, ContentAlgebraKind::CountedQuantity));
        let decl = match self.tables.content_families[slot] {
            Some(d) => d,
            None => {
                let dom = self
                    .arena
                    .insert(Term::Sort(Sort::Type(Level::Parameter(0))));
                let ty = self.pi(dom, self.type_zero);
                let d = self.position()?;
                self.declarations.push(Declaration::assumption(1, ty));
                self.tables.content_families[slot] = Some(d);
                d
            }
        };
        let f = self.arena.insert(Term::Constant {
            declaration: decl,
            levels: vec![level],
        });
        Ok(self.app(f, param))
    }

    fn content_term(
        &mut self,
        alg: &ContentAlgebra,
        term: &ContentTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let p = self.interned_keyed(
            |s| &mut s.tables.content_terms,
            (alg.clone(), term.clone()),
            |s| s.content_carrier(alg),
        )?;
        Ok(self.constant(p))
    }

    fn integer_operation(&mut self, op: IntegerOperation) -> Result<u32, BoundedDenotationError> {
        self.interned_keyed(
            |s| &mut s.tables.integer_operations,
            op,
            |s| {
                let mut ty = if op.returns_boolean() {
                    s.two
                } else {
                    s.integer_constant()?
                };
                let dom = s.integer_constant()?;
                for _ in 0..op.arity() {
                    ty = s.pi(dom, ty);
                }
                Ok(ty)
            },
        )
    }

    fn apply_integer_operation(
        &mut self,
        op: IntegerOperation,
        l: TermHandle,
        r: Option<TermHandle>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let p = self.integer_operation(op)?;
        let f = self.constant(p);
        Ok(match r {
            Some(r) => self.arena.apply_binary(f, l, r),
            None => self.app(f, l),
        })
    }

    fn integer_relation_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (op, l, r) = match term {
            ScalarTerm::IntegerEqual {
                scalar_type: t,
                left: l,
                right: r,
            } => (IntegerOperation::Rel(RelOp::Eq, *t), l, r),
            ScalarTerm::IntegerLessThan {
                scalar_type: t,
                left: l,
                right: r,
            } => (IntegerOperation::Rel(RelOp::Lt, *t), l, r),
            ScalarTerm::IntegerLessOrEqual {
                scalar_type: t,
                left: l,
                right: r,
            } => (IntegerOperation::Rel(RelOp::Le, *t), l, r),
            _ => return Ok(None),
        };
        let (lt, rt) = (self.fixed_scalar_term(l)?, self.fixed_scalar_term(r)?);
        self.apply_integer_operation(op, lt, Some(rt)).map(Some)
    }

    fn math_term(&mut self, term: &IntegerMathTerm) -> Result<TermHandle, BoundedDenotationError> {
        if let IntegerMathTerm::Embed(inner) = term {
            return self.fixed_scalar_term(inner);
        }
        let key = match eval_closed_math(term)? {
            Some(v) => MathTermKey::Closed(v),
            None => {
                let bin = match term {
                    IntegerMathTerm::Add(l, r) => Some((BinaryOp::Add, l, r)),
                    IntegerMathTerm::Subtract(l, r) => Some((BinaryOp::Sub, l, r)),
                    IntegerMathTerm::Multiply(l, r) => Some((BinaryOp::Mul, l, r)),
                    _ => None,
                };
                if let Some((op, l, r)) = bin
                    && ClosedIntegerEvaluator::default().evaluate_closed(l).is_ok()
                    && ClosedIntegerEvaluator::default().evaluate_closed(r).is_ok()
                {
                    let (lt, rt) = (self.math_term(l)?, self.math_term(r)?);
                    return self.binary_terms(op, lt, rt);
                }
                MathTermKey::Open(term.clone())
            }
        };
        self.intern_math_term(key)
    }

    fn math_literal(&mut self, value: BigInt) -> Result<TermHandle, BoundedDenotationError> {
        self.intern_math_term(MathTermKey::Closed(value))
    }

    fn intern_math_term(&mut self, key: MathTermKey) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(&p) = self.tables.math_terms.get(&key) {
            return Ok(self.constant(p));
        }
        if let MathTermKey::Closed(v) = &key
            && let Some((neg, mag)) = fixed_magnitude(v)
        {
            let p = self.binary_integer(neg, mag)?;
            self.record_closed_math_value(p, v.clone());
            self.tables.math_terms.insert(key, p);
            return Ok(self.constant(p));
        }
        let ty = self.integer_constant()?;
        let p = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        if let MathTermKey::Closed(v) = &key {
            self.record_closed_math_value(p, v.clone());
        }
        self.tables.math_terms.insert(key, p);
        Ok(self.constant(p))
    }

    fn fixed_scalar_term(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        enum Pending<'a> {
            Unary(IntegerOperation),
            Left(Option<BinaryOp>, Option<IntegerOperation>, &'a ScalarTerm),
            Right(Option<BinaryOp>, Option<IntegerOperation>, TermHandle),
        }
        let (mut pending, mut cur) = (Vec::new(), term);
        'enter: loop {
            let mut val = if let Some((_, v)) = cur.integer_value() {
                self.math_term(&IntegerMathTerm::literal(v))?
            } else {
                match cur {
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(src),
                    } => self.math_term(&IntegerMathTerm::MathValue {
                        source_type: *src,
                        value: *id,
                    })?,
                    ScalarTerm::ExactIntegerAdd { left, right, .. } => {
                        pending.push(Pending::Left(Some(BinaryOp::Add), None, right));
                        cur = left;
                        continue 'enter;
                    }
                    ScalarTerm::ExactIntegerSubtract { left, right, .. } => {
                        pending.push(Pending::Left(Some(BinaryOp::Sub), None, right));
                        cur = left;
                        continue 'enter;
                    }
                    ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
                        pending.push(Pending::Left(Some(BinaryOp::Mul), None, right));
                        cur = left;
                        continue 'enter;
                    }
                    _ => {
                        if let Some((op, l, r)) = IntegerOperation::from_scalar_term(cur) {
                            match r {
                                Some(r) => pending.push(Pending::Left(None, Some(op), r)),
                                None => pending.push(Pending::Unary(op)),
                            }
                            cur = l;
                            continue 'enter;
                        }
                        if let Some(read) = self.data_payload_read(cur)? {
                            read
                        } else {
                            let p = self.interned_keyed(
                                |s| &mut s.tables.scalar_integer_terms,
                                cur.clone(),
                                Self::integer_constant,
                            )?;
                            self.constant(p)
                        }
                    }
                }
            };
            loop {
                match pending.pop() {
                    Some(Pending::Unary(op)) => {
                        val = self.apply_integer_operation(op, val, None)?
                    }
                    Some(Pending::Left(b, p, r)) => {
                        pending.push(Pending::Right(b, p, val));
                        cur = r;
                        continue 'enter;
                    }
                    Some(Pending::Right(Some(b), None, l)) => val = self.binary_terms(b, l, val)?,
                    Some(Pending::Right(None, Some(p), l)) => {
                        val = self.apply_integer_operation(p, l, Some(val))?
                    }
                    Some(Pending::Right(..)) => unreachable!(),
                    None => return Ok(val),
                }
            }
        }
    }

    fn fixed_scalar_relation(
        &mut self,
        prop: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (l, r) = match prop {
            semantic_vocabulary::ordered_comparison_operands!(l, r) => (l, r),
            _ => return Ok(None),
        };
        let is_fixed = |t: &ScalarTerm| matches!(t.scalar_type(), ScalarType::Integer(i) if i.carrier() == IntegerCarrier::Fixed && !i.is_address());
        if !is_fixed(l) || !is_fixed(r) {
            return Ok(None);
        }
        let (lt, rt) = (self.fixed_scalar_term(l)?, self.fixed_scalar_term(r)?);
        Ok(Some(if matches!(prop, Proposition::Equal(..)) {
            let int = self.integer_constant()?;
            self.id(int, lt, rt)
        } else {
            self.order_term(matches!(prop, Proposition::LessThan(..)), lt, rt)?
        }))
    }

    fn denote(&mut self, prop: &Proposition) -> Result<TermHandle, BoundedDenotationError> {
        logical::denote(self, prop)
    }

    fn denote_leaf(&mut self, prop: &Proposition) -> Result<TermHandle, BoundedDenotationError> {
        Ok(match prop {
            Proposition::Truth => self.id(self.two, self.two_zero, self.two_zero),
            Proposition::Falsehood => self.id(self.two, self.two_zero, self.two_one),
            Proposition::Equal(..) | Proposition::LessThan(..) | Proposition::LessOrEqual(..) => {
                if let Some(lifted) = lift_fixed_integer_relation(prop) {
                    self.denote(&lifted)?
                } else if let Some(d) = self.fixed_scalar_relation(prop)? {
                    d
                } else if let Proposition::Equal(l, r) = prop {
                    let (ty, lt, rt) = (
                        self.carrier(l.scalar_type())?,
                        self.scalar_term(l)?,
                        self.scalar_term(r)?,
                    );
                    self.id(ty, lt, rt)
                } else {
                    self.atom(prop)?
                }
            }
            Proposition::IntegerMathEqual(l, r) => {
                let (lt, rt, int) = (
                    self.math_term(l)?,
                    self.math_term(r)?,
                    self.integer_constant()?,
                );
                self.id(int, lt, rt)
            }
            Proposition::IntegerMathLessThan(l, r) => {
                let (lt, rt) = (self.math_term(l)?, self.math_term(r)?);
                self.order_term(true, lt, rt)?
            }
            Proposition::IntegerMathLessOrEqual(l, r) => {
                let (lt, rt) = (self.math_term(l)?, self.math_term(r)?);
                self.order_term(false, lt, rt)?
            }
            Proposition::Conjunction(_)
            | Proposition::Disjunction(_)
            | Proposition::Implication { .. } => unreachable!(),
            Proposition::ContentConservation(c) => {
                let (ty, l, r) = (
                    self.content_carrier(c.algebra())?,
                    self.content_term(c.algebra(), c.left())?,
                    self.content_term(c.algebra(), c.right())?,
                );
                self.id(ty, l, r)
            }
            atomic => {
                if let Some(test) = self.data_case_test(atomic)? {
                    test
                } else if let Some(identity) = self.data_identity(atomic)? {
                    identity
                } else {
                    self.atom(atomic)?
                }
            }
        })
    }

    fn conjunction_terms(&mut self, mut ms: Vec<TermHandle>) -> TermHandle {
        let mut tail = ms.pop().expect("arity >= 2");
        while let Some(d) = ms.pop() {
            tail = self.sigma(d, tail);
        }
        tail
    }

    fn disjunction(&mut self, ds: &[Proposition]) -> Result<TermHandle, BoundedDenotationError> {
        if ds.len() < 2 {
            return Err(BoundedDenotationError::Certificate(
                ProofError::RuleConclusionMismatch("disjunction arity"),
            ));
        }
        let terms = ds
            .iter()
            .map(|d| self.denote(d))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.disjunction_terms(terms))
    }

    fn disjunction_terms(&mut self, mut ms: Vec<TermHandle>) -> TermHandle {
        let mut tail = ms.pop().expect("arity >= 2");
        while let Some(f) = ms.pop() {
            tail = self.tagged_sum(f, tail);
        }
        tail
    }

    fn rule_axiom(
        &mut self,
        prems: &[Proposition],
        concl: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let p = self.interned_keyed(
            |s| &mut s.tables.rule_axioms,
            (prems.to_vec(), concl.clone()),
            |s| {
                let mut ty = s.denote(concl)?;
                for prem in prems.iter().rev() {
                    let d = s.denote(prem)?;
                    ty = s.pi(d, ty);
                }
                Ok(ty)
            },
        )?;
        Ok(self.constant(p))
    }

    fn decision(&mut self, ty: TermHandle) -> Result<TermHandle, BoundedDenotationError> {
        let p = self.interned_keyed(|s| &mut s.decisions, ty, |_| Ok(ty))?;
        Ok(self.constant(p))
    }

    fn carrier_bound(
        &mut self,
        op: &ScalarTerm,
        lower: bool,
        ity: &IntegerType,
    ) -> Result<(ScalarTerm, TermHandle), BoundedDenotationError> {
        let v = if lower {
            ity.minimum_value()
        } else {
            ity.maximum_value()
        }
        .ok_or(BoundedDenotationError::Unsupported(
            "a carrier bound of an integer whose width is a parameter",
        ))?;
        let ep = ScalarTerm::integer(*ity, v).expect("carrier endpoint is representable");
        let p = self.interned_keyed(
            |s| &mut s.tables.carrier_bounds,
            (op.clone(), lower),
            |s| {
                let prop = if lower {
                    Proposition::LessOrEqual(ep.clone(), op.clone())
                } else {
                    Proposition::LessOrEqual(op.clone(), ep.clone())
                };
                s.denote(&prop)
            },
        )?;
        Ok((ep, self.constant(p)))
    }
}

// ============================================================================
// Binary numerals, literal ordering, discreteness, and numeral equations
// ============================================================================

#[derive(Clone, Copy)]
struct NumeralPositions {
    zero: u32,
    double: u32,
    odd: u32,
    negate: u32,
}

impl Denotation {
    fn numeral_zero(&mut self) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(|s| &mut s.tables.numeral_zero, Self::integer_constant)
    }
    fn numeral_ctor(&mut self, c: NumeralCtor) -> Result<u32, BoundedDenotationError> {
        self.interned_slot(
            |s| &mut s.tables.numeral_ctors[c.slot()],
            |s| {
                let int = s.integer_constant()?;
                Ok(s.pi(int, int))
            },
        )
    }
    fn numeral_prefix(
        &mut self,
        c: NumeralCtor,
        prefix: u32,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(&p) = self.tables.numeral_prefixes.get(&(c, prefix)) {
            return Ok(p);
        }
        let fp = self.numeral_ctor(c)?;
        let (f, a, int) = (
            self.constant(fp),
            self.constant(prefix),
            self.integer_constant()?,
        );
        let body = self.app(f, a);
        let p = self.position()?;
        self.declarations
            .push(Declaration::definition(0, int, body));
        self.tables.numeral_prefixes.insert((c, prefix), p);
        Ok(p)
    }
    fn binary_integer(&mut self, neg: bool, mag: u128) -> Result<u32, BoundedDenotationError> {
        let mut p = self.numeral_zero()?;
        for bit in (0..(u128::BITS - mag.leading_zeros())).rev() {
            let c = if mag & (1_u128 << bit) == 0 {
                NumeralCtor::Double
            } else {
                NumeralCtor::Odd
            };
            p = self.numeral_prefix(c, p)?;
        }
        if neg && mag != 0 {
            p = self.numeral_prefix(NumeralCtor::Negate, p)?;
        }
        Ok(p)
    }
    fn binary_term(&mut self, neg: bool, mag: u128) -> Result<TermHandle, BoundedDenotationError> {
        let p = self.binary_integer(neg, mag)?;
        Ok(self.constant(p))
    }
    fn integer_numeral(&mut self, v: i128) -> Result<TermHandle, BoundedDenotationError> {
        self.binary_term(v < 0, v.unsigned_abs())
    }
    fn declare_numerals(&mut self) -> Result<NumeralPositions, BoundedDenotationError> {
        Ok(NumeralPositions {
            zero: self.numeral_zero()?,
            double: self.numeral_ctor(NumeralCtor::Double)?,
            odd: self.numeral_ctor(NumeralCtor::Odd)?,
            negate: self.numeral_ctor(NumeralCtor::Negate)?,
        })
    }

    fn adopt_numerals(&mut self, pos: NumeralPositions) -> Result<(), BoundedDenotationError> {
        let ctors = [
            (NumeralCtor::Double, pos.double),
            (NumeralCtor::Odd, pos.odd),
            (NumeralCtor::Negate, pos.negate),
        ];
        self.tables.numeral_zero = Some(pos.zero);
        for (c, p) in ctors {
            self.tables.numeral_ctors[c.slot()] = Some(p);
        }
        let _ = self.math_literal(BigInt::zero())?;
        let (int, base) = (self.integer_constant()?, pos.zero as usize);
        let mut vals: Vec<Option<BigInt>> =
            vec![None; self.declarations.len().saturating_sub(base)];
        if let Some(first) = vals.first_mut() {
            *first = Some(BigInt::zero());
        }
        for index in pos.negate as usize + 1..self.declarations.len() {
            let Ok(decl_pos) = u32::try_from(index) else {
                break;
            };
            let Declaration {
                level_arity,
                ty,
                body,
                ..
            } = self.declarations[index].clone();
            let Some(body) = body else {
                continue;
            };
            if level_arity != 0 || !self.arena.structurally_equal(ty, int) {
                continue;
            }
            let Term::Apply { function, argument } = self.arena.get(body) else {
                continue;
            };
            let (
                Term::Constant {
                    declaration: f,
                    levels: fl,
                },
                Term::Constant {
                    declaration: pre,
                    levels: pl,
                },
            ) = (self.arena.get(function), self.arena.get(argument))
            else {
                continue;
            };
            if !fl.is_empty() || !pl.is_empty() {
                continue;
            }
            let Some(&(ctor, _)) = ctors.iter().find(|&&(_, p)| p == f) else {
                continue;
            };
            let Some(pv) = (pre as usize)
                .checked_sub(base)
                .and_then(|o| vals.get(o))
                .cloned()
                .flatten()
            else {
                continue;
            };
            let positive = !pv.is_negative() && pv != BigInt::zero();
            let val = match ctor {
                NumeralCtor::Double if positive => Some(pv.add(&pv)),
                NumeralCtor::Odd if !pv.is_negative() => {
                    Some(pv.add(&pv).add(&BigInt::from_u64(1)))
                }
                NumeralCtor::Negate if positive => Some(BigInt::zero().sub(&pv)),
                _ => None,
            };
            self.tables
                .numeral_prefixes
                .entry((ctor, pre))
                .or_insert(decl_pos);
            if let Some(val) = val {
                vals[index - base] = Some(val.clone());
                self.record_closed_math_value(decl_pos, val.clone());
                self.tables
                    .math_terms
                    .entry(MathTermKey::Closed(val))
                    .or_insert(decl_pos);
            }
        }
        Ok(())
    }

    fn rewrite_zero_prefix(
        &mut self,
        c: NumeralCtor,
        law: CoreLaw,
        left_side: bool,
        other: TermHandle,
        order: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let zp = self.numeral_zero()?;
        let rp = self.numeral_prefix(c, zp)?;
        let (raw, z, eq) = (self.constant(rp), self.constant(zp), self.law(law, &[])?);
        self.ord_subst(true, left_side, raw, z, other, eq, order)
    }

    fn positive_adjacent(&mut self, lower: u128) -> Result<TermHandle, BoundedDenotationError> {
        let half = self.binary_term(false, lower / 2)?;
        if lower & 1 != 0 {
            let ev = self.positive_adjacent(lower / 2)?;
            let next = self.binary_term(false, lower / 2 + 1)?;
            return self.law(CoreLaw::OddBeforeNextEven, &[half, next, ev]);
        }
        let ev = self.law(CoreLaw::EvenBeforeOdd, &[half])?;
        if lower != 0 {
            return Ok(ev);
        }
        let one = self.binary_term(false, 1)?;
        self.rewrite_zero_prefix(NumeralCtor::Double, CoreLaw::DoubleZero, true, one, ev)
    }

    fn literal_adjacent(
        &mut self,
        lower: &ScalarTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (_, v) = lower
            .integer_value()
            .ok_or(BoundedDenotationError::Unsupported(
                "discreteness needs a fixed literal",
            ))?;
        let (neg, mag) = integer_parts(v);
        if !neg {
            return self.positive_adjacent(mag);
        }
        let ev = self.positive_adjacent(mag - 1)?;
        let (l, r) = (
            self.binary_term(false, mag - 1)?,
            self.binary_term(false, mag)?,
        );
        let ev = self.law(CoreLaw::NegateOrder, &[l, r, ev])?;
        if mag != 1 {
            return Ok(ev);
        }
        let m1 = self.binary_term(true, 1)?;
        self.rewrite_zero_prefix(NumeralCtor::Negate, CoreLaw::NegateZero, false, m1, ev)
    }

    fn positive_order(&mut self, lo: u128, hi: u128) -> Result<TermHandle, BoundedDenotationError> {
        debug_assert!(lo < hi);
        let (lh, hh) = (
            self.binary_term(false, lo / 2)?,
            self.binary_term(false, hi / 2)?,
        );
        let ev = if lo / 2 == hi / 2 {
            self.law(CoreLaw::EvenBeforeOdd, &[lh])?
        } else {
            let pre = self.positive_order(lo / 2, hi / 2)?;
            let mut ev = self.law(CoreLaw::OddBeforeNextEven, &[lh, hh, pre])?;
            let (lo_odd, hi_even) = (
                self.binary_term(false, lo / 2 * 2 + 1)?,
                self.binary_term(false, hi / 2 * 2)?,
            );
            if lo & 1 == 0 {
                let p = self.binary_integer(false, lo / 2)?;
                let dp = self.numeral_prefix(NumeralCtor::Double, p)?;
                let dbl = self.constant(dp);
                let first = self.law(CoreLaw::EvenBeforeOdd, &[lh])?;
                ev = self.ord_trans(true, true, dbl, lo_odd, hi_even, first, ev)?;
            }
            if hi & 1 != 0 {
                let last = self.law(CoreLaw::EvenBeforeOdd, &[hh])?;
                let p = self.binary_integer(false, lo / 2)?;
                let c = if lo & 1 == 0 {
                    NumeralCtor::Double
                } else {
                    NumeralCtor::Odd
                };
                let rp = self.numeral_prefix(c, p)?;
                let (lraw, ht) = (self.constant(rp), self.binary_term(false, hi)?);
                ev = self.ord_trans(true, true, lraw, hi_even, ht, ev, last)?;
            }
            ev
        };
        if lo != 0 {
            return Ok(ev);
        }
        let ht = self.binary_term(false, hi)?;
        self.rewrite_zero_prefix(NumeralCtor::Double, CoreLaw::DoubleZero, true, ht, ev)
    }

    fn literal_order(
        &mut self,
        lo: IntegerValue,
        hi: IntegerValue,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let ((nl, l), (nu, u)) = (integer_parts(lo), integer_parts(hi));
        if !nl {
            return self.positive_order(l, u);
        }
        if nu || u == 0 {
            let ord = self.positive_order(u, l)?;
            let (pl, pr) = (self.binary_term(false, u)?, self.binary_term(false, l)?);
            let ord = self.law(CoreLaw::NegateOrder, &[pl, pr, ord])?;
            if u != 0 {
                return Ok(ord);
            }
            let left = self.binary_term(true, l)?;
            return self.rewrite_zero_prefix(
                NumeralCtor::Negate,
                CoreLaw::NegateZero,
                false,
                left,
                ord,
            );
        }
        let lv = if l == 1_u128 << 127 {
            i128::MIN
        } else {
            -(l as i128)
        };
        let (first, second) = (
            self.literal_order(IntegerValue::Signed(lv), IntegerValue::Signed(0))?,
            self.positive_order(0, u)?,
        );
        let (lt, z, rt) = (
            self.binary_term(true, l)?,
            self.binary_term(false, 0)?,
            self.binary_term(false, u)?,
        );
        self.ord_trans(true, true, lt, z, rt, first, second)
    }

    fn integer_le_numeral(
        &mut self,
        lo: IntegerValue,
        hi: IntegerValue,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ((nl, lm), (nu, um)) = (integer_parts(lo), integer_parts(hi));
        let (lt, ut) = (self.binary_term(nl, lm)?, self.binary_term(nu, um)?);
        if lo == hi {
            return self.le_refl(lt).map(Some);
        }
        if unbounded_integer(lo) >= unbounded_integer(hi) {
            return Ok(None);
        }
        let s = self.literal_order(lo, hi)?;
        self.le_of_lt(lt, ut, s).map(Some)
    }

    fn discreteness_evidence(
        &mut self,
        prem: &Proposition,
        ev: TermHandle,
        goal: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (Proposition::LessOrEqual(l, r), Proposition::LessThan(sl, sr)) = (prem, goal) else {
            return Err(BoundedDenotationError::Unsupported(
                "checked discreteness has integer order endpoints",
            ));
        };
        let (pt, gt) = (self.denote(prem)?, self.denote(goal)?);
        let (
            Some(IntegerRelation::LessOrEqual { left: a, right: b }),
            Some(IntegerRelation::LessThan { left: c, right: d }),
        ) = (self.integer_relation(pt), self.integer_relation(gt))
        else {
            return Err(BoundedDenotationError::Unsupported(
                "checked discreteness denotes in Int",
            ));
        };
        if r == sr {
            let adj = self.literal_adjacent(sl)?;
            self.ord_trans(true, false, c, a, b, adj, ev)
        } else {
            debug_assert_eq!(l, sl);
            let adj = self.literal_adjacent(r)?;
            self.ord_trans(false, true, a, b, d, ev, adj)
        }
    }

    /// Discreteness away from a literal: `n < 9` gives `n <= 8`, and
    /// `3 < n` gives `4 <= n`. The step adds no assumption about `n`: the
    /// difference `d` of the two sides is strictly on one side of zero, and
    /// `DivisorCaseSplit` (every integer is at most -2, -1, 0 or at least 1)
    /// leaves only the cases a unit away from zero, so `d <= -1` or `1 <= d`.
    /// Addition moves the literal back, and a numeral equation names the
    /// adjacent literal. Every law cited is closed, so the proof depends on
    /// the theorem's subjects only through its arguments and abstracts over
    /// them. Any other shape returns `None` and keeps the rule instance.
    fn successor_evidence(
        &mut self,
        prem: &Proposition,
        ev: TermHandle,
        goal: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (Proposition::LessThan(l, r), Proposition::LessOrEqual(bl, br)) = (prem, goal) else {
            return Ok(None);
        };
        let (pt, gt) = (self.denote(prem)?, self.denote(goal)?);
        let (
            Some(IntegerRelation::LessThan { left: a, right: b }),
            Some(IntegerRelation::LessOrEqual { .. }),
        ) = (self.integer_relation(pt), self.integer_relation(gt))
        else {
            return Ok(None);
        };
        let zero = self.integer_numeral(0)?;
        if r == br {
            // `k < x`: `0 < x - k`, so `1 <= x - k`, `1 + k <= x`, `k + 1 <= x`.
            let (Some((_, k)), Some((_, next))) = (l.integer_value(), bl.integer_value()) else {
                return Ok(None);
            };
            let (kn, km) = integer_parts(k);
            let (nn, nm) = integer_parts(next);
            let (kt, nt, one) = (
                self.binary_term(kn, km)?,
                self.binary_term(nn, nm)?,
                self.integer_numeral(1)?,
            );
            let Some(eq) = self.numeral_binary_equation(
                BinaryOp::Add,
                IntegerValue::Unsigned(1),
                k,
                next,
                one,
                kt,
                nt,
            )?
            else {
                return Ok(None);
            };
            let (d, same) = (
                self.binary_terms(BinaryOp::Sub, b, a)?,
                self.binary_terms(BinaryOp::Sub, b, b)?,
            );
            let order = self.law(CoreLaw::SubAntitone, &[b, a, b, ev])?;
            let to_zero = self.law(CoreLaw::SubSelfZero, &[b])?;
            let positive = self.ord_subst(true, true, same, zero, d, to_zero, order)?;
            let Some(unit) = self.unit_from_zero(d, positive, false)? else {
                return Ok(None);
            };
            let moved = self.law(CoreLaw::AddMonotone, &[one, d, a, unit])?;
            let (back, sum) = (
                self.binary_terms(BinaryOp::Add, d, a)?,
                self.binary_terms(BinaryOp::Add, one, a)?,
            );
            let cancel = self.law(CoreLaw::AddCancelSub, &[b, a])?;
            let bound = self.ord_subst(false, false, back, b, sum, cancel, moved)?;
            return self.ord_subst(false, true, sum, nt, b, eq, bound).map(Some);
        }
        if l != bl {
            return Ok(None);
        }
        // `x < k`: `x - k < 0`, so `x - k <= -1`, `x <= -1 + k`, `x <= k - 1`.
        let (Some((_, k)), Some((_, previous))) = (r.integer_value(), br.integer_value()) else {
            return Ok(None);
        };
        let (kn, km) = integer_parts(k);
        let (pn, pm) = integer_parts(previous);
        let (kt, prev_t, minus_one) = (
            self.binary_term(kn, km)?,
            self.binary_term(pn, pm)?,
            self.integer_numeral(-1)?,
        );
        let Some(eq) = self.numeral_binary_equation(
            BinaryOp::Add,
            IntegerValue::Signed(-1),
            k,
            previous,
            minus_one,
            kt,
            prev_t,
        )?
        else {
            return Ok(None);
        };
        let (d, same) = (
            self.binary_terms(BinaryOp::Sub, a, b)?,
            self.binary_terms(BinaryOp::Sub, a, a)?,
        );
        let order = self.law(CoreLaw::SubAntitone, &[a, a, b, ev])?;
        let to_zero = self.law(CoreLaw::SubSelfZero, &[a])?;
        let negative = self.ord_subst(true, false, same, zero, d, to_zero, order)?;
        let Some(unit) = self.unit_from_zero(d, negative, true)? else {
            return Ok(None);
        };
        let moved = self.law(CoreLaw::AddMonotone, &[d, minus_one, b, unit])?;
        let (back, sum) = (
            self.binary_terms(BinaryOp::Add, d, b)?,
            self.binary_terms(BinaryOp::Add, minus_one, b)?,
        );
        let cancel = self.law(CoreLaw::AddCancelSub, &[a, b])?;
        let bound = self.ord_subst(false, true, back, a, sum, cancel, moved)?;
        self.ord_subst(false, false, sum, prev_t, a, eq, bound)
            .map(Some)
    }

    /// From `d < 0` (`negative`) prove `d <= -1`, or from `0 < d` prove
    /// `1 <= d`, by `DivisorCaseSplit` on `d`. The two cases on the wrong
    /// side of zero, and zero itself, contradict the strict premise.
    fn unit_from_zero(
        &mut self,
        d: TermHandle,
        strict: TermHandle,
        negative: bool,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        // The case types are read again under the branch binders, so `d`
        // must have no bound variable a binder would capture.
        let lifted = shift(&mut self.arena, d, 0, 1);
        if !self.arena.structurally_equal(lifted, d) {
            return Ok(None);
        }
        let int = self.integer_constant()?;
        let (z, one, m1, m2) = (
            self.integer_numeral(0)?,
            self.integer_numeral(1)?,
            self.integer_numeral(-1)?,
            self.integer_numeral(-2)?,
        );
        let goal = if negative {
            self.order_term(false, d, m1)?
        } else {
            self.order_term(false, one, d)?
        };
        let (cm2, cm1, c0, c1) = (
            self.order_term(false, d, m2)?,
            self.id(int, d, m1),
            self.id(int, d, z),
            self.order_term(false, one, d)?,
        );
        // Each branch binds its case's hypothesis as variable 0.
        let (hyp, held) = (self.var(0), shift(&mut self.arena, strict, 0, 1));
        let Some(m2_m1) =
            self.integer_le_numeral(IntegerValue::Signed(-2), IntegerValue::Signed(-1))?
        else {
            return Ok(None);
        };
        let (m2_z, m1_z, z_one) = (
            self.literal_order(IntegerValue::Signed(-2), IntegerValue::Signed(0))?,
            self.literal_order(IntegerValue::Signed(-1), IntegerValue::Signed(0))?,
            self.literal_order(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1))?,
        );
        // `held` is `d < 0` or `0 < d`; `at` replaces `d` by a literal in it.
        let at = |s: &mut Self, value: TermHandle| -> Result<TermHandle, BoundedDenotationError> {
            if negative {
                s.ord_subst(true, true, d, value, z, hyp, held)
            } else {
                s.ord_subst(true, false, d, value, z, hyp, held)
            }
        };
        let m2_body = if negative {
            self.ord_trans(false, false, d, m2, m1, hyp, m2_m1)?
        } else {
            // `0 < d <= -2 < 0`.
            let below = self.ord_trans(true, false, z, d, m2, held, hyp)?;
            let loop_ = self.ord_trans(true, true, z, m2, z, below, m2_z)?;
            let e = self.irreflexive(z, loop_)?;
            self.empty_elim(goal, e)
        };
        let m1_body = if negative {
            self.le_of_eq(d, m1, hyp)?
        } else {
            // `0 < -1 < 0`.
            let below = at(self, m1)?;
            let loop_ = self.ord_trans(true, true, z, m1, z, below, m1_z)?;
            let e = self.irreflexive(z, loop_)?;
            self.empty_elim(goal, e)
        };
        let z_body = {
            let loop_ = at(self, z)?;
            let e = self.irreflexive(z, loop_)?;
            self.empty_elim(goal, e)
        };
        let one_body = if negative {
            // `1 <= d < 0 < 1`.
            let below = self.ord_trans(false, true, one, d, z, hyp, held)?;
            let loop_ = self.ord_trans(true, true, one, z, one, below, z_one)?;
            let e = self.irreflexive(one, loop_)?;
            self.empty_elim(goal, e)
        } else {
            hyp
        };
        let (m2b, m1b, zb, ob) = (
            self.lam(cm2, m2_body),
            self.lam(cm1, m1_body),
            self.lam(c0, z_body),
            self.lam(c1, one_body),
        );
        let split = self.law(CoreLaw::DivisorCaseSplit, &[d])?;
        let inner = self.tagged_sum(c0, c1);
        let mid = self.tagged_sum(cm1, inner);
        let s0 = self.var(0);
        let (zb2, ob2) = (
            shift(&mut self.arena, zb, 0, 2),
            shift(&mut self.arena, ob, 0, 2),
        );
        let l3 = self.tagged_sum_elim(c0, c1, s0, zb2, ob2, goal);
        let m1_one = self.lam(inner, l3);
        let m1l = shift(&mut self.arena, m1b, 0, 1);
        let l2 = self.tagged_sum_elim(cm1, inner, s0, m1l, m1_one, goal);
        let top_one = self.lam(mid, l2);
        Ok(Some(
            self.tagged_sum_elim(cm2, mid, split, m2b, top_one, goal),
        ))
    }

    fn numeral_binary_equation(
        &mut self,
        op: BinaryOp,
        l: IntegerValue,
        r: IntegerValue,
        res: IntegerValue,
        lt: TermHandle,
        rt: TermHandle,
        rest: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if op.eval_big(&unbounded_integer(l), &unbounded_integer(r)) != unbounded_integer(res) {
            return Ok(None);
        }
        let p = self.interned_keyed(
            |s| &mut s.tables.numeral_equations,
            (op, l, r, res),
            |s| {
                let (int, app) = (s.integer_constant()?, s.binary_terms(op, lt, rt)?);
                Ok(s.id(int, app, rest))
            },
        )?;
        Ok(Some(self.constant(p)))
    }

    fn numeral_quotient(
        &mut self,
        ity: IntegerType,
        divd: IntegerValue,
        divr: IntegerValue,
        quot: IntegerValue,
        dt: TermHandle,
        rt: TermHandle,
        qt: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some((exact, _)) = unbounded_integer(divd).div_rem(&unbounded_integer(divr)) else {
            return Ok(None);
        };
        if exact != unbounded_integer(quot) {
            return Ok(None);
        }
        let p = self.interned_keyed(
            |s| &mut s.tables.numeral_quotients,
            (ity, divd, divr, quot),
            |s| {
                let (int, app) = (
                    s.integer_constant()?,
                    s.apply_integer_operation(IntegerOperation::exact_divide(ity), dt, Some(rt))?,
                );
                Ok(s.id(int, app, qt))
            },
        )?;
        Ok(Some(self.constant(p)))
    }
}

// ============================================================================
// Booleans: scalar denotation, case-analysis transport, and Two discriminant
// ============================================================================

#[derive(Clone, Copy)]
enum BooleanEval {
    Literal(bool),
    Blocked(TermHandle),
    Unsupported,
}
enum BooleanHead {
    Lambda(TermHandle),
    Neutral,
    Blocked(TermHandle),
    Unsupported,
}
#[derive(Clone, Copy)]
enum AtomRewrite {
    Binder,
    Literal(TermHandle),
}

impl Denotation {
    fn scalar_term(&mut self, term: &ScalarTerm) -> Result<TermHandle, BoundedDenotationError> {
        enum Pending<'a> {
            Not,
            EqL(&'a ScalarTerm),
            EqR(TermHandle),
        }
        let (mut pending, mut cur) = (Vec::new(), term);
        'enter: loop {
            let mut val = match cur {
                ScalarTerm::Boolean(false) => self.two_zero,
                ScalarTerm::Boolean(true) => self.two_one,
                ScalarTerm::BooleanNot { operand } => {
                    pending.push(Pending::Not);
                    cur = operand;
                    continue 'enter;
                }
                ScalarTerm::BooleanEqual { left, right } => {
                    pending.push(Pending::EqL(right));
                    cur = left;
                    continue 'enter;
                }
                _ => {
                    if let Some(rel) = self.integer_relation_term(cur)? {
                        rel
                    } else if matches!(
                        cur.scalar_type(),
                        ScalarType::Boolean | ScalarType::IeeeFloat(_)
                    ) && let Some(read) = self.data_payload_read(cur)?
                    {
                        read
                    } else {
                        let key = match cur.integer_value() {
                            Some((ity, v)) => {
                                ScalarTerm::integer(ity, v).unwrap_or_else(|_| cur.clone())
                            }
                            None => cur.clone(),
                        };
                        let ty = key.scalar_type();
                        let p =
                            self.interned_keyed(|s| &mut s.tables.terms, key, |s| s.carrier(ty))?;
                        self.constant(p)
                    }
                }
            };
            loop {
                match pending.pop() {
                    Some(Pending::Not) => {
                        let m = self.lam(self.two, self.two);
                        val = self.case_two(m, self.two_one, self.two_zero, val);
                    }
                    Some(Pending::EqL(r)) => {
                        pending.push(Pending::EqR(val));
                        cur = r;
                        continue 'enter;
                    }
                    Some(Pending::EqR(l)) => {
                        if !self.boolean_equality.is_valid() {
                            let (rv, lv, m) =
                                (self.var(0), self.var(1), self.lam(self.two, self.two));
                            let opp = self.case_two(m, self.two_one, self.two_zero, rv);
                            let res = self.case_two(m, opp, rv, lv);
                            let inner = self.lam(self.two, res);
                            self.boolean_equality = self.lam(self.two, inner);
                        }
                        val = self.arena.apply_binary(self.boolean_equality, l, val);
                    }
                    None => return Ok(val),
                }
            }
        }
    }

    fn boolean_goal_transport(
        &mut self,
        prem: TermHandle,
        goal: TermHandle,
        ev: TermHandle,
        require_two_goal: bool,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if require_two_goal
            && !self
                .identity_parts(goal)
                .is_some_and(|(c, ..)| self.arena.structurally_equal(c, self.two))
        {
            return Ok(None);
        }
        let Some((c, pl, pr)) = self.identity_parts(prem) else {
            return Ok(None);
        };
        if !self.arena.structurally_equal(c, self.two) {
            return Ok(None);
        }
        let mut rem = MAX_ELABORATION_NODES;
        Ok(self
            .boolean_goal_case(pl, pr, goal, &mut rem)?
            .map(|f| self.app(f, ev)))
    }

    fn boolean_goal_case(
        &mut self,
        pl: TermHandle,
        pr: TermHandle,
        goal: TermHandle,
        rem: &mut u64,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let evals = [pl, pr].map(|t| self.boolean_eval(t, rem));
        if evals.iter().any(|e| matches!(e, BooleanEval::Unsupported)) {
            return Ok(None);
        }
        let atom = evals
            .iter()
            .find_map(|e| match e {
                BooleanEval::Blocked(a) => Some(*a),
                _ => None,
            })
            .or_else(|| self.first_boolean_atom(goal, rem));
        if let Some(atom) = atom {
            let (Some(apl), Some(apr), Some(ag)) = (
                self.rewrite_atom(pl, atom, 0, AtomRewrite::Binder, rem),
                self.rewrite_atom(pr, atom, 0, AtomRewrite::Binder, rem),
                self.rewrite_atom(goal, atom, 1, AtomRewrite::Binder, rem),
            ) else {
                return Ok(None);
            };
            let pty = self.id(self.two, apl, apr);
            let body = self.pi(pty, ag);
            let motive = self.lam(self.two, body);
            let Some(zb) = self.instantiated_goal_case(pl, pr, goal, atom, self.two_zero, rem)?
            else {
                return Ok(None);
            };
            let Some(ob) = self.instantiated_goal_case(pl, pr, goal, atom, self.two_one, rem)?
            else {
                return Ok(None);
            };
            return Ok(Some(self.case_two(motive, zb, ob, atom)));
        }
        let (BooleanEval::Literal(lv), BooleanEval::Literal(rv)) = (evals[0], evals[1]) else {
            return Ok(None);
        };
        let dom = self.id(self.two, pl, pr);
        let body = if let Some(inh) = self.boolean_leaf_inhabitant(goal, rem)? {
            inh
        } else {
            if lv == rv {
                return Ok(None);
            }
            let proof = self.var(0);
            self.two_identity_discriminant(pl, pr, goal, lv, proof)
        };
        Ok(Some(self.lam(dom, body)))
    }

    fn boolean_leaf_inhabitant(
        &mut self,
        goal: TermHandle,
        rem: &mut u64,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if let Some((ty, l, r)) = self.identity_parts(goal)
            && self.arena.structurally_equal(ty, self.two)
        {
            let (BooleanEval::Literal(lv), BooleanEval::Literal(rv)) =
                (self.boolean_eval(l, rem), self.boolean_eval(r, rem))
            else {
                return Ok(None);
            };
            return Ok((lv == rv).then(|| self.refl(self.two, l)));
        }
        self.closed_inhabitant(goal)
    }

    fn instantiated_goal_case(
        &mut self,
        pl: TermHandle,
        pr: TermHandle,
        goal: TermHandle,
        atom: TermHandle,
        lit: TermHandle,
        rem: &mut u64,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let rw = AtomRewrite::Literal(lit);
        let (Some(pl), Some(pr), Some(goal)) = (
            self.rewrite_atom(pl, atom, 0, rw, rem),
            self.rewrite_atom(pr, atom, 0, rw, rem),
            self.rewrite_atom(goal, atom, 0, rw, rem),
        ) else {
            return Ok(None);
        };
        self.boolean_goal_case(pl, pr, goal, rem)
    }

    fn first_boolean_atom(&mut self, term: TermHandle, rem: &mut u64) -> Option<TermHandle> {
        if let BooleanEval::Blocked(a) = self.boolean_eval(term, rem)
            && self.neutral_atom_is_two(a)
        {
            return Some(a);
        }
        let cs: &[TermHandle] = match self.arena.get(term) {
            Term::Apply { function, argument } => &[function, argument],
            Term::Id { ty, left, right } => &[left, right, ty],
            Term::Pi { domain, codomain }
            | Term::Sigma { domain, codomain }
            | Term::Lambda {
                domain,
                body: codomain,
            } => &[domain, codomain],
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => &[scrutinee, zero_branch, one_branch, motive],
            _ => return None,
        };
        cs.iter().find_map(|&c| self.first_boolean_atom(c, rem))
    }

    fn neutral_atom_is_two(&self, mut atom: TermHandle) -> bool {
        let mut apps = 0u32;
        let head = loop {
            match self.arena.get(atom) {
                Term::Apply { function, .. } => {
                    apps = match apps.checked_add(1) {
                        Some(n) => n,
                        None => return false,
                    };
                    atom = function;
                }
                Term::Constant { declaration, .. } => break declaration,
                _ => return false,
            }
        };
        let Some(d) = self
            .declarations
            .get(head as usize)
            .filter(|d| d.is_assumption())
        else {
            return false;
        };
        let mut ty = d.ty;
        for _ in 0..apps {
            let Term::Pi { codomain, .. } = self.arena.get(ty) else {
                return false;
            };
            ty = codomain;
        }
        self.arena.structurally_equal(ty, self.two)
    }

    fn two_identity_discriminant(
        &mut self,
        pl: TermHandle,
        pr: TermHandle,
        at_right: TermHandle,
        pl_lit: bool,
        proof: TermHandle,
    ) -> TermHandle {
        let spl = shift(&mut self.arena, pl, 0, 1);
        let bound = self.var(0);
        let pdom = self.id(self.two, spl, bound);
        let (sl, sr) = (
            shift(&mut self.arena, pl, 0, 2),
            shift(&mut self.arena, at_right, 0, 2),
        );
        let self_id = self.id(self.two, sl, sl);
        let dm = self.lam(self.two, self.type_zero);
        let scrut = self.var(1);
        let (zb, ob) = if pl_lit { (sr, self_id) } else { (self_id, sr) };
        let disc = self.case_two(dm, zb, ob, scrut);
        let plam = self.lam(pdom, disc);
        let motive = self.lam(self.two, plam);
        let base = self.refl(self.two, pl);
        self.arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: pr,
            proof,
        })
    }

    fn boolean_eval(&mut self, mut term: TermHandle, rem: &mut u64) -> BooleanEval {
        loop {
            let Some(r) = rem.checked_sub(1) else {
                return BooleanEval::Unsupported;
            };
            *rem = r;
            match self.arena.get(term) {
                Term::TwoZero => return BooleanEval::Literal(false),
                Term::TwoOne => return BooleanEval::Literal(true),
                Term::Constant { declaration, .. } => {
                    return if self
                        .declarations
                        .get(declaration as usize)
                        .is_some_and(|d| {
                            d.is_assumption() && self.arena.structurally_equal(d.ty, self.two)
                        }) {
                        BooleanEval::Blocked(term)
                    } else {
                        BooleanEval::Unsupported
                    };
                }
                Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    scrutinee,
                    ..
                } => match self.boolean_eval(scrutinee, rem) {
                    BooleanEval::Literal(false) => term = zero_branch,
                    BooleanEval::Literal(true) => term = one_branch,
                    other => return other,
                },
                Term::Apply { function, argument } => match self
                    .boolean_function_head(function, rem)
                {
                    BooleanHead::Lambda(body) => term = substitute(&mut self.arena, body, argument),
                    BooleanHead::Neutral => return BooleanEval::Blocked(term),
                    BooleanHead::Blocked(a) => return BooleanEval::Blocked(a),
                    BooleanHead::Unsupported => return BooleanEval::Unsupported,
                },
                _ => return BooleanEval::Unsupported,
            }
        }
    }

    fn boolean_function_head(&mut self, mut term: TermHandle, rem: &mut u64) -> BooleanHead {
        loop {
            let Some(r) = rem.checked_sub(1) else {
                return BooleanHead::Unsupported;
            };
            *rem = r;
            match self.arena.get(term) {
                Term::Lambda { body, .. } => return BooleanHead::Lambda(body),
                Term::Constant { declaration, .. } => {
                    return if self
                        .declarations
                        .get(declaration as usize)
                        .is_some_and(Declaration::is_assumption)
                    {
                        BooleanHead::Neutral
                    } else {
                        BooleanHead::Unsupported
                    };
                }
                Term::Apply { function, argument } => match self
                    .boolean_function_head(function, rem)
                {
                    BooleanHead::Lambda(body) => term = substitute(&mut self.arena, body, argument),
                    other => return other,
                },
                Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    scrutinee,
                    ..
                } => match self.boolean_eval(scrutinee, rem) {
                    BooleanEval::Literal(false) => term = zero_branch,
                    BooleanEval::Literal(true) => term = one_branch,
                    BooleanEval::Blocked(a) => return BooleanHead::Blocked(a),
                    BooleanEval::Unsupported => return BooleanHead::Unsupported,
                },
                _ => return BooleanHead::Unsupported,
            }
        }
    }

    fn term_children(orig: &Term) -> Option<([TermHandle; 4], usize, bool)> {
        let z = TermHandle::invalid();
        Some(match *orig {
            Term::Apply {
                function: a,
                argument: b,
            } => ([a, b, z, z], 2, false),
            Term::Id {
                ty: a,
                left: b,
                right: c,
            } => ([a, b, c, z], 3, false),
            Term::Pi {
                domain: a,
                codomain: b,
            }
            | Term::Sigma {
                domain: a,
                codomain: b,
            }
            | Term::Lambda { domain: a, body: b } => ([a, b, z, z], 2, true),
            Term::CaseTwo {
                motive: a,
                zero_branch: b,
                one_branch: c,
                scrutinee: d,
            } => ([a, b, c, d], 4, false),
            _ => return None,
        })
    }

    fn rebuild_term(&mut self, orig: &Term, orig_h: TermHandle, c: [TermHandle; 4]) -> TermHandle {
        let built = match orig {
            Term::Apply { .. } => Term::Apply {
                function: c[0],
                argument: c[1],
            },
            Term::Id { .. } => Term::Id {
                ty: c[0],
                left: c[1],
                right: c[2],
            },
            Term::Pi { .. } => Term::Pi {
                domain: c[0],
                codomain: c[1],
            },
            Term::Sigma { .. } => Term::Sigma {
                domain: c[0],
                codomain: c[1],
            },
            Term::Lambda { .. } => Term::Lambda {
                domain: c[0],
                body: c[1],
            },
            Term::CaseTwo { .. } => Term::CaseTwo {
                motive: c[0],
                zero_branch: c[1],
                one_branch: c[2],
                scrutinee: c[3],
            },
            _ => unreachable!(),
        };
        if built == *orig {
            orig_h
        } else {
            self.arena.insert(built)
        }
    }

    fn rewrite_atom(
        &mut self,
        term: TermHandle,
        atom: TermHandle,
        depth: u32,
        mode: AtomRewrite,
        rem: &mut u64,
    ) -> Option<TermHandle> {
        *rem = rem.checked_sub(1)?;
        if self.arena.structurally_equal(term, atom) {
            return Some(match mode {
                AtomRewrite::Binder => self.var(depth),
                AtomRewrite::Literal(l) => l,
            });
        }
        let orig = self.arena.get(term);
        match orig {
            Term::Variable(i) => {
                return Some(match mode {
                    AtomRewrite::Binder if i >= depth => self.var(i.checked_add(1)?),
                    _ => term,
                });
            }
            Term::Constant { .. } | Term::Sort(_) | Term::Two | Term::TwoZero | Term::TwoOne => {
                return Some(term);
            }
            _ => {}
        }
        let (mut cs, len, binds) = Self::term_children(&orig)?;
        for (idx, slot) in cs[..len].iter_mut().enumerate() {
            let d = if binds && idx == 1 {
                depth.checked_add(1)?
            } else {
                depth
            };
            *slot = self.rewrite_atom(*slot, atom, d, mode, rem)?;
        }
        Some(self.rebuild_term(&orig, term, cs))
    }
}

// ============================================================================
// Equality transport, iterative coercion, closed inhabitants & refutations
// ============================================================================

#[derive(Clone, Copy)]
struct Equation {
    carrier: TermHandle,
    from: TermHandle,
    to: TermHandle,
    evidence: TermHandle,
}
struct Replacement {
    equation: Equation,
    context: TermHandle,
}

impl Denotation {
    fn symmetry_evidence(
        &mut self,
        prem: TermHandle,
        goal: TermHandle,
        ev: TermHandle,
    ) -> Option<TermHandle> {
        if self.arena.structurally_equal(prem, goal) {
            return Some(ev);
        }
        let ((c, l, r), (gc, gl, gr)) = (self.identity_parts(prem)?, self.identity_parts(goal)?);
        (self.arena.structurally_equal(c, gc)
            && self.arena.structurally_equal(gl, r)
            && self.arena.structurally_equal(gr, l))
        .then(|| self.symmetry(c, l, r, ev))
    }

    fn transitivity_evidence(
        &mut self,
        ft: TermHandle,
        f: TermHandle,
        st: TermHandle,
        s: TermHandle,
        gt: TermHandle,
    ) -> Option<TermHandle> {
        let ((c1, a, b), (c2, c, d), (cg, e, g)) = (
            self.identity_parts(ft)?,
            self.identity_parts(st)?,
            self.identity_parts(gt)?,
        );
        if !(self.arena.structurally_equal(c1, c2) && self.arena.structurally_equal(c1, cg)) {
            return None;
        }
        let (comp, l, r) = if self.arena.structurally_equal(b, c) {
            (self.transitivity(c1, a, b, d, f, s), a, d)
        } else if self.arena.structurally_equal(b, d) {
            let s = self.symmetry(c2, c, d, s);
            (self.transitivity(c1, a, b, c, f, s), a, c)
        } else if self.arena.structurally_equal(a, c) {
            let f = self.symmetry(c1, a, b, f);
            (self.transitivity(c1, b, a, d, f, s), b, d)
        } else if self.arena.structurally_equal(a, d) {
            let (f, s) = (self.symmetry(c1, a, b, f), self.symmetry(c2, c, d, s));
            (self.transitivity(c1, b, a, c, f, s), b, c)
        } else {
            return None;
        };
        if self.arena.structurally_equal(e, l) && self.arena.structurally_equal(g, r) {
            Some(comp)
        } else if self.arena.structurally_equal(e, r) && self.arena.structurally_equal(g, l) {
            Some(self.symmetry(cg, l, r, comp))
        } else {
            None
        }
    }

    fn weakening_evidence(
        &mut self,
        pt: TermHandle,
        ev: TermHandle,
        gt: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(gt) else {
            return Ok(None);
        };
        match self.integer_relation(pt) {
            Some(IntegerRelation::LessThan { left: a, right: b })
                if self.arena.structurally_equal(a, left)
                    && self.arena.structurally_equal(b, right) =>
            {
                self.le_of_lt(a, b, ev).map(Some)
            }
            Some(IntegerRelation::Equality { left: a, right: b }) => {
                let Some(eq) = self.directed_equality(a, b, left, right, ev) else {
                    return Ok(None);
                };
                self.le_of_eq(left, right, eq).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn order_transitivity_evidence(
        &mut self,
        ft: TermHandle,
        f: TermHandle,
        st: TermHandle,
        s: TermHandle,
        gt: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let unpack = |r| match r {
            IntegerRelation::LessThan { left, right } => Some((true, left, right)),
            IntegerRelation::LessOrEqual { left, right } => Some((false, left, right)),
            IntegerRelation::Equality { .. } => None,
        };
        let (Some((s1, a, b)), Some((s2, mid, c)), Some((sg, gl, gr))) = (
            self.integer_relation(ft).and_then(unpack),
            self.integer_relation(st).and_then(unpack),
            self.integer_relation(gt).and_then(unpack),
        ) else {
            return Ok(None);
        };
        if sg != (s1 || s2)
            || !self.arena.structurally_equal(b, mid)
            || !self.arena.structurally_equal(a, gl)
            || !self.arena.structurally_equal(c, gr)
        {
            return Ok(None);
        }
        self.ord_trans(s1, s2, a, b, c, f, s).map(Some)
    }

    fn order_substitution_evidence(
        &mut self,
        rt: TermHandle,
        rev: TermHandle,
        et: TermHandle,
        eev: TermHandle,
        ep: usize,
        gt: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (strict, rl, rr) = match self.integer_relation(rt) {
            Some(IntegerRelation::LessThan { left, right }) => (true, left, right),
            Some(IntegerRelation::LessOrEqual { left, right }) => (false, left, right),
            _ => return Ok(None),
        };
        let (gl, gr) = match self.integer_relation(gt) {
            Some(IntegerRelation::LessThan { left, right }) if strict => (left, right),
            Some(IntegerRelation::LessOrEqual { left, right }) if !strict => (left, right),
            _ => return Ok(None),
        };
        let Some(IntegerRelation::Equality { left: s, right: t }) = self.integer_relation(et)
        else {
            return Ok(None);
        };
        let (left_side, old, new, other) = match ep {
            0 if self.arena.structurally_equal(rr, gr) => (true, rl, gl, rr),
            1 if self.arena.structurally_equal(rl, gl) => (false, rr, gr, rl),
            _ => return Ok(None),
        };
        let Some(eq) = self.directed_equality(s, t, old, new, eev) else {
            return Ok(None);
        };
        self.ord_subst(strict, left_side, old, new, other, eq, rev)
            .map(Some)
    }

    fn tagged_sum_parts(&self, term: TermHandle) -> Option<(TermHandle, TermHandle, TermHandle)> {
        let Term::Sigma { domain, codomain } = self.arena.get(term) else {
            return None;
        };
        if !self.arena.structurally_equal(domain, self.two) {
            return None;
        }
        let Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } = self.arena.get(codomain)
        else {
            return None;
        };
        if !matches!(self.arena.get(scrutinee), Term::Variable(0))
            || [motive, zero_branch, one_branch]
                .into_iter()
                .any(|p| self.occurs_free(p, 0))
        {
            return None;
        }
        Some((motive, zero_branch, one_branch))
    }

    fn closed_inhabitant(
        &mut self,
        goal: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        enum P {
            First(TermHandle),
            Second(TermHandle),
            Zero(TermHandle),
            One,
            Body(TermHandle),
        }
        let (mut pending, mut cur) = (Vec::new(), goal);
        'descend: loop {
            let mut val = if let Some((ty, l, r)) = self.identity_parts(cur) {
                self.arena
                    .structurally_equal(l, r)
                    .then(|| self.refl(ty, l))
            } else if let Some(rel) = self.integer_relation(cur) {
                let (strict, l, r) = match rel {
                    IntegerRelation::LessThan { left, right } => (true, left, right),
                    IntegerRelation::LessOrEqual { left, right } => (false, left, right),
                    IntegerRelation::Equality { .. } => {
                        (false, TermHandle::invalid(), TermHandle::invalid())
                    }
                };
                if !l.is_valid() {
                    None
                } else if let (Some(lv), Some(rv)) =
                    (self.closed_math_value(l), self.closed_math_value(r))
                {
                    if !strict && lv == rv {
                        Some(self.le_refl(l)?)
                    } else if lv < rv
                        && let (Some(li), Some(ri)) = (integer_value_of(&lv), integer_value_of(&rv))
                    {
                        let s = self.literal_order(li, ri)?;
                        Some(if strict { s } else { self.le_of_lt(l, r, s)? })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                match self.arena.get(cur) {
                    Term::Sigma { domain, codomain } if !self.occurs_free(codomain, 0) => {
                        pending.push(P::First(codomain));
                        cur = domain;
                        continue 'descend;
                    }
                    Term::Sigma { domain, codomain }
                        if self.arena.structurally_equal(domain, self.two) =>
                    {
                        if let Term::CaseTwo {
                            zero_branch: z,
                            one_branch: o,
                            scrutinee: s,
                            ..
                        } = self.arena.get(codomain)
                            && matches!(self.arena.get(s), Term::Variable(0))
                        {
                            if !self.occurs_free(z, 0) {
                                pending.push(P::Zero(o));
                                cur = z;
                                continue 'descend;
                            }
                            if !self.occurs_free(o, 0) {
                                pending.push(P::One);
                                cur = o;
                                continue 'descend;
                            }
                        }
                        None
                    }
                    Term::Pi { domain, codomain } if !self.occurs_free(codomain, 0) => {
                        pending.push(P::Body(domain));
                        cur = codomain;
                        continue 'descend;
                    }
                    _ => None,
                }
            };
            loop {
                match pending.pop() {
                    Some(P::First(s)) => {
                        if let Some(f) = val {
                            pending.push(P::Second(f));
                            cur = s;
                            continue 'descend;
                        }
                    }
                    Some(P::Second(f)) => val = val.map(|s| self.pair(f, s)),
                    Some(P::Zero(o)) => {
                        if let Some(p) = val {
                            val = Some(self.pair(self.two_zero, p));
                        } else if !self.occurs_free(o, 0) {
                            pending.push(P::One);
                            cur = o;
                            continue 'descend;
                        }
                    }
                    Some(P::One) => val = val.map(|p| self.pair(self.two_one, p)),
                    Some(P::Body(d)) => val = val.map(|b| self.lam(d, b)),
                    None => return Ok(val),
                }
            }
        }
    }

    fn order_contradiction(
        &mut self,
        strict: bool,
        l: TermHandle,
        r: TermHandle,
        lv: IntegerValue,
        rv: IntegerValue,
        ev: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if strict && lv == rv {
            return self.irreflexive(l, ev);
        }
        let back = self.literal_order(rv, lv)?;
        let looped = self.ord_trans(strict, true, l, r, l, ev, back)?;
        self.irreflexive(l, looped)
    }

    fn closed_relation_refutation(
        &mut self,
        prem: TermHandle,
        ev: TermHandle,
        goal: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if let Some((ty, l, r)) = self.identity_parts(prem) {
            if self.arena.structurally_equal(ty, self.two) {
                let left_lit = if self.arena.structurally_equal(l, self.two_one) {
                    true
                } else if self.arena.structurally_equal(l, self.two_zero) {
                    false
                } else {
                    return Ok(None);
                };
                if !self.arena.structurally_equal(
                    r,
                    if left_lit {
                        self.two_zero
                    } else {
                        self.two_one
                    },
                ) {
                    return Ok(None);
                }
                return Ok(Some(
                    self.two_identity_discriminant(l, r, goal, left_lit, ev),
                ));
            }
            let (Some(lv), Some(rv)) = (self.closed_math_value(l), self.closed_math_value(r))
            else {
                return Ok(None);
            };
            if lv == rv {
                return Ok(None);
            }
            let (lo, hi, lov, hiv, eq) = if lv < rv {
                (l, r, lv, rv, ev)
            } else {
                (r, l, rv, lv, self.symmetry(ty, l, r, ev))
            };
            let (Some(lov), Some(hiv)) = (integer_value_of(&lov), integer_value_of(&hiv)) else {
                return Ok(None);
            };
            let strict = self.literal_order(lov, hiv)?;
            let looped = self.ord_subst(true, true, lo, hi, hi, eq, strict)?;
            let absurd = self.irreflexive(hi, looped)?;
            return Ok(Some(self.empty_elim(goal, absurd)));
        }
        let (strict, l, r) = match self.integer_relation(prem) {
            Some(IntegerRelation::LessThan { left, right }) => (true, left, right),
            Some(IntegerRelation::LessOrEqual { left, right }) => (false, left, right),
            _ => return Ok(None),
        };
        let (Some(lv), Some(rv)) = (self.closed_math_value(l), self.closed_math_value(r)) else {
            return Ok(None);
        };
        if (strict && lv < rv) || (!strict && lv <= rv) {
            return Ok(None);
        }
        let absurd = if strict && lv == rv {
            self.irreflexive(l, ev)?
        } else {
            let (Some(lv), Some(rv)) = (integer_value_of(&lv), integer_value_of(&rv)) else {
                return Ok(None);
            };
            self.order_contradiction(strict, l, r, lv, rv, ev)?
        };
        Ok(Some(self.empty_elim(goal, absurd)))
    }

    fn oriented_evidence(
        &mut self,
        prem: TermHandle,
        goal: TermHandle,
        ev: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if self.arena.structurally_equal(prem, goal) {
            return Ok(Some(ev));
        }
        let mut rem = MAX_ELABORATION_NODES;
        self.oriented_coercion(prem, goal, ev, &mut rem)
    }

    fn oriented_coercion(
        &mut self,
        prem: TermHandle,
        goal: TermHandle,
        ev: TermHandle,
        rem: &mut u64,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        #[derive(Clone, Copy)]
        struct R {
            p: TermHandle,
            g: TermHandle,
            e: TermHandle,
        }
        #[derive(Clone, Copy)]
        struct C {
            m: TermHandle,
            z: TermHandle,
            o: TermHandle,
            gz: TermHandle,
            go: TermHandle,
            e: TermHandle,
        }
        enum W {
            Coerce(R),
            Shape(R),
            Refute(R),
            Shuffle(R),
            ShuffleRest(R),
            ShuffleGoal(R),
            Cases(C),
        }
        enum P {
            Fallback(R),
            Refutation(R),
            RefuteFirst(R, TermHandle),
            FnArg(TermHandle, TermHandle, TermHandle, TermHandle),
            FnBody(TermHandle),
            ProdFirst(R, TermHandle, TermHandle, TermHandle, TermHandle),
            PairSecond(TermHandle),
            CaseZero(C),
            CaseOne(C, TermHandle),
            ShuffleFirst(R, TermHandle),
            ShuffleSecond(R),
            GoalFirst(R, TermHandle),
            TagZero(R, TermHandle),
            TagOne,
        }
        let r = |p, g, e| R { p, g, e };
        let (mut pending, mut work) = (Vec::new(), W::Coerce(r(prem, goal, ev)));
        'drive: loop {
            let mut val = match work {
                W::Coerce(x) => {
                    if let Some(n) = rem.checked_sub(1) {
                        *rem = n;
                        pending.push(P::Fallback(x));
                        if self.arena.structurally_equal(x.p, x.g) {
                            Some(x.e)
                        } else if let Some(inh) = self.closed_inhabitant(x.g)? {
                            Some(inh)
                        } else {
                            pending.push(P::Refutation(x));
                            work = W::Refute(x);
                            continue 'drive;
                        }
                    } else {
                        None
                    }
                }
                W::Refute(x) => {
                    if let Term::Sigma { domain, codomain } = self.arena.get(x.p)
                        && !self.occurs_free(codomain, 0)
                    {
                        let f = self.fst(x.e);
                        pending.push(P::RefuteFirst(x, codomain));
                        work = W::Refute(r(domain, x.g, f));
                        continue 'drive;
                    }
                    self.closed_relation_refutation(x.p, x.e, x.g)?
                }
                W::Shape(x) => match (self.arena.get(x.p), self.arena.get(x.g)) {
                    (
                        Term::Id { ty, left, right },
                        Term::Id {
                            ty: gty,
                            left: gl,
                            right: gr,
                        },
                    ) => {
                        if self.arena.structurally_equal(ty, gty)
                            && self.arena.structurally_equal(gl, right)
                            && self.arena.structurally_equal(gr, left)
                        {
                            Some(self.symmetry(ty, left, right, x.e))
                        } else {
                            self.boolean_goal_transport(x.p, x.g, x.e, true)?
                        }
                    }
                    (Term::Id { ty, .. }, _) if self.arena.structurally_equal(ty, self.two) => {
                        self.boolean_goal_transport(x.p, x.g, x.e, false)?
                    }
                    (
                        Term::Pi { domain, codomain },
                        Term::Pi {
                            domain: gd,
                            codomain: gc,
                        },
                    ) if !self.occurs_free(codomain, 0) && !self.occurs_free(gc, 0) => {
                        let h = self.var(0);
                        pending.push(P::FnArg(x.e, codomain, gd, gc));
                        work = W::Coerce(r(gd, domain, h));
                        continue 'drive;
                    }
                    (
                        Term::Sigma { domain, codomain },
                        Term::Sigma {
                            domain: gd,
                            codomain: gc,
                        },
                    ) => {
                        let f = self.fst(x.e);
                        pending.push(P::ProdFirst(x, domain, codomain, gd, gc));
                        work = W::Coerce(r(domain, gd, f));
                        continue 'drive;
                    }
                    _ => None,
                },
                W::Shuffle(x) => {
                    if let Term::Sigma { domain, codomain } = self.arena.get(x.p)
                        && !self.occurs_free(codomain, 0)
                    {
                        let proj = self.fst(x.e);
                        pending.push(P::ShuffleFirst(x, codomain));
                        work = W::Coerce(r(domain, x.g, proj));
                    } else {
                        work = W::ShuffleRest(x);
                    }
                    continue 'drive;
                }
                W::ShuffleRest(x) => {
                    if let Some((m, z, o)) = self.tagged_sum_parts(x.p) {
                        let (tag, sg, g) = (
                            self.var(0),
                            shift(&mut self.arena, x.g, 0, 2),
                            shift(&mut self.arena, x.g, 0, 1),
                        );
                        let pf = self.case_two(m, z, o, tag);
                        let body = self.pi(pf, sg);
                        let cm = self.lam(self.two, body);
                        work = W::Cases(C {
                            m: cm,
                            z,
                            o,
                            gz: g,
                            go: g,
                            e: x.e,
                        });
                    } else {
                        work = W::ShuffleGoal(x);
                    }
                    continue 'drive;
                }
                W::ShuffleGoal(x) => {
                    if let Term::Sigma { domain, codomain } = self.arena.get(x.g)
                        && !self.occurs_free(codomain, 0)
                    {
                        pending.push(P::GoalFirst(x, codomain));
                        work = W::Coerce(r(x.p, domain, x.e));
                        continue 'drive;
                    }
                    if let Some((_, z, o)) = self.tagged_sum_parts(x.g) {
                        pending.push(P::TagZero(x, o));
                        work = W::Coerce(r(x.p, z, x.e));
                        continue 'drive;
                    }
                    None
                }
                W::Cases(c) => {
                    let h = self.var(0);
                    pending.push(P::CaseZero(c));
                    work = W::Coerce(r(c.z, c.gz, h));
                    continue 'drive;
                }
            };
            loop {
                match pending.pop() {
                    Some(P::Fallback(x)) => {
                        if val.is_none() {
                            work = W::Shuffle(x);
                            continue 'drive;
                        }
                    }
                    Some(P::Refutation(x)) => {
                        if val.is_none() {
                            work = W::Shape(x);
                            continue 'drive;
                        }
                    }
                    Some(P::RefuteFirst(x, cod)) => {
                        if val.is_none() {
                            let s = self.snd(x.e);
                            work = W::Refute(r(cod, x.g, s));
                            continue 'drive;
                        }
                    }
                    Some(P::FnArg(e, cod, gd, gc)) => {
                        if let Some(arg) = val {
                            let f = shift(&mut self.arena, e, 0, 1);
                            let app = self.app(f, arg);
                            pending.push(P::FnBody(gd));
                            work = W::Coerce(r(cod, gc, app));
                            continue 'drive;
                        }
                    }
                    Some(P::FnBody(d)) => val = val.map(|b| self.lam(d, b)),
                    Some(P::ProdFirst(x, dom, cod, gd, gc)) => {
                        if let Some(first) = val {
                            let cases = match (self.arena.get(cod), self.arena.get(gc)) {
                                (
                                    Term::CaseTwo {
                                        motive: m,
                                        zero_branch: z,
                                        one_branch: o,
                                        scrutinee: s,
                                    },
                                    Term::CaseTwo {
                                        motive: gm,
                                        zero_branch: gz,
                                        one_branch: go,
                                        scrutinee: gs,
                                    },
                                ) if self.arena.structurally_equal(m, gm)
                                    && self.arena.structurally_equal(dom, gd)
                                    && matches!(self.arena.get(s), Term::Variable(0))
                                    && matches!(self.arena.get(gs), Term::Variable(0))
                                    && [m, z, o, gz, go]
                                        .into_iter()
                                        .all(|p| !self.occurs_free(p, 0)) =>
                                {
                                    let (t0, t1) = (self.var(0), self.var(1));
                                    let pf = self.case_two(m, z, o, t0);
                                    let (sm, sgz, sgo) = (
                                        shift(&mut self.arena, m, 0, 1),
                                        shift(&mut self.arena, gz, 0, 1),
                                        shift(&mut self.arena, go, 0, 1),
                                    );
                                    let gf = self.case_two(sm, sgz, sgo, t1);
                                    let body = self.pi(pf, gf);
                                    let cm = self.lam(self.two, body);
                                    Some(C {
                                        m: cm,
                                        z,
                                        o,
                                        gz,
                                        go,
                                        e: x.e,
                                    })
                                }
                                _ => None,
                            };
                            if let Some(c) = cases {
                                pending.push(P::PairSecond(first));
                                work = W::Cases(c);
                                continue 'drive;
                            }
                            if !self.occurs_free(cod, 0) && !self.occurs_free(gc, 0) {
                                let s = self.snd(x.e);
                                pending.push(P::PairSecond(first));
                                work = W::Coerce(r(cod, gc, s));
                                continue 'drive;
                            }
                            val = None;
                        }
                    }
                    Some(P::PairSecond(f)) => val = val.map(|s| self.pair(f, s)),
                    Some(P::CaseZero(c)) => {
                        if let Some(co) = val {
                            let zb = self.lam(c.z, co);
                            let h = self.var(0);
                            pending.push(P::CaseOne(c, zb));
                            work = W::Coerce(r(c.o, c.go, h));
                            continue 'drive;
                        }
                    }
                    Some(P::CaseOne(c, zb)) => {
                        if let Some(co) = val {
                            let ob = self.lam(c.o, co);
                            let (tag, arg) = (self.fst(c.e), self.snd(c.e));
                            let elim = self.case_two(c.m, zb, ob, tag);
                            val = Some(self.app(elim, arg));
                        }
                    }
                    Some(P::ShuffleFirst(x, cod)) => {
                        if val.is_none() {
                            let p = self.snd(x.e);
                            pending.push(P::ShuffleSecond(x));
                            work = W::Coerce(r(cod, x.g, p));
                            continue 'drive;
                        }
                    }
                    Some(P::ShuffleSecond(x)) => {
                        if val.is_none() {
                            work = W::ShuffleRest(x);
                            continue 'drive;
                        }
                    }
                    Some(P::GoalFirst(x, cod)) => {
                        if let Some(f) = val {
                            pending.push(P::PairSecond(f));
                            work = W::Coerce(r(x.p, cod, x.e));
                            continue 'drive;
                        }
                    }
                    Some(P::TagZero(x, o)) => {
                        if let Some(p) = val {
                            val = Some(self.pair(self.two_zero, p));
                        } else {
                            pending.push(P::TagOne);
                            work = W::Coerce(r(x.p, o, x.e));
                            continue 'drive;
                        }
                    }
                    Some(P::TagOne) => val = val.map(|p| self.pair(self.two_one, p)),
                    None => return Ok(val),
                }
            }
        }
    }

    fn source_rewrite_proof(
        &mut self,
        goal: TermHandle,
        ids: &[(TermHandle, TermHandle)],
        solve: impl FnOnce(&mut Self, TermHandle) -> Result<Option<TermHandle>, BoundedDenotationError>,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let int = self.integer_constant()?;
        let mut eqs = Vec::new();
        for &(stmt, ev) in ids {
            let Some((c, l, r)) = self.identity_parts(stmt) else {
                continue;
            };
            if !self.arena.structurally_equal(c, int) {
                continue;
            }
            let is_unk = |t| {
                matches!(self.arena.get(t), Term::Constant { declaration: d, .. }
                if self.tables.closed_math_values.get(d as usize).and_then(Option::as_ref).is_none()
                    && self.declarations.get(d as usize).is_some_and(|x| x.body.is_none()))
            };
            let (from, to, ev) = if is_unk(l) {
                (l, r, ev)
            } else if is_unk(r) {
                (r, l, self.symmetry(c, l, r, ev))
            } else {
                continue;
            };
            if from != to && !eqs.iter().any(|e: &Equation| e.from == from) {
                eqs.push(Equation {
                    carrier: c,
                    from,
                    to,
                    evidence: ev,
                });
            }
        }
        let mut rem = MAX_ELABORATION_NODES;
        let Some((normal, reps)) = self.expand_equations(goal, &eqs, &mut rem)? else {
            return Ok(None);
        };
        let Some(mut ev) = solve(self, normal)? else {
            return Ok(None);
        };
        for rep in reps.into_iter().rev() {
            ev = self.replace_evidence(rep, ev, true);
        }
        Ok(Some(ev))
    }

    fn compositional_transport(
        &mut self,
        prem: &Proposition,
        mut ev: TermHandle,
        eq_nodes: &[ProofNode],
        eq_evs: &[TermHandle],
        concl: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let truth_prem = matches!(prem, Proposition::Truth);
        let (pt, gt) = (self.denote(prem)?, self.denote(concl)?);
        if self.arena.structurally_equal(pt, gt) {
            return Ok(Some(ev));
        }
        let mut eqs = Vec::new();
        for (node, &proof) in eq_nodes.iter().zip(eq_evs) {
            let Proposition::Equal(left, _) = &node.conclusion else {
                return Ok(None);
            };
            let (refl, id) = (
                self.denote(&Proposition::Equal(left.clone(), left.clone()))?,
                self.denote(&node.conclusion)?,
            );
            let (Some((_, from, _)), Some((c, a, b))) =
                (self.identity_parts(refl), self.identity_parts(id))
            else {
                return Ok(None);
            };
            let (to, proof) = if self.arena.structurally_equal(a, from) {
                (b, proof)
            } else if self.arena.structurally_equal(b, from) {
                (a, self.symmetry(c, a, b, proof))
            } else {
                return Ok(None);
            };
            if self.arena.structurally_equal(from, to)
                || eqs.iter().any(|p: &Equation| p.from == from)
            {
                continue;
            }
            if !matches!(self.arena.get(from), Term::Constant { .. }) {
                return Ok(None);
            }
            eqs.push(Equation {
                carrier: c,
                from,
                to,
                evidence: proof,
            });
        }
        let mut rem = MAX_ELABORATION_NODES;
        let Some((np, fwd)) = self.expand_equations(pt, &eqs, &mut rem)? else {
            return Ok(None);
        };
        let Some((ng, bwd)) = self.expand_equations(gt, &eqs, &mut rem)? else {
            return Ok(None);
        };
        if truth_prem
            && let Some((ty, v, _)) = self
                .identity_parts(ng)
                .filter(|(_, l, r)| self.arena.structurally_equal(*l, *r))
        {
            ev = self.refl(ty, v);
        } else {
            for rep in fwd {
                ev = self.replace_evidence(rep, ev, false);
            }
            if !self.arena.structurally_equal(np, ng) {
                let Some(o) = self.oriented_evidence(np, ng, ev)? else {
                    return Ok(None);
                };
                ev = o;
            }
        }
        for rep in bwd.into_iter().rev() {
            ev = self.replace_evidence(rep, ev, true);
        }
        Ok(Some(ev))
    }

    fn expand_equations(
        &mut self,
        mut stmt: TermHandle,
        eqs: &[Equation],
        rem: &mut u64,
    ) -> Result<Option<(TermHandle, Vec<Replacement>)>, BoundedDenotationError> {
        let mut reps = Vec::new();
        for _ in 0..=eqs.len() {
            let mut changed = false;
            for &eq in eqs {
                let Some((next, ctx)) = self.replacement_context(stmt, eq, 0, rem)? else {
                    return Ok(None);
                };
                if next != stmt {
                    reps.push(Replacement {
                        equation: eq,
                        context: ctx,
                    });
                    stmt = next;
                    changed = true;
                }
            }
            if !changed {
                return Ok(Some((stmt, reps)));
            }
        }
        Ok(None)
    }

    fn replace_evidence(&mut self, rep: Replacement, ev: TermHandle, reverse: bool) -> TermHandle {
        let Equation {
            carrier,
            from,
            to,
            evidence: eq,
        } = rep.equation;
        let (from, to, eq) = if reverse {
            (to, from, self.symmetry(carrier, from, to, eq))
        } else {
            (from, to, eq)
        };
        let body = shift(&mut self.arena, rep.context, 0, 1);
        self.id_elim(carrier, from, to, eq, ev, body)
    }

    fn replacement_context(
        &mut self,
        orig: TermHandle,
        eq: Equation,
        depth: u32,
        rem: &mut u64,
    ) -> Result<Option<(TermHandle, TermHandle)>, BoundedDenotationError> {
        let Some(r) = rem.checked_sub(1) else {
            return Ok(None);
        };
        *rem = r;
        if orig == eq.from {
            return Ok(Some((eq.to, self.var(depth))));
        }
        let term = self.arena.get(orig);
        match term {
            Term::Variable(p) => {
                let s = if p >= depth {
                    p.checked_add(1)
                        .ok_or(BoundedDenotationError::DepthLimitExceeded)?
                } else {
                    p
                };
                return Ok(Some((orig, if s == p { orig } else { self.var(s) })));
            }
            Term::Constant { .. } | Term::Sort(_) | Term::Two | Term::TwoZero | Term::TwoOne => {
                return Ok(Some((orig, orig)));
            }
            _ => {}
        }
        let Some((cs, len, binds)) = Self::term_children(&term) else {
            return Ok(None);
        };
        let (mut rws, mut ctxs) = (cs, cs);
        for i in 0..len {
            let d = if binds && i == 1 {
                depth
                    .checked_add(1)
                    .ok_or(BoundedDenotationError::DepthLimitExceeded)?
            } else {
                depth
            };
            let Some((rw, ctx)) = self.replacement_context(cs[i], eq, d, rem)? else {
                return Ok(None);
            };
            rws[i] = rw;
            ctxs[i] = ctx;
        }
        let (rw_h, ctx_h) = (
            self.rebuild_term(&term, orig, rws),
            self.rebuild_term(&term, orig, ctxs),
        );
        Ok(Some((rw_h, ctx_h)))
    }
}

// ============================================================================
// Parameterized arithmetic bounds, casts, order translation & forbidden roots
// ============================================================================

struct BinaryBoundSpec {
    op: BinaryOp,
    target: TermMatcher,
    is_target_math: fn(&IntegerMathTerm) -> bool,
    right_antitone: bool,
    monotone_law: CoreLaw,
}

const BINARY_BOUND_SPECS: [BinaryBoundSpec; 2] = [
    BinaryBoundSpec {
        op: BinaryOp::Add,
        target: exact_integer_add,
        is_target_math: is_math_add,
        right_antitone: false,
        monotone_law: CoreLaw::AddMonotoneBoth,
    },
    BinaryBoundSpec {
        op: BinaryOp::Sub,
        target: exact_integer_subtract,
        is_target_math: is_math_subtract,
        right_antitone: true,
        monotone_law: CoreLaw::SubMonotoneAntitone,
    },
];

struct BinaryBoundChain {
    left_ep: ScalarTerm,
    right_ep: ScalarTerm,
    left_ep_t: TermHandle,
    right_ep_t: TermHandle,
    left_t: TermHandle,
    right_t: TermHandle,
    bound_op: TermHandle,
    operand_op: TermHandle,
    left_ord: TermHandle,
    right_ord: TermHandle,
}

impl Denotation {
    fn bound_endpoint(
        &mut self,
        op: &ScalarTerm,
        prop: &Proposition,
        ev: TermHandle,
        lower: bool,
        ity: &IntegerType,
    ) -> Result<Option<(ScalarTerm, TermHandle)>, BoundedDenotationError> {
        if op.integer_value().is_some() {
            if prop != &Proposition::Truth {
                return Ok(None);
            }
            let t = self.fixed_scalar_term(op)?;
            return Ok(Some((op.clone(), self.le_refl(t)?)));
        }
        match prop {
            Proposition::Truth if op.scalar_type() == ScalarType::Integer(*ity) => {
                self.carrier_bound(op, lower, ity).map(Some)
            }
            Proposition::Equal(a, b) => {
                let lit = if a == op {
                    b
                } else if b == op {
                    a
                } else {
                    return Ok(None);
                };
                if !matches!(lit.integer_value(), Some((t, _)) if t == *ity) {
                    return Ok(None);
                }
                let (ot, lt) = (self.fixed_scalar_term(op)?, self.fixed_scalar_term(lit)?);
                let (wf, wt) = if lower { (lt, ot) } else { (ot, lt) };
                let Some(eq) = self.directed_prop_eq(prop, ev, wf, wt)? else {
                    return Ok(None);
                };
                self.le_of_eq(wf, wt, eq).map(|o| Some((lit.clone(), o)))
            }
            Proposition::LessOrEqual(a, b) if a != b => {
                let bd = if b == op && lower {
                    a
                } else if a == op && !lower {
                    b
                } else {
                    return Ok(None);
                };
                if !matches!(bd.integer_value(), Some((t, _)) if t == *ity) {
                    return Ok(None);
                }
                Ok(Some((bd.clone(), ev)))
            }
            _ => Ok(None),
        }
    }

    fn binary_bound_chain(
        &mut self,
        spec: &BinaryBoundSpec,
        l: (&ScalarTerm, &Proposition, TermHandle),
        r: (&ScalarTerm, &Proposition, TermHandle),
        lower: bool,
        ity: &IntegerType,
    ) -> Result<Option<BinaryBoundChain>, BoundedDenotationError> {
        let r_lower = if spec.right_antitone { !lower } else { lower };
        let (Some((left_ep, left_ord)), Some((right_ep, right_ord))) = (
            self.bound_endpoint(l.0, l.1, l.2, lower, ity)?,
            self.bound_endpoint(r.0, r.1, r.2, r_lower, ity)?,
        ) else {
            return Ok(None);
        };
        let (left_ep_t, right_ep_t, left_t, right_t) = (
            self.fixed_scalar_term(&left_ep)?,
            self.fixed_scalar_term(&right_ep)?,
            self.fixed_scalar_term(l.0)?,
            self.fixed_scalar_term(r.0)?,
        );
        let (bound_op, operand_op) = (
            self.binary_terms(spec.op, left_ep_t, right_ep_t)?,
            self.binary_terms(spec.op, left_t, right_t)?,
        );
        Ok(Some(BinaryBoundChain {
            left_ep,
            right_ep,
            left_ep_t,
            right_ep_t,
            left_t,
            right_t,
            bound_op,
            operand_op,
            left_ord,
            right_ord,
        }))
    }

    fn binary_bound_order(
        &mut self,
        spec: &BinaryBoundSpec,
        lower: bool,
        c: &BinaryBoundChain,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let args = if lower {
            [
                c.left_ep_t,
                c.left_t,
                c.right_ep_t,
                c.right_t,
                c.left_ord,
                c.right_ord,
            ]
        } else {
            [
                c.left_t,
                c.left_ep_t,
                c.right_t,
                c.right_ep_t,
                c.left_ord,
                c.right_ord,
            ]
        };
        self.law(spec.monotone_law, &args)
    }

    fn direct_binary_bound_evidence(
        &mut self,
        spec: &BinaryBoundSpec,
        prem: &Proposition,
        ev: TermHandle,
        w: &IntegerAffineWitness,
        concl: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::Conjunction(bs) = prem else {
            return Ok(None);
        };
        let [lb, rb] = bs.as_slice() else {
            return Ok(None);
        };
        let Some((ty, l, r)) = (spec.target)(&w.target) else {
            return Ok(None);
        };
        if ty.carrier() != IntegerCarrier::Fixed
            || w.root != *l
            || !w.definition_axioms.is_empty()
            || !w.literal_axioms.is_empty()
        {
            return Ok(None);
        }
        let Some((lower, bd, res)) = math_less_or_equal_sides(concl, spec.is_target_math) else {
            return Ok(None);
        };
        let IntegerMathTerm::IntegerLiteral(blit) = bd else {
            return Ok(None);
        };
        let Some(bv) = math_literal_value(*blit) else {
            return Ok(None);
        };
        let (le, re) = (self.fst(ev), self.snd(ev));
        let Some(c) = self.binary_bound_chain(spec, (l, lb, le), (r, rb, re), lower, ty)? else {
            return Ok(None);
        };
        let rt = self.math_term(res)?;
        if !self.arena.structurally_equal(rt, c.operand_op) {
            return Ok(None);
        }
        let ord = self.binary_bound_order(spec, lower, &c)?;
        let (Some((_, lv)), Some((_, rv))) =
            (c.left_ep.integer_value(), c.right_ep.integer_value())
        else {
            return Ok(None);
        };
        let bt = self.math_term(bd)?;
        let Some(eq) =
            self.numeral_binary_equation(spec.op, lv, rv, bv, c.left_ep_t, c.right_ep_t, bt)?
        else {
            return Ok(None);
        };
        self.ord_subst(false, lower, c.bound_op, bt, c.operand_op, eq, ord)
            .map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    fn exact_binary_definition_bound_evidence(
        &mut self,
        spec: &BinaryBoundSpec,
        lb: &Proposition,
        lev: TermHandle,
        rb: &Proposition,
        rev: TermHandle,
        def: &Proposition,
        dev: TermHandle,
        concl: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::LessOrEqual(cl, cr) = concl else {
            return Ok(None);
        };
        let (lit, out, lower) = if cl.integer_value().is_some() {
            (cl, cr, true)
        } else if cr.integer_value().is_some() {
            (cr, cl, false)
        } else {
            return Ok(None);
        };
        let (dout, expr) = match def {
            Proposition::Equal(a, b)
                if matches!(a, ScalarTerm::Value { .. }) && (spec.target)(b).is_some() =>
            {
                (a, b)
            }
            Proposition::Equal(a, b)
                if matches!(b, ScalarTerm::Value { .. }) && (spec.target)(a).is_some() =>
            {
                (b, a)
            }
            _ => return Ok(None),
        };
        let Some((ty, l, r)) = (spec.target)(expr) else {
            return Ok(None);
        };
        if ty.carrier() != IntegerCarrier::Fixed
            || dout != out
            || out.scalar_type() != ScalarType::Integer(*ty)
        {
            return Ok(None);
        }
        let Some(c) = self.binary_bound_chain(spec, (l, lb, lev), (r, rb, rev), lower, ty)? else {
            return Ok(None);
        };
        let ord = self.binary_bound_order(spec, lower, &c)?;
        let (ot, et) = (self.fixed_scalar_term(out)?, self.fixed_scalar_term(expr)?);
        let res_eq = if self.arena.structurally_equal(et, c.operand_op) {
            let Some(eq) = self.directed_prop_eq(def, dev, c.operand_op, ot)? else {
                return Ok(None);
            };
            eq
        } else {
            let (Some((_, lv)), Some((_, rv)), Some((_, ev))) =
                (l.integer_value(), r.integer_value(), expr.integer_value())
            else {
                return Ok(None);
            };
            let Some(br) =
                self.numeral_binary_equation(spec.op, lv, rv, ev, c.left_t, c.right_t, et)?
            else {
                return Ok(None);
            };
            let Some(fwd) = self.directed_prop_eq(def, dev, et, ot)? else {
                return Ok(None);
            };
            let int = self.integer_constant()?;
            self.transitivity(int, c.operand_op, et, ot, br, fwd)
        };
        let ord = self.ord_subst(false, !lower, c.operand_op, ot, c.bound_op, res_eq, ord)?;
        let (Some((_, lv)), Some((_, rv)), Some((_, bv))) = (
            c.left_ep.integer_value(),
            c.right_ep.integer_value(),
            lit.integer_value(),
        ) else {
            return Ok(None);
        };
        let lt = self.fixed_scalar_term(lit)?;
        let Some(eq) =
            self.numeral_binary_equation(spec.op, lv, rv, bv, c.left_ep_t, c.right_ep_t, lt)?
        else {
            return Ok(None);
        };
        self.ord_subst(false, lower, c.bound_op, lt, ot, eq, ord)
            .map(Some)
    }

    fn substitute_endpoint_order(
        &mut self,
        left_side: bool,
        eq: Option<(TermHandle, &Proposition)>,
        old: TermHandle,
        new: TermHandle,
        other: TermHandle,
        ord: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some((ev, prop)) = eq else {
            return Ok(Some(ord));
        };
        let Some(dir) = self.directed_prop_eq(prop, ev, old, new)? else {
            return Ok(None);
        };
        self.ord_subst(false, left_side, old, new, other, dir, ord)
            .map(Some)
    }

    fn correlated_add_sub_bound_evidence(
        &mut self,
        is_add: bool,
        prem: &Proposition,
        mut ev: TermHandle,
        w: &IntegerAffineWitness,
        concl: &Proposition,
        defs: &[(Proposition, TermHandle)],
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some((ty, l, r)) = (if is_add {
            exact_integer_add(&w.target)
        } else {
            exact_integer_subtract(&w.target)
        }) else {
            return Ok(None);
        };
        if ty.carrier() != IntegerCarrier::Fixed {
            return Ok(None);
        }
        let Some((lower, bd, em)) = math_less_or_equal_sides(
            concl,
            if is_add {
                is_math_add
            } else {
                is_math_subtract
            },
        ) else {
            return Ok(None);
        };
        let (bv, ev_v) = (eval_closed_math(bd)?, eval_closed_math(em)?);
        if let (Some(bv), Some(ev_v)) = (&bv, &ev_v) {
            let (gl, gr, lv, rv) = if lower {
                (bd, em, bv, ev_v)
            } else {
                (em, bd, ev_v, bv)
            };
            return match lv.cmp(rv) {
                Ordering::Less => match (integer_value_of(lv), integer_value_of(rv)) {
                    (Some(lo), Some(hi)) => {
                        let (s, lt, rt) = (
                            self.literal_order(lo, hi)?,
                            self.math_term(gl)?,
                            self.math_term(gr)?,
                        );
                        self.le_of_lt(lt, rt, s).map(Some)
                    }
                    _ => Ok(None),
                },
                Ordering::Equal => {
                    let (lt, rt, int) = (
                        self.math_term(gl)?,
                        self.math_term(gr)?,
                        self.integer_constant()?,
                    );
                    let rf = self.refl(int, lt);
                    self.le_of_eq(lt, rt, rf).map(Some)
                }
                Ordering::Greater => {
                    let Proposition::LessOrEqual(pl, pr) = prem else {
                        return Ok(None);
                    };
                    let (Some((_, plv)), Some((_, prv))) = (pl.integer_value(), pr.integer_value())
                    else {
                        return Ok(None);
                    };
                    if unbounded_integer(plv) <= unbounded_integer(prv) {
                        return Ok(None);
                    }
                    let (plt, prt, s) = (
                        self.fixed_scalar_term(pl)?,
                        self.fixed_scalar_term(pr)?,
                        self.literal_order(prv, plv)?,
                    );
                    let loop_ev = self.ord_trans(true, false, prt, plt, prt, s, ev)?;
                    let absurd = self.irreflexive(prt, loop_ev)?;
                    let ty = self.denote(concl)?;
                    Ok(Some(self.empty_elim(ty, absurd)))
                }
            };
        }
        if ev_v.is_some() {
            return Ok(None);
        }
        let Some((rexpr, req)) = affine_witness_root_expression(w, defs) else {
            return Ok(None);
        };
        let Some((ity, ep, sib)) = (if is_add {
            exact_integer_subtract(rexpr)
        } else {
            exact_integer_add(rexpr)
        }) else {
            return Ok(None);
        };
        if ity != ty || sib != r || prem != &affine_root_premise(w, lower, l) {
            return Ok(None);
        }
        let Some((epeq, eplit)) = affine_carrier_endpoint_equality(*ty, lower, ep, defs) else {
            return Ok(None);
        };
        let (lt, rt, ept, rext, roott) = (
            self.fixed_scalar_term(l)?,
            self.fixed_scalar_term(r)?,
            self.fixed_scalar_term(ep)?,
            self.fixed_scalar_term(rexpr)?,
            self.fixed_scalar_term(&w.root)?,
        );
        let Some(step) = self.substitute_endpoint_order(lower, req, roott, rext, lt, ev)? else {
            return Ok(None);
        };
        ev = step;
        let (ord, swapped) = if is_add {
            let (sum, canc) = (
                self.binary_terms(BinaryOp::Add, lt, rt)?,
                self.binary_terms(BinaryOp::Add, rext, rt)?,
            );
            let args = if lower {
                [rext, lt, rt, ev]
            } else {
                [lt, rext, rt, ev]
            };
            let ord = self.law(CoreLaw::AddMonotone, &args)?;
            let eq = if rexpr.integer_value().is_some() {
                let (Some((_, dv)), Some((_, rv)), Some((_, evv))) = (
                    rexpr.integer_value(),
                    sib.integer_value(),
                    ep.integer_value(),
                ) else {
                    return Ok(None);
                };
                let Some(e) =
                    self.numeral_binary_equation(BinaryOp::Add, dv, rv, evv, rext, rt, ept)?
                else {
                    return Ok(None);
                };
                e
            } else {
                self.law(CoreLaw::AddCancelSub, &[ept, rt])?
            };
            (self.ord_subst(false, lower, canc, ept, sum, eq, ord)?, sum)
        } else {
            let sum = self.binary_terms(BinaryOp::Add, ept, rt)?;
            if !self.arena.structurally_equal(rext, sum) {
                let (Some((_, evv)), Some((_, av)), Some((_, tv))) =
                    (ep.integer_value(), r.integer_value(), rexpr.integer_value())
                else {
                    return Ok(None);
                };
                let Some(eq) =
                    self.numeral_binary_equation(BinaryOp::Add, evv, av, tv, ept, rt, rext)?
                else {
                    return Ok(None);
                };
                let int = self.integer_constant()?;
                let eq = self.symmetry(int, sum, rext, eq);
                ev = self.ord_subst(false, lower, rext, sum, lt, eq, ev)?;
            }
            let law = if lower {
                CoreLaw::SubCancelAddLeft
            } else {
                CoreLaw::SubCancelAddRight
            };
            (
                self.law(law, &[ept, lt, rt, ev])?,
                self.binary_terms(BinaryOp::Sub, lt, rt)?,
            )
        };
        let litt = self.fixed_scalar_term(&eplit)?;
        self.substitute_endpoint_order(lower, epeq, ept, litt, swapped, ord)
    }

    fn correlated_multiply_bound_evidence(
        &mut self,
        prem: &Proposition,
        ev: TermHandle,
        w: &IntegerAffineWitness,
        concl: &Proposition,
        defs: &[(Proposition, TermHandle)],
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type: ty,
            left: l,
            right: r,
        } = &w.target
        else {
            return Ok(None);
        };
        if ty.carrier() != IntegerCarrier::Fixed {
            return Ok(None);
        }
        let Some((lower, bd, prod)) = math_less_or_equal_sides(concl, is_math_multiply) else {
            return Ok(None);
        };
        let IntegerMathTerm::IntegerLiteral(blit) = bd else {
            return Ok(None);
        };
        let Some(epv) = (if lower {
            ty.minimum_value()
        } else {
            ty.maximum_value()
        }) else {
            return Ok(None);
        };
        if math_literal_value(*blit).map(unbounded_integer) != Some(unbounded_integer(epv)) {
            return Ok(None);
        }
        let Proposition::Conjunction(parts) = prem else {
            return Ok(None);
        };
        let [sign_p, bd_p] = parts.as_slice() else {
            return Ok(None);
        };
        let positive = ScalarTerm::integer(*ty, ty.one())
            .ok()
            .is_some_and(|o| sign_p == &Proposition::LessOrEqual(o, (**r).clone()));
        let negative = ScalarTerm::integer(*ty, IntegerValue::Signed(-2))
            .ok()
            .is_some_and(|n| sign_p == &Proposition::LessOrEqual((**r).clone(), n));
        if (!positive && !negative) || bd_p != &affine_root_premise(w, lower == positive, l) {
            return Ok(None);
        }
        let Some((expr, deq)) = affine_witness_root_expression(w, defs) else {
            return Ok(None);
        };
        let ScalarTerm::ExactIntegerDivide {
            scalar_type: dty,
            left: ep,
            right: div,
        } = expr
        else {
            return Ok(None);
        };
        if dty != ty || div != r {
            return Ok(None);
        }
        let Some((epeq, eplit)) = affine_carrier_endpoint_equality(*ty, lower, ep, defs) else {
            return Ok(None);
        };
        let (lt, rt, ept, et, bt, litt, roott) = (
            self.fixed_scalar_term(l)?,
            self.fixed_scalar_term(r)?,
            self.fixed_scalar_term(ep)?,
            self.fixed_scalar_term(expr)?,
            self.math_term(bd)?,
            self.fixed_scalar_term(&eplit)?,
            self.fixed_scalar_term(&w.root)?,
        );
        let (pt, op_prod) = (
            self.math_term(prod)?,
            self.binary_terms(BinaryOp::Mul, lt, rt)?,
        );
        if !self.arena.structurally_equal(bt, litt) || !self.arena.structurally_equal(pt, op_prod) {
            return Ok(None);
        }
        let (sign, bev) = (self.fst(ev), self.snd(ev));
        let Some(bev) =
            self.substitute_endpoint_order(lower == positive, deq, roott, et, lt, bev)?
        else {
            return Ok(None);
        };
        let zero = self.binary_term(false, 0)?;
        let sign_t = if positive {
            sign
        } else {
            let (m1, m2, s) = (
                self.binary_term(true, 1)?,
                self.binary_term(true, 2)?,
                self.literal_order(IntegerValue::Signed(-2), IntegerValue::Signed(-1))?,
            );
            let step = self.le_of_lt(m2, m1, s)?;
            self.ord_trans(false, false, rt, m2, m1, sign, step)?
        };
        let Some(num) = (if lower {
            self.integer_le_numeral(epv, IntegerValue::Unsigned(0))
        } else {
            self.integer_le_numeral(IntegerValue::Unsigned(0), epv)
        })?
        else {
            return Ok(None);
        };
        let Some(eside) = self.substitute_endpoint_order(lower, epeq, litt, ept, zero, num)? else {
            return Ok(None);
        };
        let res = self.law(
            CoreLaw::DivBound {
                ty: *ty,
                lower,
                positive,
            },
            &[ept, rt, eside, sign_t],
        )?;
        let (a, b) = if lower == positive {
            (et, lt)
        } else {
            (lt, et)
        };
        let mono = self.law(
            if positive {
                CoreLaw::MulMonotonePos
            } else {
                CoreLaw::MulAntitoneNeg
            },
            &[a, b, rt, bev, sign_t],
        )?;
        let eprod = self.binary_terms(BinaryOp::Mul, et, rt)?;
        let comb = if lower {
            self.ord_trans(false, false, ept, eprod, op_prod, res, mono)?
        } else {
            self.ord_trans(false, false, op_prod, eprod, ept, mono, res)?
        };
        self.substitute_endpoint_order(lower, epeq, ept, litt, op_prod, comb)
    }

    fn unsigned_subtract_bound_evidence(
        &mut self,
        prem: &Proposition,
        ev: TermHandle,
        w: &IntegerAffineWitness,
        goal: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type: ty,
            left: l,
            right: r,
        } = &w.target
        else {
            return Ok(None);
        };
        let direct =
            |t: &ScalarTerm| matches!(t, ScalarTerm::Value { .. } | ScalarTerm::Integer { .. });
        if ty.carrier() != IntegerCarrier::Fixed
            || ty.sign() != IntegerSign::Unsigned
            || !w.definition_axioms.is_empty()
            || !w.literal_axioms.is_empty()
            || &w.root != r.as_ref()
            || l == r
            || !direct(l)
            || !direct(r)
            || prem != &Proposition::LessOrEqual((**r).clone(), (**l).clone())
        {
            return Ok(None);
        }
        let (lt, rt, z, gty) = (
            self.fixed_scalar_term(l)?,
            self.fixed_scalar_term(r)?,
            self.binary_term(false, 0)?,
            self.denote(goal)?,
        );
        let Some(IntegerRelation::LessOrEqual { right: diff, .. }) = self.integer_relation(gty)
        else {
            unreachable!()
        };
        if let (Some((_, IntegerValue::Unsigned(lv))), Some((_, IntegerValue::Unsigned(rv)))) =
            (l.integer_value(), r.integer_value())
        {
            if lv < rv {
                let ord =
                    self.literal_order(IntegerValue::Unsigned(lv), IntegerValue::Unsigned(rv))?;
                let c = self.ord_trans(true, false, lt, rt, lt, ord, ev)?;
                let absurd = self.irreflexive(lt, c)?;
                return Ok(Some(self.empty_elim(gty, absurd)));
            }
            let s =
                self.literal_order(IntegerValue::Unsigned(0), IntegerValue::Unsigned(lv - rv))?;
            return self.le_of_lt(z, diff, s).map(Some);
        }
        let sdiff = self.binary_terms(BinaryOp::Sub, lt, lt)?;
        let (ord, eq) = (
            self.law(CoreLaw::SubLeAntitone, &[lt, rt, lt, ev])?,
            self.law(CoreLaw::SubSelfZero, &[lt])?,
        );
        self.ord_subst(false, true, sdiff, z, diff, eq, ord)
            .map(Some)
    }

    fn sub_below_self(
        &mut self,
        minuend: TermHandle,
        subtrahend: TermHandle,
        diff: TermHandle,
        pos_ev: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let z = self.binary_term(false, 0)?;
        let sz = self.binary_terms(BinaryOp::Sub, minuend, z)?;
        let (dec, eq) = (
            self.law(CoreLaw::SubAntitone, &[minuend, z, subtrahend, pos_ev])?,
            self.law(CoreLaw::SubZero, &[minuend])?,
        );
        self.ord_subst(true, false, sz, minuend, diff, eq, dec)
    }

    fn subtraction_evidence(
        &mut self,
        diff: &Proposition,
        dev: TermHandle,
        pev: TermHandle,
        goal: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Proposition::Equal(res, sub @ ScalarTerm::ExactIntegerSubtract { left, right, .. }) =
            diff
        else {
            unreachable!()
        };
        let (rest, lt, rt, subt, z) = (
            self.fixed_scalar_term(res)?,
            self.fixed_scalar_term(left)?,
            self.fixed_scalar_term(right)?,
            self.fixed_scalar_term(sub)?,
            self.binary_term(false, 0)?,
        );
        let dec = if let Some((_, v)) = sub.integer_value() {
            let (_, d) = right.integer_value().expect("evaluated operands");
            if matches!(d, IntegerValue::Signed(x) if x <= 0) || d == IntegerValue::Unsigned(0) {
                let absurd = if d == IntegerValue::Signed(0) || d == IntegerValue::Unsigned(0) {
                    self.irreflexive(z, pev)?
                } else {
                    self.order_contradiction(true, z, rt, IntegerValue::Signed(0), d, pev)?
                };
                let ty = self.denote(goal)?;
                return Ok(self.empty_elim(ty, absurd));
            }
            self.literal_order(v, left.integer_value().expect("evaluated").1)?
        } else {
            self.sub_below_self(lt, rt, subt, pev)?
        };
        let int = self.integer_constant()?;
        let eq = self.symmetry(int, rest, subt, dev);
        self.ord_subst(true, true, subt, rest, lt, eq, dec)
    }

    fn addition_order_evidence(
        &mut self,
        sum: &Proposition,
        sev: TermHandle,
        pev: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::Equal(res, add @ ScalarTerm::ExactIntegerAdd { left, right, .. }) = sum
        else {
            unreachable!()
        };
        if add.integer_value().is_some() {
            return Ok(None);
        }
        let (int, rest, orig, inc, addt) = (
            self.integer_constant()?,
            self.fixed_scalar_term(res)?,
            self.fixed_scalar_term(left)?,
            self.fixed_scalar_term(right)?,
            self.fixed_scalar_term(add)?,
        );
        let fwd = self.symmetry(int, rest, addt, sev);
        let inv = self.law(CoreLaw::AddRightInv, &[orig, inc, rest, fwd])?;
        let diff = self.binary_terms(BinaryOp::Sub, rest, inc)?;
        let below = self.sub_below_self(rest, inc, diff, pev)?;
        let back = self.symmetry(int, orig, diff, inv);
        self.ord_subst(true, true, diff, orig, rest, back, below)
            .map(Some)
    }

    fn subtraction_antitone_evidence(
        &mut self,
        sm: &Proposition,
        sev: TermHandle,
        lg: &Proposition,
        lev: TermHandle,
        oev: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let (
            Proposition::Equal(
                sres,
                low @ ScalarTerm::ExactIntegerSubtract {
                    left: m,
                    right: big,
                    ..
                },
            ),
            Proposition::Equal(lres, high @ ScalarTerm::ExactIntegerSubtract { right: sml, .. }),
        ) = (sm, lg)
        else {
            return Ok(None);
        };
        if low.integer_value().is_some() || high.integer_value().is_some() {
            return Ok(None);
        }
        let (int, st, lt, mt, bgt, smt, lot, hit) = (
            self.integer_constant()?,
            self.fixed_scalar_term(sres)?,
            self.fixed_scalar_term(lres)?,
            self.fixed_scalar_term(m)?,
            self.fixed_scalar_term(big)?,
            self.fixed_scalar_term(sml)?,
            self.fixed_scalar_term(low)?,
            self.fixed_scalar_term(high)?,
        );
        let dec = self.law(CoreLaw::SubAntitone, &[mt, smt, bgt, oev])?;
        let lb = self.symmetry(int, st, lot, sev);
        let left = self.ord_subst(true, true, lot, st, hit, lb, dec)?;
        let hb = self.symmetry(int, lt, hit, lev);
        self.ord_subst(true, false, hit, lt, st, hb, left).map(Some)
    }

    fn mathematical_order_translation(
        &mut self,
        rel: &Proposition,
        addend: &IntegerMathTerm,
        ev: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Proposition::IntegerMathLessOrEqual(l, r) = rel else {
            return Err(BoundedDenotationError::Certificate(
                ProofError::RulePremiseMismatch("mathematical integer order translation"),
            ));
        };
        let (lt, rt, at) = (
            self.math_term(l)?,
            self.math_term(r)?,
            self.math_term(addend)?,
        );
        let mut res = self.law(CoreLaw::AddMonotone, &[lt, rt, at, ev])?;
        let (ls, rs) = (
            self.binary_terms(BinaryOp::Add, lt, at)?,
            self.binary_terms(BinaryOp::Add, rt, at)?,
        );
        let (lr, rr) = (
            self.math_term(&IntegerMathTerm::Add(
                Box::new(l.clone()),
                Box::new(addend.clone()),
            ))?,
            self.math_term(&IntegerMathTerm::Add(
                Box::new(r.clone()),
                Box::new(addend.clone()),
            ))?,
        );
        for (left_side, sum, target, orig, other) in
            [(true, ls, lr, lt, rs), (false, rs, rr, rt, lr)]
        {
            if !self.arena.structurally_equal(sum, target) {
                let iv = |t| {
                    self.closed_math_value(t)
                        .as_ref()
                        .and_then(integer_value_of)
                };
                let (Some(a), Some(b), Some(c)) = (iv(orig), iv(at), iv(target)) else {
                    return Err(BoundedDenotationError::Unsupported(
                        "translated numeral exceeds existing numeral evidence",
                    ));
                };
                let eq = self
                    .numeral_binary_equation(BinaryOp::Add, a, b, c, orig, at, target)?
                    .ok_or(BoundedDenotationError::Unsupported(
                        "translated numeral lacks exact addition evidence",
                    ))?;
                res = self.ord_subst(false, left_side, sum, target, other, eq, res)?;
            }
        }
        Ok(res)
    }

    fn order_antisymmetry_evidence(
        &mut self,
        order: &Proposition,
        fwd: TermHandle,
        bwd: TermHandle,
        concl: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (lt, rt) = match order {
            Proposition::LessOrEqual(left, right) => (
                self.fixed_scalar_term(left)?,
                self.fixed_scalar_term(right)?,
            ),
            Proposition::IntegerMathLessOrEqual(left, right) => {
                (self.math_term(left)?, self.math_term(right)?)
            }
            _ => return Err(BoundedDenotationError::Unsupported("antisymmetry order")),
        };
        let ev = self.law(CoreLaw::LeAntisym, &[lt, rt, fwd, bwd])?;
        let Some((c, al, ar)) = self.identity_parts(concl) else {
            return Err(BoundedDenotationError::Unsupported(
                "antisymmetry conclusion is not an identity",
            ));
        };
        if self.arena.structurally_equal(al, lt) && self.arena.structurally_equal(ar, rt) {
            Ok(ev)
        } else if self.arena.structurally_equal(al, rt) && self.arena.structurally_equal(ar, lt) {
            Ok(self.symmetry(c, lt, rt, ev))
        } else {
            Err(BoundedDenotationError::Unsupported(
                "antisymmetry denotation changed endpoints",
            ))
        }
    }

    fn cast_bound_evidence(
        &mut self,
        root_bd: &Proposition,
        root_ev: TermHandle,
        chain: &CheckedIntegerCastChain,
        ax_ev: &[TermHandle],
        axioms: &[Proposition],
        concl: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let goal = self.denote(concl)?;
        let (strict, gl, gr) = match self.integer_relation(goal) {
            Some(IntegerRelation::LessThan { left, right }) => (true, left, right),
            Some(IntegerRelation::LessOrEqual { left, right }) => (false, left, right),
            _ => return Ok(None),
        };
        let (mut edges, mut vals, mut prev) = (
            Vec::with_capacity(chain.definition_axioms().len()),
            vec![chain.root().clone()],
            self.fixed_scalar_term(chain.root())?,
        );
        for (&idx, &ax) in chain.definition_axioms().iter().zip(ax_ev) {
            let Some((next, def)) = axioms.get(idx).and_then(Proposition::as_equal) else {
                return Ok(None);
            };
            let op = match def {
                ScalarTerm::IntegerWiden {
                    source_type: s,
                    target_type: t,
                    ..
                } => IntegerOperation::Unary(UnaryOp::Widen(*t), *s),
                ScalarTerm::IntegerExactCast {
                    source_type: s,
                    target_type: t,
                    ..
                } => IntegerOperation::Unary(UnaryOp::ExactCast(*t), *s),
                _ => return Ok(None),
            };
            let (app, nt) = (self.fixed_scalar_term(def)?, self.fixed_scalar_term(next)?);
            edges.push((op, prev, nt, app, ax));
            prev = nt;
            vals.push(next.clone());
        }
        let target = self.fixed_scalar_term(chain.target())?;
        let (mut bd, mut bty, ep, moving, start) = if *root_bd == Proposition::Truth {
            if strict || vals.len() != chain.carriers().len() {
                return Ok(None);
            }
            let (lower, upper) = (
                self.arena.structurally_equal(gr, target),
                self.arena.structurally_equal(gl, target),
            );
            if lower == upper {
                return Ok(None);
            }
            let mut sel: Option<(usize, i128)> = None;
            for (i, c) in chain.carriers().iter().enumerate() {
                let Some((min, max)) = fixed_native_interval(*c) else {
                    return Ok(None);
                };
                let cand = if lower { min } else { max };
                if sel.is_none_or(|(_, cur)| if lower { cand > cur } else { cand < cur }) {
                    sel = Some((i, cand));
                }
            }
            let Some((origin, _)) = sel else {
                return Ok(None);
            };
            let (eps, ev) = self.carrier_bound(&vals[origin], lower, &chain.carriers()[origin])?;
            let (ep, vt) = (
                self.fixed_scalar_term(&eps)?,
                self.fixed_scalar_term(&vals[origin])?,
            );
            let bt = if lower {
                self.order_term(false, ep, vt)?
            } else {
                self.order_term(false, vt, ep)?
            };
            (ev, bt, ep, usize::from(lower), origin)
        } else {
            let rty = self.denote(root_bd)?;
            let (l, r) = match self.integer_relation(rty) {
                Some(IntegerRelation::LessThan { left, right }) if strict => (left, right),
                Some(IntegerRelation::LessOrEqual { left, right }) if !strict => (left, right),
                _ => return Ok(None),
            };
            let root = self.fixed_scalar_term(chain.root())?;
            let (ep, mv) = if self.arena.structurally_equal(r, root) {
                (l, 1)
            } else if self.arena.structurally_equal(l, root) {
                (r, 0)
            } else {
                return Ok(None);
            };
            (root_ev, rty, ep, mv, 0)
        };
        let int = self.integer_constant()?;
        for &(op, prev, next, app, def) in &edges[start..] {
            let id_ev = self.law(CoreLaw::CastId(op), &[prev])?;
            let fwd = self.transitivity(int, next, app, prev, def, id_ev);
            let eq_ty = self.id(int, next, prev);
            let sg = if moving == 1 {
                self.order_term(strict, ep, next)?
            } else {
                self.order_term(strict, next, ep)?
            };
            let Some(nb) = self.order_substitution_evidence(bty, bd, eq_ty, fwd, moving, sg)?
            else {
                return Ok(None);
            };
            bd = nb;
            bty = sg;
        }
        Ok(self.arena.structurally_equal(bty, goal).then_some(bd))
    }
}

// ============================================================================
// Correlated forbidden-root chains
// ============================================================================

#[derive(Clone)]
struct ChainStep {
    definition: TermHandle,
    literal: Option<TermHandle>,
    op: BinaryOp,
    next: TermHandle,
    sibling: TermHandle,
    literal_term: TermHandle,
    literal_value: i128,
    expression: TermHandle,
    current: TermHandle,
}
enum SolveOutcome {
    Refuted(TermHandle),
    Root {
        equality: TermHandle,
        quotient: i128,
        quotient_term: TermHandle,
    },
}

impl Denotation {
    #[allow(clippy::too_many_arguments)]
    fn chain_congruence(
        &mut self,
        int: TermHandle,
        op: BinaryOp,
        fixed: TermHandle,
        fixed_left: bool,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
        src: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let v = self.var(1);
        let rw = if fixed_left {
            self.binary_terms(op, fixed, v)?
        } else {
            self.binary_terms(op, v, fixed)?
        };
        let (body, base) = (self.id(int, src, rw), self.refl(int, src));
        Ok(self.id_elim(int, from, to, proof, base, body))
    }

    fn chain_steps(
        &mut self,
        branch: &CorrelatedAffineBranchWitness,
        ity: IntegerType,
        evs: &[(Proposition, TermHandle)],
        cur: &mut usize,
    ) -> Result<Option<Vec<ChainStep>>, BoundedDenotationError> {
        let mut steps = Vec::with_capacity(branch.steps.len());
        for step in &branch.steps {
            let lit_ev = if step.literal_axiom.is_some() {
                let Some(p) = evs.get(*cur) else {
                    return Ok(None);
                };
                *cur += 1;
                Some(p.clone())
            } else {
                None
            };
            let Some((dp, dev)) = evs.get(*cur) else {
                return Ok(None);
            };
            *cur += 1;
            let Some((current, expr)) = dp.as_equal() else {
                return Ok(None);
            };
            let (op, next, sib) = match expr {
                ScalarTerm::ExactIntegerAdd {
                    scalar_type: t,
                    left: l,
                    right: r,
                } if *t == ity => (BinaryOp::Add, l.as_ref(), r.as_ref()),
                ScalarTerm::ExactIntegerSubtract {
                    scalar_type: t,
                    left: l,
                    right: r,
                } if *t == ity => (BinaryOp::Sub, l.as_ref(), r.as_ref()),
                ScalarTerm::ExactIntegerMultiply {
                    scalar_type: t,
                    left: l,
                    right: r,
                } if *t == ity => (BinaryOp::Mul, l.as_ref(), r.as_ref()),
                _ => return Ok(None),
            };
            let (ct, nt, st) = (
                self.fixed_scalar_term(current)?,
                self.fixed_scalar_term(next)?,
                self.fixed_scalar_term(sib)?,
            );
            let et = self.binary_terms(op, nt, st)?;
            let Some(definition) = self.directed_prop_eq(dp, *dev, ct, et)? else {
                return Ok(None);
            };
            let (lv, lt, lproof) = match lit_ev {
                None => {
                    let Some(v) = signed_literal(sib, ity) else {
                        return Ok(None);
                    };
                    (v, st, None)
                }
                Some((prop, ev)) => {
                    let Proposition::Equal(l, r) = &prop else {
                        return Ok(None);
                    };
                    if l != sib {
                        return Ok(None);
                    }
                    let Some(v) = signed_literal(r, ity) else {
                        return Ok(None);
                    };
                    let lt = self.integer_numeral(v)?;
                    let Some(ev) = self.directed_prop_eq(&prop, ev, st, lt)? else {
                        return Ok(None);
                    };
                    (v, lt, Some(ev))
                }
            };
            steps.push(ChainStep {
                definition,
                literal: lproof,
                op,
                next: nt,
                sibling: st,
                literal_term: lt,
                literal_value: lv,
                expression: et,
                current: ct,
            });
        }
        Ok(Some(steps))
    }

    fn lift_step(&mut self, s: &ChainStep, amt: u32) -> ChainStep {
        let mut sh = |h| shift(&mut self.arena, h, 0, amt);
        ChainStep {
            definition: sh(s.definition),
            literal: s.literal.map(&mut sh),
            op: s.op,
            next: sh(s.next),
            sibling: sh(s.sibling),
            literal_term: sh(s.literal_term),
            literal_value: s.literal_value,
            expression: sh(s.expression),
            current: sh(s.current),
        }
    }

    fn solve_divisor(
        &mut self,
        ity: IntegerType,
        steps: &[ChainStep],
        target: TermHandle,
        hyp: TermHandle,
        mut val: i128,
        root: TermHandle,
    ) -> Result<Option<SolveOutcome>, BoundedDenotationError> {
        let (int, mut cur, mut eq, mut vt) = (
            self.integer_constant()?,
            target,
            hyp,
            self.integer_numeral(val)?,
        );
        for s in steps {
            let fl = self.symmetry(int, cur, s.expression, s.definition);
            let mut bd = self.transitivity(int, s.expression, cur, vt, fl, eq);
            let mut expr = s.expression;
            if let Some(lit) = s.literal {
                let rw = self.binary_terms(s.op, s.next, s.literal_term)?;
                let cg = self.chain_congruence(
                    int,
                    s.op,
                    s.next,
                    true,
                    s.sibling,
                    s.literal_term,
                    lit,
                    s.expression,
                )?;
                let cg = self.symmetry(int, s.expression, rw, cg);
                bd = self.transitivity(int, rw, s.expression, vt, cg, bd);
                expr = rw;
            }
            match s.op {
                BinaryOp::Add | BinaryOp::Sub => {
                    let (iop, ilaw, nv) = if s.op == BinaryOp::Add {
                        let Some(v) = val.checked_sub(s.literal_value) else {
                            return Ok(None);
                        };
                        (BinaryOp::Sub, CoreLaw::AddRightInv, v)
                    } else {
                        let Some(v) = val.checked_add(s.literal_value) else {
                            return Ok(None);
                        };
                        (BinaryOp::Add, CoreLaw::SubRightInv, v)
                    };
                    let (nt, res) = (
                        self.integer_numeral(nv)?,
                        self.binary_terms(iop, vt, s.literal_term)?,
                    );
                    let inv = self.law(ilaw, &[s.next, s.literal_term, vt, bd])?;
                    let Some(es) = self.numeral_binary_equation(
                        iop,
                        IntegerValue::Signed(val),
                        IntegerValue::Signed(s.literal_value),
                        IntegerValue::Signed(nv),
                        vt,
                        s.literal_term,
                        nt,
                    )?
                    else {
                        return Ok(None);
                    };
                    eq = self.transitivity(int, s.next, res, nt, inv, es);
                    val = nv;
                    vt = nt;
                }
                BinaryOp::Mul => {
                    if s.literal_value == 0 {
                        return Ok(None);
                    }
                    let Some(q) = val.checked_div(s.literal_value) else {
                        return Ok(None);
                    };
                    let (qt, le_pv) = (self.integer_numeral(q)?, self.le_of_eq(expr, vt, bd)?);
                    let fl = self.symmetry(int, expr, vt, bd);
                    let le_vp = self.le_of_eq(vt, expr, fl)?;
                    let pos = s.literal_value > 0;
                    let Some(sign) = (if pos {
                        self.integer_le_numeral(
                            IntegerValue::Signed(1),
                            IntegerValue::Signed(s.literal_value),
                        )?
                    } else {
                        self.integer_le_numeral(
                            IntegerValue::Signed(s.literal_value),
                            IntegerValue::Signed(-1),
                        )?
                    }) else {
                        return Ok(None);
                    };
                    let qa = self.apply_integer_operation(
                        IntegerOperation::exact_divide(ity),
                        vt,
                        Some(s.literal_term),
                    )?;
                    let (ub, lb) = if pos { (le_pv, le_vp) } else { (le_vp, le_pv) };
                    let upper = self.law(
                        CoreLaw::QuotBound {
                            ty: ity,
                            upper: true,
                            positive: pos,
                        },
                        &[s.literal_term, vt, s.next, sign, ub],
                    )?;
                    let lower = self.law(
                        CoreLaw::QuotBound {
                            ty: ity,
                            upper: false,
                            positive: pos,
                        },
                        &[s.literal_term, vt, s.next, sign, lb],
                    )?;
                    let to_q = self.law(CoreLaw::LeAntisym, &[s.next, qa, upper, lower])?;
                    let Some(num) = self.numeral_quotient(
                        ity,
                        IntegerValue::Signed(val),
                        IntegerValue::Signed(s.literal_value),
                        IntegerValue::Signed(q),
                        vt,
                        s.literal_term,
                        qt,
                    )?
                    else {
                        return Ok(None);
                    };
                    eq = self.transitivity(int, s.next, qa, qt, to_q, num);
                    if q.checked_mul(s.literal_value) != Some(val) {
                        let Some(pv) = q.checked_mul(s.literal_value) else {
                            return Ok(None);
                        };
                        let bv = self.var(1);
                        let pb = self.binary_terms(BinaryOp::Mul, bv, s.literal_term)?;
                        let body = self.id(int, pb, vt);
                        let rw = self.id_elim(int, s.next, qt, eq, bd, body);
                        let pt = self.integer_numeral(pv)?;
                        let Some(peq) = self.numeral_binary_equation(
                            BinaryOp::Mul,
                            IntegerValue::Signed(q),
                            IntegerValue::Signed(s.literal_value),
                            IntegerValue::Signed(pv),
                            qt,
                            s.literal_term,
                            pt,
                        )?
                        else {
                            return Ok(None);
                        };
                        let pa = self.binary_terms(BinaryOp::Mul, qt, s.literal_term)?;
                        let fl = self.symmetry(int, pa, vt, rw);
                        let to_p = self.transitivity(int, vt, pa, pt, fl, peq);
                        let fl2 = self.symmetry(int, vt, pt, to_p);
                        let (lo, hi, ls) = if pv < val {
                            (pv, val, true)
                        } else {
                            (val, pv, false)
                        };
                        let st =
                            self.literal_order(IntegerValue::Signed(lo), IntegerValue::Signed(hi))?;
                        let diag = self.ord_subst(true, ls, pt, vt, vt, fl2, st)?;
                        return Ok(Some(SolveOutcome::Refuted(self.irreflexive(vt, diag)?)));
                    }
                    val = q;
                    vt = qt;
                }
            }
            cur = s.next;
        }
        Ok(self
            .arena
            .structurally_equal(cur, root)
            .then_some(SolveOutcome::Root {
                equality: eq,
                quotient: val,
                quotient_term: vt,
            }))
    }

    fn eval_dividend(
        &mut self,
        steps: &[ChainStep],
        mut eq: TermHandle,
        root: TermHandle,
        mut val: i128,
        divd: TermHandle,
    ) -> Result<Option<(i128, TermHandle)>, BoundedDenotationError> {
        let (int, mut inner, mut vt) = (self.integer_constant()?, root, self.integer_numeral(val)?);
        for s in steps {
            let src = self.binary_terms(s.op, inner, s.sibling)?;
            let cg = self.chain_congruence(int, s.op, s.sibling, false, inner, vt, eq, src)?;
            let esib = self.binary_terms(s.op, vt, s.sibling)?;
            let mut bd = self.transitivity(int, s.current, s.expression, esib, s.definition, cg);
            let mut expr = esib;
            if let Some(lit) = s.literal {
                let cg = self.chain_congruence(
                    int,
                    s.op,
                    vt,
                    true,
                    s.sibling,
                    s.literal_term,
                    lit,
                    esib,
                )?;
                let elit = self.binary_terms(s.op, vt, s.literal_term)?;
                bd = self.transitivity(int, s.current, esib, elit, bd, cg);
                expr = elit;
            }
            let Some(ev) = s.op.eval_i128(val, s.literal_value) else {
                return Ok(None);
            };
            let evt = self.integer_numeral(ev)?;
            let Some(es) = self.numeral_binary_equation(
                s.op,
                IntegerValue::Signed(val),
                IntegerValue::Signed(s.literal_value),
                IntegerValue::Signed(ev),
                vt,
                s.literal_term,
                evt,
            )?
            else {
                return Ok(None);
            };
            eq = self.transitivity(int, s.current, expr, evt, bd, es);
            inner = s.current;
            val = ev;
            vt = evt;
        }
        Ok(self
            .arena
            .structurally_equal(inner, divd)
            .then_some((val, eq)))
    }

    #[allow(clippy::too_many_arguments)]
    fn endpoint_contradiction(
        &mut self,
        below: bool,
        subj: TermHandle,
        v: i128,
        vt: TermHandle,
        bv: i128,
        bt: TermHandle,
        bev: TermHandle,
        veq: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let tr = self.ord_subst(false, !below, subj, vt, bt, veq, bev)?;
        let (lo, hi) = if below { (v, bv) } else { (bv, v) };
        let st = self.literal_order(IntegerValue::Signed(lo), IntegerValue::Signed(hi))?;
        let c = if below {
            self.ord_trans(false, true, bt, vt, bt, tr, st)?
        } else {
            self.ord_trans(true, false, bt, vt, bt, st, tr)?
        };
        self.irreflexive(bt, c)
    }

    fn disjunct_3(&mut self, idx: usize, payload: TermHandle) -> TermHandle {
        match idx {
            0 => self.pair(self.two_zero, payload),
            1 => {
                let i = self.pair(self.two_zero, payload);
                self.pair(self.two_one, i)
            }
            _ => {
                let i = self.pair(self.two_one, payload);
                self.pair(self.two_one, i)
            }
        }
    }

    fn correlated_forbidden_roots_evidence(
        &mut self,
        w: &IntegerCorrelatedForbiddenRootWitness,
        chk: &CheckedIntegerCorrelatedForbiddenRoots,
        ax_ev: &[(Proposition, TermHandle)],
        bd_ev: &[(Proposition, TermHandle)],
        concl: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ity = chk.integer_type();
        if ity.carrier() != IntegerCarrier::Fixed || ity.sign() != IntegerSign::Signed {
            return Ok(None);
        }
        let (Some(IntegerValue::Signed(min)), Some(IntegerValue::Signed(max))) =
            (ity.minimum_value(), ity.maximum_value())
        else {
            return Ok(None);
        };
        let Some(min_p1) = min.checked_add(1) else {
            return Ok(None);
        };
        let (int, root_t, divd_t, divr_t, (lo, hi)) = (
            self.integer_constant()?,
            self.fixed_scalar_term(chk.root())?,
            self.fixed_scalar_term(chk.dividend())?,
            self.fixed_scalar_term(chk.divisor())?,
            chk.interval(),
        );
        let mut cur = 0;
        let (Some(d_steps), Some(r_steps)) = (
            self.chain_steps(&w.dividend, ity, ax_ev, &mut cur)?,
            self.chain_steps(&w.divisor, ity, ax_ev, &mut cur)?,
        ) else {
            return Ok(None);
        };
        let ok = |s: &Self, xs: &[ChainStep], t: TermHandle| {
            !xs.is_empty()
                && s.arena.structurally_equal(xs[0].next, root_t)
                && xs
                    .windows(2)
                    .all(|p| s.arena.structurally_equal(p[0].current, p[1].next))
                && s.arena
                    .structurally_equal(xs.last().expect("nonempty").current, t)
        };
        if cur != ax_ev.len() || !ok(self, &d_steps, divd_t) || !ok(self, &r_steps, divr_t) {
            return Ok(None);
        }
        let [lo_b, hi_b] = bd_ev else {
            return Ok(None);
        };
        let mut bside =
            |p: &Proposition,
             v: i128,
             rr: bool|
             -> Result<Option<TermHandle>, BoundedDenotationError> {
                let d = self.denote(p)?;
                let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(d)
                else {
                    return Ok(None);
                };
                let bt = self.integer_numeral(v)?;
                let (ep, op) = if rr { (left, right) } else { (right, left) };
                Ok((self.arena.structurally_equal(ep, bt)
                    && self.arena.structurally_equal(op, root_t))
                .then_some(bt))
            };
        let (Some(lo_t), Some(hi_t)) = (bside(&lo_b.0, lo, true)?, bside(&hi_b.0, hi, false)?)
        else {
            return Ok(None);
        };
        let (z_t, one_t, m1_t, m2_t, mp1_t) = (
            self.integer_numeral(0)?,
            self.integer_numeral(1)?,
            self.integer_numeral(-1)?,
            self.integer_numeral(-2)?,
            self.integer_numeral(min_p1)?,
        );
        let (d0, d1, cb, dlm1) = (
            self.order_term(false, divr_t, m2_t)?,
            self.order_term(false, one_t, divr_t)?,
            self.order_term(false, mp1_t, divd_t)?,
            self.order_term(false, divr_t, m1_t)?,
        );
        let conj = self.sigma(dlm1, cb);
        let rest = self.tagged_sum(d1, conj);
        let expected = self.tagged_sum(d0, rest);
        let goal = self.denote(concl)?;
        if !self.arena.structurally_equal(expected, goal) {
            return Ok(None);
        }
        let (cm1, c0) = (self.id(int, divr_t, m1_t), self.id(int, divr_t, z_t));
        let d_lifted: Vec<_> = d_steps.iter().map(|s| self.lift_step(s, 1)).collect();
        let r_solve: Vec<_> = r_steps.iter().rev().map(|s| self.lift_step(s, 1)).collect();
        let (lo_ev, hi_ev, hyp) = (
            shift(&mut self.arena, lo_b.1, 0, 1),
            shift(&mut self.arena, hi_b.1, 0, 1),
            self.var(0),
        );

        let zbody = match self.solve_divisor(ity, &r_solve, divr_t, hyp, 0, root_t)? {
            None => return Ok(None),
            Some(SolveOutcome::Refuted(e)) => self.empty_elim(goal, e),
            Some(SolveOutcome::Root {
                equality,
                quotient: q,
                quotient_term: qt,
            }) => {
                if q >= lo && q <= hi {
                    return Ok(None);
                }
                let below = q < lo;
                let (bv, bt, be) = if below {
                    (lo, lo_t, lo_ev)
                } else {
                    (hi, hi_t, hi_ev)
                };
                let e = self.endpoint_contradiction(below, root_t, q, qt, bv, bt, be, equality)?;
                self.empty_elim(goal, e)
            }
        };
        let zbr = self.lam(c0, zbody);
        let Some(m1_out) = self.solve_divisor(ity, &r_solve, divr_t, hyp, -1, root_t)? else {
            return Ok(None);
        };
        let bproof = match m1_out {
            SolveOutcome::Refuted(e) => self.empty_elim(cb, e),
            SolveOutcome::Root {
                equality,
                quotient: q,
                quotient_term: qt,
            } => {
                if q < lo || q > hi {
                    let below = q < lo;
                    let (bv, bt, be) = if below {
                        (lo, lo_t, lo_ev)
                    } else {
                        (hi, hi_t, hi_ev)
                    };
                    let e =
                        self.endpoint_contradiction(below, root_t, q, qt, bv, bt, be, equality)?;
                    self.empty_elim(cb, e)
                } else {
                    let Some((ev, eeq)) =
                        self.eval_dividend(&d_lifted, equality, root_t, q, divd_t)?
                    else {
                        return Ok(None);
                    };
                    if ev == min {
                        return Ok(None);
                    }
                    let evt = self.integer_numeral(ev)?;
                    if ev > min && ev <= max {
                        let Some(ord) = self.integer_le_numeral(
                            IntegerValue::Signed(min_p1),
                            IntegerValue::Signed(ev),
                        )?
                        else {
                            return Ok(None);
                        };
                        let fl = self.symmetry(int, divd_t, evt, eeq);
                        self.ord_subst(false, false, evt, divd_t, mp1_t, fl, ord)?
                    } else {
                        let below = ev < min;
                        let (ep, mem) = self.carrier_bound(chk.dividend(), below, &ity)?;
                        let ept = self.fixed_scalar_term(&ep)?;
                        let e = self.endpoint_contradiction(
                            below,
                            divd_t,
                            ev,
                            evt,
                            if below { min } else { max },
                            ept,
                            mem,
                            eeq,
                        )?;
                        self.empty_elim(cb, e)
                    }
                }
            }
        };
        let dlm1_ev = self.le_of_eq(divr_t, m1_t, hyp)?;
        let cpay = self.pair(dlm1_ev, bproof);
        let (m1b, nb, pb) = (
            self.disjunct_3(2, cpay),
            self.disjunct_3(0, hyp),
            self.disjunct_3(1, hyp),
        );
        let (m1br, nbr, pbr) = (self.lam(cm1, m1b), self.lam(d0, nb), self.lam(d1, pb));
        let split = self.law(CoreLaw::DivisorCaseSplit, &[divr_t])?;
        let inner = self.tagged_sum(c0, d1);
        let mid = self.tagged_sum(cm1, inner);
        let s0 = self.var(0);
        let (zb2, pb2) = (
            shift(&mut self.arena, zbr, 0, 2),
            shift(&mut self.arena, pbr, 0, 2),
        );
        let l3 = self.tagged_sum_elim(c0, d1, s0, zb2, pb2, goal);
        let m1_one = self.lam(inner, l3);
        let m1l = shift(&mut self.arena, m1br, 0, 1);
        let l2 = self.tagged_sum_elim(cm1, inner, s0, m1l, m1_one, goal);
        let top_one = self.lam(mid, l2);
        Ok(Some(
            self.tagged_sum_elim(d0, mid, split, nbr, top_one, goal),
        ))
    }
}

// ============================================================================
// SourceContentTheory & SourceIntegerTheory (public source-predicate theories)
// ============================================================================

/// One arena's append-only content vocabulary. Stored handles belong to that
/// arena/signature pair.
pub struct SourceContentTheory {
    state: Denotation,
}

impl SourceContentTheory {
    pub fn new(arena: &mut TermArena, declarations: &mut Vec<Declaration>) -> Self {
        let mut state = Denotation::new_in(std::mem::take(arena), std::mem::take(declarations));
        std::mem::swap(arena, &mut state.arena);
        std::mem::swap(declarations, &mut state.declarations);
        Self { state }
    }

    pub fn carrier(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        kind: ContentAlgebraKind,
        parameter: TermHandle,
        level: Level,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.state.with_external(arena, declarations, |s| {
            s.content_carrier_at(kind, parameter, level)
        })
    }
}

/// Declaration positions of the integer carrier and its relations and operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceIntegerVocabulary {
    pub carrier: u32,
    pub less_than: u32,
    pub less_or_equal: u32,
    pub add: u32,
    pub subtract: u32,
    pub multiply: u32,
}

/// Ten consecutive declaration positions declared by `SourceIntegerTheory::new`
/// and accepted by `SourceIntegerTheory::resume`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceIntegerPositions {
    pub vocabulary: SourceIntegerVocabulary,
    pub numeral_zero: u32,
    pub numeral_double: u32,
    pub numeral_odd: u32,
    pub numeral_negate: u32,
}

impl SourceIntegerPositions {
    fn in_declaration_order(&self) -> [u32; 10] {
        let v = self.vocabulary;
        [
            v.carrier,
            v.less_than,
            v.less_or_equal,
            v.add,
            v.subtract,
            v.multiply,
            self.numeral_zero,
            self.numeral_double,
            self.numeral_odd,
            self.numeral_negate,
        ]
    }

    fn check(
        &self,
        arena: &mut TermArena,
        declarations: &[Declaration],
    ) -> Result<(), BoundedDenotationError> {
        let refused = || {
            BoundedDenotationError::Unsupported(
                "source integer positions do not name the declarations SourceIntegerTheory::new appended",
            )
        };
        let order = self.in_declaration_order();
        let carrier = order[0];
        if (0..)
            .zip(order)
            .any(|(off, pos)| carrier.checked_add(off) != Some(pos))
        {
            return Err(refused());
        }
        let int = arena.insert(Term::Constant {
            declaration: carrier,
            levels: Vec::new(),
        });
        let tz = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let to_t = arena.insert(Term::Pi {
            domain: int,
            codomain: tz,
        });
        let rel = arena.insert(Term::Pi {
            domain: int,
            codomain: to_t,
        });
        let ctor = arena.insert(Term::Pi {
            domain: int,
            codomain: int,
        });
        let op = arena.insert(Term::Pi {
            domain: int,
            codomain: ctor,
        });
        let expected = [tz, rel, rel, op, op, op, int, ctor, ctor, ctor];
        for (pos, ty) in order.into_iter().zip(expected) {
            let Some(d) = declarations.get(pos as usize) else {
                return Err(refused());
            };
            if d.body.is_some() || d.level_arity != 0 || !arena.structurally_equal(d.ty, ty) {
                return Err(refused());
            }
        }
        Ok(())
    }
}

/// One integer vocabulary in one caller's arena and declarations, and proof
/// searches against it.
pub struct SourceIntegerTheory {
    state: Denotation,
    positions: SourceIntegerPositions,
}

impl SourceIntegerTheory {
    pub fn new(
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
    ) -> Result<Self, BoundedDenotationError> {
        let mut state = Denotation::new_in(std::mem::take(arena), std::mem::take(declarations));
        let positions = state.declare_source_integer();
        std::mem::swap(arena, &mut state.arena);
        std::mem::swap(declarations, &mut state.declarations);
        Ok(Self {
            state,
            positions: positions?,
        })
    }

    pub fn resume(
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        positions: SourceIntegerPositions,
    ) -> Result<Self, BoundedDenotationError> {
        positions.check(arena, declarations)?;
        let mut state = Denotation::new_in(std::mem::take(arena), std::mem::take(declarations));
        let adopted = state.adopt_source_integer(positions);
        std::mem::swap(arena, &mut state.arena);
        std::mem::swap(declarations, &mut state.declarations);
        adopted?;
        Ok(Self { state, positions })
    }

    pub fn positions(&self) -> SourceIntegerPositions {
        self.positions
    }
    pub fn vocabulary(&self) -> SourceIntegerVocabulary {
        self.positions.vocabulary
    }

    pub fn source_safety_roster(&self) -> Vec<u32> {
        let t = &self.state.tables;
        let mut pos: Vec<u32> = t
            .integer
            .into_iter()
            .chain(t.integer_less_than)
            .chain(t.integer_less_or_equal)
            .chain(t.binary_ops.into_iter().flatten())
            .chain(t.numeral_zero)
            .chain(t.numeral_ctors.into_iter().flatten())
            .chain(t.laws.values().copied())
            .chain(t.numeral_equations.values().copied())
            .chain(t.numeral_quotients.values().copied())
            .collect();
        pos.sort_unstable();
        pos.dedup();
        pos
    }

    pub fn literal(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        value: BigInt,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.state
            .with_external(arena, declarations, |s| s.math_literal(value))
    }

    pub fn closed_predicate(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        goal: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        self.state
            .with_external(arena, declarations, |s| s.closed_source_predicate(goal, 0))
    }

    pub fn predicate_from_assumptions(
        &mut self,
        arena: &mut TermArena,
        declarations: &mut Vec<Declaration>,
        goal: TermHandle,
        assumptions: &[TermHandle],
        level_arity: u32,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        self.state.with_external(arena, declarations, |s| {
            s.source_assumption_predicate(goal, assumptions, level_arity)
        })
    }
}

impl Denotation {
    fn with_external<T>(
        &mut self,
        arena: &mut TermArena,
        decls: &mut Vec<Declaration>,
        f: impl FnOnce(&mut Self) -> Result<T, BoundedDenotationError>,
    ) -> Result<T, BoundedDenotationError> {
        std::mem::swap(arena, &mut self.arena);
        std::mem::swap(decls, &mut self.declarations);
        let res = f(self);
        std::mem::swap(arena, &mut self.arena);
        std::mem::swap(decls, &mut self.declarations);
        res
    }

    fn declare_source_integer(&mut self) -> Result<SourceIntegerPositions, BoundedDenotationError> {
        let vocabulary = SourceIntegerVocabulary {
            carrier: self.integer()?,
            less_than: self.integer_less_than()?,
            less_or_equal: self.integer_less_or_equal()?,
            add: self.binary_op_position(BinaryOp::Add)?,
            subtract: self.binary_op_position(BinaryOp::Sub)?,
            multiply: self.binary_op_position(BinaryOp::Mul)?,
        };
        let n = self.declare_numerals()?;
        self.math_literal(BigInt::zero())?;
        Ok(SourceIntegerPositions {
            vocabulary,
            numeral_zero: n.zero,
            numeral_double: n.double,
            numeral_odd: n.odd,
            numeral_negate: n.negate,
        })
    }

    fn adopt_source_integer(
        &mut self,
        p: SourceIntegerPositions,
    ) -> Result<(), BoundedDenotationError> {
        let v = p.vocabulary;
        self.tables.integer = Some(v.carrier);
        self.tables.integer_less_than = Some(v.less_than);
        self.tables.integer_less_or_equal = Some(v.less_or_equal);
        self.tables.binary_ops = [Some(v.add), Some(v.subtract), Some(v.multiply)];
        self.adopt_numerals(NumeralPositions {
            zero: p.numeral_zero,
            double: p.numeral_double,
            odd: p.numeral_odd,
            negate: p.numeral_negate,
        })
    }

    fn source_head(&mut self, term: TermHandle) -> Result<TermHandle, BoundedDenotationError> {
        let sig = Signature::from_declarations(self.declarations.clone());
        let values = &self.tables.closed_math_values;
        weak_head_normalize_retaining(
            &mut self.arena,
            &sig,
            term,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            &|pos| values.get(pos as usize).and_then(Option::as_ref).is_some(),
        )
        .map_err(BoundedDenotationError::Kernel)
    }

    fn source_binary(
        &mut self,
        term: TermHandle,
    ) -> Result<Option<(u32, TermHandle, TermHandle)>, BoundedDenotationError> {
        let term = self.source_head(term)?;
        let Term::Apply {
            function,
            argument: r,
        } = self.arena.get(term)
        else {
            return Ok(None);
        };
        let f = self.source_head(function)?;
        let Term::Apply {
            function,
            argument: l,
        } = self.arena.get(f)
        else {
            return Ok(None);
        };
        let Term::Constant { declaration, .. } = self.arena.get(function) else {
            return Ok(None);
        };
        Ok(Some((declaration, l, r)))
    }

    fn source_binary_op(&mut self, decl: u32) -> Result<Option<BinaryOp>, BoundedDenotationError> {
        for op in [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul] {
            if decl == self.binary_op_position(op)? {
                return Ok(Some(op));
            }
        }
        Ok(None)
    }

    fn source_congruence(
        &mut self,
        func: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (int, start, y) = (self.integer_constant()?, self.app(func, from), self.var(1));
        let end = self.app(func, y);
        let (body, base) = (self.id(int, start, end), self.refl(int, start));
        Ok(self.id_elim(int, from, to, proof, base, body))
    }

    #[allow(clippy::too_many_arguments)]
    fn source_binary_congruence(
        &mut self,
        int: TermHandle,
        head: TermHandle,
        term: TermHandle,
        l: TermHandle,
        r: TermHandle,
        nl: TermHandle,
        lp: TermHandle,
        nr: TermHandle,
        rp: TermHandle,
    ) -> Result<(TermHandle, TermHandle), BoundedDenotationError> {
        let v0 = self.var(0);
        let fl_app = self.app(head, v0);
        let fl_body = self.app(fl_app, r);
        let fl = self.lam(int, fl_body);
        let first = self.source_congruence(fl, l, nl, lp)?;
        let mid_fn = self.app(head, nl);
        let middle = self.app(mid_fn, r);
        let fr_body = self.app(mid_fn, v0);
        let fr = self.lam(int, fr_body);
        let second = self.source_congruence(fr, r, nr, rp)?;
        let norm = self.app(mid_fn, nr);
        Ok((
            norm,
            self.transitivity(int, term, middle, norm, first, second),
        ))
    }

    fn source_integer_value(
        &mut self,
        orig: TermHandle,
        depth: usize,
    ) -> Result<Option<(BigInt, TermHandle, TermHandle)>, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let (term, int) = (self.source_head(orig)?, self.integer_constant()?);
        if let Some(v) = self.closed_math_value(term) {
            return Ok(Some((v, term, self.refl(int, term))));
        }
        let Some((decl, l, r)) = self.source_binary(term)? else {
            return Ok(None);
        };
        let Some(op) = self.source_binary_op(decl)? else {
            return Ok(None);
        };
        let (Some((lv, lt, lp)), Some((rv, rt, rp))) = (
            self.source_integer_value(l, depth + 1)?,
            self.source_integer_value(r, depth + 1)?,
        ) else {
            return Ok(None);
        };
        let val = op.eval_big(&lv, &rv);
        let (Some(li), Some(ri), Some(vi)) = (
            integer_value_of(&lv),
            integer_value_of(&rv),
            integer_value_of(&val),
        ) else {
            return Ok(None);
        };
        let res = self.math_literal(val.clone())?;
        let Some(eq) = self.numeral_binary_equation(op, li, ri, vi, lt, rt, res)? else {
            return Ok(None);
        };
        let head = self.constant(decl);
        let (norm, comb) = self.source_binary_congruence(int, head, term, l, r, lt, lp, rt, rp)?;
        Ok(Some((
            val,
            res,
            self.transitivity(int, term, norm, res, comb, eq),
        )))
    }

    fn closed_source_predicate(
        &mut self,
        orig: TermHandle,
        depth: usize,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let goal = self.source_head(orig)?;
        if let Some((ty, l, r)) = self.identity_parts(goal) {
            if self.arena.structurally_equal(l, r) {
                return Ok(Some(self.refl(ty, l)));
            }
            let (Some((lv, lc, lp)), Some((rv, rc, rp))) = (
                self.source_integer_value(l, depth + 1)?,
                self.source_integer_value(r, depth + 1)?,
            ) else {
                return Ok(None);
            };
            if lv != rv {
                return Ok(None);
            }
            let int = self.integer_constant()?;
            let rev = self.symmetry(int, r, rc, rp);
            return Ok(Some(self.transitivity(int, l, lc, r, lp, rev)));
        }
        if let Some(rel) = self.integer_relation(goal) {
            let (strict, l, r) = match rel {
                IntegerRelation::LessThan { left, right } => (true, left, right),
                IntegerRelation::LessOrEqual { left, right } => (false, left, right),
                IntegerRelation::Equality { .. } => return Ok(None),
            };
            let (Some((_, lc, lp)), Some((_, rc, rp))) = (
                self.source_integer_value(l, depth + 1)?,
                self.source_integer_value(r, depth + 1)?,
            ) else {
                return Ok(None);
            };
            let norm = self.order_term(strict, lc, rc)?;
            let Some(ev) = self.closed_inhabitant(norm)? else {
                return Ok(None);
            };
            let int = self.integer_constant()?;
            let (lr, rr) = (self.symmetry(int, l, lc, lp), self.symmetry(int, r, rc, rp));
            let first = self.ord_subst(strict, true, lc, l, rc, lr, ev)?;
            return self.ord_subst(strict, false, rc, r, l, rr, first).map(Some);
        }
        match self.arena.get(goal) {
            Term::Sigma { domain, codomain } => {
                let Some(f) = self.closed_source_predicate(domain, depth + 1)? else {
                    return Ok(None);
                };
                let rest = substitute(&mut self.arena, codomain, f);
                let Some(s) = self.closed_source_predicate(rest, depth + 1)? else {
                    return Ok(None);
                };
                Ok(Some(self.pair(f, s)))
            }
            Term::Box { ty } => {
                let Term::Pi { domain, codomain } = self.arena.get(ty) else {
                    return Ok(None);
                };
                if !matches!(self.arena.get(domain), Term::Empty)
                    || !matches!(self.arena.get(codomain), Term::Empty)
                {
                    return Ok(None);
                }
                let body = self.var(0);
                let value = self.lam(domain, body);
                Ok(Some(self.arena.insert(Term::BoxIntro { ty, value })))
            }
            _ => Ok(None),
        }
    }

    fn source_normal_proposition(
        &mut self,
        term: TermHandle,
        depth: usize,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let head = self.source_head(term)?;
        let node = match self.arena.get(head) {
            Term::Constant {
                declaration,
                levels,
            } if levels.is_empty() => return Ok(self.constant(declaration)),
            Term::Apply { function, argument } => Term::Apply {
                function: self.source_normal_proposition(function, depth + 1)?,
                argument: self.source_normal_proposition(argument, depth + 1)?,
            },
            Term::Id { ty, left, right } => Term::Id {
                ty: self.source_normal_proposition(ty, depth + 1)?,
                left: self.source_normal_proposition(left, depth + 1)?,
                right: self.source_normal_proposition(right, depth + 1)?,
            },
            Term::Sigma { domain, codomain } => Term::Sigma {
                domain: self.source_normal_proposition(domain, depth + 1)?,
                codomain: self.source_normal_proposition(codomain, depth + 1)?,
            },
            _ => return Ok(head),
        };
        Ok(self.arena.insert(node))
    }

    fn source_normal_integer(
        &mut self,
        orig: TermHandle,
        depth: usize,
    ) -> Result<(TermHandle, TermHandle), BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        if let Some((_, t, eq)) = self.source_integer_value(orig, depth + 1)? {
            return Ok((t, eq));
        }
        let (term, int) = (self.source_head(orig)?, self.integer_constant()?);
        let Some((decl, l, r)) = self.source_binary(term)? else {
            return Ok((term, self.refl(int, term)));
        };
        let Some(op) = self.source_binary_op(decl)? else {
            return Ok((term, self.refl(int, term)));
        };
        let ((lt, leq), (rt, req)) = (
            self.source_normal_integer(l, depth + 1)?,
            self.source_normal_integer(r, depth + 1)?,
        );
        let head = self.constant(decl);
        let (norm, eq) = self.source_binary_congruence(int, head, term, l, r, lt, leq, rt, req)?;
        if op == BinaryOp::Mul
            && let Some((ad, a, b)) = self.source_binary(rt)?
            && ad == self.binary_op_position(BinaryOp::Add)?
        {
            let exp_eq = self.law(CoreLaw::MulDistributeAdd, &[lt, a, b])?;
            let (p1, p2) = (
                self.binary_terms(BinaryOp::Mul, lt, a)?,
                self.binary_terms(BinaryOp::Mul, lt, b)?,
            );
            let exp = self.binary_terms(BinaryOp::Add, p1, p2)?;
            let (rt2, req2) = self.source_normal_integer(exp, depth + 1)?;
            let comb = self.transitivity(int, term, norm, exp, eq, exp_eq);
            return Ok((rt2, self.transitivity(int, term, exp, rt2, comb, req2)));
        }
        Ok((norm, eq))
    }

    fn source_flat_premises(
        &mut self,
        prop: TermHandle,
        ev: TermHandle,
        out: &mut Vec<(TermHandle, TermHandle)>,
        depth: usize,
    ) -> Result<(), BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let prop = self.source_normal_proposition(prop, 0)?;
        if let Term::Sigma { domain, codomain } = self.arena.get(prop) {
            if self.occurs_free(codomain, 0) {
                return Err(BoundedDenotationError::Unsupported(
                    "dependent source predicate premises require value abstraction",
                ));
            }
            let (f, s) = (self.fst(ev), self.snd(ev));
            self.source_flat_premises(domain, f, out, depth + 1)?;
            let cod = substitute(&mut self.arena, codomain, f);
            self.source_flat_premises(cod, s, out, depth + 1)?;
        } else {
            out.push((prop, ev));
        }
        Ok(())
    }

    fn source_order_goal(
        &mut self,
        goal: TermHandle,
        prems: &[(TermHandle, TermHandle)],
        depth: usize,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        if let Some(p) = self.closed_source_predicate(goal, 0)? {
            return Ok(Some(p));
        }
        if let Some(&(_, ev)) = prems
            .iter()
            .find(|&&(p, _)| self.arena.structurally_equal(goal, p))
        {
            return Ok(Some(ev));
        }
        let Some(IntegerRelation::LessOrEqual { left, right }) = self.integer_relation(goal) else {
            return Ok(None);
        };
        if self.arena.structurally_equal(left, right) {
            return self.le_refl(left).map(Some);
        }
        let add = self.binary_op_position(BinaryOp::Add)?;
        let lsum = self.source_binary(left)?.filter(|(d, ..)| *d == add);
        let Some((_, b, d)) = self.source_binary(right)?.filter(|(d, ..)| *d == add) else {
            return Ok(None);
        };
        if let Some((_, a, c)) = lsum {
            let (g1, g2) = (self.order_term(false, a, b)?, self.order_term(false, c, d)?);
            if let Some(p1) = self.source_order_goal(g1, prems, depth + 1)?
                && let Some(p2) = self.source_order_goal(g2, prems, depth + 1)?
            {
                return self
                    .law(CoreLaw::AddMonotoneBoth, &[a, b, c, d, p1, p2])
                    .map(Some);
            }
        }
        let (zero, sum) = (
            self.math_literal(BigInt::zero())?,
            self.binary_terms(BinaryOp::Add, b, d)?,
        );
        for kept_first in [true, false] {
            let (kept, widened) = if kept_first { (b, d) } else { (d, b) };
            let (kg, zg) = (
                self.order_term(false, left, kept)?,
                self.order_term(false, zero, widened)?,
            );
            let (Some(kp), Some(zp)) = (
                self.source_order_goal(kg, prems, depth + 1)?,
                self.source_order_goal(zg, prems, depth + 1)?,
            ) else {
                continue;
            };
            let (padded, both, law) = if kept_first {
                (
                    self.binary_terms(BinaryOp::Add, left, zero)?,
                    self.law(CoreLaw::AddMonotoneBoth, &[left, b, zero, d, kp, zp])?,
                    CoreLaw::AddZeroRight,
                )
            } else {
                (
                    self.binary_terms(BinaryOp::Add, zero, left)?,
                    self.law(CoreLaw::AddMonotoneBoth, &[zero, b, left, d, zp, kp])?,
                    CoreLaw::AddZeroLeft,
                )
            };
            let ieq = self.law(law, &[left])?;
            return self
                .ord_subst(false, true, padded, left, sum, ieq, both)
                .map(Some);
        }
        Ok(None)
    }

    fn source_assumption_predicate(
        &mut self,
        goal: TermHandle,
        assumptions: &[TermHandle],
        level_arity: u32,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ctx = Context::with_level_arity(level_arity)
            .with_signature(Signature::from_declarations(self.declarations.clone()));
        for prop in assumptions.iter().copied().chain(std::iter::once(goal)) {
            infer_sort(
                &mut self.arena,
                &ctx,
                prop,
                &mut Budget::new(DEFAULT_CONVERSION_STEPS),
            )
            .map_err(BoundedDenotationError::Kernel)?;
        }
        let goal = self.source_normal_proposition(goal, 0)?;
        let mut prems = Vec::new();
        for (pos, &a) in assumptions.iter().enumerate() {
            let ev = self.var((assumptions.len() - 1 - pos) as u32);
            self.source_flat_premises(a, ev, &mut prems, 0)?;
        }
        let ids: Vec<_> = prems
            .iter()
            .copied()
            .filter(|(p, _)| self.identity_parts(*p).is_some())
            .collect();
        let mut orders = Vec::new();
        for &(p, ev) in &prems {
            let (strict, l, r) = match self.integer_relation(p) {
                Some(IntegerRelation::LessOrEqual { left, right }) => (false, left, right),
                Some(IntegerRelation::LessThan { left, right }) => (true, left, right),
                _ => continue,
            };
            let ((lt, leq), (rt, req)) = (
                self.source_normal_integer(l, 0)?,
                self.source_normal_integer(r, 0)?,
            );
            let f = self.ord_subst(strict, true, l, lt, r, leq, ev)?;
            let pr = self.ord_subst(strict, false, r, rt, lt, req, f)?;
            orders.push((self.order_term(strict, lt, rt)?, pr));
        }
        let Some(mut proof) = self.source_rewrite_proof(goal, &ids, |this, rw| {
            if let Some((ty, l, r)) = this.identity_parts(rw) {
                let int = this.integer_constant()?;
                if ty == int {
                    let ((lt, leq), (rt, req)) = (
                        this.source_normal_integer(l, 0)?,
                        this.source_normal_integer(r, 0)?,
                    );
                    if this.arena.structurally_equal(lt, rt) {
                        let rev = this.symmetry(int, r, rt, req);
                        return Ok(Some(this.transitivity(int, l, lt, r, leq, rev)));
                    }
                }
                return this.closed_source_predicate(rw, 0);
            }
            let Some(IntegerRelation::LessOrEqual { left: l, right: r }) =
                this.integer_relation(rw)
            else {
                return this.closed_source_predicate(rw, 0);
            };
            let ((lt, leq), (rt, req)) = (
                this.source_normal_integer(l, 0)?,
                this.source_normal_integer(r, 0)?,
            );
            let norm = this.order_term(false, lt, rt)?;
            let Some(ev) = this.source_order_goal(norm, &orders, 0)? else {
                return Ok(None);
            };
            let int = this.integer_constant()?;
            let (rl, rr) = (
                this.symmetry(int, l, lt, leq),
                this.symmetry(int, r, rt, req),
            );
            let f = this.ord_subst(false, true, lt, l, rt, rl, ev)?;
            this.ord_subst(false, false, rt, r, l, rr, f).map(Some)
        })?
        else {
            return Ok(None);
        };
        for &dom in assumptions.iter().rev() {
            proof = self.lam(dom, proof);
        }
        Ok(Some(proof))
    }
}

// ============================================================================
// Iterative ProofNode elaboration
// ============================================================================

struct Elaboration<'a> {
    denotation: Denotation,
    context: &'a PropositionContext,
    ambient_assumptions: &'a [Proposition],
    axioms: &'a [Proposition],
    machine_parameter_values: &'a BTreeSet<ValueId>,
    ambient: usize,
    context_bindings: Vec<TermHandle>,
    expected: TermHandle,
    roster: Vec<Proposition>,
    positions: Vec<u32>,
    depth: u32,
    syntax_terms: HashMap<Term, TermHandle>,
    steps: u64,
    rules: BTreeSet<AcceptedProofRule>,
    assumptions: PremiseRoster,
    semantic_axioms: PremiseRoster,
}

macro_rules! cert_err {
    ($e:expr) => {
        BoundedDenotationError::Certificate($e)
    };
}

struct ChildEvidence<'p> {
    values: std::vec::IntoIter<(&'p ProofNode, TermHandle)>,
    premises: Option<Vec<Proposition>>,
}

impl ChildEvidence<'_> {
    fn take(&mut self, expected: &ProofNode) -> TermHandle {
        let (src, term) = self.values.next().expect("one elaborated child in order");
        assert!(
            std::ptr::eq(src, expected),
            "child evidence matches scheduled premise"
        );
        term
    }
    fn take_prepared_premises(&mut self) -> Vec<Proposition> {
        self.premises.take().expect("transport premises prepared")
    }
    fn finish(mut self) {
        assert!(self.values.next().is_none(), "all child evidence consumed");
        assert!(self.premises.is_none(), "all transport premises consumed");
    }
}

struct Children<'p> {
    fixed: [Option<&'p ProofNode>; 3],
    remaining: &'p [ProofNode],
}
impl<'p> Children<'p> {
    fn fixed(nodes: [Option<&'p ProofNode>; 3]) -> Self {
        Self {
            fixed: nodes,
            remaining: &[],
        }
    }
    fn slice(nodes: &'p [ProofNode]) -> Self {
        Self {
            fixed: [None; 3],
            remaining: nodes,
        }
    }
    fn len(&self) -> usize {
        self.fixed.iter().filter(|n| n.is_some()).count() + self.remaining.len()
    }
    fn next(&mut self) -> Option<&'p ProofNode> {
        for slot in &mut self.fixed {
            if let Some(n) = slot.take() {
                return Some(n);
            }
        }
        let (h, t) = self.remaining.split_first()?;
        self.remaining = t;
        Some(h)
    }
}

struct PartialNode<'p> {
    proof: &'p ProofNode,
    children: Children<'p>,
    awaiting: Option<&'p ProofNode>,
    evidence: Vec<(&'p ProofNode, TermHandle)>,
    premises: Option<Vec<Proposition>>,
}

impl<'p> PartialNode<'p> {
    fn new(proof: &'p ProofNode) -> Result<Self, BoundedDenotationError> {
        use ProofRule::*;
        let children = match &proof.rule {
            IntegerFloatExact
            | StructuralCaseExhaustion
            | Primitive(_)
            | Assumption { .. }
            | SemanticAxiom { .. }
            | IntegerCorrelatedForbiddenRoots { .. } => Children::fixed([None; 3]),
            ConjunctionIntroduction(cs) => {
                let Proposition::Conjunction(exp) = &proof.conclusion else {
                    return Err(cert_err!(ProofError::RuleConclusionMismatch(
                        "conjunction introduction"
                    )));
                };
                if cs.len() != exp.len() {
                    return Err(cert_err!(ProofError::ConjunctionArityMismatch));
                }
                Children::slice(cs)
            }
            FloatIntegerConversion { premise, computed } => {
                Children::fixed([Some(premise), computed.as_deref(), None])
            }
            IeeeFloatOrderChain(cs) | FloatMeaningNormalization(cs) => Children::slice(cs),
            ConjunctionElimination { conjunction: a, .. } => Children::fixed([Some(a), None, None]),
            EqualitySymmetry { equality: a }
            | PredicateDenotation { premise: a }
            | IntegerOrderDiscreteness { relation: a }
            | IntegerOrderWeakening { relation: a }
            | IntegerMathOrderTranslation { relation: a, .. }
            | IntegerPowerOfTwoOrder { order: a }
            | IntegerAffineBound { root_bound: a, .. }
            | IntegerCastBound { root_bound: a, .. } => Children::fixed([Some(a), None, None]),
            DisjunctionIntroduction { disjunct, index } => {
                let Proposition::Disjunction(ds) = &proof.conclusion else {
                    return Err(cert_err!(ProofError::RuleConclusionMismatch(
                        "disjunction introduction"
                    )));
                };
                if *index >= ds.len() {
                    return Err(cert_err!(ProofError::UnknownDisjunct(*index)));
                }
                Children::fixed([Some(disjunct), None, None])
            }
            ImplicationElimination {
                implication: a,
                premise: b,
            }
            | EqualityTransitivity {
                left_equals_middle: a,
                middle_equals_right: b,
            }
            | IntegerSubtractOrder {
                difference: a,
                positive: b,
            }
            | IntegerAddOrder {
                sum: a,
                positive: b,
            }
            | IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: a,
                middle_less_or_equal_right: b,
            }
            | IntegerStrictOrderTransitivity {
                left_to_middle: a,
                middle_to_right: b,
            }
            | IntegerOrderAntisymmetry {
                forward: a,
                backward: b,
            }
            | IntegerOrderSubstitution {
                relation: a,
                equality: b,
                ..
            }
            | IntegerExactAddDefinitionBound {
                left_bound: a,
                right_bound: b,
                ..
            }
            | IntegerExactSubtractDefinitionBound {
                left_bound: a,
                right_bound: b,
                ..
            } => Children::fixed([Some(a), Some(b), None]),
            IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => Children::fixed([Some(smaller), Some(larger), Some(order)]),
            ValueEqualityTransport {
                premise,
                equalities,
            } => Children {
                fixed: [Some(premise), None, None],
                remaining: equalities,
            },
            ImplicationIntroduction { .. } | DisjunctionElimination { .. } => unreachable!(),
        };
        let count = children.len();
        let premises =
            matches!(&proof.rule, ValueEqualityTransport { .. }).then(|| Vec::with_capacity(count));
        Ok(Self {
            proof,
            children,
            awaiting: None,
            evidence: Vec::with_capacity(count),
            premises,
        })
    }

    fn next_child(&mut self) -> Option<&'p ProofNode> {
        let c = self.children.next()?;
        if let Some(p) = &mut self.premises {
            p.push(c.conclusion.clone());
        }
        self.awaiting = Some(c);
        Some(c)
    }

    fn finish(self, e: &mut Elaboration<'_>) -> Result<TermHandle, BoundedDenotationError> {
        let mut ops = ChildEvidence {
            values: self.evidence.into_iter(),
            premises: self.premises,
        };
        let term = e.finish_regular(self.proof, &mut ops)?;
        ops.finish();
        Ok(term)
    }
}

#[derive(Clone, Copy)]
struct Scope {
    depth: u32,
    roster: usize,
    positions: usize,
}
impl Scope {
    fn current(e: &Elaboration<'_>) -> Self {
        Self {
            depth: e.depth,
            roster: e.roster.len(),
            positions: e.positions.len(),
        }
    }
    fn restore(self, e: &mut Elaboration<'_>) {
        e.depth = self.depth;
        e.positions.truncate(self.positions);
        e.roster.truncate(self.roster);
    }
    fn close_hypothesis(self, e: &mut Elaboration<'_>) {
        assert_eq!(
            (e.depth, e.roster.len(), e.positions.len()),
            (self.depth + 1, self.roster + 1, self.positions + 1)
        );
        self.restore(e);
    }
    fn close_intermediate(self, e: &mut Elaboration<'_>) {
        assert_eq!(
            (e.depth, e.roster.len(), e.positions.len()),
            (self.depth + 1, self.roster, self.positions)
        );
        self.restore(e);
    }
    fn assert_current(self, e: &Elaboration<'_>) {
        assert_eq!(
            (e.depth, e.roster.len(), e.positions.len()),
            (self.depth, self.roster, self.positions)
        );
    }
}

#[derive(Clone, Copy)]
struct Case<'p> {
    disjuncts: &'p [Proposition],
    branches: &'p [ProofNode],
    scrutinee: TermHandle,
    conclusion: TermHandle,
}
#[derive(Clone, Copy)]
struct CaseParts<'p> {
    case: Case<'p>,
    first: TermHandle,
    rest: TermHandle,
    zero: TermHandle,
}

impl<'a> Elaboration<'a> {
    /// `denotation` may already declare what the propositions name, such as
    /// a theorem's sum inputs.
    fn new(
        mut denotation: Denotation,
        context: &'a PropositionContext,
        goal: &Proposition,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
        machine_parameter_values: &'a BTreeSet<ValueId>,
    ) -> Result<Self, BoundedDenotationError> {
        for prop in std::iter::once(goal)
            .chain(assumptions)
            .chain(semantic_axioms)
        {
            context
                .validate(prop)
                .map_err(|e| cert_err!(ProofError::MalformedProposition(e)))?;
        }
        let mut context_bindings = Vec::with_capacity(assumptions.len() + semantic_axioms.len());
        for prop in assumptions.iter().chain(semantic_axioms) {
            context_bindings.push(denotation.denote(prop)?);
        }
        let expected = denotation.denote(goal)?;
        let depth = u32::try_from(assumptions.len() + semantic_axioms.len())
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
        let positions = (0..assumptions.len())
            .map(|i| u32::try_from(i).expect("fits"))
            .collect();
        Ok(Self {
            denotation,
            context,
            ambient_assumptions: assumptions,
            axioms: semantic_axioms,
            machine_parameter_values,
            ambient: assumptions.len(),
            context_bindings,
            expected,
            roster: assumptions.to_vec(),
            positions,
            depth,
            syntax_terms: HashMap::new(),
            steps: 0,
            rules: BTreeSet::new(),
            assumptions: PremiseRoster::default(),
            semantic_axioms: PremiseRoster::default(),
        })
    }

    fn proof_term(&mut self, term: Term) -> TermHandle {
        let arena = &mut self.denotation.arena;
        *self
            .syntax_terms
            .entry(term)
            .or_insert_with_key(|t| arena.insert(t.clone()))
    }
    fn variable(&mut self, pos: u32) -> TermHandle {
        self.proof_term(Term::Variable(self.depth - 1 - pos))
    }
    fn push_hypothesis(&mut self, prop: &Proposition) -> Scope {
        let s = Scope::current(self);
        self.roster.push(prop.clone());
        self.positions.push(self.depth);
        self.depth += 1;
        s
    }
    fn enter(&mut self, proof: &ProofNode) -> Result<(), BoundedDenotationError> {
        self.steps += 1;
        if self.steps > MAX_ELABORATION_NODES {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        if crate::proof::cites_validated_premise(proof, &self.roster, self.axioms) {
            return Ok(());
        }
        self.context
            .validate(&proof.conclusion)
            .map_err(|e| cert_err!(ProofError::MalformedProposition(e)))
    }

    fn node(&mut self, proof: &ProofNode) -> Result<TermHandle, BoundedDenotationError> {
        let root = Scope::current(self);
        let res = self.drive(proof);
        if res.is_ok() {
            root.assert_current(self);
        } else {
            root.restore(self);
        }
        res
    }

    fn drive(&mut self, proof: &ProofNode) -> Result<TermHandle, BoundedDenotationError> {
        enum W<'p> {
            Node(&'p ProofNode),
            Case(Case<'p>),
        }
        enum P<'p> {
            Children(PartialNode<'p>),
            Imp(&'p ProofNode, &'p ProofNode, TermHandle, Scope),
            Disj(&'p ProofNode, &'p ProofNode, &'p [ProofNode]),
            DisjDone(&'p ProofNode, &'p [ProofNode]),
            CaseZero(Case<'p>, TermHandle, Scope),
            CaseOne(CaseParts<'p>, Scope, bool),
        }
        let (mut pending, mut cur) = (Vec::new(), W::Node(proof));
        'drive: loop {
            let mut val = match cur {
                W::Node(p) => {
                    self.enter(p)?;
                    match &p.rule {
                        ProofRule::ImplicationIntroduction { body } => {
                            let Proposition::Implication { premise, .. } = &p.conclusion else {
                                return Err(cert_err!(ProofError::RuleConclusionMismatch(
                                    "implication introduction"
                                )));
                            };
                            let dom = self.denotation.denote(premise)?;
                            let sc = self.push_hypothesis(premise);
                            pending.push(P::Imp(p, body, dom, sc));
                            cur = W::Node(body);
                            continue 'drive;
                        }
                        ProofRule::DisjunctionElimination {
                            disjunction,
                            branches,
                        } => {
                            pending.push(P::Disj(p, disjunction, branches));
                            cur = W::Node(disjunction);
                            continue 'drive;
                        }
                        _ => {
                            let mut node = PartialNode::new(p)?;
                            if let Some(c) = node.next_child() {
                                pending.push(P::Children(node));
                                cur = W::Node(c);
                                continue 'drive;
                            }
                            node.finish(self)?
                        }
                    }
                }
                W::Case(case) => {
                    assert!(
                        case.disjuncts.len() >= 2 && case.disjuncts.len() == case.branches.len()
                    );
                    let first = self.denotation.denote(&case.disjuncts[0])?;
                    let sc = self.push_hypothesis(&case.disjuncts[0]);
                    pending.push(P::CaseZero(case, first, sc));
                    cur = W::Node(&case.branches[0]);
                    continue 'drive;
                }
            };
            loop {
                match pending.pop() {
                    Some(P::Children(mut node)) => {
                        let src = node.awaiting.take().expect("scheduled child");
                        node.evidence.push((src, val));
                        if let Some(c) = node.next_child() {
                            pending.push(P::Children(node));
                            cur = W::Node(c);
                            continue 'drive;
                        }
                        val = node.finish(self)?;
                    }
                    Some(P::Imp(p, body, dom, sc)) => {
                        sc.close_hypothesis(self);
                        let term = self.denotation.lam(dom, val);
                        let Proposition::Implication { conclusion, .. } = &p.conclusion else {
                            unreachable!()
                        };
                        if &body.conclusion != conclusion.as_ref() {
                            return Err(cert_err!(ProofError::ImplicationConclusionMismatch));
                        }
                        self.rules
                            .insert(AcceptedProofRule::ImplicationIntroduction);
                        val = term;
                    }
                    Some(P::Disj(p, disj, branches)) => {
                        let Proposition::Disjunction(ds) = &disj.conclusion else {
                            return Err(cert_err!(ProofError::RulePremiseMismatch(
                                "disjunction elimination"
                            )));
                        };
                        if branches.len() != ds.len() {
                            return Err(cert_err!(ProofError::DisjunctionArityMismatch));
                        }
                        let conclusion = self.denotation.denote(&p.conclusion)?;
                        pending.push(P::DisjDone(p, branches));
                        cur = W::Case(Case {
                            disjuncts: ds,
                            branches,
                            scrutinee: val,
                            conclusion,
                        });
                        continue 'drive;
                    }
                    Some(P::DisjDone(p, bs)) => {
                        if bs.iter().any(|b| b.conclusion != p.conclusion) {
                            return Err(cert_err!(ProofError::DisjunctionBranchConclusionMismatch));
                        }
                        self.rules.insert(AcceptedProofRule::DisjunctionElimination);
                    }
                    Some(P::CaseZero(case, first, sc)) => {
                        sc.close_hypothesis(self);
                        let zero = self.denotation.lam(first, val);
                        let rest = if case.disjuncts.len() == 2 {
                            self.denotation.denote(&case.disjuncts[1])?
                        } else {
                            self.denotation.disjunction(&case.disjuncts[1..])?
                        };
                        let parts = CaseParts {
                            case,
                            first,
                            rest,
                            zero,
                        };
                        if case.disjuncts.len() == 2 {
                            let sc = self.push_hypothesis(&case.disjuncts[1]);
                            pending.push(P::CaseOne(parts, sc, false));
                            cur = W::Node(&case.branches[1]);
                        } else {
                            let sc = Scope::current(self);
                            self.depth += 1;
                            let bound = self.denotation.var(0);
                            pending.push(P::CaseOne(parts, sc, true));
                            cur = W::Case(Case {
                                disjuncts: &case.disjuncts[1..],
                                branches: &case.branches[1..],
                                scrutinee: bound,
                                conclusion: case.conclusion,
                            });
                        }
                        continue 'drive;
                    }
                    Some(P::CaseOne(parts, sc, nested)) => {
                        if nested {
                            sc.close_intermediate(self);
                        } else {
                            sc.close_hypothesis(self);
                        }
                        let one = self.denotation.lam(parts.rest, val);
                        let (tag, payload) = (
                            self.proof_term(Term::Fst {
                                pair: parts.case.scrutinee,
                            }),
                            self.proof_term(Term::Snd {
                                pair: parts.case.scrutinee,
                            }),
                        );
                        val = self.denotation.tagged_sum_elim_with(
                            parts.first,
                            parts.rest,
                            tag,
                            payload,
                            parts.zero,
                            one,
                            parts.case.conclusion,
                        );
                    }
                    None => return Ok(val),
                }
            }
        }
    }

    fn finish_or_instance(
        &mut self,
        rule: AcceptedProofRule,
        derived: Option<TermHandle>,
        prems: Vec<Proposition>,
        evs: Vec<TermHandle>,
        concl: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        if let Some(t) = derived {
            self.rules.insert(rule);
            return Ok(t);
        }
        self.rule_instance(rule, prems, evs, concl)
    }

    fn finish_regular(
        &mut self,
        proof: &ProofNode,
        ops: &mut ChildEvidence<'_>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        match &proof.rule {
            ProofRule::StructuralCaseExhaustion => {
                let term = self.denotation.data_case_exhaustion(&proof.conclusion)?;
                self.rules
                    .insert(AcceptedProofRule::StructuralCaseExhaustion);
                Ok(term)
            }
            ProofRule::IntegerFloatExact => {
                crate::check_integer_float_exact(self.context, &proof.conclusion)
                    .map_err(|e| cert_err!(ProofError::FloatIntegerConversion(Box::new(e))))?;
                self.rule_instance(
                    AcceptedProofRule::IntegerFloatExact,
                    Vec::new(),
                    Vec::new(),
                    &proof.conclusion,
                )
            }
            ProofRule::FloatIntegerConversion { premise, computed } => {
                crate::check_float_integer_conversion(
                    self.context,
                    &proof.conclusion,
                    &premise.conclusion,
                    computed.as_ref().map(|p| &p.conclusion),
                )
                .map_err(|e| cert_err!(ProofError::FloatIntegerConversion(Box::new(e))))?;
                let (mut ps, mut evs) = (vec![premise.conclusion.clone()], vec![ops.take(premise)]);
                if let Some(c) = computed {
                    ps.push(c.conclusion.clone());
                    evs.push(ops.take(c));
                }
                self.rule_instance(
                    AcceptedProofRule::FloatIntegerConversion,
                    ps,
                    evs,
                    &proof.conclusion,
                )
            }
            ProofRule::FloatMeaningNormalization(ps) => {
                crate::check_float_meaning_normalization(
                    &proof.conclusion,
                    ps.iter().map(|p| &p.conclusion),
                )
                .map_err(|e| cert_err!(ProofError::FloatMeaningNormalization(Box::new(e))))?;
                let (evs, prems) = (
                    ps.iter().map(|p| ops.take(p)).collect(),
                    ps.iter().map(|p| p.conclusion.clone()).collect(),
                );
                self.rule_instance(
                    AcceptedProofRule::FloatMeaningNormalization,
                    prems,
                    evs,
                    &proof.conclusion,
                )
            }
            ProofRule::IeeeFloatOrderChain(ps) => {
                crate::ieee_float_rules::check_chain(
                    &proof.conclusion,
                    ps.iter().map(|p| &p.conclusion),
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (evs, prems) = (
                    ps.iter().map(|p| ops.take(p)).collect(),
                    ps.iter().map(|p| p.conclusion.clone()).collect(),
                );
                self.rule_instance(
                    AcceptedProofRule::IeeeFloatOrderChain,
                    prems,
                    evs,
                    &proof.conclusion,
                )
            }
            ProofRule::Primitive(j) => {
                decide_primitive(self.context, &proof.conclusion, *j)
                    .map_err(|e| cert_err!(ProofError::PrimitiveJudgment(e)))?;
                self.rules.insert(AcceptedProofRule::Primitive);
                let d = self.denotation.denote(&proof.conclusion)?;
                if let Some(ev) = self.denotation.closed_inhabitant(d)? {
                    return Ok(ev);
                }
                self.denotation.decision(d)
            }
            ProofRule::Assumption { index } => self.cited(*index, &proof.conclusion),
            ProofRule::SemanticAxiom { index } => {
                let Some(ax) = self.axioms.get(*index) else {
                    return Err(cert_err!(ProofError::UnknownSemanticAxiom(*index)));
                };
                if !propositions_match(ax, &proof.conclusion) {
                    return Err(cert_err!(ProofError::SemanticAxiomConclusionMismatch(
                        *index
                    )));
                }
                Self::same_denotation(&mut self.denotation, ax, &proof.conclusion)?;
                self.rules.insert(AcceptedProofRule::SemanticAxiom);
                self.semantic_axioms.record(*index, ax);
                let pos = u32::try_from(self.ambient + *index)
                    .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
                Ok(self.variable(pos))
            }
            ProofRule::ConjunctionIntroduction(cs) => {
                let Proposition::Conjunction(exp) = &proof.conclusion else {
                    unreachable!()
                };
                let mut terms: Vec<_> = cs.iter().map(|c| ops.take(c)).collect();
                if cs.iter().zip(exp).any(|(c, e)| &c.conclusion != e) {
                    return Err(cert_err!(ProofError::ConjunctConclusionMismatch));
                }
                self.rules
                    .insert(AcceptedProofRule::ConjunctionIntroduction);
                let mut t = terms.pop().expect("arity");
                while let Some(f) = terms.pop() {
                    t = self.denotation.pair(f, t);
                }
                Ok(t)
            }
            ProofRule::ConjunctionElimination {
                conjunction,
                conjunct,
            } => {
                let pair = ops.take(conjunction);
                let Proposition::Conjunction(cs) = &conjunction.conclusion else {
                    return Err(cert_err!(ProofError::RulePremiseMismatch(
                        "conjunction elimination"
                    )));
                };
                let sel = cs
                    .get(*conjunct)
                    .ok_or(cert_err!(ProofError::UnknownConjunct(*conjunct)))?;
                if *sel != proof.conclusion {
                    return Err(cert_err!(ProofError::ConjunctConclusionMismatch));
                }
                self.rules.insert(AcceptedProofRule::ConjunctionElimination);
                let (last, mut t) = (cs.len() - 1, pair);
                for _ in 0..(*conjunct).min(last) {
                    t = self.proof_term(Term::Snd { pair: t });
                }
                if *conjunct < last {
                    t = self.proof_term(Term::Fst { pair: t });
                }
                Ok(t)
            }
            ProofRule::DisjunctionIntroduction { disjunct, index } => {
                let Proposition::Disjunction(ds) = &proof.conclusion else {
                    unreachable!()
                };
                let ev = ops.take(disjunct);
                if ds[*index] != disjunct.conclusion {
                    return Err(cert_err!(ProofError::DisjunctConclusionMismatch));
                }
                self.rules
                    .insert(AcceptedProofRule::DisjunctionIntroduction);
                let pair = self.disjunction_intro(ds.len(), *index, ev);
                // The kernel infers an unannotated pair's type as a plain product, which
                // loses the tag-indexed family of a disjunction. Where the introduction is
                // inferred, as inside an implication proof applied on the spot, that product
                // differs from the denoted disjunction, so the pair is ascribed its type.
                let ty = self.denotation.denote(&proof.conclusion)?;
                Ok(self.denotation.ascribe(ty, pair))
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => {
                let (f, a) = (ops.take(implication), ops.take(premise));
                let Proposition::Implication {
                    premise: req,
                    conclusion,
                } = &implication.conclusion
                else {
                    return Err(cert_err!(ProofError::RulePremiseMismatch(
                        "implication elimination"
                    )));
                };
                if premise.conclusion != **req {
                    return Err(cert_err!(ProofError::ImplicationPremiseMismatch));
                }
                if &proof.conclusion != conclusion.as_ref() {
                    return Err(cert_err!(ProofError::ImplicationConclusionMismatch));
                }
                self.rules.insert(AcceptedProofRule::ImplicationElimination);
                Ok(self.denotation.app(f, a))
            }
            ProofRule::EqualitySymmetry { equality } => {
                let ev = ops.take(equality);
                let Proposition::Equal(l, r) = &equality.conclusion else {
                    return Err(cert_err!(ProofError::RulePremiseMismatch(
                        "equality symmetry"
                    )));
                };
                if proof.conclusion != Proposition::Equal(r.clone(), l.clone()) {
                    return Err(cert_err!(ProofError::EqualityConclusionMismatch));
                }
                let (pt, gt) = (
                    self.denotation.denote(&equality.conclusion)?,
                    self.denotation.denote(&proof.conclusion)?,
                );
                let d = self.denotation.symmetry_evidence(pt, gt, ev);
                self.finish_or_instance(
                    AcceptedProofRule::EqualitySymmetry,
                    d,
                    vec![equality.conclusion.clone()],
                    vec![ev],
                    &proof.conclusion,
                )
            }
            ProofRule::EqualityTransitivity {
                left_equals_middle: a,
                middle_equals_right: b,
            } => {
                let (f, s) = (ops.take(a), ops.take(b));
                equality_rules::equality_transitivity_relation(
                    &a.conclusion,
                    &b.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (ft, st, gt) = (
                    self.denotation.denote(&a.conclusion)?,
                    self.denotation.denote(&b.conclusion)?,
                    self.denotation.denote(&proof.conclusion)?,
                );
                let d = self.denotation.transitivity_evidence(ft, f, st, s, gt);
                self.finish_or_instance(
                    AcceptedProofRule::EqualityTransitivity,
                    d,
                    vec![a.conclusion.clone(), b.conclusion.clone()],
                    vec![f, s],
                    &proof.conclusion,
                )
            }
            ProofRule::PredicateDenotation { premise } => {
                let ev = ops.take(premise);
                equality_rules::predicate_denotation_relation(
                    self.context,
                    &premise.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (pt, gt) = (
                    self.denotation.denote(&premise.conclusion)?,
                    self.denotation.denote(&proof.conclusion)?,
                );
                let d = self.denotation.oriented_evidence(pt, gt, ev)?;
                self.finish_or_instance(
                    AcceptedProofRule::PredicateDenotation,
                    d,
                    vec![premise.conclusion.clone()],
                    vec![ev],
                    &proof.conclusion,
                )
            }
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => {
                let prems = ops.take_prepared_premises();
                let mut evs = Vec::with_capacity(equalities.len() + 1);
                evs.push(ops.take(premise));
                for eq in equalities {
                    evs.push(ops.take(eq));
                }
                equality_rules::value_equality_transport_relation(
                    self.context,
                    &premise.conclusion,
                    equalities.iter().map(|e| &e.conclusion),
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let d = self.denotation.compositional_transport(
                    &premise.conclusion,
                    evs[0],
                    equalities,
                    &evs[1..],
                    &proof.conclusion,
                )?;
                self.finish_or_instance(
                    AcceptedProofRule::ValueEqualityTransport,
                    d,
                    prems,
                    evs,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => {
                let (de, pe) = (ops.take(difference), ops.take(positive));
                subtract_order::check(
                    &difference.conclusion,
                    &positive.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::IntegerSubtractOrder);
                self.denotation.subtraction_evidence(
                    &difference.conclusion,
                    de,
                    pe,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerAddOrder { sum, positive } => {
                let (se, pe) = (ops.take(sum), ops.take(positive));
                add_order::check(&sum.conclusion, &positive.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                let d = self
                    .denotation
                    .addition_order_evidence(&sum.conclusion, se, pe)?;
                self.finish_or_instance(
                    AcceptedProofRule::IntegerAddOrder,
                    d,
                    vec![sum.conclusion.clone(), positive.conclusion.clone()],
                    vec![se, pe],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => {
                let (se, le, oe) = (ops.take(smaller), ops.take(larger), ops.take(order));
                subtract_antitone::check(
                    &smaller.conclusion,
                    &larger.conclusion,
                    &order.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let d = self.denotation.subtraction_antitone_evidence(
                    &smaller.conclusion,
                    se,
                    &larger.conclusion,
                    le,
                    oe,
                )?;
                self.finish_or_instance(
                    AcceptedProofRule::IntegerSubtractAntitone,
                    d,
                    vec![
                        smaller.conclusion.clone(),
                        larger.conclusion.clone(),
                        order.conclusion.clone(),
                    ],
                    vec![se, le, oe],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerOrderDiscreteness { relation } => {
                let ev = ops.take(relation);
                order_discreteness::check(&relation.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                if !matches!(
                    (&relation.conclusion, &proof.conclusion),
                    (Proposition::LessOrEqual(..), Proposition::LessThan(..))
                ) {
                    let d = self.denotation.successor_evidence(
                        &relation.conclusion,
                        ev,
                        &proof.conclusion,
                    )?;
                    return self.finish_or_instance(
                        AcceptedProofRule::IntegerOrderDiscreteness,
                        d,
                        vec![relation.conclusion.clone()],
                        vec![ev],
                        &proof.conclusion,
                    );
                }
                self.rules
                    .insert(AcceptedProofRule::IntegerOrderDiscreteness);
                self.denotation
                    .discreteness_evidence(&relation.conclusion, ev, &proof.conclusion)
            }
            ProofRule::IntegerOrderWeakening { relation } => {
                let ev = ops.take(relation);
                integer_order_rules::order_weakening(&relation.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                let (pt, gt) = (
                    self.denotation.denote(&relation.conclusion)?,
                    self.denotation.denote(&proof.conclusion)?,
                );
                let d = self.denotation.weakening_evidence(pt, ev, gt)?;
                self.finish_or_instance(
                    AcceptedProofRule::IntegerOrderWeakening,
                    d,
                    vec![relation.conclusion.clone()],
                    vec![ev],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: a,
                middle_less_or_equal_right: b,
            }
            | ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: a,
                middle_to_right: b,
            } => {
                let (f, s) = (ops.take(a), ops.take(b));
                let rule = if matches!(
                    &proof.rule,
                    ProofRule::IntegerLessOrEqualTransitivity { .. }
                ) {
                    integer_order_rules::less_or_equal_transitivity(
                        &a.conclusion,
                        &b.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    AcceptedProofRule::IntegerLessOrEqualTransitivity
                } else {
                    strict_order_transitivity::check(
                        &a.conclusion,
                        &b.conclusion,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                    AcceptedProofRule::IntegerStrictOrderTransitivity
                };
                let (ft, st, gt) = (
                    self.denotation.denote(&a.conclusion)?,
                    self.denotation.denote(&b.conclusion)?,
                    self.denotation.denote(&proof.conclusion)?,
                );
                let d = self
                    .denotation
                    .order_transitivity_evidence(ft, f, st, s, gt)?;
                self.finish_or_instance(
                    rule,
                    d,
                    vec![a.conclusion.clone(), b.conclusion.clone()],
                    vec![f, s],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                let (fe, be) = (ops.take(forward), ops.take(backward));
                integer_order_rules::order_antisymmetry(
                    &forward.conclusion,
                    &backward.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerOrderAntisymmetry);
                let concl = self.denotation.denote(&proof.conclusion)?;
                self.denotation
                    .order_antisymmetry_evidence(&forward.conclusion, fe, be, concl)
            }
            ProofRule::IntegerMathOrderTranslation { relation, addend } => {
                let ev = ops.take(relation);
                integer_order_rules::math_order_translation(
                    &relation.conclusion,
                    addend,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerMathOrderTranslation);
                self.denotation
                    .mathematical_order_translation(&relation.conclusion, addend, ev)
            }
            ProofRule::IntegerOrderSubstitution {
                relation,
                equality,
                endpoint,
            } => {
                let (re, ee) = (ops.take(relation), ops.take(equality));
                integer_order_rules::endpoint_substitution(
                    &relation.conclusion,
                    &equality.conclusion,
                    *endpoint,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (rt, et, gt) = (
                    self.denotation.denote(&relation.conclusion)?,
                    self.denotation.denote(&equality.conclusion)?,
                    self.denotation.denote(&proof.conclusion)?,
                );
                let d = self
                    .denotation
                    .order_substitution_evidence(rt, re, et, ee, *endpoint, gt)?;
                self.finish_or_instance(
                    AcceptedProofRule::IntegerOrderSubstitution,
                    d,
                    vec![relation.conclusion.clone(), equality.conclusion.clone()],
                    vec![re, ee],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerAffineBound {
                root_bound,
                witness,
            } => {
                let root = ops.take(root_bound);
                let (rule, cited) = integer_bound_rules::affine_bound_relation(
                    self.context,
                    self.axioms,
                    &root_bound.conclusion,
                    witness,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                if let Some(ev) = self.denotation.unsigned_subtract_bound_evidence(
                    &root_bound.conclusion,
                    root,
                    witness,
                    &proof.conclusion,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(ev);
                }
                let defs = cited
                    .iter()
                    .map(|&i| self.cited_axiom(i))
                    .collect::<Result<Vec<_>, _>>()?;
                let rb = &root_bound.conclusion;
                let derived = self
                    .denotation
                    .correlated_add_sub_bound_evidence(
                        false,
                        rb,
                        root,
                        witness,
                        &proof.conclusion,
                        &defs,
                    )?
                    .or(self.denotation.correlated_add_sub_bound_evidence(
                        true,
                        rb,
                        root,
                        witness,
                        &proof.conclusion,
                        &defs,
                    )?)
                    .or(self.denotation.correlated_multiply_bound_evidence(
                        rb,
                        root,
                        witness,
                        &proof.conclusion,
                        &defs,
                    )?)
                    .or(self.denotation.direct_binary_bound_evidence(
                        &BINARY_BOUND_SPECS[0],
                        rb,
                        root,
                        witness,
                        &proof.conclusion,
                    )?)
                    .or(self.denotation.direct_binary_bound_evidence(
                        &BINARY_BOUND_SPECS[1],
                        rb,
                        root,
                        witness,
                        &proof.conclusion,
                    )?);
                if let Some(ev) = derived {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(ev);
                }
                let (mut ps, mut evs) = (
                    Vec::with_capacity(defs.len() + 1),
                    Vec::with_capacity(defs.len() + 1),
                );
                ps.push(rb.clone());
                evs.push(root);
                for (p, v) in defs {
                    ps.push(p);
                    evs.push(v);
                }
                self.rule_instance(rule, ps, evs, &proof.conclusion)
            }
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            }
            | ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => {
                let is_add = matches!(
                    &proof.rule,
                    ProofRule::IntegerExactAddDefinitionBound { .. }
                );
                let (op, rule, spec) = if is_add {
                    (
                        integer_bound_rules::ExactDefinitionOperation::Add,
                        AcceptedProofRule::IntegerExactAddDefinitionBound,
                        &BINARY_BOUND_SPECS[0],
                    )
                } else {
                    (
                        integer_bound_rules::ExactDefinitionOperation::Subtract,
                        AcceptedProofRule::IntegerExactSubtractDefinitionBound,
                        &BINARY_BOUND_SPECS[1],
                    )
                };
                let (l, r) = (ops.take(left_bound), ops.take(right_bound));
                integer_bound_rules::exact_definition_bound_relation(
                    self.context,
                    self.axioms,
                    &left_bound.conclusion,
                    &right_bound.conclusion,
                    *definition_axiom,
                    &proof.conclusion,
                    op,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (def, var) = self.cited_axiom(*definition_axiom)?;
                let d = self.denotation.exact_binary_definition_bound_evidence(
                    spec,
                    &left_bound.conclusion,
                    l,
                    &right_bound.conclusion,
                    r,
                    &def,
                    var,
                    &proof.conclusion,
                )?;
                self.finish_or_instance(
                    rule,
                    d,
                    vec![
                        left_bound.conclusion.clone(),
                        right_bound.conclusion.clone(),
                        def,
                    ],
                    vec![l, r, var],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerCastBound {
                root_bound,
                witness,
            } => {
                let root = ops.take(root_bound);
                let (cits, chain) = integer_bound_rules::cast_bound_relation(
                    self.context,
                    self.axioms,
                    &root_bound.conclusion,
                    witness,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (mut ps, mut evs) = (vec![root_bound.conclusion.clone()], vec![root]);
                for &i in &cits {
                    let (p, v) = self.cited_axiom(i)?;
                    ps.push(p);
                    evs.push(v);
                }
                let d = self.denotation.cast_bound_evidence(
                    &root_bound.conclusion,
                    evs[0],
                    &chain,
                    &evs[1..],
                    self.axioms,
                    &proof.conclusion,
                )?;
                self.finish_or_instance(
                    AcceptedProofRule::IntegerCastBound,
                    d,
                    ps,
                    evs,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
                let (cits, chk) = integer_bound_rules::correlated_forbidden_roots_relation(
                    self.context,
                    self.ambient_assumptions,
                    self.axioms,
                    self.machine_parameter_values,
                    witness,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (mut ps, mut evs, mut ax_ev, mut bd_ev) =
                    (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for i in cits.semantic_axioms {
                    let (p, v) = self.cited_axiom(i)?;
                    ax_ev.push((p.clone(), v));
                    ps.push(p);
                    evs.push(v);
                }
                for i in cits.assumptions {
                    let (p, v) = self.ambient_premise(i)?;
                    bd_ev.push((p.clone(), v));
                    ps.push(p);
                    evs.push(v);
                }
                let d = self.denotation.correlated_forbidden_roots_evidence(
                    witness,
                    &chk,
                    &ax_ev,
                    &bd_ev,
                    &proof.conclusion,
                )?;
                self.finish_or_instance(
                    AcceptedProofRule::IntegerCorrelatedForbiddenRoots,
                    d,
                    ps,
                    evs,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerPowerOfTwoOrder { order } => {
                let ev = ops.take(order);
                crate::proof::power_of_two_order::check(&order.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                self.rule_instance(
                    AcceptedProofRule::IntegerPowerOfTwoOrder,
                    vec![order.conclusion.clone()],
                    vec![ev],
                    &proof.conclusion,
                )
            }
            ProofRule::DisjunctionElimination { .. }
            | ProofRule::ImplicationIntroduction { .. } => unreachable!(),
        }
    }

    fn cited(
        &mut self,
        idx: usize,
        concl: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Some(a) = self.roster.get(idx) else {
            return Err(cert_err!(ProofError::UnknownAssumption(idx)));
        };
        if !propositions_match(a, concl) {
            return Err(cert_err!(ProofError::AssumptionConclusionMismatch(idx)));
        }
        Self::same_denotation(&mut self.denotation, a, concl)?;
        self.rules.insert(AcceptedProofRule::Assumption);
        if idx < self.ambient {
            self.assumptions.record(idx, a);
        }
        let pos = self.positions[idx];
        Ok(self.variable(pos))
    }

    fn same_denotation(
        d: &mut Denotation,
        a: &Proposition,
        b: &Proposition,
    ) -> Result<(), BoundedDenotationError> {
        if a == b {
            return Ok(());
        }
        let (ra, rb) = (d.denote(a)?, d.denote(b)?);
        if d.arena.structurally_equal(ra, rb) {
            Ok(())
        } else {
            Err(BoundedDenotationError::Unsupported(
                "integer-math-normalized citation across denotation shapes",
            ))
        }
    }

    fn ambient_premise(
        &mut self,
        idx: usize,
    ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
        if idx >= self.ambient {
            return Err(cert_err!(ProofError::UnknownAssumption(idx)));
        }
        let Some(p) = self.roster.get(idx).cloned() else {
            return Err(cert_err!(ProofError::UnknownAssumption(idx)));
        };
        self.assumptions.record(idx, &p);
        let pos = self.positions[idx];
        Ok((p, self.variable(pos)))
    }

    fn cited_axiom(
        &mut self,
        idx: usize,
    ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
        let Some(p) = self.axioms.get(idx).cloned() else {
            return Err(cert_err!(ProofError::UnknownSemanticAxiom(idx)));
        };
        self.semantic_axioms.record(idx, &p);
        let pos = u32::try_from(self.ambient + idx)
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
        Ok((p, self.variable(pos)))
    }

    fn rule_instance(
        &mut self,
        rule: AcceptedProofRule,
        prems: Vec<Proposition>,
        evs: Vec<TermHandle>,
        concl: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        debug_assert_eq!(prems.len(), evs.len());
        let ax = self.denotation.rule_axiom(&prems, concl)?;
        let term = self.denotation.arena.apply_arguments(ax, &evs);
        self.rules.insert(rule);
        Ok(term)
    }

    fn disjunction_intro(
        &mut self,
        mut rem: usize,
        mut idx: usize,
        mut term: TermHandle,
    ) -> TermHandle {
        let mut tags = Vec::new();
        loop {
            tags.push(self.denotation.arena.insert(if idx == 0 {
                Term::TwoZero
            } else {
                Term::TwoOne
            }));
            if idx == 0 || rem == 2 {
                break;
            }
            rem -= 1;
            idx -= 1;
        }
        while let Some(f) = tags.pop() {
            term = self.denotation.pair(f, term);
        }
        term
    }

    fn finish(self, term: TermHandle) -> BoundedDenotation {
        BoundedDenotation {
            arena: self.denotation.arena,
            certificate: MathematicalCertificate {
                signature: self.denotation.declarations,
                level_arity: 0,
                context: self.context_bindings,
                term,
                expected: self.expected,
            },
            rules: self.rules.into_iter().collect(),
            assumptions: self.assumptions.premises,
            semantic_axioms: self.semantic_axioms.premises,
        }
    }
}

#[cfg(test)]
pub(crate) mod test_utilities {
    use mathematical_kernel::{Budget, DEFAULT_CONVERSION_STEPS, verify_mathematical_certificate};
    use semantic_vocabulary::{Proposition, PropositionContext};

    use super::{BoundedDenotation, verify_bounded_certificate};
    use crate::proof::ProofNode;

    pub(crate) fn verify_and_assert_forged(
        context: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        axioms: &[Proposition],
        proof: &ProofNode,
    ) -> BoundedDenotation {
        let mut denoted = verify_bounded_certificate(
            context,
            goal,
            assumptions,
            axioms,
            proof,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .expect("checked certificate must pass the kernel");
        let mut forged = denoted.certificate.clone();
        forged.expected = denoted.denotation_forged_falsehood();
        verify_mathematical_certificate(
            &mut denoted.arena,
            &forged,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        )
        .expect_err("forged expectation must be rejected");
        denoted
    }

    pub(crate) fn assert_refused(
        context: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        axioms: &[Proposition],
        proof: &ProofNode,
    ) {
        let result = verify_bounded_certificate(
            context,
            goal,
            assumptions,
            axioms,
            proof,
            &mut Budget::new(DEFAULT_CONVERSION_STEPS),
        );
        assert!(result.is_err(), "invalid certificate must be refused");
    }
}

#[cfg(test)]
impl BoundedDenotation {
    fn denotation_forged_falsehood(&mut self) -> TermHandle {
        let two = self.arena.insert(Term::Two);
        let z = self.arena.insert(Term::TwoZero);
        let o = self.arena.insert(Term::TwoOne);
        self.arena.insert(Term::Id {
            ty: two,
            left: z,
            right: o,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, IntegerAffineWitness,
        IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness,
        forge_term_and_assert_rejected,
    };
    use mathematical_kernel::Term;
    use semantic_vocabulary::{
        ContentConservation, ContentDomainId, ContentPlaceSegment, ContentPlaceVersion,
        ContentProjectionIdentity, ContentStructuralPlace, PlaceId, StructuralPlaceKind,
    };
    use terminal_psi::PrimitiveJudgment;

    fn int_ty(sign: IntegerSign, bits: u16) -> IntegerType {
        IntegerType::new(sign, bits).unwrap()
    }
    fn u8_ty() -> IntegerType {
        int_ty(IntegerSign::Unsigned, 8)
    }
    fn u16_ty() -> IntegerType {
        int_ty(IntegerSign::Unsigned, 16)
    }
    fn u32_ty() -> IntegerType {
        int_ty(IntegerSign::Unsigned, 32)
    }
    fn u64_ty() -> IntegerType {
        int_ty(IntegerSign::Unsigned, 64)
    }
    fn i1_ty() -> IntegerType {
        int_ty(IntegerSign::Signed, 1)
    }
    fn i2_ty() -> IntegerType {
        int_ty(IntegerSign::Signed, 2)
    }
    fn i8_ty() -> IntegerType {
        int_ty(IntegerSign::Signed, 8)
    }
    fn i16_ty() -> IntegerType {
        int_ty(IntegerSign::Signed, 16)
    }
    fn i32_ty() -> IntegerType {
        int_ty(IntegerSign::Signed, 32)
    }
    fn i128_ty() -> IntegerType {
        int_ty(IntegerSign::Signed, 128)
    }

    fn budget() -> Budget {
        Budget::new(DEFAULT_CONVERSION_STEPS)
    }
    fn vid(n: u32) -> ValueId {
        ValueId::new(u64::from(n) + 1).unwrap()
    }
    fn val(n: u32, ty: IntegerType) -> ScalarTerm {
        ScalarTerm::value(vid(n), ScalarType::Integer(ty))
    }
    fn bval(n: u32) -> ScalarTerm {
        ScalarTerm::value(vid(n), ScalarType::Boolean)
    }
    fn u(ty: IntegerType, v: u128) -> ScalarTerm {
        ScalarTerm::integer(ty, IntegerValue::Unsigned(v)).unwrap()
    }
    fn s(ty: IntegerType, v: i128) -> ScalarTerm {
        ScalarTerm::integer(ty, IntegerValue::Signed(v)).unwrap()
    }
    fn mlit_u(v: u128) -> IntegerMathTerm {
        IntegerMathTerm::literal(IntegerValue::Unsigned(v))
    }
    fn mlit_s(v: i128) -> IntegerMathTerm {
        IntegerMathTerm::literal(IntegerValue::Signed(v))
    }
    fn mvar(n: u32, ty: IntegerType) -> IntegerMathTerm {
        IntegerMathTerm::MathValue {
            source_type: ty,
            value: vid(n),
        }
    }
    fn ctx_of(pairs: &[(u32, ScalarType)]) -> PropositionContext {
        PropositionContext::from_value_types(pairs.iter().map(|&(id, ty)| (vid(id), ty))).unwrap()
    }
    fn ctx_int(ids: &[u32], ty: IntegerType) -> PropositionContext {
        ctx_of(
            &ids.iter()
                .map(|&id| (id, ScalarType::Integer(ty)))
                .collect::<Vec<_>>(),
        )
    }
    fn ctx_bool(ids: &[u32]) -> PropositionContext {
        ctx_of(
            &ids.iter()
                .map(|&id| (id, ScalarType::Boolean))
                .collect::<Vec<_>>(),
        )
    }
    fn node(conclusion: Proposition, rule: ProofRule) -> ProofNode {
        ProofNode { conclusion, rule }
    }
    fn assume(index: usize, prop: Proposition) -> ProofNode {
        node(prop, ProofRule::Assumption { index })
    }
    fn sax(index: usize, prop: Proposition) -> ProofNode {
        node(prop, ProofRule::SemanticAxiom { index })
    }
    fn prim(j: PrimitiveJudgment, prop: Proposition) -> ProofNode {
        node(prop, ProofRule::Primitive(j))
    }
    fn verify_ok(
        ctx: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        axioms: &[Proposition],
        proof: &ProofNode,
    ) -> BoundedDenotation {
        verify_bounded_certificate(ctx, goal, assumptions, axioms, proof, &mut budget())
            .expect("valid certificate")
    }
    fn verify_and_forge(
        ctx: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        axioms: &[Proposition],
        proof: &ProofNode,
    ) -> BoundedDenotation {
        let d = test_utilities::verify_and_assert_forged(ctx, goal, assumptions, axioms, proof);
        let mut copy = BoundedDenotation {
            arena: d.arena.clone(),
            certificate: d.certificate.clone(),
            rules: d.rules.clone(),
            assumptions: d.assumptions.clone(),
            semantic_axioms: d.semantic_axioms.clone(),
        };
        let _ = forge_term_and_assert_rejected(&mut copy);
        d
    }
    fn expect_err(
        ctx: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        axioms: &[Proposition],
        proof: &ProofNode,
    ) {
        test_utilities::assert_refused(ctx, goal, assumptions, axioms, proof);
    }
    fn add_s(ty: IntegerType, l: ScalarTerm, r: ScalarTerm) -> ScalarTerm {
        ScalarTerm::ExactIntegerAdd {
            scalar_type: ty,
            left: Box::new(l),
            right: Box::new(r),
        }
    }
    fn sub_s(ty: IntegerType, l: ScalarTerm, r: ScalarTerm) -> ScalarTerm {
        ScalarTerm::ExactIntegerSubtract {
            scalar_type: ty,
            left: Box::new(l),
            right: Box::new(r),
        }
    }
    fn mul_s(ty: IntegerType, l: ScalarTerm, r: ScalarTerm) -> ScalarTerm {
        ScalarTerm::ExactIntegerMultiply {
            scalar_type: ty,
            left: Box::new(l),
            right: Box::new(r),
        }
    }
    fn div_s(ty: IntegerType, l: ScalarTerm, r: ScalarTerm) -> ScalarTerm {
        ScalarTerm::ExactIntegerDivide {
            scalar_type: ty,
            left: Box::new(l),
            right: Box::new(r),
        }
    }
    fn bnot(x: ScalarTerm) -> ScalarTerm {
        ScalarTerm::BooleanNot {
            operand: Box::new(x),
        }
    }
    fn beq(l: ScalarTerm, r: ScalarTerm) -> ScalarTerm {
        ScalarTerm::BooleanEqual {
            left: Box::new(l),
            right: Box::new(r),
        }
    }
    fn le(l: ScalarTerm, r: ScalarTerm) -> Proposition {
        Proposition::LessOrEqual(l, r)
    }
    fn lt(l: ScalarTerm, r: ScalarTerm) -> Proposition {
        Proposition::LessThan(l, r)
    }
    fn eq(l: ScalarTerm, r: ScalarTerm) -> Proposition {
        Proposition::Equal(l, r)
    }
    fn mle(l: IntegerMathTerm, r: IntegerMathTerm) -> Proposition {
        Proposition::IntegerMathLessOrEqual(l, r)
    }
    fn mlt(l: IntegerMathTerm, r: IntegerMathTerm) -> Proposition {
        Proposition::IntegerMathLessThan(l, r)
    }
    fn meq(l: IntegerMathTerm, r: IntegerMathTerm) -> Proposition {
        Proposition::IntegerMathEqual(l, r)
    }
    fn imp(p: Proposition, c: Proposition) -> Proposition {
        Proposition::Implication {
            premise: Box::new(p),
            conclusion: Box::new(c),
        }
    }

    fn assert_rechecks_and_rejects_forge(d: &BoundedDenotation) {
        let mut cert = d.certificate.clone();
        let mut arena = d.arena.clone();
        verify_mathematical_certificate(&mut arena, &cert, &mut budget()).expect("re-verifies");
        let two = arena.insert(Term::Two);
        let z = arena.insert(Term::TwoZero);
        let o = arena.insert(Term::TwoOne);
        cert.expected = arena.insert(Term::Id {
            ty: two,
            left: z,
            right: o,
        });
        assert!(verify_mathematical_certificate(&mut arena, &cert, &mut budget()).is_err());
    }

    // ------------------------------------------------------------------------
    // 1. Core closures, contracts, equality/order/disjunction/semantic axioms
    // ------------------------------------------------------------------------

    #[test]
    fn exact_closures_contracts_equality_order_and_primitive_decisions() {
        let u8 = u8_ty();
        // Contract entailment discharge shape: signature.len() == 4, closure == {0..=3}
        let ctx = ctx_int(&[0, 1], u8);
        let (x, y) = (val(0, u8), val(1, u8));
        let p = le(x.clone(), y.clone());
        let d = verify_and_forge(
            &ctx,
            &p,
            std::slice::from_ref(&p),
            &[],
            &assume(0, p.clone()),
        );
        assert_eq!(d.certificate.signature.len(), 4);
        assert!(d.certificate.signature[0].is_assumption());
        assert_eq!(d.certificate.context.len(), 1);
        assert_eq!(
            certificate_assumption_closure(&d.arena, &d.certificate),
            BTreeSet::from([0, 1, 2, 3])
        );
        assert_eq!(d.rules, vec![AcceptedProofRule::Assumption]);

        // Discharged hypotheses stay inside the evidence term (ImplicationIntroduction)
        let imp_goal = imp(p.clone(), p.clone());
        let imp_proof = node(
            imp_goal.clone(),
            ProofRule::ImplicationIntroduction {
                body: Box::new(assume(0, p.clone())),
            },
        );
        let d_imp = verify_and_forge(&ctx, &imp_goal, &[], &[], &imp_proof);
        assert!(d_imp.certificate.context.is_empty());
        assert!(d_imp.assumptions.is_empty());

        // Order weakening named citation: signature.len() == 6, closure == {0..=5}
        let lt_xy = lt(x.clone(), y.clone());
        let wk = node(
            p.clone(),
            ProofRule::IntegerOrderWeakening {
                relation: Box::new(assume(0, lt_xy.clone())),
            },
        );
        let d_wk = verify_and_forge(&ctx, &p, std::slice::from_ref(&lt_xy), &[], &wk);
        assert_eq!(d_wk.certificate.signature.len(), 6);
        assert_eq!(
            certificate_assumption_closure(&d_wk.arena, &d_wk.certificate),
            BTreeSet::from([0, 1, 2, 3, 4, 5])
        );

        // Closed equality is Refl in the kernel: signature.len() == 6, closure == {0,1,2,4}
        let sum_2_0 =
            ScalarTerm::wrapping_integer_add(u64_ty(), u(u64_ty(), 2), u(u64_ty(), 0)).unwrap();
        let eq_1 = eq(sum_2_0, u(u64_ty(), 2));
        let d_eq = verify_and_forge(
            &PropositionContext::default(),
            &eq_1,
            &[],
            &[],
            &prim(PrimitiveJudgment::ClosedIntegerRelation, eq_1.clone()),
        );
        assert_eq!(d_eq.certificate.signature.len(), 6);
        assert!(d_eq.certificate.context.is_empty());
        assert!(matches!(
            d_eq.arena.get(d_eq.certificate.term),
            Term::Refl { .. }
        ));
        assert_eq!(
            certificate_assumption_closure(&d_eq.arena, &d_eq.certificate),
            BTreeSet::from([0, 1, 2, 4])
        );

        // Closed order uses only the fixed numeral law closure: signature.len() == 14, closure == {0,1,2,4,6,8,10,12,13}
        let lt_1_2 = lt(u(u8, 1), u(u8, 2));
        let mut d_ord = verify_and_forge(
            &PropositionContext::default(),
            &lt_1_2,
            &[],
            &[],
            &prim(PrimitiveJudgment::ClosedIntegerRelation, lt_1_2.clone()),
        );
        assert_eq!(d_ord.certificate.signature.len(), 14);
        assert_eq!(
            certificate_assumption_closure(&d_ord.arena, &d_ord.certificate),
            BTreeSet::from([0, 1, 2, 4, 6, 8, 10, 12, 13])
        );
        let mut missing_def = d_ord.certificate.clone();
        missing_def.signature[3].body = None;
        assert!(
            verify_mathematical_certificate(&mut d_ord.arena, &missing_def, &mut budget()).is_err()
        );
        let mut tampered = d_ord.certificate.clone();
        let zero = d_ord.arena.insert(Term::Constant {
            declaration: 2,
            levels: Vec::new(),
        });
        tampered.signature[3].body = Some(zero);
        assert!(
            verify_mathematical_certificate(&mut d_ord.arena, &tampered, &mut budget()).is_err()
        );

        // Truth has empty signature and empty closure
        let d_t = verify_ok(
            &PropositionContext::default(),
            &Proposition::Truth,
            &[],
            &[],
            &prim(PrimitiveJudgment::Truth, Proposition::Truth),
        );
        assert!(d_t.certificate.signature.is_empty());
        assert!(certificate_assumption_closure(&d_t.arena, &d_t.certificate).is_empty());

        // Closed primitive false order rejected before derivation
        let false_lt = lt(u(u8, 5), u(u8, 2));
        expect_err(
            &PropositionContext::default(),
            &false_lt,
            &[],
            &[],
            &prim(PrimitiveJudgment::ClosedIntegerRelation, false_lt.clone()),
        );
    }

    #[test]
    fn equality_symmetry_transitivity_order_substitution_and_semantic_axioms() {
        let u8 = u8_ty();
        let ctx = ctx_int(&[0, 1, 2], u8);
        let (x, y, z) = (val(0, u8), val(1, u8), val(2, u8));
        let (exy, eyz, exz, eyx) = (
            eq(x.clone(), y.clone()),
            eq(y.clone(), z.clone()),
            eq(x.clone(), z.clone()),
            eq(y.clone(), x.clone()),
        );
        let sym = node(
            eyx.clone(),
            ProofRule::EqualitySymmetry {
                equality: Box::new(assume(0, exy.clone())),
            },
        );
        let d_sym = verify_and_forge(&ctx, &eyx, std::slice::from_ref(&exy), &[], &sym);
        assert_eq!(d_sym.certificate.signature.len(), 3);
        assert_eq!(d_sym.arena.get(d_sym.certificate.term), Term::Variable(0));
        assert_eq!(
            certificate_assumption_closure(&d_sym.arena, &d_sym.certificate),
            BTreeSet::from([0, 1, 2])
        );

        // Non-canonicalized Boolean equality symmetry becomes IdElim
        let bctx = ctx_bool(&[0, 1]);
        let (eb01, eb10) = (eq(bval(0), bval(1)), eq(bval(1), bval(0)));
        let bsym = node(
            eb10.clone(),
            ProofRule::EqualitySymmetry {
                equality: Box::new(assume(0, eb01.clone())),
            },
        );
        let d_bsym = verify_and_forge(&bctx, &eb10, std::slice::from_ref(&eb01), &[], &bsym);
        assert!(matches!(
            d_bsym.arena.get(d_bsym.certificate.term),
            Term::IdElim { .. }
        ));

        let tr = node(
            exz.clone(),
            ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(assume(0, exy.clone())),
                middle_equals_right: Box::new(assume(1, eyz.clone())),
            },
        );
        let d_tr = verify_and_forge(&ctx, &exz, &[exy.clone(), eyz.clone()], &[], &tr);
        assert!(matches!(
            d_tr.arena.get(d_tr.certificate.term),
            Term::IdElim { .. }
        ));

        // Integer math transitivity becomes IdElim
        let (mxy, myz, mxz) = (
            meq(mvar(0, u8), mvar(1, u8)),
            meq(mvar(1, u8), mvar(2, u8)),
            meq(mvar(0, u8), mvar(2, u8)),
        );
        let mtr = node(
            mxz.clone(),
            ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(assume(0, mxy.clone())),
                middle_equals_right: Box::new(assume(1, myz.clone())),
            },
        );
        let d_mtr = verify_and_forge(&ctx, &mxz, &[mxy.clone(), myz.clone()], &[], &mtr);
        assert!(matches!(
            d_mtr.arena.get(d_mtr.certificate.term),
            Term::IdElim { .. }
        ));

        // Fixed <-> math equality crossing in both directions
        let d_fm = verify_and_forge(
            &ctx,
            &mxy,
            std::slice::from_ref(&exy),
            &[],
            &assume(0, mxy.clone()),
        );
        assert_eq!(d_fm.arena.get(d_fm.certificate.term), Term::Variable(0));
        let d_mf = verify_and_forge(
            &ctx,
            &exy,
            std::slice::from_ref(&mxy),
            &[],
            &assume(0, exy.clone()),
        );
        assert_eq!(d_mf.arena.get(d_mf.certificate.term), Term::Variable(0));

        // Order substitution citation + mismatch rejection
        let (lxz, lyz) = (lt(x.clone(), z.clone()), lt(y.clone(), z.clone()));
        let subst = node(
            lyz.clone(),
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(assume(0, lxz.clone())),
                equality: Box::new(assume(1, exy.clone())),
                endpoint: 0,
            },
        );
        let d_sub = verify_and_forge(&ctx, &lyz, &[lxz.clone(), exy.clone()], &[], &subst);
        assert_rechecks_and_rejects_forge(&d_sub);
        let bad_subst = node(
            lyz.clone(),
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(assume(0, lxz.clone())),
                equality: Box::new(assume(1, exy.clone())),
                endpoint: 1,
            },
        );
        expect_err(&ctx, &lyz, &[lxz.clone(), exy.clone()], &[], &bad_subst);

        // Semantic axioms bind after assumption roster
        let ax_node = sax(0, eyz.clone());
        let d_ax = verify_and_forge(
            &ctx,
            &eyz,
            std::slice::from_ref(&exy),
            std::slice::from_ref(&eyz),
            &ax_node,
        );
        assert_eq!(d_ax.certificate.context.len(), 2);
        assert_eq!(d_ax.semantic_axioms.len(), 1);
        assert!(d_ax.assumptions.is_empty());

        // Wrong goal rejected
        expect_err(
            &ctx,
            &exz,
            std::slice::from_ref(&exy),
            &[],
            &assume(0, exy.clone()),
        );
    }

    // ------------------------------------------------------------------------
    // 2. Binary numerals & carry/signed-zero closures
    // ------------------------------------------------------------------------

    #[test]
    fn binary_carry_signed_zero_closures_and_large_opaque_values() {
        let i8 = i8_ty();
        for (lo, hi) in [(0i128, 1i128), (2, 3), (7, 8), (-2, -1), (-1, 0)] {
            for upper_closed in [false, true] {
                let ctx = ctx_int(&[0], i8);
                let x = val(0, i8);
                let (prem, goal) = if upper_closed {
                    (le(x.clone(), s(i8, lo)), lt(x.clone(), s(i8, hi)))
                } else {
                    (le(s(i8, hi), x.clone()), lt(s(i8, lo), x.clone()))
                };
                let proof = node(
                    goal.clone(),
                    ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(assume(0, prem.clone())),
                    },
                );
                let d = verify_and_forge(&ctx, &goal, std::slice::from_ref(&prem), &[], &proof);
                assert_rechecks_and_rejects_forge(&d);
            }
        }
        // Large closed values (>128-bit) stay exact opaque identity without new capacity limit
        let mut bigs = [
            IntegerMathTerm::ShiftLeft {
                value: Box::new(mlit_u(1)),
                count: Box::new(mlit_u(65535)),
            },
            IntegerMathTerm::ShiftLeft {
                value: Box::new(mlit_u(2)),
                count: Box::new(mlit_u(65534)),
            },
        ];
        bigs.sort();
        let goal = meq(bigs[0].clone(), bigs[1].clone());
        let d = verify_and_forge(
            &PropositionContext::default(),
            &goal,
            &[],
            &[],
            &prim(PrimitiveJudgment::ClosedIntegerRelation, goal.clone()),
        );
        assert_eq!(d.certificate.signature.len(), 2);
        assert!(matches!(d.arena.get(d.certificate.term), Term::Refl { .. }));
    }

    // ------------------------------------------------------------------------
    // 3. Booleans: truth tables, case analysis, linear size, non-boolean skip
    // ------------------------------------------------------------------------

    #[test]
    fn boolean_truth_tables_case_analysis_transport_and_linear_growth() {
        let u8 = u8_ty();
        // Closed truth tables: empty signature + wrong-value rejection
        for a in [false, true] {
            let not_a = eq(bnot(ScalarTerm::Boolean(a)), ScalarTerm::Boolean(!a));
            let d = verify_and_forge(
                &PropositionContext::default(),
                &not_a,
                &[],
                &[],
                &node(
                    not_a.clone(),
                    ProofRule::PredicateDenotation {
                        premise: Box::new(prim(PrimitiveJudgment::Truth, Proposition::Truth)),
                    },
                ),
            );
            assert!(d.certificate.signature.is_empty());
            let wrong = eq(bnot(ScalarTerm::Boolean(a)), ScalarTerm::Boolean(a));
            expect_err(
                &PropositionContext::default(),
                &wrong,
                &[],
                &[],
                &node(
                    wrong.clone(),
                    ProofRule::PredicateDenotation {
                        premise: Box::new(prim(PrimitiveJudgment::Truth, Proposition::Truth)),
                    },
                ),
            );
            for b in [false, true] {
                let eq_ab = eq(
                    beq(ScalarTerm::Boolean(a), ScalarTerm::Boolean(b)),
                    ScalarTerm::Boolean(a == b),
                );
                let d2 = verify_ok(
                    &PropositionContext::default(),
                    &eq_ab,
                    &[],
                    &[],
                    &node(
                        eq_ab.clone(),
                        ProofRule::PredicateDenotation {
                            premise: Box::new(prim(PrimitiveJudgment::Truth, Proposition::Truth)),
                        },
                    ),
                );
                assert!(d2.certificate.signature.is_empty());
            }
        }
        // Symbolic boolean distinctness & closure {0, 1}
        let ctx = ctx_bool(&[0, 1]);
        let (b, c) = (bval(0), bval(1));
        let eq_bc = eq(b.clone(), c.clone());
        let d_sym = verify_and_forge(
            &ctx,
            &eq_bc,
            std::slice::from_ref(&eq_bc),
            &[],
            &assume(0, eq_bc.clone()),
        );
        assert_eq!(
            certificate_assumption_closure(&d_sym.arena, &d_sym.certificate),
            BTreeSet::from([0, 1])
        );

        // Nested boolean equality keeps linear arena size <= 6*d + 32
        for depth in [8usize, 32] {
            let mut term = b.clone();
            for _ in 0..depth {
                term = beq(term, ScalarTerm::Boolean(true));
            }
            let mut den = Denotation::new();
            den.scalar_term(&term).unwrap();
            assert!(
                den.arena.len() <= 6 * depth + 32,
                "arena {} > {}",
                den.arena.len(),
                6 * depth + 32
            );
        }

        // Double negation, (b==c) => (b==c)==true, crossed negations, nested in implication domain
        let cases: Vec<(Proposition, Proposition)> = vec![
            (
                eq(bnot(bnot(b.clone())), ScalarTerm::Boolean(true)),
                eq(b.clone(), ScalarTerm::Boolean(true)),
            ),
            (
                eq_bc.clone(),
                eq(beq(b.clone(), c.clone()), ScalarTerm::Boolean(true)),
            ),
            (
                eq(bnot(b.clone()), c.clone()),
                eq(b.clone(), bnot(c.clone())),
            ),
            (
                imp(
                    eq(bnot(bnot(b.clone())), ScalarTerm::Boolean(true)),
                    eq(c.clone(), ScalarTerm::Boolean(true)),
                ),
                imp(
                    eq(b.clone(), ScalarTerm::Boolean(true)),
                    eq(c.clone(), ScalarTerm::Boolean(true)),
                ),
            ),
        ];
        for (prem, goal) in cases {
            let proof = node(
                goal.clone(),
                ProofRule::PredicateDenotation {
                    premise: Box::new(assume(0, prem.clone())),
                },
            );
            let d = verify_and_forge(&ctx, &goal, std::slice::from_ref(&prem), &[], &proof);
            assert_rechecks_and_rejects_forge(&d);
        }

        // Value equality transport through boolean endpoints
        let p_true = eq(b.clone(), ScalarTerm::Boolean(true));
        let g_true = eq(c.clone(), ScalarTerm::Boolean(true));
        let vet = node(
            g_true.clone(),
            ProofRule::ValueEqualityTransport {
                premise: Box::new(assume(0, p_true.clone())),
                equalities: vec![assume(1, eq_bc.clone())],
            },
        );
        verify_and_forge(&ctx, &g_true, &[p_true.clone(), eq_bc.clone()], &[], &vet);

        // Boolean transport declines uncovered open leaf and skips non-boolean neutrals
        let mut den = Denotation::new();
        let (b_true, c_true, ev) = (
            den.denote(&p_true).unwrap(),
            den.denote(&g_true).unwrap(),
            den.var(0),
        );
        assert!(
            den.boolean_goal_transport(b_true, c_true, ev, true)
                .unwrap()
                .is_none()
        );
        let int_lt = eq(
            ScalarTerm::IntegerLessThan {
                scalar_type: u8,
                left: Box::new(val(0, u8)),
                right: Box::new(val(1, u8)),
            },
            ScalarTerm::Boolean(true),
        );
        let ilt_t = den.denote(&int_lt).unwrap();
        assert!(
            den.boolean_goal_transport(ilt_t, ilt_t, ev, false)
                .unwrap()
                .is_some()
        );
    }

    // ------------------------------------------------------------------------
    // 4. Casts, policy/carrier distinctions, order translation & antisymmetry
    // ------------------------------------------------------------------------

    #[test]
    fn casts_policy_distinctions_order_translation_and_antisymmetry() {
        let (u8, u16, u32, u64, i8, i16, i32) = (
            u8_ty(),
            u16_ty(),
            u32_ty(),
            u64_ty(),
            i8_ty(),
            i16_ty(),
            i32_ty(),
        );
        // Single exact cast edge i16 -> i8
        let ctx = ctx_of(&[(0, ScalarType::Integer(i16)), (1, ScalarType::Integer(i8))]);
        let (x16, x8) = (val(0, i16), val(1, i8));
        let def_cast = eq(
            x8.clone(),
            ScalarTerm::IntegerExactCast {
                source_type: i16,
                target_type: i8,
                operand: Box::new(x16.clone()),
            },
        );
        let (prem, goal) = (le(s(i16, -5), x16.clone()), le(s(i8, -5), x8.clone()));
        let wit = IntegerCastChainWitness {
            root: x16.clone(),
            target: x8.clone(),
            definition_axioms: vec![0],
        };
        let proof = node(
            goal.clone(),
            ProofRule::IntegerCastBound {
                root_bound: Box::new(assume(0, prem.clone())),
                witness: wit,
            },
        );
        let d = verify_and_forge(
            &ctx,
            &goal,
            std::slice::from_ref(&prem),
            std::slice::from_ref(&def_cast),
            &proof,
        );
        assert_rechecks_and_rejects_forge(&d);
        expect_err(&ctx, &goal, std::slice::from_ref(&prem), &[], &proof);

        // Multi-edge chain i16 -> u16 -> i8 and Truth-root u8 -> u64 carrier membership
        let ctx_m = ctx_of(&[
            (0, ScalarType::Integer(i16)),
            (1, ScalarType::Integer(u16)),
            (2, ScalarType::Integer(i8)),
        ]);
        let (a, b, c) = (val(0, i16), val(1, u16), val(2, i8));
        let ax0 = eq(
            b.clone(),
            ScalarTerm::IntegerExactCast {
                source_type: i16,
                target_type: u16,
                operand: Box::new(a.clone()),
            },
        );
        let ax1 = eq(
            c.clone(),
            ScalarTerm::IntegerExactCast {
                source_type: u16,
                target_type: i8,
                operand: Box::new(b.clone()),
            },
        );
        let (p_hi, g_hi) = (le(a.clone(), s(i16, 40)), le(c.clone(), s(i8, 40)));
        let wit_m = IntegerCastChainWitness {
            root: a.clone(),
            target: c.clone(),
            definition_axioms: vec![0, 1],
        };
        verify_and_forge(
            &ctx_m,
            &g_hi,
            std::slice::from_ref(&p_hi),
            &[ax0, ax1],
            &node(
                g_hi.clone(),
                ProofRule::IntegerCastBound {
                    root_bound: Box::new(assume(0, p_hi.clone())),
                    witness: wit_m,
                },
            ),
        );

        let ctx_w = ctx_of(&[(0, ScalarType::Integer(u8)), (1, ScalarType::Integer(u64))]);
        let (w8, w64) = (val(0, u8), val(1, u64));
        let ax_w = eq(
            w64.clone(),
            ScalarTerm::IntegerWiden {
                source_type: u8,
                target_type: u64,
                operand: Box::new(w8.clone()),
            },
        );
        for g in [le(u(u64, 0), w64.clone()), le(w64.clone(), u(u64, 255))] {
            let wit_w = IntegerCastChainWitness {
                root: w8.clone(),
                target: w64.clone(),
                definition_axioms: vec![0],
            };
            let pr = node(
                g.clone(),
                ProofRule::IntegerCastBound {
                    root_bound: Box::new(prim(PrimitiveJudgment::Truth, Proposition::Truth)),
                    witness: wit_w,
                },
            );
            verify_and_forge(&ctx_w, &g, &[], std::slice::from_ref(&ax_w), &pr);
        }

        // Fixed scalar policy/carrier distinctions are not definitional equalities
        let mut den = Denotation::new();
        let (v0, v1) = (val(0, u8), val(1, u8));
        let wa = den
            .fixed_scalar_term(&ScalarTerm::WrappingIntegerAdd {
                scalar_type: u8,
                left: Box::new(v0.clone()),
                right: Box::new(v1.clone()),
            })
            .unwrap();
        let sa = den
            .fixed_scalar_term(&ScalarTerm::SaturatingIntegerAdd {
                scalar_type: u8,
                left: Box::new(v0.clone()),
                right: Box::new(v1.clone()),
            })
            .unwrap();
        assert!(!den.arena.structurally_equal(wa, sa));
        let w16 = den
            .fixed_scalar_term(&ScalarTerm::IntegerWiden {
                source_type: u8,
                target_type: u16,
                operand: Box::new(v0.clone()),
            })
            .unwrap();
        let w32 = den
            .fixed_scalar_term(&ScalarTerm::IntegerWiden {
                source_type: u8,
                target_type: u32,
                operand: Box::new(v0.clone()),
            })
            .unwrap();
        assert!(!den.arena.structurally_equal(w16, w32));
        let sh8 = den
            .fixed_scalar_term(&ScalarTerm::ExactIntegerShiftLeft {
                value_type: u8,
                count_type: u8,
                value: Box::new(v0.clone()),
                count: Box::new(v1.clone()),
            })
            .unwrap();
        let sh16 = den
            .fixed_scalar_term(&ScalarTerm::ExactIntegerShiftLeft {
                value_type: u8,
                count_type: u16,
                value: Box::new(v0.clone()),
                count: Box::new(val(2, u16)),
            })
            .unwrap();
        assert!(!den.arena.structurally_equal(sh8, sh16));

        // Order translation: open + closed numeral + rejections + 16384-deep addend rejection
        let ctx_t = ctx_int(&[0, 1, 2], i32);
        let (ma, mb, mc) = (mvar(0, i32), mvar(1, i32), mvar(2, i32));
        let prem_t = mle(ma.clone(), mb.clone());
        let goal_t = mle(
            IntegerMathTerm::Add(Box::new(ma.clone()), Box::new(mc.clone())),
            IntegerMathTerm::Add(Box::new(mb.clone()), Box::new(mc.clone())),
        );
        let tr_p = node(
            goal_t.clone(),
            ProofRule::IntegerMathOrderTranslation {
                relation: Box::new(assume(0, prem_t.clone())),
                addend: mc.clone(),
            },
        );
        let d_tr = verify_and_forge(&ctx_t, &goal_t, std::slice::from_ref(&prem_t), &[], &tr_p);
        assert_rechecks_and_rejects_forge(&d_tr);
        let (closed_p, closed_g) = (
            mle(mlit_s(-4), mlit_s(2)),
            mle(
                IntegerMathTerm::Add(Box::new(mlit_s(-4)), Box::new(mlit_s(-3))),
                IntegerMathTerm::Add(Box::new(mlit_s(2)), Box::new(mlit_s(-3))),
            ),
        );
        verify_and_forge(
            &ctx_t,
            &closed_g,
            std::slice::from_ref(&closed_p),
            &[],
            &node(
                closed_g.clone(),
                ProofRule::IntegerMathOrderTranslation {
                    relation: Box::new(assume(0, closed_p.clone())),
                    addend: mlit_s(-3),
                },
            ),
        );
        let bad_strict = mlt(
            IntegerMathTerm::Add(Box::new(mlit_s(-4)), Box::new(mlit_s(-3))),
            IntegerMathTerm::Add(Box::new(mlit_s(2)), Box::new(mlit_s(-3))),
        );
        expect_err(
            &ctx_t,
            &bad_strict,
            std::slice::from_ref(&closed_p),
            &[],
            &node(
                bad_strict.clone(),
                ProofRule::IntegerMathOrderTranslation {
                    relation: Box::new(assume(0, closed_p.clone())),
                    addend: mlit_s(-3),
                },
            ),
        );
        let mut deep_add = mlit_s(1);
        for _ in 0..16_384 {
            deep_add = IntegerMathTerm::Add(Box::new(deep_add), Box::new(mlit_s(1)));
        }
        expect_err(
            &ctx_t,
            &goal_t,
            std::slice::from_ref(&prem_t),
            &[],
            &node(
                goal_t.clone(),
                ProofRule::IntegerMathOrderTranslation {
                    relation: Box::new(assume(0, prem_t.clone())),
                    addend: deep_add,
                },
            ),
        );

        // Order antisymmetry (signed + unsigned + literal + reversed + rejections)
        for ty in [u8, i32] {
            let c = ctx_int(&[0, 1], ty);
            let (p0, p1) = (val(0, ty), val(1, ty));
            let (fwd, bwd, eq_g, eq_rev) = (
                le(p0.clone(), p1.clone()),
                le(p1.clone(), p0.clone()),
                eq(p0.clone(), p1.clone()),
                eq(p1.clone(), p0.clone()),
            );
            verify_and_forge(
                &c,
                &eq_g,
                &[fwd.clone(), bwd.clone()],
                &[],
                &node(
                    eq_g.clone(),
                    ProofRule::IntegerOrderAntisymmetry {
                        forward: Box::new(assume(0, fwd.clone())),
                        backward: Box::new(assume(1, bwd.clone())),
                    },
                ),
            );
            verify_and_forge(
                &c,
                &eq_rev,
                &[fwd.clone(), bwd.clone()],
                &[],
                &node(
                    eq_rev.clone(),
                    ProofRule::IntegerOrderAntisymmetry {
                        forward: Box::new(assume(1, bwd.clone())),
                        backward: Box::new(assume(0, fwd.clone())),
                    },
                ),
            );
            expect_err(
                &c,
                &eq_rev,
                &[fwd.clone(), bwd.clone()],
                &[],
                &node(
                    eq_rev.clone(),
                    ProofRule::IntegerOrderAntisymmetry {
                        forward: Box::new(assume(0, fwd.clone())),
                        backward: Box::new(assume(1, bwd.clone())),
                    },
                ),
            );
            expect_err(
                &c,
                &eq_g,
                &[lt(p0.clone(), p1.clone()), bwd.clone()],
                &[],
                &node(
                    eq_g.clone(),
                    ProofRule::IntegerOrderAntisymmetry {
                        forward: Box::new(assume(0, lt(p0.clone(), p1.clone()))),
                        backward: Box::new(assume(1, bwd.clone())),
                    },
                ),
            );
        }
    }

    #[test]
    fn two_mathematical_orders_certify_equality_of_their_exact_endpoints() {
        let carrier = u64_ty();
        let at = |id| IntegerMathTerm::embed(val(id, carrier)).unwrap();
        let a = at(2);
        let b = IntegerMathTerm::Add(Box::new(at(0)), Box::new(at(1)));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let premises = [
            Proposition::IntegerMathLessOrEqual(a.clone(), b.clone()),
            Proposition::IntegerMathLessOrEqual(b.clone(), a.clone()),
        ];
        let goal = Proposition::IntegerMathEqual(a, b);
        let proof = node(
            goal.clone(),
            ProofRule::IntegerOrderAntisymmetry {
                forward: Box::new(assume(0, premises[0].clone())),
                backward: Box::new(assume(1, premises[1].clone())),
            },
        );
        let context = ctx_int(&[0, 1, 2], carrier);
        verify_and_forge(&context, &goal, &premises, &[], &proof);
        expect_err(&context, &goal, &premises[..1], &[], &proof);
        let wrong = Proposition::IntegerMathEqual(at(0), at(2));
        expect_err(
            &context,
            &wrong,
            &premises,
            &[],
            &node(wrong.clone(), proof.rule),
        );
    }

    // ------------------------------------------------------------------------
    // 5. Addition, Subtraction, Multiplication & Correlated Forbidden Roots
    // ------------------------------------------------------------------------

    #[test]
    fn addition_subtraction_and_multiplication_bounds() {
        let (u8, u64, i1, i2, i32, i128) =
            (u8_ty(), u64_ty(), i1_ty(), i2_ty(), i32_ty(), i128_ty());
        // Exact add & subtract definition bounds across u8/u64/i32/i128 (lower/upper/reversed) + rejections
        for ty in [u8, u64, i32, i128] {
            let c = ctx_int(&[0, 1, 2], ty);
            let (x, y, out) = (val(0, ty), val(1, ty), val(2, ty));
            let (a_lo, b_lo, a_hi, b_hi) = if ty.sign() == IntegerSign::Unsigned {
                (u(ty, 2), u(ty, 3), u(ty, 10), u(ty, 20))
            } else {
                (s(ty, -5), s(ty, -3), s(ty, 10), s(ty, 20))
            };
            let (sum_lo, sum_hi, diff_lo, diff_hi) = if ty.sign() == IntegerSign::Unsigned {
                (u(ty, 5), u(ty, 30), u(ty, 0), u(ty, 7))
            } else {
                (s(ty, -8), s(ty, 30), s(ty, -25), s(ty, 13))
            };
            for rev in [false, true] {
                let add_def = if rev {
                    eq(add_s(ty, x.clone(), y.clone()), out.clone())
                } else {
                    eq(out.clone(), add_s(ty, x.clone(), y.clone()))
                };
                let (pl, pr, gl) = (
                    le(a_lo.clone(), x.clone()),
                    le(b_lo.clone(), y.clone()),
                    le(sum_lo.clone(), out.clone()),
                );
                verify_and_forge(
                    &c,
                    &gl,
                    &[pl.clone(), pr.clone()],
                    std::slice::from_ref(&add_def),
                    &node(
                        gl.clone(),
                        ProofRule::IntegerExactAddDefinitionBound {
                            left_bound: Box::new(assume(0, pl)),
                            right_bound: Box::new(assume(1, pr)),
                            definition_axiom: 0,
                        },
                    ),
                );
                let (pu, qv, gu) = (
                    le(x.clone(), a_hi.clone()),
                    le(y.clone(), b_hi.clone()),
                    le(out.clone(), sum_hi.clone()),
                );
                verify_and_forge(
                    &c,
                    &gu,
                    &[pu.clone(), qv.clone()],
                    std::slice::from_ref(&add_def),
                    &node(
                        gu.clone(),
                        ProofRule::IntegerExactAddDefinitionBound {
                            left_bound: Box::new(assume(0, pu)),
                            right_bound: Box::new(assume(1, qv)),
                            definition_axiom: 0,
                        },
                    ),
                );

                let sub_def = if rev {
                    eq(sub_s(ty, x.clone(), y.clone()), out.clone())
                } else {
                    eq(out.clone(), sub_s(ty, x.clone(), y.clone()))
                };
                if ty.sign() == IntegerSign::Signed {
                    let (p1, p2, g1) = (
                        le(a_lo.clone(), x.clone()),
                        le(y.clone(), b_hi.clone()),
                        le(diff_lo.clone(), out.clone()),
                    );
                    verify_and_forge(
                        &c,
                        &g1,
                        &[p1.clone(), p2.clone()],
                        std::slice::from_ref(&sub_def),
                        &node(
                            g1.clone(),
                            ProofRule::IntegerExactSubtractDefinitionBound {
                                left_bound: Box::new(assume(0, p1)),
                                right_bound: Box::new(assume(1, p2)),
                                definition_axiom: 0,
                            },
                        ),
                    );
                    let (p3, p4, g2) = (
                        le(x.clone(), a_hi.clone()),
                        le(b_lo.clone(), y.clone()),
                        le(out.clone(), diff_hi.clone()),
                    );
                    verify_and_forge(
                        &c,
                        &g2,
                        &[p3.clone(), p4.clone()],
                        std::slice::from_ref(&sub_def),
                        &node(
                            g2.clone(),
                            ProofRule::IntegerExactSubtractDefinitionBound {
                                left_bound: Box::new(assume(0, p3)),
                                right_bound: Box::new(assume(1, p4)),
                                definition_axiom: 0,
                            },
                        ),
                    );
                }
            }
            let bad_def = eq(out.clone(), add_s(ty, x.clone(), y.clone()));
            let (pl, pr, bad_g) = (
                le(a_lo.clone(), x.clone()),
                le(b_lo.clone(), y.clone()),
                le(sum_hi.clone(), out.clone()),
            );
            expect_err(
                &c,
                &bad_g,
                &[pl.clone(), pr.clone()],
                std::slice::from_ref(&bad_def),
                &node(
                    bad_g.clone(),
                    ProofRule::IntegerExactAddDefinitionBound {
                        left_bound: Box::new(assume(0, pl)),
                        right_bound: Box::new(assume(1, pr)),
                        definition_axiom: 0,
                    },
                ),
            );
        }

        // Unsigned subtraction conditional domain (open + rejection)
        let cu = ctx_int(&[0, 1], u8);
        let (x, y) = (val(0, u8), val(1, u8));
        let prem = le(y.clone(), x.clone());
        let goal = mle(
            mlit_u(0),
            IntegerMathTerm::Subtract(Box::new(mvar(0, u8)), Box::new(mvar(1, u8))),
        );
        let wit = IntegerAffineWitness {
            root: y.clone(),
            target: sub_s(u8, x.clone(), y.clone()),
            definition_axioms: vec![],
            literal_axioms: vec![],
        };
        verify_and_forge(
            &cu,
            &goal,
            std::slice::from_ref(&prem),
            &[],
            &node(
                goal.clone(),
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assume(0, prem.clone())),
                    witness: wit.clone(),
                },
            ),
        );
        expect_err(
            &cu,
            &goal,
            std::slice::from_ref(&le(x.clone(), y.clone())),
            &[],
            &node(
                goal.clone(),
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assume(0, le(x.clone(), y.clone()))),
                    witness: wit,
                },
            ),
        );

        // Correlated add & subtract bounds with SSA root definitions
        let cs = ctx_int(&[0, 1, 2], i32);
        let (a, b, root) = (val(0, i32), val(1, i32), val(2, i32));
        let ax_sub = eq(
            root.clone(),
            sub_s(i32, s(i32, i32::MAX as i128), b.clone()),
        );
        let p_add = le(a.clone(), root.clone());
        let g_add = mle(
            IntegerMathTerm::Add(Box::new(mvar(0, i32)), Box::new(mvar(1, i32))),
            mlit_s(i32::MAX as i128),
        );
        let w_add = IntegerAffineWitness {
            root: root.clone(),
            target: add_s(i32, a.clone(), b.clone()),
            definition_axioms: vec![0],
            literal_axioms: vec![None],
        };
        verify_and_forge(
            &cs,
            &g_add,
            std::slice::from_ref(&p_add),
            std::slice::from_ref(&ax_sub),
            &node(
                g_add.clone(),
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assume(0, p_add.clone())),
                    witness: w_add,
                },
            ),
        );

        let ax_add = eq(
            root.clone(),
            add_s(i32, s(i32, i32::MIN as i128), b.clone()),
        );
        let p_sub = le(root.clone(), a.clone());
        let g_sub = mle(
            mlit_s(i32::MIN as i128),
            IntegerMathTerm::Subtract(Box::new(mvar(0, i32)), Box::new(mvar(1, i32))),
        );
        let w_sub = IntegerAffineWitness {
            root: root.clone(),
            target: sub_s(i32, a.clone(), b.clone()),
            definition_axioms: vec![0],
            literal_axioms: vec![None],
        };
        verify_and_forge(
            &cs,
            &g_sub,
            std::slice::from_ref(&p_sub),
            std::slice::from_ref(&ax_add),
            &node(
                g_sub.clone(),
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assume(0, p_sub.clone())),
                    witness: w_sub,
                },
            ),
        );

        // Correlated multiply bounds (positive max, i2 twin, i1 decline, mutated goal rejection)
        let ax_div_max = eq(
            root.clone(),
            div_s(i32, s(i32, i32::MAX as i128), b.clone()),
        );
        let p_mul =
            Proposition::Conjunction(vec![le(s(i32, 1), b.clone()), le(a.clone(), root.clone())]);
        let g_mul = mle(
            IntegerMathTerm::Multiply(Box::new(mvar(0, i32)), Box::new(mvar(1, i32))),
            mlit_s(i32::MAX as i128),
        );
        let w_mul = IntegerAffineWitness {
            root: root.clone(),
            target: mul_s(i32, a.clone(), b.clone()),
            definition_axioms: vec![0],
            literal_axioms: vec![None],
        };
        verify_and_forge(
            &cs,
            &g_mul,
            std::slice::from_ref(&p_mul),
            std::slice::from_ref(&ax_div_max),
            &node(
                g_mul.clone(),
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assume(0, p_mul.clone())),
                    witness: w_mul.clone(),
                },
            ),
        );
        let bad_g_mul = mle(
            IntegerMathTerm::Multiply(Box::new(mvar(0, i32)), Box::new(mvar(1, i32))),
            mlit_s(0),
        );
        expect_err(
            &cs,
            &bad_g_mul,
            std::slice::from_ref(&p_mul),
            std::slice::from_ref(&ax_div_max),
            &node(
                bad_g_mul.clone(),
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assume(0, p_mul.clone())),
                    witness: w_mul,
                },
            ),
        );

        let mut den_i1 = Denotation::new();
        let (a1, b1) = (val(0, i1), val(1, i1));
        let w_i1 = IntegerAffineWitness {
            root: div_s(i1, s(i1, 0), b1.clone()),
            target: mul_s(i1, a1, b1),
            definition_axioms: vec![],
            literal_axioms: vec![],
        };
        let g_i1 = mle(
            IntegerMathTerm::Multiply(Box::new(mvar(0, i1)), Box::new(mvar(1, i1))),
            mlit_s(0),
        );
        let ev_i1 = den_i1.var(0);
        assert!(
            den_i1
                .correlated_multiply_bound_evidence(&Proposition::Truth, ev_i1, &w_i1, &g_i1, &[])
                .unwrap()
                .is_none()
        );

        let ci2 = ctx_int(&[0, 1], i2);
        let (a2, b2) = (val(0, i2), val(1, i2));
        let root2 = div_s(i2, s(i2, 1), b2.clone());
        let p_i2 = Proposition::Conjunction(vec![
            le(s(i2, 1), b2.clone()),
            le(a2.clone(), root2.clone()),
        ]);
        let g_i2 = mle(
            IntegerMathTerm::Multiply(Box::new(mvar(0, i2)), Box::new(mvar(1, i2))),
            mlit_s(1),
        );
        let w_i2 = IntegerAffineWitness {
            root: root2,
            target: mul_s(i2, a2, b2),
            definition_axioms: vec![],
            literal_axioms: vec![],
        };
        verify_and_forge(
            &ci2,
            &g_i2,
            std::slice::from_ref(&p_i2),
            &[],
            &node(
                g_i2.clone(),
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assume(0, p_i2.clone())),
                    witness: w_i2,
                },
            ),
        );

        // IntegerSubtractOrder, IntegerAddOrder, IntegerSubtractAntitone + rejections
        let c4 = ctx_int(&[0, 1, 2, 3, 4], i32);
        let (v0, v1, v2, v3, v4) = (
            val(0, i32),
            val(1, i32),
            val(2, i32),
            val(3, i32),
            val(4, i32),
        );
        let (sub_eq, pos_v1, lt_sub) = (
            eq(v2.clone(), sub_s(i32, v0.clone(), v1.clone())),
            lt(s(i32, 0), v1.clone()),
            lt(v2.clone(), v0.clone()),
        );
        verify_and_forge(
            &c4,
            &lt_sub,
            &[sub_eq.clone(), pos_v1.clone()],
            &[],
            &node(
                lt_sub.clone(),
                ProofRule::IntegerSubtractOrder {
                    difference: Box::new(assume(0, sub_eq.clone())),
                    positive: Box::new(assume(1, pos_v1.clone())),
                },
            ),
        );
        expect_err(
            &c4,
            &lt(v0.clone(), v2.clone()),
            &[sub_eq, pos_v1.clone()],
            &[],
            &node(
                lt(v0.clone(), v2.clone()),
                ProofRule::IntegerSubtractOrder {
                    difference: Box::new(assume(
                        0,
                        eq(v2.clone(), sub_s(i32, v0.clone(), v1.clone())),
                    )),
                    positive: Box::new(assume(1, pos_v1.clone())),
                },
            ),
        );

        let (add_eq, lt_add) = (
            eq(v2.clone(), add_s(i32, v0.clone(), v1.clone())),
            lt(v0.clone(), v2.clone()),
        );
        verify_and_forge(
            &c4,
            &lt_add,
            &[add_eq.clone(), pos_v1.clone()],
            &[],
            &node(
                lt_add.clone(),
                ProofRule::IntegerAddOrder {
                    sum: Box::new(assume(0, add_eq.clone())),
                    positive: Box::new(assume(1, pos_v1.clone())),
                },
            ),
        );
        expect_err(
            &c4,
            &lt(v2.clone(), v0.clone()),
            &[add_eq, pos_v1],
            &[],
            &node(
                lt(v2.clone(), v0.clone()),
                ProofRule::IntegerAddOrder {
                    sum: Box::new(assume(
                        0,
                        eq(v2.clone(), add_s(i32, v0.clone(), v1.clone())),
                    )),
                    positive: Box::new(assume(1, lt(s(i32, 0), v1.clone()))),
                },
            ),
        );

        let (sm_eq, lg_eq, ord_v, anti_g) = (
            eq(v2.clone(), sub_s(i32, v0.clone(), v3.clone())),
            eq(v4.clone(), sub_s(i32, v0.clone(), v1.clone())),
            lt(v1.clone(), v3.clone()),
            lt(v2.clone(), v4.clone()),
        );
        verify_and_forge(
            &c4,
            &anti_g,
            &[sm_eq.clone(), lg_eq.clone(), ord_v.clone()],
            &[],
            &node(
                anti_g.clone(),
                ProofRule::IntegerSubtractAntitone {
                    smaller: Box::new(assume(0, sm_eq)),
                    larger: Box::new(assume(1, lg_eq)),
                    order: Box::new(assume(2, ord_v)),
                },
            ),
        );
    }

    #[test]
    fn correlated_forbidden_roots_positive_negative_out_of_interval_and_mutations() {
        let i8 = i8_ty();
        for (mul_k, add_c, lo, hi) in [
            (2i128, 1i128, -5i128, 5i128),
            (-2, 1, -5, 5),
            (1, 10, -5, 5),
        ] {
            let ctx5 = ctx_int(&[0, 1, 2, 3, 4], i8);
            let (r, d_mid, divd, divr, t_mul) =
                (val(0, i8), val(1, i8), val(2, i8), val(3, i8), val(4, i8));
            let axioms = vec![
                eq(d_mid.clone(), add_s(i8, r.clone(), s(i8, 5))),
                eq(divd.clone(), add_s(i8, d_mid.clone(), s(i8, 1))),
                eq(t_mul.clone(), mul_s(i8, r.clone(), s(i8, mul_k))),
                eq(divr.clone(), add_s(i8, t_mul.clone(), s(i8, add_c))),
            ];
            let (lo_p, hi_p) = (le(s(i8, lo), r.clone()), le(r.clone(), s(i8, hi)));
            let assumptions = vec![lo_p.clone(), hi_p.clone()];
            let goal = Proposition::Disjunction(vec![
                le(divr.clone(), s(i8, -2)),
                le(s(i8, 1), divr.clone()),
                Proposition::Conjunction(vec![
                    le(divr.clone(), s(i8, -1)),
                    le(s(i8, -127), divd.clone()),
                ]),
            ]);
            let witness = IntegerCorrelatedForbiddenRootWitness {
                dividend: CorrelatedAffineBranchWitness {
                    root: r.clone(),
                    target: divd.clone(),
                    steps: vec![
                        CorrelatedAffineStepWitness {
                            definition_axiom: 0,
                            literal_axiom: None,
                        },
                        CorrelatedAffineStepWitness {
                            definition_axiom: 1,
                            literal_axiom: None,
                        },
                    ],
                },
                divisor: CorrelatedAffineBranchWitness {
                    root: r.clone(),
                    target: divr.clone(),
                    steps: vec![
                        CorrelatedAffineStepWitness {
                            definition_axiom: 2,
                            literal_axiom: None,
                        },
                        CorrelatedAffineStepWitness {
                            definition_axiom: 3,
                            literal_axiom: None,
                        },
                    ],
                },
                definition_axiom_count: 4,
                lower_bound_axiom: 4,
                upper_bound_axiom: 5,
                conclusion: Proposition::Conjunction(vec![lo_p, hi_p]),
            };
            let params = BTreeSet::from([vid(0)]);
            let proof = node(
                goal.clone(),
                ProofRule::IntegerCorrelatedForbiddenRoots {
                    witness: witness.clone(),
                },
            );
            let mut d = verify_bounded_certificate_with_machine_parameters(
                &ctx5,
                &goal,
                &assumptions,
                &axioms,
                &params,
                &proof,
                &mut budget(),
            )
            .expect("correlated forbidden roots accepted");
            let _ = forge_term_and_assert_rejected(&mut d);

            let bad_goal = Proposition::Disjunction(vec![
                le(divr.clone(), s(i8, -2)),
                le(s(i8, 1), divr.clone()),
                Proposition::Conjunction(vec![
                    le(divr.clone(), s(i8, -1)),
                    le(s(i8, -126), divd.clone()),
                ]),
            ]);
            assert!(
                verify_bounded_certificate_with_machine_parameters(
                    &ctx5,
                    &bad_goal,
                    &assumptions,
                    &axioms,
                    &params,
                    &node(
                        bad_goal.clone(),
                        ProofRule::IntegerCorrelatedForbiddenRoots { witness }
                    ),
                    &mut budget()
                )
                .is_err()
            );
        }
    }

    // ------------------------------------------------------------------------
    // 6. SourceContentTheory & SourceIntegerTheory
    // ------------------------------------------------------------------------

    #[test]
    fn source_content_and_source_integer_theories() {
        let mut arena = TermArena::new();
        let mut decls = Vec::new();
        let mut ct = SourceContentTheory::new(&mut arena, &mut decls);
        let p0 = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let p1 = arena.insert(Term::Sort(Sort::Type(Level::Constant(1))));
        let c_is0 = ct
            .carrier(
                &mut arena,
                &mut decls,
                ContentAlgebraKind::IntervalSet,
                p0,
                Level::Constant(0),
            )
            .unwrap();
        let c_is0_again = ct
            .carrier(
                &mut arena,
                &mut decls,
                ContentAlgebraKind::IntervalSet,
                p0,
                Level::Constant(0),
            )
            .unwrap();
        let c_cq0 = ct
            .carrier(
                &mut arena,
                &mut decls,
                ContentAlgebraKind::CountedQuantity,
                p0,
                Level::Constant(0),
            )
            .unwrap();
        let c_is1 = ct
            .carrier(
                &mut arena,
                &mut decls,
                ContentAlgebraKind::IntervalSet,
                p1,
                Level::Constant(1),
            )
            .unwrap();
        assert!(arena.structurally_equal(c_is0, c_is0_again));
        assert!(!arena.structurally_equal(c_is0, c_cq0));
        assert!(!arena.structurally_equal(c_is0, c_is1));
        assert_eq!(decls.len(), 2);

        // SourceIntegerTheory: VectorStep budget bound (Total <= 10 * Length) + safety roster
        let mut arena = TermArena::new();
        let mut decls = Vec::new();
        let mut theory = SourceIntegerTheory::new(&mut arena, &mut decls).unwrap();
        let v = theory.vocabulary();
        let int = arena.insert(Term::Constant {
            declaration: v.carrier,
            levels: Vec::new(),
        });
        let add_h = arena.insert(Term::Constant {
            declaration: v.add,
            levels: Vec::new(),
        });
        let mul_h = arena.insert(Term::Constant {
            declaration: v.multiply,
            levels: Vec::new(),
        });
        let le_h = arena.insert(Term::Constant {
            declaration: v.less_or_equal,
            levels: Vec::new(),
        });
        let lt_h = arena.insert(Term::Constant {
            declaration: v.less_than,
            levels: Vec::new(),
        });
        let decl_int = |a: &mut TermArena, ds: &mut Vec<Declaration>| {
            let p = ds.len() as u32;
            ds.push(Declaration::assumption(0, int));
            a.insert(Term::Constant {
                declaration: p,
                levels: Vec::new(),
            })
        };
        let (total, len, item, next_total, next_len) = (
            decl_int(&mut arena, &mut decls),
            decl_int(&mut arena, &mut decls),
            decl_int(&mut arena, &mut decls),
            decl_int(&mut arena, &mut decls),
            decl_int(&mut arena, &mut decls),
        );
        let ten = theory
            .literal(&mut arena, &mut decls, BigInt::from_u64(10))
            .unwrap();
        let nine = theory
            .literal(&mut arena, &mut decls, BigInt::from_u64(9))
            .unwrap();
        let one = theory
            .literal(&mut arena, &mut decls, BigInt::from_u64(1))
            .unwrap();
        let zero = theory
            .literal(&mut arena, &mut decls, BigInt::zero())
            .unwrap();

        let ten_len = arena.apply_binary(mul_h, ten, len);
        let h_inv = arena.apply_binary(le_h, total, ten_len);
        let h_item = arena.apply_binary(le_h, item, ten);
        let sum_tot = arena.apply_binary(add_h, total, item);
        let h_tot = arena.insert(Term::Id {
            ty: int,
            left: next_total,
            right: sum_tot,
        });
        let sum_len = arena.apply_binary(add_h, len, one);
        let h_len = arena.insert(Term::Id {
            ty: int,
            left: next_len,
            right: sum_len,
        });
        let ten_next = arena.apply_binary(mul_h, ten, next_len);
        let goal = arena.apply_binary(le_h, next_total, ten_next);
        let assumptions = [h_inv, h_item, h_tot, h_len];
        let proof = theory
            .predicate_from_assumptions(&mut arena, &mut decls, goal, &assumptions, 0)
            .unwrap()
            .expect("proves VectorStep");
        let mut expected = goal;
        for &a in assumptions.iter().rev() {
            expected = arena.insert(Term::Pi {
                domain: a,
                codomain: expected,
            });
        }
        let cert = MathematicalCertificate {
            signature: decls.clone(),
            level_arity: 0,
            context: Vec::new(),
            term: proof,
            expected,
        };
        verify_mathematical_certificate(&mut arena, &cert, &mut budget())
            .expect("VectorStep checks");
        let roster: BTreeSet<u32> = theory.source_safety_roster().into_iter().collect();
        for d in certificate_assumption_closure(&arena, &cert) {
            assert!(
                roster.contains(&d) || (10..15).contains(&d),
                "unexpected assumption {d}"
            );
        }

        assert!(
            theory
                .predicate_from_assumptions(&mut arena, &mut decls, goal, &[h_inv, h_tot, h_len], 0)
                .unwrap()
                .is_none()
        );
        let nine_next = arena.apply_binary(mul_h, nine, next_len);
        let tighter = arena.apply_binary(le_h, next_total, nine_next);
        assert!(
            theory
                .predicate_from_assumptions(&mut arena, &mut decls, tighter, &assumptions, 0)
                .unwrap()
                .is_none()
        );
        for g in [goal, tighter] {
            assert!(
                !decls
                    .iter()
                    .any(|d| d.body.is_none() && arena.structurally_equal(d.ty, g))
            );
        }

        let two = theory
            .literal(&mut arena, &mut decls, BigInt::from_u64(2))
            .unwrap();
        let three = theory
            .literal(&mut arena, &mut decls, BigInt::from_u64(3))
            .unwrap();
        let five = theory
            .literal(&mut arena, &mut decls, BigInt::from_u64(5))
            .unwrap();
        let two_plus_three = arena.apply_binary(add_h, two, three);
        let eq_23_5 = arena.insert(Term::Id {
            ty: int,
            left: two_plus_three,
            right: five,
        });
        let pr_closed = theory
            .closed_predicate(&mut arena, &mut decls, eq_23_5)
            .unwrap()
            .expect("2+3=5");
        let cert_c = MathematicalCertificate {
            signature: decls.clone(),
            level_arity: 0,
            context: Vec::new(),
            term: pr_closed,
            expected: eq_23_5,
        };
        verify_mathematical_certificate(&mut arena, &cert_c, &mut budget()).unwrap();
        let lt_2_5 = arena.apply_binary(lt_h, two, five);
        assert!(
            theory
                .closed_predicate(&mut arena, &mut decls, lt_2_5)
                .unwrap()
                .is_some()
        );
        let bad_lt = arena.apply_binary(lt_h, five, two);
        assert!(
            theory
                .closed_predicate(&mut arena, &mut decls, bad_lt)
                .unwrap()
                .is_none()
        );

        // Resume reuses vocabulary & proves goals; refuses swapped/shifted/truncated records
        let saved = theory.positions();
        let decl_before_resume = decls.len();
        let mut resumed = SourceIntegerTheory::resume(&mut arena, &mut decls, saved).unwrap();
        assert_eq!(decls.len(), decl_before_resume);
        assert!(
            resumed
                .predicate_from_assumptions(&mut arena, &mut decls, goal, &assumptions, 0)
                .unwrap()
                .is_some()
        );
        let mut bad_swap = saved;
        std::mem::swap(
            &mut bad_swap.vocabulary.add,
            &mut bad_swap.vocabulary.subtract,
        );
        assert!(SourceIntegerTheory::resume(&mut arena, &mut decls, bad_swap).is_err());
        let mut bad_shift = saved;
        bad_shift.vocabulary.carrier += 1;
        assert!(SourceIntegerTheory::resume(&mut arena, &mut decls, bad_shift).is_err());
        let mut trunc = decls[..5].to_vec();
        assert!(SourceIntegerTheory::resume(&mut arena, &mut trunc, saved).is_err());

        // Part <= sum when other part >= 0 (both sides), and fails without non-negativity
        let x_plus_y = arena.apply_binary(add_h, total, item);
        let x_le_sum = arena.apply_binary(le_h, total, x_plus_y);
        let y_le_sum = arena.apply_binary(le_h, item, x_plus_y);
        let zero_le_item = arena.apply_binary(le_h, zero, item);
        let zero_le_total = arena.apply_binary(le_h, zero, total);
        assert!(
            resumed
                .predicate_from_assumptions(&mut arena, &mut decls, x_le_sum, &[zero_le_item], 0)
                .unwrap()
                .is_some()
        );
        assert!(
            resumed
                .predicate_from_assumptions(&mut arena, &mut decls, y_le_sum, &[zero_le_total], 0)
                .unwrap()
                .is_some()
        );
        assert!(
            resumed
                .predicate_from_assumptions(&mut arena, &mut decls, x_le_sum, &[], 0)
                .unwrap()
                .is_none()
        );

        // Dependent Sigma premise explicitly refused as Unsupported
        let v0 = arena.insert(Term::Variable(0));
        let dep_body = arena.insert(Term::Id {
            ty: int,
            left: v0,
            right: zero,
        });
        let dep_sig = arena.insert(Term::Sigma {
            domain: int,
            codomain: dep_body,
        });
        assert!(matches!(
            resumed.predicate_from_assumptions(&mut arena, &mut decls, x_le_sum, &[dep_sig], 0),
            Err(BoundedDenotationError::Unsupported(_))
        ));
    }

    // ------------------------------------------------------------------------
    // 7. Connectives, disjunctions, content conservation, equality transport
    // ------------------------------------------------------------------------

    #[test]
    fn connectives_disjunctions_content_conservation_and_equality_transport() {
        let u8 = u8_ty();
        let ctx = ctx_int(&[0, 1, 2], u8);
        let (x, y, z) = (val(0, u8), val(1, u8), val(2, u8));
        let (p, q, r) = (
            le(x.clone(), y.clone()),
            le(y.clone(), z.clone()),
            le(x.clone(), z.clone()),
        );

        let conv_cases: Vec<(Proposition, Proposition)> = vec![
            (
                Proposition::Conjunction(vec![p.clone(), q.clone()]),
                Proposition::Conjunction(vec![q.clone(), p.clone()]),
            ),
            (
                Proposition::Conjunction(vec![p.clone(), q.clone()]),
                Proposition::Conjunction(vec![p.clone(), p.clone(), q.clone()]),
            ),
            (
                Proposition::Conjunction(vec![
                    Proposition::Conjunction(vec![p.clone(), q.clone()]),
                    r.clone(),
                ]),
                Proposition::Conjunction(vec![p.clone(), q.clone(), r.clone()]),
            ),
            (
                Proposition::Conjunction(vec![Proposition::Truth, p.clone()]),
                p.clone(),
            ),
            (
                p.clone(),
                Proposition::Conjunction(vec![p.clone(), Proposition::Truth]),
            ),
            (
                Proposition::Disjunction(vec![p.clone(), q.clone()]),
                Proposition::Disjunction(vec![q.clone(), p.clone()]),
            ),
            (
                Proposition::Disjunction(vec![Proposition::Falsehood, p.clone()]),
                p.clone(),
            ),
            (
                p.clone(),
                Proposition::Disjunction(vec![p.clone(), Proposition::Falsehood]),
            ),
            (
                imp(
                    Proposition::Conjunction(vec![p.clone(), q.clone()]),
                    r.clone(),
                ),
                imp(
                    Proposition::Conjunction(vec![q.clone(), p.clone()]),
                    r.clone(),
                ),
            ),
            (
                Proposition::Disjunction(vec![eq(x.clone(), y.clone()), r.clone()]),
                Proposition::Disjunction(vec![eq(y.clone(), x.clone()), r.clone()]),
            ),
            (
                Proposition::Conjunction(vec![eq(x.clone(), y.clone()), r.clone()]),
                Proposition::Conjunction(vec![eq(y.clone(), x.clone()), r.clone()]),
            ),
            (
                Proposition::Disjunction(vec![lt(u(u8, 5), u(u8, 2)), p.clone()]),
                p.clone(),
            ),
            (
                Proposition::Conjunction(vec![Proposition::Falsehood, p.clone()]),
                Proposition::Falsehood,
            ),
            (
                Proposition::Conjunction(vec![eq(u(u8, 3), u(u8, 3)), p.clone()]),
                p.clone(),
            ),
        ];
        for (prem, goal) in conv_cases {
            let pr = node(
                goal.clone(),
                ProofRule::PredicateDenotation {
                    premise: Box::new(assume(0, prem.clone())),
                },
            );
            if goal == Proposition::Falsehood {
                let mut d = verify_ok(&ctx, &goal, std::slice::from_ref(&prem), &[], &pr);
                let _ = forge_term_and_assert_rejected(&mut d);
            } else {
                let d = verify_and_forge(&ctx, &goal, std::slice::from_ref(&prem), &[], &pr);
                assert_rechecks_and_rejects_forge(&d);
            }
        }

        let bad_p = Proposition::Conjunction(vec![eq(x.clone(), y.clone()), r.clone()]);
        let bad_g = Proposition::Conjunction(vec![eq(x.clone(), z.clone()), r.clone()]);
        expect_err(
            &ctx,
            &bad_g,
            std::slice::from_ref(&bad_p),
            &[],
            &node(
                bad_g.clone(),
                ProofRule::PredicateDenotation {
                    premise: Box::new(assume(0, bad_p.clone())),
                },
            ),
        );

        // Disjunction intro/elim + 64-member disjunction tags
        let disj = Proposition::Disjunction(vec![p.clone(), q.clone(), r.clone()]);
        let intro = node(
            disj.clone(),
            ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(assume(0, q.clone())),
                index: 1,
            },
        );
        verify_and_forge(&ctx, &disj, std::slice::from_ref(&q), &[], &intro);
        let elim = node(
            disj.clone(),
            ProofRule::DisjunctionElimination {
                disjunction: Box::new(assume(0, disj.clone())),
                branches: vec![
                    node(
                        disj.clone(),
                        ProofRule::DisjunctionIntroduction {
                            disjunct: Box::new(assume(1, p.clone())),
                            index: 0,
                        },
                    ),
                    node(
                        disj.clone(),
                        ProofRule::DisjunctionIntroduction {
                            disjunct: Box::new(assume(1, q.clone())),
                            index: 1,
                        },
                    ),
                    node(
                        disj.clone(),
                        ProofRule::DisjunctionIntroduction {
                            disjunct: Box::new(assume(1, r.clone())),
                            index: 2,
                        },
                    ),
                ],
            },
        );
        verify_and_forge(&ctx, &disj, std::slice::from_ref(&disj), &[], &elim);

        let many: Vec<Proposition> = (0..64u128).map(|i| le(x.clone(), u(u8, i))).collect();
        let wide_disj = Proposition::Disjunction(many.clone());
        for idx in [0usize, 31, 63] {
            let pr = node(
                wide_disj.clone(),
                ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(assume(0, many[idx].clone())),
                    index: idx,
                },
            );
            verify_ok(&ctx, &wide_disj, std::slice::from_ref(&many[idx]), &[], &pr);
        }

        // Content conservation transitivity (IdElim), reflexivity (Refl), & rejection
        let root = PlaceId::new(1).unwrap();
        let c_ctx = PropositionContext::from_value_types_and_places(
            [],
            [(root, StructuralPlaceKind::Parameter { position: 0 })],
        )
        .unwrap();
        let alg = ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "items".into(),
        };
        let cterm = |field: &str| ContentTerm::Projection {
            projection: ContentProjectionIdentity {
                domain: ContentDomainId::new(1).unwrap(),
                projection_report_fingerprint: 7,
            },
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root,
                segments: vec![ContentPlaceSegment::Field(field.into())],
            },
        };
        let cons = |l: &str, r: &str| {
            Proposition::ContentConservation(ContentConservation::new(
                alg.clone(),
                cterm(l),
                cterm(r),
            ))
        };
        for (a, b, c) in [
            (("a", "b"), ("b", "c"), ("a", "c")),
            (("a", "c"), ("b", "c"), ("a", "b")),
            (("b", "c"), ("a", "b"), ("a", "c")),
        ] {
            let (p1, p2, g) = (cons(a.0, a.1), cons(b.0, b.1), cons(c.0, c.1));
            let pr = node(
                g.clone(),
                ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(sax(0, p1.clone())),
                    middle_equals_right: Box::new(sax(1, p2.clone())),
                },
            );
            let d = verify_and_forge(&c_ctx, &g, &[], &[p1, p2], &pr);
            assert_eq!(d.certificate.signature.len(), 5);
            assert!(matches!(
                d.arena.get(d.certificate.term),
                Term::IdElim { .. }
            ));
        }
        let cc00 = cons("a", "a");
        let d_refl = verify_and_forge(
            &c_ctx,
            &cc00,
            &[],
            &[],
            &prim(PrimitiveJudgment::ReflexiveEquality, cc00.clone()),
        );
        assert!(matches!(
            d_refl.arena.get(d_refl.certificate.term),
            Term::Refl { .. }
        ));
        let (p_ab, g_ac) = (cons("a", "b"), cons("a", "c"));
        expect_err(
            &c_ctx,
            &g_ac,
            &[],
            &[p_ab.clone(), cc00.clone()],
            &node(
                g_ac.clone(),
                ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(sax(0, p_ab)),
                    middle_equals_right: Box::new(sax(1, cc00)),
                },
            ),
        );
    }

    // ------------------------------------------------------------------------
    // 8. Default-stack depth/width, scope restoration, budget & logical table
    // ------------------------------------------------------------------------

    #[test]
    fn default_stack_depth_width_scope_restoration_and_logical_table_invariants() {
        let u8 = u8_ty();
        std::thread::spawn(move || {
            let ctx = ctx_int(&[0, 1], u8);
            let atom = le(val(0, u8), val(1, u8));
            let mut nested = atom.clone();
            for _ in 0..8192 {
                nested = imp(atom.clone(), nested);
            }
            let mut den = Denotation::new();
            let t1 = den.denote(&nested).unwrap();
            let t2 = den.denote(&nested).unwrap();
            assert_eq!(t1, t2);

            let wide = Proposition::Conjunction(vec![atom.clone(); 16_384]);
            let mut den_w = Denotation::new();
            assert!(den_w.denote(&wide).is_ok());

            let mut true_chain = Proposition::Truth;
            for _ in 0..8192 {
                true_chain = imp(atom.clone(), true_chain);
            }
            let t_ty = den_w.denote(&true_chain).unwrap();
            assert!(den_w.closed_inhabitant(t_ty).unwrap().is_some());
            let mut false_chain = Proposition::Falsehood;
            for _ in 0..8192 {
                false_chain = imp(atom.clone(), false_chain);
            }
            let f_ty = den_w.denote(&false_chain).unwrap();
            assert!(den_w.closed_inhabitant(f_ty).unwrap().is_none());

            let mut false_conj = Proposition::Falsehood;
            for _ in 0..8192 {
                false_conj = Proposition::Conjunction(vec![atom.clone(), false_conj]);
            }
            let (fc_ty, g_ty, ev) = (
                den_w.denote(&false_conj).unwrap(),
                den_w.denote(&atom).unwrap(),
                den_w.var(0),
            );
            assert!(den_w.oriented_evidence(fc_ty, g_ty, ev).unwrap().is_some());

            for valid in [true, false] {
                let mut pnode = prim(
                    PrimitiveJudgment::Truth,
                    if valid {
                        Proposition::Truth
                    } else {
                        Proposition::Falsehood
                    },
                );
                for _ in 0..4096 {
                    pnode = node(
                        Proposition::Truth,
                        ProofRule::PredicateDenotation {
                            premise: Box::new(pnode),
                        },
                    );
                }
                let res = verify_bounded_certificate(
                    &PropositionContext::default(),
                    &Proposition::Truth,
                    &[],
                    &[],
                    &pnode,
                    &mut budget(),
                );
                if valid {
                    res.expect("4096-deep valid chain passes");
                } else {
                    assert!(res.is_err());
                }
            }
            let _ = ctx;
        })
        .join()
        .expect("default-stack thread succeeded");

        // Shared allowance across failed alternatives (allowance in 0..=3)
        let mut den = Denotation::new();
        let a = den.denote(&le(val(0, u8), val(1, u8))).unwrap();
        let b = den.denote(&le(val(1, u8), val(0, u8))).unwrap();
        let conj = den.sigma(a, a);
        let ev = den.var(0);
        for allow in 0..=3u64 {
            let mut rem = allow;
            assert!(
                den.oriented_coercion(conj, b, ev, &mut rem)
                    .unwrap()
                    .is_none()
            );
            assert!(rem <= allow);
        }

        // Scope restoration after scope errors
        let ctx = ctx_int(&[0, 1], u8);
        let (p, q) = (le(val(0, u8), val(1, u8)), le(val(1, u8), val(0, u8)));
        let mp = BTreeSet::new();
        let mut elab = Elaboration::new(
            Denotation::new(),
            &ctx,
            &p,
            std::slice::from_ref(&p),
            &[],
            &mp,
        )
        .unwrap();
        let bad_imp = node(
            imp(q.clone(), q.clone()),
            ProofRule::ImplicationIntroduction {
                body: Box::new(assume(99, q.clone())),
            },
        );
        assert!(elab.node(&bad_imp).is_err());
        assert_eq!(
            (elab.depth, elab.roster.len(), elab.positions.len()),
            (1, 1, 1)
        );
        assert!(elab.node(&assume(0, p.clone())).is_ok());

        // Logical table: distinct syntax with equal leaf denotations, arity errors, & occurs_free overflow
        let mut den2 = Denotation::new();
        let (t_f, t_m) = (
            den2.denote(&eq(val(0, u8), val(1, u8))).unwrap(),
            den2.denote(&meq(mvar(0, u8), mvar(1, u8))).unwrap(),
        );
        assert_eq!(t_f, t_m);
        assert!(
            den2.denote(&Proposition::Conjunction(vec![p.clone()]))
                .is_err()
        );
        assert!(
            den2.denote(&Proposition::Disjunction(vec![p.clone()]))
                .is_err()
        );
        let v_max = den2.var(u32::MAX);
        assert!(den2.occurs_free(v_max, u32::MAX));
        assert!(!den2.occurs_free(v_max, 0));
        let dom = den2.two;
        let v0 = den2.var(0);
        let dep_cod = den2.id(dom, v0, den2.two_zero);
        let dep_sig = den2.sigma(dom, dep_cod);
        assert!(den2.closed_inhabitant(dep_sig).unwrap().is_none());
        let (z_id, o_id) = (
            den2.id(dom, den2.two_zero, den2.two_zero),
            den2.id(dom, den2.two_one, den2.two_one),
        );
        let sum_tt = den2.tagged_sum(z_id, o_id);
        let inh = den2
            .closed_inhabitant(sum_tt)
            .unwrap()
            .expect("inhabited sum");
        assert!(
            matches!(den2.arena.get(inh), Term::Pair { first, .. } if den2.arena.get(first) == Term::TwoZero)
        );
    }
}

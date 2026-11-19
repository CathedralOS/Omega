//! A bound through `x & y` or `x | y` must cite every axiom it uses.
//!
//! `IntegerAffineBound` replays a bitwise step only when the other operand
//! is a literal, either written in place or fixed by a cited landing axiom
//! `y == literal` (`wiki/spec/terminal-psi/integer_certificates.md`,
//! "IntegerAffineBound"). The kernel once also searched earlier axioms for
//! `y <= U` and used it without citing it, so the mathematical core assumed
//! `z == x & y ⊢ z <= 3` alone, which `x = y = 255` falsifies. Each positive
//! case below has a negative twin that differs only in that `y` is bounded
//! rather than fixed, and the kernel must refuse it.

use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

const X: u64 = 1;
const Y: u64 = 2;
const Z: u64 = 3;

fn u8_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(u8_type()),
    )
}

fn literal(number: i128) -> ScalarTerm {
    ScalarTerm::integer(
        u8_type(),
        IntegerValue::for_sign(IntegerSign::Unsigned, number).unwrap(),
    )
    .unwrap()
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types([X, Y, Z].map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(u8_type()),
        )
    }))
    .unwrap()
}

#[derive(Clone, Copy)]
enum Operation {
    And,
    Or,
}

/// `z == x & y` or `z == x | y`.
fn definition(operation: Operation) -> Proposition {
    let expression = match operation {
        Operation::And => ScalarTerm::integer_bitwise_and(u8_type(), value(X), value(Y)),
        Operation::Or => ScalarTerm::integer_bitwise_or(u8_type(), value(X), value(Y)),
    }
    .unwrap();
    Proposition::Equal(value(Z), expression)
}

/// The certificate of `conclusion` from `root_bound` on `x` through the
/// definition at axiom 1, landing `y` on axiom 0 when `landing` says so.
fn certificate(
    root_bound: ProofNode,
    landing: Option<usize>,
    conclusion: Proposition,
) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness: IntegerAffineWitness {
                root: value(X),
                target: value(Z),
                definition_axioms: vec![1],
                literal_axioms: vec![landing],
            },
        },
    }
}

fn truth() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

fn admitted(facts: &[Proposition], axioms: &[Proposition], proof: &ProofNode) -> bool {
    check_certificate(&context(), &proof.conclusion, facts, axioms, proof).is_ok()
}

/// `z <= 3` from `z == x & y` when `y == 3` is cited; refused when the only
/// fact about `y` is `y <= 3`, whether the landing is left out or names it.
#[test]
fn bitwise_and_mask_must_be_a_cited_literal() {
    let conclusion = Proposition::LessOrEqual(value(Z), literal(3));
    let fixed = [
        Proposition::Equal(value(Y), literal(3)),
        definition(Operation::And),
    ];
    assert!(admitted(
        &[],
        &fixed,
        &certificate(truth(), Some(0), conclusion.clone())
    ));

    let bounded = [
        Proposition::LessOrEqual(value(Y), literal(3)),
        definition(Operation::And),
    ];
    for landing in [None, Some(0)] {
        assert!(
            !admitted(
                &[],
                &bounded,
                &certificate(truth(), landing, conclusion.clone())
            ),
            "landing {landing:?}"
        );
    }
}

/// `z <= 7` from `x <= 4` and `z == x | y` when `y == 3` is cited; refused
/// when the only fact about `y` is `y <= 3`.
#[test]
fn bitwise_or_sibling_must_be_a_cited_literal() {
    let root = Proposition::LessOrEqual(value(X), literal(4));
    let root_bound = ProofNode {
        conclusion: root.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    let conclusion = Proposition::LessOrEqual(value(Z), literal(7));
    let fixed = [
        Proposition::Equal(value(Y), literal(3)),
        definition(Operation::Or),
    ];
    assert!(admitted(
        std::slice::from_ref(&root),
        &fixed,
        &certificate(root_bound.clone(), Some(0), conclusion.clone())
    ));

    let bounded = [
        Proposition::LessOrEqual(value(Y), literal(3)),
        definition(Operation::Or),
    ];
    for landing in [None, Some(0)] {
        assert!(
            !admitted(
                std::slice::from_ref(&root),
                &bounded,
                &certificate(root_bound.clone(), landing, conclusion.clone())
            ),
            "landing {landing:?}"
        );
    }
}

//! The bounded certificate language crossing the canonical mathematical
//! wire: a `terminal_psi::ProofNode` certificate — the shape every shipped
//! producer emits — is denoted by `proof_admission::denote_bounded_certificate`
//! into the common mathematical core, encoded, decoded into a fresh arena,
//! and re-decided by `verify_mathematical_certificate`. The kernel never
//! sees the bounded checker or a producer success flag on this route —
//! only the judgment and its exact assumption closure survive the wire.

use std::collections::BTreeSet;

use proof_admission::{
    Budget, CoreError, DEFAULT_CONVERSION_STEPS, Term, certificate_assumption_closure,
    denote_bounded_certificate, verify_mathematical_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::{decode_mathematical_certificate, encode_mathematical_certificate};
use terminal_psi::{ProofNode, ProofRule};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

fn unsigned64() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")
}

fn unsigned64_type() -> ScalarType {
    ScalarType::Integer(unsigned64())
}

fn value(id: u64) -> (ValueId, ScalarTerm) {
    let id = ValueId::new(id).expect("value id");
    (id, ScalarTerm::value(id, unsigned64_type()))
}

/// The contract-entailment discharge shape `[x <= y] ⊢ x <= y` — the
/// judgment `typed-trees-to-checked-trees`'s assumption discharge emits —
/// denoted, encoded, decoded and re-decided end to end.
#[test]
fn a_bounded_discharge_certificate_crosses_the_wire() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let bound = Proposition::LessOrEqual(x, y);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let proof = ProofNode {
        conclusion: bound.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    let denoted =
        denote_bounded_certificate(&context, &bound, std::slice::from_ref(&bound), &[], &proof)
            .expect("denote");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");

    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the decoded judgment re-verifies");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("canonical re-encode"),
        bytes,
    );
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1, 2, 3]),
        "the judgment retains Int, IntLe and the two integer endpoints",
    );

    // A receiver that swaps the evidence for a free variable — the forged
    // wire decodes fine but the kernel re-decides it away.
    decoded.certificate.term = decoded.arena.insert(Term::Variable(7));
    assert!(matches!(
        verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget()),
        Err(CoreError::UnboundVariable { .. }),
    ));

    // A claimed type the judgment does not establish — `⟦Falsehood⟧` —
    // rejects on re-decision.
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    let ty = decoded.arena.insert(Term::Two);
    let left = decoded.arena.insert(Term::TwoZero);
    let right = decoded.arena.insert(Term::TwoOne);
    decoded.certificate.expected = decoded.arena.insert(Term::Id { ty, left, right });
    assert!(matches!(
        verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget()),
        Err(CoreError::TypeMismatch { .. }),
    ));

    // Malformed wire shapes never reach the kernel.
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_mathematical_certificate(&trailing).is_err());
    assert!(decode_mathematical_certificate(&bytes[..bytes.len() - 1]).is_err());
    let mut forged_magic = bytes.clone();
    forged_magic[0] = b'X';
    assert!(decode_mathematical_certificate(&forged_magic).is_err());
}

/// Fixed and mathematical integer equality share one Id Int judgment.
/// Citation conversion survives the wire without a conversion assumption;
/// changing an endpoint still rejects before encoding.
#[test]
fn a_fixed_integer_equality_citation_crosses_the_mathematical_wire() {
    let (x_id, x) = value(1);
    let (y_id, y) = value(2);
    let context = PropositionContext::from_value_types([
        (x_id, unsigned64_type()),
        (y_id, unsigned64_type()),
    ])
    .expect("context");
    let fixed = Proposition::Equal(x.clone(), y);
    let lifted = proof_admission::lift_fixed_integer_relation(&fixed).expect("lifts");
    let proof = ProofNode {
        conclusion: lifted.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    let denoted =
        denote_bounded_certificate(&context, &lifted, std::slice::from_ref(&fixed), &[], &proof)
            .expect("both integer equality forms share their denotation");
    let bytes =
        encode_mathematical_certificate(&denoted.arena, &denoted.certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
        .expect("the equality citation re-verifies after decode");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("canonical re-encode"),
        bytes,
    );
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        BTreeSet::from([0, 1, 2]),
        "Int and the two endpoints, without a conversion law",
    );
    assert!(matches!(
        decoded.arena.get(decoded.certificate.expected),
        Term::Id { .. }
    ));

    let changed = Proposition::Equal(x.clone(), x);
    let changed = proof_admission::lift_fixed_integer_relation(&changed).expect("lifts");
    let proof = ProofNode {
        conclusion: changed.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    assert!(denote_bounded_certificate(&context, &changed, &[fixed], &[], &proof).is_err());
}

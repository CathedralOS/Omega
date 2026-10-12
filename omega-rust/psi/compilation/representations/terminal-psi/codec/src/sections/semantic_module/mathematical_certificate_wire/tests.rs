//! Pure mathematical wire protocol tests, with no Omega source input.

use super::{encode_mathematical_certificate, encode_mathematical_signature};
use proof_admission::{Declaration, MathematicalCertificate, Term, TermArena};

#[test]
fn mathematical_wire_encoding_preserves_certificate_format_three_bytes() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let zero = arena.insert(Term::TwoZero);
    let constant = arena.insert(Term::Constant {
        declaration: 0,
        levels: Vec::new(),
    });
    let certificate = MathematicalCertificate {
        signature: vec![Declaration::definition(0, two, zero)],
        level_arity: 0,
        context: Vec::new(),
        term: constant,
        expected: two,
    };
    // Format 3: Two, TwoZero and Constant(0); one definition; closed judgment.
    let expected: [u8; 58] = [
        0x50, 0x53, 0x49, 0x43, 0x4f, 0x52, 0x45, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
        0x00, 0x00, 0x00, 0x0a, 0x0b, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    assert_eq!(
        encode_mathematical_certificate(&arena, &certificate).unwrap(),
        expected
    );
}

#[test]
fn mathematical_wire_encoding_retains_signature_assumption_and_dependency() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let earlier = arena.insert(Term::Constant {
        declaration: 0,
        levels: Vec::new(),
    });
    let declarations = [
        Declaration::assumption(0, two),
        Declaration::definition(0, two, earlier),
    ];
    // The body references the preceding assumption in this same cohort.
    let expected: [u8; 50] = [
        0x50, 0x53, 0x49, 0x53, 0x49, 0x47, 0x4e, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x0a,
        0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01, 0x01, 0x00, 0x00, 0x00,
    ];
    let encoded = encode_mathematical_signature(&arena, &declarations).unwrap();
    assert_eq!(encoded, expected);
    let zero = arena.insert(Term::TwoZero);
    let independent = [
        Declaration::assumption(0, two),
        Declaration::definition(0, two, zero),
    ];
    assert_ne!(
        encoded,
        encode_mathematical_signature(&arena, &independent).unwrap()
    );
}

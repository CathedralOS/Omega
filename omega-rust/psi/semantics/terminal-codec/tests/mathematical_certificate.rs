//! The certification bridge for the common mathematical kernel: an untrusted
//! producer encodes a complete judgment `Γ ⊢ t : T`; the receiver decodes the
//! certificate and proof-admission's kernel re-decides it. Nothing the
//! producer asserts is trusted — changing the term, the context, the claimed
//! type, or the bytes rejects or changes the checked outcome.

use proof_admission::{
    Budget, CoreError, DEFAULT_CONVERSION_STEPS, Level, MathematicalCertificate, Sort, Term,
    TermArena, TermHandle, verify_mathematical_certificate,
};
use terminal_codec::{
    CodecError, decode_mathematical_certificate, encode_mathematical_certificate,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level(level))))
}

fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

/// `λ(A : Type 0). λ(x : A). x : Π(A : Type 0). Π(x : A). A`.
fn polymorphic_identity(arena: &mut TermArena) -> (TermHandle, TermHandle) {
    let type_zero = type_sort(arena, 0);
    let bound = variable(arena, 0);
    let inner = lambda(arena, bound, bound);
    let identity = lambda(arena, type_zero, inner);
    let codomain_domain = variable(arena, 0);
    let codomain_body = variable(arena, 1);
    let codomain = pi(arena, codomain_domain, codomain_body);
    let expected = pi(arena, type_zero, codomain);
    (identity, expected)
}

fn verify(decoded: &mut terminal_codec::DecodedMathematicalCertificate) -> Result<(), CoreError> {
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
}

#[test]
fn an_untrusted_certificate_verifies_end_to_end() {
    let mut arena = TermArena::new();
    let (identity, expected) = polymorphic_identity(&mut arena);
    let certificate = MathematicalCertificate {
        context: Vec::new(),
        term: identity,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

    // The receiver sees only bytes: decode, then the kernel re-decides.
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("the claimed judgment must hold");
}

#[test]
fn changing_the_claimed_type_rejects() {
    let mut arena = TermArena::new();
    let (identity, _) = polymorphic_identity(&mut arena);
    let type_zero = type_sort(&mut arena, 0);
    // A different claimed type: Π(A : Type 0). Π(x : A). Type 0.
    let bound = variable(&mut arena, 0);
    let wrong_inner = pi(&mut arena, bound, type_zero);
    let wrong_expected = pi(&mut arena, type_zero, wrong_inner);
    let certificate = MathematicalCertificate {
        context: Vec::new(),
        term: identity,
        expected: wrong_expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

#[test]
fn dropping_a_context_binding_rejects() {
    let mut arena = TermArena::new();
    // Γ = A : Type 0, x : A proves x : A; without x's binding it cannot.
    let type_zero = type_sort(&mut arena, 0);
    let bound = variable(&mut arena, 0);
    let shifted = variable(&mut arena, 1);
    let complete = MathematicalCertificate {
        context: vec![type_zero, bound],
        term: bound,
        expected: shifted,
    };
    let bytes = encode_mathematical_certificate(&arena, &complete).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("dependent telescope must check");

    let mut arena = TermArena::new();
    let type_zero = type_sort(&mut arena, 0);
    let bound = variable(&mut arena, 0);
    let shifted = variable(&mut arena, 1);
    let missing_binding = MathematicalCertificate {
        context: vec![type_zero],
        term: bound,
        expected: shifted,
    };
    let bytes = encode_mathematical_certificate(&arena, &missing_binding).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::UnboundVariable { .. })
    ));
}

#[test]
fn substituting_the_evidence_term_rejects() {
    let mut arena = TermArena::new();
    let (_, expected) = polymorphic_identity(&mut arena);
    // A bare variable is not the polymorphic identity: it is unbound here.
    let bound = variable(&mut arena, 0);
    let certificate = MathematicalCertificate {
        context: Vec::new(),
        term: bound,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::UnboundVariable { .. })
    ));
}

#[test]
fn byte_level_forgery_cannot_alias_a_certificate() {
    let mut arena = TermArena::new();
    let (identity, expected) = polymorphic_identity(&mut arena);
    let certificate = MathematicalCertificate {
        context: Vec::new(),
        term: identity,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

    // Flipping a payload bit decodes to a different judgment or not at all;
    // either way it must not verify the original claim.
    let mut forged = bytes.clone();
    let last_node_byte = bytes.len() - 12 - 1;
    forged[last_node_byte] ^= 0x01;
    let outcome = decode_mathematical_certificate(&forged).map(|mut decoded| verify(&mut decoded));
    match outcome {
        Err(_) | Ok(Err(_)) => {}
        Ok(Ok(())) => panic!("a forged certificate must not verify"),
    }

    // Pointing the claimed-type root at an earlier node leaves the remaining
    // table nodes unreachable, so the stream is not a canonical certificate
    // at all — decode rejects before the kernel is even asked.
    let mut swapped = bytes.clone();
    let expected_root = bytes.len() - 4;
    swapped[expected_root] = 0; // claim `Type 0` instead of the Π type
    assert!(matches!(
        decode_mathematical_certificate(&swapped),
        Err(CodecError::NonCanonicalEncoding)
    ));

    // Trailing garbage is not part of the canonical form.
    let mut trailing = bytes.clone();
    trailing.extend_from_slice(&[0, 0]);
    assert!(matches!(
        decode_mathematical_certificate(&trailing),
        Err(CodecError::TrailingBytes(2))
    ));
}

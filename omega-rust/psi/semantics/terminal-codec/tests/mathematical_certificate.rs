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
fn a_two_elimination_certificate_verifies_end_to_end() {
    let mut arena = TermArena::new();
    // Γ = A : Type 0, a : A, t : Two proves `caseTwo(C, a, a, t) : C t`
    // for the constant family `C := λ(_:Two). A`; on a constructor
    // scrutinee the same elimination checks at `A` by computation. The
    // certificate carries the inductive profile's eliminator as data and
    // the kernel re-decides it after decode.
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let two_binding = arena.insert(Term::Two);
    let family = {
        let domain = arena.insert(Term::Two);
        // Under C's binder (depth 4) A is index 3.
        let a_under = variable(&mut arena, 3);
        lambda(&mut arena, domain, a_under)
    };
    let a_term = variable(&mut arena, 1);
    let scrutinee = variable(&mut arena, 0);
    let term = arena.insert(Term::CaseTwo {
        motive: family,
        zero_branch: a_term,
        one_branch: a_term,
        scrutinee,
    });
    let expected = arena.insert(Term::Apply {
        function: family,
        argument: scrutinee,
    });
    let certificate = MathematicalCertificate {
        context: vec![type_zero, a_binding, two_binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("dependent elimination over Two must check");

    // The same elimination on `zero` claims `A` — the result type
    // computes through the constructor branch after decode.
    let mut arena = TermArena::new();
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let two_binding = arena.insert(Term::Two);
    let family = {
        let domain = arena.insert(Term::Two);
        let a_under = variable(&mut arena, 3);
        lambda(&mut arena, domain, a_under)
    };
    let a_term = variable(&mut arena, 1);
    let zero = arena.insert(Term::TwoZero);
    let term = arena.insert(Term::CaseTwo {
        motive: family,
        zero_branch: a_term,
        one_branch: a_term,
        scrutinee: zero,
    });
    let a_type = variable(&mut arena, 2);
    let certificate = MathematicalCertificate {
        context: vec![type_zero, a_binding, two_binding],
        term,
        expected: a_type,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("constructor computation decides the type");

    // The same elimination claimed at `Two` is a different, false
    // judgment.
    let certificate = MathematicalCertificate {
        context: vec![type_zero, a_binding, two_binding],
        term,
        expected: two_binding,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

#[test]
fn an_identity_elimination_certificate_verifies_end_to_end() {
    let mut arena = TermArena::new();
    // Γ = A : Type 0, x : A, P : Π(y:A). Type 0, y : A,
    // p : Id A x y, h : P x proves `J(C, λ(h:Px).h, y, p) h : P y` —
    // the certificate carries the identity eliminator as data across
    // the wire and the kernel re-decides it after decode. In depth 6:
    // A is index 5, x is 4, P is 3, y is 2, p is 1, h is 0.
    let type_zero = type_sort(&mut arena, 0);
    let x_binding = variable(&mut arena, 0);
    let predicate_binding = {
        let domain = variable(&mut arena, 1);
        pi(&mut arena, domain, type_zero)
    };
    let y_binding = variable(&mut arena, 2);
    let proof_binding = {
        let ty = variable(&mut arena, 3);
        let x = variable(&mut arena, 2);
        let y = variable(&mut arena, 0);
        arena.insert(Term::Id {
            ty,
            left: x,
            right: y,
        })
    };
    let hypothesis_binding = {
        let predicate = variable(&mut arena, 2);
        let x = variable(&mut arena, 3);
        arena.insert(Term::Apply {
            function: predicate,
            argument: x,
        })
    };
    let motive = {
        let domain = variable(&mut arena, 5);
        let inner_domain = {
            let ty = variable(&mut arena, 6);
            let x = variable(&mut arena, 5);
            let bound = variable(&mut arena, 0);
            arena.insert(Term::Id {
                ty,
                left: x,
                right: bound,
            })
        };
        let body = {
            let domain = {
                let predicate = variable(&mut arena, 5);
                let x = variable(&mut arena, 6);
                arena.insert(Term::Apply {
                    function: predicate,
                    argument: x,
                })
            };
            let codomain = {
                let predicate = variable(&mut arena, 6);
                let bound_y = variable(&mut arena, 2);
                arena.insert(Term::Apply {
                    function: predicate,
                    argument: bound_y,
                })
            };
            pi(&mut arena, domain, codomain)
        };
        let inner = lambda(&mut arena, inner_domain, body);
        lambda(&mut arena, domain, inner)
    };
    let base = {
        let domain = {
            let predicate = variable(&mut arena, 3);
            let x = variable(&mut arena, 4);
            arena.insert(Term::Apply {
                function: predicate,
                argument: x,
            })
        };
        let bound = variable(&mut arena, 0);
        lambda(&mut arena, domain, bound)
    };
    let endpoint = variable(&mut arena, 2);
    let proof = variable(&mut arena, 1);
    let elimination = arena.insert(Term::IdElim {
        motive,
        base,
        endpoint,
        proof,
    });
    let hypothesis = variable(&mut arena, 0);
    let term = arena.insert(Term::Apply {
        function: elimination,
        argument: hypothesis,
    });
    let expected = {
        let predicate = variable(&mut arena, 3);
        let y = variable(&mut arena, 2);
        arena.insert(Term::Apply {
            function: predicate,
            argument: y,
        })
    };
    let certificate = MathematicalCertificate {
        context: vec![
            type_zero,
            x_binding,
            predicate_binding,
            y_binding,
            proof_binding,
            hypothesis_binding,
        ],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("identity elimination must re-verify after decode");

    // Re-encoding the decoded judgment reproduces the same canonical
    // bytes — the identity nodes participate in deduplication and
    // canonical ordering like every other constructor.
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // A certificate claiming `P x` — the transported-away subject — is a
    // different, false judgment and must not verify after decode.
    let certificate = MathematicalCertificate {
        context: vec![
            type_zero,
            x_binding,
            predicate_binding,
            y_binding,
            proof_binding,
            hypothesis_binding,
        ],
        term,
        expected: {
            let predicate = variable(&mut arena, 3);
            let x = variable(&mut arena, 4);
            arena.insert(Term::Apply {
                function: predicate,
                argument: x,
            })
        },
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

#[test]
fn a_w_induction_certificate_verifies_end_to_end() {
    let mut arena = TermArena::new();
    // Γ = A : Type 0, B : Π(_:A). Type 0, a : A, k : Π(b:B a). W A B,
    //     P : Π(_:W A B). Type 0, s : <step type>, t : W A B
    // proves `indW(P, s, sup A B a k) : P (sup A B a k)` — the
    // certificate carries the W-type, its constructor, and the
    // dependent induction across the wire (term tags 17-19), and the
    // kernel re-decides the whole judgment after decode. In depth 7:
    // t = 0, s = 1, P = 2, k = 3, a = 4, B = 5, A = 6.
    let type_zero = type_sort(&mut arena, 0);
    let b_binding = {
        let domain = variable(&mut arena, 0);
        pi(&mut arena, domain, type_zero)
    };
    let a_binding = variable(&mut arena, 1);
    let k_binding = {
        let b_at = variable(&mut arena, 1);
        let a_at = variable(&mut arena, 0);
        let child_positions = arena.insert(Term::Apply {
            function: b_at,
            argument: a_at,
        });
        let carrier = variable(&mut arena, 3);
        let children = variable(&mut arena, 2);
        let w_under_b = arena.insert(Term::W { carrier, children });
        pi(&mut arena, child_positions, w_under_b)
    };
    let p_binding = {
        let carrier = variable(&mut arena, 3);
        let children = variable(&mut arena, 2);
        let w = arena.insert(Term::W { carrier, children });
        pi(&mut arena, w, type_zero)
    };
    // `Π(a' : A). Π(k' : Π(b : B a'). W A B). Π(_ : Π(b : B a').
    // P (k' b)). P (sup A B a' k')` over the five-binding prefix.
    let s_binding = {
        let a_domain = variable(&mut arena, 4);
        let function_type = {
            let b_at = variable(&mut arena, 4);
            let a_bound = variable(&mut arena, 0);
            let child_positions = arena.insert(Term::Apply {
                function: b_at,
                argument: a_bound,
            });
            let carrier = variable(&mut arena, 6);
            let children = variable(&mut arena, 5);
            let w_at_two = arena.insert(Term::W { carrier, children });
            pi(&mut arena, child_positions, w_at_two)
        };
        let hypothesis = {
            let b_at = variable(&mut arena, 5);
            let a_bound = variable(&mut arena, 1);
            let hypothesis_domain = arena.insert(Term::Apply {
                function: b_at,
                argument: a_bound,
            });
            let p_at = variable(&mut arena, 3);
            let k_bound = variable(&mut arena, 1);
            let b_bound = variable(&mut arena, 0);
            let child = arena.insert(Term::Apply {
                function: k_bound,
                argument: b_bound,
            });
            let hypothesis_codomain = arena.insert(Term::Apply {
                function: p_at,
                argument: child,
            });
            pi(&mut arena, hypothesis_domain, hypothesis_codomain)
        };
        let result = {
            let p_at = variable(&mut arena, 3);
            let carrier = variable(&mut arena, 7);
            let children = variable(&mut arena, 6);
            let label = variable(&mut arena, 2);
            let function = variable(&mut arena, 1);
            let node = arena.insert(Term::Sup {
                carrier,
                children,
                label,
                function,
            });
            arena.insert(Term::Apply {
                function: p_at,
                argument: node,
            })
        };
        let inner = pi(&mut arena, hypothesis, result);
        let middle = pi(&mut arena, function_type, inner);
        pi(&mut arena, a_domain, middle)
    };
    let t_binding = {
        let carrier = variable(&mut arena, 5);
        let children = variable(&mut arena, 4);
        arena.insert(Term::W { carrier, children })
    };
    let node = {
        let carrier = variable(&mut arena, 6);
        let children = variable(&mut arena, 5);
        let label = variable(&mut arena, 4);
        let function = variable(&mut arena, 3);
        arena.insert(Term::Sup {
            carrier,
            children,
            label,
            function,
        })
    };
    let term = {
        let motive = variable(&mut arena, 2);
        let step = variable(&mut arena, 1);
        arena.insert(Term::IndW {
            motive,
            step,
            tree: node,
        })
    };
    let expected = {
        let p = variable(&mut arena, 2);
        arena.insert(Term::Apply {
            function: p,
            argument: node,
        })
    };
    let certificate = MathematicalCertificate {
        context: vec![
            type_zero, b_binding, a_binding, k_binding, p_binding, s_binding, t_binding,
        ],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("W induction must re-verify after decode");

    // Re-encoding the decoded judgment reproduces the same canonical
    // bytes — the W nodes participate in deduplication and canonical
    // ordering like every other constructor.
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // Claiming `P t` — the neutral tree, not the constructor the
    // induction ran on — is a different, false judgment.
    let certificate = MathematicalCertificate {
        context: vec![
            type_zero, b_binding, a_binding, k_binding, p_binding, s_binding, t_binding,
        ],
        term,
        expected: {
            let p = variable(&mut arena, 2);
            let t = variable(&mut arena, 0);
            arena.insert(Term::Apply {
                function: p,
                argument: t,
            })
        },
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
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

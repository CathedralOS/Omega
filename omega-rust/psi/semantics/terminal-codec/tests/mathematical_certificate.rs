//! The certification bridge for the common mathematical kernel: an untrusted
//! producer encodes a complete judgment `Γ ⊢ t : T`; the receiver decodes the
//! certificate and proof-admission's kernel re-decides it. Nothing the
//! producer asserts is trusted — changing the term, the context, the claimed
//! type, or the bytes rejects or changes the checked outcome.

use proof_admission::{
    Budget, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
    MathematicalCertificate, Sort, Term, TermArena, TermHandle, indexed_scheme, shift,
    verify_mathematical_certificate,
};
use terminal_codec::{
    CodecError, decode_mathematical_certificate, encode_mathematical_certificate,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
}

fn sort_level(arena: &mut TermArena, level: Level) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(level)))
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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
        signature: Vec::new(),
        level_arity: 0,
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

#[test]
fn a_universe_polymorphic_certificate_round_trips_and_re_verifies() {
    // `λ(A : Type u). λ(x : A). x : Π(A : Type u). Π(x : A). A` at level
    // arity 1: the certificate claims the judgment for every
    // instantiation of `u`, the wire carries the parameter, and the
    // kernel re-decides.
    let mut arena = TermArena::new();
    let type_u = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
    let bound = variable(&mut arena, 0);
    let inner = lambda(&mut arena, bound, bound);
    let identity = lambda(&mut arena, type_u, inner);
    let codomain_domain = variable(&mut arena, 0);
    let codomain_body = variable(&mut arena, 1);
    let codomain = pi(&mut arena, codomain_domain, codomain_body);
    let expected = pi(&mut arena, type_u, codomain);
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 1,
        context: Vec::new(),
        term: identity,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(decoded.certificate.level_arity, 1);
    verify(&mut decoded).expect("the polymorphic judgment must re-check");

    // The arity is part of the checked judgment: zero it on the wire and
    // the same term table decodes to a certificate the kernel refuses —
    // `u` names nothing in a closed judgment.
    let mut forged = bytes.clone();
    forged[10..14].copy_from_slice(&0_u32.to_le_bytes());
    let mut decoded = decode_mathematical_certificate(&forged).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::UnboundLevelParameter { index: 0, arity: 0 })
    );
}

#[test]
fn compound_level_syntax_survives_the_wire_and_the_kernel() {
    // Γ = A : Type max(u+1, v), x : A ⊢ x : A at arity 2 — a binding
    // whose sort is a genuine level expression, not a constant. The
    // certificate's scope is exactly its level arity.
    let mut arena = TermArena::new();
    let level = Level::Parameter(0)
        .successor()
        .unwrap()
        .maximum(Level::Parameter(1));
    let binding_type = arena.insert(Term::Sort(Sort::Type(level)));
    let bound = variable(&mut arena, 0);
    let shifted = variable(&mut arena, 1);
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 2,
        context: vec![binding_type, bound],
        term: bound,
        expected: shifted,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("the polymorphic context must re-check");

    // One parameter fewer and the binding's `v` is out of scope — the
    // arity is checked, not asserted.
    let mut forged = bytes.clone();
    forged[10..14].copy_from_slice(&1_u32.to_le_bytes());
    let mut decoded = decode_mathematical_certificate(&forged).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::UnboundLevelParameter { index: 1, arity: 1 })
    );
}

// ── Declarations on the wire ───────────────────────────────────────────

/// `polyId : Π(A : Type u). Π(x : A). A := λA. λx. x` at level arity 1.
fn polymorphic_identity_declaration(arena: &mut TermArena) -> Declaration {
    let type_u = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
    let bound_a = variable(arena, 0);
    let inner_a = variable(arena, 1);
    let inner_pi = pi(arena, bound_a, inner_a);
    let ty = pi(arena, type_u, inner_pi);
    let type_u = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
    let bound_a = variable(arena, 0);
    let inner_x = variable(arena, 0);
    let inner = lambda(arena, bound_a, inner_x);
    let body = lambda(arena, type_u, inner);
    Declaration::definition(1, ty, body)
}

/// `Π(A : Type u). Π(x : A). A` instantiated at a level expression.
fn instantiated_identity_type(arena: &mut TermArena, u: Level) -> TermHandle {
    let type_u = arena.insert(Term::Sort(Sort::Type(u)));
    let bound_a = variable(arena, 0);
    let inner_a = variable(arena, 1);
    let inner_pi = pi(arena, bound_a, inner_a);
    pi(arena, type_u, inner_pi)
}

#[test]
fn a_declaration_certificate_verifies_end_to_end() {
    let mut arena = TermArena::new();
    let declaration = polymorphic_identity_declaration(&mut arena);
    // `polyId([max(u, 1)])` under judgment arity 1 — the constant's level
    // argument is an expression, not just a parameter or constant.
    let level = Level::Parameter(0).maximum(Level::Constant(1));
    let evidence = arena.insert(Term::Constant {
        declaration: 0,
        levels: vec![level.clone()],
    });
    let expected = instantiated_identity_type(&mut arena, level);
    let certificate = MathematicalCertificate {
        signature: vec![declaration],
        level_arity: 1,
        context: Vec::new(),
        term: evidence,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    // The signature survives the wire exactly: the constant still resolves
    // and the decoded bytes re-encode identically (canonicality already
    // decided that on decode).
    assert_eq!(decoded.certificate.signature.len(), 1);
    verify(&mut decoded).expect("the instantiated declaration must re-check");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode"),
        bytes
    );
}

#[test]
fn a_forged_constant_index_never_verifies() {
    let mut arena = TermArena::new();
    let declaration = polymorphic_identity_declaration(&mut arena);
    // The signature carries one declaration; `Constant 1` names nothing.
    let forged = arena.insert(Term::Constant {
        declaration: 1,
        levels: vec![Level::Constant(0)],
    });
    let expected = instantiated_identity_type(&mut arena, Level::Constant(0));
    let certificate = MathematicalCertificate {
        signature: vec![declaration],
        level_arity: 0,
        context: Vec::new(),
        term: forged,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::UnknownDeclaration {
            declaration: 1,
            signature_len: 1,
        })
    );
}

#[test]
fn a_wrong_level_argument_count_never_verifies() {
    let mut arena = TermArena::new();
    let declaration = polymorphic_identity_declaration(&mut arena);
    // `polyId` takes one level; supplying none is a malformed constant.
    let evidence = arena.insert(Term::Constant {
        declaration: 0,
        levels: Vec::new(),
    });
    let expected = instantiated_identity_type(&mut arena, Level::Constant(0));
    let certificate = MathematicalCertificate {
        signature: vec![declaration],
        level_arity: 0,
        context: Vec::new(),
        term: evidence,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::DeclarationArityMismatch {
            declaration: 0,
            expected: 1,
            supplied: 0,
        })
    );
}

#[test]
fn an_assumption_chain_verifies_and_stays_unfolded() {
    let mut arena = TermArena::new();
    // `axiom : Type 0` (assumption), `uses : Type 0 := axiom`
    // (definition). The evidence names `uses` alone; the judgment
    // commits to `axiom` through the signature.
    let type_zero = type_sort(&mut arena, 0);
    let axiom = Declaration::assumption(0, type_zero);
    let axiom_constant = arena.insert(Term::Constant {
        declaration: 0,
        levels: Vec::new(),
    });
    let type_zero = type_sort(&mut arena, 0);
    let uses = Declaration::definition(0, type_zero, axiom_constant);
    let evidence = arena.insert(Term::Constant {
        declaration: 1,
        levels: Vec::new(),
    });
    let type_zero = type_sort(&mut arena, 0);
    let certificate = MathematicalCertificate {
        signature: vec![axiom, uses],
        level_arity: 0,
        context: Vec::new(),
        term: evidence,
        expected: type_zero,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(decoded.certificate.signature.len(), 2);
    verify(&mut decoded).expect("the assumption chain must re-check");
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [0].into_iter().collect()
    );
}

#[test]
fn a_signature_with_a_self_reference_never_verifies() {
    let mut arena = TermArena::new();
    // `loop : Π(_ : Type 0). loop` — the statement names its own
    // position. The bytes encode fine; verification rejects it because
    // the signature only holds the checked prefix.
    let type_zero = type_sort(&mut arena, 0);
    let self_index = arena.insert(Term::Constant {
        declaration: 0,
        levels: Vec::new(),
    });
    let statement = pi(&mut arena, type_zero, self_index);
    let self_referential = Declaration::assumption(0, statement);
    let evidence = arena.insert(Term::Constant {
        declaration: 0,
        levels: Vec::new(),
    });
    let expected = pi(&mut arena, type_zero, self_index);
    let certificate = MathematicalCertificate {
        signature: vec![self_referential],
        level_arity: 0,
        context: Vec::new(),
        term: evidence,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::UnknownDeclaration {
            declaration: 0,
            signature_len: 0,
        })
    );
}

#[test]
fn a_truncated_signature_section_never_decodes() {
    let mut arena = TermArena::new();
    let declaration = polymorphic_identity_declaration(&mut arena);
    let evidence = arena.insert(Term::Constant {
        declaration: 0,
        levels: vec![Level::Constant(0)],
    });
    let expected = instantiated_identity_type(&mut arena, Level::Constant(0));
    let certificate = MathematicalCertificate {
        signature: vec![declaration],
        level_arity: 0,
        context: Vec::new(),
        term: evidence,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    // Dropping the declaration's trailing bytes truncates the signature
    // section mid-record.
    for cut in [bytes.len() - 13, bytes.len() - 9, bytes.len() - 5] {
        assert!(
            decode_mathematical_certificate(&bytes[..cut]).is_err(),
            "a truncated signature record must not decode"
        );
    }
}

// ── A length-indexed family on the wire ────────────────────────────────
//
// The derived indexed scheme's first real consumer: `Vec n` over the
// assumed `Nat`/`Elem`. The whole construction — five scheme
// definitions, four assumptions, the `caseTwo`-computed description,
// the `isup` constructor application — crosses the wire as data and the
// receiver re-decides the signature and the judgment together.

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Sigma { domain, codomain })
}

fn pair(arena: &mut TermArena, first: TermHandle, second: TermHandle) -> TermHandle {
    arena.insert(Term::Pair { first, second })
}

fn fst(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair })
}

fn snd(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Snd { pair })
}

fn two(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Two)
}

fn two_zero(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoZero)
}

fn two_one(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoOne)
}

fn case_two(
    arena: &mut TermArena,
    motive: TermHandle,
    zero_branch: TermHandle,
    one_branch: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::CaseTwo {
        motive,
        zero_branch,
        one_branch,
        scrutinee,
    })
}

fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

/// The vector assumptions' positions after the scheme's five
/// declarations.
const NAT: u32 = 5;
const NAT_ZERO: u32 = 6;
const NAT_SUCC: u32 = 7;
const ELEM: u32 = 8;

fn nat(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT)
}

fn nat_zero(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT_ZERO)
}

fn nat_succ(arena: &mut TermArena, predecessor: TermHandle) -> TermHandle {
    let succ = constant(arena, NAT_SUCC);
    apply(arena, succ, predecessor)
}

fn elem(arena: &mut TermArena) -> TermHandle {
    constant(arena, ELEM)
}

/// `λ(_ : Two). Type 0` — the constant motive selecting types by tag.
fn type_motive(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let codomain = type_sort(arena, 0);
    lambda(arena, domain, codomain)
}

/// `Id Two zero one` — no closed inhabitant: the dead child position.
fn empty_positions(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_one(arena);
    id(arena, ty, left, right)
}

/// `Id Two zero zero` — the single live child position.
fn unit_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_zero(arena);
    id(arena, ty, left, right)
}

/// `Payload = λ(tag : Two). caseTwo(λ_.Type u, E, Σ(e : E). Nat, tag)` —
/// nil's dummy payload is the element type itself and cons's is
/// `⟨element, predecessor⟩`. `element` is any `Type u` expression valid
/// where the returned handle lands — a `Constant` like `Elem` (shifting
/// it is a no-op) or the bound `E` inside a polymorphic declaration —
/// and `level` the universe it lives at, `Parameter(0)` included.
fn payload(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let motive = {
        let domain = two(arena);
        let codomain = sort_level(arena, level.clone());
        lambda(arena, domain, codomain)
    };
    // Under `λ(tag)` the element type is one binder deeper.
    let nil_payload = shift(arena, element, 0, 1);
    let cons_payload = {
        let element = shift(arena, element, 0, 1);
        let length = nat(arena);
        sigma(arena, element, length)
    };
    let tag = variable(arena, 0);
    let body = case_two(arena, motive, nil_payload, cons_payload, tag);
    let domain = two(arena);
    lambda(arena, domain, body)
}

/// `A = Σ(tag : Two). Payload tag` — the carrier lives at `Type u`.
fn carrier(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let tag = variable(arena, 0);
    let element = shift(arena, element, 0, 1);
    let payload_fn = payload(arena, element, level);
    let codomain = apply(arena, payload_fn, tag);
    let domain = two(arena);
    sigma(arena, domain, codomain)
}

/// `B a = caseTwo(λ_.Type 0, Id Two zero one, Id Two zero zero, fst a)`
/// — child positions stay at `Type 0` whatever `u` is.
fn branching(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let motive = type_motive(arena);
    let empty = empty_positions(arena);
    let unit = unit_position(arena);
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let body = case_two(arena, motive, empty, unit, tag);
    let domain = carrier(arena, element, level);
    lambda(arena, domain, body)
}

/// `out a = caseTwo(Π(_:Payload _).Nat, λ_.zeroN, λp. succN (snd p),
/// fst a) (snd a)` — `nil ↦ zeroN`, `cons ⟨e, n⟩ ↦ succN n`.
fn out_index(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let motive = {
        let tag = variable(arena, 0);
        let element = shift(arena, element, 0, 2);
        let payload_fn = payload(arena, element, level);
        let payload_at = apply(arena, payload_fn, tag);
        let length = nat(arena);
        let codomain = pi(arena, payload_at, length);
        let domain = two(arena);
        lambda(arena, domain, codomain)
    };
    let zero_branch = {
        // `λ(_ : E). zeroN` checked at `Π(_ : Payload zero ≡ E). Nat`.
        let domain = shift(arena, element, 0, 1);
        let zero = nat_zero(arena);
        lambda(arena, domain, zero)
    };
    let one_branch = {
        let domain = {
            let element = shift(arena, element, 0, 1);
            let length = nat(arena);
            sigma(arena, element, length)
        };
        let bound = variable(arena, 0);
        let predecessor = snd(arena, bound);
        let body = nat_succ(arena, predecessor);
        lambda(arena, domain, body)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload_component = snd(arena, bound);
    let body = apply(arena, selected, payload_component);
    let domain = carrier(arena, element, level);
    lambda(arena, domain, body)
}

/// `next a = caseTwo(Π(p:Payload _).Π(_:B' _).Nat, λp.λ_.zeroN,
/// λp.λ_.snd p, fst a) (snd a)` — the cons child's required index is
/// the recorded predecessor.
fn next_index(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let motive = {
        let tag = variable(arena, 0);
        let element = shift(arena, element, 0, 2);
        let payload_fn = payload(arena, element, level);
        let payload_at = apply(arena, payload_fn, tag);
        let branching_at = {
            // Under the `p` binder the tag is index 1.
            let motive = type_motive(arena);
            let empty = empty_positions(arena);
            let unit = unit_position(arena);
            let tag = variable(arena, 1);
            case_two(arena, motive, empty, unit, tag)
        };
        let length = nat(arena);
        let inner = pi(arena, branching_at, length);
        let body = pi(arena, payload_at, inner);
        let domain = two(arena);
        lambda(arena, domain, body)
    };
    let zero_branch = {
        let payload_domain = shift(arena, element, 0, 1);
        let position_domain = empty_positions(arena);
        let body = nat_zero(arena);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let one_branch = {
        let payload_domain = {
            let element = shift(arena, element, 0, 1);
            let length = nat(arena);
            sigma(arena, element, length)
        };
        let position_domain = unit_position(arena);
        let bound = variable(arena, 1);
        let body = snd(arena, bound);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload_component = snd(arena, bound);
    let body = apply(arena, selected, payload_component);
    let domain = carrier(arena, element, level);
    lambda(arena, domain, body)
}

/// The vector family at element type `element : Type element_level`:
/// `IW[0, u, 0] Nat (Σ(t : Two). Payload t) B out next`.
fn vector_family(
    arena: &mut TermArena,
    element: TermHandle,
    element_level: Level,
) -> IndexedFamily {
    IndexedFamily {
        levels: [
            Level::Constant(0),
            element_level.clone(),
            Level::Constant(0),
        ],
        index: nat(arena),
        carrier: carrier(arena, element, &element_level),
        children: branching(arena, element, &element_level),
        out: out_index(arena, element, &element_level),
        next: next_index(arena, element, &element_level),
    }
}

/// The nine-declaration producer signature: the derived scheme then
/// `Nat`, `zeroN`, `succN`, `Elem` as assumptions.
fn vector_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Nat
    declarations.push(Declaration::assumption(0, nat(arena))); // zeroN
    let succ_domain = nat(arena);
    let succ_codomain = nat(arena);
    let succ_statement = pi(arena, succ_domain, succ_codomain);
    declarations.push(Declaration::assumption(0, succ_statement)); // succN
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Elem
    declarations
}

/// `⟨one, ⟨e, n⟩⟩` — a cons node recording element and predecessor.
fn cons_node(arena: &mut TermArena, element: TermHandle, predecessor: TermHandle) -> TermHandle {
    let payload_value = pair(arena, element, predecessor);
    let tag = two_one(arena);
    pair(arena, tag, payload_value)
}

#[test]
fn a_length_indexed_vector_certificate_verifies_end_to_end() {
    let mut arena = TermArena::new();
    // Γ = e : Elem, n : Nat, tail : Vec n proves
    //   `cons e n tail := isup ⟨one, ⟨e, n⟩⟩ (λ_. tail) : Vec (succN n)`
    // — the derived indexed family, its constructor and the computed
    // length index all travel as data; the kernel re-decides after
    // decode.
    let element = elem(&mut arena);
    let family = vector_family(&mut arena, element, Level::Constant(0));
    let declarations = vector_declarations(&mut arena);
    let tail_type = {
        let length = variable(&mut arena, 0);
        family.indexed_w(&mut arena, length)
    };
    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);
    let expected = {
        let predecessor = variable(&mut arena, 1);
        let successor = nat_succ(&mut arena, predecessor);
        family.indexed_w(&mut arena, successor)
    };
    let element_type = elem(&mut arena);
    let index_type = nat(&mut arena);
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![element_type, index_type, tail_type],
        term: cons,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    // All nine declarations — the scheme's definitions and the vector
    // assumptions — survive as signature data.
    assert_eq!(decoded.certificate.signature.len(), 9);
    verify(&mut decoded).expect("the length-indexed judgment must re-verify");

    // Re-encoding the decoded judgment reproduces the canonical bytes.
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // The exact assumption record travels too: the judgment commits to
    // `Nat`, `zeroN`, `succN`, `Elem` — positions 5–8 — and to none of
    // the scheme's definitions.
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [NAT, NAT_ZERO, NAT_SUCC, ELEM].into_iter().collect()
    );

    // Claiming the tail's own length is a different, false judgment.
    let mut arena = TermArena::new();
    let element = elem(&mut arena);
    let family = vector_family(&mut arena, element, Level::Constant(0));
    let declarations = vector_declarations(&mut arena);
    let tail_type = {
        let length = variable(&mut arena, 0);
        family.indexed_w(&mut arena, length)
    };
    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);
    let forged = {
        let predecessor = variable(&mut arena, 1);
        family.indexed_w(&mut arena, predecessor)
    };
    let element_type = elem(&mut arena);
    let index_type = nat(&mut arena);
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![element_type, index_type, tail_type],
        term: cons,
        expected: forged,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

// ── Level instantiation on the wire ────────────────────────────────────
//
// The producer's `vecOf`/`consVec` declarations are polymorphic over the
// element universe: the certificate's context, term and claimed type all
// speak `Level::Parameter(0)`, and the scheme constants inside the
// declaration bodies carry the parameter in their level arguments. The
// wire therefore exercises level data in three places — certificate
// arity, declaration arities, and the `Constant` level vectors inside
// the term table.

/// `vecOf : Π(E : Type u). Π(n : Nat). Type u` — declaration 8 of the
/// level-parameterized signature.
const VEC_OF: u32 = 8;
/// `consVec : Π(E : Type u). Π(e : E). Π(n : Nat). Π(t : vecOf u E n).
/// vecOf u E (succN n)` — declaration 9.
const CONS_VEC: u32 = 9;

fn constant_levels(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

/// `vecOf[u] · E · n` — the declared family applied at instantiation `u`.
fn vec_at(arena: &mut TermArena, u: Level, element: TermHandle, length: TermHandle) -> TermHandle {
    let family = constant_levels(arena, VEC_OF, vec![u]);
    let applied = apply(arena, family, element);
    apply(arena, applied, length)
}

/// `consVec[u] · E · e · n · t` — the declared constructor at `u`.
fn cons_at(
    arena: &mut TermArena,
    u: Level,
    element: TermHandle,
    member: TermHandle,
    length: TermHandle,
    tail: TermHandle,
) -> TermHandle {
    let constructor = constant_levels(arena, CONS_VEC, vec![u]);
    let applied = apply(arena, constructor, element);
    let applied = apply(arena, applied, member);
    let applied = apply(arena, applied, length);
    apply(arena, applied, tail)
}

/// `vecOf : Π(E : Type u). Π(n : Nat). Type u
///       := λE. λn. IW[0, u, 0] Nat (A E) (B E) (out E) (next E) n`
/// — `u` is `Parameter(0)`, including inside the scheme constant's
/// level arguments.
fn vec_of_declaration(arena: &mut TermArena) -> Declaration {
    let statement = {
        let type_u = sort_level(arena, Level::Parameter(0));
        let index = nat(arena);
        let codomain = sort_level(arena, Level::Parameter(0));
        let inner = pi(arena, index, codomain);
        pi(arena, type_u, inner)
    };
    let body = {
        // Under `λE. λn`: E is index 1, n is index 0.
        let type_u = sort_level(arena, Level::Parameter(0));
        let element = variable(arena, 1);
        let family = vector_family(arena, element, Level::Parameter(0));
        let index = nat(arena);
        let length = variable(arena, 0);
        let applied = family.indexed_w(arena, length);
        let inner = lambda(arena, index, applied);
        lambda(arena, type_u, inner)
    };
    Declaration::definition(1, statement, body)
}

/// `consVec : Π(E : Type u). Π(e : E). Π(n : Nat). Π(t : vecOf u E n).
///              vecOf u E (succN n)
///          := λE. λe. λn. λt. isup[0, u, 0] … ⟨one, ⟨e, n⟩⟩ (λ_. t)`.
fn cons_vec_declaration(arena: &mut TermArena) -> Declaration {
    let statement = {
        let type_u = sort_level(arena, Level::Parameter(0));
        let e_domain = variable(arena, 0);
        let index = nat(arena);
        let tail_type = {
            // Under E, e, n: E is index 2, n is index 0.
            let element = variable(arena, 2);
            let length = variable(arena, 0);
            vec_at(arena, Level::Parameter(0), element, length)
        };
        let codomain = {
            // Under the tail binder: E is index 3, n is index 1.
            let element = variable(arena, 3);
            let predecessor = variable(arena, 1);
            let successor = nat_succ(arena, predecessor);
            vec_at(arena, Level::Parameter(0), element, successor)
        };
        let over_tail = pi(arena, tail_type, codomain);
        let over_length = pi(arena, index, over_tail);
        let over_element = pi(arena, e_domain, over_length);
        pi(arena, type_u, over_element)
    };
    let body = {
        let type_u = sort_level(arena, Level::Parameter(0));
        // Under `λE. λe. λn. λt`: E is 3, e is 2, n is 1, t is 0.
        let element = variable(arena, 3);
        let family = vector_family(arena, element, Level::Parameter(0));
        let node = {
            let member = variable(arena, 2);
            let predecessor = variable(arena, 1);
            cons_node(arena, member, predecessor)
        };
        let children_fn = {
            let domain = apply(arena, family.children, node);
            let tail = variable(arena, 1);
            lambda(arena, domain, tail)
        };
        let applied = family.sup(arena, node, children_fn);
        let tail_domain = {
            // Under E, e, n: E is index 2, n is index 0.
            let element = variable(arena, 2);
            let length = variable(arena, 0);
            vec_at(arena, Level::Parameter(0), element, length)
        };
        let over_tail = lambda(arena, tail_domain, applied);
        let index = nat(arena);
        let over_length = lambda(arena, index, over_tail);
        let e_domain = variable(arena, 0);
        let over_element = lambda(arena, e_domain, over_length);
        lambda(arena, type_u, over_element)
    };
    Declaration::definition(1, statement, body)
}

/// The ten-declaration producer signature: the derived scheme, the
/// `Nat`/`zeroN`/`succN` assumptions, then `vecOf` and `consVec`.
fn level_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Nat
    declarations.push(Declaration::assumption(0, nat(arena))); // zeroN
    let succ_domain = nat(arena);
    let succ_codomain = nat(arena);
    let succ_statement = pi(arena, succ_domain, succ_codomain);
    declarations.push(Declaration::assumption(0, succ_statement)); // succN
    declarations.push(vec_of_declaration(arena)); // vecOf
    declarations.push(cons_vec_declaration(arena)); // consVec
    declarations
}

/// `E : Type u, e : E, n : Nat, t : vecOf[u] E n` — the context,
/// term `consVec[u] E e n t` and claim `vecOf[u] E (succN n)` of the
/// level-polymorphic vector certificate.
fn level_certificate(arena: &mut TermArena) -> MathematicalCertificate {
    let element_type = sort_level(arena, Level::Parameter(0));
    let member_type = variable(arena, 0);
    let index_type = nat(arena);
    let tail_type = {
        // Under E, e, n: E is index 2, n is index 0.
        let element = variable(arena, 2);
        let length = variable(arena, 0);
        vec_at(arena, Level::Parameter(0), element, length)
    };
    let term = {
        let element = variable(arena, 3);
        let member = variable(arena, 2);
        let length = variable(arena, 1);
        let tail = variable(arena, 0);
        cons_at(arena, Level::Parameter(0), element, member, length, tail)
    };
    let expected = {
        let element = variable(arena, 3);
        let predecessor = variable(arena, 1);
        let successor = nat_succ(arena, predecessor);
        vec_at(arena, Level::Parameter(0), element, successor)
    };
    MathematicalCertificate {
        signature: level_declarations(arena),
        level_arity: 1,
        context: vec![element_type, member_type, index_type, tail_type],
        term,
        expected,
    }
}

#[test]
fn a_level_polymorphic_indexed_certificate_round_trips_and_re_verifies() {
    let mut arena = TermArena::new();
    let certificate = level_certificate(&mut arena);
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

    // The receiver sees only bytes: the arity, the declaration table —
    // scheme definitions carrying `Parameter` levels in their constant
    // spines plus the three `Nat` assumptions and the two polymorphic
    // producer definitions — and the parameterized judgment all decode
    // to a certificate the kernel re-decides independently.
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(decoded.certificate.level_arity, 1);
    assert_eq!(decoded.certificate.signature.len(), 10);
    assert_eq!(
        decoded
            .certificate
            .signature
            .iter()
            .map(|declaration| declaration.level_arity)
            .collect::<Vec<_>>(),
        vec![3, 3, 3, 3, 4, 0, 0, 0, 1, 1]
    );
    verify(&mut decoded).expect("the polymorphic vector judgment must re-check");

    // Re-encoding the decoded judgment reproduces the canonical bytes:
    // the level parameters inside constant spines are data, not a
    // receiver-side reconstruction.
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // The judgment commits to exactly the three `Nat` assumptions —
    // `vecOf`, `consVec` and the scheme stay definitions.
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [NAT, NAT_ZERO, NAT_SUCC].into_iter().collect()
    );

    // The certificate's arity is part of the checked judgment: zero it
    // on the wire and the same table decodes to a certificate the
    // kernel refuses — `u` names nothing in a closed judgment.
    let mut forged = bytes.clone();
    forged[10..14].copy_from_slice(&0_u32.to_le_bytes());
    let mut decoded = decode_mathematical_certificate(&forged).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::UnboundLevelParameter { index: 0, arity: 0 })
    );
}

#[test]
fn a_squashed_indexed_family_certificate_round_trips_and_re_verifies() {
    // Γ = n : Nat, tail : Vec n proves `sq_{Vec n} tail : Squash (Vec
    // n)` — squashed existence over the derived indexed family: the
    // `Squash`/`SquashIntro` tags, the `IW` constant spine and the
    // vector producer assumptions all travel as data and the kernel
    // re-decides the strict-layer judgment after decode.
    let mut arena = TermArena::new();
    let element = elem(&mut arena);
    let family = vector_family(&mut arena, element, Level::Constant(0));
    let declarations = vector_declarations(&mut arena);
    let n_binding = nat(&mut arena);
    let tail_binding = {
        // `Vec n` under [n]: n is 0.
        let n = variable(&mut arena, 0);
        family.indexed_w(&mut arena, n)
    };
    let term = {
        // Under Γ: tail is 0, n is 1.
        let n = variable(&mut arena, 1);
        let carrier = family.indexed_w(&mut arena, n);
        let tail = variable(&mut arena, 0);
        squash_intro(&mut arena, carrier, tail)
    };
    let expected = {
        let n = variable(&mut arena, 1);
        let carrier = family.indexed_w(&mut arena, n);
        squash(&mut arena, carrier)
    };
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![n_binding, tail_binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("the squashed family judgment must re-check");
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [NAT, NAT_ZERO, NAT_SUCC, ELEM].into_iter().collect(),
        "the description constants keep the four producer assumptions in scope"
    );

    // The receiver re-decides: claiming the same evidence at the
    // un-squashed `Vec n` — asking the squash to leak its witness —
    // decodes fine and the kernel refuses it.
    let mut arena = TermArena::new();
    let element = elem(&mut arena);
    let family = vector_family(&mut arena, element, Level::Constant(0));
    let declarations = vector_declarations(&mut arena);
    let n_binding = nat(&mut arena);
    let tail_binding = {
        let n = variable(&mut arena, 0);
        family.indexed_w(&mut arena, n)
    };
    let term = {
        let n = variable(&mut arena, 1);
        let carrier = family.indexed_w(&mut arena, n);
        let tail = variable(&mut arena, 0);
        squash_intro(&mut arena, carrier, tail)
    };
    let wrong_claim = {
        let n = variable(&mut arena, 1);
        family.indexed_w(&mut arena, n)
    };
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![n_binding, tail_binding],
        term,
        expected: wrong_claim,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
}

fn empty(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Empty)
}

fn empty_elim(arena: &mut TermArena, ty: TermHandle, scrutinee: TermHandle) -> TermHandle {
    arena.insert(Term::EmptyElim { ty, scrutinee })
}

fn squash(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Squash { ty })
}

fn squash_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::SquashIntro { ty, value })
}

fn squash_elim(
    arena: &mut TermArena,
    proposition: TermHandle,
    function: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::SquashElim {
        proposition,
        function,
        scrutinee,
    })
}

fn boxed(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Box { ty })
}

fn box_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::BoxIntro { ty, value })
}

fn box_elim(
    arena: &mut TermArena,
    motive: TermHandle,
    body: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::BoxElim {
        motive,
        body,
        scrutinee,
    })
}

#[test]
fn strict_layer_certificates_round_trip_and_re_verify() {
    // Squashed existence itself: Γ = A : Type 0, a : A proves
    // `sq_A a : Squash A` — tags 23–24.
    let mut arena = TermArena::new();
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let term = {
        let carrier = variable(&mut arena, 1);
        let value = variable(&mut arena, 0);
        squash_intro(&mut arena, carrier, value)
    };
    let expected = {
        let carrier = variable(&mut arena, 1);
        squash(&mut arena, carrier)
    };
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 0,
        context: vec![type_zero, a_binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("squash introduction must re-check");
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // Squashed elimination: Γ = A : Type 0, a : A, P : Strict 0,
    // f : Π(_ : A). P proves `unsq P f (sq_A a) : P` — tag 25 plus the
    // `Strict` sort on the wire.
    let mut arena = TermArena::new();
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let strict_zero = strict_sort(&mut arena, 0);
    let f_binding = {
        // Π(_ : A). P under prefix [A, a, P]: A is 2, P is 0; under the
        // binder P is 1.
        let domain = variable(&mut arena, 2);
        let codomain = variable(&mut arena, 1);
        pi(&mut arena, domain, codomain)
    };
    let term = {
        // Under Γ: f is 0, P is 1, a is 2, A is 3.
        let proposition = variable(&mut arena, 1);
        let function = variable(&mut arena, 0);
        let carrier = variable(&mut arena, 3);
        let value = variable(&mut arena, 2);
        let witness = squash_intro(&mut arena, carrier, value);
        squash_elim(&mut arena, proposition, function, witness)
    };
    let expected = variable(&mut arena, 1);
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 0,
        context: vec![type_zero, a_binding, strict_zero, f_binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("squash elimination must re-check");
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // The receiver re-decides: claiming the relevant `A` instead of
    // `P` — asking the squash to leak a witness — decodes fine but the
    // kernel refuses it.
    let mut arena = TermArena::new();
    let type_zero = type_sort(&mut arena, 0);
    let a_binding = variable(&mut arena, 0);
    let strict_zero = strict_sort(&mut arena, 0);
    let f_binding = {
        let domain = variable(&mut arena, 2);
        let codomain = variable(&mut arena, 1);
        pi(&mut arena, domain, codomain)
    };
    let term = {
        let proposition = variable(&mut arena, 1);
        let function = variable(&mut arena, 0);
        let carrier = variable(&mut arena, 3);
        let value = variable(&mut arena, 2);
        let witness = squash_intro(&mut arena, carrier, value);
        squash_elim(&mut arena, proposition, function, witness)
    };
    let wrong_claim = variable(&mut arena, 3);
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 0,
        context: vec![type_zero, a_binding, strict_zero, f_binding],
        term,
        expected: wrong_claim,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Ex falso: Γ = A : Type 0, e : sEmpty proves
    // `sEmpty_rect A e : A` — tags 21–22.
    let mut arena = TermArena::new();
    let type_zero = type_sort(&mut arena, 0);
    let empty_binding = empty(&mut arena);
    let term = {
        let target = variable(&mut arena, 1);
        let scrutinee = variable(&mut arena, 0);
        empty_elim(&mut arena, target, scrutinee)
    };
    let expected = variable(&mut arena, 1);
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 0,
        context: vec![type_zero, empty_binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("empty elimination must re-check");
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);
}

#[test]
fn a_box_elimination_certificate_round_trips_and_re_verifies() {
    // Γ = P : Strict 0, p : P, C : Π(_ : Box P). Type 0,
    // g : Π(a : P). C (box_P a), b : Box P proves `unbox C g b : C b` —
    // tags 26–28.
    let mut arena = TermArena::new();
    let strict_zero = strict_sort(&mut arena, 0);
    let p_binding = variable(&mut arena, 0);
    let c_binding = {
        // Π(_ : Box P). Type 0 under prefix [P, p]: P is 1.
        let payload = variable(&mut arena, 1);
        let domain = boxed(&mut arena, payload);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let g_binding = {
        // Π(a : P). C (box_P a) under prefix [P, p, C]: P is 2, C is 1;
        // under the a binder C is index 1, P is index 3, a is index 0.
        let domain = variable(&mut arena, 2);
        let codomain = {
            let payload = variable(&mut arena, 3);
            let bound = variable(&mut arena, 0);
            let boxed_a = box_intro(&mut arena, payload, bound);
            let family = variable(&mut arena, 1);
            apply(&mut arena, family, boxed_a)
        };
        pi(&mut arena, domain, codomain)
    };
    let b_binding = {
        // Under prefix [P, p, C, g]: P is 3.
        let payload = variable(&mut arena, 3);
        boxed(&mut arena, payload)
    };
    let term = {
        // Under Γ: b is 0, g is 1, C is 2.
        let motive = variable(&mut arena, 2);
        let body = variable(&mut arena, 1);
        let scrutinee = variable(&mut arena, 0);
        box_elim(&mut arena, motive, body, scrutinee)
    };
    let expected = {
        let family = variable(&mut arena, 2);
        let argument = variable(&mut arena, 0);
        apply(&mut arena, family, argument)
    };
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 0,
        context: vec![strict_zero, p_binding, c_binding, g_binding, b_binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("box elimination must re-check");
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // `unbox C g b` cannot be claimed at `C (box_P p)`: the neutral
    // `b` stays distinct from the canonical `box_P p` at the relevant
    // `Box P`, so the producer cannot smuggle in a different landing.
    let mut arena = TermArena::new();
    let strict_zero = strict_sort(&mut arena, 0);
    let p_binding = variable(&mut arena, 0);
    let c_binding = {
        let payload = variable(&mut arena, 1);
        let domain = boxed(&mut arena, payload);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let g_binding = {
        let domain = variable(&mut arena, 2);
        let codomain = {
            let payload = variable(&mut arena, 3);
            let bound = variable(&mut arena, 0);
            let boxed_a = box_intro(&mut arena, payload, bound);
            let family = variable(&mut arena, 1);
            apply(&mut arena, family, boxed_a)
        };
        pi(&mut arena, domain, codomain)
    };
    let b_binding = {
        let payload = variable(&mut arena, 3);
        boxed(&mut arena, payload)
    };
    let term = {
        let motive = variable(&mut arena, 2);
        let body = variable(&mut arena, 1);
        let scrutinee = variable(&mut arena, 0);
        box_elim(&mut arena, motive, body, scrutinee)
    };
    let wrong_claim = {
        // `C (box_P p)` — under Γ: p is 3, P is 4.
        let payload = variable(&mut arena, 4);
        let value = variable(&mut arena, 3);
        let boxed_p = box_intro(&mut arena, payload, value);
        let family = variable(&mut arena, 2);
        apply(&mut arena, family, boxed_p)
    };
    let certificate = MathematicalCertificate {
        signature: Vec::new(),
        level_arity: 0,
        context: vec![strict_zero, p_binding, c_binding, g_binding, b_binding],
        term,
        expected: wrong_claim,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

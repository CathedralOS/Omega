//! Published theorems crossing the certification bridge: the producer's
//! signature carries the derived indexed scheme plus *proved*
//! declarations — `subst` (identity transport over an arbitrary
//! predicate) and `indexCorrect` (the indexed family's index-soundness
//! theorem, proved by the derived `iindW` eliminator) — and the consumer
//! judgment cites them by `Constant` application. Verification after
//! decode re-decides each theorem's own proof and the citing judgment;
//! the assumption closure is computed over the stored signature, never
//! from which constants conversion happened to unfold, so a theorem
//! whose statement references an axiom-dependent definition retains that
//! axiom for a receiver's admission policy.

use proof_admission::{
    Budget, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
    MathematicalCertificate, Sort, Term, TermArena, TermHandle, identity_substitution,
    indexed_correctness, indexed_scheme, verify_mathematical_certificate,
};
use terminal_codec::{decode_mathematical_certificate, encode_mathematical_certificate};

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

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

fn fst(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair })
}

fn w(arena: &mut TermArena, carrier: TermHandle, children: TermHandle) -> TermHandle {
    arena.insert(Term::W { carrier, children })
}

fn indw(
    arena: &mut TermArena,
    motive: TermHandle,
    step: TermHandle,
    tree: TermHandle,
) -> TermHandle {
    arena.insert(Term::IndW { motive, step, tree })
}

fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
}

fn two(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Two)
}

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

fn constant_levels(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

fn verify(decoded: &mut terminal_codec::DecodedMathematicalCertificate) -> Result<(), CoreError> {
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
}

// ── The producer signature ─────────────────────────────────────────────
//
// Positions 0–4 are the derived indexed scheme; 5 and 6 are the proved
// theorems; 7–10 are the axiom scenario: `Elem` is a named assumption,
// `Box` a definition whose body names it, and `boxRefl` a theorem whose
// statement references `Box` while its proof never unfolds it.

/// `subst` — identity transport — is declaration 5.
const SUBST: u32 = 5;
/// `indexCorrect` — the indexed scheme's index-soundness theorem — is
/// declaration 6.
const INDEX_CORRECT: u32 = 6;
/// `Elem : Type 0`, a named assumption — declaration 7.
const ELEM: u32 = 7;
/// `e0 : Elem`, a named element assumption — declaration 8.
const ELEM_ZERO: u32 = 8;
/// `Box : Type 0 := Elem` — a definition whose meaning is an axiom —
/// declaration 9.
const BOX: u32 = 9;
/// `boxRefl : Π(x : Box). Id Box x x := λx. refl Box x` — a theorem
/// whose statement and body both reference `Box` without unfolding it —
/// declaration 10.
const BOX_REFL: u32 = 10;

fn elem(arena: &mut TermArena) -> TermHandle {
    constant(arena, ELEM)
}

fn elem_zero(arena: &mut TermArena) -> TermHandle {
    constant(arena, ELEM_ZERO)
}

/// `boxRefl : Π(x : Box). Id Box x x := λx. refl Box x`. Checking the
/// body never unfolds `Box`: `x`'s binding is `Constant BOX` and the
/// `refl` annotation is `Constant BOX`, so every comparison is the
/// reflexive one — yet `Elem` remains in the declaration's closure
/// through `Box`'s stored body.
fn box_refl_declaration(arena: &mut TermArena) -> Declaration {
    let statement = {
        let domain = constant(arena, BOX);
        let ty = constant(arena, BOX);
        let bound = variable(arena, 0);
        let body = id(arena, ty, bound, bound);
        pi(arena, domain, body)
    };
    let body = {
        let domain = constant(arena, BOX);
        let ty = constant(arena, BOX);
        let bound = variable(arena, 0);
        let inner = refl(arena, ty, bound);
        lambda(arena, domain, inner)
    };
    Declaration::definition(0, statement, body)
}

/// The eleven-declaration producer signature: the scheme, the two
/// theorems, then `Elem`, `e0`, `Box`, `boxRefl`.
fn producer_signature(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(identity_substitution(arena)); // subst
    declarations.push(indexed_correctness(arena)); // indexCorrect
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Elem
    declarations.push(Declaration::assumption(0, elem(arena))); // e0
    declarations.push(Declaration::definition(0, type_sort(arena, 0), elem(arena))); // Box
    declarations.push(box_refl_declaration(arena)); // boxRefl
    declarations
}

/// The same signature with `Elem` — position 7 — replaced by the
/// definition `Elem : Type 0 := Two`: every declaration still checks and
/// every judgment still verifies, but the axiom's meaning changed and so
/// does the exact assumption closure a receiver computes.
fn defect_signature(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(identity_substitution(arena));
    declarations.push(indexed_correctness(arena));
    declarations.push(Declaration::definition(0, type_sort(arena, 0), two(arena))); // Elem := Two
    declarations.push(Declaration::assumption(0, elem(arena))); // e0
    declarations.push(Declaration::definition(0, type_sort(arena, 0), elem(arena))); // Box
    declarations.push(box_refl_declaration(arena)); // boxRefl
    declarations
}

// ── The instantiated description ───────────────────────────────────────
//
// The theorem certificates run the indexed family at the closed
// description `I = Elem, A = Two, B = λ_.Two, out = λ_.e0,
// next = λa.λ_.e0` — every tree's root index and every child's required
// index is the assumed element `e0`, so `IW i` forces `i ≡ e0` and the
// theorem's conclusion `Id Elem (out (rootOf t)) i` is the claim
// `Id Elem e0 i` up to one β step.

/// `B₀ = λ(_ : Two). Two` — the constant child-position family.
fn b_children(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let body = two(arena);
    lambda(arena, domain, body)
}

/// `out₀ = λ(_ : Two). e0` — every node's index is `e0`.
fn out_index(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let body = elem_zero(arena);
    lambda(arena, domain, body)
}

/// `next₀ = λ(_ : Two). λ(_ : Two). e0` — every child requires `e0`.
fn next_index(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let inner_domain = two(arena);
    let body = elem_zero(arena);
    let inner = lambda(arena, inner_domain, body);
    lambda(arena, domain, inner)
}

/// `IW[0,0,0] Elem Two B₀ out₀ next₀` applied to an index handle.
fn indexed_w_at(arena: &mut TermArena, index: TermHandle) -> TermHandle {
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: elem(arena),
        carrier: two(arena),
        children: b_children(arena),
        out: out_index(arena),
        next: next_index(arena),
    }
    .indexed_w(arena, index)
}

/// `indexCorrect[0,0,0] · Elem · Two · B₀ · out₀ · next₀ · i · t` —
/// the theorem application citing declaration 6.
fn index_correct_at(arena: &mut TermArena, index: TermHandle, tree: TermHandle) -> TermHandle {
    let theorem = constant_levels(
        arena,
        INDEX_CORRECT,
        vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
    );
    let carrier_type = elem(arena);
    let spine = apply(arena, theorem, carrier_type);
    let labels = two(arena);
    let spine = apply(arena, spine, labels);
    let positions = b_children(arena);
    let spine = apply(arena, spine, positions);
    let index_of = out_index(arena);
    let spine = apply(arena, spine, index_of);
    let required = next_index(arena);
    let spine = apply(arena, spine, required);
    let spine = apply(arena, spine, index);
    apply(arena, spine, tree)
}

/// `out₀ (indW(λ(_ : W Two B₀). Two, λa.λk.λih.a, fst tree))` — the
/// root label's index, matching the instantiated `indexCorrect`
/// statement's left endpoint up to conversion. `tree` is a bound
/// variable of type `IW[0,0,0] i`.
fn root_index(arena: &mut TermArena, tree: TermHandle) -> TermHandle {
    let carrier = two(arena);
    let positions_family = b_children(arena);
    let w_type = w(arena, carrier, positions_family);
    let codomain = two(arena);
    let motive = lambda(arena, w_type, codomain);
    let step = {
        // `λ(a : Two). λ(k : Π(b : B₀ a). W Two B₀). λ(_ : Π(_ : B₀ a).
        // Two). a` — the constant-motive hypothesis `Π(b : B₀ a). (λ_.Two)
        // (k b)` written β-reduced, as in the theorem's own `root_step`.
        let a_domain = two(arena);
        let k_domain = {
            let family = b_children(arena);
            let label = variable(arena, 0);
            let positions = apply(arena, family, label);
            let carrier = two(arena);
            let family = b_children(arena);
            let codomain = w(arena, carrier, family);
            pi(arena, positions, codomain)
        };
        let ih_domain = {
            let family = b_children(arena);
            let label = variable(arena, 1);
            let positions = apply(arena, family, label);
            let codomain = two(arena);
            pi(arena, positions, codomain)
        };
        let root = variable(arena, 2);
        let inner = lambda(arena, ih_domain, root);
        let inner = lambda(arena, k_domain, inner);
        lambda(arena, a_domain, inner)
    };
    let projected = fst(arena, tree);
    let induction = indw(arena, motive, step, projected);
    let index_of = out_index(arena);
    apply(arena, index_of, induction)
}

/// `subst[0,0] · Elem · P · X · i · prf · h` — the transport application
/// citing declaration 5.
fn subst_at(
    arena: &mut TermArena,
    predicate: TermHandle,
    source: TermHandle,
    target: TermHandle,
    proof: TermHandle,
    hypothesis: TermHandle,
) -> TermHandle {
    let theorem = constant_levels(arena, SUBST, vec![Level::Constant(0), Level::Constant(0)]);
    let carrier_type = elem(arena);
    let spine = apply(arena, theorem, carrier_type);
    let spine = apply(arena, spine, predicate);
    let spine = apply(arena, spine, source);
    let spine = apply(arena, spine, target);
    let spine = apply(arena, spine, proof);
    apply(arena, spine, hypothesis)
}

#[test]
fn a_theorem_certificate_verifies_after_wire_decode() {
    let mut arena = TermArena::new();
    // Γ = i : Elem, t : IW[0,0,0] i proves
    //   `indexCorrect[0,0,0] Elem Two B₀ out₀ next₀ i t : Id Elem e0 i`
    // — a published theorem applied to bound evidence. The claimed type
    // is the reduced claim: the statement's left endpoint `out₀ (rootOf
    // t)` is the unreduced `Apply`, so the receiver decides the claim
    // through conversion, not by matching the producer's spelling.
    let index_binding = elem(&mut arena);
    let tree_index = variable(&mut arena, 0);
    let tree_binding = indexed_w_at(&mut arena, tree_index);
    let index = variable(&mut arena, 1);
    let tree = variable(&mut arena, 0);
    let term = index_correct_at(&mut arena, index, tree);
    let expected = {
        let ty = elem(&mut arena);
        let left = elem_zero(&mut arena);
        let right = variable(&mut arena, 1);
        id(&mut arena, ty, left, right)
    };
    let certificate = MathematicalCertificate {
        signature: producer_signature(&mut arena),
        level_arity: 0,
        context: vec![index_binding, tree_binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

    // The receiver sees only bytes: eleven declarations — five scheme
    // definitions, two proved theorems, two assumptions and the
    // `Box`/`boxRefl` pair — then the judgment.
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(decoded.certificate.signature.len(), 11);
    assert_eq!(
        decoded
            .certificate
            .signature
            .iter()
            .map(|declaration| declaration.level_arity)
            .collect::<Vec<_>>(),
        vec![3, 3, 3, 3, 4, 2, 3, 0, 0, 0, 0]
    );
    verify(&mut decoded).expect("the theorem application must re-verify");

    // Canonical re-encode is byte-identical.
    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // The exact assumption record: the judgment commits to `Elem` and
    // `e0` — the constants its context and the theorem's spine name —
    // and to none of the scheme or theorem *definitions*.
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [ELEM, ELEM_ZERO].into_iter().collect()
    );

    // The swapped-endpoint claim `Id Elem i e0` is a different, false
    // judgment over the same evidence: a bound variable never converts
    // to an assumption constant.
    let mut arena = TermArena::new();
    let index_binding = elem(&mut arena);
    let tree_index = variable(&mut arena, 0);
    let tree_binding = indexed_w_at(&mut arena, tree_index);
    let index = variable(&mut arena, 1);
    let tree = variable(&mut arena, 0);
    let term = index_correct_at(&mut arena, index, tree);
    let forged = {
        let ty = elem(&mut arena);
        let left = variable(&mut arena, 1);
        let right = elem_zero(&mut arena);
        id(&mut arena, ty, left, right)
    };
    let certificate = MathematicalCertificate {
        signature: producer_signature(&mut arena),
        level_arity: 0,
        context: vec![index_binding, tree_binding],
        term,
        expected: forged,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

#[test]
fn a_theorem_dependent_obligation_transports_over_the_wire() {
    let mut arena = TermArena::new();
    // Γ = i : Elem, t : IW i, P : Π(_:Elem). Type 0, h : P X proves
    //   `subst[0,0] Elem P X i (indexCorrect … i t) h : P i`
    // where `X = out₀ (rootOf t)` is the unreduced root index — a
    // consumer obligation `P i` established by two published theorems
    // composing: `indexCorrect` supplies the identity `Id Elem X i` and
    // `subst` transports `h : P X` along it.
    let index_binding = elem(&mut arena);
    let tree_index = variable(&mut arena, 0);
    let tree_binding = indexed_w_at(&mut arena, tree_index);
    let predicate_binding = {
        let domain = elem(&mut arena);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let hypothesis_binding = {
        let predicate = variable(&mut arena, 0);
        let tree = variable(&mut arena, 1);
        let source = root_index(&mut arena, tree);
        apply(&mut arena, predicate, source)
    };
    let term = {
        let predicate = variable(&mut arena, 1);
        let tree = variable(&mut arena, 2);
        let source = root_index(&mut arena, tree);
        let index = variable(&mut arena, 3);
        let tree = variable(&mut arena, 2);
        let proof = index_correct_at(&mut arena, index, tree);
        let index = variable(&mut arena, 3);
        let hypothesis = variable(&mut arena, 0);
        subst_at(&mut arena, predicate, source, index, proof, hypothesis)
    };
    let expected = {
        let predicate = variable(&mut arena, 1);
        let index = variable(&mut arena, 3);
        apply(&mut arena, predicate, index)
    };
    let certificate = MathematicalCertificate {
        signature: producer_signature(&mut arena),
        level_arity: 0,
        context: vec![
            index_binding,
            tree_binding,
            predicate_binding,
            hypothesis_binding,
        ],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("the composed transport must re-verify");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode"),
        bytes
    );
    // The composed judgment commits to the same two assumptions: the
    // index carrier and the element the description returns.
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [ELEM, ELEM_ZERO].into_iter().collect()
    );

    // Claiming `P X` — the untransported subject — is a different, false
    // judgment: `P i` never converts to `P (out₀ (rootOf t))` while `i`
    // is a bound variable.
    let mut arena = TermArena::new();
    let index_binding = elem(&mut arena);
    let tree_index = variable(&mut arena, 0);
    let tree_binding = indexed_w_at(&mut arena, tree_index);
    let predicate_binding = {
        let domain = elem(&mut arena);
        let codomain = type_sort(&mut arena, 0);
        pi(&mut arena, domain, codomain)
    };
    let hypothesis_binding = {
        let predicate = variable(&mut arena, 0);
        let tree = variable(&mut arena, 1);
        let source = root_index(&mut arena, tree);
        apply(&mut arena, predicate, source)
    };
    let term = {
        let predicate = variable(&mut arena, 1);
        let tree = variable(&mut arena, 2);
        let source = root_index(&mut arena, tree);
        let index = variable(&mut arena, 3);
        let tree = variable(&mut arena, 2);
        let proof = index_correct_at(&mut arena, index, tree);
        let index = variable(&mut arena, 3);
        let hypothesis = variable(&mut arena, 0);
        subst_at(&mut arena, predicate, source, index, proof, hypothesis)
    };
    let forged = {
        let predicate = variable(&mut arena, 1);
        let tree = variable(&mut arena, 2);
        let source = root_index(&mut arena, tree);
        apply(&mut arena, predicate, source)
    };
    let certificate = MathematicalCertificate {
        signature: producer_signature(&mut arena),
        level_arity: 0,
        context: vec![
            index_binding,
            tree_binding,
            predicate_binding,
            hypothesis_binding,
        ],
        term,
        expected: forged,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert!(matches!(
        verify(&mut decoded),
        Err(CoreError::TypeMismatch { .. })
    ));
}

#[test]
fn an_axiom_dependent_theorem_keeps_its_assumption_through_the_wire() {
    let mut arena = TermArena::new();
    // Γ = x : Box proves `boxRefl x : Id Box x x`. The theorem's
    // statement and body name `Box` — a definition whose own body is the
    // `Elem` assumption — and checking never unfolds `Box`: every
    // comparison is reflexive. The judgment's closure nonetheless records
    // `Elem`, because closure walks the stored declaration graph rather
    // than watching which constants conversion unfolded.
    let binding = constant(&mut arena, BOX);
    let theorem = constant(&mut arena, BOX_REFL);
    let bound = variable(&mut arena, 0);
    let term = apply(&mut arena, theorem, bound);
    let expected = {
        let ty = constant(&mut arena, BOX);
        let bound = variable(&mut arena, 0);
        id(&mut arena, ty, bound, bound)
    };
    let certificate = MathematicalCertificate {
        signature: producer_signature(&mut arena),
        level_arity: 0,
        context: vec![binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("the axiom-dependent theorem must re-verify");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode"),
        bytes
    );
    let closure =
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate);
    assert_eq!(closure, [ELEM].into_iter().collect());

    // Two receiving policies read the same decoded bytes: one admitting
    // `Elem` accepts the theorem's commitment; one admitting only `e0`
    // (or nothing) rejects it — the judgment's meaning is its exact
    // closure, not a producer label.
    assert!(closure.is_subset(&[ELEM].into_iter().collect()));
    assert!(!closure.is_subset(&[ELEM_ZERO].into_iter().collect()));
    assert!(!closure.is_subset(&std::collections::BTreeSet::new()));

    // The same certificate over the defect signature — `Elem := Two` in
    // place of the axiom — still verifies: `Box` and `boxRefl` check
    // identically. But its closure is empty, so a receiver comparing the
    // decoded closure to the admitted-axiom policy distinguishes the
    // substituted meaning without trusting the producer's claim.
    let mut arena = TermArena::new();
    let binding = constant(&mut arena, BOX);
    let theorem = constant(&mut arena, BOX_REFL);
    let bound = variable(&mut arena, 0);
    let term = apply(&mut arena, theorem, bound);
    let expected = {
        let ty = constant(&mut arena, BOX);
        let bound = variable(&mut arena, 0);
        id(&mut arena, ty, bound, bound)
    };
    let certificate = MathematicalCertificate {
        signature: defect_signature(&mut arena),
        level_arity: 0,
        context: vec![binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("the defect signature still checks");
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        std::collections::BTreeSet::new()
    );
}

#[test]
fn a_missing_dependency_rejects_the_signature() {
    let mut arena = TermArena::new();
    // Drop positions 7 and 8 (`Elem`, `e0`) and keep `Box`/`boxRefl`
    // verbatim: `Box`'s stored body still names position 7, which is now
    // `Box` itself — a self-reference the ordered signature check
    // refuses. The judgment's dependency is missing, not merely
    // unreferenced.
    let mut signature = producer_signature(&mut arena);
    signature.drain(ELEM as usize..=(ELEM_ZERO as usize));
    let binding = constant(&mut arena, BOX - 2);
    let theorem = constant(&mut arena, BOX_REFL - 2);
    let bound = variable(&mut arena, 0);
    let term = apply(&mut arena, theorem, bound);
    let expected = {
        let ty = constant(&mut arena, BOX - 2);
        let bound = variable(&mut arena, 0);
        id(&mut arena, ty, bound, bound)
    };
    let certificate = MathematicalCertificate {
        signature,
        level_arity: 0,
        context: vec![binding],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::UnknownDeclaration {
            declaration: 7,
            signature_len: 7,
        })
    );
}

#[test]
fn a_polymorphic_theorem_reference_round_trips_at_its_own_arity() {
    let mut arena = TermArena::new();
    // `subst` publishes polymorphically: at judgment arity 1 the
    // reference `subst[u, u]` is the transport theorem instantiated at
    // `A : Type u`, `P : Π(_:A). Type u` — level parameters, not
    // constants. The certificate claims the theorem's own statement at
    // that instantiation.
    let type_u = sort_level(&mut arena, Level::Parameter(0));
    // `Π(A : Type u). Π(P : Π(_:A). Type u). Π(x : A). Π(y : A).
    //   Π(_ : Id A x y). Π(_ : P x). P y`
    let expected = {
        let result = {
            // Under A, P, x, y, _p, _h: P is 4, y is 2, x is 3.
            let predicate = variable(&mut arena, 4);
            let endpoint = variable(&mut arena, 2);
            apply(&mut arena, predicate, endpoint)
        };
        let hypothesis_domain = {
            // Under A, P, x, y, _p: P is 3, x is 2.
            let predicate = variable(&mut arena, 3);
            let source = variable(&mut arena, 2);
            apply(&mut arena, predicate, source)
        };
        let over_hypothesis = pi(&mut arena, hypothesis_domain, result);
        let proof_domain = {
            // Under A, P, x, y: A is 3, x is 1, y is 0.
            let ty = variable(&mut arena, 3);
            let left = variable(&mut arena, 1);
            let right = variable(&mut arena, 0);
            id(&mut arena, ty, left, right)
        };
        let over_proof = pi(&mut arena, proof_domain, over_hypothesis);
        let carrier = variable(&mut arena, 2);
        let over_y = pi(&mut arena, carrier, over_proof);
        let carrier = variable(&mut arena, 1);
        let over_x = pi(&mut arena, carrier, over_y);
        let predicate_domain = {
            let domain = variable(&mut arena, 0);
            let codomain = sort_level(&mut arena, Level::Parameter(0));
            pi(&mut arena, domain, codomain)
        };
        let over_predicate = pi(&mut arena, predicate_domain, over_x);
        pi(&mut arena, type_u, over_predicate)
    };
    let term = constant_levels(
        &mut arena,
        SUBST,
        vec![Level::Parameter(0), Level::Parameter(0)],
    );
    let certificate = MathematicalCertificate {
        signature: producer_signature(&mut arena),
        level_arity: 1,
        context: Vec::new(),
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(decoded.certificate.level_arity, 1);
    verify(&mut decoded).expect("the polymorphic theorem reference must re-check");
    assert_eq!(
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode"),
        bytes
    );

    // Forging the arity to zero on the wire makes the level arguments
    // out of scope — the kernel refuses, as for any malformed universe.
    let mut forged = bytes.clone();
    forged[10..14].copy_from_slice(&0_u32.to_le_bytes());
    let mut decoded = decode_mathematical_certificate(&forged).expect("decode");
    assert_eq!(
        verify(&mut decoded),
        Err(CoreError::UnboundLevelParameter { index: 0, arity: 0 })
    );
}

//! External arithmetic proof import: a producer signature carries the
//! arithmetic vocabulary (`Nat`, `zero`, `succ`, `add`) plus the two
//! recursion axioms as named assumptions, and the checked proof object
//! derives `add n (succ zero) = succ n` by transporting the `addSucc`
//! axiom along the `addZero` axiom through the `subst` library theorem.
//! Decoding and re-verification re-decide the whole judgment — the
//! producer's declared axioms are producer evidence, and the computed
//! assumption closure names exactly the rows the receiver must admit.

use proof_admission::{
    Budget, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, Level, MathematicalCertificate, Sort,
    Term, TermArena, TermHandle, identity_substitution, verify_mathematical_certificate,
};
use terminal_codec::{decode_mathematical_certificate, encode_mathematical_certificate};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
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

fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

fn verify(decoded: &mut terminal_codec::DecodedMathematicalCertificate) -> Result<(), CoreError> {
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget())
}

/// `subst` — the identity-transport library theorem — is declaration 0.
const SUBST: u32 = 0;
/// `Nat : Type 0`, a named assumption — declaration 1.
const NAT: u32 = 1;
/// `zero : Nat`, a named assumption — declaration 2.
const ZERO: u32 = 2;
/// `succ : Nat → Nat`, a named assumption — declaration 3.
const SUCC: u32 = 3;
/// `add : Nat → Nat → Nat`, a named assumption — declaration 4.
const ADD: u32 = 4;
/// `addZero : Π(n : Nat). Id Nat (add n zero) n`, a named axiom —
/// declaration 5.
const ADD_ZERO: u32 = 5;
/// `addSucc : Π(n m : Nat). Id Nat (add n (succ m)) (succ (add n m))`,
/// a named axiom — declaration 6.
const ADD_SUCC: u32 = 6;

fn nat(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT)
}

fn nat_zero(arena: &mut TermArena) -> TermHandle {
    constant(arena, ZERO)
}

fn nat_succ(arena: &mut TermArena, argument: TermHandle) -> TermHandle {
    let succ = constant(arena, SUCC);
    apply(arena, succ, argument)
}

fn nat_add(arena: &mut TermArena, left: TermHandle, right: TermHandle) -> TermHandle {
    let add = constant(arena, ADD);
    let partial = apply(arena, add, left);
    apply(arena, partial, right)
}

/// The seven-declaration producer signature: `subst`, then `Nat`, `zero`,
/// `succ`, `add`, and the two recursion axioms — the imported source's
/// exact axiom set.
fn arithmetic_signature(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    declarations.push(identity_substitution(arena)); // subst
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Nat
    declarations.push(Declaration::assumption(0, nat(arena))); // zero
    declarations.push(Declaration::assumption(0, {
        // Π(_ : Nat). Nat
        let domain = nat(arena);
        let codomain = nat(arena);
        pi(arena, domain, codomain)
    })); // succ
    declarations.push(Declaration::assumption(0, {
        // Π(_ : Nat). Π(_ : Nat). Nat
        let domain = nat(arena);
        let inner_domain = nat(arena);
        let inner_codomain = nat(arena);
        let codomain = pi(arena, inner_domain, inner_codomain);
        pi(arena, domain, codomain)
    })); // add
    declarations.push(Declaration::assumption(0, {
        // Π(n : Nat). Id Nat (add n zero) n
        let domain = nat(arena);
        let bound = variable(arena, 0);
        let zero = nat_zero(arena);
        let left = nat_add(arena, bound, zero);
        let nat_ty = nat(arena);
        let body = id(arena, nat_ty, left, bound);
        pi(arena, domain, body)
    })); // addZero
    declarations.push(Declaration::assumption(0, {
        // Π(n m : Nat). Id Nat (add n (succ m)) (succ (add n m))
        let domain = nat(arena);
        let body = {
            let domain = nat(arena);
            let n = variable(arena, 1);
            let m = variable(arena, 0);
            let succ_m = nat_succ(arena, m);
            let left = nat_add(arena, n, succ_m);
            let n = variable(arena, 1);
            let m = variable(arena, 0);
            let add_n_m = nat_add(arena, n, m);
            let right = nat_succ(arena, add_n_m);
            let nat_ty = nat(arena);
            let body = id(arena, nat_ty, left, right);
            pi(arena, domain, body)
        };
        pi(arena, domain, body)
    })); // addSucc
    declarations
}

/// The imported derivation `add n (succ zero) = succ n` under
/// `Γ = n : Nat`: `subst[0,0] Nat P (add n zero) n (addZero n)
/// (addSucc n zero)` transports the step axiom along the base axiom,
/// where `P = λ(r : Nat). Id Nat (add n (succ zero)) (succ r)`.
fn import_certificate(arena: &mut TermArena) -> MathematicalCertificate {
    let declarations = arithmetic_signature(arena);
    // P = λ(r : Nat). Id Nat (add n (succ zero)) (succ r); under the
    // binder the context's `n` is de Bruijn 1 and `r` is 0.
    let motive = {
        let n = variable(arena, 1);
        let r = variable(arena, 0);
        let zero = nat_zero(arena);
        let succ_zero = nat_succ(arena, zero);
        let left = nat_add(arena, n, succ_zero);
        let right = nat_succ(arena, r);
        let domain = nat(arena);
        let nat_ty = nat(arena);
        let body = id(arena, nat_ty, left, right);
        lambda(arena, domain, body)
    };
    let n = variable(arena, 0);
    let subst = arena.insert(Term::Constant {
        declaration: SUBST,
        levels: vec![Level::Constant(0), Level::Constant(0)],
    });
    let term = {
        let nat_ty = nat(arena);
        let over_carrier = apply(arena, subst, nat_ty);
        let over_motive = apply(arena, over_carrier, motive);
        let zero = nat_zero(arena);
        let add_n_zero = nat_add(arena, n, zero);
        let over_source = apply(arena, over_motive, add_n_zero);
        let over_target = apply(arena, over_source, n);
        let add_zero = constant(arena, ADD_ZERO);
        let base_proof = apply(arena, add_zero, n);
        let step_proof = {
            let add_succ = constant(arena, ADD_SUCC);
            let applied_n = apply(arena, add_succ, n);
            let zero = nat_zero(arena);
            apply(arena, applied_n, zero)
        };
        let over_base = apply(arena, over_target, base_proof);
        apply(arena, over_base, step_proof)
    };
    let expected = {
        let zero = nat_zero(arena);
        let succ_zero = nat_succ(arena, zero);
        let left = nat_add(arena, n, succ_zero);
        let right = nat_succ(arena, n);
        let nat_ty = nat(arena);
        id(arena, nat_ty, left, right)
    };
    MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![nat(arena)],
        term,
        expected,
    }
}

#[test]
fn an_imported_arithmetic_derivation_re_verifies_with_its_exact_axiom_closure() {
    let mut arena = TermArena::new();
    let certificate = import_certificate(&mut arena);
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");

    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    assert_eq!(decoded.certificate.signature.len(), 7);
    verify(&mut decoded).expect("the imported derivation must re-verify");

    let repacked =
        encode_mathematical_certificate(&decoded.arena, &decoded.certificate).expect("re-encode");
    assert_eq!(repacked, bytes);

    // The exact source axiom closure: the derivation commits to the four
    // vocabulary constants and both recursion axioms — positions 1–6 —
    // and to none of `subst`'s own (body-checked, assumption-free)
    // definition.
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [NAT, ZERO, SUCC, ADD, ADD_ZERO, ADD_SUCC]
            .into_iter()
            .collect()
    );
}

#[test]
fn the_imported_arithmetic_derivation_has_a_measured_size_and_step_cost() {
    // The evidence row the interchange design asks for: certificate size
    // and checking cost, pinned exactly so drift in either is visible.
    let mut arena = TermArena::new();
    let certificate = import_certificate(&mut arena);
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    assert_eq!(bytes.len(), 716usize, "encoded certificate size (bytes)");

    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut budget)
        .expect("re-verify under the default budget");
    let used = DEFAULT_CONVERSION_STEPS - budget.remaining();
    assert_eq!(used, 6u32, "conversion steps consumed by re-verification");

    // The measured bound binds: one step short of the exact cost rejects
    // on StepCeiling, and the exact cost is enough.
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    let mut short = Budget::new(used - 1);
    assert!(matches!(
        verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut short),
        Err(CoreError::StepCeiling)
    ));
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    let mut exact = Budget::new(used);
    verify_mathematical_certificate(&mut decoded.arena, &decoded.certificate, &mut exact)
        .expect("the measured budget exactly suffices");
}

#[test]
fn an_imported_arithmetic_axiom_alone_carries_only_the_cited_rows() {
    // `addZero n` proves `add n zero = n` directly — the closure must
    // name exactly the cited axiom and its statement's vocabulary,
    // excluding `succ` and `addSucc`, which the judgment never touches.
    let mut arena = TermArena::new();
    let declarations = arithmetic_signature(&mut arena);
    let n = variable(&mut arena, 0);
    let add_zero = constant(&mut arena, ADD_ZERO);
    let term = apply(&mut arena, add_zero, n);
    let expected = {
        let zero = nat_zero(&mut arena);
        let left = nat_add(&mut arena, n, zero);
        let nat_ty = nat(&mut arena);
        id(&mut arena, nat_ty, left, n)
    };
    let context_ty = nat(&mut arena);
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![context_ty],
        term,
        expected,
    };
    let bytes = encode_mathematical_certificate(&arena, &certificate).expect("encode");
    let mut decoded = decode_mathematical_certificate(&bytes).expect("decode");
    verify(&mut decoded).expect("the direct citation must re-verify");
    assert_eq!(
        proof_admission::certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        [NAT, ZERO, ADD, ADD_ZERO].into_iter().collect()
    );
}

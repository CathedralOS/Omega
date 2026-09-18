//! Level instantiation on the derived indexed-family scheme: the scheme
//! declarations `IndexedAt`, `IW`, `iwPack`, `isup`, `iindW` are
//! universe-polymorphic over the description levels `l, u, v` (and the
//! eliminator's motive level `w`), and this file exercises them at
//! instantiations other than the all-`Constant(0)` the `Vec`, mutual and
//! nested demonstrations use.
//!
//! The vehicle is a length-indexed vector whose *element type is the
//! universe parameter*: a producer declares
//!
//! ```text
//! vecOf   : Π(E : Type u). Π(n : Nat). Type u
//!         := λE. λn. IW[0, u, 0] Nat (A E) (B E) (out E) (next E) n
//! consVec : Π(E : Type u). Π(e : E). Π(n : Nat). Π(t : vecOf u E n).
//!             vecOf u E (succN n)
//!         := λE. λe. λn. λt. isup[0, u, 0] … ⟨one, ⟨e, n⟩⟩ (λ_. t)
//! ```
//!
//! once, at level arity 1 — `u` is `Level::Parameter(0)` throughout the
//! bodies, including inside the `Constant` level arguments of the scheme
//! spines. `check_signature` re-decides both declarations parametrically,
//! so `vecOf[0] Two` is a vector of booleans at `Type 0` and
//! `vecOf[1] (Type 0)` is a vector of *types* at `Type 1` — the same five
//! scheme constants at two different instantiations inside one checked
//! signature. The uniform payload `caseTwo(λ_. Type u, E, Σ(e : E). Nat,
//! tag)` keeps both branches at `Type u` for every `u`: without
//! cumulativity a fixed `Two` nil payload could never check against
//! `Type u` at a parameter level, so the nil tag simply stores `E` as a
//! dummy — at `u := 0, E := Two` that is exactly the `Vec` description.
//!
//! `Nat`, `zeroN` and `succN` remain signature *assumptions*; `vecOf` and
//! `consVec` are definitions, so the certificate's assumption closure
//! records exactly the three axioms. Every application is still re-decided
//! by the checker — a wrong universe on the element argument, a wrong
//! level-argument count, an out-of-scope parameter, or a description
//! instantiated at levels that do not match its real sorts all reject
//! with the exact typed error.

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
    MathematicalCertificate, Signature, Sort, Term, TermArena, TermHandle,
    certificate_assumption_closure, check_signature, check_type, convertible, indexed_scheme,
    infer_sort, infer_type, shift, verify_mathematical_certificate,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

/// The eliminator test's ceiling: `iindW` unfolds the whole
/// `IndexedAt`/`IW` encoding for every application; the level-1
/// description spends the same order as the level-0 vector's measured
/// ~39 thousand steps. This bound leaves wide headroom so the run stays
/// budgeted rather than open-ended — `StepCeiling` is still the
/// decidability witness.
fn measure_budget() -> Budget {
    Budget::new(1 << 20)
}

fn sort_level(arena: &mut TermArena, level: Level) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(level)))
}

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    sort_level(arena, Level::Constant(level))
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

fn constant(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

// ── The producer signature ────────────────────────────────────────────
//
// Positions 0–4 are the derived scheme; the vector's index arithmetic is
// the three appended assumptions; positions 8 and 9 are the
// universe-polymorphic producer declarations:

/// `Nat : Type 0` — the index type, declaration 5.
const NAT: u32 = 5;
/// `zeroN : Nat` — declaration 6.
const NAT_ZERO: u32 = 6;
/// `succN : Π(_ : Nat). Nat` — declaration 7.
const NAT_SUCC: u32 = 7;
/// `vecOf : Π(E : Type u). Π(n : Nat). Type u` — declaration 8.
const VEC_OF: u32 = 8;
/// `consVec : Π(E : Type u). Π(e : E). Π(n : Nat). Π(t : vecOf u E n).
/// vecOf u E (succN n)` — declaration 9.
const CONS_VEC: u32 = 9;

fn nat(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT, Vec::new())
}

fn nat_zero(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT_ZERO, Vec::new())
}

fn nat_succ(arena: &mut TermArena, predecessor: TermHandle) -> TermHandle {
    let succ = constant(arena, NAT_SUCC, Vec::new());
    apply(arena, succ, predecessor)
}

/// `vecOf[u] · E · n` — the declared family applied at instantiation `u`.
fn vec_at(arena: &mut TermArena, u: Level, element: TermHandle, length: TermHandle) -> TermHandle {
    let family = constant(arena, VEC_OF, vec![u]);
    let applied = apply(arena, family, element);
    apply(arena, applied, length)
}

/// `consVec[u] · E · e · n · t` — the declared constructor applied at `u`.
fn cons_at(
    arena: &mut TermArena,
    u: Level,
    element: TermHandle,
    member: TermHandle,
    length: TermHandle,
    tail: TermHandle,
) -> TermHandle {
    let constructor = constant(arena, CONS_VEC, vec![u]);
    let applied = apply(arena, constructor, element);
    let applied = apply(arena, applied, member);
    let applied = apply(arena, applied, length);
    apply(arena, applied, tail)
}

/// `λ(_ : Two). Type 0` — the constant motive every `caseTwo` selecting
/// *child positions* shares; positions are always `Id Two … : Type 0`.
fn position_motive(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let codomain = type_sort(arena, 0);
    lambda(arena, domain, codomain)
}

/// `Id Two zero one` — the dead child position under the nil tag.
fn empty_positions(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_one(arena);
    id(arena, ty, left, right)
}

/// `Id Two zero zero` — the single live child position under cons.
fn unit_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_zero(arena);
    id(arena, ty, left, right)
}

// ── The description, parameterized by element type and level ──────────
//
// Every helper takes `element` as a term valid at the position the
// returned handle will occupy, and `shift`s it under the binders the
// helper introduces. `level` is `u`: the universe the element type and
// the payloads live at — `Parameter(0)` inside the polymorphic
// declarations, a `Constant` at each concrete instantiation.

/// `Payload = λ(tag : Two). caseTwo(λ_. Type u, E, Σ(e : E). Nat, tag)` —
/// nil stores a dummy `E`; cons stores `⟨element, predecessor-length⟩`.
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

/// The constructor carrier `A = Σ(tag : Two). Payload tag : Type u`.
fn carrier(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let tag = variable(arena, 0);
    let element = shift(arena, element, 0, 1);
    let payload_fn = payload(arena, element, level);
    let codomain = apply(arena, payload_fn, tag);
    let domain = two(arena);
    sigma(arena, domain, codomain)
}

/// `B = λ(a : A). caseTwo(λ_.Type 0, Id Two zero one, Id Two zero zero,
/// fst a)` — child positions selected by tag: none under nil, one under
/// cons. `v` stays `0` at every instantiation: positions never carry the
/// element universe.
fn branching(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let motive = position_motive(arena);
    let empty = empty_positions(arena);
    let unit = unit_position(arena);
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let body = case_two(arena, motive, empty, unit, tag);
    let domain = carrier(arena, element, level);
    lambda(arena, domain, body)
}

/// `out = λ(a : A). caseTwo(M, λ_. zeroN, λp. succN (snd p), fst a)
/// (snd a)` with `M tag = Π(_ : Payload tag). Nat` — the index a node
/// produces: `zeroN` under nil, `succN n` under `cons ⟨e, n⟩`.
fn out_index(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let motive = {
        // Under `λ(a)` then `λ(tag)`: the element type is two binders deep.
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
        // `λ(p : Σ(e : E). Nat). succN (snd p)` at
        // `Π(_ : Payload one ≡ Σ(e : E). Nat). Nat`.
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

/// `next = λ(a : A). caseTwo(M, λp.λ_. zeroN, λp.λ_. snd p, fst a)
/// (snd a)` with `M tag = Π(p : Payload tag). Π(_ : B' tag). Nat` and
/// `B' tag = caseTwo(λ_.Type 0, Id Two zero one, Id Two zero zero, tag)`
/// — the index each child position requires: unreachable under nil, and
/// `snd p = n`, the predecessor, under cons.
fn next_index(arena: &mut TermArena, element: TermHandle, level: &Level) -> TermHandle {
    let motive = {
        // `λ(tag : Two). Π(p : Payload tag). Π(_ : B' tag). Nat`.
        let tag = variable(arena, 0);
        let element = shift(arena, element, 0, 2);
        let payload_fn = payload(arena, element, level);
        let payload_at = apply(arena, payload_fn, tag);
        let branching_at = {
            // Under the `p` binder the tag is index 1.
            let motive = position_motive(arena);
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
        // `λ(p : E). λ(_ : Id Two zero one). zeroN` at
        // `Π(_ : Payload zero). Π(_ : B' zero). Nat`.
        let payload_domain = shift(arena, element, 0, 1);
        let position_domain = empty_positions(arena);
        let body = nat_zero(arena);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let one_branch = {
        // `λ(p : Σ(e : E). Nat). λ(_ : Id Two zero zero). snd p` —
        // the child's required index is the recorded predecessor.
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

/// The family whose elements live at `element_level`: `I = Nat`,
/// `A = carrier(element, u)`, `B`, `out`, `next` as above, instantiated
/// at `[0, u, 0]`. `element` must itself be a `Type u` expression.
fn family_at(arena: &mut TermArena, element: TermHandle, element_level: Level) -> IndexedFamily {
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

// ── The two polymorphic producer declarations ─────────────────────────

/// `vecOf : Π(E : Type u). Π(n : Nat). Type u` at level arity 1, defined
/// as `λE. λn. IW[0, u, 0] Nat (A E) (B E) (out E) (next E) n` — the
/// scheme's `IW` constant instantiated with the declaration's own
/// universe parameter inside its level arguments.
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
        let family = family_at(arena, element, Level::Parameter(0));
        let index = nat(arena);
        let length = variable(arena, 0);
        let applied = family.indexed_w(arena, length);
        let inner = lambda(arena, index, applied);
        lambda(arena, type_u, inner)
    };
    Declaration::definition(1, statement, body)
}

/// `consVec : Π(E : Type u). Π(e : E). Π(n : Nat). Π(t : vecOf u E n).
/// vecOf u E (succN n)` at level arity 1 — the body applies `isup` at
/// `[0, u, 0]` to the cons node `⟨one, ⟨e, n⟩⟩` and the constant child
/// function `λ(_ : B (cons e n)). t`.
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
        let family = family_at(arena, element, Level::Parameter(0));
        let node = {
            let member = variable(arena, 2);
            let predecessor = variable(arena, 1);
            let payload_value = pair(arena, member, predecessor);
            let tag = two_one(arena);
            pair(arena, tag, payload_value)
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

/// The producer signature: the five scheme declarations, the three
/// `Nat` assumptions, then `vecOf` and `consVec`.
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

fn level_signature(arena: &mut TermArena) -> Signature {
    let declarations = level_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

/// `⟨one, ⟨e, n⟩⟩` — a cons node whose payload records the element and
/// the predecessor length.
fn cons_node(arena: &mut TermArena, element: TermHandle, predecessor: TermHandle) -> TermHandle {
    let payload_value = pair(arena, element, predecessor);
    let tag = two_one(arena);
    pair(arena, tag, payload_value)
}

#[test]
fn the_polymorphic_family_declaration_checks_once_and_instantiates_at_each_level() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let declarations = level_declarations(&mut arena);
    assert_eq!(declarations.len(), 10);

    let before = budget.remaining();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 10);
    assert!(before - budget.remaining() > 0);

    // Under `n : Nat`, `vecOf[0] Two n` is a `Type 0` vector of booleans
    // and `vecOf[1] (Type 0) n` a `Type 1` vector of types — the same
    // declaration, the same `IW` constant, two level instantiations in
    // one judgment.
    let index_type = nat(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);
    let length = variable(&mut arena, 0);

    let element = two(&mut arena);
    let vec_two = vec_at(&mut arena, Level::Constant(0), element, length);
    let sort = infer_sort(&mut arena, &context, vec_two, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(0)));

    let length = variable(&mut arena, 0);
    let element = type_sort(&mut arena, 0);
    let vec_types = vec_at(&mut arena, Level::Constant(1), element, length);
    let sort = infer_sort(&mut arena, &context, vec_types, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(1)));

    // Each agrees with the direct `IndexedFamily` spine at the same
    // instantiation: the declaration wrapper is exactly the scheme
    // applied, never a parallel former.
    let length = variable(&mut arena, 0);
    let element = two(&mut arena);
    let direct_two =
        family_at(&mut arena, element, Level::Constant(0)).indexed_w(&mut arena, length);
    let shared_zero = type_sort(&mut arena, 0);
    assert!(
        convertible(
            &mut arena,
            &context,
            vec_two,
            direct_two,
            shared_zero,
            &mut budget
        )
        .unwrap()
    );

    let length = variable(&mut arena, 0);
    let element = type_sort(&mut arena, 0);
    let direct_types =
        family_at(&mut arena, element, Level::Constant(1)).indexed_w(&mut arena, length);
    let shared_one = type_sort(&mut arena, 1);
    assert!(
        convertible(
            &mut arena,
            &context,
            vec_types,
            direct_types,
            shared_one,
            &mut budget
        )
        .unwrap()
    );

    // The two instantiations are not interchangeable:
    // `vecOf[0] Two n` never converts to the level-1 family.
    assert!(
        !convertible(
            &mut arena,
            &context,
            vec_two,
            direct_types,
            shared_one,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn the_polymorphic_constructor_builds_at_each_instantiation() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let signature = level_signature(&mut arena);

    // Γ = n : Nat, t : vecOf[0] Two n — a boolean vector extended by
    // `one` lands at the successor length.
    let index_type = nat(&mut arena);
    let length = variable(&mut arena, 0);
    let element = two(&mut arena);
    let tail_type = vec_at(&mut arena, Level::Constant(0), element, length);
    let context = Context::empty()
        .with_signature(signature.clone())
        .extend(index_type)
        .extend(tail_type);
    // In Γ: t = 0, n = 1.
    let element = two(&mut arena);
    let member = two_one(&mut arena);
    let length = variable(&mut arena, 1);
    let tail = variable(&mut arena, 0);
    let cons_two = cons_at(
        &mut arena,
        Level::Constant(0),
        element,
        member,
        length,
        tail,
    );
    let expected = {
        let predecessor = variable(&mut arena, 1);
        let successor = nat_succ(&mut arena, predecessor);
        let element = two(&mut arena);
        vec_at(&mut arena, Level::Constant(0), element, successor)
    };
    check_type(&mut arena, &context, cons_two, expected, &mut budget).unwrap();

    // Γ = n : Nat, t : vecOf[1] (Type 0) n — a vector of types extended
    // by the element *type* `Two` lands at the successor length, one
    // universe up.
    let index_type = nat(&mut arena);
    let length = variable(&mut arena, 0);
    let element = type_sort(&mut arena, 0);
    let tail_type = vec_at(&mut arena, Level::Constant(1), element, length);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type)
        .extend(tail_type);
    let element = type_sort(&mut arena, 0);
    let member = two(&mut arena);
    let length = variable(&mut arena, 1);
    let tail = variable(&mut arena, 0);
    let cons_types = cons_at(
        &mut arena,
        Level::Constant(1),
        element,
        member,
        length,
        tail,
    );
    let expected = {
        let predecessor = variable(&mut arena, 1);
        let successor = nat_succ(&mut arena, predecessor);
        let element = type_sort(&mut arena, 0);
        vec_at(&mut arena, Level::Constant(1), element, successor)
    };
    check_type(&mut arena, &context, cons_types, expected, &mut budget).unwrap();

    // And the declared constructor is definitionally the scheme's `isup`
    // at the same instantiation: `consVec[1] (Type 0) Two n t` converts
    // to the direct `IndexedFamily` spine's `isup` application.
    let element = type_sort(&mut arena, 0);
    let direct = family_at(&mut arena, element, Level::Constant(1));
    let member = two(&mut arena);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, member, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, direct.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let isup = direct.sup(&mut arena, node, children_fn);
    assert!(
        convertible(
            &mut arena,
            &context,
            cons_types,
            isup,
            expected,
            &mut budget
        )
        .unwrap()
    );
}

#[test]
fn the_eliminator_computes_at_the_higher_instantiation() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let element = type_sort(&mut arena, 0);
    let family = family_at(&mut arena, element, Level::Constant(1));
    let signature = level_signature(&mut arena);

    // Γ = Q : Π(i : Nat). Π(_ : VecT i). Type 1, s : <iindW step>,
    //     T : Type 0, n : Nat, tail : VecT n
    // where `VecT i = IW[0,1,0] … i` — indices tail = 0, n = 1, T = 2,
    // s = 3, Q = 4. The motive lands at `Type 1`, so `w = 1` exercises
    // the eliminator's fourth level parameter as well.
    let motive_type = {
        let index = variable(&mut arena, 0);
        let family_at_index = family.indexed_w(&mut arena, index);
        let type_one = type_sort(&mut arena, 1);
        let inner = pi(&mut arena, family_at_index, type_one);
        let index_type = nat(&mut arena);
        pi(&mut arena, index_type, inner)
    };
    let step_type = {
        // `Π(a : A). Π(g : Π(b : B a). VecT (next a b)).
        //   Π(_ : Π(b : B a). Q (next a b) (g b)). Q (out a) (isup a g)`
        // written under the one-binding prefix [Q].
        let g_type = {
            // Under a: a = 0, Q = 1.
            let bound = variable(&mut arena, 0);
            let b_domain = apply(&mut arena, family.children, bound);
            let codomain = {
                // Under b: a = 1.
                let bound_a = variable(&mut arena, 1);
                let next_a = apply(&mut arena, family.next, bound_a);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                family.indexed_w(&mut arena, required)
            };
            pi(&mut arena, b_domain, codomain)
        };
        let hypothesis_type = {
            // Under g, a: a = 1, g = 0, Q = 2.
            let bound_a = variable(&mut arena, 1);
            let b_domain = apply(&mut arena, family.children, bound_a);
            let codomain = {
                // Under b: a = 2, g = 1, Q = 3.
                let bound_a = variable(&mut arena, 2);
                let next_a = apply(&mut arena, family.next, bound_a);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let motive = variable(&mut arena, 3);
                let at_index = apply(&mut arena, motive, required);
                let bound_g = variable(&mut arena, 1);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, bound_g, bound);
                apply(&mut arena, at_index, child)
            };
            pi(&mut arena, b_domain, codomain)
        };
        let result = {
            // Under the hypothesis, g, a: a = 2, g = 1, Q = 3.
            let bound_a = variable(&mut arena, 2);
            let produced = apply(&mut arena, family.out, bound_a);
            let bound_a = variable(&mut arena, 2);
            let bound_g = variable(&mut arena, 1);
            let node = family.sup(&mut arena, bound_a, bound_g);
            let motive = variable(&mut arena, 3);
            let at_index = apply(&mut arena, motive, produced);
            apply(&mut arena, at_index, node)
        };
        let inner = pi(&mut arena, hypothesis_type, result);
        let middle = pi(&mut arena, g_type, inner);
        let domain = family.carrier;
        pi(&mut arena, domain, middle)
    };
    let element_type = type_sort(&mut arena, 0);
    let index_type = nat(&mut arena);
    let tail_type = {
        let length = variable(&mut arena, 0);
        family.indexed_w(&mut arena, length)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(motive_type)
        .extend(step_type)
        .extend(element_type)
        .extend(index_type)
        .extend(tail_type);
    // In Γ: tail = 0, n = 1, T = 2, s = 3, Q = 4.

    // `consT T n tail = isup ⟨one, ⟨T, n⟩⟩ (λ_. tail)` — the stored
    // element is a type.
    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);

    // `iindW[0,1,0,1] Q s (succN n) (consT T n tail) : Q (succN n) cons`.
    let predecessor = variable(&mut arena, 1);
    let successor = nat_succ(&mut arena, predecessor);
    let motive = variable(&mut arena, 4);
    let step = variable(&mut arena, 3);
    let elimination = family.ind(
        &mut arena,
        Level::Constant(1),
        motive,
        step,
        successor,
        cons,
    );
    let shared = {
        let motive = variable(&mut arena, 4);
        let predecessor = variable(&mut arena, 1);
        let successor = nat_succ(&mut arena, predecessor);
        let at_index = apply(&mut arena, motive, successor);
        apply(&mut arena, at_index, cons)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // The profile's constructor computation judgment, with `tail` still
    // a neutral variable:
    //   `iindW Q s (succN n) (consT T n tail)`
    //     `≡ s ⟨one,⟨T,n⟩⟩ (λ_. tail) (b ↦ iindW Q s (next node b)
    //          ((λ_.tail) b))`
    //     `≡ s ⟨one,⟨T,n⟩⟩ (λ_. tail) (b ↦ iindW Q s n tail)`
    // — the induction hypothesis lands at the predecessor length, one
    // universe up.
    let hypothesis = {
        // The domain `B (consT T n)` is written at top level (T = 2,
        // n = 1); under the hypothesis's own `b` binder everything
        // shifts one further: tail = 1, n = 2, T = 3, s = 4, Q = 5.
        let element = variable(&mut arena, 2);
        let predecessor = variable(&mut arena, 1);
        let node = cons_node(&mut arena, element, predecessor);
        let domain = apply(&mut arena, family.children, node);
        let shifted_children = shift(&mut arena, children_fn, 0, 1);
        let body = {
            let element = variable(&mut arena, 3);
            let predecessor = variable(&mut arena, 2);
            let node = cons_node(&mut arena, element, predecessor);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, shifted_children, bound);
            let motive = variable(&mut arena, 5);
            let step = variable(&mut arena, 4);
            family.ind(
                &mut arena,
                Level::Constant(1),
                motive,
                step,
                required,
                child,
            )
        };
        lambda(&mut arena, domain, body)
    };
    let expected = {
        let step = variable(&mut arena, 3);
        let at_node = apply(&mut arena, step, node);
        let at_children = apply(&mut arena, at_node, children_fn);
        apply(&mut arena, at_children, hypothesis)
    };
    check_type(&mut arena, &context, expected, shared, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &context,
            elimination,
            expected,
            shared,
            &mut budget
        )
        .unwrap()
    );

    // Work receipt, pinned to the measured spend over this description.
    assert_eq!(total - budget.remaining(), 43_636);
}

#[test]
fn level_instantiation_rejects_the_wrong_universe() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let signature = level_signature(&mut arena);
    let index_type = nat(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);
    // In Γ: n = 0.

    // `vecOf[1] Two n` — `Two : Type 0` is one universe too low for the
    // `Type 1` domain the instantiation claims.
    let length = variable(&mut arena, 0);
    let element = two(&mut arena);
    let low = vec_at(&mut arena, Level::Constant(1), element, length);
    let error = infer_type(&mut arena, &context, low, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::ArgumentTypeMismatch { .. }));

    // `vecOf` takes one level argument; supplying none is malformed.
    let bare = constant(&mut arena, VEC_OF, Vec::new());
    let error = infer_type(&mut arena, &context, bare, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: VEC_OF,
            expected: 1,
            supplied: 0,
        }
    );

    // A `Parameter` level argument names the *judgment's* scope: under a
    // closed context `vecOf[u]` is out of scope.
    let closed = Context::empty().with_signature(context.signature().clone());
    let escaped = constant(&mut arena, VEC_OF, vec![Level::Parameter(0)]);
    let error = infer_type(&mut arena, &closed, escaped, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundLevelParameter { index: 0, arity: 0 }
    );

    // The description must match the instantiation it is claimed at:
    // the level-1 carrier `A : Type 1` supplied at `[0, 0, 0]`…
    let length = variable(&mut arena, 0);
    let element = type_sort(&mut arena, 0);
    let mut low_family = family_at(&mut arena, element, Level::Constant(1));
    low_family.levels = [Level::Constant(0), Level::Constant(0), Level::Constant(0)];
    let malformed = low_family.indexed_w(&mut arena, length);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::ArgumentTypeMismatch { .. } | CoreError::TypeMismatch { .. }
    ));

    // …and the level-0 carrier `A : Type 0` at `[0, 1, 0]` reject alike.
    let length = variable(&mut arena, 0);
    let element = two(&mut arena);
    let mut high_family = family_at(&mut arena, element, Level::Constant(0));
    high_family.levels = [Level::Constant(0), Level::Constant(1), Level::Constant(0)];
    let malformed = high_family.indexed_w(&mut arena, length);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::ArgumentTypeMismatch { .. } | CoreError::TypeMismatch { .. }
    ));

    // `iindW` is polymorphic over `l, u, v, w`: three level arguments is
    // a malformed constant.
    let short = constant(
        &mut arena,
        4,
        vec![Level::Constant(0), Level::Constant(1), Level::Constant(0)],
    );
    let error = infer_type(&mut arena, &context, short, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: 4,
            expected: 4,
            supplied: 3,
        }
    );
}

#[test]
fn a_polymorphic_vector_certificate_verifies_for_every_level() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let declarations = level_declarations(&mut arena);

    // `E : Type u, e : E, n : Nat, t : vecOf[u] E n ⊢
    //   consVec[u] E e n t : vecOf[u] E (succN n)`
    // — the whole judgment, signature included, re-decided
    // parametrically: the certificate claims it for every `u` at once,
    // and the scheme constants inside `vecOf`/`consVec`'s bodies carry
    // the parameter in their level arguments.
    let element_type = sort_level(&mut arena, Level::Parameter(0));
    let member_type = variable(&mut arena, 0);
    let index_type = nat(&mut arena);
    let tail_type = {
        // Under E, e, n: E is index 2, n is index 0.
        let element = variable(&mut arena, 2);
        let length = variable(&mut arena, 0);
        vec_at(&mut arena, Level::Parameter(0), element, length)
    };
    let term = {
        let element = variable(&mut arena, 3);
        let member = variable(&mut arena, 2);
        let length = variable(&mut arena, 1);
        let tail = variable(&mut arena, 0);
        cons_at(
            &mut arena,
            Level::Parameter(0),
            element,
            member,
            length,
            tail,
        )
    };
    let expected = {
        let element = variable(&mut arena, 3);
        let predecessor = variable(&mut arena, 1);
        let successor = nat_succ(&mut arena, predecessor);
        vec_at(&mut arena, Level::Parameter(0), element, successor)
    };
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 1,
        context: vec![element_type, member_type, index_type, tail_type],
        term,
        expected,
    };
    let before = budget.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    let spent = before - budget.remaining();
    assert!(spent > 0, "checking the certificate must do real work");

    // The judgment commits to exactly the three `Nat` assumptions —
    // `vecOf` and `consVec` are definitions and carry no assumption
    // force, and the scheme's definitions likewise stay out of the
    // closure.
    let closure = certificate_assumption_closure(&arena, &certificate);
    assert_eq!(closure, [NAT, NAT_ZERO, NAT_SUCC].into_iter().collect());

    // Claiming the tail's own length is a different, false judgment.
    let forged = {
        let element = variable(&mut arena, 3);
        let length = variable(&mut arena, 1);
        vec_at(&mut arena, Level::Parameter(0), element, length)
    };
    let element_type = sort_level(&mut arena, Level::Parameter(0));
    let member_type = variable(&mut arena, 0);
    let index_type = nat(&mut arena);
    let tail_type = {
        let element = variable(&mut arena, 2);
        let length = variable(&mut arena, 0);
        vec_at(&mut arena, Level::Parameter(0), element, length)
    };
    let certificate = MathematicalCertificate {
        signature: level_declarations(&mut arena),
        level_arity: 1,
        context: vec![element_type, member_type, index_type, tail_type],
        term,
        expected: forged,
    };
    assert!(matches!(
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Shifting the instantiation one universe up — `consVec[u+1]` — is a
    // different judgment: `E : Type u` never inhabits `Type (u+1)`.
    let lifted = {
        let constructor = constant(
            &mut arena,
            CONS_VEC,
            vec![Level::Parameter(0).successor().unwrap()],
        );
        let element = variable(&mut arena, 3);
        let applied = apply(&mut arena, constructor, element);
        let member = variable(&mut arena, 2);
        let applied = apply(&mut arena, applied, member);
        let length = variable(&mut arena, 1);
        let applied = apply(&mut arena, applied, length);
        let tail = variable(&mut arena, 0);
        apply(&mut arena, applied, tail)
    };
    let certificate = MathematicalCertificate {
        signature: level_declarations(&mut arena),
        level_arity: 1,
        context: vec![element_type, member_type, index_type, tail_type],
        term: lifted,
        expected,
    };
    assert!(matches!(
        verify_mathematical_certificate(&mut arena, &certificate, &mut budget),
        Err(CoreError::ArgumentTypeMismatch { .. } | CoreError::TypeMismatch { .. })
    ));

    // Retained-storage and work receipts: the ten-declaration signature
    // plus the `consVec` judgment, measured like the vector
    // certificate's — exact pins, not quotas.
    assert_eq!(arena.len(), 310_412);
    assert_eq!(spent, 2_705);
}

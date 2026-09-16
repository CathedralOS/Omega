//! The profile's "derivation context/conclusion indices" demonstration:
//! an indexed family of natural-deduction derivations built on the
//! derived [`indexed_scheme`] and checked end-to-end by the kernel.
//!
//! Where the vector's index was a single `Nat`, a derivation's index is
//! a *judgment*: a context paired with the conclusion it proves,
//! `I = Σ(Γ : Ctx). Frm`. The family `Deriv ⟨Γ, φ⟩` is a tiny
//! implicational logic with three rules — a leaf rule plus one unary
//! and one binary implication rule:
//!
//! ```text
//! assm φ Γ      : Deriv ⟨cons φ Γ, φ⟩            (head hypothesis)
//! impI φ ψ Γ d  : Deriv ⟨Γ, imp φ ψ⟩  for  d : Deriv ⟨cons φ Γ, ψ⟩
//! impE φ ψ Γ d₁ d₂ : Deriv ⟨Γ, ψ⟩     for  d₁ : Deriv ⟨Γ, imp φ ψ⟩,
//!                                        d₂ : Deriv ⟨Γ, φ⟩
//! ```
//!
//! The W-description is
//!
//! ```text
//! A    = Σ(t : Two). Σ(s : Two). Σ(φ : Frm). Σ(ψ : Frm). Ctx
//!          t = zero ↦ assm (leaf), t = one, s = zero ↦ impI,
//!          t = one, s = one ↦ impE
//! B a  = caseTwo(…, Id Two zero one,                    assm: dead
//!                caseTwo(…, Id Two zero zero,           impI: one
//!                          Two,                         impE: two
//!                          fst (snd a)),
//!                fst a)
//! out  = assm ↦ ⟨cons φ Γ, φ⟩, impI ↦ ⟨Γ, imp φ ψ⟩, impE ↦ ⟨Γ, ψ⟩
//! next = assm ↦ dead; impI's child ↦ ⟨cons φ Γ, ψ⟩;
//!        impE's child ↦ caseTwo(λ_.I, ⟨Γ, imp φ ψ⟩, ⟨Γ, φ⟩, b)
//! ```
//!
//! so the index discipline is computed in both index components:
//! `impI` *moves the context index* — the child is required at the
//! `φ`-extended context while the parent lands at `Γ` — and `impE`
//! selects the required *conclusion* by child position, the major
//! premise at `imp φ ψ` and the minor at `φ`. The `impE` `next` is a
//! `caseTwo` on the child position itself, so two positions of the same
//! node carry different required judgments — the shape a real premise
//! discipline needs.
//!
//! `Ctx`, `Frm`, `consF` and `impF` are signature *assumptions* — the
//! demonstration exercises the scheme against an arbitrary producer
//! context/formula grammar exactly as a producer would supply it, and
//! the certificate's assumption closure records precisely those four
//! axioms. The kernel has no primitive lists or connectives; every use
//! is still re-decided by the checker.

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
    MathematicalCertificate, Signature, Sort, Term, TermArena, TermHandle,
    certificate_assumption_closure, check_signature, check_type, convertible, indexed_scheme,
    infer_sort, infer_type, shift, verify_mathematical_certificate,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

/// The eliminator tests' ceiling: `iindW`'s typing and computation
/// unfold the whole `IndexedAt`/`IW` encoding — the `J` transport, the
/// repacked child functions, the pair-eta closures — and this family's
/// `next` runs a third `caseTwo` to select the required judgment per
/// child position. Measured spend is pinned per test below (~3.7 and
/// ~25.1 thousand steps); this bound leaves an order of magnitude of
/// headroom so the run stays budgeted rather than open-ended —
/// `StepCeiling` is still the decidability witness.
fn measure_budget() -> Budget {
    Budget::new(1 << 18)
}

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
}

fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
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

/// One side of a nested pair projection, innermost step first:
/// `project(arena, p, &[Side::Snd, Side::Fst])` is `fst (snd p)`.
#[derive(Clone, Copy)]
enum Side {
    Fst,
    Snd,
}

fn project(arena: &mut TermArena, mut term: TermHandle, path: &[Side]) -> TermHandle {
    for side in path {
        term = match side {
            Side::Fst => fst(arena, term),
            Side::Snd => snd(arena, term),
        };
    }
    term
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

fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
}

fn w_type(arena: &mut TermArena, carrier: TermHandle, children: TermHandle) -> TermHandle {
    arena.insert(Term::W { carrier, children })
}

fn sup(
    arena: &mut TermArena,
    carrier: TermHandle,
    children: TermHandle,
    label: TermHandle,
    function: TermHandle,
) -> TermHandle {
    arena.insert(Term::Sup {
        carrier,
        children,
        label,
        function,
    })
}

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

// ── The producer signature ────────────────────────────────────────────
//
// Positions 0–4 are the derived scheme (`IndexedAt`, `IW`, `iwPack`,
// `isup`, `iindW`); the derivation system's grammar is the appended
// assumptions:

/// `Ctx : Type 0` — derivation contexts, declaration 5.
const CTX: u32 = 5;
/// `Frm : Type 0` — formulas, declaration 6.
const FRM: u32 = 6;
/// `consF : Π(_ : Frm). Π(_ : Ctx). Ctx` — context extension, declaration 7.
const CONS: u32 = 7;
/// `impF : Π(_ : Frm). Π(_ : Frm). Frm` — implication formation, declaration 8.
const IMP: u32 = 8;

fn ctx(arena: &mut TermArena) -> TermHandle {
    constant(arena, CTX)
}

fn frm(arena: &mut TermArena) -> TermHandle {
    constant(arena, FRM)
}

/// `consF φ Γ : Ctx` — the context extended by hypothesis `φ`.
fn cons_f(arena: &mut TermArena, formula: TermHandle, context: TermHandle) -> TermHandle {
    let cons = constant(arena, CONS);
    let at_formula = apply(arena, cons, formula);
    apply(arena, at_formula, context)
}

/// `impF φ ψ : Frm` — the implication `φ ⇒ ψ`.
fn imp_f(arena: &mut TermArena, antecedent: TermHandle, consequent: TermHandle) -> TermHandle {
    let imp = constant(arena, IMP);
    let at_antecedent = apply(arena, imp, antecedent);
    apply(arena, at_antecedent, consequent)
}

/// `I = Σ(Γ : Ctx). Frm` — the index type: a judgment is a context
/// paired with the conclusion it proves.
fn judgment_type(arena: &mut TermArena) -> TermHandle {
    let context = ctx(arena);
    let formula = frm(arena);
    sigma(arena, context, formula)
}

/// `⟨Γ, φ⟩` — the judgment value with context `context` and conclusion
/// `formula`.
fn judgment(arena: &mut TermArena, context: TermHandle, formula: TermHandle) -> TermHandle {
    pair(arena, context, formula)
}

/// `P = Σ(φ : Frm). Σ(ψ : Frm). Ctx` — the uniform rule payload: every
/// rule records the formulas it mentions and the ambient context (the
/// `assm` leaf's `ψ` slot is a dummy, exactly as `fnil`'s element was
/// in the mutual demonstration — a dependent `Payload tag` cannot feed
/// the second `caseTwo`, since `snd a`'s type would be the stuck
/// `Payload (fst a)` rather than the two-element sub-tag).
fn rule_payload(arena: &mut TermArena) -> TermHandle {
    let antecedent = frm(arena);
    let consequent = frm(arena);
    let context = ctx(arena);
    let tail = sigma(arena, consequent, context);
    sigma(arena, antecedent, tail)
}

/// `Σ(s : Two). P` — the constructor sub-tag plus uniform payload.
fn sub_payload(arena: &mut TermArena) -> TermHandle {
    let tag = two(arena);
    let payload = rule_payload(arena);
    sigma(arena, tag, payload)
}

/// The constructor carrier `A = Σ(t : Two). Σ(s : Two). P`.
fn carrier(arena: &mut TermArena) -> TermHandle {
    let tag = two(arena);
    let rest = sub_payload(arena);
    sigma(arena, tag, rest)
}

/// `λ(_ : Two). Type 0` — the constant motive every `caseTwo` selecting
/// *types* here shares.
fn type_motive(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let codomain = type_sort(arena, 0);
    lambda(arena, domain, codomain)
}

/// `Id Two zero one` — the dead child position: no closed inhabitant.
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

/// `⟨zero, ⟨zero, ⟨φ, ⟨φ, Γ⟩⟩⟩⟩` — an `assm` label: the head hypothesis
/// `φ`, a dummy `ψ` slot (set to `φ`), and the ambient context `Γ`.
fn assm_label(arena: &mut TermArena, formula: TermHandle, context: TermHandle) -> TermHandle {
    let tail = pair(arena, formula, context);
    let payload = pair(arena, formula, tail);
    let sub_tag = two_zero(arena);
    let rest = pair(arena, sub_tag, payload);
    let tag = two_zero(arena);
    pair(arena, tag, rest)
}

/// `⟨one, ⟨zero, ⟨φ, ⟨ψ, Γ⟩⟩⟩⟩` — an `impI` label: antecedent `φ`,
/// consequent `ψ`, ambient context `Γ`.
fn impi_label(
    arena: &mut TermArena,
    antecedent: TermHandle,
    consequent: TermHandle,
    context: TermHandle,
) -> TermHandle {
    let tail = pair(arena, consequent, context);
    let payload = pair(arena, antecedent, tail);
    let sub_tag = two_zero(arena);
    let rest = pair(arena, sub_tag, payload);
    let tag = two_one(arena);
    pair(arena, tag, rest)
}

/// `⟨one, ⟨one, ⟨φ, ⟨ψ, Γ⟩⟩⟩⟩` — an `impE` label: the implication's
/// antecedent `φ` and consequent `ψ`, ambient context `Γ`.
fn impe_label(
    arena: &mut TermArena,
    antecedent: TermHandle,
    consequent: TermHandle,
    context: TermHandle,
) -> TermHandle {
    let tail = pair(arena, consequent, context);
    let payload = pair(arena, antecedent, tail);
    let sub_tag = two_one(arena);
    let rest = pair(arena, sub_tag, payload);
    let tag = two_one(arena);
    pair(arena, tag, rest)
}

/// `B = λ(a : A). caseTwo(λ_.Type 0, Id Two zero one,
///   caseTwo(λ_.Type 0, Id Two zero zero, Two, fst (snd a)), fst a)` —
/// child positions by rule: none under `assm`, one under `impI`, two
/// under `impE`.
fn branching(arena: &mut TermArena) -> TermHandle {
    let motive = type_motive(arena);
    let empty = empty_positions(arena);
    let inner = {
        let motive = type_motive(arena);
        let unit = unit_position(arena);
        let positions = two(arena);
        let bound = variable(arena, 0);
        let rest = snd(arena, bound);
        let sub_tag = fst(arena, rest);
        case_two(arena, motive, unit, positions, sub_tag)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let body = case_two(arena, motive, empty, inner, tag);
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// `B ⟨t, rest⟩` as an application of the `branching` lambda — the form
/// `next`'s motives quantify over, so the required position type under
/// abstract `t`/`rest` still reduces once both are concrete.
fn branching_at(arena: &mut TermArena, tag: TermHandle, rest: TermHandle) -> TermHandle {
    let node = pair(arena, tag, rest);
    let family = branching(arena);
    apply(arena, family, node)
}

/// `out = λ(a : A). caseTwo(Mo, assmOut, impOut, fst a) (snd a)` — the
/// judgment a node proves:
///
/// - `Mo t = Π(rest : Σ(s : Two). P). I`
/// - `assmOut = λrest. ⟨consF φ Γ, φ⟩` — the head hypothesis, in the
///   extended context.
/// - `impOut = λrest. caseTwo(Mi, impIOut, impEOut, fst rest)
///   (snd rest)` with `Mi s = Π(p : P). I`:
///   `impIOut = λp. ⟨Γ, impF φ ψ⟩`, `impEOut = λp. ⟨Γ, ψ⟩`.
///
/// For `rest = ⟨s, p⟩`: `fst (snd rest) = φ`, `fst (snd (snd rest)) =
/// ψ`, `snd (snd (snd rest)) = Γ`; for `p = ⟨φ, ⟨ψ, Γ⟩⟩`: `fst p = φ`,
/// `fst (snd p) = ψ`, `snd (snd p) = Γ`.
fn out_index(arena: &mut TermArena) -> TermHandle {
    let motive = {
        let rest_domain = sub_payload(arena);
        let codomain = judgment_type(arena);
        let body = pi(arena, rest_domain, codomain);
        let domain = two(arena);
        lambda(arena, domain, body)
    };
    let assm_out = {
        // `λrest. ⟨consF (fst (snd rest)) (snd (snd (snd rest))),
        //            fst (snd rest)⟩`.
        let rest = variable(arena, 0);
        let phi = project(arena, rest, &[Side::Snd, Side::Fst]);
        let rest = variable(arena, 0);
        let gamma = project(arena, rest, &[Side::Snd, Side::Snd, Side::Snd]);
        let extended = cons_f(arena, phi, gamma);
        let rest = variable(arena, 0);
        let phi = project(arena, rest, &[Side::Snd, Side::Fst]);
        let body = judgment(arena, extended, phi);
        let domain = sub_payload(arena);
        lambda(arena, domain, body)
    };
    let imp_out = {
        let inner_motive = {
            // `Mi s = Π(p : P). I`.
            let payload = rule_payload(arena);
            let codomain = judgment_type(arena);
            let body = pi(arena, payload, codomain);
            let domain = two(arena);
            lambda(arena, domain, body)
        };
        let impi_out = {
            // `λp. ⟨snd (snd p), impF (fst p) (fst (snd p))⟩`.
            let p = variable(arena, 0);
            let gamma = project(arena, p, &[Side::Snd, Side::Snd]);
            let p = variable(arena, 0);
            let phi = fst(arena, p);
            let p = variable(arena, 0);
            let psi = project(arena, p, &[Side::Snd, Side::Fst]);
            let implied = imp_f(arena, phi, psi);
            let body = judgment(arena, gamma, implied);
            let domain = rule_payload(arena);
            lambda(arena, domain, body)
        };
        let impe_out = {
            // `λp. ⟨snd (snd p), fst (snd p)⟩`.
            let p = variable(arena, 0);
            let gamma = project(arena, p, &[Side::Snd, Side::Snd]);
            let p = variable(arena, 0);
            let psi = project(arena, p, &[Side::Snd, Side::Fst]);
            let body = judgment(arena, gamma, psi);
            let domain = rule_payload(arena);
            lambda(arena, domain, body)
        };
        let rest = variable(arena, 0);
        let sub_tag = fst(arena, rest);
        let selected = case_two(arena, inner_motive, impi_out, impe_out, sub_tag);
        let rest = variable(arena, 0);
        let payload = snd(arena, rest);
        let body = apply(arena, selected, payload);
        let domain = sub_payload(arena);
        lambda(arena, domain, body)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, assm_out, imp_out, tag);
    let bound = variable(arena, 0);
    let rest = snd(arena, bound);
    let body = apply(arena, selected, rest);
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// `next = λ(a : A). caseTwo(Mn, assmN, impN, fst a) (snd a)` — the
/// judgment each child position must prove:
///
/// - `Mn t = Π(rest : Σ(s : Two). P). Π(b : B ⟨t, rest⟩). I`
/// - `assmN = λrest. λ(_ : Id Two zero one). ⟨Γ, φ⟩` — every position
///   is dead, so the required judgment is arbitrary.
/// - `impN = λrest. caseTwo(MiN, impIN, impEN, fst rest) (snd rest)`
///   with `MiN s = Π(p : P). Π(_ : caseTwo(λ_.Type 0, Id Two zero zero,
///   Two, s)). I`:
///   - `impIN = λp. λ(_ : Id Two zero zero). ⟨consF φ Γ, ψ⟩` — the
///     premise proves the consequent under the *extended* context.
///   - `impEN = λp. λ(b : Two). caseTwo(λ_.I, ⟨Γ, impF φ ψ⟩, ⟨Γ, φ⟩,
///     b)` — the major position requires the implication, the minor
///     position the antecedent: `next` selects the required judgment
///     by child position.
///
/// `impE`'s minor-position requirement is supplied by `minor` (built
/// under `[p, b]`): the real description uses `⟨Γ, φ⟩`; the rejection
/// test swaps in `⟨Γ, ψ⟩` (a well-typed defect) or a bare `Frm` (an
/// ill-typed one) without rebuilding the whole lambda.
fn next_index_with(
    arena: &mut TermArena,
    minor: impl FnOnce(&mut TermArena) -> TermHandle,
) -> TermHandle {
    let motive = {
        // Under the `rest` binder: `t` is index 1, `rest` index 0.
        let tag = variable(arena, 1);
        let rest = variable(arena, 0);
        let positions = branching_at(arena, tag, rest);
        let codomain = judgment_type(arena);
        let inner = pi(arena, positions, codomain);
        let rest_domain = sub_payload(arena);
        let body = pi(arena, rest_domain, inner);
        let domain = two(arena);
        lambda(arena, domain, body)
    };
    let assm_next = {
        // `λrest. λ(_ : Id Two zero one). ⟨Γ, φ⟩` — under `b`,
        // `rest` is index 1.
        let rest = variable(arena, 1);
        let gamma = project(arena, rest, &[Side::Snd, Side::Snd, Side::Snd]);
        let rest = variable(arena, 1);
        let phi = project(arena, rest, &[Side::Snd, Side::Fst]);
        let body = judgment(arena, gamma, phi);
        let position = empty_positions(arena);
        let inner = lambda(arena, position, body);
        let domain = sub_payload(arena);
        lambda(arena, domain, inner)
    };
    let imp_next = {
        let inner_motive = {
            // `MiN s = Π(p : P). Π(_ : caseTwo(λ_.Type 0, Id00, Two,
            // s)). I` — under the `p` binder, `s` is index 1.
            let sub_tag = variable(arena, 1);
            let positions_motive = type_motive(arena);
            let unit = unit_position(arena);
            let two_positions = two(arena);
            let b_domain = case_two(arena, positions_motive, unit, two_positions, sub_tag);
            let codomain = judgment_type(arena);
            let inner = pi(arena, b_domain, codomain);
            let payload = rule_payload(arena);
            let body = pi(arena, payload, inner);
            let domain = two(arena);
            lambda(arena, domain, body)
        };
        let impi_next = {
            // `λp. λ(_ : Id Two zero zero). ⟨consF φ Γ, ψ⟩` — under
            // `b`, `p` is index 1.
            let p = variable(arena, 1);
            let phi = fst(arena, p);
            let p = variable(arena, 1);
            let gamma = project(arena, p, &[Side::Snd, Side::Snd]);
            let extended = cons_f(arena, phi, gamma);
            let p = variable(arena, 1);
            let psi = project(arena, p, &[Side::Snd, Side::Fst]);
            let body = judgment(arena, extended, psi);
            let position = unit_position(arena);
            let inner = lambda(arena, position, body);
            let domain = rule_payload(arena);
            lambda(arena, domain, inner)
        };
        let impe_next = {
            // `λp. λ(b : Two). caseTwo(λ_.I, ⟨Γ, impF φ ψ⟩, minor, b)`
            // — under `b`, `p` is index 1.
            let premise_motive = {
                let codomain = judgment_type(arena);
                let domain = two(arena);
                lambda(arena, domain, codomain)
            };
            let major = {
                // `⟨snd (snd p), impF (fst p) (fst (snd p))⟩`.
                let p = variable(arena, 1);
                let gamma = project(arena, p, &[Side::Snd, Side::Snd]);
                let p = variable(arena, 1);
                let phi = fst(arena, p);
                let p = variable(arena, 1);
                let psi = project(arena, p, &[Side::Snd, Side::Fst]);
                let implied = imp_f(arena, phi, psi);
                judgment(arena, gamma, implied)
            };
            let minor = minor(arena);
            let bound = variable(arena, 0);
            let body = case_two(arena, premise_motive, major, minor, bound);
            let position = two(arena);
            let inner = lambda(arena, position, body);
            let domain = rule_payload(arena);
            lambda(arena, domain, inner)
        };
        let rest = variable(arena, 0);
        let sub_tag = fst(arena, rest);
        let selected = case_two(arena, inner_motive, impi_next, impe_next, sub_tag);
        let rest = variable(arena, 0);
        let payload = snd(arena, rest);
        let body = apply(arena, selected, payload);
        let domain = sub_payload(arena);
        lambda(arena, domain, body)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, assm_next, imp_next, tag);
    let bound = variable(arena, 0);
    let rest = snd(arena, bound);
    let body = apply(arena, selected, rest);
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

fn next_index(arena: &mut TermArena) -> TermHandle {
    next_index_with(arena, |arena| {
        // `⟨snd (snd p), fst p⟩` — `⟨Γ, φ⟩` under `[p, b]`.
        let p = variable(arena, 1);
        let gamma = project(arena, p, &[Side::Snd, Side::Snd]);
        let p = variable(arena, 1);
        let phi = fst(arena, p);
        judgment(arena, gamma, phi)
    })
}

/// The derivation family: `Deriv ⟨Γ, φ⟩ = IW I A B out next ⟨Γ, φ⟩`.
fn derivation_family(arena: &mut TermArena) -> IndexedFamily {
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: judgment_type(arena),
        carrier: carrier(arena),
        children: branching(arena),
        out: out_index(arena),
        next: next_index(arena),
    }
}

/// The producer signature: the five scheme declarations then the four
/// grammar assumptions `Ctx`, `Frm`, `consF`, `impF`.
fn derivation_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Ctx
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Frm
    let cons_statement = {
        let formula = frm(arena);
        let context = ctx(arena);
        let tail = ctx(arena);
        let inner = pi(arena, context, tail);
        pi(arena, formula, inner)
    };
    declarations.push(Declaration::assumption(0, cons_statement)); // consF
    let imp_statement = {
        let antecedent = frm(arena);
        let consequent = frm(arena);
        let tail = frm(arena);
        let inner = pi(arena, consequent, tail);
        pi(arena, antecedent, inner)
    };
    declarations.push(Declaration::assumption(0, imp_statement)); // impF
    declarations
}

fn derivation_signature(arena: &mut TermArena) -> Signature {
    let declarations = derivation_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

/// `Deriv ⟨Γ, φ⟩` — the family applied at a judgment.
fn deriv_at(
    arena: &mut TermArena,
    family: &IndexedFamily,
    context: TermHandle,
    formula: TermHandle,
) -> TermHandle {
    let index = judgment(arena, context, formula);
    family.indexed_w(arena, index)
}

#[test]
fn the_derivation_scheme_signature_checks_and_the_family_forms() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = derivation_family(&mut arena);
    let declarations = derivation_declarations(&mut arena);
    assert_eq!(declarations.len(), 9);

    let before = budget.remaining();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 9);
    assert!(before - budget.remaining() > 0);

    // Formation: under `j : I`, `Deriv j` is a `Type 0` — the family's
    // `max(l, u, v)` lands on the closed level-0 description.
    let index_type = judgment_type(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);
    let index = variable(&mut arena, 0);
    let deriv = family.indexed_w(&mut arena, index);
    let sort = infer_sort(&mut arena, &context, deriv, &mut budget).unwrap();
    assert!(matches!(sort, Sort::Type(_)));
    let type_zero = type_sort(&mut arena, 0);
    check_type(&mut arena, &context, deriv, type_zero, &mut budget).unwrap();
}

/// The `IndexedAt` unfolding context `φ : Frm, ψ : Frm, Γ : Ctx,
/// k : Π(b : B label). W A B, j : I` for a caller-supplied label —
/// indices j = 0, k = 1, Γ = 2, ψ = 3, φ = 4.
fn condition_context(arena: &mut TermArena, family: &IndexedFamily, label: TermHandle) -> Context {
    // Under the [φ, ψ, Γ] prefix: φ = 2, ψ = 1, Γ = 0.
    let k_type = {
        let domain = apply(arena, family.children, label);
        let w = w_type(arena, family.carrier, family.children);
        pi(arena, domain, w)
    };
    let phi_type = frm(arena);
    let psi_type = frm(arena);
    let context_type = ctx(arena);
    let index_type = judgment_type(arena);
    Context::empty()
        .with_signature(derivation_signature(arena))
        .extend(phi_type)
        .extend(psi_type)
        .extend(context_type)
        .extend(k_type)
        .extend(index_type)
}

#[test]
fn the_indexing_condition_assigns_each_premise_its_judgment() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = derivation_family(&mut arena);

    // `impI`: `IndexedAt j (sup (impI-label) k)` unfolds to
    //   `Σ(_ : Id I ⟨Γ, impF φ ψ⟩ j). Π(b : B label). IndexedAt ⟨consF
    //      φ Γ, ψ⟩ (k b)`
    // — produced judgment `⟨Γ, imp φ ψ⟩`, and the one premise is
    // required at the *extended* context `cons φ Γ` with the bare
    // consequent `ψ`: the context index moves while the conclusion
    // index splits.
    let label = {
        let phi = variable(&mut arena, 2);
        let psi = variable(&mut arena, 1);
        let gamma = variable(&mut arena, 0);
        impi_label(&mut arena, phi, psi, gamma)
    };
    let impi_context = condition_context(&mut arena, &family, label);
    let node = {
        let phi = variable(&mut arena, 4);
        let psi = variable(&mut arena, 3);
        let gamma = variable(&mut arena, 2);
        let label = impi_label(&mut arena, phi, psi, gamma);
        let function = variable(&mut arena, 1);
        sup(&mut arena, family.carrier, family.children, label, function)
    };
    let index = variable(&mut arena, 0);
    let impi_condition = family.indexed_at(&mut arena, index, node);
    let impi_expected = {
        let domain = {
            let ty = judgment_type(&mut arena);
            let gamma = variable(&mut arena, 2);
            let phi = variable(&mut arena, 4);
            let psi = variable(&mut arena, 3);
            let implied = imp_f(&mut arena, phi, psi);
            let produced = judgment(&mut arena, gamma, implied);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, produced, index)
        };
        let codomain = {
            // Under the dead `_` binder: j = 1, k = 2, Γ = 3, ψ = 4,
            // φ = 5.
            let phi = variable(&mut arena, 5);
            let psi = variable(&mut arena, 4);
            let gamma = variable(&mut arena, 3);
            let label = impi_label(&mut arena, phi, psi, gamma);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                // Under `b`: j = 2, k = 3, Γ = 4, ψ = 5, φ = 6. The
                // required judgment is `⟨consF φ Γ, ψ⟩` — `next`'s
                // computed answer written reduced.
                let phi = variable(&mut arena, 6);
                let gamma = variable(&mut arena, 4);
                let extended = cons_f(&mut arena, phi, gamma);
                let psi = variable(&mut arena, 5);
                let required = judgment(&mut arena, extended, psi);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, required, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    let type_zero = type_sort(&mut arena, 0);
    check_type(
        &mut arena,
        &impi_context,
        impi_condition,
        type_zero,
        &mut budget,
    )
    .unwrap();
    check_type(
        &mut arena,
        &impi_context,
        impi_expected,
        type_zero,
        &mut budget,
    )
    .unwrap();
    assert!(
        convertible(
            &mut arena,
            &impi_context,
            impi_condition,
            impi_expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // The weakening misreading — the child required at the *same*
    // context `⟨Γ, ψ⟩` — is a different, false discipline: `consF φ Γ`
    // is a neutral `consF` application, never convertible with `Γ`.
    let unextended = {
        let domain = {
            let ty = judgment_type(&mut arena);
            let gamma = variable(&mut arena, 2);
            let phi = variable(&mut arena, 4);
            let psi = variable(&mut arena, 3);
            let implied = imp_f(&mut arena, phi, psi);
            let produced = judgment(&mut arena, gamma, implied);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, produced, index)
        };
        let codomain = {
            let phi = variable(&mut arena, 5);
            let psi = variable(&mut arena, 4);
            let gamma = variable(&mut arena, 3);
            let label = impi_label(&mut arena, phi, psi, gamma);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                let gamma = variable(&mut arena, 4);
                let psi = variable(&mut arena, 5);
                let required = judgment(&mut arena, gamma, psi);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, required, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(
        &mut arena,
        &impi_context,
        unextended,
        type_zero,
        &mut budget,
    )
    .unwrap();
    assert!(
        !convertible(
            &mut arena,
            &impi_context,
            impi_condition,
            unextended,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // `impE`: `IndexedAt j (sup (impE-label) k)` unfolds to
    //   `Σ(_ : Id I ⟨Γ, ψ⟩ j). Π(b : Two). IndexedAt (next label b)
    //      (k b)`
    // — produced conclusion `ψ`, and the required index is `next`'s
    // `caseTwo` on the position itself.
    let label = {
        let phi = variable(&mut arena, 2);
        let psi = variable(&mut arena, 1);
        let gamma = variable(&mut arena, 0);
        impe_label(&mut arena, phi, psi, gamma)
    };
    let impe_context = condition_context(&mut arena, &family, label);
    let node = {
        let phi = variable(&mut arena, 4);
        let psi = variable(&mut arena, 3);
        let gamma = variable(&mut arena, 2);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let function = variable(&mut arena, 1);
        sup(&mut arena, family.carrier, family.children, label, function)
    };
    let index = variable(&mut arena, 0);
    let impe_condition = family.indexed_at(&mut arena, index, node);
    let impe_expected = {
        let domain = {
            let ty = judgment_type(&mut arena);
            let gamma = variable(&mut arena, 2);
            let psi = variable(&mut arena, 3);
            let produced = judgment(&mut arena, gamma, psi);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, produced, index)
        };
        let codomain = {
            // Under `_`: j = 1, k = 2, Γ = 3, ψ = 4, φ = 5.
            let phi = variable(&mut arena, 5);
            let psi = variable(&mut arena, 4);
            let gamma = variable(&mut arena, 3);
            let label = impe_label(&mut arena, phi, psi, gamma);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                // Under `b`: j = 2, k = 3, Γ = 4, ψ = 5, φ = 6.
                let phi = variable(&mut arena, 6);
                let psi = variable(&mut arena, 5);
                let gamma = variable(&mut arena, 4);
                let label = impe_label(&mut arena, phi, psi, gamma);
                let next_a = apply(&mut arena, family.next, label);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, required, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(
        &mut arena,
        &impe_context,
        impe_condition,
        type_zero,
        &mut budget,
    )
    .unwrap();
    check_type(
        &mut arena,
        &impe_context,
        impe_expected,
        type_zero,
        &mut budget,
    )
    .unwrap();
    assert!(
        convertible(
            &mut arena,
            &impe_context,
            impe_condition,
            impe_expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // Position selection itself is definitional: the major position
    // `zero` requires `⟨Γ, impF φ ψ⟩`, the minor `one` requires
    // `⟨Γ, φ⟩` — `next label b` computes to a different judgment at
    // each.
    let index_type = judgment_type(&mut arena);
    let at_major = {
        let phi = variable(&mut arena, 4);
        let psi = variable(&mut arena, 3);
        let gamma = variable(&mut arena, 2);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let next_a = apply(&mut arena, family.next, label);
        let position = two_zero(&mut arena);
        apply(&mut arena, next_a, position)
    };
    check_type(&mut arena, &impe_context, at_major, index_type, &mut budget).unwrap();
    let major_judgment = {
        let gamma = variable(&mut arena, 2);
        let phi = variable(&mut arena, 4);
        let psi = variable(&mut arena, 3);
        let implied = imp_f(&mut arena, phi, psi);
        judgment(&mut arena, gamma, implied)
    };
    assert!(
        convertible(
            &mut arena,
            &impe_context,
            at_major,
            major_judgment,
            index_type,
            &mut budget
        )
        .unwrap()
    );
    let at_minor = {
        let phi = variable(&mut arena, 4);
        let psi = variable(&mut arena, 3);
        let gamma = variable(&mut arena, 2);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let next_a = apply(&mut arena, family.next, label);
        let position = two_one(&mut arena);
        apply(&mut arena, next_a, position)
    };
    let minor_judgment = {
        let gamma = variable(&mut arena, 2);
        let phi = variable(&mut arena, 4);
        judgment(&mut arena, gamma, phi)
    };
    assert!(
        convertible(
            &mut arena,
            &impe_context,
            at_minor,
            minor_judgment,
            index_type,
            &mut budget
        )
        .unwrap()
    );

    // `assm`: `IndexedAt j (sup (assm-label) k)` unfolds to
    //   `Σ(_ : Id I ⟨consF φ Γ, φ⟩ j). Π(b : Id Two zero one).
    //      IndexedAt (next label b) (k b)`
    // — produced judgment `⟨cons φ Γ, φ⟩`: the leaf's context index
    // is the *extended* context and its conclusion its head.
    let label = {
        let phi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 0);
        assm_label(&mut arena, phi, gamma)
    };
    let assm_context = condition_context(&mut arena, &family, label);
    let node = {
        let phi = variable(&mut arena, 4);
        let gamma = variable(&mut arena, 2);
        let label = assm_label(&mut arena, phi, gamma);
        let function = variable(&mut arena, 1);
        sup(&mut arena, family.carrier, family.children, label, function)
    };
    let index = variable(&mut arena, 0);
    let assm_condition = family.indexed_at(&mut arena, index, node);
    let assm_expected = {
        let domain = {
            let ty = judgment_type(&mut arena);
            let phi = variable(&mut arena, 4);
            let gamma = variable(&mut arena, 2);
            let extended = cons_f(&mut arena, phi, gamma);
            let phi = variable(&mut arena, 4);
            let produced = judgment(&mut arena, extended, phi);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, produced, index)
        };
        let codomain = {
            // Under `_`: j = 1, k = 2, Γ = 3, ψ = 4, φ = 5.
            let phi = variable(&mut arena, 5);
            let gamma = variable(&mut arena, 3);
            let label = assm_label(&mut arena, phi, gamma);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                // Under `b`: j = 2, k = 3, Γ = 4, ψ = 5, φ = 6.
                let phi = variable(&mut arena, 6);
                let gamma = variable(&mut arena, 4);
                let label = assm_label(&mut arena, phi, gamma);
                let next_a = apply(&mut arena, family.next, label);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, required, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(
        &mut arena,
        &assm_context,
        assm_condition,
        type_zero,
        &mut budget,
    )
    .unwrap();
    check_type(
        &mut arena,
        &assm_context,
        assm_expected,
        type_zero,
        &mut budget,
    )
    .unwrap();
    assert!(
        convertible(
            &mut arena,
            &assm_context,
            assm_condition,
            assm_expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );
}

/// The constructor-side context `φ : Frm, ψ : Frm, Γ : Ctx,
/// dI : Deriv ⟨consF φ Γ, ψ⟩, dM : Deriv ⟨Γ, impF φ ψ⟩, dm : Deriv ⟨Γ,
/// φ⟩, g' : Π(b : B assm-label). Deriv (next assm-label b)` — indices
/// g' = 0, dm = 1, dM = 2, dI = 3, Γ = 4, ψ = 5, φ = 6.
fn constructor_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    // Under the [φ, ψ, Γ] prefix: φ = 2, ψ = 1, Γ = 0.
    let intro_premise = {
        let phi = variable(arena, 2);
        let gamma = variable(arena, 0);
        let extended = cons_f(arena, phi, gamma);
        let psi = variable(arena, 1);
        deriv_at(arena, family, extended, psi)
    };
    // Under [φ, ψ, Γ, dI]: Γ = 1, ψ = 2, φ = 3.
    let major_premise = {
        let gamma = variable(arena, 1);
        let phi = variable(arena, 3);
        let psi = variable(arena, 2);
        let implied = imp_f(arena, phi, psi);
        deriv_at(arena, family, gamma, implied)
    };
    // Under [φ, ψ, Γ, dI, dM]: Γ = 2, ψ = 3, φ = 4.
    let minor_premise = {
        let gamma = variable(arena, 2);
        let phi = variable(arena, 4);
        deriv_at(arena, family, gamma, phi)
    };
    // Under [φ, ψ, Γ, dI, dM, dm]: Γ = 3, ψ = 4, φ = 5.
    let leaf_children = {
        let phi = variable(arena, 5);
        let gamma = variable(arena, 3);
        let label = assm_label(arena, phi, gamma);
        let domain = apply(arena, family.children, label);
        let codomain = {
            // Under `b`: Γ = 4, ψ = 5, φ = 6.
            let phi = variable(arena, 6);
            let gamma = variable(arena, 4);
            let label = assm_label(arena, phi, gamma);
            let next_a = apply(arena, family.next, label);
            let bound = variable(arena, 0);
            let required = apply(arena, next_a, bound);
            family.indexed_w(arena, required)
        };
        pi(arena, domain, codomain)
    };
    let phi_type = frm(arena);
    let psi_type = frm(arena);
    let context_type = ctx(arena);
    Context::empty()
        .with_signature(derivation_signature(arena))
        .extend(phi_type)
        .extend(psi_type)
        .extend(context_type)
        .extend(intro_premise)
        .extend(major_premise)
        .extend(minor_premise)
        .extend(leaf_children)
}

#[test]
fn constructors_build_at_their_judgment_indices() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = derivation_family(&mut arena);
    let context = constructor_context(&mut arena, &family);
    // In Γ: g' = 0, dm = 1, dM = 2, dI = 3, Γ = 4, ψ = 5, φ = 6.

    // `assm φ Γ := isup (assm-label) g'` for the neutral — vacuous —
    // child function `g'`: it builds at `Deriv ⟨consF φ Γ, φ⟩`, the
    // extended context with the hypothesis itself as conclusion.
    let assm = {
        let phi = variable(&mut arena, 6);
        let gamma = variable(&mut arena, 4);
        let label = assm_label(&mut arena, phi, gamma);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, label, function)
    };
    let expected = {
        let phi = variable(&mut arena, 6);
        let gamma = variable(&mut arena, 4);
        let extended = cons_f(&mut arena, phi, gamma);
        let phi = variable(&mut arena, 6);
        deriv_at(&mut arena, &family, extended, phi)
    };
    check_type(&mut arena, &context, assm, expected, &mut budget).unwrap();

    // `impI φ ψ Γ dI := isup (impI-label) (λ(_ : B label). dI)` — the
    // unit-position child function returns the premise `dI`, whose
    // required judgment `next` computes to `⟨consF φ Γ, ψ⟩`.
    let intro = {
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impi_label(&mut arena, phi, psi, gamma);
        let children_fn = {
            // The domain annotation sits at ambient level; under the
            // `b` binder itself: g' = 1, dm = 2, dM = 3, dI = 4.
            let phi = variable(&mut arena, 6);
            let psi = variable(&mut arena, 5);
            let gamma = variable(&mut arena, 4);
            let label = impi_label(&mut arena, phi, psi, gamma);
            let domain = apply(&mut arena, family.children, label);
            let premise = variable(&mut arena, 4);
            lambda(&mut arena, domain, premise)
        };
        family.sup(&mut arena, label, children_fn)
    };
    let expected = {
        let gamma = variable(&mut arena, 4);
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let implied = imp_f(&mut arena, phi, psi);
        deriv_at(&mut arena, &family, gamma, implied)
    };
    check_type(&mut arena, &context, intro, expected, &mut budget).unwrap();

    // `impE φ ψ Γ dM dm := isup (impE-label) g` where
    // `g = λ(b : Two). caseTwo(λw. Deriv (next label w), dM, dm, b)` —
    // the child function *selects* the premise per position: the major
    // `dM` at `zero`, the minor `dm` at `one`. Its motive is the
    // required-judgment family itself, so the case checks exactly when
    // each branch proves the judgment `next` assigns its position.
    let mp_children_fn = {
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            // Under `b`: g' = 1, dm = 2, dM = 3, dI = 4, Γ = 5, ψ = 6,
            // φ = 7.
            let motive = {
                // `λ(w : Two). Deriv (next label w)` — under `w`:
                // b = 1, g' = 2, dm = 3, dM = 4, dI = 5, Γ = 6, ψ = 7,
                // φ = 8.
                let phi = variable(&mut arena, 8);
                let psi = variable(&mut arena, 7);
                let gamma = variable(&mut arena, 6);
                let label = impe_label(&mut arena, phi, psi, gamma);
                let next_a = apply(&mut arena, family.next, label);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let codomain = family.indexed_w(&mut arena, required);
                let domain = two(&mut arena);
                lambda(&mut arena, domain, codomain)
            };
            let major = variable(&mut arena, 3);
            let minor = variable(&mut arena, 2);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, major, minor, bound)
        };
        lambda(&mut arena, domain, body)
    };
    let mp = {
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        family.sup(&mut arena, label, mp_children_fn)
    };
    let expected = {
        let gamma = variable(&mut arena, 4);
        let psi = variable(&mut arena, 5);
        deriv_at(&mut arena, &family, gamma, psi)
    };
    check_type(&mut arena, &context, mp, expected, &mut budget).unwrap();

    // The two-premise composition: `impE` applied to the checked
    // `impI` node — `impE φ ψ Γ (impI φ ψ Γ dI) dm` is a two-node
    // derivation of `⟨Γ, ψ⟩` from a neutral `dI` at `⟨consF φ Γ, ψ⟩`
    // and `dm` at `⟨Γ, φ⟩`.
    let composite_children_fn = {
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            // Under `b`: intro's label positions are g' = 2 … φ = 8
            // inside `motive`, dI = 5 inside the inner label below.
            let motive = {
                let phi = variable(&mut arena, 8);
                let psi = variable(&mut arena, 7);
                let gamma = variable(&mut arena, 6);
                let label = impe_label(&mut arena, phi, psi, gamma);
                let next_a = apply(&mut arena, family.next, label);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let codomain = family.indexed_w(&mut arena, required);
                let domain = two(&mut arena);
                lambda(&mut arena, domain, codomain)
            };
            let shifted = shift(&mut arena, intro, 0, 1);
            let minor = variable(&mut arena, 2);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, shifted, minor, bound)
        };
        lambda(&mut arena, domain, body)
    };
    let composite = {
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        family.sup(&mut arena, label, composite_children_fn)
    };
    let expected = {
        let gamma = variable(&mut arena, 4);
        let psi = variable(&mut arena, 5);
        deriv_at(&mut arena, &family, gamma, psi)
    };
    check_type(&mut arena, &context, composite, expected, &mut budget).unwrap();
}

/// `Π(j : I). Π(_ : Deriv j). Type 0` — the `iindW` motive shape.
fn motive_type(arena: &mut TermArena, family: &IndexedFamily) -> TermHandle {
    let index = variable(arena, 0);
    let family_at = family.indexed_w(arena, index);
    let type_zero = type_sort(arena, 0);
    let inner = pi(arena, family_at, type_zero);
    let index_type = judgment_type(arena);
    pi(arena, index_type, inner)
}

/// `Π(a : A). Π(g : Π(b : B a). Deriv (next a b)). Π(_ : Π(b : B a).
/// Q (next a b) (g b)). Q (out a) (isup a g)` — the `iindW` step type,
/// written under the one-binding prefix `[Q]`.
fn step_type(arena: &mut TermArena, family: &IndexedFamily) -> TermHandle {
    let g_type = {
        // Under `a`: a = 0, Q = 1.
        let bound = variable(arena, 0);
        let b_domain = apply(arena, family.children, bound);
        let codomain = {
            // Under `b`: a = 1, Q = 2.
            let bound_a = variable(arena, 1);
            let next_a = apply(arena, family.next, bound_a);
            let bound = variable(arena, 0);
            let required = apply(arena, next_a, bound);
            family.indexed_w(arena, required)
        };
        pi(arena, b_domain, codomain)
    };
    let hypothesis_type = {
        // Under `g`, `a`: a = 1, g = 0, Q = 2.
        let bound_a = variable(arena, 1);
        let b_domain = apply(arena, family.children, bound_a);
        let codomain = {
            // Under `b`: a = 2, g = 1, Q = 3.
            let bound_a = variable(arena, 2);
            let next_a = apply(arena, family.next, bound_a);
            let bound = variable(arena, 0);
            let required = apply(arena, next_a, bound);
            let motive = variable(arena, 3);
            let at_index = apply(arena, motive, required);
            let bound_g = variable(arena, 1);
            let bound = variable(arena, 0);
            let child = apply(arena, bound_g, bound);
            apply(arena, at_index, child)
        };
        pi(arena, b_domain, codomain)
    };
    let result = {
        // Under the hypothesis, `g`, `a`: a = 2, g = 1, Q = 3.
        let bound_a = variable(arena, 2);
        let produced = apply(arena, family.out, bound_a);
        let bound_a = variable(arena, 2);
        let bound_g = variable(arena, 1);
        let node = family.sup(arena, bound_a, bound_g);
        let motive = variable(arena, 3);
        let at_index = apply(arena, motive, produced);
        apply(arena, at_index, node)
    };
    let inner = pi(arena, hypothesis_type, result);
    let middle = pi(arena, g_type, inner);
    let domain = carrier(arena);
    pi(arena, domain, middle)
}

/// The eliminator context `Q : Π(j : I). Π(_ : Deriv j). Type 0`,
/// `s : <iindW step>`, `φ : Frm`, `ψ : Frm`, `Γ : Ctx`,
/// `g : Π(b : B impE-label). Deriv (next impE-label b)` — indices
/// g = 0, Γ = 1, ψ = 2, φ = 3, s = 4, Q = 5.
fn eliminator_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let motive = motive_type(arena, family);
    let step = step_type(arena, family);
    let phi_type = frm(arena);
    let psi_type = frm(arena);
    let context_type = ctx(arena);
    // Under [Q, s, φ, ψ, Γ]: Γ = 0, ψ = 1, φ = 2.
    let g_type = {
        let phi = variable(arena, 2);
        let psi = variable(arena, 1);
        let gamma = variable(arena, 0);
        let label = impe_label(arena, phi, psi, gamma);
        let domain = apply(arena, family.children, label);
        let codomain = {
            // Under `b`: Γ = 1, ψ = 2, φ = 3.
            let phi = variable(arena, 3);
            let psi = variable(arena, 2);
            let gamma = variable(arena, 1);
            let label = impe_label(arena, phi, psi, gamma);
            let next_a = apply(arena, family.next, label);
            let bound = variable(arena, 0);
            let required = apply(arena, next_a, bound);
            family.indexed_w(arena, required)
        };
        pi(arena, domain, codomain)
    };
    Context::empty()
        .with_signature(derivation_signature(arena))
        .extend(motive)
        .extend(step)
        .extend(phi_type)
        .extend(psi_type)
        .extend(context_type)
        .extend(g_type)
}

#[test]
fn induction_computes_on_modus_ponens_with_a_neutral_child_function() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = derivation_family(&mut arena);
    let context = eliminator_context(&mut arena, &family);
    // In Γ: g = 0, Γ = 1, ψ = 2, φ = 3, s = 4, Q = 5.

    // `mp = isup (impE-label) g` with `g` the neutral child function:
    // an `impE` node over arbitrary premises.
    let mp = {
        let phi = variable(&mut arena, 3);
        let psi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 1);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, label, function)
    };

    // `iindW Q s ⟨Γ, ψ⟩ mp : Q ⟨Γ, ψ⟩ mp`.
    let motive = variable(&mut arena, 5);
    let step = variable(&mut arena, 4);
    let gamma = variable(&mut arena, 1);
    let psi = variable(&mut arena, 2);
    let index = judgment(&mut arena, gamma, psi);
    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, index, mp);
    let shared = {
        let motive = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 1);
        let psi = variable(&mut arena, 2);
        let index = judgment(&mut arena, gamma, psi);
        let at_index = apply(&mut arena, motive, index);
        apply(&mut arena, at_index, mp)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // The profile's constructor computation judgment, `g` still
    // neutral:
    //   `iindW Q s ⟨Γ, ψ⟩ (isup (impE-label) g)`
    //     `≡ s (impE-label) g (b ↦ iindW Q s (next label b) (g b))`
    // — the induction hypothesis's index is `next`'s stuck `caseTwo`
    // on `b`: applied at `zero` it is a `Q ⟨Γ, impF φ ψ⟩ (g zero)`
    // major hypothesis, at `one` a `Q ⟨Γ, φ⟩ (g one)` minor one.
    let hypothesis = {
        // The domain annotation sits at ambient level (φ = 3, ψ = 2,
        // Γ = 1); under the `b` binder itself: g = 1, Γ = 2, ψ = 3,
        // φ = 4, s = 5, Q = 6.
        let phi = variable(&mut arena, 3);
        let psi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 1);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            let phi = variable(&mut arena, 4);
            let psi = variable(&mut arena, 3);
            let gamma = variable(&mut arena, 2);
            let label = impe_label(&mut arena, phi, psi, gamma);
            let next_a = apply(&mut arena, family.next, label);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            let function = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, function, bound);
            let motive = variable(&mut arena, 6);
            let step = variable(&mut arena, 5);
            family.ind(
                &mut arena,
                Level::Constant(0),
                motive,
                step,
                required,
                child,
            )
        };
        lambda(&mut arena, domain, body)
    };
    let expected = {
        let step = variable(&mut arena, 4);
        let phi = variable(&mut arena, 3);
        let psi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 1);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let at_node = apply(&mut arena, step, label);
        let function = variable(&mut arena, 0);
        let at_children = apply(&mut arena, at_node, function);
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

    // Work receipt: checking the `iindW` application and closing the
    // computation across the packed `IW` pair and the position-selected
    // `next` costs a measured 25_100 budgeted steps.
    assert_eq!(total - budget.remaining(), 25_100);
}

#[test]
fn induction_with_a_dependent_motive_proves_the_conclusion_identity() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = derivation_family(&mut arena);

    // A concrete dependent motive: `Q j _ = Id Frm (snd j) (snd j)` —
    // the induction's conclusion mentions the index, so `Q ⟨Γ, ψ⟩ t`
    // is the identity `ψ ≡ ψ` on the judgment's conclusion component.
    // The step `s = λa. λg. λih. refl Frm (snd (out a))` is checked at
    // `Q (out a) (isup a g)`, which unfolds to exactly `Id Frm (snd
    // (out a)) (snd (out a))`.
    let motive = {
        // `λ(j : I). λ(_ : Deriv j). Id Frm (snd j) (snd j)`.
        let family_at = {
            let index = variable(&mut arena, 0);
            family.indexed_w(&mut arena, index)
        };
        let bound = variable(&mut arena, 1);
        let conclusion = snd(&mut arena, bound);
        let ty = frm(&mut arena);
        let body = id(&mut arena, ty, conclusion, conclusion);
        let inner = lambda(&mut arena, family_at, body);
        let domain = judgment_type(&mut arena);
        lambda(&mut arena, domain, inner)
    };
    let step = {
        // `λ(a : A). λ(g : Π(b : B a). Deriv (next a b)).
        //    λ(_ : Π(b : B a). Q (next a b) (g b)). refl Frm (snd
        //    (out a))`.
        let g_type = {
            // Under `a`: a = 0.
            let bound = variable(&mut arena, 0);
            let b_domain = apply(&mut arena, family.children, bound);
            let codomain = {
                // Under `b`: a = 1.
                let bound_a = variable(&mut arena, 1);
                let next_a = apply(&mut arena, family.next, bound_a);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                family.indexed_w(&mut arena, required)
            };
            pi(&mut arena, b_domain, codomain)
        };
        let hypothesis_type = {
            // Under `g`, `a`: a = 1, g = 0.
            let bound_a = variable(&mut arena, 1);
            let b_domain = apply(&mut arena, family.children, bound_a);
            let codomain = {
                // Under `b`: a = 2, g = 1.
                let bound_a = variable(&mut arena, 2);
                let next_a = apply(&mut arena, family.next, bound_a);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let bound_g = variable(&mut arena, 1);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, bound_g, bound);
                let at_index = apply(&mut arena, motive, required);
                apply(&mut arena, at_index, child)
            };
            pi(&mut arena, b_domain, codomain)
        };
        let body = {
            // Under the hypothesis, `g`, `a`: a = 2.
            let bound_a = variable(&mut arena, 2);
            let produced = apply(&mut arena, family.out, bound_a);
            let conclusion = snd(&mut arena, produced);
            let ty = frm(&mut arena);
            refl(&mut arena, ty, conclusion)
        };
        let inner = lambda(&mut arena, hypothesis_type, body);
        let middle = lambda(&mut arena, g_type, inner);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, middle)
    };

    // Γ = φ : Frm, ψ : Frm, Γ : Ctx,
    //     g : Π(b : B impE-label). Deriv (next impE-label b) —
    // indices g = 0, Γ = 1, ψ = 2, φ = 3.
    let phi_type = frm(&mut arena);
    let psi_type = frm(&mut arena);
    let context_type = ctx(&mut arena);
    let g_type = {
        let phi = variable(&mut arena, 2);
        let psi = variable(&mut arena, 1);
        let gamma = variable(&mut arena, 0);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let domain = apply(&mut arena, family.children, label);
        let codomain = {
            // Under `b`: Γ = 1, ψ = 2, φ = 3.
            let phi = variable(&mut arena, 3);
            let psi = variable(&mut arena, 2);
            let gamma = variable(&mut arena, 1);
            let label = impe_label(&mut arena, phi, psi, gamma);
            let next_a = apply(&mut arena, family.next, label);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            family.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = Context::empty()
        .with_signature(derivation_signature(&mut arena))
        .extend(phi_type)
        .extend(psi_type)
        .extend(context_type)
        .extend(g_type);

    let mp = {
        let phi = variable(&mut arena, 3);
        let psi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 1);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, label, function)
    };
    let gamma = variable(&mut arena, 1);
    let psi = variable(&mut arena, 2);
    let index = judgment(&mut arena, gamma, psi);
    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, index, mp);

    // `iindW Q s ⟨Γ, ψ⟩ mp : Id Frm ψ ψ`, and the whole elimination
    // *computes* to `refl Frm ψ`: the induction over the `impE` node
    // produces the reflexivity proof of the judgment's conclusion.
    let ty = frm(&mut arena);
    let psi = variable(&mut arena, 2);
    let shared = id(&mut arena, ty, psi, psi);
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();
    let ty = frm(&mut arena);
    let psi = variable(&mut arena, 2);
    let reflected = refl(&mut arena, ty, psi);
    assert!(
        convertible(
            &mut arena,
            &context,
            elimination,
            reflected,
            shared,
            &mut budget
        )
        .unwrap()
    );

    // Work receipt: a measured 3_689 budgeted steps.
    assert_eq!(total - budget.remaining(), 3_689);
}

#[test]
fn derivations_reject_wrong_judgments_and_malformed_descriptions() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = derivation_family(&mut arena);
    let context = constructor_context(&mut arena, &family);
    // In Γ: g' = 0, dm = 1, dM = 2, dI = 3, Γ = 4, ψ = 5, φ = 6.

    let mp_children_fn = |arena: &mut TermArena, major: TermHandle, minor: TermHandle| {
        // The domain annotation sits at ambient level (φ = 6, ψ = 5,
        // Γ = 4); under the `b` binder itself: g' = 1, dm = 2, dM = 3,
        // dI = 4, Γ = 5, ψ = 6, φ = 7.
        let phi = variable(arena, 6);
        let psi = variable(arena, 5);
        let gamma = variable(arena, 4);
        let label = impe_label(arena, phi, psi, gamma);
        let domain = apply(arena, family.children, label);
        let body = {
            let motive = {
                // Under `w`: φ = 8, ψ = 7, Γ = 6.
                let phi = variable(arena, 8);
                let psi = variable(arena, 7);
                let gamma = variable(arena, 6);
                let label = impe_label(arena, phi, psi, gamma);
                let next_a = apply(arena, family.next, label);
                let bound = variable(arena, 0);
                let required = apply(arena, next_a, bound);
                let codomain = family.indexed_w(arena, required);
                let domain = two(arena);
                lambda(arena, domain, codomain)
            };
            let bound = variable(arena, 0);
            case_two(arena, motive, major, minor, bound)
        };
        lambda(arena, domain, body)
    };

    let mp = {
        let children_fn = {
            let major = variable(&mut arena, 3);
            let minor = variable(&mut arena, 2);
            mp_children_fn(&mut arena, major, minor)
        };
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        family.sup(&mut arena, label, children_fn)
    };

    // An `impE` node never lands at its minor premise's judgment:
    // `out` computes `⟨Γ, ψ⟩`, not `⟨Γ, φ⟩`.
    let wrong_conclusion = {
        let gamma = variable(&mut arena, 4);
        let phi = variable(&mut arena, 6);
        deriv_at(&mut arena, &family, gamma, phi)
    };
    let error = check_type(&mut arena, &context, mp, wrong_conclusion, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // …nor at the extended context `⟨consF φ Γ, ψ⟩` that `impI`'s
    // premise occupies.
    let wrong_context = {
        let phi = variable(&mut arena, 6);
        let gamma = variable(&mut arena, 4);
        let extended = cons_f(&mut arena, phi, gamma);
        let psi = variable(&mut arena, 5);
        deriv_at(&mut arena, &family, extended, psi)
    };
    let error = check_type(&mut arena, &context, mp, wrong_context, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // Swapped premises are a different (and rejected) derivation: the
    // `zero` branch must prove `⟨Γ, impF φ ψ⟩`, not `⟨Γ, φ⟩`.
    let swapped = {
        let children_fn = {
            let major = variable(&mut arena, 2);
            let minor = variable(&mut arena, 3);
            mp_children_fn(&mut arena, major, minor)
        };
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        family.sup(&mut arena, label, children_fn)
    };
    let expected = {
        let gamma = variable(&mut arena, 4);
        let psi = variable(&mut arena, 5);
        deriv_at(&mut arena, &family, gamma, psi)
    };
    let error = check_type(&mut arena, &context, swapped, expected, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // An `assm` leaf never lands at the unextended context either:
    // `out` computes `⟨consF φ Γ, φ⟩`, not `⟨Γ, φ⟩`.
    let assm = {
        let phi = variable(&mut arena, 6);
        let gamma = variable(&mut arena, 4);
        let label = assm_label(&mut arena, phi, gamma);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, label, function)
    };
    let unextended = {
        let gamma = variable(&mut arena, 4);
        let phi = variable(&mut arena, 6);
        deriv_at(&mut arena, &family, gamma, phi)
    };
    let error = check_type(&mut arena, &context, assm, unextended, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A producer defect: a `next` whose `impE` minor position requires
    // the *conclusion* `⟨Γ, ψ⟩` instead of the antecedent `⟨Γ, φ⟩` is
    // still a well-typed description — but derivations built against
    // it reject, since `dm : Deriv ⟨Γ, φ⟩` no longer answers the
    // computed `next label one`.
    let mut captured = family.clone();
    captured.next = next_index_with(&mut arena, |arena| {
        // The defect: `⟨Γ, ψ⟩` — the conclusion, not `⟨Γ, φ⟩` —
        // built under `[p, b]` like the real minor branch.
        let p = variable(arena, 1);
        let gamma = project(arena, p, &[Side::Snd, Side::Snd]);
        let p = variable(arena, 1);
        let psi = project(arena, p, &[Side::Snd, Side::Fst]);
        judgment(arena, gamma, psi)
    });

    // The defective family still forms — the description is well-typed —
    let index = {
        let gamma = variable(&mut arena, 4);
        let psi = variable(&mut arena, 5);
        judgment(&mut arena, gamma, psi)
    };
    let malformed_member = captured.indexed_w(&mut arena, index);
    let type_zero = type_sort(&mut arena, 0);
    check_type(
        &mut arena,
        &context,
        malformed_member,
        type_zero,
        &mut budget,
    )
    .unwrap();
    // …but `impE`'s two premises under it require `Deriv ⟨Γ, ψ⟩` at the
    // minor position, which `dm : Deriv ⟨Γ, φ⟩` cannot supply.
    let captured_children = {
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let domain = apply(&mut arena, captured.children, label);
        let body = {
            let motive = {
                let phi = variable(&mut arena, 8);
                let psi = variable(&mut arena, 7);
                let gamma = variable(&mut arena, 6);
                let label = impe_label(&mut arena, phi, psi, gamma);
                let next_a = apply(&mut arena, captured.next, label);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let codomain = captured.indexed_w(&mut arena, required);
                let domain = two(&mut arena);
                lambda(&mut arena, domain, codomain)
            };
            let major = variable(&mut arena, 3);
            let minor = variable(&mut arena, 2);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, major, minor, bound)
        };
        lambda(&mut arena, domain, body)
    };
    let captured_mp = {
        let phi = variable(&mut arena, 6);
        let psi = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 4);
        let label = impe_label(&mut arena, phi, psi, gamma);
        captured.sup(&mut arena, label, captured_children)
    };
    let captured_expected = {
        let gamma = variable(&mut arena, 4);
        let psi = variable(&mut arena, 5);
        let index = judgment(&mut arena, gamma, psi);
        captured.indexed_w(&mut arena, index)
    };
    let error = check_type(
        &mut arena,
        &context,
        captured_mp,
        captured_expected,
        &mut budget,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // An *ill-typed* `next` is malformed outright: answering the minor
    // position with the bare conclusion `ψ : Frm` where a judgment `I`
    // is required fails inside the description's own checking.
    let mut ill_typed = family.clone();
    ill_typed.next = next_index_with(&mut arena, |arena| {
        // `fst (snd p) : Frm` where a judgment `I` is required — the
        // description's own checking rejects it.
        let p = variable(arena, 1);
        project(arena, p, &[Side::Snd, Side::Fst])
    });
    let index = {
        let gamma = variable(&mut arena, 4);
        let psi = variable(&mut arena, 5);
        judgment(&mut arena, gamma, psi)
    };
    let malformed = ill_typed.indexed_w(&mut arena, index);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A branching family into `Strict` is an illegal strict elimination,
    // rejected by `caseTwo`'s own motive rule before `W` formation ever
    // runs.
    let mut strict = family.clone();
    strict.children = {
        let motive = {
            let domain = two(&mut arena);
            let codomain = strict_sort(&mut arena, 0);
            lambda(&mut arena, domain, codomain)
        };
        let empty = empty_positions(&mut arena);
        let unit = unit_position(&mut arena);
        let positions = two(&mut arena);
        let bound = variable(&mut arena, 0);
        let rest = snd(&mut arena, bound);
        let sub_tag = fst(&mut arena, rest);
        let inner = case_two(&mut arena, motive, unit, positions, sub_tag);
        let bound = variable(&mut arena, 0);
        let tag = fst(&mut arena, bound);
        let body = case_two(&mut arena, motive, empty, inner, tag);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let index = {
        let gamma = variable(&mut arena, 4);
        let psi = variable(&mut arena, 5);
        judgment(&mut arena, gamma, psi)
    };
    let malformed = strict.indexed_w(&mut arena, index);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::StrictCaseMotiveCodomain { .. } | CoreError::TypeMismatch { .. }
    ));

    // The eliminator is universe-polymorphic over the motive level `w`:
    // claiming `w = 1` while `Q` lands at `Type 0` is a bad universe,
    // not a different judgment.
    let context = eliminator_context(&mut arena, &family);
    // In Γ: g = 0, Γ = 1, ψ = 2, φ = 3, s = 4, Q = 5.
    let mp = {
        let phi = variable(&mut arena, 3);
        let psi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 1);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, label, function)
    };
    let motive = variable(&mut arena, 5);
    let step = variable(&mut arena, 4);
    let gamma = variable(&mut arena, 1);
    let psi = variable(&mut arena, 2);
    let index = judgment(&mut arena, gamma, psi);
    let elimination = family.ind(&mut arena, Level::Constant(1), motive, step, index, mp);
    let shared = {
        let motive = variable(&mut arena, 5);
        let gamma = variable(&mut arena, 1);
        let psi = variable(&mut arena, 2);
        let index = judgment(&mut arena, gamma, psi);
        let at_index = apply(&mut arena, motive, index);
        apply(&mut arena, at_index, mp)
    };
    let error = check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));
}

#[test]
fn a_derivation_certificate_verifies_with_exact_closure_and_bounded_cost() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = derivation_family(&mut arena);
    let declarations = derivation_declarations(&mut arena);

    // The certificate's judgment: `φ : Frm, ψ : Frm, Γ : Ctx,
    // dI : Deriv ⟨consF φ Γ, ψ⟩, dm : Deriv ⟨Γ, φ⟩ ⊢
    //   impE φ ψ Γ (impI φ ψ Γ dI) dm : Deriv ⟨Γ, ψ⟩`
    // — a two-node derivation from neutral premises, re-decided from
    // data, signature included.
    let intro_premise = {
        // Under [φ, ψ, Γ]: φ = 2, ψ = 1, Γ = 0.
        let phi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 0);
        let extended = cons_f(&mut arena, phi, gamma);
        let psi = variable(&mut arena, 1);
        deriv_at(&mut arena, &family, extended, psi)
    };
    let minor_premise = {
        // Under [φ, ψ, Γ, dI]: Γ = 1, ψ = 2, φ = 3.
        let gamma = variable(&mut arena, 1);
        let phi = variable(&mut arena, 3);
        deriv_at(&mut arena, &family, gamma, phi)
    };
    // In the certificate context: dm = 0, dI = 1, Γ = 2, ψ = 3, φ = 4.
    let intro = {
        // Inside `g`'s `b` binder: dm = 1, dI = 2, Γ = 3, ψ = 4, φ = 5.
        let phi = variable(&mut arena, 5);
        let psi = variable(&mut arena, 4);
        let gamma = variable(&mut arena, 3);
        let label = impi_label(&mut arena, phi, psi, gamma);
        let children_fn = {
            // Its domain annotation sits where `intro` is used — under
            // `b`: dI = 2, Γ = 3, ψ = 4, φ = 5; under `children_fn`'s
            // own binder: dI = 3.
            let phi = variable(&mut arena, 5);
            let psi = variable(&mut arena, 4);
            let gamma = variable(&mut arena, 3);
            let label = impi_label(&mut arena, phi, psi, gamma);
            let domain = apply(&mut arena, family.children, label);
            let premise = variable(&mut arena, 3);
            lambda(&mut arena, domain, premise)
        };
        family.sup(&mut arena, label, children_fn)
    };
    let mp = {
        let phi = variable(&mut arena, 4);
        let psi = variable(&mut arena, 3);
        let gamma = variable(&mut arena, 2);
        let label = impe_label(&mut arena, phi, psi, gamma);
        let children_fn = {
            // Its domain annotation sits at ambient level; under the
            // `b` binder itself: dm = 1, dI = 2, Γ = 3, ψ = 4, φ = 5.
            let phi = variable(&mut arena, 4);
            let psi = variable(&mut arena, 3);
            let gamma = variable(&mut arena, 2);
            let label = impe_label(&mut arena, phi, psi, gamma);
            let domain = apply(&mut arena, family.children, label);
            let body = {
                let motive = {
                    // Under `w`: dm = 2, dI = 3, Γ = 4, ψ = 5, φ = 6.
                    let phi = variable(&mut arena, 6);
                    let psi = variable(&mut arena, 5);
                    let gamma = variable(&mut arena, 4);
                    let label = impe_label(&mut arena, phi, psi, gamma);
                    let next_a = apply(&mut arena, family.next, label);
                    let bound = variable(&mut arena, 0);
                    let required = apply(&mut arena, next_a, bound);
                    let codomain = family.indexed_w(&mut arena, required);
                    let domain = two(&mut arena);
                    lambda(&mut arena, domain, codomain)
                };
                let major = intro;
                let minor = variable(&mut arena, 1);
                let bound = variable(&mut arena, 0);
                case_two(&mut arena, motive, major, minor, bound)
            };
            lambda(&mut arena, domain, body)
        };
        family.sup(&mut arena, label, children_fn)
    };
    let expected = {
        let gamma = variable(&mut arena, 2);
        let psi = variable(&mut arena, 3);
        deriv_at(&mut arena, &family, gamma, psi)
    };
    let phi_type = frm(&mut arena);
    let psi_type = frm(&mut arena);
    let context_type = ctx(&mut arena);
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![
            phi_type,
            psi_type,
            context_type,
            intro_premise,
            minor_premise,
        ],
        term: mp,
        expected,
    };
    let before = budget.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    let spent = before - budget.remaining();
    assert!(spent > 0, "checking the certificate must do real work");

    // The judgment commits to exactly the four grammar assumptions —
    // `Ctx`, `Frm`, `consF`, `impF` — and to none of the scheme's
    // *definitions*, which carry no assumption force.
    let closure = certificate_assumption_closure(&arena, &certificate);
    assert_eq!(closure, [CTX, FRM, CONS, IMP].into_iter().collect());

    // The certificate claiming the minor premise's own judgment
    // `Deriv ⟨Γ, φ⟩` is a different, false judgment.
    let wrong = {
        let gamma = variable(&mut arena, 2);
        let phi = variable(&mut arena, 4);
        deriv_at(&mut arena, &family, gamma, phi)
    };
    let phi_type = frm(&mut arena);
    let psi_type = frm(&mut arena);
    let context_type = ctx(&mut arena);
    let intro_premise = {
        let phi = variable(&mut arena, 2);
        let gamma = variable(&mut arena, 0);
        let extended = cons_f(&mut arena, phi, gamma);
        let psi = variable(&mut arena, 1);
        deriv_at(&mut arena, &family, extended, psi)
    };
    let minor_premise = {
        let gamma = variable(&mut arena, 1);
        let phi = variable(&mut arena, 3);
        deriv_at(&mut arena, &family, gamma, phi)
    };
    let forged = MathematicalCertificate {
        signature: derivation_declarations(&mut arena),
        level_arity: 0,
        context: vec![
            phi_type,
            psi_type,
            context_type,
            intro_premise,
            minor_premise,
        ],
        term: mp,
        expected: wrong,
    };
    assert!(matches!(
        verify_mathematical_certificate(&mut arena, &forged, &mut budget),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Retained-storage and work receipts: checking the certificate —
    // the full nine-declaration signature plus the `isup` judgments —
    // re-materializes substituted instances of the encoding. Both
    // numbers are measured, not quotas.
    assert_eq!(arena.len(), 282_055);
    assert_eq!(spent, 4_342);
}

//! Families indexed by mathematical data, and theorems by induction over
//! them.
//!
//! `Lookup<C: Ctx, T: Ty>` is a derivation that `T` is in the context `C`:
//! its cases relate the indices to the payload by an Identity over data, not
//! by integers. `Even<N: Nat>` is evidence that a natural is even. Stage 05
//! declares each as the kernel's indexed family over its data, and a theorem
//! machine over erased evidence of it as a statement for every index and
//! every value. Stage 06 proves each case by rewriting with what the case
//! requires and the theorem at the children the body cites, and assembles
//! the cases with the family's induction.

use super::seq_laws::{judged, signature, unproved};
use super::*;

/// A context of types, and the derivation that a type is in one.
const LOOKUP: &str = "
data Ty {
    case Base;
    case Arrow(from: Ty, to: Ty);
}

data Ctx {
    case Empty;
    case Extend(ty: Ty, rest: Ctx);
}

machine extended(c: Ctx) -> bool
{
    transition c {
        Ctx::Empty -> false,
        Ctx::Extend { ty, rest } -> true
    }
}

data Lookup<C: Ctx, T: Ty>
    requires ((self in Lookup::Here) && |proof|::Identity::<Ctx>(C, Ctx::Extend { ty: T, rest: self.rest }))
          || ((self in Lookup::There) && |proof|::Identity::<Ctx>(C, Ctx::Extend { ty: self.other, rest: self.rest }))
{
    case Here(rest: Ctx);
    case There(other: Ty, rest: Ctx, inner [erased]: Lookup<rest, T>);
}

machine lookup_extended<C: Ctx, T: Ty>(evidence [erased]: Lookup<C, T>)
terminates by evidence
ensures |proof|::Identity::<bool>(extended(C), true)
{
    transition evidence {
        Lookup::Here { rest, .. } -> here(rest),
        Lookup::There { other, rest, inner, .. } -> there(other, rest, inner)
    }
    machine here(rest: Ctx) requires |proof|::Identity::<Ctx>(C, Ctx::Extend { ty: T, rest: rest }) {}
    machine there(other: Ty, rest: Ctx, inner [erased]: Lookup<rest, T>)
    requires |proof|::Identity::<Ctx>(C, Ctx::Extend { ty: other, rest: rest })
    {
        lookup_extended::<rest, T>(inner);
    }
}
";

/// Evenness of a natural, and that an even natural's parity is zero: the
/// step needs the theorem at the smaller natural.
const EVEN: &str = "
machine one_minus(n: Nat) -> Nat
{
    transition n {
        Nat::Zero -> Nat::Succ { prev: Nat::Zero },
        Nat::Succ { prev } -> Nat::Zero
    }
}

machine parity(n: Nat) -> Nat
terminates by n
{
    transition n {
        Nat::Zero -> Nat::Zero,
        Nat::Succ { prev } -> one_minus(parity(prev))
    }
}

data Even<N: Nat>
    requires ((self in Even::Zero) && |proof|::Identity::<Nat>(N, Nat::Zero))
          || ((self in Even::Two) && |proof|::Identity::<Nat>(N, Nat::Succ { prev: Nat::Succ { prev: self.prev } }))
{
    case Zero;
    case Two(prev: Nat, inner [erased]: Even<prev>);
}

machine even_parity<N: Nat>(evidence [erased]: Even<N>)
terminates by evidence
ensures |proof|::Identity::<Nat>(parity(N), Nat::Zero)
{
    transition evidence {
        Even::Zero { .. } -> zero(),
        Even::Two { prev, inner, .. } -> two(prev, inner)
    }
    machine zero() requires |proof|::Identity::<Nat>(N, Nat::Zero) {}
    machine two(prev: Nat, inner [erased]: Even<prev>)
    requires |proof|::Identity::<Nat>(N, Nat::Succ { prev: Nat::Succ { prev: prev } })
    {
        even_parity::<prev>(inner);
    }
}
";

fn lookup_program() -> String {
    format!("{}{LOOKUP}", super::nat_semiring::library_declarations())
}

fn even_program() -> String {
    format!("{}{EVEN}", super::nat_semiring::library_declarations())
}

/// The program's theorem `machine`, judged, with a certificate that checks
/// on its own.
fn judged_theorem(source: &str, machine: &str) {
    let verdicts = judged(source);
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == machine)
        .unwrap_or_else(|| panic!("`{machine}` is judged: {judgment:?}"));
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    // The theorem rests on nothing: the families, their constructors, the
    // machines and the induction are definitions.
    assert_eq!(receipt.assumption_closure, 0, "{receipt:?}");
    let mut checked = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
}

#[test]
fn a_theorem_over_a_derivation_indexed_by_a_context_and_a_type_is_judged() {
    judged_theorem(&lookup_program(), "lookup_extended");
}

#[test]
fn a_false_theorem_over_a_derivation_is_refused() {
    let source = lookup_program().replace(
        "ensures |proof|::Identity::<bool>(extended(C), true)",
        "ensures |proof|::Identity::<bool>(extended(C), false)",
    );
    assert_ne!(source, lookup_program(), "the theorem is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "lookup_extended", "{law:?}");
    assert_eq!(law.cause, UnprovedCause::NoDerivation, "{law:?}");
}

#[test]
fn a_theorem_by_induction_over_data_indexed_by_a_natural_is_judged() {
    judged_theorem(&even_program(), "even_parity");
}

#[test]
fn the_step_needs_the_theorem_at_the_child() {
    // Without the citation there is no induction hypothesis, and the
    // parity of a natural two larger does not follow.
    let source = even_program().replace("        even_parity::<prev>(inner);\n", "");
    assert_ne!(source, even_program(), "the citation is removed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "even_parity");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `even_parity` in the case `Even::Two`: \
         |proof|::Identity::<Nat>(parity(N), Nat::Zero)"
    );
}

#[test]
fn a_false_theorem_by_induction_is_refused() {
    let source = even_program().replace(
        "ensures |proof|::Identity::<Nat>(parity(N), Nat::Zero)",
        "ensures |proof|::Identity::<Nat>(parity(N), Nat::Succ { prev: Nat::Zero })",
    );
    assert_ne!(source, even_program(), "the theorem is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "even_parity");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    // The empty case already fails: the parity of zero is zero.
    assert!(law.statement.contains("`Even::Zero`"), "{law:?}");
}

#[test]
fn a_step_requiring_what_its_case_does_not_say_is_refused() {
    let source = even_program().replace(
        "machine zero() requires |proof|::Identity::<Nat>(N, Nat::Zero) {}",
        "machine zero() requires |proof|::Identity::<Nat>(N, Nat::Succ { prev: Nat::Zero }) {}",
    );
    assert_ne!(source, even_program(), "the step is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "even_parity");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.detail,
        "what the step of this case requires does not follow from what the data requires of \
         the case"
    );
}

#[test]
fn a_published_proof_over_a_derivation_with_a_forged_recursor_is_refused() {
    let verdicts = judged(&lookup_program());
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "lookup_extended")
        .expect("the theorem is judged");
    let mut decoded = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    let signature = &mut decoded.certificate.signature;
    // The family's recursor: the last recursor of the signature, which
    // declares the family after the data it is indexed by.
    let recursor = signature
        .iter()
        .rposition(|declaration| {
            matches!(
                declaration.kind,
                proof_admission::DeclarationKind::Recursor { .. }
            )
        })
        .expect("the family's recursor");
    let proof_admission::DeclarationKind::Recursor { inductive } = signature[recursor].kind else {
        unreachable!()
    };
    signature[recursor].ty = signature[inductive as usize].ty;
    let bytes =
        terminal_codec::encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("a forged certificate still encodes");
    let mut read = terminal_codec::decode_mathematical_certificate(&bytes).unwrap();
    assert_eq!(
        proof_admission::verify_mathematical_certificate(
            &mut read.arena,
            &read.certificate,
            &mut Default::default(),
        )
        .map(|_| ()),
        Err(proof_admission::CoreError::InvalidRecursorType)
    );
}

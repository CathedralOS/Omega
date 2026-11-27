//! `core/seq.omg`'s laws over `Seq<T>`, judged by the mathematical kernel.
//!
//! `Seq<T>` is generic recursive data with a payload: its `Cons` case holds a
//! value of the type parameter beside the tail. Stage 05 declares it once as
//! a kernel inductive with one uniform parameter and reads `Seq<u64>` as that
//! declaration applied to the carrier of `u64`.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};

const SEQ: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/seq.omg"
));
const ALGEBRA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/algebra.omg"
));

/// The file's declarations from `Seq<T>` on, compiled as one program with
/// `core/nat.omg`'s, as the library compiles them in one package: `seq.omg`
/// cites laws `nat.omg` keeps private. The proof views (`BagView`, `Bag`) and
/// the consuming transforms (`map`, `filter`), which take machine parameters
/// and are not mathematical definitions, are left out.
pub(super) fn library_declarations() -> String {
    let mut program = super::nat_semiring::library_declarations();
    let start = SEQ
        .find("public data Seq<T> {")
        .expect("the data declaration");
    let mut skipping = false;
    for line in SEQ[start..].lines() {
        skipping |= [
            "public data BagView",
            "public machine Bag(",
            "machine map<",
            "machine filter<",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix));
        if !skipping {
            program.push_str(&line.replace("|nat|::", ""));
            program.push('\n');
        } else if line == "}" {
            skipping = false;
        }
    }
    program
}

pub(super) fn judged(source: &str) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[("algebra.omg", "CommutativeSemiring", ALGEBRA)],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    crate::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect()
}

pub(super) fn signature(verdicts: &[RouteVerdict]) -> &SignatureJudgment {
    let mut laws = None;
    for verdict in verdicts {
        match verdict {
            RouteVerdict::Accepted(_) => {}
            RouteVerdict::Laws(judgment) => {
                assert!(laws.replace(&**judgment).is_none(), "one signature");
            }
            other => panic!("every part must be accepted: {other:?}"),
        }
    }
    laws.expect("the signature's judgment")
}

pub(super) fn unproved(verdicts: &[RouteVerdict]) -> &UnprovedLaw {
    let mut laws = verdicts.iter().filter_map(|verdict| match verdict {
        RouteVerdict::UnprovedLaws(laws) => Some(laws),
        RouteVerdict::Laws(judgment) => panic!("the law must not be accepted: {judgment:?}"),
        _ => None,
    });
    let (Some([law]), None) = (laws.next().map(Vec::as_slice), laws.next()) else {
        panic!("expected exactly one unproved law: {verdicts:?}")
    };
    law
}

/// The refusal of a program whose data declaration is refused: the
/// declaration's own, which each machine over the data repeats.
fn refusal(verdicts: &[RouteVerdict]) -> String {
    let refused: Vec<_> = verdicts
        .iter()
        .filter(|verdict| !matches!(verdict, RouteVerdict::Accepted(_)))
        .map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => refusal.detail.clone(),
            other => panic!("the data must be refused before any law is judged: {other:?}"),
        })
        .collect();
    let declaration = refused
        .iter()
        .find(|detail| detail.ends_with("the mathematical declaration of `Tagged`"))
        .unwrap_or_else(|| panic!("the declaration is refused: {refused:#?}"))
        .clone();
    let reason = declaration
        .strip_suffix(" while reading the mathematical declaration of `Tagged`")
        .expect("found above");
    assert!(
        refused.iter().all(|detail| detail.contains(reason)),
        "{refused:#?}"
    );
    declaration
}

/// The laws of `seq.omg`, each proved by the kernel over the native
/// inductive `Seq`.
const SEQ_LAWS: [&str; 12] = [
    "length_empty",
    "length_cons",
    "append_empty_right",
    "append_assoc",
    "length_append",
    "length_reverse",
    "reverse_append",
    "reverse_reverse",
    "reverse_snoc",
    "length_snoc",
    "snoc_append",
    "snoc_reverse",
];

#[test]
fn every_law_of_seq_is_judged() {
    let verdicts = judged(&library_declarations());
    let judgment = signature(&verdicts);
    for law in &judgment.laws {
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
    for machine in SEQ_LAWS {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("`{machine}` is judged"));
        eprintln!("{machine}: {} conversion steps", law.conversion_steps);
    }
    // `Seq<u64>` is the generic block applied to the carrier of `u64`.
    assert!(
        judgment
            .carriers
            .iter()
            .any(|carrier| carrier.type_identity.contains("Seq")),
        "{:?}",
        judgment
            .carriers
            .iter()
            .map(|carrier| &carrier.type_identity)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_false_law_over_seq_is_refused() {
    let source = library_declarations().replace(
        "|proof|::Identity::<Seq<u64>>(reverse(reverse(s)), s)",
        "|proof|::Identity::<Seq<u64>>(reverse(s), s)",
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "reverse_reverse", "{law:?}");
}

#[test]
fn a_seq_law_without_its_induction_hypothesis_is_refused() {
    // `length_reverse`'s step cites itself at the tail; citing nothing there
    // leaves the step's goal open.
    let source = library_declarations().replace(
        "transition { _ -> Nat::Succ { prev: length_reverse(tail) } }",
        "transition { _ -> Nat::Succ { prev: length(tail) } }",
    );
    assert_ne!(source, library_declarations(), "the citation is removed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "length_reverse", "{law:?}");
}

/// Generic data over `Seq<T>` whose recursion `rest` is at `rest_type`,
/// with one field `item: item_type`, and one law about it.
fn generic_program(item_type: &str, rest_type: &str) -> String {
    format!(
        "{}
data Tagged<T> {{
    case Nil;
    case More(item: {item_type}, rest: {rest_type});
}}

machine count(t: Tagged<u64>) -> Nat
terminates by t
{{
    transition t {{
        Tagged::Nil -> Nat::Zero,
        Tagged::More {{ item, rest }} -> Nat::Succ {{ prev: count(rest) }}
    }}
}}

machine count_nil()
ensures
    |proof|::Identity::<Nat>(count(Tagged::Nil), Nat::Zero)
{{
}}
",
        library_declarations()
    )
}

#[test]
fn generic_data_with_a_field_of_its_parameter_is_judged() {
    let verdicts = judged(&generic_program("T", "Tagged<T>"));
    let judgment = signature(&verdicts);
    assert!(
        judgment.laws.iter().any(|law| law.machine == "count_nil"
            && matches!(law.decision, MathematicalCoreDecision::Judged(_))),
        "{judgment:?}"
    );
}

#[test]
fn generic_data_with_a_field_applying_its_parameter_is_refused() {
    // `Seq<T>` names the parameter inside another application: a nested
    // inductive, which the profile does not declare.
    let refused = refusal(&judged(&generic_program("Seq<T>", "Tagged<T>")));
    assert!(
        refused.contains("generic data with a field whose type applies a parameter"),
        "{refused}"
    );
}

#[test]
fn generic_data_recursive_at_other_arguments_is_refused() {
    // A recursion at `Tagged<u64>` inside `Tagged<T>` is not uniform in the
    // parameter, so it is no inductive with that parameter.
    let refused = refusal(&judged(&generic_program("T", "Tagged<u64>")));
    assert!(
        refused.contains("generic data that contains itself at other arguments"),
        "{refused}"
    );
}

/// `reverse_reverse`'s published certificate, with `forge` applied to the
/// recursor of the generic `Seq` block in its signature, through the wire
/// format a Psi consumer reads, and the kernel's verdict on it.
fn verify_forged(
    forge: impl FnOnce(&mut proof_admission::TermArena, &mut Vec<proof_admission::Declaration>, usize),
) -> Result<(), proof_admission::CoreError> {
    let verdicts = judged(&library_declarations());
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "reverse_reverse")
        .expect("the law is judged");
    let mut decoded = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    // The block's recursor: the one whose family has a uniform parameter.
    let signature = &mut decoded.certificate.signature;
    let recursor = signature
        .iter()
        .position(|declaration| match declaration.kind {
            proof_admission::DeclarationKind::Recursor { inductive } => matches!(
                signature[inductive as usize].kind,
                proof_admission::DeclarationKind::Inductive { num_params: 1, .. }
            ),
            _ => false,
        })
        .expect("the generic block's recursor");
    forge(&mut decoded.arena, signature, recursor);
    let bytes =
        terminal_codec::encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .expect("a forged certificate still encodes");
    let mut read = terminal_codec::decode_mathematical_certificate(&bytes).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut read.arena,
        &read.certificate,
        &mut Default::default(),
    )
    .map(|_| ())
}

#[test]
fn a_published_seq_proof_checks_on_its_own() {
    verify_forged(|_, _, _| {}).expect("the certificate as published verifies");
}

#[test]
fn a_published_seq_proof_with_a_forged_recursor_type_is_refused() {
    // The recursor stated at the family's own type: not the type the kernel
    // derives from the block.
    let refused = verify_forged(|_, signature, recursor| {
        let proof_admission::DeclarationKind::Recursor { inductive } = signature[recursor].kind
        else {
            unreachable!()
        };
        signature[recursor].ty = signature[inductive as usize].ty;
    });
    assert_eq!(
        refused,
        Err(proof_admission::CoreError::InvalidRecursorType)
    );
}

#[test]
fn a_published_seq_proof_whose_recursor_is_an_assumption_is_refused() {
    // The same type as an ordinary assumption: it no longer computes on the
    // block's constructors, and the block lacks its recursor.
    let refused = verify_forged(|_, signature, recursor| {
        let ty = signature[recursor].ty;
        let level_arity = signature[recursor].level_arity;
        signature[recursor] = proof_admission::Declaration::assumption(level_arity, ty);
    });
    assert!(refused.is_err(), "{refused:?}");
}

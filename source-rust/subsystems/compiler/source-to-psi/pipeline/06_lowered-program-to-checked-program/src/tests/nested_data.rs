//! Mutual blocks read from closed applications of generic data, judged by
//! the mathematical kernel.
//!
//! Stage 05 finds a block among the records a program uses, not among its
//! declarations. `Tree<u64>` and `Forest<u64>`, applications of the generic
//! `Tree<T>` and `Forest<T>` that hold each other, are one block. A `Rose`
//! that holds a `Seq<Rose>` is a nested inductive: `Rose` and `Seq<Rose>`
//! reach each other, so they are one block too, and machines and laws over
//! them are read as over any mutual block.

use super::seq_laws::{judged, signature, unproved};
use super::*;

/// Every named law of the program's one signature is judged.
fn all_judged(verdicts: &[RouteVerdict], machines: &[&str]) {
    let judgment = signature(verdicts);
    for machine in machines {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == *machine)
            .unwrap_or_else(|| panic!("`{machine}` is judged: {judgment:?}"));
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
}

/// The machines of the unproved laws, sorted; none may be accepted.
fn unproved_machines(verdicts: &[RouteVerdict]) -> Vec<&str> {
    let mut refused: Vec<_> = verdicts
        .iter()
        .flat_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => {
                laws.iter().map(|law| law.machine.as_str()).collect()
            }
            RouteVerdict::Laws(judgment) => panic!("the laws must not be accepted: {judgment:?}"),
            _ => Vec::new(),
        })
        .collect();
    refused.sort_unstable();
    refused
}

const GENERIC_TREE: &str = "
data Tree<T> {
    case Node(value: T, children: Forest<T>);
}

data Forest<T> {
    case Nil;
    case Cons(head: Tree<T>, tail: Forest<T>);
}

machine size(t: Tree<u64>) -> Nat
terminates by t
{
    transition t {
        Tree::Node { value, children } -> Nat::Succ { prev: size_forest(children) }
    }
}

machine size_forest(f: Forest<u64>) -> Nat
terminates by f
{
    transition f {
        Forest::Nil -> Nat::Zero,
        Forest::Cons { head, tail } -> add(size(head), size_forest(tail))
    }
}

machine size_pair()
ensures
    |proof|::Identity::<Nat>(
        size(Tree::Node {
            value: 1u64,
            children: Forest::Cons {
                head: Tree::Node { value: 2u64, children: Forest::Nil },
                tail: Forest::Nil
            }
        }),
        Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }
    )
{
}
";

fn tree_program() -> String {
    format!(
        "{}{GENERIC_TREE}",
        super::nat_semiring::library_declarations()
    )
}

#[test]
fn a_law_over_a_mutual_block_of_generic_data_is_judged() {
    let verdicts = judged(&tree_program());
    all_judged(&verdicts, &["size_pair"]);
    let judgment = signature(&verdicts);
    // Each member is declared at its application, beside the other.
    for member in ["Tree", "Forest"] {
        assert!(
            judgment
                .carriers
                .iter()
                .any(|carrier| carrier.type_identity.contains(member)
                    && carrier.type_identity.contains("u64")),
            "{:?}",
            judgment.carriers
        );
    }
}

#[test]
fn a_false_law_over_a_mutual_block_of_generic_data_is_refused() {
    let source = tree_program().replace(
        "Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }\n    )",
        "Nat::Succ { prev: Nat::Zero }\n    )",
    );
    assert_ne!(source, tree_program(), "the law is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "size_pair", "{law:?}");
}

/// A rose tree whose children are a `Seq<Rose>`, and laws by induction over
/// it: rebuilding a tree or a sequence of trees gives it back, each law
/// citing the other at its field. `size_leaf` computes through both.
const ROSE: &str = "
data Rose {
    case Node(label: u64, children: Seq<Rose>);
}

machine size(r: Rose) -> Nat
terminates by r
{
    transition r {
        Rose::Node { label, children } -> Nat::Succ { prev: size_all(children) }
    }
}

machine size_all(s: Seq<Rose>) -> Nat
terminates by s
{
    transition s {
        Seq::Empty -> Nat::Zero,
        Seq::Cons { head, tail } -> add(size(head), size_all(tail))
    }
}

machine size_leaf()
ensures
    |proof|::Identity::<Nat>(
        size(Rose::Node { label: 1u64, children: Seq::Empty }),
        Nat::Succ { prev: Nat::Zero }
    )
{
}

machine rebuild(r: Rose) -> Rose
terminates by r
{
    transition r {
        Rose::Node { label, children } -> Rose::Node { label: label, children: rebuild_all(children) }
    }
}

machine rebuild_all(s: Seq<Rose>) -> Seq<Rose>
terminates by s
{
    transition s {
        Seq::Empty -> Seq::Empty,
        Seq::Cons { head, tail } -> Seq::Cons { head: rebuild(head), tail: rebuild_all(tail) }
    }
}

machine rebuild_id(r: Rose) -> Nat
terminates by r
ensures
    |proof|::Identity::<Rose>(rebuild(r), r)
{
    transition r {
        Rose::Node { label, children } -> rebuild_all_id(children)
    }
}

machine rebuild_all_id(s: Seq<Rose>) -> Nat
terminates by s
ensures
    |proof|::Identity::<Seq<Rose>>(rebuild_all(s), s)
{
    transition s {
        Seq::Empty -> Nat::Zero,
        Seq::Cons { head, tail } -> add(rebuild_id(head), rebuild_all_id(tail))
    }
}
";

/// `core/seq.omg` with `core/nat.omg`, and the rose tree over them: the
/// library's laws over `Seq<u64>` are judged beside the block that holds
/// `Seq<Rose>`.
fn rose_program() -> String {
    format!("{}{ROSE}", super::seq_laws::library_declarations())
}

#[test]
fn laws_over_a_nested_inductive_are_judged() {
    let verdicts = judged(&rose_program());
    all_judged(
        &verdicts,
        &[
            "size_leaf",
            "rebuild_id",
            "rebuild_all_id",
            // A case written in a law is read at the application it
            // constructs, not at the first application of its declaration.
            "length_empty",
        ],
    );
}

#[test]
fn a_false_law_over_a_nested_inductive_is_refused() {
    let source = rose_program().replace(
        "        Nat::Succ { prev: Nat::Zero }\n    )",
        "        Nat::Zero\n    )",
    );
    assert_ne!(source, rose_program(), "the law is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "size_leaf", "{law:?}");
}

#[test]
fn a_false_law_by_induction_over_a_nested_inductive_is_refused() {
    // The two laws are proved by one induction, so neither stands.
    let source = rose_program().replace(
        "Seq::Cons { head, tail } -> Seq::Cons { head: rebuild(head), tail: rebuild_all(tail) }",
        "Seq::Cons { head, tail } -> Seq::Cons { head: rebuild(head), tail: Seq::Empty }",
    );
    assert_ne!(source, rose_program(), "the definition is changed");
    let verdicts = judged(&source);
    assert_eq!(
        unproved_machines(&verdicts),
        ["rebuild_all_id", "rebuild_id"],
        "{verdicts:?}"
    );
}

/// `rebuild_id`'s published certificate, with `forge` applied to the
/// recursor of the `Rose`/`Seq<Rose>` block, through the wire format a Psi
/// consumer reads, and the kernel's verdict on it.
fn verify_forged(
    forge: impl FnOnce(&mut proof_admission::TermArena, &mut Vec<proof_admission::Declaration>, usize),
) -> Result<(), proof_admission::CoreError> {
    let verdicts = judged(&rose_program());
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "rebuild_id")
        .expect("the law is judged");
    let mut decoded = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    // A recursor of the two-member block.
    let signature = &mut decoded.certificate.signature;
    let recursor = signature
        .iter()
        .position(|declaration| match declaration.kind {
            proof_admission::DeclarationKind::Recursor { inductive } => matches!(
                signature[inductive as usize].kind,
                proof_admission::DeclarationKind::Inductive { num_mutual: 2, .. }
            ),
            _ => false,
        })
        .expect("the nested block's recursor");
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
fn a_published_proof_over_a_nested_inductive_checks_on_its_own() {
    verify_forged(|_, _, _| {}).expect("the certificate as published verifies");
}

#[test]
fn a_published_proof_over_a_nested_inductive_with_a_forged_recursor_is_refused() {
    // The recursor stated at its family's own type: not the type the kernel
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

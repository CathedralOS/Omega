//! Subject attribution for abstract requirement calls.
//!
//! A proof term may cite a trait's requirement signature directly
//! (`Trait::requirement(args)`). The callee it names is the signature, not a
//! realization, so the requires gate must read the signature's own contracts
//! and bind the call's arguments to the signature parameters. These tests pin
//! that owner: the same-subject citation discharges, and citing the
//! requirement at an operand the caller never established rejects.

use typed_trees::domain::ProofFact;
use typed_trees::signature::SignatureContractKind;

const WRONG_SUBJECT_REJECTION: &str =
    "cannot prove requires contract for specification call `take_empty`";

/// The driver's contract walk in miniature: requires facts accumulate into
/// `prior_facts`, then every ensures fact is checked against them.
fn machine_contract_diagnostics(source: &str, machine_name: &str) -> Vec<String> {
    let program = crate::front_end::typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("fixture declares the named machine");
    let entry_state = program.machine_states(machine).first();
    let mut prior_facts = Vec::new();
    let mut diagnostics = Vec::new();
    for contract in program.machine_contracts(machine) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            super::validate_specification_call_requirements(
                &program,
                machine,
                entry_state,
                fact,
                &prior_facts,
                "test",
                &mut diagnostics,
            );
            if contract.kind == SignatureContractKind::Requires
                && let ProofFact::Expression(expression) = fact
            {
                prior_facts.push(*expression);
            }
        }
    }
    diagnostics.into_iter().map(|entry| entry.message).collect()
}

const ABSTRACT_FIXTURE: &str = "\
data Tree {
    marked: bool;
    case Empty;
    case Node(child: Tree);
}

trait Emptiable {
    machine take_empty(value: Tree) -> Tree
    requires value in Tree::Empty;
}

machine take_empty_peano(value: Tree) -> Tree
satisfies Emptiable::take_empty
requires value in Tree::Empty;
terminates;
{ value }
";

#[test]
fn abstract_requirement_call_at_unestablished_subject_rejects() {
    let source = format!(
        "{ABSTRACT_FIXTURE}
machine wrong_subject(known: Tree, other: Tree)
requires known in Tree::Empty;
ensures Emptiable::take_empty(other) == Emptiable::take_empty(other);
{{}}
"
    );
    let messages = machine_contract_diagnostics(&source, "wrong_subject");
    assert!(
        messages
            .iter()
            .any(|message| message.contains(WRONG_SUBJECT_REJECTION)),
        "citing the requirement at an unestablished subject must reject: {messages:?}"
    );
}

#[test]
fn abstract_requirement_call_at_established_subject_compiles() {
    let source = format!(
        "{ABSTRACT_FIXTURE}
machine right_subject(known: Tree)
requires known in Tree::Empty;
ensures Emptiable::take_empty(known) == Emptiable::take_empty(known);
{{}}
"
    );
    let messages = machine_contract_diagnostics(&source, "right_subject");
    assert!(
        messages.is_empty(),
        "citing the requirement at an established subject must prove: {messages:?}"
    );
}

#[test]
fn requirement_call_at_the_matching_constructor_leaf_proves() {
    // The required membership is `value in Tree::Empty`: citing the
    // requirement at the `Tree::Empty` constructor leaf itself is the exact
    // established subject, so the structural judgment must prove it.
    let source = format!(
        "{ABSTRACT_FIXTURE}
machine empty_leaf()
ensures Emptiable::take_empty(Tree::Empty) == Emptiable::take_empty(Tree::Empty);
{{}}
"
    );
    let messages = machine_contract_diagnostics(&source, "empty_leaf");
    assert!(
        messages.is_empty(),
        "citing the requirement at the matching constructor leaf must prove: {messages:?}"
    );
}

#[test]
fn requirement_call_at_a_wrong_constructor_leaf_rejects() {
    // `Tree::Node { child: known }` is a constructor leaf whose case is not
    // `Empty`; an established `known` does not change the constructed value's
    // case, so the requirement citation must reject — and reject at each of
    // the two occurrences.
    let source = format!(
        "{ABSTRACT_FIXTURE}
machine node_leaf(known: Tree)
requires known in Tree::Empty;
ensures Emptiable::take_empty(Tree::Node {{ child: known }}) == Emptiable::take_empty(Tree::Node {{ child: known }});
{{}}
"
    );
    let messages = machine_contract_diagnostics(&source, "node_leaf");
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.contains(WRONG_SUBJECT_REJECTION))
            .count(),
        2,
        "each requirement citation at the wrong constructor leaf must reject: {messages:?}"
    );
}

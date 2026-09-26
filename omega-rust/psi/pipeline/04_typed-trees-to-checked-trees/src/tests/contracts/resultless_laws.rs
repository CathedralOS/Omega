use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn result_bearing_machine_cannot_satisfy_a_resultless_law() {
    let source = r#"
        trait ReflexiveLaw {
            machine reflexive(value: u64)
            ensures value == value;
        }

        machine reflexive(value: u64) -> u64
        satisfies ReflexiveLaw::reflexive
        ensures value == value
        {
            transition { _ -> value }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("a runtime result must not satisfy a theorem-only slot");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        messages.contains("expected return `()`, got `u64`"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn unchanged_resultless_self_citation_cannot_prove_itself() {
    let source = r#"
        data Nat {
            case Zero;
            case Succ(prev: Nat);
        }

        machine bogus(n: Nat)
        terminates by n;
        ensures n == n
        {
            bogus(n);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("a theorem must not obtain its own ensures from an unchanged citation");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        messages.contains("cannot prove the measure `n` structurally decreases"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn explicitly_discarded_recursive_call_still_requires_exact_descent() {
    let source = r#"
        data Nat {
            case Zero;
            case Succ(prev: Nat);
        }

        machine bogus(n: Nat) -> Nat
        terminates by n;
        ensures result == n
        {
            _ = bogus(n);
            transition { _ -> n }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("discarding a recursive result must not erase its induction edge");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        messages.contains("cannot prove the measure `n` structurally decreases"),
        "unexpected diagnostics: {messages}"
    );
}

#[test]
fn unchanged_resultless_mutual_citations_cannot_certify_each_other() {
    let source = r#"
        data Nat {
            case Zero;
            case Succ(prev: Nat);
        }

        machine left(n: Nat)
        terminates by n;
        ensures n == n
        {
            right(n);
        }

        machine right(n: Nat)
        terminates by n;
        ensures n == n
        {
            left(n);
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("mutual theorem citations must prove descent on every exact edge");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        messages.contains("proof-only machine call cycle")
            && messages.contains("ranking subject does not structurally decrease"),
        "unexpected diagnostics: {messages}"
    );
}

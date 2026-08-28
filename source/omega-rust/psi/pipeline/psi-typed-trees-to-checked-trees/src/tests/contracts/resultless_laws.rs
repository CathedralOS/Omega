use super::*;

#[test]
fn resultless_law_accepts_an_exact_resultless_satisfier() {
    let source = r#"
        trait ReflexiveLaw {
            machine reflexive(value: u64)
            ensures value == value;
        }

        machine reflexive(value: u64)
        satisfies ReflexiveLaw::reflexive
        ensures value == value
        {
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("an exact resultless theorem satisfier should check");
}

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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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
fn descending_resultless_self_citation_is_a_checked_induction_edge() {
    let source = r#"
        data Nat {
            case Zero;
            case Succ(prev: Nat);
        }

        machine copy(n: Nat) -> Nat
        terminates by n;
        {
            transition n {
                Nat::Zero -> Nat::Zero
                Nat::Succ { prev } -> Nat::Succ { prev: copy(prev) }
            }
        }

        machine copy_identity(n: Nat)
        terminates by n;
        ensures copy(n) == n
        {
            transition n {
                Nat::Zero -> base()
                Nat::Succ { prev } -> step(prev)
            }

            state base() {
            }

            state step(prev: Nat) {
                copy_identity(prev);
            }
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a resultless citation should prove both exact descent and its induction step");
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
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

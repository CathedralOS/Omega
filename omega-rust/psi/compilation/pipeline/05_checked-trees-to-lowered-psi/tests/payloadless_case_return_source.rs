//! Fixtures shared by the payloadless case return source tests.

#[path = "payloadless_case_return_source/guarded_payloadless_calls.rs"]
mod guarded_payloadless_calls;
#[path = "payloadless_case_return_source/ordered_case_returns.rs"]
mod ordered_case_returns;
#[path = "payloadless_case_return_source/selected_witness_tail_uses.rs"]
mod selected_witness_tail_uses;

const SOURCE: &str = r#"
    data Outcome [copy] {
        case Success;
        case Failure;
    }
    data Root {}

    machine Root::choose() -> Outcome {
        Outcome::Success
    }
"#;

fn checked_source() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(SOURCE)
}

const GUARDED_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Outcome [copy] {
        case Success;
        case Failure;
    }
    data Root {}

    machine Root::choose() -> Outcome
    ensures Outcome::Success -> { selected: ready(); true; }
    ensures Outcome::Failure -> { skipped: ready(); true; }
    {
        selected = ConcreteEvidence;
        Outcome::Success
    }
"#;

const GUARDED_CALL_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { selected: ready(); true; }
    ensures Outcome::Failure -> { sibling: ready(); }
    { selected = ConcreteEvidence; Outcome::Success }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; selected: local } -> saved
            Outcome::Failure { } -> saved
        }
    }
"#;

const OMITTED_GUARDED_CALL_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { selected: ready(); true; }
    ensures Outcome::Failure -> { sibling: ready(); }
    { selected = ConcreteEvidence; Outcome::Success }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { } -> saved
            Outcome::Failure { } -> saved
        }
    }
"#;

const MULTI_SELECTED_GUARDED_CALL_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { first: ready(); second: ready(); true; }
    ensures Outcome::Failure -> { sibling: ready(); }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; second: local_second, first: local_first } -> saved
            Outcome::Failure { } -> saved
        }
    }
"#;

const SELECTED_WITNESS_TAIL_USE_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> { selected: accepted(result); }
    { selected = ConcreteEvidence; Outcome::Success }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success { ; selected: local } -> finish(saved; local)
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed: accepted(value)
        { value }
    }
"#;

const THREE_SELECTED_WITNESS_TAIL_USES_SOURCE: &str = r#"
    trait Evidence {}
    data Outcome [copy] { case Success; case Failure; }
    proposition accepted(value: Outcome) evidence Evidence;
    proposition trusted(value: Outcome) evidence Evidence;
    proposition certified(value: Outcome) evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}
    data Root {}

    machine Root::produce() -> Outcome
    ensures Outcome::Success -> {
        first: accepted(result);
        second: trusted(result);
        third: certified(result);
    }
    ensures Outcome::Failure -> { sibling: accepted(result); }
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        third = ConcreteEvidence;
        Outcome::Success
    }

    machine Root::caller() -> Outcome {
        let saved: Outcome = Root::produce();
        transition saved {
            Outcome::Success {
                ; first: local_first, second: local_second, third: local_third
            } -> finish(saved; local_first, local_second, local_third)
            Outcome::Failure { } -> saved
        }
        state finish(value: Outcome) -> Outcome
        requires needed_first: accepted(value)
        requires needed_second: trusted(value)
        requires needed_third: certified(value)
        { value }
    }
"#;

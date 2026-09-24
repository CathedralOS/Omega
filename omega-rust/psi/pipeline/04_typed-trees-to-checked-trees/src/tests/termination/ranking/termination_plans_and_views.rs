use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::{checked_program_result, typed_program};

/// TPR2 (decision 23): the normalized `MachineTerminationPlan` populates at
/// the syntax->resolved lowering and copies -- never re-derives -- through
/// resolved->typed. Bare `terminates;` authors the PUBLIC guarantee and no
/// witness; `terminates by ...` supplies the PRIVATE witness and publishes
/// NOTHING; canonical defaults elaborate immediately to the explicit view
/// (single unsigned subject -> Nat::Descending, two subjects ->
/// Nat::BoundedDistance); `checked_summary` stays NoGuarantee at this stage
/// (the checker's to establish, TPR3).
#[test]
fn termination_plan_splits_guarantee_from_witness_with_elaborated_defaults() {
    use language_semantics::{RankingViewId, TerminationGuarantee};

    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let a: u64 = self.promise();
        let b: u64 = self.countdown(2);
        let c: u64 = self.walk(4, 0);
    }

    machine Main::promise(&mut self) -> u64 terminates; { 7 }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by (index, limit);
    -> u64
    {
        transition index < limit {
            true -> self.walk(limit, index + 1)
            false -> index
        }
    }
    "#;

    let typed = typed_program(source);

    let plan_of = |name: &str| {
        &typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .termination_plan
    };

    // Bare `terminates;`: the authored public promise, no witness.
    let promise = plan_of("Main::promise");
    assert_eq!(
        &promise.interface,
        &language_semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new()
        })
    );
    assert!(promise.implementation_witness.is_none());

    // `terminates by remaining;`: witness only (publishes nothing); the
    // single u64 subject's canonical default elaborates immediately.
    let countdown = plan_of("Main::countdown");
    assert_eq!(
        &countdown.interface,
        &language_semantics::TerminationInterface::InternalDerived
    );
    let witness = countdown
        .implementation_witness
        .as_ref()
        .expect("countdown witness");
    assert_eq!(witness.subjects, vec!["remaining".to_string()]);
    assert_eq!(witness.ranking_view, RankingViewId::NAT_DESCENDING);
    assert_eq!(witness.view_path, "Nat::Descending");

    // Two-subject short form: the only builtin two-subject view.
    let walk = plan_of("Main::walk");
    assert_eq!(
        &walk.interface,
        &language_semantics::TerminationInterface::InternalDerived
    );
    let witness = walk.implementation_witness.as_ref().expect("walk witness");
    assert_eq!(
        witness.subjects,
        vec!["index".to_string(), "limit".to_string()]
    );
    assert_eq!(witness.ranking_view, RankingViewId::NAT_BOUNDED_DISTANCE);
    assert_eq!(witness.view_path, "Nat::BoundedDistance");

    // Nothing claims a checked summary before the checker runs (TPR3).
    for name in ["Main::promise", "Main::countdown", "Main::walk"] {
        assert_eq!(
            plan_of(name).checked_summary,
            TerminationGuarantee::NoGuarantee
        );
    }
}

/// TPR2: an authored `-> View` records verbatim -- canonical builtins carry
/// their fixed ids; a declared measure keeps the spelled path with a NULL id
/// until TPR3 assigns normalized measure identity.
#[test]
fn termination_plan_records_authored_views_verbatim() {
    use language_semantics::RankingViewId;

    let source = r#"
    data Card { power: u64 }
    data Main {}

    measure Card::PowerOrder(card: Card) -> u64 { card.power }

    machine Main::main(&mut self) {
        let a: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }

    machine Main::weaken(&mut self, card: Card)
    terminates by card -> Card::PowerOrder;
    -> u64
    {
        transition card.power > 0 {
            true -> self.weaken(Card { power: card.power - 1 })
            false -> card.power
        }
    }
    "#;

    let typed = typed_program(source);

    let witness_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .termination_plan
            .implementation_witness
            .as_ref()
            .unwrap_or_else(|| panic!("{name} witness"))
    };

    let countdown = witness_of("Main::countdown");
    assert_eq!(countdown.ranking_view, RankingViewId::NAT_DESCENDING);
    assert_eq!(countdown.view_path, "Nat::Descending");

    let weaken = witness_of("Main::weaken");
    assert_eq!(weaken.ranking_view, RankingViewId::NULL);
    assert_eq!(weaken.view_path, "Card::PowerOrder");
}

/// TPR3/TPR6 firewall: the witness's normalized builtin identity and its
/// explicit path must agree. Constructed by mutating the path post-typing;
/// real lowering stamps both together.
#[test]
fn recorded_view_divergence_is_loud() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let a: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }
    "#;

    let mut typed = typed_program(source);

    let machine = typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "Main::countdown")
        .expect("countdown machine");
    let witness = machine
        .termination_plan
        .implementation_witness
        .as_mut()
        .expect("countdown witness");
    assert_eq!(witness.view_path, "Nat::Descending");
    witness.view_path = "Slice::Length".to_string();

    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("a diverging recorded view must be loud");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("recorded ranking view `Nat::Descending`")
                && diagnostic.message.contains("resolved view `Slice::Length`")
        }),
        "expected the divergence invariant diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
}

/// TPR3 (decision 23's acceptance test 5): an INCREASING cursor is accepted
/// through the bounded argumented view `Nat::IncreasingTo(limit)` without an
/// authored subtraction -- the bound is part of the view, the measure is the
/// distance up to it, and the plan witness records the base path plus the
/// argument.
#[test]
fn accepts_increasing_cursor_via_bounded_argumented_view() {
    use language_semantics::RankingViewId;

    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.climb(4, 0);
    }

    machine Main::climb(&mut self, limit: u64, index: u64)
    terminates by index -> Nat::IncreasingTo(limit);
    -> u64
    {
        transition index < limit {
            true -> self.climb(limit, index + 1)
            false -> index
        }
    }
    "#;

    let typed = typed_program(source);

    let witness = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::climb")
        .expect("climb machine")
        .termination_plan
        .implementation_witness
        .as_ref()
        .expect("climb witness");
    assert_eq!(witness.subjects, vec!["index".to_string()]);
    assert_eq!(witness.ranking_view, RankingViewId::NAT_INCREASING_TO);
    assert_eq!(witness.view_path, "Nat::IncreasingTo");
    assert_eq!(witness.view_arguments, vec!["limit".to_string()]);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("the bounded increasing cursor should prove");
}

/// TPR3: the unbounded `Nat::Increasing` is NOT a valid ranking -- the
/// rejection is directed at the bounded spelling instead of a bare
/// "cannot prove".
#[test]
fn rejects_unbounded_increasing_view_with_directed_message() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.climb(4, 0);
    }

    machine Main::climb(&mut self, limit: u64, index: u64)
    terminates by index -> Nat::Increasing;
    -> u64
    {
        transition index < limit {
            true -> self.climb(limit, index + 1)
            false -> index
        }
    }
    "#;

    let diagnostics =
        checked_program_result(source).expect_err("the unbounded increasing view must be rejected");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("unbounded `Nat::Increasing` is not a well-founded ranking")
                && diagnostic.message.contains("`-> Nat::IncreasingTo(limit)`")
        }),
        "expected the directed unbounded-increasing rejection, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
}

/// TPR3: argument-arity misuse gets directed rejections too -- a missing
/// bound on `IncreasingTo`, and arguments on a plain view.
#[test]
fn rejects_view_argument_arity_misuse_with_directed_messages() {
    let cases = [
        (
            "terminates by index -> Nat::IncreasingTo;",
            "`Nat::IncreasingTo` names exactly one bound argument",
        ),
        (
            "terminates by index -> Nat::Descending(limit);",
            "view arguments are only meaningful on an argumented view",
        ),
    ];
    for (clause, expected) in cases {
        let source = format!(
            r#"
    data Main {{}}

    machine Main::main(&mut self) {{
        let value: u64 = self.climb(4, 0);
    }}

    machine Main::climb(&mut self, limit: u64, index: u64)
    {clause}
    -> u64
    {{
        transition index < limit {{
            true -> self.climb(limit, index + 1)
            false -> index
        }}
    }}
    "#
        );

        let diagnostics =
            checked_program_result(&source).expect_err("arity misuse must be rejected");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "clause `{clause}`: expected `{expected}`, got: {:?}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>()
        );
    }
}

/// The view's natural distance proves its canonical dependent range and the
/// private witness retains the constraint independently of the public contract.
#[test]
fn rank_range_on_increasing_to_is_consumed_and_recorded() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.climb(4, 0);
    }

    machine Main::climb(&mut self, limit: u64, index: u64)
    terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
    -> u64
    {
        transition index < limit {
            true -> self.climb(limit, index + 1)
            false -> index
        }
    }
    "#;

    let typed = typed_program(source);

    let witness = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::climb")
        .expect("climb machine")
        .termination_plan
        .implementation_witness
        .as_ref()
        .expect("climb witness");
    let range = witness.rank_range.as_ref().expect("recorded rank range");
    assert_eq!(range.floor, "0");
    assert_eq!(range.ceiling, "limit");
    assert!(range.ceiling_inclusive);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("the structurally-true rank range should verify");
}

/// TPR3 completion: checker legality resolves subjects, view arguments, view
/// identity, and rank range from the normalized witness. No parallel authored
/// spans survive on the typed machine.
#[test]
fn termination_checker_uses_normalized_witness_without_parallel_spans() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.climb(4, 0);
    }

    machine Main::climb(&mut self, limit: u64, index: u64)
    terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
    -> u64
    {
        transition index < limit {
            true -> self.climb(limit, index + 1)
            false -> index
        }
    }
    "#;

    let typed = typed_program(source);

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::climb")
        .expect("climb machine");
    assert_eq!(
        machine
            .termination_plan
            .implementation_witness
            .as_ref()
            .expect("ranking witness")
            .view_path,
        "Nat::IncreasingTo"
    );

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("normalized witness should independently prove the bounded climb");
}

/// Unproved constraints report the failed rank-range obligation, not a
/// blanket source-shape restriction.
#[test]
fn rank_range_unverifiable_shapes_are_rejected_with_directed_messages() {
    let cases = [
        (
            "terminates by index -> Nat::IncreasingTo(limit) in 1..=limit;",
            "cannot prove rank range",
        ),
        (
            "terminates by index -> Nat::IncreasingTo(limit) in 0..=index;",
            "cannot prove rank range",
        ),
        (
            "terminates by index -> Nat::IncreasingTo(limit) in 0..limit;",
            "cannot prove rank range",
        ),
        (
            "terminates by index in 0..=limit;",
            "cannot prove rank range",
        ),
    ];
    for (clause, expected) in cases {
        let source = format!(
            r#"
    data Main {{}}

    machine Main::main(&mut self) {{
        let value: u64 = self.climb(4, 0);
    }}

    machine Main::climb(&mut self, limit: u64, index: u64)
    {clause}
    -> u64
    {{
        transition index < limit {{
            true -> self.climb(limit, index + 1)
            false -> index
        }}
    }}
    "#
        );

        let diagnostics =
            checked_program_result(&source).expect_err("an unverifiable range must be rejected");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "clause `{clause}`: expected `{expected}`, got: {:?}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>()
        );
    }
}

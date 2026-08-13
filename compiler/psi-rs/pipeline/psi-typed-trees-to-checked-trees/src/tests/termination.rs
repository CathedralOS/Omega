use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

#[test]
fn rejects_terminating_recursive_machine_without_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64) terminates {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("termination check should fail");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("recursive cycle"))
    );
}

#[test]
fn accepts_slice_range_surface_during_checked_lowering() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> u64 {
        let values: [u64; 4] = [1, 2, 3, 4];
        let view: &[u64] = values.as_slice();
        let tail: &[u64] = view[1..];
        tail.len
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("checked lowering should accept ranges");
}

#[test]
fn accepts_terminating_countdown_machine_with_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("termination check should succeed");
}

#[test]
fn accepts_terminating_distance_machine_with_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by (index, limit) -> Nat::BoundedDistance;
    -> u64
    {
        transition index < limit {
            true -> self.walk(limit, index + 1)
            false -> index
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("termination distance proof should succeed");
}

#[test]
fn accepts_terminating_slice_distance_machine_with_decreases() {
    let source = r#"
    data Entry {
        value: i32;
    }

    data Main {
        entries: [Entry; 4];
    }

    machine Main::main(&mut self) {
        let view: &[Entry] = self.entries.as_slice();
        let value: u64 = self.walk(view, 0);
    }

    machine Main::walk(&mut self, entries: &[Entry], index: u64)
    terminates by (index, entries.len) -> Nat::BoundedDistance;
    -> u64
    {
        transition index < entries.len {
            true -> self.walk(entries, index + 1)
            false -> index
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("termination slice distance proof should succeed");
}

#[test]
fn rejects_terminating_countdown_machine_with_stalled_decrease() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining)
            false -> 0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("termination check should fail");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")
    }));
}

#[test]
fn rejects_terminating_slice_distance_machine_with_stalled_index() {
    let source = r#"
    data Entry {
        value: i32;
    }

    data Main {
        entries: [Entry; 4];
    }

    machine Main::main(&mut self) {
        let view: &[Entry] = self.entries.as_slice();
        let value: u64 = self.walk(view, 0);
    }

    machine Main::walk(&mut self, entries: &[Entry], index: u64)
    terminates by (index, entries.len) -> Nat::BoundedDistance;
    -> u64
    {
        transition index < entries.len {
            true -> self.walk(entries, index)
            false -> index
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("termination check should fail");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")
    }));
}

#[test]
fn rejects_terminating_slice_length_order_without_supported_progress_shape() {
    let source = r#"
    data Entry {
        value: i32;
    }

    data Main {
        entries: [Entry; 4];
    }

    machine Main::main(&mut self) {
        let view: &[Entry] = self.entries.as_slice();
        let value: u64 = self.walk(view, 0);
    }

    machine Main::walk(&mut self, entries: &[Entry], index: u64)
    terminates by entries -> Slice::Length;
    -> u64
    {
        transition index < entries.len {
            true -> self.walk(entries, index + 1)
            false -> index
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("termination check should fail");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")
    }));
}

#[test]
fn accepts_terminating_slice_length_order_with_shrinking_subslice() {
    let source = r#"
    data Entry {
        value: i32;
    }

    data Main {
        entries: [Entry; 4];
    }

    machine Main::main(&mut self) {
        let view: &[Entry] = self.entries.as_slice();
        let value: u64 = self.walk(view);
    }

    machine Main::walk(&mut self, entries: &[Entry])
    terminates by entries -> Slice::Length;
    -> u64
    {
        transition entries.len > 0 {
            true -> self.walk(entries[1..])
            false -> 0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("termination slice length proof should succeed");
}

#[test]
fn accepts_terminating_mutually_recursive_states_with_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> u64 {
        transition {
            _ -> self.ping(4)
        }
    }

    machine Main::ping(&mut self, remaining: u64)
    terminates by remaining -> Nat::Descending;
    -> u64
    {
        transition remaining > 0 {
            true -> pong(remaining - 1)
            false -> 0
        }

        state pong(&mut self, remaining: u64) -> u64 {
            transition remaining > 0 {
                true -> ping(remaining - 1)
                false -> 0
            }
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("mutual recursion decrease proof should succeed");
}

#[test]
fn rejects_terminating_mutually_recursive_states_without_decrease() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> u64 {
        transition {
            _ -> self.ping(4)
        }
    }

    machine Main::ping(&mut self, remaining: u64)
    terminates by remaining -> Nat::Descending;
    -> u64
    {
        transition remaining > 0 {
            true -> pong(remaining)
            false -> 0
        }

        state pong(&mut self, remaining: u64) -> u64 {
            transition remaining > 0 {
                true -> ping(remaining)
                false -> 0
            }
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("termination check should fail");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")
    }));
}

#[test]
fn infers_default_nat_descending_for_plain_usize_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("default nat-descending inference should succeed");
}

#[test]
fn infers_default_slice_length_for_plain_slice_decreases() {
    let source = r#"
    data Entry {
        value: i32;
    }

    data Main {
        entries: [Entry; 4];
    }

    machine Main::main(&mut self) {
        let view: &[Entry] = self.entries.as_slice();
        let value: u64 = self.walk(view);
    }

    machine Main::walk(&mut self, entries: &[Entry])
    terminates by entries;
    -> u64
    {
        transition entries.len > 0 {
            true -> self.walk(entries[1..])
            false -> 0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("default slice-length inference should succeed");
}

#[test]
fn infers_default_bounded_distance_for_plain_two_subject_tuple() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("default bounded-distance inference should succeed");
}

#[test]
fn accepts_explicit_named_bounded_distance_view() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by (index, limit) -> Nat::BoundedDistance;
    -> u64
    {
        transition index < limit {
            true -> self.walk(limit, index + 1)
            false -> index
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("explicit named bounded-distance view should prove");
}

#[test]
fn rejects_inverted_bounded_distance_with_naming_diagnostic() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by (limit, index);
    -> u64
    {
        transition index < limit {
            true -> self.walk(limit, index + 1)
            false -> index
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("inverted distance should fail");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`terminates by (limit, index)` inverts the named bounded distance")
                && diagnostic.message.contains("`Nat::BoundedDistance`")
                && diagnostic
                    .message
                    .contains("write `terminates by (index, limit) -> Nat::BoundedDistance`")
        }),
        "expected the inverted bounded-distance diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn rejects_retired_subtraction_decreases_spelling_with_tuple_guidance() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by limit - index;
    -> u64
    {
        transition index < limit {
            true -> self.walk(limit, index + 1)
            false -> index
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::walk")
        .expect("walk machine");
    assert_eq!(
        machine
            .termination_plan
            .implementation_witness
            .as_ref()
            .expect("ranking witness")
            .subjects,
        ["limit - index"]
    );
    let diagnostics =
        lower_typed_trees(typed).expect_err("the subtraction spelling is retired surface");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("the use-site subtraction `terminates by limit - index`")
                && diagnostic.message.contains("is retired")
                && diagnostic.message.contains(
                    "spell the ranking as `terminates by (index, limit) -> Nat::BoundedDistance`",
                )
        }),
        "expected the retired-subtraction diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn rejects_named_bounded_distance_view_over_single_subject() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates by remaining -> Nat::BoundedDistance;
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> 0
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics =
        lower_typed_trees(typed).expect_err("the view ranks a (lower, upper) pair only");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking")
    }));
}

#[test]
fn rejects_ambiguous_default_order_requiring_explicit_form() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> i32 {
        transition {
            _ -> self.countdown(2)
        }
    }

    machine Main::countdown(&mut self, remaining: i32)
    terminates by remaining;
    -> i32
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> remaining
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("ambiguous default order should fail");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot infer a ranking for `terminates by remaining`")
                && diagnostic
                    .message
                    .contains("signed values have no default well-founded order")
                && diagnostic
                    .message
                    .contains("`terminates by remaining -> View`")
        }),
        "expected a signed-value ambiguity diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn infers_default_nat_descending_for_plain_u32_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u32 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u32)
    terminates by remaining;
    -> u32
    {
        transition remaining > 0 {
            true -> self.countdown(remaining - 1)
            false -> remaining
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("default nat-descending inference should cover u32");
}

#[test]
fn plain_decreases_never_selects_a_declared_measure_even_when_unique() {
    let source = r#"
    data Card {
        power: u64;
    }

    measure Card::PowerOrder(card: Card) -> u64 { card.power }

    data Main {
    }

    machine Main::main(&mut self) {
        let value: u64 = self.weaken(Card { power: 3 });
    }

    machine Main::weaken(&mut self, card: Card)
    terminates by card;
    -> u64
    {
        transition card.power > 0 {
            true -> self.weaken(Card { power: card.power - 1 })
            false -> card.power
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics =
        lower_typed_trees(typed).expect_err("a unique declared measure must not be inferred");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot infer a ranking for `terminates by card`")
                && diagnostic
                    .message
                    .contains("declared measures are never selected implicitly")
                && diagnostic
                    .message
                    .contains("`terminates by card -> Card::PowerOrder`")
        }),
        "expected the declared-measure suggestion diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
}

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
    use psi_language_semantics::{RankingViewId, TerminationGuarantee};

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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

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
        &psi_language_semantics::TerminationInterface::Published(
            TerminationGuarantee::Terminates {
                premises: Vec::new()
            }
        )
    );
    assert!(promise.implementation_witness.is_none());

    // `terminates by remaining;`: witness only (publishes nothing); the
    // single u64 subject's canonical default elaborates immediately.
    let countdown = plan_of("Main::countdown");
    assert_eq!(
        &countdown.interface,
        &psi_language_semantics::TerminationInterface::InternalDerived
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
        &psi_language_semantics::TerminationInterface::InternalDerived
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
    use psi_language_semantics::RankingViewId;

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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

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

    let diagnostics = lower_typed_trees(typed).expect_err("a diverging recorded view must be loud");
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
    use psi_language_semantics::RankingViewId;

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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

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

    lower_typed_trees(typed).expect("the bounded increasing cursor should prove");
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics =
        lower_typed_trees(typed).expect_err("the unbounded increasing view must be rejected");

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

        let tokens = Lexer::new(source.as_str())
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let diagnostics = lower_typed_trees(typed).expect_err("arity misuse must be rejected");
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

/// TPR3 slice 3: the `in <range>` rank constraint is CONSUMED -- v1 accepts
/// exactly the shape true by the view's definition (`in 0..=limit` on
/// `Nat::IncreasingTo(limit)`) and records the verified fact in the plan
/// witness; every other shape gets a directed rejection.
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

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

    lower_typed_trees(typed).expect("the structurally-true rank range should verify");
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

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

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

    lower_typed_trees(typed)
        .expect("normalized witness should independently prove the bounded climb");
}

/// TPR3 slice 3: the range shapes v1 cannot verify are rejected with
/// DIRECTED messages -- nonzero floor, ceiling that is not the view's own
/// bound, exclusive ceiling, and a range on a non-argumented view.
#[test]
fn rank_range_unverifiable_shapes_are_rejected_with_directed_messages() {
    let cases = [
        (
            "terminates by index -> Nat::IncreasingTo(limit) in 1..=limit;",
            "rank floor above the natural floor",
        ),
        (
            "terminates by index -> Nat::IncreasingTo(limit) in 0..=index;",
            "is not the view's own bound",
        ),
        (
            "terminates by index -> Nat::IncreasingTo(limit) in 0..limit;",
            "spell the ceiling inclusively",
        ),
        (
            "terminates by index in 0..=limit;",
            "rank range is only consumed on the argumented",
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

        let tokens = Lexer::new(source.as_str())
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let diagnostics =
            lower_typed_trees(typed).expect_err("an unverifiable range must be rejected");
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

/// TPR3 slice 4: the checked termination facts -- the `checked_summary`'s
/// producer. Every acyclic checked body derives Terminates without a
/// witness; a proven witness establishes it WITH the resolved explicit
/// view. The local summary remains separate from the authored public promise.
#[test]
fn termination_facts_record_checked_summaries_and_resolved_views() {
    use psi_language_semantics::TerminationGuarantee;

    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let a: u64 = self.promise();
        let b: u64 = self.countdown(2);
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
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let facts = crate::checks::termination::build_termination_facts(&typed);
    let machine_symbol = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };

    let promise = facts
        .for_machine(machine_symbol("Main::promise"))
        .expect("acyclic claimant fact");
    assert_eq!(
        promise.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert!(promise.resolved_view_path.is_empty());

    let countdown = facts
        .for_machine(machine_symbol("Main::countdown"))
        .expect("proven witness fact");
    assert_eq!(
        countdown.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert_eq!(countdown.resolved_view_path, "Nat::Descending");

    let inferred = facts
        .for_machine(machine_symbol("Main::main"))
        .expect("unannotated acyclic body still gets a local summary");
    assert_eq!(
        inferred.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert!(inferred.resolved_view_path.is_empty());
}

#[test]
fn inferred_completion_never_publishes_a_promise() {
    use psi_language_semantics::TerminationGuarantee;

    let source = r#"
    data Main {}
    machine Main::run(&mut self) -> u64 { self.inferred() }
    machine Main::inferred(&mut self) -> u64 { 1 }
    machine Main::promised(&mut self) -> u64 terminates; { 1 }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let inferred = symbol_of("Main::inferred");
    let promised = symbol_of("Main::promised");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    assert_eq!(
        checked
            .facts
            .termination
            .for_machine(inferred)
            .expect("inferred local summary")
            .checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(inferred)
            .expect("inferred contract plan")
            .termination,
        psi_language_semantics::TerminationInterface::InternalDerived,
        "body inference must never redefine the published contract"
    );
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(promised)
            .expect("promised contract plan")
            .termination,
        psi_language_semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new()
        })
    );
}

/// TPR4 slice 2: the requirement's authored guarantee PROPAGATES into the
/// resolved trait-signature record (populated at syntax->resolved, per
/// signature -- inheritance at conformance consumes it next).
#[test]
fn trait_requirement_guarantee_propagates_to_resolved_signatures() {
    let source = r#"
    trait Worker {
        machine run(&mut self, n: u64) -> u64 terminates;
        machine peek(&self) -> u64;
    }

    data Main {}

    machine Main::main(&mut self) -> u64 { 7 }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");

    let worker = resolved
        .traits
        .iter()
        .find(|definition| definition.name.as_str() == "Worker")
        .expect("Worker trait");
    let signatures = resolved.trait_machine_signatures(worker.machines);
    let flag_of = |name: &str| {
        signatures
            .iter()
            .find(|signature| signature.name.as_str() == name)
            .unwrap_or_else(|| panic!("signature {name}"))
            .terminates_guarantee
    };
    assert!(flag_of("run"), "run authored the guarantee");
    assert!(!flag_of("peek"), "peek promised nothing");
}

/// TPR4 slice 3 (decision 23): an implementation satisfying a requirement
/// that authored `terminates;` INHERITS the published guarantee -- it does
/// not repeat the clause. A cyclic inheritor must then supply the
/// discharging witness or FAIL (the inherited claim is not optional), and
/// with a witness it proves like any measured machine.
#[test]
fn implementation_inherits_requirement_guarantee() {
    use psi_language_semantics::TerminationGuarantee;

    let source = r#"
    trait Worker {
        machine run(&mut self, n: u64) -> u64 terminates;
    }

    data Main {}

    machine Main::run(&mut self, n: u64) -> u64 satisfies Worker::run {
        n
    }

    machine Main::main(&mut self) -> u64 {
        let value: u64 = self.run(7);
        value
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let run = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("run machine");
    assert_eq!(
        &run.termination_plan.interface,
        &psi_language_semantics::TerminationInterface::Published(
            TerminationGuarantee::Terminates {
                premises: Vec::new()
            }
        ),
        "the implementation inherits the requirement's published guarantee"
    );
    assert!(run.termination_plan.implementation_witness.is_none());

    lower_typed_trees(typed).expect("an acyclic inheritor discharges the claim for free");
}

/// TPR4: omission has different normalized meaning on a private body and a
/// public requirement. The implementation inherits the requirement's
/// published `NoGuarantee`; its locally inferred completion remains
/// implementation evidence rather than silently strengthening that contract.
#[test]
fn public_termination_omission_is_distinct_from_private_derivation() {
    let source = r#"
    trait Worker {
        machine run(&self) -> u64;
    }

    data Main {}

    machine Main::run(&self) -> u64 satisfies Worker::run {
        1
    }

    machine Main::local(&self) -> u64 {
        2
    }

    machine Main::main(&mut self) -> u64 {
        0
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let plan_of = |name: &str| {
        &typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .termination_plan
            .interface
    };
    assert_eq!(
        plan_of("Main::run"),
        &psi_language_semantics::TerminationInterface::Published(
            psi_language_semantics::TerminationGuarantee::NoGuarantee,
        )
    );
    assert_eq!(
        plan_of("Main::local"),
        &psi_language_semantics::TerminationInterface::InternalDerived
    );
}

/// TPR4 slice 3: a CYCLIC implementation inheriting the guarantee without a
/// witness FAILS with the missing-witness diagnostic -- the inherited claim
/// is enforced by the same plan gate as an authored one. Supplying the
/// witness (`terminates by n;`) discharges it.
#[test]
fn cyclic_inheritor_without_witness_fails_and_witness_discharges() {
    let template = |witness_clause: &str| {
        format!(
            r#"
    trait Worker {{
        machine run(&mut self, n: u64) -> u64 terminates;
    }}

    data Main {{}}

    machine Main::run(&mut self, n: u64) -> u64 satisfies Worker::run
    {witness_clause}
    {{
        transition n > 0 {{
            true -> self.run(n - 1)
            false -> 0
        }}
    }}

    machine Main::main(&mut self) -> u64 {{
        let value: u64 = self.run(7);
        value
    }}
    "#
        )
    };

    // Without a witness: the inherited claim cannot be checked on a cycle.
    let source = template("");
    let tokens = Lexer::new(source.as_str())
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics =
        lower_typed_trees(typed).expect_err("a cyclic inheritor without a witness must fail");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("recursive cycle")
                && diagnostic.message.contains("ranking witness")
        }),
        "expected the missing-witness diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );

    // With the witness: the inherited claim discharges.
    let source = template("terminates by n;");
    let tokens = Lexer::new(source.as_str())
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("the witness discharges the inherited claim");
}

#[test]
fn symbol_resolved_service_reach_propagates_boundary_identity_and_parent_closure() {
    let source = r#"
    boundary trait Readable {
        machine read() -> u64;
    }

    boundary trait Filesystem: Readable {
    }

    data Worker { reader: Readable; }
    machine Worker::run(&mut self) -> u64 reaches Filesystem {
        self.reader.read()
    }

    data Main { worker: Worker; }
    machine Main::main(&mut self) -> u64 {
        self.worker.run()
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let worker = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .expect("worker")
        .symbol;
    let main = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main")
        .symbol;
    let checked = lower_typed_trees(typed).expect("service ceiling should admit the body");

    let worker_reach = checked
        .facts
        .service_reaches
        .for_machine(worker)
        .expect("worker service facts");
    let published = checked
        .facts
        .service_reaches
        .rows
        .services(worker_reach.published_ceiling);
    let published_names = published
        .iter()
        .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(published_names, ["Filesystem", "Readable"]);

    let main_reach = checked
        .facts
        .service_reaches
        .for_machine(main)
        .expect("main service facts");
    assert_eq!(
        checked
            .facts
            .service_reaches
            .rows
            .services(main_reach.inferred_transitive),
        published,
        "an internal caller consumes the callee's published service ceiling",
    );
}

#[test]
fn symbol_resolved_service_ceiling_rejects_undeclared_boundary_reach() {
    let source = r#"
    boundary trait Readable { machine read() -> u64; }
    boundary trait Queryable { }

    data Main { reader: Readable; }
    machine Main::run(&mut self) -> u64 reaches Queryable {
        self.reader.read()
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("service ceiling must reject widening");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("reaches undeclared service `Readable`")
        }),
        "expected symbol-resolved service ceiling diagnostic, got {diagnostics:#?}",
    );
}

#[test]
fn operational_plans_are_independent_from_service_reach_rows() {
    use psi_language_semantics::{
        BlockingInterface, ServiceReachInterface, ServiceReachRowTable, SuspensionInterface,
    };

    let source = r#"
    boundary trait Clock { machine read(); }

    data Sleeper { clock: Clock; }
    machine Sleeper::wait(&mut self) reaches Clock suspends; blocks; {}

    data Main { sleeper: Sleeper; }
    machine Main::run(&mut self) {
        self.sleeper.wait();
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let wait = symbol_of("Sleeper::wait");
    let run = symbol_of("Main::run");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    let wait_plan = checked
        .facts
        .contract_plans
        .for_machine(wait)
        .expect("published callee plan");
    assert_eq!(
        wait_plan.suspension.interface,
        SuspensionInterface::PublishedMaySuspend(true)
    );
    assert_eq!(
        wait_plan.blocking.interface,
        BlockingInterface::PublishedMayBlock(true)
    );
    assert!(!wait_plan.suspension.checked_may_suspend);
    assert!(!wait_plan.blocking.checked_may_block);
    let ServiceReachInterface::PublishedCeiling(wait_ceiling) = wait_plan.service_reach.interface
    else {
        panic!("wait should publish its authored service ceiling");
    };
    let clock = checked
        .facts
        .service_reaches
        .services
        .id_for_name("Clock")
        .expect("Clock service");
    assert_eq!(
        checked.facts.service_reaches.rows.services(wait_ceiling),
        &[clock]
    );
    assert_eq!(
        wait_plan.service_reach.checked_inferred,
        ServiceReachRowTable::EMPTY_ROW
    );

    let run_plan = checked
        .facts
        .contract_plans
        .for_machine(run)
        .expect("caller plan");
    assert_eq!(
        run_plan.suspension.interface,
        SuspensionInterface::InternalInferred
    );
    assert_eq!(
        run_plan.blocking.interface,
        BlockingInterface::InternalInferred
    );
    // Local calls to checked bodies consume the honest checked summary, not
    // the callee's authored ceiling. `wait` is quiet, so the private caller
    // remains quiet even though `wait` publishes room to suspend and block.
    assert!(!run_plan.suspension.checked_may_suspend);
    assert!(!run_plan.blocking.checked_may_block);

    let run_reach = checked
        .facts
        .service_reaches
        .for_machine(run)
        .expect("caller service row");
    assert_eq!(
        checked
            .facts
            .service_reaches
            .rows
            .services(run_reach.inferred_transitive),
        &[clock],
        "the authored service ceiling remains the modular caller contract",
    );
}

#[test]
fn qualification_facts_record_policy_commitments() {
    // STR4 checked plans, slice 2: a machine whose body casts under an
    // arithmetic policy COMMITS to that policy's fixed semantic identity;
    // a cast-free machine carries no entry.
    use psi_language_semantics::SemanticDomainTable;

    let source = r#"
    data Main {}

    domain i64::Km
    requires
        self >= 0;

    machine Main::clamped(&mut self, value: u64) -> u8 {
        let squeezed: u8 in Saturating = value as u8 in Saturating;
        squeezed as u8
    }

    machine Main::minted(&mut self) -> i64 {
        let distance: i64 in Km = 5 as i64 in Km;
        distance as i64
    }

    machine Main::main(&mut self) -> u64 {
        7
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let clamped_symbol = symbol_of("Main::clamped");
    let minted_symbol = symbol_of("Main::minted");
    let main_symbol = symbol_of("Main::main");
    let km_id = typed
        .semantic_domains
        .lookup("i64::Km")
        .expect("Km interned");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    let clamped = checked
        .facts
        .qualifications
        .for_machine(clamped_symbol)
        .expect("clamped's qualification fact");
    assert_eq!(
        clamped.body_committed,
        vec![SemanticDomainTable::SATURATING],
        "the saturating cast commits to the fixed Saturating identity"
    );
    // The MINT commits to the DECLARED domain's interned identity.
    let minted = checked
        .facts
        .qualifications
        .for_machine(minted_symbol)
        .expect("minted's qualification fact");
    assert_eq!(minted.body_committed, vec![km_id]);
    assert!(
        checked
            .facts
            .qualifications
            .for_machine(main_symbol)
            .is_none(),
        "a cast-free machine carries no qualification entry"
    );
}

#[test]
fn contract_plans_fingerprint_published_halves() {
    // STR4 checked plans (machine_taxonomy.md): the contract fingerprint
    // covers ONLY the published halves -- two machines with the same
    // declared surface share it; a different `reaches` clause changes it;
    // inferred rows never enter (prover-independence by construction).
    let source = r#"
    boundary trait Filesystem {}
    boundary trait Network {}

    data Main {
        left: u64;
        right: u64;
    }
    data FrameCell { value: u64; }
    data FrameOwner {
        first: FrameCell;
        second: FrameCell;
    }

    machine Main::quiet_a(&mut self) -> u64 reaches Filesystem {
        self.left = 1;
        1
    }
    machine Main::quiet_b(&mut self) -> u64 reaches Filesystem {
        self.right = 2;
        2
    }
    machine Main::loud(&mut self) -> u64 reaches Network { 3 }
    machine bounded_ab(x: u64, y: u64) -> u64
    requires
        x >= 1;
        y >= 2
    { x }
    machine bounded_ba(x: u64, y: u64) -> u64
    requires
        y >= 2;
        x >= 1
    { x }
    machine bounded_wider(x: u64, y: u64) -> u64
    requires
        x >= 1;
        y >= 3
    { x }
    machine bounded_renamed(alpha: u64, beta: u64) -> u64
    requires
        alpha >= 1;
        beta >= 2
    { alpha }
    machine write_alpha(alpha: &mut u64) {
        alpha = 1;
    }
    machine write_beta(beta: &mut u64) {
        beta = 2;
    }
    machine Main::transitioning(&mut self) {
        transition { _ -> write_left() }
        state write_left(&mut self) {
            self.left = 3;
            transition { _ -> finished() }
        }
        state finished(&mut self) { }
    }
    machine write_through_transition(value: &mut u64) {
        transition { _ -> write(value) }
        state write(slot: &mut u64) { slot = 4; }
    }
    machine Main::cyclic(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) { transition { _ -> cycle() } }
    }
    machine reordered_cycle(first: u64, second: u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: u64, right: u64) {
            transition { _ -> cycle(right, left) }
        }
    }
    machine reordered_shared_cycle(first: &u64, second: &u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &u64, right: &u64) {
            transition { _ -> cycle(right, left) }
        }
    }
    machine reordered_mut_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            transition { _ -> cycle(right, left) }
        }
    }
    machine rotating_mut_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            left = 11;
            transition { _ -> cycle(right, left) }
        }
    }
    machine rotating_mut_scc(first: &mut u64, second: &mut u64) {
        transition { _ -> write(first, second) }
        state write(left: &mut u64, right: &mut u64) {
            left = 12;
            transition { _ -> forward(right, left) }
        }
        state forward(left: &mut u64, right: &mut u64) {
            transition { _ -> write(right, left) }
        }
    }
    machine call_rotating_mut_cycle(first: &mut u64, second: &mut u64) {
        rotating_mut_cycle(first, second);
    }
    machine call_rotating_mut_scc(first: &mut u64, second: &mut u64) {
        rotating_mut_scc(first, second);
    }
    machine rotating_mut_fields(first: &mut FrameCell, second: &mut FrameCell) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut FrameCell, right: &mut FrameCell) {
            left.value = 13;
            transition { _ -> cycle(right, left) }
        }
    }
    machine FrameOwner::call_rotating_mut_fields(&mut self) {
        rotating_mut_fields(&mut self.first, &mut self.second);
    }
    boundary trait Device {
        machine overwrite(value: &mut u64);
    }
    data Wrapper { device: Device; value: u64; }
    machine Wrapper::boundary_call(&mut self) {
        self.device.overwrite(&mut self.value);
    }
    machine Main::direct_self_loop(&mut self) {
        self.left = 9;
        transition { _ -> self }
    }
    machine Main::branching(&mut self) {
        transition self.left == 0 {
            true -> write_left_branch()
            false -> write_right_branch()
        }
        state write_left_branch(&mut self) {
            self.left = 5;
            transition { _ -> branch_done() }
        }
        state write_right_branch(&mut self) {
            self.right = 6;
            transition { _ -> branch_done() }
        }
        state branch_done(&mut self) { }
    }
    machine Main::touch_right_value(&mut self) -> bool {
        self.right = 8;
        true
    }
    machine Main::call_bearing(&mut self) -> bool {
        let seed: bool = self.touch_right_value();
        transition self.touch_right_value() == seed {
            true -> call_bearing_done(self.touch_right_value())
            false -> call_bearing_done(seed)
        }
        state call_bearing_done(&mut self, value: bool) -> bool {
            let answer: bool = self.touch_right_value();
            answer
        }
    }
    machine Main::main(&mut self) -> u64 { 7 }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let quiet_a = symbol_of("Main::quiet_a");
    let quiet_b = symbol_of("Main::quiet_b");
    let loud = symbol_of("Main::loud");
    let write_alpha = symbol_of("write_alpha");
    let write_beta = symbol_of("write_beta");
    let transitioning = symbol_of("Main::transitioning");
    let write_through_transition = symbol_of("write_through_transition");
    let cyclic = symbol_of("Main::cyclic");
    let reordered_cycle = symbol_of("reordered_cycle");
    let reordered_shared_cycle = symbol_of("reordered_shared_cycle");
    let reordered_mut_cycle = symbol_of("reordered_mut_cycle");
    let rotating_mut_cycle = symbol_of("rotating_mut_cycle");
    let rotating_mut_scc = symbol_of("rotating_mut_scc");
    let call_rotating_mut_cycle = symbol_of("call_rotating_mut_cycle");
    let call_rotating_mut_scc = symbol_of("call_rotating_mut_scc");
    let rotating_mut_fields = symbol_of("rotating_mut_fields");
    let call_rotating_mut_fields = symbol_of("FrameOwner::call_rotating_mut_fields");
    let boundary_call = symbol_of("Wrapper::boundary_call");
    let direct_self_loop = symbol_of("Main::direct_self_loop");
    let branching = symbol_of("Main::branching");
    let call_bearing = symbol_of("Main::call_bearing");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    let plan = |symbol| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol)
            .expect("contract plan")
    };
    // Same declared surface (different BODIES) -> same fingerprint.
    assert_eq!(plan(quiet_a).fingerprint, plan(quiet_b).fingerprint);
    let frame = |symbol| {
        &plan(symbol)
            .inferred_write_frames
            .first()
            .expect("entry-state frame")
            .frame
    };
    assert_eq!(frame(quiet_a).paths(), &["self.left".to_owned()]);
    assert_eq!(frame(quiet_b).paths(), &["self.right".to_owned()]);
    assert_ne!(frame(quiet_a).fingerprint(), frame(quiet_b).fingerprint());
    assert_eq!(frame(write_alpha).paths(), &["$P0".to_owned()]);
    assert_eq!(frame(write_alpha), frame(write_beta));
    assert_eq!(frame(transitioning).paths(), &["self.left".to_owned()]);
    assert_eq!(frame(write_through_transition).paths(), &["$P0".to_owned()]);
    assert_eq!(
        frame(cyclic).complete_paths(),
        Some([].as_slice()),
        "an argument-free named state cycle preserves its complete empty namespace"
    );
    assert_eq!(
        frame(reordered_cycle).complete_paths(),
        Some([].as_slice()),
        "reordering read-only scalar parameters cannot redirect a caller-visible write"
    );
    assert_eq!(
        frame(reordered_shared_cycle).complete_paths(),
        Some([].as_slice()),
        "reordering shared-reference parameters cannot redirect a caller-visible write"
    );
    assert_eq!(
        frame(reordered_mut_cycle).complete_paths(),
        Some([].as_slice()),
        "an exact exclusive-parameter permutation with no writes has a complete empty frame"
    );
    assert_eq!(
        frame(rotating_mut_cycle).paths(),
        &["$P0".to_owned(), "$P1".to_owned()],
        "a write rotating through an exclusive-parameter permutation reaches the complete finite orbit"
    );
    assert_eq!(
        frame(rotating_mut_scc).paths(),
        &["$P0".to_owned()],
        "a multi-state SCC composes its exact permutations before publishing the entry frame"
    );
    assert_eq!(
        frame(call_rotating_mut_cycle).paths(),
        &["$P0".to_owned(), "$P1".to_owned()],
        "a resolved caller instantiates the complete permutation-orbit frame"
    );
    assert_eq!(
        frame(call_rotating_mut_scc).paths(),
        &["$P0".to_owned()],
        "a resolved caller preserves the multi-state SCC's exact positional frame"
    );
    assert_eq!(
        frame(rotating_mut_fields).paths(),
        &["$P0.value".to_owned(), "$P1.value".to_owned()],
        "permutation closure preserves written member suffixes"
    );
    assert_eq!(
        frame(call_rotating_mut_fields).paths(),
        &[
            "self.first.value".to_owned(),
            "self.second.value".to_owned(),
        ],
        "caller instantiation preserves member arguments and written suffixes"
    );
    assert_eq!(
        frame(boundary_call).paths(),
        &["self.device".to_owned(), "self.value".to_owned()],
        "checked frames retain exact nested boundary receiver and out-argument writes"
    );
    assert_eq!(
        frame(direct_self_loop).paths(),
        &["self.left".to_owned()],
        "a direct self target repeats the same finite may-write frame"
    );
    assert_eq!(
        frame(branching).paths(),
        &["self.left".to_owned(), "self.right".to_owned()],
        "both conditional arms compose and may share one memoized tail state"
    );
    assert_eq!(
        frame(call_bearing).paths(),
        &["self.right".to_owned()],
        "value calls in locals, guards, jump arguments, and terminal results compose"
    );
    // A different `reaches` clause -> a different fingerprint.
    assert_ne!(plan(quiet_a).fingerprint, plan(loud).fingerprint);
    // Slice 2: REQUIRES clause ORDER never enters the identity...
    let ab = symbol_of_checked(&checked, "bounded_ab");
    let ba = symbol_of_checked(&checked, "bounded_ba");
    let wider = symbol_of_checked(&checked, "bounded_wider");
    assert_eq!(plan(ab).fingerprint, plan(ba).fingerprint);
    // ...but a changed BOUND does.
    assert_ne!(plan(ab).fingerprint, plan(wider).fingerprint);
    // Parameter RENAMES normalize positionally -- identical contracts.
    let renamed = symbol_of_checked(&checked, "bounded_renamed");
    assert_eq!(plan(ab).fingerprint, plan(renamed).fingerprint);
}

#[test]
fn write_frame_stays_opaque_for_non_bijective_exclusive_cycle() {
    // This deliberately duplicates one exclusive parameter on a backedge.
    // Query the typed-tree frame resolver directly: later borrow validation is
    // allowed to reject the source independently, while R5 must still fail
    // closed if it is asked to summarize the malformed cycle.
    let source = r#"
    machine duplicate_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            left = 1;
            transition { _ -> cycle(left, left) }
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "duplicate_cycle")
        .expect("duplicate-cycle machine");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("duplicate-cycle entry state");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    assert!(
        !resolver
            .inferred_state_write_frame(machine, entry)
            .is_complete(),
        "duplicating one exclusive root is not a permutation and must leave the frame opaque"
    );
}

#[test]
fn write_frame_composes_transparent_helpers_in_exclusive_cycles() {
    let source = r#"
    machine identity(value: &mut u64) -> &mut u64 {
        value
    }

    machine write(value: &mut u64) {
        value = 2;
    }

    machine write_then_identity(value: &mut u64) -> &mut u64 {
        write(value);
        value
    }

    machine transparent_cycle(value: &mut u64) {
        transition { _ -> cycle(value) }
        state cycle(item: &mut u64) {
            item = 1;
            transition { _ -> cycle(identity(item)) }
        }
    }

    machine write_through_helper_cycle(value: &mut u64) {
        transition { _ -> cycle(value) }
        state cycle(item: &mut u64) {
            item = 1;
            transition { _ -> cycle(write_then_identity(item)) }
        }
    }

    machine duplicate_transparent_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            left = 1;
            transition { _ -> cycle(identity(left), identity(left)) }
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");
    let frame = |name: &str| {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        resolver.inferred_state_write_frame(machine, entry)
    };

    assert_eq!(
        frame("transparent_cycle").complete_paths(),
        Some(["$P0".to_owned()].as_slice()),
        "a transparent identity helper preserves the cycle's exact root permutation"
    );
    assert_eq!(
        frame("write_through_helper_cycle").complete_paths(),
        Some(["$P0".to_owned()].as_slice()),
        "a write-through helper publishes its write without obscuring the cycle's root permutation"
    );
    assert!(
        !frame("duplicate_transparent_cycle").is_complete(),
        "duplicate_transparent_cycle must remain opaque without an exact bijection"
    );
}

#[test]
fn write_frame_substitutes_stable_local_exclusive_alias_origins() {
    let source = r#"
    data Cell { value: u64; }
    data BorrowCell<'source> { value: &'source mut u64; }
    data Group { cells: [Cell; 2]; }
    data Main {
        value: u64;
        cell: Cell;
        cells: [Cell; 2];
        values: [u64; 2];
        group: Group;
        groups: [Group; 2];
    }

    machine Main::local_alias_acyclic(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = 1;
    }

    machine Main::local_alias_member(&mut self) {
        let alias: &mut Cell = &mut self.cell;
        alias.value = 2;
    }

    machine write_local_alias(value: &mut u64) {
        value = 3;
    }

    machine Main::local_alias_call(&mut self) {
        let alias: &mut u64 = &mut self.value;
        write_local_alias(alias);
    }

    machine alias_parameter(value: &mut u64) {
        let alias: &mut u64 = &mut value;
        alias = 4;
    }

    machine Main::call_alias_parameter(&mut self) {
        alias_parameter(&mut self.value);
    }

    machine Main::local_alias_self_loop(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = 5;
        transition { _ -> self }
    }

    machine Main::named_alias_acyclic(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> finish(alias) }
        state finish(&mut self, value: &mut u64) {
            value = 6;
        }
    }

    machine Main::named_alias_multihop(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> forward(alias) }
        state forward(&mut self, value: &mut u64) {
            transition { _ -> finish(value) }
        }
        state finish(&mut self, value: &mut u64) {
            value = 7;
        }
    }

    machine Main::named_alias_member(&mut self) {
        let alias: &mut Cell = &mut self.cell;
        transition { _ -> finish(alias) }
        state finish(&mut self, value: &mut Cell) {
            value.value = 8;
        }
    }

    machine alias_parameter_named(value: &mut u64) {
        let alias: &mut u64 = &mut value;
        transition { _ -> finish(alias) }
        state finish(value: &mut u64) {
            value = 9;
        }
    }

    machine Main::call_alias_parameter_named(&mut self) {
        alias_parameter_named(&mut self.value);
    }

    machine Main::local_alias_chain(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        let third: &mut u64 = &mut second;
        third = 10;
    }

    machine Main::local_alias_chain_member_write(&mut self) {
        let first: &mut Cell = &mut self.cell;
        let second: &mut Cell = &mut first;
        second.value = 11;
    }

    machine Main::local_alias_projected_reborrow(&mut self) {
        let first: &mut Cell = &mut self.cell;
        let second: &mut u64 = &mut first.value;
        second = 11;
    }

    machine Main::local_alias_chain_call(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        write_local_alias(second);
    }

    machine alias_parameter_chain(value: &mut u64) {
        let first: &mut u64 = &mut value;
        let second: &mut u64 = &mut first;
        second = 12;
    }

    machine alias_parameter_projection(cell: &mut Cell) {
        let root: &mut Cell = &mut cell;
        let value: &mut u64 = &mut root.value;
        value = 12;
    }

    machine Main::call_alias_parameter_chain(&mut self) {
        alias_parameter_chain(&mut self.value);
    }

    machine Main::call_alias_parameter_projection(&mut self) {
        alias_parameter_projection(&mut self.cell);
    }

    machine Main::local_alias_chain_self_loop(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        second = 13;
        transition { _ -> self }
    }

    machine Main::named_alias_chain(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        transition { _ -> finish(second) }
        state finish(&mut self, value: &mut u64) {
            value = 14;
        }
    }

    machine write_cell(cell: &mut Cell) {
        cell.value = 15;
    }

    machine Main::indexed_alias_fixed(&mut self) {
        let alias: &mut u64 = &mut self.values[0];
        alias = 16;
    }

    machine Main::indexed_alias_dynamic(&mut self, index: u64)
    requires
        index < 2
    {
        let alias: &mut u64 = &mut self.values[index];
        alias = 17;
    }

    machine Main::indexed_alias_member_write(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        alias.value = 18;
    }

    machine Main::indexed_alias_call(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        write_cell(alias);
    }

    machine indexed_alias_parameter(cells: &mut [Cell; 2]) {
        let alias: &mut Cell = &mut cells[0];
        alias.value = 19;
    }

    machine Main::call_indexed_alias_parameter(&mut self) {
        indexed_alias_parameter(&mut self.cells);
    }

    machine Main::indexed_alias_chain(&mut self) {
        let root: &mut Cell = &mut self.cells[0];
        let alias: &mut Cell = &mut root;
        alias.value = 20;
    }

    machine Main::indexed_alias_projected_reborrow(&mut self) {
        let root: &mut [u64; 2] = &mut self.values;
        let alias: &mut u64 = &mut root[0];
        alias = 20;
    }

    machine Main::coarse_alias_projected_reborrow(&mut self) {
        let root: &mut Cell = &mut self.cells[0];
        let alias: &mut u64 = &mut root.value;
        alias = 20;
    }

    machine Main::member_indexed_alias_projected_reborrow(&mut self) {
        let group: &mut Group = &mut self.group;
        let alias: &mut Cell = &mut group.cells[0];
        alias.value = 20;
    }

    machine Main::direct_member_after_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[0].value;
        alias = 20;
    }

    machine Main::indexed_alias_self_loop(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        alias.value = 21;
        transition { _ -> self }
    }

    machine Main::indexed_alias_named(&mut self) {
        let alias: &mut Cell = &mut self.cells[0];
        transition { _ -> finish(alias) }
        state finish(&mut self, cell: &mut Cell) {
            cell.value = 22;
        }
    }

    machine Main::direct_indexed_call(&mut self) {
        write_cell(&mut self.cells[0]);
    }

    machine Main::direct_indexed_transition(&mut self) {
        transition { _ -> finish(&mut self.cells[0]) }
        state finish(&mut self, cell: &mut Cell) {
            cell.value = 23;
        }
    }

    machine mutate_group(group: &mut Group) {
        let alias: &mut Cell = &mut group.cells[0];
        alias.value = 24;
    }

    machine Main::call_indexed_group(&mut self) {
        mutate_group(&mut self.groups[0]);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    let expected = [
        ("Main::local_alias_acyclic", "self.value"),
        ("Main::local_alias_member", "self.cell.value"),
        ("Main::local_alias_call", "self.value"),
        ("alias_parameter", "$P0"),
        ("Main::call_alias_parameter", "self.value"),
        ("Main::local_alias_self_loop", "self.value"),
        ("Main::named_alias_acyclic", "self.value"),
        ("Main::named_alias_multihop", "self.value"),
        ("Main::named_alias_member", "self.cell.value"),
        ("alias_parameter_named", "$P0"),
        ("Main::call_alias_parameter_named", "self.value"),
        ("Main::local_alias_chain", "self.value"),
        ("Main::local_alias_chain_member_write", "self.cell.value"),
        ("Main::local_alias_projected_reborrow", "self.cell.value"),
        ("Main::local_alias_chain_call", "self.value"),
        ("alias_parameter_chain", "$P0"),
        ("alias_parameter_projection", "$P0.value"),
        ("Main::call_alias_parameter_chain", "self.value"),
        ("Main::call_alias_parameter_projection", "self.cell.value"),
        ("Main::local_alias_chain_self_loop", "self.value"),
        ("Main::named_alias_chain", "self.value"),
        ("Main::indexed_alias_fixed", "self.values"),
        ("Main::indexed_alias_dynamic", "self.values"),
        ("Main::indexed_alias_member_write", "self.cells"),
        ("Main::indexed_alias_call", "self.cells"),
        ("indexed_alias_parameter", "$P0"),
        ("Main::call_indexed_alias_parameter", "self.cells"),
        ("Main::indexed_alias_chain", "self.cells"),
        ("Main::indexed_alias_projected_reborrow", "self.values"),
        ("Main::coarse_alias_projected_reborrow", "self.cells"),
        (
            "Main::member_indexed_alias_projected_reborrow",
            "self.group.cells",
        ),
        ("Main::direct_member_after_index_alias", "self.cells"),
        ("Main::indexed_alias_self_loop", "self.cells"),
        ("Main::indexed_alias_named", "self.cells"),
        ("Main::direct_indexed_call", "self.cells"),
        ("Main::direct_indexed_transition", "self.cells"),
        ("mutate_group", "$P0.cells"),
        ("Main::call_indexed_group", "self.groups"),
    ];
    for (name, expected_path) in expected {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some([expected_path.to_owned()].as_slice()),
            "{name} must substitute the local alias back to its visible origin"
        );
    }
}

#[test]
fn write_frame_distinguishes_isolated_and_unrepresentable_local_aliases() {
    let source = r#"
    data Cell { value: u64; }
    data Main {
        value: u64;
        other: u64;
        cell: Cell;
        cells: [u64; 2];
        cell_items: [Cell; 2];
    }

    machine Main::rebound_alias(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = &mut self.other;
        alias = 1;
    }

    machine Main::alias_chain_upstream_rebind(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        first = &mut self.other;
        second = 2;
    }

    machine Main::alias_chain_leaf_rebind(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        second = &mut self.other;
        second = 2;
    }

    machine Main::alias_chain_rebind_from_alias(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut self.other;
        second = &mut first;
        second = 2;
    }

    machine Main::local_origin(&mut self) {
        let local: u64 = 0;
        let alias: &mut u64 = &mut local;
        alias = 2;
    }

    machine Main::indexed_local_origin(&mut self) {
        let local: [u64; 2] = [0, 1];
        let alias: &mut u64 = &mut local[0];
        alias = 3;
    }

    machine Main::constrained_local_origin(&mut self) {
        let local: u64 [0..=3] = 0;
        let alias: &mut u64 = &mut local;
        alias = 2;
    }

    machine Main::indexed_constrained_local_origin(&mut self) {
        let local: [u64 [0..=3]; 2] = [0, 1];
        let alias: &mut u64 = &mut local[0];
        alias = 2;
    }

    machine Main::indexed_local_member_after_index(&mut self) {
        let local: [Cell; 2] = [Cell { value: 0 }, Cell { value: 1 }];
        let alias: &mut u64 = &mut local[0].value;
        alias = 3;
    }

    machine reference_bearing_named_local_origin<'source>(source: &'source mut u64) {
        let local: BorrowCell<'source> = BorrowCell { value: source };
        let alias: &mut u64 = &mut local.value;
        alias = 3;
    }

    machine Main::indexed_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other;
        alias = 3;
    }

    machine overwrite_alias_binding(value: &mut u64) {
        value = 5;
    }

    machine return_alias(value: &mut u64) -> &mut u64 {
        value
    }

    machine write_argument(value: &mut u64) {
        value = 8;
    }

    machine return_local_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = &mut value;
        alias
    }

    machine return_projected_local_alias(cell: &mut Cell) -> &mut u64 {
        let alias: &mut Cell = &mut cell;
        &mut alias.value
    }

    machine return_call_initialized_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = return_alias(value);
        alias
    }

    machine return_call_initialized_projection(cell: &mut Cell) -> &mut u64 {
        let alias: &mut u64 = project_value(cell);
        alias
    }

    machine return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells
    }

    machine write_then_return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells[0] = 4;
        cells
    }

    machine return_cell_items(cells: &mut [Cell; 2]) -> &mut [Cell; 2] {
        cells
    }

    machine project_cell_value(cells: &mut [Cell; 2]) -> &mut u64 {
        &mut cells[0].value
    }

    machine project_value(cell: &mut Cell) -> &mut u64 {
        &mut cell.value
    }

    machine Main::return_attached_alias(&self, value: &mut u64) -> &mut u64 {
        value
    }

    machine Main::project_attached_value(&self, cell: &mut Cell) -> &mut u64 {
        &mut cell.value
    }

    machine Main::return_attached_receiver(&mut self) -> &mut u64 {
        &mut self.value
    }

    machine Main::return_attached_receiver_via_local_alias(&mut self) -> &mut u64 {
        let alias: &mut u64 = &mut self.value;
        alias
    }

    machine Main::write_then_return_attached_receiver(&mut self) -> &mut u64 {
        self.other = 4;
        &mut self.value
    }

    machine write_then_return(value: &mut u64) -> &mut u64 {
        value = 4;
        value
    }

    machine return_effectful_call_initialized_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = write_then_return(value);
        alias
    }

    machine return_recursive_alias(value: &mut u64) -> &mut u64 {
        let alias: &mut u64 = return_recursive_alias(value);
        alias
    }

    machine return_with_isolated_scratch(value: &mut u64) -> &mut u64 {
        let mut scratch: u64 = 0;
        scratch = 1;
        value
    }

    machine return_with_reference_scratch<'source>(
        value: &'source mut u64,
        other: &'source mut u64
    ) -> &'source mut u64 {
        let scratch: BorrowCell<'source> = BorrowCell { value: other };
        value
    }

    machine make_scratch() -> u64 {
        0
    }

    machine return_with_call_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = make_scratch();
        value
    }

    machine impure_scratch(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine mixed_scratch(first: &mut u64, second: &mut u64) -> u64 {
        first = 1;
        second = 2;
        0
    }

    machine scratch_from(value: u64) -> u64 {
        value
    }

    machine return_with_impure_call_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = impure_scratch(value);
        value
    }

    machine return_with_isolated_write_call_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = impure_scratch(&mut prior);
        value
    }

    machine return_with_mixed_write_call_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = mixed_scratch(&mut prior, value);
        value
    }

    machine return_after_isolated_write_statement_call(value: &mut u64) -> &mut u64 {
        let mut scratch: u64 = 0;
        impure_scratch(&mut scratch);
        value
    }

    machine return_after_mixed_write_statement_call(value: &mut u64) -> &mut u64 {
        let mut scratch: u64 = 0;
        mixed_scratch(&mut scratch, value);
        value
    }

    machine return_with_nested_call_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(make_scratch());
        value
    }

    machine return_after_pure_expression(value: &mut u64) -> &mut u64 {
        value == value;
        value
    }

    machine return_after_recast_write(value: &mut u64) -> &mut u64 {
        let view: &mut f64 = &mut value as &mut f64;
        view = 4.0;
        value
    }

    machine return_after_effectful_recast_write(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let view: &mut f64 = &mut cells[make_index()] as &mut f64;
        view = 4.0;
        cells
    }

    machine return_after_discarded_call(value: &mut u64) -> &mut u64 {
        _ = make_scratch();
        value
    }

    machine return_after_transparent_call_target_write(value: &mut u64) -> &mut u64 {
        write_then_return(value) = 4;
        value
    }

    machine return_after_opaque_call_target_write(value: &mut u64) -> &mut u64 {
        call_then_return(value) = 4;
        value
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine return_after_hidden_index_call_target_write(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells[make_index()] = 4;
        cells
    }

    machine return_mutable_local_alias(value: &mut u64) -> &mut u64 {
        let mut alias: &mut u64 = &mut value;
        alias
    }

    machine return_rebound_mutable_local_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let mut alias: &mut u64 = &mut first;
        alias = &mut second;
        alias
    }

    machine return_call_rebound_mutable_local_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let mut alias: &mut u64 = &mut first;
        alias = call_then_return(second);
        alias
    }

    machine return_rebound_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let alias: &mut u64 = &mut first;
        alias = &mut second;
        alias
    }

    machine return_pre_rebind_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'first mut u64 {
        let alias: &mut u64 = &mut first;
        let prior: &mut u64 = &mut alias;
        alias = &mut second;
        prior
    }

    machine return_call_rebound_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let alias: &mut u64 = &mut first;
        alias = return_alias(second);
        alias
    }

    machine call_then_return(value: &mut u64) -> &mut u64 {
        overwrite_alias_binding(&mut value);
        value
    }

    machine opaque_choose<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        overwrite_alias_binding(&mut first);
        second
    }

    machine return_escaping_call_rebound_alias<'first, 'second>(
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'second mut u64 {
        let alias: &mut u64 = &mut first;
        alias = call_then_return(second);
        alias
    }

    machine Main::call_rebound_alias(&mut self) {
        let alias: &mut u64 = &mut self.value;
        overwrite_alias_binding(&mut alias);
    }

    machine Main::call_escaped_alias_chain(&mut self) {
        let first: &mut u64 = &mut self.value;
        let second: &mut u64 = &mut first;
        overwrite_alias_binding(&mut second);
    }

    machine Main::call_escaped_indexed_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        overwrite_alias_binding(&mut alias);
    }

    machine Main::call_produced_alias_chain(&mut self) {
        let first: &mut u64 = return_alias(&mut self.value);
        let second: &mut u64 = &mut first;
        second = 3;
    }

    machine Main::recast_local_origin(&mut self) {
        let view: &mut f64 = &mut self.value as &mut f64;
        view = 3.0;
    }

    machine Main::effectful_index_recast_origin(&mut self) {
        let view: &mut f64 = &mut self.cells[make_index()] as &mut f64;
        view = 3.0;
    }

    machine Main::transparent_result_statement_argument(&mut self) {
        write_argument(return_alias(&mut self.value));
    }

    machine Main::opaque_result_statement_argument(&mut self) {
        write_argument(opaque_choose(&mut self.value, &mut self.other));
    }

    machine Main::effectful_index_statement_argument(&mut self) {
        write_argument(&mut self.cells[make_index()]);
    }

    machine Main::nested_call_produced_alias_chain(&mut self) {
        let first: &mut u64 = return_alias(return_alias(&mut self.value));
        let second: &mut u64 = &mut first;
        second = 3;
    }

    machine Main::local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_local_alias(&mut self.value);
        alias = 3;
    }

    machine Main::projected_local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_projected_local_alias(&mut self.cell);
        alias = 3;
    }

    machine Main::call_initialized_local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_call_initialized_alias(&mut self.value);
        alias = 3;
    }

    machine Main::call_initialized_projected_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_call_initialized_projection(&mut self.cell);
        alias = 3;
    }

    machine Main::effectful_call_initialized_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_effectful_call_initialized_alias(&mut self.value);
        alias = 3;
    }

    machine Main::recursive_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_recursive_alias(&mut self.value);
        alias = 3;
    }

    machine Main::isolated_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_isolated_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::reference_scratch_helper_result(&mut self) {
        let alias: &mut u64 =
            return_with_reference_scratch(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::impure_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_impure_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::isolated_write_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 =
            return_with_isolated_write_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::mixed_write_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_mixed_write_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::isolated_write_statement_call_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_isolated_write_statement_call(&mut self.value);
        alias = 3;
    }

    machine Main::mixed_write_statement_call_helper_result(&mut self) {
        let alias: &mut u64 = return_after_mixed_write_statement_call(&mut self.value);
        alias = 3;
    }

    machine Main::nested_call_scratch_helper_result(&mut self) {
        let alias: &mut u64 = return_with_nested_call_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::pure_expression_helper_result(&mut self) {
        let alias: &mut u64 = return_after_pure_expression(&mut self.value);
        alias = 3;
    }

    machine Main::recast_write_helper_result(&mut self) {
        let alias: &mut u64 = return_after_recast_write(&mut self.value);
        alias = 3;
    }

    machine Main::effectful_recast_write_helper_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_effectful_recast_write(&mut self.cells);
        alias[0] = 3;
    }

    machine Main::discarded_call_helper_result(&mut self) {
        let alias: &mut u64 = return_after_discarded_call(&mut self.value);
        alias = 3;
    }

    machine Main::transparent_call_target_write_helper_result(&mut self) {
        let alias: &mut u64 =
            return_after_transparent_call_target_write(&mut self.value);
        alias = 3;
    }

    machine Main::opaque_call_target_write_helper_result(&mut self) {
        let alias: &mut u64 = return_after_opaque_call_target_write(&mut self.value);
        alias = 3;
    }

    machine Main::hidden_index_call_target_write_helper_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_hidden_index_call_target_write(&mut self.cells);
        alias[0] = 3;
    }

    machine Main::mutable_local_alias_helper_result(&mut self) {
        let alias: &mut u64 = return_mutable_local_alias(&mut self.value);
        alias = 3;
    }

    machine Main::rebound_mutable_local_alias_helper_result(&mut self) {
        let alias: &mut u64 =
            return_rebound_mutable_local_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::call_rebound_mutable_local_alias_helper_result(&mut self) {
        let alias: &mut u64 =
            return_call_rebound_mutable_local_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::rebound_helper_result(&mut self) {
        let alias: &mut u64 = return_rebound_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::pre_rebind_helper_result(&mut self) {
        let alias: &mut u64 = return_pre_rebind_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::call_rebound_helper_result(&mut self) {
        let alias: &mut u64 = return_call_rebound_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::escaping_call_rebound_helper_result(&mut self) {
        let alias: &mut u64 =
            return_escaping_call_rebound_alias(&mut self.value, &mut self.other);
        alias = 3;
    }

    machine Main::escaping_call_then_result_alias(&mut self) {
        let alias: &mut u64 = call_then_return(&mut self.value);
        alias = 3;
    }

    machine Main::call_produced_indexed_alias(&mut self) {
        let cells: &mut [u64; 2] = return_cells(&mut self.cells);
        let alias: &mut u64 = &mut cells[0];
        alias = 3;
    }

    machine Main::call_produced_member_after_index_alias(&mut self) {
        let alias: &mut u64 = &mut return_cell_items(&mut self.cell_items)[0].value;
        alias = 3;
    }

    machine Main::projected_call_result_alias(&mut self) {
        let alias: &mut u64 = project_cell_value(&mut self.cell_items);
        alias = 3;
    }

    machine Main::exact_projected_call_result_alias(&mut self) {
        let alias: &mut u64 = project_value(&mut self.cell);
        alias = 3;
    }

    machine Main::attached_call_produced_alias(&mut self) {
        let alias: &mut u64 = self.return_attached_alias(&mut self.value);
        alias = 3;
    }

    machine Main::attached_projected_call_result_alias(&mut self) {
        let alias: &mut u64 = self.project_attached_value(&mut self.cell);
        alias = 3;
    }

    machine Main::attached_receiver_result_alias(&mut self) {
        let alias: &mut u64 = self.return_attached_receiver();
        alias = 3;
    }

    machine Main::attached_receiver_local_alias_result(&mut self) {
        let alias: &mut u64 = self.return_attached_receiver_via_local_alias();
        alias = 3;
    }

    machine Main::nontrivial_attached_receiver_result_alias(&mut self) {
        let alias: &mut u64 = self.write_then_return_attached_receiver();
        alias = 3;
    }

    machine Main::nontrivial_call_result_alias(&mut self) {
        let alias: &mut u64 = write_then_return(&mut self.value);
        alias = 3;
    }

    machine Main::nontrivial_call_rebound_alias(&mut self) {
        let alias: &mut u64 = &mut self.value;
        alias = write_then_return(&mut self.other);
        alias = 3;
    }

    machine Main::computed_local_collection_origin(&mut self) {
        let local: [u64; 2] = [0, 1];
        let values: &mut [u64; 2] = return_cells(&mut local);
        let alias: &mut u64 = &mut values[0];
        alias = 3;
    }

    machine Main::effectful_computed_local_collection_origin(&mut self) {
        let local: [u64; 2] = [0, 1];
        let values: &mut [u64; 2] = write_then_return_cells(&mut local);
        let alias: &mut u64 = &mut values[0];
        alias = 3;
    }

    machine Main::named_alias_cycle(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) {
            let first: &mut u64 = &mut self.value;
            let second: &mut u64 = &mut first;
            second = 4;
            transition { _ -> cycle() }
        }
    }

    machine Main::named_indexed_alias_cycle(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) {
            let alias: &mut u64 = &mut self.cells[0];
            alias = 4;
            transition { _ -> cycle() }
        }
    }

    machine Main::named_alias_multistate_cycle(&mut self) {
        let root: &mut u64 = &mut self.value;
        let alias: &mut u64 = &mut root;
        transition { _ -> first(alias) }
        state first(&mut self, value: &mut u64) {
            transition { _ -> second(value) }
        }
        state second(&mut self, value: &mut u64) {
            value = 5;
            transition { _ -> first(value) }
        }
    }

    machine Main::named_alias_downstream_cycle(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> prefix(alias) }
        state prefix(&mut self, value: &mut u64) {
            transition { _ -> cycle(value) }
        }
        state cycle(&mut self, value: &mut u64) {
            value = 6;
            transition { _ -> cycle(value) }
        }
    }

    machine Main::named_stable_rebound_alias_cycle(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) {
            let alias: &mut u64 = &mut self.value;
            alias = &mut self.other;
            alias = 7;
            transition { _ -> cycle() }
        }
    }

    machine parameter_alias_cycle(value: &mut u64) {
        transition { _ -> cycle(value) }
        state cycle(value: &mut u64) {
            let alias: &mut u64 = &mut value;
            alias = 7;
            transition { _ -> cycle(alias) }
        }
    }

    machine duplicate_parameter_alias_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            let alias: &mut u64 = &mut left;
            alias = 7;
            transition { _ -> cycle(alias, alias) }
        }
    }

    machine Main::named_alias_cross_state_local(&mut self) {
        let alias: &mut u64 = &mut self.value;
        transition { _ -> finish() }
        state finish(&mut self) {
            alias = 7;
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::local_origin",
        "Main::indexed_local_origin",
        "Main::constrained_local_origin",
        "Main::indexed_constrained_local_origin",
        "Main::indexed_local_member_after_index",
        "Main::computed_local_collection_origin",
        "Main::effectful_computed_local_collection_origin",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some([].as_slice()),
            "{name} writes only through a caller-isolated local origin"
        );
    }

    for (name, expected_path) in [
        ("Main::rebound_alias", "self.other"),
        ("Main::alias_chain_upstream_rebind", "self.value"),
        ("Main::alias_chain_leaf_rebind", "self.other"),
        ("Main::alias_chain_rebind_from_alias", "self.value"),
        ("Main::indexed_alias_rebind", "self.other"),
        ("Main::call_produced_alias_chain", "self.value"),
        ("Main::recast_local_origin", "self.value"),
        ("Main::transparent_result_statement_argument", "self.value"),
        ("Main::nested_call_produced_alias_chain", "self.value"),
        (
            "Main::effectful_call_initialized_alias_helper_result",
            "self.value",
        ),
        ("Main::nontrivial_call_result_alias", "self.value"),
        ("Main::nontrivial_call_rebound_alias", "self.other"),
        ("Main::isolated_scratch_helper_result", "self.value"),
        ("Main::call_scratch_helper_result", "self.value"),
        (
            "Main::isolated_write_call_scratch_helper_result",
            "self.value",
        ),
        ("Main::nested_call_scratch_helper_result", "self.value"),
        (
            "Main::isolated_write_statement_call_helper_result",
            "self.value",
        ),
        (
            "Main::mixed_write_statement_call_helper_result",
            "self.value",
        ),
        ("Main::pure_expression_helper_result", "self.value"),
        ("Main::recast_write_helper_result", "self.value"),
        (
            "Main::transparent_call_target_write_helper_result",
            "self.value",
        ),
        (
            "Main::hidden_index_call_target_write_helper_result",
            "self.cells",
        ),
        ("Main::mutable_local_alias_helper_result", "self.value"),
        (
            "Main::rebound_mutable_local_alias_helper_result",
            "self.other",
        ),
        ("Main::rebound_helper_result", "self.other"),
        ("Main::pre_rebind_helper_result", "self.value"),
        ("Main::call_rebound_helper_result", "self.other"),
        ("Main::local_alias_helper_result", "self.value"),
        (
            "Main::call_initialized_local_alias_helper_result",
            "self.value",
        ),
        (
            "Main::call_initialized_projected_alias_helper_result",
            "self.cell.value",
        ),
        (
            "Main::projected_local_alias_helper_result",
            "self.cell.value",
        ),
        ("Main::call_produced_indexed_alias", "self.cells"),
        (
            "Main::call_produced_member_after_index_alias",
            "self.cell_items",
        ),
        ("Main::projected_call_result_alias", "self.cell_items"),
        ("Main::exact_projected_call_result_alias", "self.cell.value"),
        ("Main::attached_call_produced_alias", "self.value"),
        (
            "Main::attached_projected_call_result_alias",
            "self.cell.value",
        ),
        ("Main::attached_receiver_result_alias", "self.value"),
        ("Main::attached_receiver_local_alias_result", "self.value"),
        ("Main::named_alias_cycle", "self.value"),
        ("Main::named_indexed_alias_cycle", "self.cells"),
        ("Main::named_alias_multistate_cycle", "self.value"),
        ("Main::named_alias_downstream_cycle", "self.value"),
        ("Main::named_stable_rebound_alias_cycle", "self.other"),
        ("parameter_alias_cycle", "$P0"),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some([expected_path.to_owned()].as_slice()),
            "{name} must substitute the transparent call result back to its argument origin"
        );
    }

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::opaque_result_statement_argument")
        .expect("opaque nested-result argument caller");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("opaque nested-result argument caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(machine, entry)
            .complete_paths(),
        Some(["self".to_owned(), "self.value".to_owned()].as_slice()),
        "an opaque nested result must retain its conservative whole-receiver fence"
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::effectful_index_statement_argument")
        .expect("effectful nested index argument caller");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("effectful nested index argument caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(machine, entry)
            .complete_paths(),
        Some(["self.cells".to_owned()].as_slice()),
        "a bounded complete index call must coarsen the written argument to its collection"
    );

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::nontrivial_attached_receiver_result_alias")
        .expect("attached value-write helper caller");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("attached value-write helper caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(machine, entry)
            .complete_paths(),
        Some(["self.other".to_owned(), "self.value".to_owned()].as_slice()),
        "the attached helper's own write and returned-alias write must both remain exact"
    );

    for name in [
        "reference_bearing_named_local_origin",
        "Main::call_rebound_alias",
        "Main::call_escaped_alias_chain",
        "Main::call_escaped_indexed_alias",
        "Main::effectful_index_recast_origin",
        "Main::recursive_alias_helper_result",
        "Main::reference_scratch_helper_result",
        "Main::impure_call_scratch_helper_result",
        "Main::mixed_write_call_scratch_helper_result",
        "Main::discarded_call_helper_result",
        "Main::effectful_recast_write_helper_result",
        "Main::opaque_call_target_write_helper_result",
        "Main::call_rebound_mutable_local_alias_helper_result",
        "Main::escaping_call_rebound_helper_result",
        "Main::escaping_call_then_result_alias",
        "duplicate_parameter_alias_cycle",
        "Main::named_alias_cross_state_local",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque without one stable representable local origin"
        );
    }
}

#[test]
fn transparent_returned_index_frame_accepts_a_bounded_exact_call_tree() {
    let source = r#"
    data Main {
        value: u64;
        other_value: u64;
        cells: [u64; 2];
        matrix: [[u64; 2]; 2];
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 1;
        0
    }

    machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {
        index
    }

    machine recursive_index() -> u64 [0..=1] {
        recursive_index()
    }

    machine return_local_index(cells: &mut [u64; 2]) -> &mut u64 {
        let index: u64 = 0;
        &mut cells[index]
    }

    machine return_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[make_index()]
    }

    machine return_write_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        &mut cells[write_index(value)]
    }

    machine return_nested_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[identity_index(make_index())]
    }

    machine return_nested_write_call_index<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut u64 {
        &mut cells[identity_index(write_index(value))]
    }

    machine return_deep_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[identity_index(identity_index(make_index()))]
    }

    machine return_recursive_call_index(cells: &mut [u64; 2]) -> &mut u64 {
        &mut cells[recursive_index()]
    }

    machine return_repeated_call_index<'matrix, 'first, 'second>(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut u64 {
        &mut matrix[write_index(first)][write_index(second)]
    }

    machine return_deep_repeated_call_index(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut u64 {
        &mut matrix[identity_index(identity_index(make_index()))][make_index()]
    }

    machine Main::local_index_result(&mut self) {
        let alias: &mut u64 = return_local_index(&mut self.cells);
        alias = 1;
    }

    machine Main::call_index_result(&mut self) {
        let alias: &mut u64 = return_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::write_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_write_call_index(&mut self.cells, &mut self.value);
        alias = 1;
    }

    machine Main::nested_call_index_result(&mut self) {
        let alias: &mut u64 = return_nested_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::nested_write_call_index_result(&mut self) {
        let alias: &mut u64 =
            return_nested_write_call_index(&mut self.cells, &mut self.value);
        alias = 1;
    }

    machine Main::deep_call_index_result(&mut self) {
        let alias: &mut u64 = return_deep_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::recursive_call_index_result(&mut self) {
        let alias: &mut u64 = return_recursive_call_index(&mut self.cells);
        alias = 1;
    }

    machine Main::repeated_call_index_result(&mut self) {
        let alias: &mut u64 = return_repeated_call_index(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias = 1;
    }

    machine Main::deep_repeated_call_index_result(&mut self) {
        let alias: &mut u64 = return_deep_repeated_call_index(&mut self.matrix);
        alias = 1;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    let local = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::local_index_result")
        .expect("local-index helper caller");
    let local_entry = typed
        .machine_states(local)
        .first()
        .expect("local-index helper caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(local, local_entry)
            .complete_paths(),
        Some(["self.cells".to_owned()].as_slice()),
        "an effect-free local index preserves the returned collection origin"
    );

    for (name, expected_paths) in [
        ("Main::call_index_result", vec!["self.cells"]),
        (
            "Main::write_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        ("Main::nested_call_index_result", vec!["self.cells"]),
        (
            "Main::nested_write_call_index_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::repeated_call_index_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} helper caller"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} helper caller entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must publish the index call's writes and preserve the coarse collection origin"
        );
    }

    for name in [
        "Main::deep_call_index_result",
        "Main::deep_repeated_call_index_result",
        "Main::recursive_call_index_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} helper caller"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} helper caller entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque outside the bounded depth-two index rung"
        );
    }
}

#[test]
fn stable_alias_index_frame_accepts_a_bounded_exact_call_tree() {
    let source = r#"
    data Main {
        value: u64;
        other_value: u64;
        cells: [u64; 2];
        other_cells: [u64; 2];
        matrix: [[u64; 2]; 2];
        other_matrix: [[u64; 2]; 2];
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 1;
        0
    }

    machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {
        index
    }

    machine recursive_index() -> u64 [0..=1] {
        recursive_index()
    }

    machine Main::local_index_alias(&mut self) {
        let index: u64 = 0;
        let alias: &mut u64 = &mut self.cells[index];
        alias = 1;
    }

    machine Main::call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[make_index()];
        alias = 1;
    }

    machine Main::write_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[write_index(&mut self.value)];
        alias = 1;
    }

    machine Main::nested_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[identity_index(make_index())];
        alias = 1;
    }

    machine Main::nested_write_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.cells[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::deep_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.cells[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::recursive_call_index_alias(&mut self) {
        let alias: &mut u64 = &mut self.cells[recursive_index()];
        alias = 1;
    }

    machine Main::repeated_call_index_alias(&mut self) {
        let alias: &mut u64 =
            &mut self.matrix[write_index(&mut self.value)][
                write_index(&mut self.other_value)
            ];
        alias = 1;
    }

    machine Main::call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[make_index()];
        alias = 1;
    }

    machine Main::write_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[write_index(&mut self.value)];
        alias = 1;
    }

    machine Main::prior_alias_survives_call_index_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        let prior: &mut u64 = &mut alias;
        alias = &mut self.other_cells[make_index()];
        prior = 1;
        alias = 2;
    }

    machine Main::nested_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[identity_index(make_index())];
        alias = 1;
    }

    machine Main::nested_write_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias =
            &mut self.other_cells[identity_index(write_index(&mut self.value))];
        alias = 1;
    }

    machine Main::deep_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias =
            &mut self.other_cells[identity_index(identity_index(make_index()))];
        alias = 1;
    }

    machine Main::binding_reborrow_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[identity_index(write_index(&mut alias))];
        alias = 1;
    }

    machine Main::recursive_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_cells[recursive_index()];
        alias = 1;
    }

    machine Main::repeated_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_matrix[write_index(&mut self.value)][
            write_index(&mut self.other_value)
        ];
        alias = 1;
    }

    machine Main::deep_repeated_call_index_alias_rebind(&mut self) {
        let alias: &mut u64 = &mut self.cells[0];
        alias = &mut self.other_matrix[
            identity_index(identity_index(make_index()))
        ][make_index()];
        alias = 1;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    let local = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::local_index_alias")
        .expect("local-index alias machine");
    let local_entry = typed
        .machine_states(local)
        .first()
        .expect("local-index alias entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(local, local_entry)
            .complete_paths(),
        Some(["self.cells".to_owned()].as_slice()),
        "an effect-free local index preserves the alias's collection origin"
    );

    for (name, expected_paths) in [
        ("Main::call_index_alias", vec!["self.cells"]),
        (
            "Main::write_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        ("Main::nested_call_index_alias", vec!["self.cells"]),
        (
            "Main::nested_write_call_index_alias",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::repeated_call_index_alias",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must publish the index call's writes and retain the coarse collection origin"
        );
    }

    for name in [
        "Main::deep_call_index_alias",
        "Main::recursive_call_index_alias",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque outside the bounded depth-two alias-index rung"
        );
    }

    for (name, expected_paths) in [
        ("Main::call_index_alias_rebind", vec!["self.other_cells"]),
        (
            "Main::write_call_index_alias_rebind",
            vec!["self.other_cells", "self.value"],
        ),
        (
            "Main::prior_alias_survives_call_index_rebind",
            vec!["self.cells", "self.other_cells"],
        ),
        (
            "Main::nested_call_index_alias_rebind",
            vec!["self.other_cells"],
        ),
        (
            "Main::nested_write_call_index_alias_rebind",
            vec!["self.other_cells", "self.value"],
        ),
        (
            "Main::repeated_call_index_alias_rebind",
            vec!["self.other_matrix", "self.other_value", "self.value"],
        ),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must move only the rebound alias to the direct-call indexed origin"
        );
    }

    for name in [
        "Main::deep_call_index_alias_rebind",
        "Main::binding_reborrow_call_index_alias_rebind",
        "Main::recursive_call_index_alias_rebind",
        "Main::deep_repeated_call_index_alias_rebind",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque outside the bounded depth-two alias-rebind rung"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_indexed_target_calls() {
    let source = r#"
    data Bucket {
        cells: [u64; 2];
    }

    data Cell {
        value: u64;
    }

    data CellBucket {
        cells: [Cell; 2];
    }

    data GridBucket {
        rows: [[u64; 2]; 2];
    }

    data Main {
        value: u64;
        other_value: u64;
        result: u64;
        cells: [u64; 2];
        matrix: [[u64; 2]; 2];
        bucket: Bucket;
        cell_bucket: CellBucket;
        grid_bucket: GridBucket;
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 1;
        0
    }

    machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {
        index
    }

    machine recursive_index() -> u64 [0..=1] {
        recursive_index()
    }

    machine return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells
    }

    machine recursive_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        recursive_cells(cells)
    }

    machine return_bucket(bucket: &mut Bucket) -> &mut Bucket {
        bucket
    }

    machine recursive_bucket(bucket: &mut Bucket) -> &mut Bucket {
        recursive_bucket(bucket)
    }

    machine return_cell_bucket(bucket: &mut CellBucket) -> &mut CellBucket {
        bucket
    }

    machine recursive_cell_bucket(bucket: &mut CellBucket) -> &mut CellBucket {
        recursive_cell_bucket(bucket)
    }

    machine return_grid_bucket(bucket: &mut GridBucket) -> &mut GridBucket {
        bucket
    }

    machine recursive_grid_bucket(bucket: &mut GridBucket) -> &mut GridBucket {
        recursive_grid_bucket(bucket)
    }

    machine Main::return_attached_cells(&mut self) -> &mut [u64; 2] {
        &mut self.cells
    }

    machine Main::recursive_attached_cells(&mut self) -> &mut [u64; 2] {
        self.recursive_attached_cells()
    }

    machine return_after_index_target(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells[make_index()] = 1;
        cells
    }

    machine return_after_nested_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_alias_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_helper_result_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        return_cells(cells)[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_slice_view_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_deep_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        cells
    }

    machine return_after_recursive_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells.as_mut_slice()[recursive_index()] = 1;
        cells
    }

    machine return_after_alias_slice_view_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias.as_mut_slice()[identity_index(write_index(value))] = 1;
        cells
    }

    machine return_after_deep_alias_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        cells
    }

    machine return_after_recursive_alias_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias.as_mut_slice()[recursive_index()] = 1;
        cells
    }

    machine return_after_member_alias_slice_view_index_target<
        'bucket, 'result, 'value
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        alias.as_mut_slice()[identity_index(write_index(value))] = 1;
        result
    }

    machine return_after_deep_member_alias_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        alias.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        result
    }

    machine return_after_recursive_member_alias_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = &mut bucket.cells;
        alias.as_mut_slice()[recursive_index()] = 1;
        result
    }

    machine return_after_slice_view_repeated_index_target<
        'matrix, 'first, 'second
    >(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut [[u64; 2]; 2] {
        matrix.as_mut_slice()[write_index(first)][write_index(second)] = 1;
        matrix
    }

    machine return_after_deep_slice_view_repeated_index_target(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut [[u64; 2]; 2] {
        matrix.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ][make_index()] = 1;
        matrix
    }

    machine return_after_recursive_slice_view_repeated_index_target(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut [[u64; 2]; 2] {
        matrix.as_mut_slice()[recursive_index()][make_index()] = 1;
        matrix
    }

    machine return_after_helper_slice_view_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        return_cells(cells).as_mut_slice()[
            identity_index(write_index(value))
        ] = 1;
        cells
    }

    machine return_after_deep_helper_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        return_cells(cells).as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        cells
    }

    machine return_after_recursive_helper_slice_view_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        recursive_cells(cells).as_mut_slice()[make_index()] = 1;
        cells
    }

    machine return_after_recursive_helper_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        recursive_cells(cells)[make_index()] = 1;
        cells
    }

    machine return_after_projected_helper_index_target<'bucket, 'result, 'value>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells[
            identity_index(write_index(value))
        ] = 1;
        result
    }

    machine return_after_deep_projected_helper_index_target<'bucket, 'result>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells[
            identity_index(identity_index(make_index()))
        ] = 1;
        result
    }

    machine return_after_recursive_projected_helper_index_target<'bucket, 'result>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_bucket(bucket).cells[make_index()] = 1;
        result
    }

    machine return_after_projected_helper_slice_view_index_target<
        'bucket, 'result, 'value
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells.as_mut_slice()[
            identity_index(write_index(value))
        ] = 1;
        result
    }

    machine return_after_deep_projected_helper_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_bucket(bucket).cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        result
    }

    machine return_after_recursive_projected_helper_slice_view_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_bucket(bucket).cells.as_mut_slice()[make_index()] = 1;
        result
    }

    machine return_after_slice_view_member_after_index_target<
        'bucket, 'result, 'value
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells.as_mut_slice()[
            identity_index(write_index(value))
        ].value = 1;
        result
    }

    machine return_after_deep_slice_view_member_after_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells.as_mut_slice()[
            identity_index(identity_index(make_index()))
        ].value = 1;
        result
    }

    machine return_after_recursive_slice_view_member_after_index_target<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_cell_bucket(bucket).cells.as_mut_slice()[make_index()].value = 1;
        result
    }

    machine return_after_member_after_index_target<'bucket, 'result, 'value>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        value: &'value mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells[
            identity_index(write_index(value))
        ].value = 1;
        result
    }

    machine return_after_deep_member_after_index_target<'bucket, 'result>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_cell_bucket(bucket).cells[
            identity_index(identity_index(make_index()))
        ].value = 1;
        result
    }

    machine return_after_recursive_member_after_index_target<'bucket, 'result>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_cell_bucket(bucket).cells[make_index()].value = 1;
        result
    }

    machine return_after_projected_repeated_index_target<
        'bucket, 'result, 'first, 'second
    >(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'result mut u64 {
        return_grid_bucket(bucket).rows[
            write_index(first)
        ][write_index(second)] = 1;
        result
    }

    machine return_after_deep_projected_repeated_index_target<'bucket, 'result>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        return_grid_bucket(bucket).rows[
            identity_index(identity_index(make_index()))
        ][make_index()] = 1;
        result
    }

    machine return_after_recursive_projected_repeated_index_target<'bucket, 'result>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        recursive_grid_bucket(bucket).rows[make_index()][make_index()] = 1;
        result
    }

    machine Main::return_after_attached_helper_index_target(
        &mut self
    ) -> &mut u64 {
        self.return_attached_cells()[
            identity_index(write_index(&mut self.value))
        ] = 1;
        &mut self.result
    }

    machine Main::return_after_recursive_attached_index_target(
        &mut self
    ) -> &mut u64 {
        self.recursive_attached_cells()[make_index()] = 1;
        &mut self.result
    }

    machine Main::return_after_attached_slice_view_index_target(
        &mut self
    ) -> &mut u64 {
        self.return_attached_cells().as_mut_slice()[
            identity_index(write_index(&mut self.value))
        ] = 1;
        &mut self.result
    }

    machine Main::return_after_deep_attached_slice_view_index_target(
        &mut self
    ) -> &mut u64 {
        self.return_attached_cells().as_mut_slice()[
            identity_index(identity_index(make_index()))
        ] = 1;
        &mut self.result
    }

    machine Main::return_after_recursive_attached_slice_view_index_target(
        &mut self
    ) -> &mut u64 {
        self.recursive_attached_cells().as_mut_slice()[make_index()] = 1;
        &mut self.result
    }

    machine return_after_deep_index_target(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells[identity_index(identity_index(make_index()))] = 1;
        cells
    }

    machine return_after_deep_alias_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        let alias: &mut [u64; 2] = cells;
        alias[identity_index(identity_index(make_index()))] = 1;
        cells
    }

    machine return_after_binding_reborrow_index_target<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(&mut value))] = 1;
        cells
    }

    machine return_after_recursive_index_target(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells[recursive_index()] = 1;
        cells
    }

    machine return_after_repeated_index_target<'matrix, 'first, 'second>(
        matrix: &'matrix mut [[u64; 2]; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'matrix mut [[u64; 2]; 2] {
        matrix[write_index(first)][write_index(second)] = 1;
        matrix
    }

    machine return_after_deep_repeated_index_target(
        matrix: &mut [[u64; 2]; 2]
    ) -> &mut [[u64; 2]; 2] {
        matrix[identity_index(identity_index(make_index()))][make_index()] = 1;
        matrix
    }

    machine Main::index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::nested_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_nested_index_target(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::alias_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_alias_index_target(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::helper_result_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_helper_result_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_slice_view_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deep_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::alias_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_alias_slice_view_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deep_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_alias_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_alias_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::member_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_member_alias_slice_view_index_target(
            &mut self.bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_member_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_member_alias_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_member_alias_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_member_alias_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::slice_view_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] = return_after_slice_view_repeated_index_target(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias[0][0] = 2;
    }

    machine Main::deep_slice_view_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] =
            return_after_deep_slice_view_repeated_index_target(&mut self.matrix);
        alias[0][0] = 2;
    }

    machine Main::recursive_slice_view_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] =
            return_after_recursive_slice_view_repeated_index_target(&mut self.matrix);
        alias[0][0] = 2;
    }

    machine Main::helper_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_helper_slice_view_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::deep_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_helper_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_helper_slice_view_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::recursive_helper_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_helper_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::projected_helper_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_index_target(
            &mut self.bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_projected_helper_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_helper_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_projected_helper_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_helper_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::projected_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_slice_view_index_target(
            &mut self.bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_projected_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_helper_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_projected_helper_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_helper_slice_view_index_target(
            &mut self.bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::slice_view_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_slice_view_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_slice_view_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_slice_view_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_slice_view_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_slice_view_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.value
        );
        alias = 2;
    }

    machine Main::deep_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_member_after_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_member_after_index_target(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::projected_repeated_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_projected_repeated_index_target(
            &mut self.grid_bucket,
            &mut self.result,
            &mut self.value,
            &mut self.other_value
        );
        alias = 2;
    }

    machine Main::deep_projected_repeated_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_repeated_index_target(
            &mut self.grid_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::recursive_projected_repeated_index_target_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_repeated_index_target(
            &mut self.grid_bucket,
            &mut self.result
        );
        alias = 2;
    }

    machine Main::attached_helper_index_target_result(&mut self) {
        let alias: &mut u64 = self.return_after_attached_helper_index_target();
        alias = 2;
    }

    machine Main::recursive_attached_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_index_target();
        alias = 2;
    }

    machine Main::attached_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_attached_slice_view_index_target();
        alias = 2;
    }

    machine Main::deep_attached_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_deep_attached_slice_view_index_target();
        alias = 2;
    }

    machine Main::recursive_attached_slice_view_index_target_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_slice_view_index_target();
        alias = 2;
    }

    machine Main::deep_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::deep_alias_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_alias_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::binding_reborrow_index_target_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_binding_reborrow_index_target(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::recursive_index_target_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_index_target(&mut self.cells);
        alias[0] = 2;
    }

    machine Main::repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] = return_after_repeated_index_target(
            &mut self.matrix,
            &mut self.value,
            &mut self.other_value
        );
        alias[0][0] = 2;
    }

    machine Main::deep_repeated_index_target_result(&mut self) {
        let alias: &mut [[u64; 2]; 2] =
            return_after_deep_repeated_index_target(&mut self.matrix);
        alias[0][0] = 2;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for (name, expected_paths) in [
        ("Main::index_target_result", vec!["self.cells"]),
        (
            "Main::nested_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::alias_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::helper_result_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::slice_view_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::alias_slice_view_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::member_alias_slice_view_index_target_result",
            vec!["self.bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::helper_slice_view_index_target_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::projected_helper_index_target_result",
            vec!["self.bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::projected_helper_slice_view_index_target_result",
            vec!["self.bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::slice_view_member_after_index_target_result",
            vec!["self.cell_bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::member_after_index_target_result",
            vec!["self.cell_bucket.cells", "self.result", "self.value"],
        ),
        (
            "Main::projected_repeated_index_target_result",
            vec![
                "self.grid_bucket.rows",
                "self.other_value",
                "self.result",
                "self.value",
            ],
        ),
        (
            "Main::slice_view_repeated_index_target_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
        (
            "Main::attached_helper_index_target_result",
            vec!["self.cells", "self.result", "self.value"],
        ),
        (
            "Main::attached_slice_view_index_target_result",
            vec!["self.cells", "self.result", "self.value"],
        ),
        (
            "Main::repeated_index_target_result",
            vec!["self.matrix", "self.other_value", "self.value"],
        ),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must preserve the returned collection and publish index-call writes"
        );
    }

    for name in [
        "Main::deep_index_target_result",
        "Main::deep_alias_index_target_result",
        "Main::binding_reborrow_index_target_result",
        "Main::recursive_index_target_result",
        "Main::recursive_helper_index_target_result",
        "Main::deep_slice_view_index_target_result",
        "Main::recursive_slice_view_index_target_result",
        "Main::deep_alias_slice_view_index_target_result",
        "Main::recursive_alias_slice_view_index_target_result",
        "Main::deep_member_alias_slice_view_index_target_result",
        "Main::recursive_member_alias_slice_view_index_target_result",
        "Main::deep_helper_slice_view_index_target_result",
        "Main::recursive_helper_slice_view_index_target_result",
        "Main::deep_projected_helper_index_target_result",
        "Main::recursive_projected_helper_index_target_result",
        "Main::deep_projected_helper_slice_view_index_target_result",
        "Main::recursive_projected_helper_slice_view_index_target_result",
        "Main::deep_slice_view_member_after_index_target_result",
        "Main::recursive_slice_view_member_after_index_target_result",
        "Main::deep_member_after_index_target_result",
        "Main::recursive_member_after_index_target_result",
        "Main::deep_projected_repeated_index_target_result",
        "Main::recursive_projected_repeated_index_target_result",
        "Main::deep_slice_view_repeated_index_target_result",
        "Main::recursive_slice_view_repeated_index_target_result",
        "Main::recursive_attached_index_target_result",
        "Main::deep_attached_slice_view_index_target_result",
        "Main::recursive_attached_slice_view_index_target_result",
        "Main::deep_repeated_index_target_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque outside the bounded indexed-target rung"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_value_call_assignments() {
    let source = r#"
    data Main {
        value: u64;
        other: u64;
        cells: [u64; 2];
    }

    machine compute(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine identity(value: u64) -> u64 {
        value
    }

    machine recursive_value() -> u64 {
        recursive_value()
    }

    machine combine(first: u64, second: u64) -> u64 {
        first + second
    }

    machine recursive_value() -> u64 {
        recursive_value()
    }

    machine return_after_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = compute(value);
        cells
    }

    machine return_after_nested_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(compute(value));
        cells
    }

    machine return_after_sibling_value_calls<'cells, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = combine(compute(first), compute(second));
        cells
    }

    machine return_after_deep_sibling_value_call<'cells, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = combine(identity(compute(first)), compute(second));
        cells
    }

    machine return_after_reborrow_sibling_value_call<'cells, 'first, 'second>(
        cells: &'cells mut [u64; 2],
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = combine(compute(first), compute(&mut second));
        cells
    }

    machine return_after_deep_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(identity(compute(value)));
        cells
    }

    machine return_after_binding_reborrow_value_call<'cells, 'value>(
        cells: &'cells mut [u64; 2],
        value: &'value mut u64
    ) -> &'cells mut [u64; 2] {
        cells[0] = identity(compute(&mut value));
        cells
    }

    machine return_after_recursive_value_call(
        cells: &mut [u64; 2]
    ) -> &mut [u64; 2] {
        cells[0] = recursive_value();
        cells
    }

    machine Main::value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::nested_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_nested_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::sibling_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_sibling_value_calls(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::deep_sibling_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_sibling_value_call(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::reborrow_sibling_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reborrow_sibling_value_call(
            &mut self.cells,
            &mut self.value,
            &mut self.other
        );
        alias[0] = 2;
    }

    machine Main::deep_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_deep_value_call(&mut self.cells, &mut self.value);
        alias[0] = 2;
    }

    machine Main::binding_reborrow_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_binding_reborrow_value_call(
            &mut self.cells,
            &mut self.value
        );
        alias[0] = 2;
    }

    machine Main::recursive_value_call_assignment_result(&mut self) {
        let alias: &mut [u64; 2] =
            return_after_recursive_value_call(&mut self.cells);
        alias[0] = 2;
    }

    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for (name, expected_paths) in [
        (
            "Main::value_call_assignment_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::nested_value_call_assignment_result",
            vec!["self.cells", "self.value"],
        ),
        (
            "Main::sibling_value_call_assignment_result",
            vec!["self.cells", "self.other", "self.value"],
        ),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must distinguish value-call writes from reference rebinding"
        );
    }

    for name in [
        "Main::deep_value_call_assignment_result",
        "Main::deep_sibling_value_call_assignment_result",
        "Main::reborrow_sibling_value_call_assignment_result",
        "Main::binding_reborrow_value_call_assignment_result",
        "Main::recursive_value_call_assignment_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque outside the bounded value-call assignment rung"
        );
    }
}

#[test]
fn transparent_returned_place_composes_bounded_assignment_call_trees() {
    let source = r#"
    data Main {
        target_value: u64;
        source_value: u64;
        cells: [u64; 2];
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 1;
        0
    }

    machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {
        index
    }

    machine compute(value: &mut u64) -> u64 {
        value = 2;
        0
    }

    machine identity(value: u64) -> u64 {
        value
    }

    machine return_after_composed_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(target_value))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_slice_view_composed_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[identity_index(write_index(target_value))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_deep_slice_view_target_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[
            identity_index(identity_index(write_index(target_value)))
        ] = identity(compute(source_value));
        cells
    }

    machine return_after_recursive_slice_view_value_assignment<'cells, 'target>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64
    ) -> &'cells mut [u64; 2] {
        cells.as_mut_slice()[identity_index(write_index(target_value))] =
            recursive_value();
        cells
    }

    machine return_after_deep_target_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(identity_index(write_index(target_value)))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_deep_value_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(target_value))] =
            identity(identity(compute(source_value)));
        cells
    }

    machine return_after_reborrow_target_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(&mut target_value))] =
            identity(compute(source_value));
        cells
    }

    machine return_after_reborrow_value_assignment<'cells, 'target, 'source>(
        cells: &'cells mut [u64; 2],
        target_value: &'target mut u64,
        source_value: &'source mut u64
    ) -> &'cells mut [u64; 2] {
        cells[identity_index(write_index(target_value))] =
            identity(compute(&mut source_value));
        cells
    }

    machine Main::composed_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_composed_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::slice_view_composed_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_slice_view_composed_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::deep_slice_view_target_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_slice_view_target_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::recursive_slice_view_value_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_recursive_slice_view_value_assignment(
            &mut self.cells,
            &mut self.target_value
        );
        alias[0] = 3;
    }

    machine Main::deep_target_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_target_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::deep_value_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_deep_value_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::reborrow_target_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reborrow_target_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }

    machine Main::reborrow_value_assignment_result(&mut self) {
        let alias: &mut [u64; 2] = return_after_reborrow_value_assignment(
            &mut self.cells,
            &mut self.target_value,
            &mut self.source_value
        );
        alias[0] = 3;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::composed_assignment_result",
        "Main::slice_view_composed_assignment_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                ["self.cells", "self.source_value", "self.target_value"]
                    .map(str::to_owned)
                    .as_slice()
            ),
            "{name} target and value call trees must independently publish their writes"
        );
    }

    for name in [
        "Main::deep_target_assignment_result",
        "Main::deep_value_assignment_result",
        "Main::reborrow_target_assignment_result",
        "Main::reborrow_value_assignment_result",
        "Main::deep_slice_view_target_assignment_result",
        "Main::recursive_slice_view_value_assignment_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque when either assignment side exceeds its rail"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_indexed_statement_arguments() {
    let source = r#"
    data Bucket {
        cells: [u64; 2];
    }

    data Cell {
        value: u64;
    }

    data CellBucket {
        cells: [Cell; 2];
    }

    data GridBucket {
        rows: [[u64; 2]; 2];
    }

    data Main {
        result: u64;
        index_write: u64;
        second_index_write: u64;
        cells: [u64; 2];
        bucket: Bucket;
        cell_bucket: CellBucket;
        grid_bucket: GridBucket;
    }

    machine write_argument(value: &mut u64) {
        value = 1;
    }

    machine make_index() -> u64 [0..=1] {
        0
    }

    machine write_index(value: &mut u64) -> u64 [0..=1] {
        value = 2;
        0
    }

    machine identity_index(index: u64 [0..=1]) -> u64 [0..=1] {
        index
    }

    machine recursive_index() -> u64 [0..=1] {
        recursive_index()
    }

    machine return_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        cells
    }

    machine recursive_cells(cells: &mut [u64; 2]) -> &mut [u64; 2] {
        recursive_cells(cells)
    }

    machine return_bucket(bucket: &mut Bucket) -> &mut Bucket {
        bucket
    }

    machine recursive_bucket(bucket: &mut Bucket) -> &mut Bucket {
        recursive_bucket(bucket)
    }

    machine return_cell_bucket(bucket: &mut CellBucket) -> &mut CellBucket {
        bucket
    }

    machine recursive_cell_bucket(
        bucket: &mut CellBucket
    ) -> &mut CellBucket {
        recursive_cell_bucket(bucket)
    }

    machine return_grid_bucket(bucket: &mut GridBucket) -> &mut GridBucket {
        bucket
    }

    machine Main::return_attached_cells(&mut self) -> &mut [u64; 2] {
        &mut self.cells
    }

    machine Main::recursive_attached_cells(&mut self) -> &mut [u64; 2] {
        self.recursive_attached_cells()
    }

    machine return_after_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells[make_index()]);
        result
    }

    machine return_after_nested_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells[identity_index(write_index(index_write))]);
        result
    }

    machine return_after_slice_view_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells.as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_slice_view_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_slice_view_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells.as_mut_slice()[recursive_index()]);
        result
    }

    machine return_after_alias_slice_view_indexed_statement<
        'cells, 'result, 'write
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias.as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_alias_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_alias_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(&mut alias.as_mut_slice()[recursive_index()]);
        result
    }

    machine return_after_helper_slice_view_indexed_statement<
        'cells, 'result, 'write
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cells(cells).as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_helper_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cells(cells).as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_helper_slice_view_indexed_statement<
        'cells, 'result
    >(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_cells(cells).as_mut_slice()[make_index()]
        );
        result
    }

    machine return_after_alias_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias[identity_index(write_index(index_write))]
        );
        result
    }

    machine return_after_helper_result_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cells(cells)[identity_index(write_index(index_write))]
        );
        result
    }

    machine return_after_recursive_helper_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut recursive_cells(cells)[make_index()]);
        result
    }

    machine return_after_projected_helper_indexed_statement<'bucket, 'result, 'write>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_bucket(bucket).cells[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_projected_helper_slice_view_indexed_statement<
        'bucket, 'result, 'write
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_bucket(bucket).cells.as_mut_slice()[
                identity_index(write_index(index_write))
            ]
        );
        result
    }

    machine return_after_deep_projected_helper_slice_view_indexed_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_bucket(bucket).cells.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        result
    }

    machine return_after_recursive_projected_helper_slice_view_indexed_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_bucket(bucket).cells.as_mut_slice()[make_index()]
        );
        result
    }

    machine return_after_slice_view_member_after_index_statement<
        'bucket, 'result, 'write
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cell_bucket(bucket).cells.as_mut_slice()[
                identity_index(write_index(index_write))
            ].value
        );
        result
    }

    machine return_after_deep_slice_view_member_after_index_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cell_bucket(bucket).cells.as_mut_slice()[
                identity_index(identity_index(make_index()))
            ].value
        );
        result
    }

    machine return_after_recursive_slice_view_member_after_index_statement<
        'bucket, 'result
    >(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_cell_bucket(bucket).cells.as_mut_slice()[make_index()].value
        );
        result
    }

    machine return_after_recursive_projected_helper_statement<'bucket, 'result>(
        bucket: &'bucket mut Bucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut recursive_bucket(bucket).cells[make_index()]);
        result
    }

    machine return_after_member_after_index_statement<'bucket, 'result, 'write>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_cell_bucket(bucket).cells[
                identity_index(write_index(index_write))
            ].value
        );
        result
    }

    machine return_after_recursive_member_after_index_statement<'bucket, 'result>(
        bucket: &'bucket mut CellBucket,
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut recursive_cell_bucket(bucket).cells[make_index()].value
        );
        result
    }

    machine return_after_repeated_index_statement<'bucket, 'result, 'first, 'second>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64,
        first: &'first mut u64,
        second: &'second mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_grid_bucket(bucket).rows[
                identity_index(write_index(first))
            ][identity_index(write_index(second))]
        );
        result
    }

    machine return_after_deep_repeated_index_statement<'bucket, 'result, 'first>(
        bucket: &'bucket mut GridBucket,
        result: &'result mut u64,
        first: &'first mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut return_grid_bucket(bucket).rows[
                identity_index(identity_index(write_index(first)))
            ][make_index()]
        );
        result
    }

    machine Main::return_after_attached_result_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_cells()[
                identity_index(write_index(&mut self.index_write))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_recursive_attached_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(&mut self.recursive_attached_cells()[make_index()]);
        &mut self.result
    }

    machine Main::return_after_attached_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_cells().as_mut_slice()[
                identity_index(write_index(&mut self.index_write))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_deep_attached_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.return_attached_cells().as_mut_slice()[
                identity_index(identity_index(make_index()))
            ]
        );
        &mut self.result
    }

    machine Main::return_after_recursive_attached_slice_view_indexed_statement(
        &mut self
    ) -> &mut u64 {
        write_argument(
            &mut self.recursive_attached_cells().as_mut_slice()[make_index()]
        );
        &mut self.result
    }

    machine return_after_deep_alias_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        let alias: &mut [u64; 2] = cells;
        write_argument(
            &mut alias[identity_index(identity_index(make_index()))]
        );
        result
    }

    machine return_after_deep_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells[identity_index(identity_index(make_index()))]
        );
        result
    }

    machine return_after_reborrow_indexed_statement<'cells, 'result, 'write>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64,
        index_write: &'write mut u64
    ) -> &'result mut u64 {
        write_argument(
            &mut cells[identity_index(write_index(&mut index_write))]
        );
        result
    }

    machine return_after_recursive_indexed_statement<'cells, 'result>(
        cells: &'cells mut [u64; 2],
        result: &'result mut u64
    ) -> &'result mut u64 {
        write_argument(&mut cells[recursive_index()]);
        result
    }

    machine Main::indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::nested_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_nested_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_alias_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_alias_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_alias_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_alias_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_helper_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_helper_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_helper_slice_view_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::alias_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_alias_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::helper_result_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_helper_result_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::recursive_helper_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_helper_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::projected_helper_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_indexed_statement(
            &mut self.bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::projected_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_projected_helper_slice_view_indexed_statement(
            &mut self.bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_projected_helper_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_projected_helper_slice_view_indexed_statement(
            &mut self.bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_projected_helper_slice_view_indexed_statement_result(
        &mut self
    ) {
        let alias: &mut u64 =
            return_after_recursive_projected_helper_slice_view_indexed_statement(
                &mut self.bucket,
                &mut self.result
            );
        alias = 3;
    }

    machine Main::slice_view_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_slice_view_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::deep_slice_view_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_slice_view_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_slice_view_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_slice_view_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::recursive_projected_helper_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_projected_helper_statement(
            &mut self.bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::recursive_member_after_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_member_after_index_statement(
            &mut self.cell_bucket,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::repeated_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_repeated_index_statement(
            &mut self.grid_bucket,
            &mut self.result,
            &mut self.index_write,
            &mut self.second_index_write
        );
        alias = 3;
    }

    machine Main::deep_repeated_index_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_repeated_index_statement(
            &mut self.grid_bucket,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::attached_result_indexed_statement_result(&mut self) {
        let alias: &mut u64 = self.return_after_attached_result_indexed_statement();
        alias = 3;
    }

    machine Main::recursive_attached_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_indexed_statement();
        alias = 3;
    }

    machine Main::attached_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_attached_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::deep_attached_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_deep_attached_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::recursive_attached_slice_view_indexed_statement_result(&mut self) {
        let alias: &mut u64 =
            self.return_after_recursive_attached_slice_view_indexed_statement();
        alias = 3;
    }

    machine Main::deep_alias_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_alias_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::deep_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_deep_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }

    machine Main::reborrow_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_reborrow_indexed_statement(
            &mut self.cells,
            &mut self.result,
            &mut self.index_write
        );
        alias = 3;
    }

    machine Main::recursive_indexed_statement_result(&mut self) {
        let alias: &mut u64 = return_after_recursive_indexed_statement(
            &mut self.cells,
            &mut self.result
        );
        alias = 3;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for (name, expected_paths) in [
        (
            "Main::indexed_statement_result",
            vec!["self.cells", "self.result"],
        ),
        (
            "Main::nested_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::alias_slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::helper_slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::alias_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::helper_result_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::attached_result_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::attached_slice_view_indexed_statement_result",
            vec!["self.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::projected_helper_indexed_statement_result",
            vec!["self.bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::projected_helper_slice_view_indexed_statement_result",
            vec!["self.bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::member_after_index_statement_result",
            vec!["self.cell_bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::slice_view_member_after_index_statement_result",
            vec!["self.cell_bucket.cells", "self.index_write", "self.result"],
        ),
        (
            "Main::repeated_index_statement_result",
            vec![
                "self.grid_bucket.rows",
                "self.index_write",
                "self.result",
                "self.second_index_write",
            ],
        ),
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                expected_paths
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
                    .as_slice()
            ),
            "{name} must publish the coarse argument, index writes, and returned-place write"
        );
    }

    for name in [
        "Main::deep_indexed_statement_result",
        "Main::deep_alias_indexed_statement_result",
        "Main::reborrow_indexed_statement_result",
        "Main::recursive_indexed_statement_result",
        "Main::deep_slice_view_indexed_statement_result",
        "Main::recursive_slice_view_indexed_statement_result",
        "Main::deep_alias_slice_view_indexed_statement_result",
        "Main::recursive_alias_slice_view_indexed_statement_result",
        "Main::deep_helper_slice_view_indexed_statement_result",
        "Main::recursive_helper_slice_view_indexed_statement_result",
        "Main::recursive_helper_indexed_statement_result",
        "Main::recursive_attached_indexed_statement_result",
        "Main::deep_attached_slice_view_indexed_statement_result",
        "Main::recursive_attached_slice_view_indexed_statement_result",
        "Main::recursive_projected_helper_statement_result",
        "Main::deep_projected_helper_slice_view_indexed_statement_result",
        "Main::recursive_projected_helper_slice_view_indexed_statement_result",
        "Main::deep_slice_view_member_after_index_statement_result",
        "Main::recursive_slice_view_member_after_index_statement_result",
        "Main::recursive_member_after_index_statement_result",
        "Main::deep_repeated_index_statement_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque outside the bounded indexed-argument rung"
        );
    }
}

#[test]
fn transparent_returned_place_accepts_bounded_isolated_scratch_initializers() {
    let source = r#"
    data Main {
        value: u64;
    }

    machine make_scratch() -> u64 {
        0
    }

    machine scratch_from(value: u64) -> u64 {
        value
    }

    machine write_scratch(value: &mut u64) -> u64 {
        value = 1;
        0
    }

    machine mixed_scratch(first: &mut u64, second: &mut u64) -> u64 {
        first = 1;
        second = 2;
        0
    }

    machine recursive_scratch() -> u64 {
        recursive_scratch()
    }

    machine return_with_nested_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(make_scratch());
        value
    }

    machine return_with_nested_write_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = scratch_from(write_scratch(&mut prior));
        value
    }

    machine return_with_deep_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = scratch_from(scratch_from(make_scratch()));
        value
    }

    machine return_with_external_write_scratch(value: &mut u64) -> &mut u64 {
        let mut prior: u64 = 0;
        let scratch: u64 = scratch_from(mixed_scratch(&mut prior, value));
        value
    }

    machine return_with_recursive_scratch(value: &mut u64) -> &mut u64 {
        let scratch: u64 = recursive_scratch();
        value
    }

    machine Main::nested_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_nested_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::nested_write_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_nested_write_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::deep_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_deep_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::external_write_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_external_write_scratch(&mut self.value);
        alias = 3;
    }

    machine Main::recursive_scratch_result(&mut self) {
        let alias: &mut u64 = return_with_recursive_scratch(&mut self.value);
        alias = 3;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::nested_scratch_result",
        "Main::nested_write_scratch_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(["self.value".to_owned()].as_slice()),
            "{name} must hide writes confined to earlier caller-isolated scratch roots"
        );
    }

    for name in [
        "Main::deep_scratch_result",
        "Main::external_write_scratch_result",
        "Main::recursive_scratch_result",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert!(
            !resolver
                .inferred_state_write_frame(machine, entry)
                .is_complete(),
            "{name} must remain opaque outside the bounded isolated-scratch rung"
        );
    }
}

#[test]
fn mutable_slice_views_preserve_array_storage_origins() {
    let source = r#"
    data Main {
        cells: [u64; 2];
    }

    machine return_slice(cells: &mut [u64; 2]) -> &mut [u64] {
        let view: &mut [u64] = cells.as_mut_slice();
        view
    }

    machine return_recursive_slice(cells: &mut [u64; 2]) -> &mut [u64] {
        return_recursive_slice(cells)
    }

    machine write_slice(view: &mut [u64]) {
        transition view.len > 0 {
            true -> write(view)
            false -> {}
        }

        state write(view: &mut [u64]) {
            view[0] = 1;
        }
    }

    machine write_slices(first: &mut [u64], second: &mut [u64]) {
        write_slice(first);
        write_slice(second);
    }

    machine noop() {}

    machine write_value(value: &mut u64) {
        value = 1;
    }

    machine return_value(value: &mut u64) -> &mut u64 {
        value
    }

    machine return_after_discarded_slice_view<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells mut [u64; 2]
    ) -> &'value mut u64 {
        cells.as_mut_slice().len;
        value
    }

    machine return_after_discarded_shared_slice_view<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells [u64; 2]
    ) -> &'value mut u64 {
        cells.as_slice().len;
        value
    }

    machine return_after_empty_statement_call(value: &mut u64) -> &mut u64 {
        noop();
        value
    }

    machine return_after_write_statement_call(value: &mut u64) -> &mut u64 {
        write_value(value);
        value
    }

    machine return_after_binding_reborrow_statement_call(value: &mut u64) -> &mut u64 {
        write_value(&mut value);
        value
    }

    machine return_after_direct_call_argument<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells mut [u64; 2]
    ) -> &'value mut u64 {
        write_slice(return_slice(cells));
        value
    }

    machine return_after_recursive_call_argument<'value, 'cells>(
        value: &'value mut u64,
        cells: &'cells mut [u64; 2]
    ) -> &'value mut u64 {
        write_slice(return_recursive_slice(cells));
        value
    }

    machine return_after_deep_call_argument(value: &mut u64) -> &mut u64 {
        write_value(return_value(return_value(value)));
        value
    }

    machine return_after_too_deep_call_argument(value: &mut u64) -> &mut u64 {
        write_value(return_value(return_value(return_value(value))));
        value
    }

    machine return_after_sibling_call_arguments<'value, 'first, 'second>(
        value: &'value mut u64,
        first: &'first mut [u64; 2],
        second: &'second mut [u64; 2]
    ) -> &'value mut u64 {
        write_slices(return_slice(first), return_slice(second));
        value
    }

    machine return_after_mixed_sibling_call_arguments<'value, 'first, 'second>(
        value: &'value mut u64,
        first: &'first mut [u64; 2],
        second: &'second mut [u64; 2]
    ) -> &'value mut u64 {
        write_slices(return_slice(first), return_recursive_slice(second));
        value
    }

    machine Main::direct_view(&mut self) {
        let view: &mut [u64] = self.cells.as_mut_slice();
        view[0] = 1;
    }

    machine Main::helper_view(&mut self) {
        let view: &mut [u64] = return_slice(&mut self.cells);
        view[0] = 1;
    }

    machine Main::recursive_view(&mut self) {
        let view: &mut [u64] = return_recursive_slice(&mut self.cells);
        view[0] = 1;
    }

    machine Main::statement_view(&mut self) {
        write_slice(self.cells.as_mut_slice());
    }

    machine Main::recursive_statement_view(&mut self) {
        write_slice(return_recursive_slice(&mut self.cells));
    }

    machine Main::discarded_slice_view(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_slice_view(&mut self.value, &mut self.cells);
        alias = 1;
    }

    machine Main::empty_statement_call(&mut self) {
        let alias: &mut u64 = return_after_empty_statement_call(&mut self.value);
        alias = 1;
    }

    machine Main::write_statement_call(&mut self) {
        let alias: &mut u64 = return_after_write_statement_call(&mut self.value);
        alias = 1;
    }

    machine Main::binding_reborrow_statement_call(&mut self) {
        let alias: &mut u64 =
            return_after_binding_reborrow_statement_call(&mut self.value);
        alias = 1;
    }

    machine Main::direct_call_argument_statement_call(&mut self) {
        let alias: &mut u64 =
            return_after_direct_call_argument(&mut self.value, &mut self.cells);
        alias = 1;
    }

    machine Main::recursive_call_argument_statement_call(&mut self) {
        let alias: &mut u64 =
            return_after_recursive_call_argument(&mut self.value, &mut self.cells);
        alias = 1;
    }

    machine Main::deep_call_argument_statement_call(&mut self) {
        let alias: &mut u64 = return_after_deep_call_argument(&mut self.value);
        alias = 1;
    }

    machine Main::too_deep_call_argument_statement_call(&mut self) {
        let alias: &mut u64 = return_after_too_deep_call_argument(&mut self.value);
        alias = 1;
    }

    machine Main::sibling_call_arguments_statement_call(&mut self) {
        let alias: &mut u64 = return_after_sibling_call_arguments(
            &mut self.value,
            &mut self.cells,
            &mut self.other_cells
        );
        alias = 1;
    }

    machine Main::mixed_sibling_call_arguments_statement_call(&mut self) {
        let alias: &mut u64 = return_after_mixed_sibling_call_arguments(
            &mut self.value,
            &mut self.cells,
            &mut self.other_cells
        );
        alias = 1;
    }

    machine Main::discarded_shared_slice_view(&mut self) {
        let alias: &mut u64 =
            return_after_discarded_shared_slice_view(&mut self.value, self.cells);
        alias = 1;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("valid symbol cache");

    for name in [
        "Main::direct_view",
        "Main::helper_view",
        "Main::statement_view",
        "Main::discarded_slice_view",
        "Main::discarded_shared_slice_view",
        "Main::empty_statement_call",
        "Main::write_statement_call",
    ] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} machine"));
        let entry = typed
            .machine_states(machine)
            .first()
            .unwrap_or_else(|| panic!("{name} entry state"));
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(
                [if matches!(
                    name,
                    "Main::discarded_slice_view"
                        | "Main::discarded_shared_slice_view"
                        | "Main::empty_statement_call"
                        | "Main::write_statement_call"
                ) {
                    "self.value"
                } else {
                    "self.cells"
                }
                .to_owned()]
                .as_slice()
            ),
            "{name} must retain the mutable view's array storage origin"
        );
    }

    let recursive = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::recursive_view")
        .expect("recursive view caller");
    let recursive_entry = typed
        .machine_states(recursive)
        .first()
        .expect("recursive view caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(recursive, recursive_entry)
            .is_complete(),
        "an opaque recursive slice producer must remain a frame fence"
    );

    let recursive_statement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::recursive_statement_view")
        .expect("recursive statement-view caller");
    let recursive_statement_entry = typed
        .machine_states(recursive_statement)
        .first()
        .expect("recursive statement-view caller entry state");
    let recursive_statement_frame =
        resolver.inferred_state_write_frame(recursive_statement, recursive_statement_entry);
    assert!(
        recursive_statement_frame
            .complete_paths()
            .is_none_or(|paths| paths.iter().any(|path| path == "self")),
        "an opaque recursive statement argument must retain a whole-receiver fence"
    );

    let binding_reborrow_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::binding_reborrow_statement_call")
        .expect("binding-reborrow statement-call caller");
    let binding_reborrow_statement_call_entry = typed
        .machine_states(binding_reborrow_statement_call)
        .first()
        .expect("binding-reborrow statement-call caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                binding_reborrow_statement_call,
                binding_reborrow_statement_call_entry,
            )
            .is_complete(),
        "an explicit mutable-reference binding reborrow must keep the helper relation opaque"
    );

    let direct_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::direct_call_argument_statement_call")
        .expect("direct call-argument statement caller");
    let direct_call_argument_statement_call_entry = typed
        .machine_states(direct_call_argument_statement_call)
        .first()
        .expect("direct call-argument statement caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(
                direct_call_argument_statement_call,
                direct_call_argument_statement_call_entry,
            )
            .complete_paths(),
        Some(["self.cells".to_owned(), "self.value".to_owned()].as_slice()),
        "one exact direct value-call argument must preserve both its write and the returned origin"
    );

    let recursive_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::recursive_call_argument_statement_call")
        .expect("recursive call-argument statement caller");
    let recursive_call_argument_statement_call_entry = typed
        .machine_states(recursive_call_argument_statement_call)
        .first()
        .expect("recursive call-argument statement caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                recursive_call_argument_statement_call,
                recursive_call_argument_statement_call_entry,
            )
            .is_complete(),
        "an opaque recursive value-call argument must remain a returned-place fence"
    );

    let deep_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::deep_call_argument_statement_call")
        .expect("deep call-argument statement caller");
    let deep_call_argument_statement_call_entry = typed
        .machine_states(deep_call_argument_statement_call)
        .first()
        .expect("deep call-argument statement caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(
                deep_call_argument_statement_call,
                deep_call_argument_statement_call_entry,
            )
            .complete_paths(),
        Some(["self.value".to_owned()].as_slice()),
        "a two-level exact value-call argument tree must preserve the returned origin"
    );

    let too_deep_call_argument_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::too_deep_call_argument_statement_call")
        .expect("too-deep call-argument statement caller");
    let too_deep_call_argument_statement_call_entry = typed
        .machine_states(too_deep_call_argument_statement_call)
        .first()
        .expect("too-deep call-argument statement caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                too_deep_call_argument_statement_call,
                too_deep_call_argument_statement_call_entry,
            )
            .is_complete(),
        "a value-call argument tree deeper than two calls must remain opaque"
    );

    let sibling_call_arguments_statement_call = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::sibling_call_arguments_statement_call")
        .expect("sibling call-arguments statement caller");
    let sibling_call_arguments_statement_call_entry = typed
        .machine_states(sibling_call_arguments_statement_call)
        .first()
        .expect("sibling call-arguments statement caller entry state");
    assert_eq!(
        resolver
            .inferred_state_write_frame(
                sibling_call_arguments_statement_call,
                sibling_call_arguments_statement_call_entry,
            )
            .complete_paths(),
        Some(
            [
                "self.cells".to_owned(),
                "self.other_cells".to_owned(),
                "self.value".to_owned(),
            ]
            .as_slice()
        ),
        "exact sibling value-call arguments must compose their writes and the returned origin"
    );

    let mixed_sibling_call_arguments_statement_call = typed
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == "Main::mixed_sibling_call_arguments_statement_call"
        })
        .expect("mixed sibling call-arguments statement caller");
    let mixed_sibling_call_arguments_statement_call_entry = typed
        .machine_states(mixed_sibling_call_arguments_statement_call)
        .first()
        .expect("mixed sibling call-arguments statement caller entry state");
    assert!(
        !resolver
            .inferred_state_write_frame(
                mixed_sibling_call_arguments_statement_call,
                mixed_sibling_call_arguments_statement_call_entry,
            )
            .is_complete(),
        "one opaque sibling value-call argument must fence the whole returned-place relation"
    );
}

#[test]
fn crash_bucket_identity_includes_cause_routes_and_unconditional_presence() {
    let source = r#"
    machine baseline() {}
    machine trap_activation(flag: bool)
    crashes Trap
        flag
    {}
    machine trap_domain(flag: bool)
    crashes Trap
        flag
    {}
    machine abort_activation(flag: bool)
    crashes Abort
        flag
    {}
    machine unconditional_abort()
    crashes Abort
    {}
    machine explicit_true_abort()
    crashes Abort
        true
    {}
    machine grouped(first: bool, second: bool)
    crashes Trap
        first
        second
    {}
    machine split(first: bool, second: bool)
    crashes Trap
        first
    crashes Trap
        second
    {}
    machine reordered(first: bool, second: bool)
    crashes Trap
        second
        first
    {}
    machine duplicated(first: bool, second: bool)
    crashes Trap
        first
        second
        first
    {}
    machine unconditional_with_guard(flag: bool)
    crashes Abort
        flag
    crashes Abort
    {}
    machine unconditional_only(flag: bool)
    crashes Abort
    {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let fingerprint = |name: &str| {
        let symbol = symbol_of_checked(&checked, name);
        checked
            .facts
            .contract_plans
            .for_machine(symbol)
            .expect("contract plan")
            .fingerprint
    };

    assert_ne!(fingerprint("baseline"), fingerprint("unconditional_abort"));
    assert_eq!(
        fingerprint("unconditional_abort"),
        fingerprint("explicit_true_abort")
    );
    assert_eq!(fingerprint("trap_activation"), fingerprint("trap_domain"));
    assert_ne!(
        fingerprint("trap_activation"),
        fingerprint("abort_activation")
    );
    assert_eq!(fingerprint("grouped"), fingerprint("split"));
    assert_eq!(fingerprint("grouped"), fingerprint("reordered"));
    assert_eq!(fingerprint("grouped"), fingerprint("duplicated"));
    assert_eq!(
        fingerprint("unconditional_with_guard"),
        fingerprint("unconditional_only")
    );

    let crash = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
            .crash
            .clone()
    };
    assert_eq!(crash("grouped"), crash("split"));
    assert_eq!(crash("grouped"), crash("reordered"));
    assert_eq!(crash("grouped"), crash("duplicated"));
    assert_eq!(crash("unconditional_abort"), crash("explicit_true_abort"));
    let grouped = crash("grouped");
    assert_eq!(
        grouped.interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
    assert_eq!(grouped.published().len(), 1);
    assert_eq!(
        grouped.published()[0].cause(),
        psi_checked_trees::CrashCause::Trap
    );
    assert_eq!(grouped.published()[0].alternative_guards().len(), 2);
    assert!(!grouped.published()[0].is_unconditional());
}

#[test]
fn checked_crash_sites_are_body_evidence_not_contract_identity() {
    let source = r#"
    machine clear_body() -> i32
    crashes Abort
    { 0 }

    machine crashing_body() -> i32
    crashes Abort
    {
        crash Abort;
    }

    machine guarded_body(flag: bool) -> i32
    crashes Trap
        flag
    {
        crash Trap;
    }

    machine path_guarded_body(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag -> fail()
            _ -> 0i32
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine fallthrough_guarded_body(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag -> 0i32
            _ -> fail()
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine conjunct_guarded_body(flag: bool, other: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag && other -> fail()
            _ -> 0i32
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine demorgan_guarded_body(flag: bool, other: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag || other -> 0i32
            _ -> fail()
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine disjunction_does_not_cover(flag: bool, other: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag || other -> fail()
            _ -> 0i32
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine negated_conjunction_does_not_cover(flag: bool, other: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag && other -> 0i32
            _ -> fail()
        }

        state fail() -> i32 {
            crash Trap;
        }
    }

    machine narrow_abort() -> i32
    crashes Abort
    {
        crash Abort;
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    assert_eq!(
        plan("clear_body").fingerprint,
        plan("crashing_body").fingerprint,
        "changing the checked body must not change a published contract identity"
    );
    assert!(plan("clear_body").crash.checked_sites().is_empty());
    let [site] = plan("crashing_body").crash.checked_sites() else {
        panic!("the explicit crash should produce exactly one checked site")
    };
    assert_eq!(site.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(site.location().statement_ordinal(), 0);
    let [covering_bucket] = site.guard_covering_buckets() else {
        panic!("an unconditional same-cause route should cover every site guard")
    };
    assert!(
        plan("crashing_body")
            .crash
            .published_bucket(*covering_bucket)
            .is_some_and(|bucket| bucket.is_unconditional()
                && bucket.cause() == psi_checked_trees::CrashCause::Abort)
    );
    assert_eq!(
        site.location().state(),
        checked.machine_states(
            checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "crashing_body")
                .expect("crashing machine")
        )[0]
        .symbol
    );

    let [guarded_site] = plan("guarded_body").crash.checked_sites() else {
        panic!("the guarded machine should retain its explicit crash site")
    };
    assert!(
        guarded_site.guard_covering_buckets().is_empty(),
        "a route predicate is not unconditional guard-coverage evidence"
    );
    assert!(
        guarded_site.path_guard_conjuncts().is_empty(),
        "an unconditional body crash has no incoming path predicate"
    );

    let [path_guarded_site] = plan("path_guarded_body").crash.checked_sites() else {
        panic!("the guarded target state should retain its explicit crash site")
    };
    let [path_covering_bucket] = path_guarded_site.guard_covering_buckets() else {
        panic!("the exact incoming path guard should cover its published route")
    };
    assert_eq!(path_guarded_site.path_guard_conjuncts().len(), 1);
    assert!(
        plan("path_guarded_body")
            .crash
            .published_bucket(*path_covering_bucket)
            .is_some_and(|bucket| !bucket.is_unconditional()
                && bucket.cause() == psi_checked_trees::CrashCause::Trap)
    );

    let [fallthrough_guarded_site] = plan("fallthrough_guarded_body").crash.checked_sites() else {
        panic!("the fallthrough target state should retain its explicit crash site")
    };
    let [fallthrough_covering_bucket] = fallthrough_guarded_site.guard_covering_buckets() else {
        panic!("the negated incoming path guard should cover its published route")
    };
    assert_eq!(fallthrough_guarded_site.path_guard_conjuncts().len(), 1);
    assert!(
        plan("fallthrough_guarded_body")
            .crash
            .published_bucket(*fallthrough_covering_bucket)
            .is_some_and(|bucket| !bucket.is_unconditional()
                && bucket.cause() == psi_checked_trees::CrashCause::Trap)
    );

    for name in ["conjunct_guarded_body", "demorgan_guarded_body"] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one explicit crash site")
        };
        let [bucket] = site.guard_covering_buckets() else {
            panic!("{name} should prove its structurally implied route")
        };
        assert_eq!(
            site.path_guard_conjuncts().len(),
            1,
            "the exact derived guard remains separate from its consequences"
        );
        assert!(
            !site.path_guard_consequences().is_empty(),
            "the implication witness remains available to terminal lowering"
        );
        assert!(
            plan(name)
                .crash
                .published_bucket(*bucket)
                .is_some_and(|bucket| bucket.cause() == psi_checked_trees::CrashCause::Trap)
        );
    }
    for name in [
        "disjunction_does_not_cover",
        "negated_conjunction_does_not_cover",
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one explicit crash site")
        };
        assert!(
            site.guard_covering_buckets().is_empty(),
            "{name} must not use the unsound converse implication"
        );
    }

    let [narrow_abort_site] = plan("narrow_abort").crash.checked_sites() else {
        panic!("the narrow abort should retain its explicit crash site")
    };
    assert_eq!(narrow_abort_site.guard_covering_buckets().len(), 1);
    assert_eq!(
        plan("narrow_abort")
            .crash
            .covering_buckets_for_site(narrow_abort_site)
            .count(),
        1,
        "a same-cause unconditional route covers the crash site"
    );
}

#[test]
fn crash_guard_entailment_normalizes_boolean_literal_relations() {
    let source = r#"
    machine risky() -> i32
    crashes Trap
    { 1 }

    machine equal_true(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag == true -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine equal_false(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag == false -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine not_equal_true(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag != true -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine not_equal_false(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag != false -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine fallthrough_equal_true(flag: bool) -> i32
    crashes Trap
        !flag
    {
        transition {
            flag == true -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine guarded_call(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag == true -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("Boolean literal relations should imply their normalized operand polarity");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    for name in [
        "equal_true",
        "equal_false",
        "not_equal_true",
        "not_equal_false",
        "fallthrough_equal_true",
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one crash site")
        };
        assert_eq!(
            site.guard_covering_buckets().len(),
            1,
            "{name} should cover its route through the normalized relation"
        );
    }

    let [call] = plan("guarded_call").crash.checked_calls() else {
        panic!("guarded_call should retain one checked call")
    };
    assert_eq!(call.path_guard_conjuncts().len(), 1);
    assert_eq!(
        call.path_guard_consequences().len(),
        3,
        "the exact equality, reversed equality, and implied operand remain separate"
    );
}

#[test]
fn crash_guard_entailment_normalizes_comparison_equivalences() {
    let source = r#"
    machine risky() -> i32
    crashes Trap
    { 1 }

    machine reversed_order(left: i32, right: i32) -> i32
    crashes Trap
        right > left
    {
        transition {
            left < right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine strict_order_weakens(left: i32, right: i32) -> i32
    crashes Trap
        left <= right
    {
        transition {
            left < right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine strict_order_is_distinct(left: i32, right: i32) -> i32
    crashes Trap
        left != right
    {
        transition {
            left < right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_equality_bounds(left: i32, right: i32) -> i32
    crashes Trap
        right >= left
    {
        transition {
            left == right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_order_fallthrough(left: i32, right: i32) -> i32
    crashes Trap
        left >= right
    {
        transition {
            left < right -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine float_order_fallthrough_stays_opaque(left: f32, right: f32) -> i32
    crashes Trap
        left >= right
    {
        transition {
            left < right -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine negated_equality(left: i32, right: i32) -> i32
    crashes Trap
        left != right
    {
        transition {
            left == right -> 0i32
            _ -> fail()
        }
        state fail() -> i32 { crash Trap; }
    }

    machine reversed_equality(left: i32, right: i32) -> i32
    crashes Trap
        right == left
    {
        transition {
            left == right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine transitive_integer_order(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left < middle && middle <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine nontransitive_integer_order(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left < middle && right < middle -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine transitive_nonstrict_order(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left <= right
    {
        transition {
            left <= middle && middle <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_order_antisymmetry(left: i32, right: i32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right && right <= left -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine integer_nonstrict_plus_disequality(left: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= right && left != right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine one_sided_order_does_not_prove_equality(left: i32, right: i32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine float_order_antisymmetry_stays_opaque(left: f32, right: f32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right && right <= left -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine float_nonstrict_plus_disequality_stays_opaque(left: f32, right: f32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= right && left != right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine transitive_order_across_states(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left < middle -> compare(left, middle, right)
            _ -> 0i32
        }
        state compare(left: i32, middle: i32, right: i32) -> i32 {
            transition {
                middle <= right -> fail()
                _ -> 0i32
            }
        }
        state fail() -> i32 { crash Trap; }
    }

    machine nonstrict_chain_does_not_prove_strict(
        left: i32,
        middle: i32,
        right: i32
    ) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= middle && middle <= right -> fail()
            _ -> 0i32
        }
        state fail() -> i32 { crash Trap; }
    }

    machine guarded_call(left: i32, right: i32) -> i32
    crashes Trap
        left != right
    {
        transition {
            left == right -> 0i32
            _ -> invoke()
        }
        state invoke() -> i32 { risky() }
    }

    machine transitive_guarded_call(left: i32, middle: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= middle && middle < right -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }

    machine antisymmetric_guarded_call(left: i32, right: i32) -> i32
    crashes Trap
        left == right
    {
        transition {
            left <= right && right <= left -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }


    machine strict_refined_guarded_call(left: i32, right: i32) -> i32
    crashes Trap
        left < right
    {
        transition {
            left <= right && left != right -> invoke()
            _ -> 0i32
        }
        state invoke() -> i32 { risky() }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("equivalent comparison spellings should cover crash routes");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    for name in [
        "reversed_order",
        "strict_order_weakens",
        "strict_order_is_distinct",
        "integer_equality_bounds",
        "integer_order_fallthrough",
        "negated_equality",
        "reversed_equality",
        "transitive_integer_order",
        "transitive_nonstrict_order",
        "integer_order_antisymmetry",
        "integer_nonstrict_plus_disequality",
        "transitive_order_across_states",
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one crash site")
        };
        assert_eq!(
            site.guard_covering_buckets().len(),
            1,
            "{name} should cover its equivalent comparison route"
        );
    }
    let [opaque_site] = plan("float_order_fallthrough_stays_opaque")
        .crash
        .checked_sites()
    else {
        panic!("float ordered fallthrough should retain one crash site")
    };
    assert!(
        opaque_site.guard_covering_buckets().is_empty(),
        "unordered float comparison negation must remain opaque"
    );
    for (name, reason) in [
        (
            "nontransitive_integer_order",
            "relations without a shared ordered endpoint must not compose",
        ),
        (
            "nonstrict_chain_does_not_prove_strict",
            "an all-nonstrict chain must not imply a strict endpoint relation",
        ),
        (
            "one_sided_order_does_not_prove_equality",
            "one nonstrict direction must not imply integer equality",
        ),
        (
            "float_order_antisymmetry_stays_opaque",
            "unordered float relations must not enter integer antisymmetry",
        ),
        (
            "float_nonstrict_plus_disequality_stays_opaque",
            "unordered float relations must not enter integer strict refinement",
        ),
    ] {
        let [site] = plan(name).crash.checked_sites() else {
            panic!("{name} should retain one crash site")
        };
        assert!(site.guard_covering_buckets().is_empty(), "{reason}");
    }

    let [call] = plan("guarded_call").crash.checked_calls() else {
        panic!("guarded_call should retain one checked call")
    };
    assert!(
        call.path_guard_consequences().len() >= 3,
        "the exact fallthrough predicate and normalized comparison forms remain distinct"
    );
    let [transitive_call] = plan("transitive_guarded_call").crash.checked_calls() else {
        panic!("transitive_guarded_call should retain one checked call")
    };
    assert!(
        transitive_call.path_guard_consequences().len() > call.path_guard_consequences().len(),
        "transitive integer order should add source-independent call-path consequences"
    );
    let [antisymmetric_call] = plan("antisymmetric_guarded_call").crash.checked_calls() else {
        panic!("antisymmetric_guarded_call should retain one checked call")
    };
    assert!(
        antisymmetric_call.path_guard_consequences().len() > call.path_guard_consequences().len(),
        "integer antisymmetry should add source-independent call-path equality"
    );
    let [strict_refined_call] = plan("strict_refined_guarded_call").crash.checked_calls() else {
        panic!("strict_refined_guarded_call should retain one checked call")
    };
    assert!(
        strict_refined_call.path_guard_consequences().len() > call.path_guard_consequences().len(),
        "integer disequality should sharpen a nonstrict call-path bound"
    );
}

#[test]
fn checked_crash_calls_retain_invocation_specific_route_refinement() {
    let source = r#"
    machine risky(flag: bool) -> i32
    crashes Trap
        flag
    { 1 }

    machine safe() -> i32 { risky(false) }

    machine certain() -> i32
    crashes Trap
    { risky(true) }

    machine forwarded(flag: bool) -> i32
    crashes Trap
        flag
    { risky(flag) }

    machine local_forwarded(flag: bool) -> i32 {
        let forwarded: bool = flag;
        risky(forwarded)
    }

    machine computed_local_forwarded(flag: bool) -> i32
    crashes Trap
    {
        let forwarded: bool = !flag;
        risky(forwarded)
    }

    machine conditioned(flag: bool) -> i32
    crashes Trap
        flag
    {
        transition {
            flag -> invoke()
            _ -> 0i32
        }

        state invoke() -> i32 { risky(true) }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    let [safe_call] = plan("safe").crash.checked_calls() else {
        panic!("the crash-capable invocation should retain one checked call row")
    };
    assert!(
        safe_call.surviving_buckets().is_empty(),
        "a concrete false argument disproves the callee's only crash route"
    );
    assert_eq!(safe_call.location().statement_ordinal(), 0);
    assert_eq!(safe_call.location().call_ordinal(), 0);
    assert_eq!(
        safe_call.target_machine(),
        symbol_of_checked(&checked, "risky")
    );
    assert_eq!(
        safe_call.target_contract_fingerprint(),
        plan("risky").fingerprint
    );

    let [certain_call] = plan("certain").crash.checked_calls() else {
        panic!("the concrete true invocation should retain one checked call row")
    };
    let [certain_bucket] = certain_call.surviving_buckets() else {
        panic!("the concrete true route should survive")
    };
    assert!(certain_bucket.is_unconditional());
    assert_eq!(certain_bucket.cause(), psi_checked_trees::CrashCause::Trap);

    let [forwarded_call] = plan("forwarded").crash.checked_calls() else {
        panic!("the unresolved invocation should retain one checked call row")
    };
    let [forwarded_bucket] = forwarded_call.surviving_buckets() else {
        panic!("the unresolved route should survive")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(forwarded_route)] =
        forwarded_bucket.alternative_guards()
    else {
        panic!("the unresolved route should remain a predicate")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(published_route)] =
        plan("forwarded").crash.published()[0].alternative_guards()
    else {
        panic!("the caller should publish its forwarded predicate")
    };
    assert_eq!(
        forwarded_route, published_route,
        "argument substitution should move the callee route into the caller's positional namespace"
    );
    assert_eq!(
        forwarded_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Parameter { position: 0 }),
        "invocation refinement must retain checked scalar meaning, not only predicate identity",
    );

    let [local_forwarded_call] = plan("local_forwarded").crash.checked_calls() else {
        panic!("the local-argument invocation should retain one checked call row")
    };
    let [local_forwarded_bucket] = local_forwarded_call.surviving_buckets() else {
        panic!("the local-argument route should survive")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(local_forwarded_route)] =
        local_forwarded_bucket.alternative_guards()
    else {
        panic!("the local-argument route should remain a predicate")
    };
    assert_eq!(
        local_forwarded_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Local { position: 1 }),
        "direct refinement should retain the caller-local value position assigned after its one parameter",
    );

    let [computed_local_call] = plan("computed_local_forwarded").crash.checked_calls() else {
        panic!("the computed-local invocation should retain one checked call row")
    };
    let [computed_local_bucket] = computed_local_call.surviving_buckets() else {
        panic!("the computed-local route should survive")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(computed_local_route)] =
        computed_local_bucket.alternative_guards()
    else {
        panic!("the computed-local route should remain a predicate")
    };
    assert_eq!(
        computed_local_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Local { position: 1 }),
        "computed refinement should retain the caller-local value position",
    );

    let [conditioned_call] = plan("conditioned").crash.checked_calls() else {
        panic!("the named transition itself is not a public machine invocation")
    };
    assert_eq!(
        conditioned_call.path_guard_conjuncts().len(),
        1,
        "the checked call retains the exact incoming path conjunction"
    );
}

#[test]
fn published_caller_must_cover_every_surviving_call_crash_route() {
    let source = r#"
    machine risky() -> i32
    crashes Abort
    {
        crash Abort;
    }

    machine wrong_cause() -> i32
    crashes Trap
    {
        risky()
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the caller's Trap ceiling cannot cover a surviving Abort route");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("call from `wrong_cause` to `risky`")
            && diagnostic.message.contains("uncovered Abort crash route")
    }));
}

#[test]
fn checked_crash_calls_select_acyclic_private_body_summaries() {
    let source = r#"
    machine inferred_abort() -> i32 {
        crash Abort;
    }

    machine inferred_safe() -> i32 { 1 }

    machine call_abort() -> i32 { inferred_abort() }
    machine call_safe() -> i32 { inferred_safe() }

    machine nonleaf() -> i32 { inferred_abort() }
    machine call_nonleaf() -> i32 { nonleaf() }

    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    assert_eq!(
        plan("inferred_abort").crash.interface(),
        psi_checked_trees::CrashInterface::InternalInferred
    );
    let [abort_call] = plan("call_abort").crash.checked_calls() else {
        panic!("a call to a private crashing leaf should retain one selected body summary")
    };
    let [abort_bucket] = abort_call.surviving_buckets() else {
        panic!("the private leaf's explicit crash should survive as one inferred bucket")
    };
    assert!(abort_bucket.is_unconditional());
    assert_eq!(abort_bucket.cause(), psi_checked_trees::CrashCause::Abort);

    let [safe_call] = plan("call_safe").crash.checked_calls() else {
        panic!("a call to a private crash-free leaf should retain positive empty evidence")
    };
    assert!(safe_call.surviving_buckets().is_empty());

    assert_eq!(plan("nonleaf").crash.checked_calls().len(), 1);
    let [nonleaf_call] = plan("call_nonleaf").crash.checked_calls() else {
        panic!("the acyclic private wrapper should publish one selected body summary")
    };
    let [nonleaf_bucket] = nonleaf_call.surviving_buckets() else {
        panic!("the nested abort should propagate through the private wrapper")
    };
    assert!(nonleaf_bucket.is_unconditional());
    assert_eq!(nonleaf_bucket.cause(), psi_checked_trees::CrashCause::Abort);
}

#[test]
fn private_crash_summaries_compose_guarded_routes_across_nonleaf_calls() {
    let source = r#"
    machine risky(flag: bool) -> i32
    crashes Trap
        flag
    { 1 }

    machine inner(flag: bool) -> i32 { risky(flag) }
    machine outer(flag: bool) -> i32 { inner(flag) }

    machine covered(flag: bool) -> i32
    crashes Trap
        flag
    { outer(flag) }

    machine disproved() -> i32 { outer(false) }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("a published caller should cover a guard retained through private wrappers");
    let plan = |name: &str| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol_of_checked(&checked, name))
            .expect("contract plan")
    };

    let [outer_to_inner] = plan("outer").crash.checked_calls() else {
        panic!("outer should retain its private call")
    };
    let [outer_bucket] = outer_to_inner.surviving_buckets() else {
        panic!("the inner summary should retain its guarded route")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(outer_route)] =
        outer_bucket.alternative_guards()
    else {
        panic!("the private nonleaf route should remain guarded")
    };

    let [covered_call] = plan("covered").crash.checked_calls() else {
        panic!("covered should retain its outer call")
    };
    let [covered_bucket] = covered_call.surviving_buckets() else {
        panic!("covered should retain the composed route")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(covered_route)] =
        covered_bucket.alternative_guards()
    else {
        panic!("the composed route should remain guarded")
    };
    let [psi_checked_trees::CrashRouteGuard::Predicate(published_route)] =
        plan("covered").crash.published()[0].alternative_guards()
    else {
        panic!("covered should publish one guarded route")
    };
    assert_eq!(outer_route, covered_route);
    assert_eq!(covered_route, published_route);
    assert_eq!(
        covered_route.scalar_expression(),
        Some(&psi_checked_trees::CheckedBooleanExpression::Parameter { position: 0 }),
        "acyclic private-summary substitution must preserve terminal-lowerable scalar meaning",
    );

    let [disproved_call] = plan("disproved").crash.checked_calls() else {
        panic!("disproved should retain positive evidence for its outer call")
    };
    assert!(
        disproved_call.surviving_buckets().is_empty(),
        "substitution through both private wrappers should prove false"
    );
}

#[test]
fn checked_crash_calls_select_machine_requirement_capsules() {
    let source = r#"
    machine apply<machine Selected>(flag: bool)
    where machine Selected(value: bool)
        crashes Abort
            value;
    crashes Abort
        flag
    {
        Selected(flag);
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("requirement crash capsule should lower");
    let apply = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("apply machine");
    let plan = checked
        .facts
        .contract_plans
        .for_machine(apply.symbol)
        .expect("apply contract plan");
    let [call] = plan.crash.checked_calls() else {
        panic!("the requirement call should retain one checked crash row");
    };
    let capsule = checked
        .facts
        .contract_plans
        .crash_capsule(call.target_machine(), call.target_state())
        .expect("the abstract target should retain its normalized capsule");
    assert_eq!(
        call.target_contract_fingerprint(),
        capsule.target_contract_fingerprint()
    );
    let [bucket] = call.surviving_buckets() else {
        panic!("the unknown flag should retain the guarded Abort bucket");
    };
    assert_eq!(bucket.cause(), psi_checked_trees::CrashCause::Abort);
    assert!(matches!(
        bucket.alternative_guards(),
        [psi_checked_trees::CrashRouteGuard::Predicate(_)]
    ));
}

fn symbol_of_checked(
    checked: &psi_checked_trees::CheckedTrees,
    name: &str,
) -> psi_symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("machine {name}"))
        .symbol
}

/// R2 rung 2 slice 2: the admitted zero-satisfying default-domain facts
/// travel to the TYPED data definition -- rung 3's consumer substrate.
#[test]
fn data_where_facts_propagate_to_typed() {
    let source = r#"
    data Ledger
    where
        count <= len,
    {
        len: u32;
        count: u32;
    }

    data Main { ledger: Ledger; }

    machine Main::main(&mut self) -> u64 { 7 }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let ledger = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Ledger")
        .expect("Ledger data");
    assert_eq!(typed.proof_facts.span_or_empty(ledger.where_facts).len(), 1);
}

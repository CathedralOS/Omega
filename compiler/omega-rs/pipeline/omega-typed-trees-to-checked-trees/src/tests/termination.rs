use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

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

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove decreases clause"))
    );
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

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove decreases clause"))
    );
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

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove decreases clause"))
    );
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

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove decreases clause"))
    );
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
                .contains("`decreases (limit, index)` inverts the named bounded distance")
                && diagnostic.message.contains("`Nat::BoundedDistance`")
                && diagnostic
                    .message
                    .contains("write `decreases (index, limit) -> Nat::BoundedDistance`")
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
    let diagnostics =
        lower_typed_trees(typed).expect_err("the subtraction spelling is retired surface");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("the use-site subtraction `decreases limit - index`")
                && diagnostic.message.contains("is retired")
                && diagnostic.message.contains(
                    "spell the ranking as `decreases (index, limit) -> Nat::BoundedDistance`",
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

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove decreases clause"))
    );
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
                .contains("cannot infer a ranking for `decreases remaining`")
                && diagnostic
                    .message
                    .contains("signed values have no default well-founded order")
                && diagnostic.message.contains("`decreases remaining -> View`")
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
                .contains("cannot infer a ranking for `decreases card`")
                && diagnostic
                    .message
                    .contains("declared measures are never selected implicitly")
                && diagnostic
                    .message
                    .contains("`decreases card -> Card::PowerOrder`")
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
    use omega_core::semantics::{RankingViewId, TerminationGuarantee};

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
        promise.published,
        Some(TerminationGuarantee::EventualTerminal {
            premises: Vec::new()
        })
    );
    assert!(promise.implementation_witness.is_none());

    // `terminates by remaining;`: witness only (publishes nothing); the
    // single u64 subject's canonical default elaborates immediately.
    let countdown = plan_of("Main::countdown");
    assert_eq!(countdown.published, None);
    let witness = countdown
        .implementation_witness
        .as_ref()
        .expect("countdown witness");
    assert_eq!(witness.subjects, vec!["remaining".to_string()]);
    assert_eq!(witness.ranking_view, RankingViewId::NAT_DESCENDING);
    assert_eq!(witness.view_path, "Nat::Descending");

    // Two-subject short form: the only builtin two-subject view.
    let walk = plan_of("Main::walk");
    assert_eq!(walk.published, None);
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
    use omega_core::semantics::RankingViewId;

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

/// TPR3 slice 1 (decision 23 firewall): a recorded witness view that
/// DIVERGES from the checker's independently resolved order is an internal
/// invariant violation surfaced loudly -- never a silent preference for
/// either side. (Constructed by mutating the plan post-typing; the real
/// lowering mirrors the checker's inference exactly.)
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
    witness.ranking_view = omega_core::semantics::RankingViewId::SLICE_LENGTH;

    let diagnostics =
        lower_typed_trees(typed).expect_err("a diverging recorded view must be loud");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("recorded ranking view `Slice::Length`")
                && diagnostic.message.contains("resolved view `Nat::Descending`")
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
    use omega_core::semantics::RankingViewId;

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
/// producer. An acyclic claimant derives EventualTerminal without a
/// witness; a proven witness establishes it WITH the resolved explicit
/// view; a machine claiming nothing gets NO fact (its termination story is
/// nobody's to assume).
#[test]
fn termination_facts_record_checked_summaries_and_resolved_views() {
    use omega_core::semantics::TerminationGuarantee;

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
        TerminationGuarantee::EventualTerminal {
            premises: Vec::new()
        }
    );
    assert!(promise.resolved_view_path.is_empty());

    let countdown = facts
        .for_machine(machine_symbol("Main::countdown"))
        .expect("proven witness fact");
    assert_eq!(
        countdown.checked_summary,
        TerminationGuarantee::EventualTerminal {
            premises: Vec::new()
        }
    );
    assert_eq!(countdown.resolved_view_path, "Nat::Descending");

    // A machine claiming nothing carries NO fact.
    assert!(facts.for_machine(machine_symbol("Main::main")).is_none());
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
    use omega_core::semantics::TerminationGuarantee;

    let source = r#"
    trait Worker {
        machine run(&mut self, n: u64) -> u64 terminates;
    }

    data Main {}

    machine Main::run(&mut self, n: u64) -> u64 satisfies Worker {
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
        run.termination_plan.published,
        Some(TerminationGuarantee::EventualTerminal {
            premises: Vec::new()
        }),
        "the implementation inherits the requirement's published guarantee"
    );
    assert!(run.termination_plan.implementation_witness.is_none());

    lower_typed_trees(typed).expect("an acyclic inheritor discharges the claim for free");
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

    machine Main::run(&mut self, n: u64) -> u64 satisfies Worker
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

/// STR4 slice 1 (decision 22): the normalized kinded effect row propagates
/// -- populated ONCE at syntax->resolved from the flat effects span
/// (order/duplicate-blind), copied with its interner into the typed trees.
/// Same member set -> same row id; no effects -> the fixed EMPTY row.
#[test]
fn machine_effect_rows_normalize_and_propagate() {
    use omega_core::semantics::{EffectRowTable, effect_member_id};

    let source = r#"
    data Main {}

    machine Main::alpha(&mut self) effects filesystem_io, clock_read { 1 }
    machine Main::beta(&mut self) effects clock_read, filesystem_io { 2 }
    machine Main::plain(&mut self) -> u64 { 3 }

    machine Main::main(&mut self) {
        let a: u64 = self.alpha();
        let b: u64 = self.beta();
        let c: u64 = self.plain();
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let row_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .effect_row
    };

    // Spelling order is identity-blind: alpha and beta share one row.
    let alpha = row_of("Main::alpha");
    assert_eq!(alpha, row_of("Main::beta"));
    assert!(alpha.is_valid());
    assert_ne!(alpha, EffectRowTable::EMPTY_ROW);

    // The interner traveled with the trees: the row's members are the
    // canonical ids, sorted.
    let filesystem = effect_member_id("filesystem_io").expect("catalog");
    let clock = effect_member_id("clock_read").expect("catalog");
    let mut expected = vec![filesystem, clock];
    expected.sort_by_key(|member| member.0);
    assert_eq!(typed.effect_rows.members(alpha), expected.as_slice());

    // No effects -> the fixed empty row (never NULL: it was computed).
    assert_eq!(row_of("Main::plain"), EffectRowTable::EMPTY_ROW);
    assert_eq!(row_of("Main::main"), EffectRowTable::EMPTY_ROW);
}

/// STR4 slice 2 (decision 22): the checked facts split the PUBLISHED
/// ceiling (the authored `effects` clause) from the checker-INFERRED
/// direct/transitive summaries, all as normalized kinded row identities.
/// A ceiling wider than the body's reality is visible as row inequality.
#[test]
fn effect_row_facts_split_ceiling_from_inferred_summaries() {
    use omega_core::semantics::EffectRowTable;

    let source = r#"
    data Main {}

    machine Main::quiet(&mut self) -> u64 effects filesystem_io { 1 }

    machine Main::main(&mut self) -> u64 {
        let a: u64 = self.quiet();
        a
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
    let quiet_symbol = symbol_of("Main::quiet");
    let main_symbol = symbol_of("Main::main");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    // quiet: the authored ceiling names filesystem_io while the BODY
    // observes nothing -- the declaration-free inferred direct (STR4 slice
    // 3) is the fixed EMPTY row, visibly different from the ceiling.
    let quiet = checked
        .facts
        .effect_rows
        .for_machine(quiet_symbol)
        .expect("quiet's effect-row fact");
    assert_ne!(quiet.published_ceiling, EffectRowTable::EMPTY_ROW);
    assert_eq!(quiet.inferred_direct, EffectRowTable::EMPTY_ROW);
    assert_ne!(quiet.published_ceiling, quiet.inferred_direct);
    assert_eq!(
        checked.facts.effect_rows.rows.members(quiet.published_ceiling),
        &[omega_core::semantics::effect_member_id("filesystem_io").expect("catalog")]
    );

    // main: NO authored clause (ceiling = the fixed EMPTY row) but the
    // TRANSITIVE summary reaches quiet's filesystem_io -- the published
    // ceiling and the inferred reality are visibly DIFFERENT rows.
    let main = checked
        .facts
        .effect_rows
        .for_machine(main_symbol)
        .expect("main's effect-row fact");
    assert_eq!(main.published_ceiling, EffectRowTable::EMPTY_ROW);
    assert_eq!(main.inferred_transitive, quiet.published_ceiling);
    assert_ne!(main.published_ceiling, main.inferred_transitive);
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
    assert_eq!(
        typed.proof_facts.span_or_empty(ledger.where_facts).len(),
        1
    );
}

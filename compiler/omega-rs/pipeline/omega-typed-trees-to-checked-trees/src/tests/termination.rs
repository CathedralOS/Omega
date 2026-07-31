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
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let machine = typed
        .machines_mut()
        .iter_mut()
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
    machine.decreases = Default::default();
    machine.decrease_order = Default::default();
    machine.decrease_view_arguments = Default::default();
    machine.decrease_range = omega_typed_trees::expression::ExpressionHandle::invalid();
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
        &promise.interface,
        &omega_core::semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new()
        })
    );
    assert!(promise.implementation_witness.is_none());

    // `terminates by remaining;`: witness only (publishes nothing); the
    // single u64 subject's canonical default elaborates immediately.
    let countdown = plan_of("Main::countdown");
    assert_eq!(
        &countdown.interface,
        &omega_core::semantics::TerminationInterface::InternalDerived
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
        &omega_core::semantics::TerminationInterface::InternalDerived
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

/// TPR3 completion: checker legality resolves subjects, view arguments, view
/// identity, and rank range from the normalized witness. The old typed-machine
/// spans are compatibility output only and can be cleared without changing the
/// judgment.
#[test]
fn termination_checker_uses_witness_not_compatibility_spans() {
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
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let machine = typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "Main::climb")
        .expect("climb machine");
    machine.decreases = Default::default();
    machine.decrease_order = Default::default();
    machine.decrease_view_arguments = Default::default();
    machine.decrease_range = omega_typed_trees::expression::ExpressionHandle::invalid();

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
    use omega_core::semantics::TerminationGuarantee;

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
        omega_core::semantics::TerminationInterface::InternalDerived,
        "body inference must never redefine the published contract"
    );
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(promised)
            .expect("promised contract plan")
            .termination,
        omega_core::semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
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
        &run.termination_plan.interface,
        &omega_core::semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new()
        }),
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

    machine Main::run(&self) -> u64 satisfies Worker {
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
        &omega_core::semantics::TerminationInterface::Published(
            omega_core::semantics::TerminationGuarantee::NoGuarantee,
        )
    );
    assert_eq!(
        plan_of("Main::local"),
        &omega_core::semantics::TerminationInterface::InternalDerived
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
    use omega_core::semantics::{
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
    use omega_core::semantics::SemanticDomainTable;

    let source = r#"
    data Main {}

    domain i64::Km
    requires
        self >= 0;

    machine Main::clamped(&mut self, value: u64) -> u8 {
        let squeezed: u8 = value as u8 in Saturating;
        squeezed
    }

    machine Main::minted(&mut self) -> i64 {
        let distance: i64 = 5 as i64 in Km;
        distance
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

fn symbol_of_checked(
    checked: &omega_checked_trees::CheckedTrees,
    name: &str,
) -> omega_core::symbols::SymbolHandle {
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

use super::*;

mod guard_facts;

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
fn direct_unsigned_countdown_exports_exact_ranked_scc_evidence() {
    let source = r#"
    data Main {}

    machine Main::countdown(&mut self, remaining: u32)
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
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::countdown")
        .expect("countdown machine");
    let components = crate::checks::termination::proven_nat_countdown_sccs(&typed, machine)
        .expect("the existing ranking proof should export its direct countdown");
    let [component] = components.as_slice() else {
        panic!("one ranked SCC")
    };
    assert_eq!(
        component.header_state,
        typed.machine_states(machine)[0].symbol
    );
    assert_eq!(component.header_rank_parameter_position, 1);
    assert_eq!(
        component.rank_primitive_type,
        typed_trees::types::PrimitiveType::U32
    );
    assert_eq!(component.rank_lower_bound, 0);
    assert_eq!(component.rank_upper_bound, u128::from(u32::MAX));
    let [edge] = component.covered_cyclic_edges.as_slice() else {
        panic!("one covered backedge")
    };
    assert_eq!(edge.source_state, component.header_state);
    assert_eq!(edge.target_state, component.header_state);
    assert_eq!(edge.statement_ordinal, 0);
    assert_eq!(edge.source_rank_parameter_position, 1);
    assert_eq!(edge.target_rank_parameter_position, 1);

    let stalled = source.replace("remaining - 1", "remaining");
    let tokens = Lexer::new(&stalled)
        .tokenize()
        .expect("stalled source should tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("stalled source should parse");
    let resolved = lower_syntax_trees(&syntax).expect("stalled source should resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("stalled source should type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::countdown")
        .expect("stalled countdown machine");
    assert!(
        crate::checks::termination::proven_nat_countdown_sccs(&typed, machine).is_none(),
        "forwarding the original rank must not export ranked-SCC evidence"
    );
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
fn checked_termination_plans_record_summaries_and_resolved_views() {
    use language_semantics::TerminationGuarantee;

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

    let machine = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
    };

    let promise = crate::checks::termination::build_checked_termination_plan(
        &typed,
        machine("Main::promise"),
    );
    assert_eq!(
        promise.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert!(promise.implementation_witness.is_none());

    let countdown = crate::checks::termination::build_checked_termination_plan(
        &typed,
        machine("Main::countdown"),
    );
    assert_eq!(
        countdown.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert_eq!(
        countdown
            .implementation_witness
            .as_ref()
            .expect("proven witness")
            .view_path,
        "Nat::Descending"
    );

    let inferred =
        crate::checks::termination::build_checked_termination_plan(&typed, machine("Main::main"));
    assert_eq!(
        inferred.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        }
    );
    assert!(inferred.implementation_witness.is_none());
}

#[test]
fn checked_proof_scc_retains_every_exact_structural_subterm_call_site() {
    let source = r#"
    data ProofTree {
        case Leaf;
        case Branch(first: ProofTree, second: ProofTree);
    }

    data Main {}
    machine Main::main(&mut self) {}

    machine left(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: right(first),
                second: right(second),
            }
        }
    }

    machine right(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: left(first),
                second: left(second),
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
    let machine_symbol = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let left = machine_symbol("left");
    let right = machine_symbol("right");
    let checked = lower_typed_trees(typed).expect("measured proof SCC should check");

    let [component] = checked
        .facts
        .termination
        .proof_recursive_components
        .as_slice()
    else {
        panic!("one proof SCC should be retained")
    };
    assert_eq!(
        component.ranking_relation,
        checked_trees::CheckedProofRankingRelation::StructuralSubterm
    );
    assert!(component.rank_type_identity.contains("ProofTree"));
    assert_eq!(
        component
            .members
            .iter()
            .map(|member| member.machine)
            .collect::<Vec<_>>(),
        vec![left, right]
    );
    assert_eq!(component.edges.len(), 4);
    assert_eq!(
        component
            .edges
            .iter()
            .filter(|edge| edge.caller == left && edge.callee == right)
            .count(),
        2,
        "two exact calls between the same machine pair must not collapse"
    );
    assert_eq!(
        component
            .edges
            .iter()
            .filter(|edge| edge.caller == right && edge.callee == left)
            .count(),
        2,
        "the reverse pair must retain both exact calls too"
    );
    assert!(component.edges.iter().all(|edge| matches!(
        edge.site,
        checked_trees::CheckedProofRecursiveCallSite::Expression { .. }
    )));
    let exact_sites = component
        .edges
        .iter()
        .map(|edge| match edge.site {
            checked_trees::CheckedProofRecursiveCallSite::Expression {
                state,
                statement_index,
                expression_ordinal,
            } => (
                state.arena_index(),
                state.generation(),
                statement_index,
                expression_ordinal,
            ),
            _ => unreachable!("all retained sites are expression calls"),
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        exact_sites.len(),
        4,
        "checked coordinates must distinguish every recursive call without arena expression handles"
    );
    assert!(
        component
            .edges
            .iter()
            .all(|edge| edge.strict_member_path.len() == 1)
    );
    let unique_paths = component
        .edges
        .iter()
        .map(|edge| {
            edge.strict_member_path
                .iter()
                .map(|member| (member.arena_index(), member.generation()))
                .collect::<Vec<_>>()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        unique_paths.len(),
        2,
        "the two payload declarations must remain distinct exact witnesses"
    );
}

#[test]
fn checked_singleton_proof_scc_retains_its_exact_self_edge() {
    let source = r#"
    data ProofTree {
        case Leaf;
        case Branch(first: ProofTree, second: ProofTree);
    }

    machine descend(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: descend(first),
                second: second,
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
    let descend = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "descend")
        .expect("descend machine")
        .symbol;
    let checked = lower_typed_trees(typed).expect("measured self recursion should check");

    let [component] = checked
        .facts
        .termination
        .proof_recursive_components
        .as_slice()
    else {
        panic!("one singleton proof SCC should be retained")
    };
    assert_eq!(component.members.len(), 1);
    assert_eq!(component.members[0].machine, descend);
    assert_eq!(component.edges.len(), 1);
    assert_eq!(component.edges[0].caller, descend);
    assert_eq!(component.edges[0].callee, descend);
    assert_eq!(component.edges[0].strict_member_path.len(), 1);
    assert!(matches!(
        component.edges[0].site,
        checked_trees::CheckedProofRecursiveCallSite::Expression { .. }
    ));
}

#[test]
fn inferred_completion_never_publishes_a_promise() {
    use language_semantics::TerminationGuarantee;

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
            .termination
            .for_machine(inferred)
            .expect("inferred contract plan")
            .interface,
        language_semantics::TerminationInterface::InternalDerived,
        "body inference must never redefine the published contract"
    );
    assert_eq!(
        checked
            .facts
            .termination
            .for_machine(promised)
            .expect("promised contract plan")
            .interface,
        language_semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
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
    use language_semantics::TerminationGuarantee;

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
        &language_semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
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
        &language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::NoGuarantee,
        )
    );
    assert_eq!(
        plan_of("Main::local"),
        &language_semantics::TerminationInterface::InternalDerived
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

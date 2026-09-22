use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::{checked_program_result, typed_program};

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

    let diagnostics = checked_program_result(source).expect_err("termination check should fail");

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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("checked lowering should accept ranges");
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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("termination check should succeed");
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

    let typed = typed_program(source);
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
    let typed = typed_program(&stalled);
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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("termination distance proof should succeed");
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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("termination slice distance proof should succeed");
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

    let diagnostics = checked_program_result(source).expect_err("termination check should fail");

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

    let diagnostics = checked_program_result(source).expect_err("termination check should fail");

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

    let diagnostics = checked_program_result(source).expect_err("termination check should fail");

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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("termination slice length proof should succeed");
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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("mutual recursion decrease proof should succeed");
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

    let diagnostics = checked_program_result(source).expect_err("termination check should fail");

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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("default nat-descending inference should succeed");
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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("default slice-length inference should succeed");
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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("default bounded-distance inference should succeed");
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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("explicit named bounded-distance view should prove");
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

    let diagnostics = checked_program_result(source).expect_err("inverted distance should fail");

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

    let typed = typed_program(source);
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("the subtraction spelling is retired surface");

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

    let diagnostics =
        checked_program_result(source).expect_err("the view ranks a (lower, upper) pair only");

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

    let diagnostics =
        checked_program_result(source).expect_err("ambiguous default order should fail");

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

    let typed = typed_program(source);

    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("default nat-descending inference should cover u32");
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

    let diagnostics =
        checked_program_result(source).expect_err("a unique declared measure must not be inferred");

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

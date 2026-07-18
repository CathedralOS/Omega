use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

#[test]
fn rejects_public_termination_guarantee_without_private_ranking_witness() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64) terminates; {
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
fn inherits_public_termination_guarantee_from_satisfied_requirement() {
    let source = r#"
    trait Countdown {
        machine countdown(remaining: u64) -> u64 terminates;
    }

    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = countdown(3);
    }

    machine countdown(remaining: u64) -> u64
    satisfies Countdown::countdown
    terminates by remaining;
    {
        transition remaining > 0 {
            true -> countdown(remaining - 1)
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
    let authored = typed
        .machines()
        .iter()
        .find(|machine| machine.ranking_witness.is_present())
        .expect("satisfier should exist");
    assert!(!authored.termination_guarantee.is_eventual_terminal());

    let checked = lower_typed_trees(typed).expect("inherited guarantee should be discharged");
    let normalized = checked
        .machines()
        .iter()
        .find(|machine| machine.ranking_witness.is_present())
        .expect("satisfier should exist");
    assert!(normalized.termination_guarantee.is_eventual_terminal());
    assert!(normalized.ranking_witness.is_present());
}

#[test]
fn acyclic_satisfier_inherits_guarantee_without_any_termination_clause() {
    let source = r#"
    trait Value {
        machine value() -> u64 terminates;
    }

    data Main {}

    machine Main::main(&mut self) {
        let result: u64 = value();
    }

    machine value() -> u64
    satisfies Value::value
    {
        7
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let authored = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .expect("satisfier should exist");
    assert!(!authored.termination_guarantee.is_eventual_terminal());
    assert!(!authored.ranking_witness.is_present());

    let checked = lower_typed_trees(typed).expect("acyclic satisfier should discharge guarantee");
    let normalized = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .expect("satisfier should exist");
    assert!(normalized.termination_guarantee.is_eventual_terminal());
    assert!(!normalized.ranking_witness.is_present());
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

    let checked = lower_typed_trees(typed).expect("checked lowering should accept ranges");
    let machine = checked.machines().first().expect("main machine should exist");
    assert!(!machine.termination_guarantee.is_eventual_terminal());
    assert!(
        checked
            .machine_termination_summary(machine.symbol)
            .is_eventual_terminal()
    );
}

#[test]
fn accepts_terminating_countdown_machine_with_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates;
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
    terminates;
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
fn accepts_private_increasing_to_witness_with_rank_range() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
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
    let witness_owner = typed
        .machines()
        .iter()
        .find(|machine| machine.ranking_witness.is_present())
        .expect("walk machine should carry its private witness");
    assert!(!witness_owner.termination_guarantee.is_eventual_terminal());
    assert_eq!(
        typed
            .expression_table
            .expression_handles(witness_owner.ranking_witness.view_arguments)
            .len(),
        1
    );
    assert!(witness_owner.ranking_witness.range.is_present());

    let checked =
        lower_typed_trees(typed).expect("bounded increasing rank should prove termination");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.ranking_witness.is_present())
        .expect("walk machine should exist");
    assert!(
        checked
            .machine_termination_summary(machine.symbol)
            .is_eventual_terminal()
    );
}

#[test]
fn rejects_rank_range_that_excludes_the_natural_floor() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by index -> Nat::IncreasingTo(limit) in 1..=limit;
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
    let diagnostics = lower_typed_trees(typed).expect_err("rank floor must be in range");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove rank range `1..=limit`")
        }),
        "unexpected diagnostics: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn accepts_same_shaped_joint_ranking_across_machine_call_cycle() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> u64 {
        transition { _ -> self.scan_a(4) }
    }

    machine Main::scan_a(&mut self, remaining: u64)
    terminates by remaining;
    -> u64
    {
        transition remaining > 0 {
            true -> self.scan_b(remaining)
            false -> 0
        }
    }

    machine Main::scan_b(&mut self, remaining: u64)
    terminates by remaining;
    -> u64
    {
        transition remaining > 0 {
            true -> self.scan_a(remaining - 1)
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

    lower_typed_trees(typed).expect("joint same-shaped ranking should prove the call SCC");
}

#[test]
fn rejects_same_shaped_machine_call_cycle_without_a_strict_edge() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> u64 {
        transition { _ -> self.scan_a(4) }
    }

    machine Main::scan_a(&mut self, remaining: u64)
    terminates by remaining;
    -> u64
    {
        transition remaining > 0 {
            true -> self.scan_b(remaining)
            false -> 0
        }
    }

    machine Main::scan_b(&mut self, remaining: u64)
    terminates by remaining;
    -> u64
    {
        transition remaining > 0 {
            true -> self.scan_a(remaining)
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
    let diagnostics = lower_typed_trees(typed).expect_err("forwarding-only SCC must fail");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove one joint `terminates by` ranking")
    }));
}

#[test]
fn accepts_same_shaped_lexicographic_machine_call_cycle() {
    let source = r#"
    data Progress {
        outer: u64;
        inner: u64;
    }

    measure Progress::Steps lexicographic { outer, inner }

    data Main {}

    machine Main::main(&mut self) -> u64 {
        transition {
            _ -> self.scan_a(Progress { outer: 1, inner: 4 })
        }
    }

    machine Main::scan_a(&mut self, progress: Progress)
    terminates by progress -> Progress::Steps;
    -> u64
    {
        transition progress.inner > 0 {
            true -> self.scan_b(progress)
            false -> 0
        }
    }

    machine Main::scan_b(&mut self, progress: Progress)
    terminates by progress -> Progress::Steps;
    -> u64
    {
        transition progress.inner > 0 {
            true -> self.scan_a(Progress {
                outer: progress.outer,
                inner: progress.inner - 1,
            })
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

    lower_typed_trees(typed).expect("joint lexicographic ranking should prove the call SCC");
}

#[test]
fn accepts_same_shaped_slice_length_machine_call_cycle() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> u64 {
        let values: [u64; 4] = [1, 2, 3, 4];
        let view: &[u64] = values.as_slice();
        transition { _ -> self.scan_a(view) }
    }

    machine Main::scan_a(&mut self, items: &[u64])
    terminates by items -> Slice::Length;
    -> u64
    {
        transition items.len > 0 {
            true -> self.scan_b(items)
            false -> 0
        }
    }

    machine Main::scan_b(&mut self, items: &[u64])
    terminates by items -> Slice::Length;
    -> u64
    {
        transition items.len > 0 {
            true -> self.scan_a(items[1..])
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

    lower_typed_trees(typed).expect("joint slice-length ranking should prove the call SCC");
}

#[test]
fn accepts_non_tail_joint_cycle_in_proof_stratum() {
    let source = r#"
    data ProofNat {
        case Zero;
        case Succ(prev: ProofNat);
    }

    data Main {}

    machine Main::main(&mut self) {}

    machine left(n: ProofNat)
    terminates by n;
    -> ProofNat
    {
        transition n {
            ProofNat::Zero -> ProofNat::Zero
            ProofNat::Succ { prev } -> ProofNat::Succ { prev: right(prev) }
        }
    }

    machine right(n: ProofNat)
    terminates by n;
    -> ProofNat
    {
        transition n {
            ProofNat::Zero -> ProofNat::Zero
            ProofNat::Succ { prev } -> ProofNat::Succ { prev: left(prev) }
        }
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("proof-only non-tail SCC should use structural ranking");
}

#[test]
fn rejects_non_tail_joint_cycle_in_runtime_stratum() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> u64 {
        transition { _ -> self.left(4) }
    }

    machine Main::left(&mut self, remaining: u64)
    terminates by remaining;
    -> u64
    {
        transition remaining > 0 {
            true -> (1 + self.right(remaining - 1))
            false -> 0
        }
    }

    machine Main::right(&mut self, remaining: u64)
    terminates by remaining;
    -> u64
    {
        transition remaining > 0 {
            true -> (1 + self.left(remaining - 1))
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
    let diagnostics = lower_typed_trees(typed).expect_err("runtime non-tail SCC must fail");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("ranked runtime cycles must be tail-position calls")
    }));
}

#[test]
fn rejects_increasing_to_witness_when_index_stalls() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates by index -> Nat::IncreasingTo(limit);
    -> u64
    {
        transition index < limit {
            true -> self.walk(limit, index)
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
    let diagnostics = lower_typed_trees(typed).expect_err("stalled rank should be rejected");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove `terminates by` ranking witness")
    }));
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
    terminates;
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
    terminates;
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
            .contains("cannot prove `terminates by` ranking witness")
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
    terminates;
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
            .contains("cannot prove `terminates by` ranking witness")
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
    terminates;
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
            .contains("cannot prove `terminates by` ranking witness")
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
    terminates;
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
    terminates;
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
    terminates;
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
            .contains("cannot prove `terminates by` ranking witness")
    }));
}

#[test]
fn elaborates_short_form_nat_witness_without_changing_public_guarantee() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: u64)
    terminates;
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
    let authored = typed
        .machines()
        .iter()
        .find(|machine| machine.ranking_witness.is_present())
        .expect("countdown machine should exist");
    assert!(authored.termination_guarantee.is_eventual_terminal());
    assert!(authored.ranking_witness.is_present());
    assert!(authored.ranking_witness.view.is_empty());

    let checked =
        lower_typed_trees(typed).expect("default nat-descending inference should succeed");
    let elaborated = checked
        .machines()
        .iter()
        .find(|machine| machine.ranking_witness.is_present())
        .expect("countdown machine should exist");
    assert!(elaborated.termination_guarantee.is_eventual_terminal());
    assert_eq!(
        checked
            .machine_decrease_order(elaborated.ranking_witness.view)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["Nat", "Descending"]
    );
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
    terminates;
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
fn does_not_infer_a_default_view_for_two_subject_tuple() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates;
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
    let diagnostics =
        lower_typed_trees(typed).expect_err("a tuple has no carrier-owned canonical default");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove `terminates by` ranking witness for machine")
    }));
}

#[test]
fn accepts_explicit_named_bounded_distance_view() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: u64 = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: u64, index: u64)
    terminates;
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
    terminates;
    terminates by (limit, index) -> Nat::BoundedDistance;
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
                    .contains("write `terminates by (index, limit) -> Nat::BoundedDistance;`")
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
    terminates;
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
                .contains("the use-site subtraction `terminates by limit - index;`")
                && diagnostic.message.contains("is retired")
                && diagnostic.message.contains(
                    "spell the ranking as `terminates by (index, limit) -> Nat::BoundedDistance;`",
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
    terminates;
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
            .contains("cannot prove `terminates by` ranking witness")
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
    terminates;
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
                .contains("cannot infer a ranking view for `terminates by remaining;`")
                && diagnostic
                    .message
                    .contains("signed values have no default well-founded order")
                && diagnostic
                    .message
                    .contains("`terminates by remaining -> View;`")
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
    terminates;
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
    terminates;
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
                .contains("cannot infer a ranking view for `terminates by card;`")
                && diagnostic
                    .message
                    .contains("declared measures are never selected implicitly")
                && diagnostic
                    .message
                    .contains("`terminates by card -> Card::PowerOrder;`")
        }),
        "expected the declared-measure suggestion diagnostic, got: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn adding_a_second_declared_measure_does_not_reinterpret_short_form() {
    let source = r#"
    data Card {
        power: u64;
    }

    measure Card::PowerOrder(card: Card) -> u64 { card.power }
    measure Card::AlternateOrder(card: Card) -> u64 { card.power }

    data Main {}

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
        lower_typed_trees(typed).expect_err("declared measures must not reinterpret short form");
    let message = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .find(|message| message.contains("cannot infer a ranking view"))
        .expect("ranking diagnostic should be present");

    assert!(message.contains("declared measures are never selected implicitly"));
    assert!(message.contains("Card::AlternateOrder"));
    assert!(message.contains("Card::PowerOrder"));
}

use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

#[test]
fn rejects_terminating_recursive_machine_without_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: usize = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: usize) terminates {
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

    machine Main::main(&mut self) -> usize {
        let values: [usize; 4] = [1, 2, 3, 4];
        let view: &[usize] = values.as_slice();
        let tail: &[usize] = view[1..];
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
        let value: usize = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: usize)
    terminates {
        decreases remaining -> Nat::Descending;
    }
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
        let value: usize = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: usize, index: usize)
    terminates {
        decreases limit - index -> Nat::Descending;
    }
    -> usize
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
        let value: usize = self.walk(view, 0);
    }

    machine Main::walk(&mut self, entries: &[Entry], index: usize)
    terminates {
        decreases entries.len - index -> Nat::Descending;
    }
    -> usize
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
        let value: usize = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: usize)
    terminates {
        decreases remaining -> Nat::Descending;
    }
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
        let value: usize = self.walk(view, 0);
    }

    machine Main::walk(&mut self, entries: &[Entry], index: usize)
    terminates {
        decreases entries.len - index -> Nat::Descending;
    }
    -> usize
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
        let value: usize = self.walk(view, 0);
    }

    machine Main::walk(&mut self, entries: &[Entry], index: usize)
    terminates {
        decreases entries -> Slice::Length;
    }
    -> usize
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
        let value: usize = self.walk(view);
    }

    machine Main::walk(&mut self, entries: &[Entry])
    terminates {
        decreases entries -> Slice::Length;
    }
    -> usize
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

    machine Main::main(&mut self) -> usize {
        transition {
            _ -> self.ping(4)
        }
    }

    machine Main::ping(&mut self, remaining: usize)
    terminates {
        decreases remaining -> Nat::Descending;
    }
    -> usize
    {
        transition remaining > 0 {
            true -> pong(remaining - 1)
            false -> 0
        }

        state pong(&mut self, remaining: usize) -> usize {
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

    machine Main::main(&mut self) -> usize {
        transition {
            _ -> self.ping(4)
        }
    }

    machine Main::ping(&mut self, remaining: usize)
    terminates {
        decreases remaining -> Nat::Descending;
    }
    -> usize
    {
        transition remaining > 0 {
            true -> pong(remaining)
            false -> 0
        }

        state pong(&mut self, remaining: usize) -> usize {
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
        let value: usize = self.countdown(2);
    }

    machine Main::countdown(&mut self, remaining: usize)
    terminates {
        decreases remaining;
    }
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
        let value: usize = self.walk(view);
    }

    machine Main::walk(&mut self, entries: &[Entry])
    terminates {
        decreases entries;
    }
    -> usize
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
fn infers_default_bounded_distance_for_plain_subtraction_decreases() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) {
        let value: usize = self.walk(4, 0);
    }

    machine Main::walk(&mut self, limit: usize, index: usize)
    terminates {
        decreases limit - index;
    }
    -> usize
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
fn rejects_ambiguous_default_order_requiring_explicit_form() {
    let source = r#"
    data Main {}

    machine Main::main(&mut self) -> i32 {
        transition {
            _ -> self.countdown(2)
        }
    }

    machine Main::countdown(&mut self, remaining: i32)
    terminates {
        decreases remaining;
    }
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
    terminates {
        decreases remaining;
    }
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
        power: usize;
    }

    measure Card::PowerOrder(card: Card) -> usize { card.power }

    data Main {
    }

    machine Main::main(&mut self) {
        let value: usize = self.weaken(Card { power: 3 });
    }

    machine Main::weaken(&mut self, card: Card)
    terminates {
        decreases card;
    }
    -> usize
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

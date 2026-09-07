use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

fn float_ordering_declarations(scalar: &str) -> String {
    format!(
        "operator > {scalar}::greater(left: {scalar}, right: {scalar}) -> bool;
         operator < {scalar}::less(left: {scalar}, right: {scalar}) -> bool;
         operator >= {scalar}::greater_or_equal(left: {scalar}, right: {scalar}) -> bool;"
    )
}

fn slice_source(declarations: &str) -> String {
    // Two acyclic guarded edges exercise the console's head/tail staging
    // without introducing a termination obligation into this bounds fixture.
    format!(
        "{declarations}
         machine read(bytes: &[u8]) -> u8 {{
             transition bytes.len > 0 {{
                 true -> next(bytes[0], bytes[1..])
                 false -> 0
             }}
             state next(byte: u8, bytes: &[u8]) -> u8 {{
                 transition bytes.len > 0 {{
                     true -> finish(bytes[0], bytes[1..])
                     false -> byte
                 }}
             }}
             state finish(byte: u8, bytes: &[u8]) -> u8 {{ byte }}
         }}"
    )
}

fn computed_index_source(declarations: &str) -> String {
    format!(
        "{declarations}
         data Cursor {{ depth: u64; stack: [u8; 16]; }}
         machine Cursor::read(&self) -> u8 {{
             transition self.depth >= 1 && self.depth - 1 < 16 {{
                 true -> read_top()
                 false -> 0
             }}
             state read_top(&self) -> u8 {{ self.stack[self.depth - 1] }}
         }}"
    )
}

#[test]
fn builtin_slice_length_guards_prove_head_and_tail_on_both_edges() {
    let source = slice_source("");
    check(&source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

#[test]
fn unrelated_float_ordering_preserves_slice_length_guards() {
    for scalar in ["f32", "f64"] {
        let source = slice_source(&float_ordering_declarations(scalar));
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn selected_integer_greater_cannot_supply_head_or_tail_bounds() {
    let source = slice_source("operator > u64::custom(left: u64, right: u64) -> bool;");
    let diagnostics = check(&source).expect_err("selected user ordering cannot prove bounds");
    for expected in ["cannot prove index", "cannot prove subslice range"] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected}: {source}\n{diagnostics:#?}"
        );
    }
}

#[test]
fn builtin_computed_index_guard_reaches_successor_state() {
    let source = computed_index_source("");
    check(&source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

#[test]
fn unrelated_float_ordering_preserves_computed_index_guard() {
    for scalar in ["f32", "f64"] {
        let source = computed_index_source(&float_ordering_declarations(scalar));
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn unrelated_float_ordering_preserves_explicitly_typed_computed_index_guard() {
    for scalar in ["f32", "f64"] {
        let source = computed_index_source(&float_ordering_declarations(scalar))
            .replace("self.depth - 1", "self.depth - 1u64");
        check(&source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn selected_integer_less_cannot_supply_computed_index_bound() {
    let source = computed_index_source("operator < u64::custom(left: u64, right: u64) -> bool;");
    let diagnostics = check(&source).expect_err("selected user ordering cannot prove bounds");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot prove index")
                && diagnostic.message.contains("within length 16")
        }),
        "expected the missing upper bound: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn selected_integer_subtraction_cannot_supply_computed_index_bound() {
    let source = computed_index_source("operator - u64::custom(left: u64, right: u64) -> u64;");
    let diagnostics =
        check(&source).expect_err("selected subtraction cannot supply builtin bounds");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot prove index")
                && diagnostic.message.contains("within length 16")
        }),
        "expected the missing upper bound: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn heterogeneous_ordering_with_anonymous_literal_cannot_supply_computed_index_bound() {
    // The right-hand 16 has no landed carrier. Recovering the computed left
    // operand's u64 type must not copy that type onto the independent literal
    // and thereby discard this still-participating heterogeneous candidate.
    let source = computed_index_source("operator < u64::custom(left: u64, right: f64) -> bool;");
    let diagnostics =
        check(&source).expect_err("unknown literal cannot exclude an operator candidate");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot prove index")
                && diagnostic.message.contains("within length 16")
        }),
        "expected the missing upper bound: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn nominal_float_len_field_retains_selected_ordering_without_builtin_bound_meaning() {
    for parameter in ["Measurement", "&Measurement"] {
        // Keep a real u64 reference available so a spelling-only length
        // recognizer could wrongly classify this nominal f64 field as u64.
        let source = format!(
            "data Measurement {{ len: f64; }}
             operator > f64::custom(left: f64, right: f64) -> bool;
             machine inspect(value: {parameter}, capacity: u64) -> bool {{
                 value.len > 0.0f64
             }}"
        );
        let checked =
            check(&source).unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
        let comparison = checked
            .facts
            .operators
            .resolved_uses()
            .find(|operator_use| operator_use.spelling == language_core::OperatorSpelling::Greater)
            .expect("nominal f64 field selects its declared greater operator");
        assert_eq!(
            comparison.selected_operator_symbol,
            checked.typed.operators()[0].symbol,
            "the nominal field retains its actual operand type: {source}"
        );
        let machine = &checked.typed.machines()[0];
        let state = &checked.typed.machine_states(machine)[0];
        assert!(
            !validation::has_builtin_bound_expression_meaning(
                &checked.typed,
                machine,
                Some(state),
                comparison.expression,
            ),
            "a field named len is not a builtin collection length: {source}"
        );
    }
}

use crate::tests::front_end::checked_program_result;

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
fn selected_integer_greater_cannot_supply_head_or_tail_bounds() {
    let source = slice_source("operator > u64::custom(left: u64, right: u64) -> bool;");
    let diagnostics =
        checked_program_result(&source).expect_err("selected user ordering cannot prove bounds");
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
fn selected_integer_less_cannot_supply_computed_index_bound() {
    let source = computed_index_source("operator < u64::custom(left: u64, right: u64) -> bool;");
    let diagnostics =
        checked_program_result(&source).expect_err("selected user ordering cannot prove bounds");
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
    let diagnostics = checked_program_result(&source)
        .expect_err("selected subtraction cannot supply builtin bounds");
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
    let diagnostics = checked_program_result(&source)
        .expect_err("unknown literal cannot exclude an operator candidate");
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
        let checked = checked_program_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
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

fn transported_subject_source(write: &str, argument: &str) -> String {
    format!(
        "data Main {{ a: i32 in Wrapping; b: i32 in Wrapping;
             sibling: i32 in Wrapping; grid: [i32; 200] in Wrapping; }}
         machine touch(main: &mut Main) {{ main.sibling = 9; }}
         machine Main::main(main: &mut Main, other: &mut Main) {{
             transition main.b * 20 + main.a >= 0 && main.b * 20 + main.a < 200 {{
                 true -> put({argument})
                 false -> done(main)
             }}
             state put(main: &mut Main) {{ {write} main.grid[main.b * 20 + main.a] = 7; }}
             state done(main: &mut Main) {{}}
         }}"
    )
}

#[test]
fn transported_compound_guard_survives_disjoint_subject_field_writes() {
    for write in ["main.sibling = 9;", "touch(main);"] {
        let source = transported_subject_source(write, "main");
        checked_program_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
    }
}

#[test]
fn transported_compound_guard_retires_when_an_operand_field_changes() {
    for write in ["main.a = 200;", "main.b = 200;"] {
        let source = transported_subject_source(write, "main");
        let diagnostics = checked_program_result(&source).expect_err("the old bound expired");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
            "{source}\n{diagnostics:#?}"
        );
    }
}

#[test]
fn transported_compound_guard_requires_the_exact_subject_argument() {
    let source = transported_subject_source("", "other");
    let diagnostics = checked_program_result(&source).expect_err("a different record has no bound");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
        "{source}\n{diagnostics:#?}"
    );
}

use super::*;

fn check(
    source: &str,
) -> Result<psi_checked_trees::CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed)
}

#[test]
fn mutating_calls_retire_local_index_values() {
    for call in [
        "set_index(&mut index);",
        "let ignored: u64 = set_index_value(&mut index);",
        "let alias: &mut u64 = &mut index; set_index(alias);",
    ] {
        let source = format!(
            r#"
            machine set_index(index: &mut u64) {{ index = 255; }}
            machine set_index_value(index: &mut u64) -> u64 {{ index = 255; 0 }}
            machine main() -> u64 {{
                let values: [u64; 2] = [10, 20];
                let mut index: u64 = 0;
                {call}
                values[index]
            }}
        "#
        );
        let Err(diagnostics) = check(&source) else {
            panic!("stale local index was accepted after {call}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("index")),
            "expected an index rejection after {call}: {diagnostics:#?}"
        );
    }
}

#[test]
fn mutating_array_contents_preserves_its_declared_extent() {
    check(
        r#"
        machine touch(values: &mut [u64; 2]) { values[0] = 9; }
        machine main() -> u64 {
            let mut values: [u64; 2] = [10, 20];
            touch(&mut values);
            values[1]
        }
    "#,
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn readonly_reference_call_preserves_a_live_nonconstant_bound() {
    check(
        r#"
        machine inspect(index: &u64) {}
        machine main(values: &[u64; 2], index: u64) -> u64
        requires index < 2;
        {
            inspect(&index);
            values[index]
        }
    "#,
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn readonly_and_disjoint_calls_preserve_index_values() {
    for call in [
        "inspect(index);",
        "set_index(&mut unrelated);",
        "let ignored: u64 = inspect_value(index);",
        "let ignored: u64 = set_index_value(&mut unrelated);",
    ] {
        let source = format!(
            r#"
            machine inspect(index: u64) {{}}
            machine inspect_value(index: u64) -> u64 {{ index }}
            machine set_index(index: &mut u64) {{ index = 255; }}
            machine set_index_value(index: &mut u64) -> u64 {{ index = 255; 0 }}
            machine main() -> u64 {{
                let values: [u64; 2] = [10, 20];
                let index: u64 = 0;
                let mut unrelated: u64 = 0;
                {call}
                values[index]
            }}
        "#
        );
        check(&source).unwrap_or_else(|diagnostics| panic!("{call}: {diagnostics:#?}"));
    }
}

#[test]
fn mutating_expression_operands_retire_bounds_before_indexing() {
    for value in [
        "combine(set_index(&mut index), values[index])",
        "[set_index(&mut index), values[index]][0]",
    ] {
        let source = format!(
            r#"
            machine set_index(index: &mut u64) -> u64 {{ index = 255; 0 }}
            machine combine(left: u64, right: u64) -> u64 {{ right }}
            machine main() -> u64 {{
                let values: [u64; 2] = [10, 20];
                let mut index: u64 = 0;
                {value}
            }}
        "#
        );
        let Err(diagnostics) = check(&source) else {
            panic!("stale sibling index accepted: {value}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("index")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn indexed_method_statement_receivers_are_bounds_checked() {
    for (index, accepted) in [(0, true), (2, false)] {
        let source = format!(
            r#"
            data Cell {{ value: u64; }}
            machine Cell::touch(&mut self) -> u64 {{ self.value = 1; 0 }}
            machine main() -> u64 {{
                let mut cells: [Cell; 2] = [Cell {{value: 0}}, Cell {{value: 0}}];
                cells[{index}].touch();
                0
            }}
        "#
        );
        let result = check(&source);
        if accepted {
            result.unwrap_or_else(|diagnostics| panic!("valid indexed receiver: {diagnostics:#?}"));
        } else {
            let Err(diagnostics) = result else {
                panic!("out-of-bounds method receiver accepted")
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("index")),
                "{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn calls_retire_collection_relative_guard_bounds() {
    let source = r#"
        machine set_index(index: &mut u64) { index = 255; }
        machine main(values: &[u64], mut index: u64) -> u64 {
            transition index < values.len {
                true -> read(values, index)
                false -> 0
            }
            state read(values: &[u64], mut index: u64) -> u64 {
                set_index(&mut index);
                values[index]
            }
        }
    "#;
    let Err(diagnostics) = check(source) else {
        panic!("stale collection-relative index accepted")
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("index")),
        "{diagnostics:#?}"
    );
}

#[test]
fn mutable_guard_calls_cannot_reestablish_earlier_bounds() {
    for (prefix, guard) in [
        ("", "index < values.len && set_index(&mut index)"),
        (
            "let allowed: bool = index < values.len && set_index(&mut index);",
            "allowed",
        ),
    ] {
        let source = format!(
            r#"
        machine set_index(index: &mut u64) -> bool {{ index = 255; true }}
        machine main(values: &[u64], mut index: u64) -> u64 {{
            {prefix}
            transition {guard} {{
                true -> (values[index])
                _ -> 0
            }}
        }}
    "#
        );
        let Err(diagnostics) = check(&source) else {
            panic!("mutating guard replayed its earlier index bound")
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("index")),
            "{diagnostics:#?}"
        );
    }
}

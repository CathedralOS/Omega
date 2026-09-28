use crate::tests::front_end::checked_program_result;

#[test]
fn effectful_guard_cannot_restore_an_old_bound_in_its_fallback() {
    let source = r#"
        machine corrupt(index: &mut u64) -> bool { index = 255; false }
        machine main(values: &[u64; 4]) -> u64 {
            let mut index: u64 = 0;
            transition (index >= 4) || corrupt(&mut index) {
                true -> 0
                false -> (values[index])
            }
        }
    "#;
    let Err(diagnostics) = checked_program_result(source) else {
        panic!("guard writes must invalidate the earlier index bound before fall-through")
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("index")),
        "{diagnostics:#?}"
    );
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
        let Err(diagnostics) = checked_program_result(&source) else {
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
        let result = checked_program_result(&source);
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

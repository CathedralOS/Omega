use crate::tests::front_end::checked_program_result;

fn accepts(source: &str) {
    checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

fn rejects_ensures(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("unproved result must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{source}: {diagnostics:#?}"
    );
}

#[test]
fn selected_indexed_return_retains_guard_and_full_u64_carrier() {
    accepts(
        r#"
        data Scale {}
        machine Scale::choose(position: u64) -> u64
            requires 0 <= position && position <= 1
            ensures 1 <= result && result <= 18446744073709551615
        {
            let scales: [u64; 2] = [1, 1000];
            transition scales[position] >= 1 {
                true -> (scales[position])
                false -> (1)
            }
        }
    "#,
    );
}

#[test]
fn selected_indexed_return_does_not_inherit_other_reads_or_false_arm() {
    let source = r#"
        machine choose(scales: &[u64; 2], other: &[u64; 2], position: u64, alternate: u64) -> u64
            requires 0 <= position && position < 2 && 0 <= alternate && alternate < 2
            ensures 1 <= result && result <= 18446744073709551615
        {
            transition scales[position] >= 1 {
                true -> (RETURNED)
                false -> (1)
            }
        }
    "#;
    accepts(&source.replace("RETURNED", "scales[position]"));
    for returned in ["other[position]", "scales[alternate]"] {
        rejects_ensures(&source.replace("RETURNED", returned));
    }
    let false_arm = source
        .replace("true -> (RETURNED)", "true -> (1)")
        .replace("false -> (1)", "false -> (scales[position])");
    rejects_ensures(&false_arm);
}

#[test]
fn indexed_return_cannot_reuse_guard_after_array_write() {
    rejects_ensures(
        r#"
        data Table { values: [u64; 2]; }
        machine Table::clear(&mut self) -> bool {
            self.values[0] = 0;
            self.values[1] = 0;
            true
        }
        machine Table::choose(&mut self, position: u64) -> u64
            requires 0 <= position && position < 2
            ensures 1 <= result
        {
            transition self.values[position] >= 1 && self.clear() {
                true -> (self.values[position])
                false -> (1)
            }
        }
    "#,
    );
}

#[test]
fn indexed_return_cannot_reuse_guard_after_selector_write() {
    rejects_ensures(
        r#"
        data Table
        where 0 <= position, position < 2
        { values: [u64; 2]; position: u64; }
        machine Table::advance(&mut self) -> bool { self.position = 1; true }
        machine Table::choose(&mut self) -> u64
            ensures 1 <= result
        {
            transition self.values[self.position] >= 1 && self.advance() {
                true -> (self.values[self.position])
                false -> (1)
            }
        }
    "#,
    );
}

#[test]
fn full_width_carrier_does_not_prove_a_stronger_result_bound() {
    accepts("machine identity(value: u64) -> u64 ensures result <= 18446744073709551615 { value }");
    rejects_ensures(
        "machine identity(value: u64) -> u64 ensures result < 18446744073709551615 { value }",
    );
}

#[test]
fn selected_operator_meanings_do_not_supply_repeated_builtin_reads() {
    for declaration in [
        "boundary operator >= Meaning::compare(left: u64, right: u64) -> bool;",
        "boundary operator [] Meaning::read(values: &[u64; 2], index: u64) -> u64;",
    ] {
        let source = format!("{declaration}\n")
            + r#"
            machine choose(values: &[u64; 2], position: u64) -> u64
                requires 0 <= position && position < 2
                ensures 1 <= result
            {
                transition values[position] >= 1 {
                    true -> (values[position])
                    false -> (1)
                }
            }
        "#;
        rejects_ensures(&source);
    }
}

#[test]
fn captured_predicate_requires_continuous_storage_preservation() {
    let source = r#"
        data Table
        where 0 <= position, position < 2
        { values: [u64; 2]; position: u64; }
        machine Table::choose(&mut self) -> u64
            ensures 1 <= result
        {
            let selected: bool = self.values[self.position] >= 1;
            INTERVENING
            transition selected {
                true -> (self.values[self.position])
                false -> (1)
            }
        }
    "#;
    accepts(&source.replace("INTERVENING", ""));
    for intervening in ["self.values[0] = 0;", "self.position = 1;"] {
        rejects_ensures(&source.replace("INTERVENING", intervening));
    }
}

#[test]
fn intervening_call_operands_retain_selected_meaning_fence() {
    let source = r#"
        SELECTED_OPERATOR
        machine observe(value: u64) -> u64 { value }
        machine choose(scales: &[u64; 2], position: u64) -> u64
            requires 0 <= position && position < 2
            ensures 1 <= result
        {
            let selected: bool = scales[position] >= 1;
            _ = observe(OPERAND);
            transition selected {
                true -> (scales[position])
                false -> (1)
            }
        }
    "#;
    accepts(
        &source
            .replace("SELECTED_OPERATOR", "")
            .replace("OPERAND", "1"),
    );
    rejects_ensures(
        &source
            .replace(
                "SELECTED_OPERATOR",
                "boundary operator + Meaning::add(left: u64, right: u64) -> u64;",
            )
            .replace("OPERAND", "1 + 2"),
    );
}

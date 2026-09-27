use crate::tests::front_end::checked_program_result;

fn accepts(source: &str) {
    checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

fn rejects_requirement(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("requirement must remain unproved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{source}: {diagnostics:#?}"
    );
}

#[test]
fn unsigned_floor_composes_with_exact_slice_guard() {
    for requirement in ["0 <= at && at < values.len", "at < values.len && 0 <= at"] {
        let source = r#"
            machine lookup(values: &[u8], index: u64) -> u8 {
                transition index < values.len {
                    true -> read(values, index)
                    false -> (0)
                }
                state read(values: &[u8], at: u64) -> u8
                requires {requirement}
                { values[at] }
            }
        "#
        .replace("{requirement}", requirement);
        accepts(&source);
    }
}

#[test]
fn conjuncts_do_not_reuse_a_different_slice_or_a_disjunctive_guard() {
    for (guard, actual) in [
        ("index < values.len", "other"),
        ("index < values.len || index == values.len", "values"),
    ] {
        let source = r#"
            machine lookup(values: &[u8], other: &[u8], index: u64) -> u8 {
                transition {guard} {
                    true -> read({actual}, index)
                    false -> (0)
                }
                state read(values: &[u8], at: u64) -> u8
                requires 0 <= at && at < values.len
                { values[at] }
            }
        "#
        .replace("{guard}", guard)
        .replace("{actual}", actual);
        rejects_requirement(&source);
    }
}

#[test]
fn conjunction_does_not_recover_an_invalidated_arrival_bound() {
    rejects_requirement(
        r#"
        machine lookup(values: &[u8], index: u64) -> u8 {
            transition index < values.len {
                true -> change(values, index)
                false -> (0)
            }
            state change(values: &[u8], mut index: u64) -> u8 {
                index = values.len;
                transition { _ -> read(values, index) }
            }
            state read(values: &[u8], at: u64) -> u8
            requires 0 <= at && at < values.len
            { values[at] }
        }
    "#,
    );
}

#[test]
fn signed_carrier_does_not_supply_an_unsigned_floor() {
    rejects_requirement(
        r#"
        machine lookup(value: i64, ceiling: i64) -> i64 {
            transition value < ceiling {
                true -> read(value, ceiling)
                false -> (0)
            }
            state read(value: i64, ceiling: i64) -> i64
            requires 0 <= value && value < ceiling
            { value }
        }
    "#,
    );
}

#[test]
fn immutable_arrival_range_survives_a_call() {
    accepts(
        r#"
data Leaf { count: u64; }
machine Leaf::count(&self) -> u64 { self.count }
data Main { leaves: [Leaf; 1]; }
machine Main::leaf(&self) -> Leaf { Leaf { count: self.leaves[0].count } }
machine Main::main(&self) -> u64 {
    transition { _ -> tally(0) }
    state tally(&self, position: u64) -> u64
        requires 0 <= position && position < 1
    {
        let count: u64 = self.leaf().count();
        transition { _ -> done(position + 1) }
    }
    state done(&self, position: u64) -> u64 { position }
}
    "#,
    );
}

#[test]
fn mutable_arrival_range_is_not_recovered_after_write_and_call() {
    let source = r#"
data Leaf { count: u64; }
machine Leaf::count(&self) -> u64 { self.count }
data Main { leaves: [Leaf; 1]; }
machine Main::leaf(&self) -> Leaf { Leaf { count: self.leaves[0].count } }
machine Main::main(&self) -> u64 {
    transition { _ -> tally(0) }
    state tally(&self, mut position: u64) -> u64
        requires 0 <= position && position < 1
    {
        position = 18446744073709551615;
        let count: u64 = self.leaf().count();
        transition { _ -> done(position + 1) }
    }
    state done(&self, position: u64) -> u64 { position }
}
    "#;
    let diagnostics = checked_program_result(source).expect_err("mutable value overflows");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow `u64`")),
        "{source}: {diagnostics:#?}"
    );
}

#[test]
fn selected_arm_conjunct_keeps_operand_write_order() {
    let setup = r#"
        data Store { limit: u16; }
        machine Store::clear(&mut self) -> u16 { self.limit = 99; 0 }
        machine Store::noop(&mut self) -> u16 { 0 }
        machine Store::done(&mut self, ignored: u16, value: u16) -> u16
        requires 0 <= value && value <= 8
        { value }
        machine Store::scan(&mut self) -> u16 {
            transition self.limit <= 8 {
                true -> (self.done(self.OPERATION(), self.limit))
                false -> (0)
            }
        }
    "#;
    accepts(&setup.replace("OPERATION", "noop"));
    rejects_requirement(&setup.replace("OPERATION", "clear"));
}

#[test]
fn self_field_carrier_and_guard_prove_closed_arrival_interval() {
    let source = r#"
        data Store { length: u64; }
        machine Store::lookup(&mut self) -> u64 {
            transition self.length < CEILING {
                true -> read(self.length)
                false -> (0)
            }
            state read(&mut self, at: u64) -> u64
            requires 0 <= at && at <= 15
            { at }
        }
    "#;
    accepts(&source.replace("CEILING", "16"));
    rejects_requirement(&source.replace("CEILING", "17"));
}

#[test]
fn an_open_u64_guard_does_not_gain_a_signed_ceiling() {
    rejects_requirement(
        r#"
        data Store { length: u64; }
        machine Store::lookup(&mut self) -> u64 {
            transition self.length >= 0 {
                true -> read(self.length)
                false -> (0)
            }
            state read(&mut self, at: u64) -> u64
            requires 0 <= at && at <= 9223372036854775807
            { at }
        }
    "#,
    );
}

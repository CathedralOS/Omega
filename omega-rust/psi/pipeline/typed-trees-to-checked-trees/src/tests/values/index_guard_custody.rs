use super::*;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_trees(source)) {
        Ok(_) => assert!(accepted, "unproved index accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("index") && diagnostic.message.contains("prove")
                }),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

fn read_source(declaration: &str, guard: &str) -> String {
    format!(
        "{declaration} machine read(items: &[u8], index: u64) -> u8 {{
        transition {{ {guard} -> (items[index]) _ -> 0 }}
    }}"
    )
}

#[test]
fn builtin_index_guards_keep_their_selected_meaning() {
    for declaration in [
        "",
        "operator < f64::unrelated(left: f64, right: f64) -> bool;",
    ] {
        check(&read_source(declaration, "index < items.len"), true);
    }
}

#[test]
fn authored_ordering_cannot_supply_an_index_bound() {
    check(
        &read_source(
            "operator < u64::custom(left: u64, right: u64) -> bool;",
            "index < items.len",
        ),
        false,
    );
}

#[test]
fn authored_boolean_equality_cannot_expose_an_inner_index_bound() {
    check(
        "operator == bool::custom(left: bool, right: bool) -> bool;
        machine read(items: &[u8], index: u64) -> u8 {
            transition (index < items.len) == true {
                true -> (items[index]) false -> 0
            }
        }",
        false,
    );
}

fn loop_source(declaration: &str) -> String {
    format!(
        "{declaration}
        data Main {{ items: [u8; 4]; index: i32 in Wrapping; }}
        machine Main::run(&mut self) {{
            self.index = 0;
            transition {{ _ -> fill() }}
            state fill(&mut self) {{
                let value: u8 = self.items[self.index];
                self.index = self.index + 1;
                transition self.index < 4 {{ true -> fill() _ -> {{}} }}
            }}
        }}"
    )
}

#[test]
fn builtin_loop_guards_establish_the_write_first_index_bound() {
    check(&loop_source(""), true);
}

#[test]
fn authored_loop_guard_equality_cannot_establish_an_inductive_bound() {
    check(
        &loop_source("operator == bool::custom(left: bool, right: bool) -> bool;"),
        false,
    );
}

#[test]
fn loop_bounds_retain_comparison_and_increment_meaning() {
    for declaration in [
        "operator < i32::custom(left: i32 in Wrapping, right: i32 in Wrapping) -> bool;",
        "operator + i32::custom(left: i32 in Wrapping, right: i32 in Wrapping) -> i32 in Wrapping;",
    ] {
        check(&loop_source(declaration), false);
    }
}

#[test]
fn failed_guard_bounds_use_the_original_comparison_meaning() {
    for (declaration, accepted) in [
        ("", true),
        (
            "operator >= u64::custom(left: u64, right: u64) -> bool;",
            false,
        ),
        (
            "operator < u64::unrelated(left: u64, right: u64) -> bool;",
            true,
        ),
    ] {
        check(
            &format!(
                "{declaration}
            machine read(items: &[u8], index: u64) -> u8 {{
                transition {{ index >= items.len -> 0 _ -> (items[index]) }}
            }}"
            ),
            accepted,
        );
    }
}

#[test]
fn index_guard_constant_arithmetic_retains_its_own_meaning() {
    for (declaration, accepted) in [
        ("", true),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;",
            false,
        ),
        (
            "operator + f64::unrelated(left: f64, right: f64) -> f64;",
            true,
        ),
    ] {
        check(
            &format!(
                "{declaration}
            machine read(items: &[u8; 4], index: u64) -> u8 {{
                transition {{ index < 2u64 + 2u64 -> (items[index]) _ -> 0 }}
            }}"
            ),
            accepted,
        );
    }
}

#[test]
fn independent_builtin_conjuncts_keep_their_index_evidence() {
    for transition in [
        "transition { flag > 0.0f64 && index < items.len -> (items[index]) _ -> 0 }",
        "transition flag > 0.0f64 && index < items.len { true -> (items[index]) _ -> 0 }",
        "transition { flag > 0.0f64 || index >= items.len -> 0 _ -> (items[index]) }",
    ] {
        check(
            &format!(
                "operator > f64::custom(left: f64, right: f64) -> bool;
            machine read(items: &[u8], index: u64, flag: f64) -> u8 {{ {transition} }}"
            ),
            true,
        );
    }
}

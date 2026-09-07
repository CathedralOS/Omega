use super::{assert_call_requirement_rejected, checked};

#[test]
fn unchanged_mutable_scalar_inputs_satisfy_stronger_call_requirements() {
    for call in ["demand(input);", "transition { _ -> demand(input) }"] {
        let source = format!(
            r#"
            machine demand(mut value: u64) requires value >= 1 {{}}
            machine caller(mut input: u64) requires input >= 2 {{ {call} }}
        "#
        );
        checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn mutable_scalar_call_uses_fresh_guard_after_assignment() {
    let source = r#"
        machine demand(mut value: u64) -> u64 requires value >= 1 { value }
        machine caller(mut input: u64, other: u64) -> u64 requires input >= 2 {
            input = other;
            transition input >= 1 {
                true -> demand(input)
                false -> 0
            }
        }
    "#;
    checked(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn direct_and_aliased_mutations_retire_mutable_entry_requirements() {
    for prefix in [
        "input = 0;",
        "let alias: &mut u64 = &mut input; alias = 0;",
        "overwrite(&mut input);",
    ] {
        let source = format!(
            r#"
            machine overwrite(value: &mut u64) {{ value = 0; }}
            machine demand(value: u64) requires value >= 1 {{}}
            machine caller(mut input: u64) requires input >= 2 {{
                {prefix}
                transition {{ _ -> demand(input) }}
            }}
        "#
        );
        assert_call_requirement_rejected(&source);
    }
}

#[test]
fn scalar_actuals_captured_across_an_effect_do_not_share_current_symbol_identity() {
    for call in [
        "demand(input, overwrite(&mut input), input);",
        "transition { _ -> demand(input, overwrite(&mut input), input) }",
    ] {
        let source = format!(
            r#"
            machine overwrite(value: &mut u64) -> bool {{ value = 0; true }}
            machine demand(left: u64, ignored: bool, right: u64)
            requires left <= right && right <= left {{}}
            machine caller(mut input: u64) {{ {call} }}
        "#
        );
        assert_call_requirement_rejected(&source);
    }
}

#[test]
fn disjoint_operand_effect_preserves_captured_mutable_scalar_identity() {
    let source = r#"
        machine overwrite(value: &mut u64) -> bool { value = 0; true }
        machine demand(left: u64, ignored: bool, right: u64)
        requires left <= right && right <= left {}
        machine caller(mut input: u64, mut other: u64) {
            transition { _ -> demand(input, overwrite(&mut other), input) }
        }
    "#;
    checked(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn later_operand_alias_writes_distinguish_captured_and_disjoint_scalars() {
    for (aliased, accepted) in [("input", false), ("other", true)] {
        let source = format!(
            r#"
            machine overwrite(value: &mut u64) -> bool {{ value = 0; true }}
            machine demand(left: u64, ignored: bool, right: u64)
            requires left <= right && right <= left {{}}
            machine caller(mut input: u64, mut other: u64) {{
                let alias: &mut u64 = &mut {aliased};
                transition {{ _ -> demand(input, overwrite(alias), input) }}
            }}
        "#
        );
        if accepted {
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        } else {
            assert_call_requirement_rejected(&source);
        }
    }
}

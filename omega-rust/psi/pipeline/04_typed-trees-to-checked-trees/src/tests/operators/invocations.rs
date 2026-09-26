use crate::tests::front_end::checked_program_result;

#[test]
fn operator_requires_use_the_selected_short_circuit_branch() {
    for (connective, accepted) in [("&&", true), ("||", false)] {
        let checked = checked_program_result(&format!(
            "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left >= 0;
             machine compare(value: i32, pattern: i32) -> bool {{
                 value >= 0 {connective} (value == pattern)
             }}"
        ));
        assert_eq!(
            checked.is_ok(),
            accepted,
            "{connective}: {:?}",
            checked.err()
        );
    }
}

#[test]
fn operator_operand_custody_rejects_missing_duplicate_or_reordered_records() {
    let checked = checked_program_result(
        "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left >= 0;
         machine compare(value: i32, pattern: i32) -> bool requires value >= 0 { value == pattern }",
    ).unwrap();
    crate::checks::check_checked_facts(&checked.typed, &checked.facts).unwrap();
    let (invocation_handle, invocation) = checked
        .facts
        .flow
        .control
        .operator_invocations
        .iter()
        .next()
        .unwrap();
    for corruption in ["missing", "duplicate", "operands", "reordered"] {
        let mut facts = checked.facts.clone();
        match corruption {
            "missing" => facts.flow.control.operator_invocations = Default::default(),
            "duplicate" => {
                facts
                    .flow
                    .control
                    .operator_invocations
                    .append(invocation.clone());
            }
            "operands" => facts.flow.control.operator_operands = Default::default(),
            "reordered" => {
                let reversed = facts
                    .flow
                    .control
                    .operator_operands
                    .span_or_empty(invocation.operands)
                    .iter()
                    .rev()
                    .copied()
                    .collect::<Vec<_>>();
                let span = facts.flow.control.operator_operands.insert_many(reversed);
                facts
                    .flow
                    .control
                    .operator_invocations
                    .get_mut(invocation_handle)
                    .operands = span;
            }
            _ => unreachable!(),
        }
        let diagnostics =
            crate::checks::check_checked_facts(&checked.typed, &facts).expect_err(corruption);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("requires")),
            "{corruption}: {diagnostics:#?}"
        );
    }
}

#[test]
fn multi_operand_requires_cannot_mix_different_versions_of_a_source() {
    for (argument, accepted) in [("right", true), ("replace(&mut right)", false)] {
        let checked = checked_program_result(&format!(
            "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left <= right;
             machine replace(value: &mut i32) -> i32 {{ value = -1; value }}
             machine compare(left: i32, mut right: i32) -> bool requires left <= right {{
                 left == {argument}
             }}"
        ));
        assert_eq!(checked.is_ok(), accepted, "{argument}: {:?}", checked.err());
    }
}

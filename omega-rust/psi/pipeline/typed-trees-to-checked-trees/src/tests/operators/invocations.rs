use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize operator invocation");
    let syntax = parse_syntax_trees(&tokens).expect("parse operator invocation");
    let resolved = lower_syntax_trees(&syntax).expect("resolve operator invocation");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type operator invocation");
    lower_typed_trees(typed)
}

#[test]
fn operator_requires_can_use_facts_established_by_an_earlier_operand() {
    check(
        "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires right >= 0;
         machine prepare(value: &mut i32) -> i32 ensures value >= 0 { value = 1; 0 }
         machine compare(mut value: i32) -> bool { prepare(&mut value) == value }",
    )
    .expect("the right operand is read after prepare establishes its precondition");
}

#[test]
fn operator_requires_cannot_use_facts_invalidated_by_an_earlier_operand() {
    let diagnostics = check(
        "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires right >= 0;
         machine reset(value: &mut i32) -> i32 { value = -1; 0 }
         machine compare(mut value: i32) -> bool requires value >= 0 {
             reset(&mut value) == value
         }",
    )
    .expect_err("statement-entry facts do not describe the post-reset right operand");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn operator_requires_keep_a_copied_left_value_after_a_right_operand_write() {
    check(
        "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left >= 0;
         machine reset(value: &mut i32) -> i32 { value = -1; 0 }
         machine compare(mut value: i32) -> bool requires value >= 0 {
             value == reset(&mut value)
         }",
    )
    .expect("a later write cannot revoke facts about the already-copied left value");
}

#[test]
fn operator_requires_cannot_give_an_earlier_copy_a_later_storage_guarantee() {
    let diagnostics = check(
        "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left >= 0;
         machine prepare(value: &mut i32) -> i32 ensures value >= 0 { value = 1; 0 }
         machine compare(mut value: i32) -> bool { value == prepare(&mut value) }",
    )
    .expect_err("prepare's guarantee describes current storage, not the earlier left copy");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn operator_requires_use_the_selected_short_circuit_branch() {
    for (connective, accepted) in [("&&", true), ("||", false)] {
        let checked = check(&format!(
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
    let checked = check(
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
fn operator_requires_execute_inside_assignment_targets_after_the_value() {
    for (right_hand_side, accepted) in [("1", true), ("reset(&mut value)", false)] {
        let checked = check(&format!(
            "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left >= 0;
             machine choose_index(flag: bool) -> u64 [0..=1] {{ 0 }}
             machine reset(value: &mut i32) -> u64 {{ value = -1; 1 }}
             machine store(cells: &mut [u64; 2], mut value: i32) requires value >= 0 {{
                 cells[choose_index(value == 0)] = {right_hand_side};
             }}"
        ));
        assert_eq!(
            checked.is_ok(),
            accepted,
            "{right_hand_side}: {:?}",
            checked.err()
        );
    }
}

#[test]
fn multi_operand_requires_cannot_mix_different_versions_of_a_source() {
    for (argument, accepted) in [("right", true), ("replace(&mut right)", false)] {
        let checked = check(&format!(
            "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left <= right;
             machine replace(value: &mut i32) -> i32 {{ value = -1; value }}
             machine compare(left: i32, mut right: i32) -> bool requires left <= right {{
                 left == {argument}
             }}"
        ));
        assert_eq!(checked.is_ok(), accepted, "{argument}: {:?}", checked.err());
    }
}

#[test]
fn conjunctive_operator_requires_preserve_a_copied_value() {
    check(
        "boundary operator == Integer::equal(left: i32, right: i32) -> bool
         requires left >= 0 && left <= 100;
         machine reset(value: &mut i32) -> i32 { value = -1; 1 }
         machine compare(mut value: i32) -> bool requires value >= 0 && value <= 100 {
             value == reset(&mut value)
         }",
    )
    .expect("each conjunct describes the copied left value, not its overwritten storage");
}

#[test]
fn copied_record_operator_requires_cannot_borrow_a_later_storage_guarantee() {
    let diagnostics = check(
        "data Number [copy] { value: i32; }
         boundary operator + Number::add(left: Number, right: Number) -> Number
         requires left.value >= 0;
         machine prepare(value: &mut Number) -> Number ensures value.value >= 0 {
             value.value = 1; Number { value: 0 }
         }
         machine combine(mut value: Number) -> Number { value + prepare(&mut value) }",
    )
    .expect_err("the copied record still contains its old field value");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn relational_operator_requires_need_facts_available_to_both_operands() {
    let checked = check(
        "boundary operator == Integer::equal(left: i32, right: i32) -> bool requires left <= right;
         machine compare(left: i32, right: i32) -> bool requires left <= right { left == right }",
    )
    .unwrap();
    let span = checked
        .facts
        .flow
        .control
        .operator_invocations
        .iter()
        .next()
        .unwrap()
        .1
        .operands;
    for ordinal in 0..2 {
        let mut facts = checked.facts.clone();
        let handle = arena::Handle::from_parts(
            span.start().arena_index() + ordinal,
            span.start().generation(),
        );
        facts
            .flow
            .control
            .operator_operands
            .get_mut(handle)
            .constraints = HandleSpan::empty();
        let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
            .expect_err("one operand's facts cannot supply a relation for both versions");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("requires")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn reference_operator_parameters_need_captured_and_live_invocation_facts() {
    let checked = check(
        "boundary operator == Reference::equal(left: &i32, right: i32) -> bool requires left >= 0;
         machine compare(value: &i32, pattern: i32) -> bool requires value >= 0 { value == pattern }",
    ).expect("reference operands owe their live referent precondition");
    let (invocation_handle, invocation) = checked
        .facts
        .flow
        .control
        .operator_invocations
        .iter()
        .next()
        .unwrap();
    for clear_capture in [true, false] {
        let mut facts = checked.facts.clone();
        if clear_capture {
            facts
                .flow
                .control
                .operator_operands
                .get_mut(invocation.operands.start())
                .constraints = HandleSpan::empty();
        } else {
            facts
                .flow
                .control
                .operator_invocations
                .get_mut(invocation_handle)
                .requires_constraints = HandleSpan::empty();
        }
        let diagnostics = crate::checks::check_checked_facts(&checked.typed, &facts)
            .expect_err("reference facts must hold at capture and remain live at invocation");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("requires")),
            "{diagnostics:#?}"
        );
    }
}

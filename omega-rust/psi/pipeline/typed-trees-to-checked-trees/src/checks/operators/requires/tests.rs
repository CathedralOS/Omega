//! Named `Namespace::requirement(...)` calls carry the same selected
//! `requires` obligations as spelled uses. Their operand-time capture keys on
//! the `named_use` handle rather than a `uses` row, so these tests exercise
//! the checker through the named operand vocabulary: operator-parameter
//! ordering, reference formals whose predicate value is the referent, and the
//! fail-closed custody corruptions the spelled suite already covers.

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize named operator call");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse named operator call");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve named operator call");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type named operator call");
    crate::lower_typed_trees(typed)
}

#[test]
fn named_call_rejects_an_unproven_selected_requires() {
    let diagnostics = check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         requires right >= 0;
         machine compare(value: i32) -> bool { Comparison::equal(1, value) }",
    )
    .expect_err("a named call owes the selected operator's requires");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot prove `value >= 0`")
                && diagnostic.message.contains("Comparison::equal")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn named_call_discharges_a_requires_proven_at_invocation() {
    check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         requires right >= 0;
         machine compare(value: i32) -> bool requires value >= 0 {
             Comparison::equal(1, value)
         }",
    )
    .expect("the machine's requires covers the named call's operand");
}

#[test]
fn named_call_requires_can_use_facts_established_by_an_earlier_operand() {
    check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         requires right >= 0;
         machine prepare(value: &mut i32) -> i32 ensures value >= 0 { value = 1; 0 }
         machine compare(mut value: i32) -> bool {
             Comparison::equal(prepare(&mut value), value)
         }",
    )
    .expect("the right operand is captured after prepare establishes its precondition");
}

#[test]
fn named_call_requires_cannot_use_facts_invalidated_by_an_earlier_operand() {
    let diagnostics = check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         requires right >= 0;
         machine reset(value: &mut i32) -> i32 { value = -1; 0 }
         machine compare(mut value: i32) -> bool requires value >= 0 {
             Comparison::equal(reset(&mut value), value)
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
fn named_call_requires_keep_a_copied_left_value_after_a_right_operand_write() {
    check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         requires left >= 0;
         machine reset(value: &mut i32) -> i32 { value = -1; 0 }
         machine compare(mut value: i32) -> bool requires value >= 0 {
             Comparison::equal(value, reset(&mut value))
         }",
    )
    .expect("a later write cannot revoke facts about the already-copied left value");
}

#[test]
fn named_call_requires_cannot_give_an_earlier_copy_a_later_storage_guarantee() {
    let diagnostics = check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         requires left >= 0;
         machine prepare(value: &mut i32) -> i32 ensures value >= 0 { value = 1; 0 }
         machine compare(mut value: i32) -> bool {
             Comparison::equal(value, prepare(&mut value))
         }",
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
fn named_call_requires_keep_a_copied_stable_record_after_a_right_operand_write() {
    check(
        "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(left: Rec, right: i32) -> bool
         requires left.count >= 0;
         machine reset(rec: &mut Rec) -> i32 { rec.count = -1; 0 }
         machine compare(mut rec: Rec) -> bool requires rec.count >= 0 {
             Ns::probe(rec, reset(&mut rec))
         }",
    )
    .expect("a later write cannot revoke facts about the already-copied left record");
}

#[test]
fn named_call_requires_keep_a_copied_generic_record_after_a_right_operand_write() {
    // A generic instantiation is the same detached copy: `h`'s bound snapshot
    // for `left` predates `reset`'s write to the source storage.
    check(
        "pub data Holder<T> { value: T; other: i32; }
         boundary operator Ns::probe(left: Holder<i32>, right: i32) -> bool
         requires left.value >= 0;
         machine reset(h: &mut Holder<i32>) -> i32 { h.value = -1; 0 }
         machine compare(mut h: Holder<i32>) -> bool requires h.value >= 0 {
             Ns::probe(h, reset(&mut h))
         }",
    )
    .expect("a later write cannot revoke facts about the already-copied generic record");
}

#[test]
fn named_call_requires_cannot_give_a_copied_record_a_later_storage_guarantee() {
    // `prepare` establishes `rec.count >= 0` on the source storage after the
    // `left` copy was already taken — the newer guarantee describes current
    // storage, never the bound operand.
    let diagnostics = check(
        "pub data Rec { count: i32; other: i32; }
         boundary operator Ns::probe(left: Rec, right: i32) -> bool
         requires left.count >= 0;
         machine prepare(rec: &mut Rec) -> i32 ensures rec.count >= 0 { rec.count = 1; 0 }
         machine compare(mut rec: Rec) -> bool {
             Ns::probe(rec, prepare(&mut rec))
         }",
    )
    .expect_err("a guarantee newer than the copy cannot discharge its clause");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn named_call_requires_read_the_referent_under_a_reference_formal() {
    // `left: &i32` used as a predicate value reads its referent, so the
    // instantiated clause names `value`, not `&value`. Without referent
    // naming the machine's `value >= 0` could never match.
    check(
        "boundary operator == Reference::equal(left: &i32, right: i32) -> bool
         requires left >= 0;
         machine compare(value: i32, pattern: i32) -> bool requires value >= 0 {
             Reference::equal(&value, pattern)
         }",
    )
    .expect("the referent's proven fact discharges the reference formal's clause");
}

#[test]
fn named_call_operand_custody_rejects_missing_duplicate_or_reordered_records() {
    let checked = check(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
         requires left >= 0;
         machine compare(value: i32, pattern: i32) -> bool requires value >= 0 {
             Comparison::equal(value, pattern)
         }",
    )
    .unwrap();
    crate::checks::check_checked_facts(&checked.typed, &checked.facts).unwrap();
    let (invocation_handle, invocation) = checked
        .facts
        .flow
        .control
        .operator_invocations
        .iter()
        .find(|(_, invocation)| invocation.named_use.is_valid())
        .expect("the named call emits an operand-time capture row");
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
fn named_call_multi_operand_requires_cannot_mix_different_versions_of_a_source() {
    for (argument, accepted) in [("right", true), ("replace(&mut right)", false)] {
        let checked = check(&format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
             requires left <= right;
             machine replace(value: &mut i32) -> i32 {{ value = -1; value }}
             machine compare(left: i32, mut right: i32) -> bool requires left <= right {{
                 Comparison::equal(left, {argument})
             }}"
        ));
        assert_eq!(checked.is_ok(), accepted, "{argument}: {:?}", checked.err());
    }
}

#[test]
fn named_call_closed_literal_operands_discharge_the_instantiated_clause() {
    // A literal operand is its own evidence: `value == value` on `70` is a
    // concrete claim, and the provider canaries' boundary-operator surface
    // relies on exactly this discharge.
    check(
        "boundary operator CheckedMath::offset_zero(value: i32) -> i32
         requires value == value;
         machine caller() -> i32 { CheckedMath::offset_zero(70) }",
    )
    .expect("70 == 70 is a closed literal claim, not a context fact");
}

#[test]
fn named_call_closed_float_literals_discharge_finite_range_requires() {
    // The float conversion surface (`I8::from_f32` and siblings) carries
    // `value == value && range` requires clauses; a literal operand decides
    // them at the operand's own format.
    check(
        "boundary operator Convert::to_i8(value: f32) -> i32
         requires value == value && value > -129.0f32 && value < 128.0f32;
         machine caller() -> i32 { Convert::to_i8(-8.75f32) }",
    )
    .expect("a closed float conjunction is decidable at f32");
}

#[test]
fn named_call_closed_literal_out_of_range_still_rejects() {
    let diagnostics = check(
        "boundary operator Convert::to_i8(value: f32) -> i32
         requires value == value && value > -129.0f32 && value < 128.0f32;
         machine caller() -> i32 { Convert::to_i8(500.0f32) }",
    )
    .expect_err("a closed false claim is not discharged by being literal");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove")),
        "{diagnostics:#?}"
    );
}

#[test]
fn named_call_symbolic_operand_keeps_the_label_path() {
    // `operand == operand` on a symbolic i32 is mathematically reflexive, but
    // the leaf prover infers no selected-operator laws — identical to spelled
    // `x == x`, which rejects the same clause.
    let diagnostics = check(
        "boundary operator CheckedMath::offset_zero(value: i32) -> i32
         requires value == value;
         machine caller(operand: i32) -> i32 { CheckedMath::offset_zero(operand) }",
    )
    .expect_err("a symbolic reflexive clause has no literal evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove `operand == operand`")),
        "{diagnostics:#?}"
    );
}

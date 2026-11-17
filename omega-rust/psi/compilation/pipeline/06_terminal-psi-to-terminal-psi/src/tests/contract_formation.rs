//! A condition cannot make its own arithmetic safe. These tests enter the
//! declaration phase directly, including declarations with no executable body.

use super::*;

fn number(value: u128) -> ScalarTerm {
    ScalarTerm::integer(
        integer_type(IntegerSign::Unsigned),
        IntegerValue::Unsigned(value),
    )
    .unwrap()
}

fn subtract(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::exact_integer_subtract(integer_type(IntegerSign::Unsigned), left, right).unwrap()
}

fn nonnegative(term: ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(number(0), term)
}

fn form(clauses: Vec<Proposition>) -> Checked<()> {
    let elaboration = lower(vec![Machine {
        requires: clauses,
        ..machine(vec![u32_type(), u32_type()], vec![returns(unsigned(0))])
    }])?;
    let mut activity = crate::ProofSearchActivity::default();
    crate::formation::module(&elaboration.module, &[], &mut activity)
}

#[test]
fn carrier_bounds_form_maximum_subtraction_without_using_its_clause() {
    form(vec![nonnegative(subtract(
        number(u32::MAX.into()),
        formal(1, u32_type()),
    ))])
    .unwrap();
}

#[test]
fn a_clause_cannot_use_its_own_consequence_to_form_subtraction() {
    let error = form(vec![nonnegative(subtract(
        formal(1, u32_type()),
        number(1),
    ))])
    .unwrap_err();
    assert_eq!(error.reason, RefusalReason::Violation);
}

#[test]
fn independent_bounds_form_terms_in_either_authored_order() {
    let guarded = nonnegative(subtract(formal(1, u32_type()), number(1)));
    let independent = Proposition::LessOrEqual(number(1), formal(1, u32_type()));
    form(vec![guarded.clone(), independent.clone()]).unwrap();
    form(vec![independent, guarded]).unwrap();
}

#[test]
fn mutually_pending_clauses_supply_no_premises() {
    let first = Proposition::LessOrEqual(number(1), subtract(formal(1, u32_type()), number(1)));
    let second = Proposition::LessOrEqual(number(1), subtract(formal(2, u32_type()), number(1)));
    assert_eq!(
        form(vec![first, second]).unwrap_err().reason,
        RefusalReason::Violation
    );
}

#[test]
fn nested_children_form_before_their_results_supply_carrier_bounds() {
    let maximum = number(u32::MAX.into());
    let child = subtract(maximum.clone(), formal(1, u32_type()));
    form(vec![nonnegative(subtract(maximum, child))]).unwrap();
    let bad_child = subtract(formal(1, u32_type()), number(1));
    assert!(
        form(vec![nonnegative(subtract(
            number(u32::MAX.into()),
            bad_child
        ))])
        .is_err()
    );
}

#[test]
fn a_decided_disjunction_does_not_erase_an_unformed_occurrence() {
    let hidden = nonnegative(subtract(formal(1, u32_type()), number(1)));
    assert!(
        form(vec![Proposition::Disjunction(vec![
            Proposition::Truth,
            hidden
        ])])
        .is_err()
    );
}

#[test]
fn a_bodyless_contract_is_not_exempt_from_formation() {
    let mut elaboration =
        lower(vec![machine(vec![u32_type()], vec![returns(unsigned(0))])]).unwrap();
    let mut declared = elaboration.module.machines[0].contract.clone();
    declared.requires = vec![nonnegative(subtract(formal(1, u32_type()), number(1)))];
    elaboration.module.declaration_contracts.push(declared);
    let mut activity = crate::ProofSearchActivity::default();
    assert_eq!(
        crate::prepare_replay(&mut elaboration, &mut activity)
            .err()
            .unwrap()
            .reason,
        RefusalReason::Violation
    );
}

#[test]
fn predicate_definitions_form_without_assuming_their_bodies() {
    let mut module = terminal_psi::TerminalModule::declarations();
    let mut identities = module.identities.into_builder();
    let declaration = identities.intern("test-predicate");
    module.identities = identities.build();
    let input = formal(1, u32_type());
    module
        .scalar_predicate_definitions
        .push(terminal_psi::ScalarPredicateDefinition {
            declaration,
            parameters: vec![terminal_psi::ValueDeclaration {
                id: id(1),
                scalar_type: ScalarType::Integer(integer_type(IntegerSign::Unsigned)),
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            }],
            body: Proposition::Conjunction(vec![
                Proposition::LessOrEqual(number(1), input.clone()),
                nonnegative(subtract(input.clone(), number(1))),
            ]),
        });
    let mut activity = crate::ProofSearchActivity::default();
    let error = crate::formation::module(&module, &[], &mut activity).unwrap_err();
    assert_eq!(error.reason, RefusalReason::Violation);
    assert!(
        error
            .detail
            .contains("mathematical definition: contract arithmetic is not proved total")
    );
    module.scalar_predicate_definitions[0].body =
        nonnegative(subtract(number(u32::MAX.into()), input));
    crate::formation::module(&module, &[], &mut activity).unwrap();
}

#[test]
fn guarantees_cannot_form_an_input_precondition() {
    let mut elaboration =
        lower(vec![machine(vec![u32_type()], vec![returns(unsigned(0))])]).unwrap();
    let contract = &mut elaboration.module.machines[0].contract;
    contract.requires = vec![nonnegative(subtract(formal(1, u32_type()), number(1)))];
    contract.ensures.push(terminal_psi::ContractClause {
        obligation: id(1),
        proposition: Proposition::LessOrEqual(number(1), formal(1, u32_type())),
    });
    let mut activity = crate::ProofSearchActivity::default();
    assert!(crate::formation::module(&elaboration.module, &[], &mut activity).is_err());
}

#[test]
fn replay_formation_is_an_explicit_searching_entrance() {
    let mut elaboration = lower(vec![Machine {
        requires: vec![nonnegative(subtract(
            number(u32::MAX.into()),
            formal(1, u32_type()),
        ))],
        ..machine(vec![u32_type()], vec![returns(unsigned(0))])
    }])
    .unwrap();
    let mut activity = crate::ProofSearchActivity::default();
    let before = activity.started();
    let _scope = crate::prepare_replay(&mut elaboration, &mut activity).unwrap();
    assert!(activity.started() > before);
}

fn source_result(text: &str) -> RouteVerdict {
    super::build_purpose::judge_build(text).unwrap()
}

fn accepted_source(text: &str) -> Box<crate::AcceptedClosure> {
    let verdict = source_result(text);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    accepted
}

fn executed_source(text: &str) {
    // A formed contract must still survive publication and execution of the
    // same body. A bodyless boundary is checked separately without a provider.
    let publication = accepted_source(text).publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert_eq!(
        status,
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
}

fn rejected_source(text: &str) {
    let verdict = source_result(text);
    match verdict {
        RouteVerdict::Rejected { .. } => {}
        RouteVerdict::Accepted(_) => panic!("invalid contract arithmetic was accepted"),
        other => panic!("invalid contract arithmetic was not rejected: {other:?}"),
    }
}

#[test]
fn source_contract_integer_casts_form_before_their_clauses() {
    const SOURCE: &str = r#"
machine widen(value: u32) -> u64
ensures result == (value as u64)
{ value as u64 }
machine narrow(value: u64) -> u32
requires value <= 4294967295
ensures result == (value as u32)
{ value as u32 }
machine build_entry() { let roundtrip: u32 = narrow(widen(37)); }
"#;
    executed_source(SOURCE);

    let unbounded = SOURCE.replace("requires value <= 4294967295", "");
    let RouteVerdict::Rejected { violations, .. } = source_result(&unbounded) else {
        panic!("the narrowing clause must be rejected at formation");
    };
    assert!(
        violations.iter().any(|row| row
            .statement
            .contains("contract arithmetic is not proved total before its condition")),
        "{violations:?}",
    );
}

#[test]
fn source_contract_alternatives_keep_both_inputs_and_refuse_a_third() {
    const SOURCE: &str = r#"
machine choose(flags: u32) -> u32
requires flags == 34603008 || flags == 36700160
ensures result == flags && result <= 36700160
{ flags }
machine build_entry() {
    let first: u32 = choose(34603008);
    let second: u32 = choose(36700160);
}
"#;
    let pruned = SOURCE.replace(
        "requires flags == 34603008",
        "requires (u16 == u8 && flags == 0) || flags == 34603008",
    );
    for positive in [SOURCE, pruned.as_str()] {
        executed_source(positive);
        let invalid = positive.replace("choose(34603008)", "choose(0)");
        let verdict = source_result(&invalid);
        assert!(
            matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
            "{verdict:?}",
        );
    }
}

#[test]
fn source_true_alternative_does_not_erase_unformed_subtraction() {
    let verdict = source_result(
        r#"
machine choose(flags: u32) -> u32
requires true || flags - 1 <= 4294967295u32
{ flags }
machine build_entry() { let selected: u32 = choose(0); }
"#,
    );
    let RouteVerdict::Rejected { violations, .. } = verdict else {
        panic!("the alternative cannot form its own subtraction: {verdict:?}");
    };
    assert!(
        violations.iter().any(|row| row
            .statement
            .contains("contract arithmetic is not proved total before its condition")),
        "{violations:?}",
    );
}

#[test]
fn source_maximum_subtraction_forms_but_the_caller_still_owes_the_bound() {
    const SOURCE: &str = r#"
machine append(offset: u64, size: u64) -> u64
requires size <= 18446744073709551615u64 - offset
{ offset + size }
machine build_entry() { let end: u64 = append(17, 23); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("append(17, 23)", "append(18446744073709551615u64, 1)"));
}

#[test]
fn source_packed_comparison_forms_each_exact_operation_from_independent_bounds() {
    const SOURCE: &str = r#"
machine packed(rest: u8, count: u8) -> u8
requires rest <= 9, count <= 9, rest + count * 10 <= 99
{ rest }
machine build_entry() { let value: u8 = packed(9, 9); }
"#;
    executed_source(SOURCE);

    // 26 * 10 can overflow the child before the sum is formed. With the
    // child bounded to 90, 166 + 90 can still overflow the sum.
    for invalid in [
        SOURCE.replace("count <= 9", "count <= 26"),
        SOURCE.replace("rest <= 9", "rest <= 166"),
    ] {
        let verdict = source_result(&invalid);
        let RouteVerdict::Rejected { violations, .. } = verdict else {
            panic!("an operation can overflow before its comparison forms: {verdict:?}");
        };
        let [violation] = violations.as_slice() else {
            panic!("expected a single formation violation: {violations:?}");
        };
        assert!(
            violation
                .statement
                .contains("contract arithmetic is not proved total before its condition"),
            "{violation:?}"
        );
    }
}

#[test]
fn a_record_domain_uses_its_carrier_default_but_not_another_named_domain() {
    const SOURCE: &str = r#"
data Reading requires number >= 1 { number: u32; }
domain Reading::First requires self.number >= 1, self.number - 1 <= 4294967295u32;
domain Reading::Second requires self.number - 1 <= 4294967295u32;
machine build_entry() { let reading: Reading = Reading { number: 1 }; }
"#;
    executed_source(SOURCE);

    let verdict = source_result(&SOURCE.replace(" requires number >= 1", ""));
    let RouteVerdict::Rejected { violations, .. } = verdict else {
        panic!("an unrelated named domain supplied the lower bound: {verdict:?}");
    };
    let [violation] = violations.as_slice() else {
        panic!("expected a single formation violation: {violations:?}");
    };
    assert!(
        violation
            .statement
            .contains("data domain: contract arithmetic is not proved total before its condition"),
        "{violation:?}"
    );
}

#[test]
fn a_record_domains_subject_uses_the_same_carrier_observations() {
    let mut accepted = accepted_source(
        r#"
data Reading requires number >= 1 { number: u32; }
domain Reading::Nonzero requires self.number - 1 <= 4294967295u32;
machine build_entry() {}
"#,
    );
    let module = &mut accepted.elaboration.module;
    let predicate = module.structural_domains[0].predicate.as_mut().unwrap();
    let number_field = ScalarTerm::integer_field_path(
        id(7),
        vec![CanonicalStructuralPathSegment::Field(id(1))],
        integer_type(IntegerSign::Unsigned),
    );
    predicate.subject = id(7);
    predicate.proposition =
        Proposition::LessOrEqual(subtract(number_field, number(1)), number(u32::MAX.into()));
    let mut activity = crate::ProofSearchActivity::default();
    crate::formation::module(module, &[], &mut activity).unwrap();

    module.structural_domains[0]
        .predicate
        .as_mut()
        .unwrap()
        .subject = id(8);
    let refusal = crate::formation::module(module, &[], &mut activity).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        refusal
            .detail
            .contains("contract field has no declared root")
    );
}

#[test]
fn a_projected_domain_uses_its_field_carriers_default() {
    const SOURCE: &str = r#"
data Reading requires number >= 1 { number: u32; }
domain Reading::Nonzero requires self.number - 1 <= 4294967295u32;
data Carry { item: Reading in Nonzero; }
machine build_entry() {}
"#;
    executed_source(SOURCE);
}

#[test]
fn deeply_projected_defaults_form_before_their_enclosing_defaults() {
    let source = r#"
data Reading requires number >= 1 { number: u32; }
data Envelope requires reading.number - 1 <= 4294967295u32 { reading: Reading; }
domain Envelope::Checked requires self.reading.number - 1 <= 4294967294u32;
data Carry { item: Envelope in Checked; }
machine build_entry() {}
"#;
    executed_source(source);
}

#[test]
fn a_field_carriers_default_cannot_form_arithmetic_on_its_sibling_or_root() {
    const SOURCE: &str = r#"
data Reading requires number >= 1 { number: u32; }
domain Reading::Nonzero requires self.number - 1 <= 4294967295u32;
data Bare { number: u32; }
data Carry requires item.number - 1 <= 4294967294u32 {
    item: Reading in Nonzero;
    backup: Bare;
    number: u32;
}
machine build_entry() {}
"#;
    executed_source(SOURCE);

    for path in ["backup.number", "number"] {
        let verdict = source_result(&SOURCE.replace("item.number - 1", &format!("{path} - 1")));
        let RouteVerdict::Rejected { violations, .. } = verdict else {
            panic!("the bound on item.number escaped its projection: {verdict:?}");
        };
        let [violation] = violations.as_slice() else {
            panic!("expected a single formation violation: {violations:?}");
        };
        assert!(
            violation.statement.contains(
                "data domain: contract arithmetic is not proved total before its condition"
            ),
            "{violation:?}"
        );
    }
}

#[test]
fn a_bound_on_one_field_does_not_form_its_neighbors_subtraction() {
    const SOURCE: &str = r#"
data Pair [copy] { first: u32; second: u32; }
machine consume(value: &Pair)
requires value.first >= 1, value.first - 1 <= 4294967295u32
{ }
machine build_entry() { let value: Pair = Pair { first: 7, second: 0 }; consume(&value); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("value.first - 1", "value.second - 1"));
}

#[test]
fn completion_storage_does_not_inherit_the_inputs_nonzero_bound() {
    const SOURCE: &str = r#"
data Cell { value: u32; }
machine retain(value: &mut Cell)
requires value.value >= 1
ensures old(value.value) - 1 <= 4294967295u32
{ value.value = 0; }
machine build_entry() { let mut value: Cell = Cell { value: 7 }; retain(&mut value); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("old(value.value) - 1", "value.value - 1"));
}

#[test]
fn independent_result_guarantees_can_form_a_later_guarantee() {
    const SOURCE: &str = r#"
machine positive() -> u32
ensures result >= 1, result - 1 == 0
{ 1 }
machine build_entry() { let value: u32 = positive(); }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("result >= 1, ", ""));
}

#[test]
fn scalar_domains_must_form_before_membership_can_supply_their_predicate() {
    const SOURCE: &str = r#"
domain u32::Nonzero requires self >= 1, self - 1 <= 4294967295u32;
machine build_entry() { let value: u32 in Nonzero = 7 as +Nonzero; }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("self >= 1, ", ""));
}

#[test]
fn data_requirements_keep_each_partial_field_occurrence() {
    const SOURCE: &str = r#"
data Count [copy] requires value >= 1, value - 1 <= 4294967295u32 { value: u32; }
machine build_entry() { let value: Count = Count { value: 7 }; }
"#;
    executed_source(SOURCE);
    rejected_source(&SOURCE.replace("value >= 1, ", ""));
}

#[test]
fn bodyless_crash_guards_need_their_own_formed_input_context() {
    const SOURCE: &str = r#"
boundary trait Device {
    machine read(value: u32) -> u32
    reaches none
    requires value >= 1
    crashes Trap value - 1 == 0;
}
machine build_entry() crashes none { let value: u32 = Device::read(2); }
"#;
    accepted_source(SOURCE);
    rejected_source(&SOURCE.replace("requires value >= 1", "requires true"));
}

#[test]
fn overflow_policies_do_not_remove_contract_divisor_obligations() {
    for policy in ["%", "|"] {
        let source = format!(
            r#"
machine divide(left: u32, right: u32) -> u32
requires right > 0
ensures result == left /{policy} right
{{ left /{policy} right }}
machine build_entry() {{ let value: u32 = divide(31, 3); }}
"#
        );
        executed_source(&source);
        rejected_source(&source.replace("requires right > 0", "requires true"));
    }
}

/// Nested computations retain their policy and every inner formation question,
/// including when an outer multiplication cannot change the final value.
#[test]
fn nested_summary_arithmetic_keeps_each_inner_formation_obligation() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/calls/scalar_contract_route_exit/main.omg"
    ))
    .replace("machine main()", "machine build_entry()");
    executed_source(&source);

    let guarded = r#"
machine nested(value: u8) -> u8
requires value <= 254
ensures result == (value + 1u8) * 0u8
{ 0u8 }
machine build_entry() { let value: u8 = nested(0u8); }
"#;
    executed_source(guarded);
    rejected_source(&guarded.replace("requires value <= 254", "requires true"));
}

#[test]
fn mathematical_domain_predicates_follow_construction_and_replacement_values() {
    const SOURCE: &str = r#"
domain u32::Small requires embed(self) + 1 <= 8;
data Reading { number: u32; }
domain Reading::Limited requires embed(self.number) + 1 <= 8;
data Cell { before: u32; value: u32 in Small; reading: Reading in Limited; after: u32; }
machine keep(value: u32 in Small) -> u32 in Small { value }
machine tag(value: Reading) -> Reading in Limited
requires embed(value.number) + 1 <= 8
{ value as +Limited }
machine build_entry() {
    let first: u32 in Small = 3u32 as +Small;
    let mut cell: Cell = Cell {
        before: 41u32, value: first,
        reading: tag(Reading { number: 3u32 }), after: 73u32
    };
    cell.value = keep(6u32 as +Small);
    cell.reading.number = 6u32;
    let plain: Reading = cell.reading as -Limited;
    let saved: u32 in Small = keep(cell.value);
    transition cell.before == 41u32 && cell.after == 73u32
        && plain.number == 6u32 && (saved as -Small) == 6u32 {
        true -> done()
        false -> failed()
    }
    machine done() {}
    machine failed() { crash Trap; }
}
"#;
    executed_source(SOURCE);

    // The earlier valid record contents cannot justify its domain after
    // this plain scalar write: the owner must prove the new field's bound.
    let invalid = SOURCE.replace("cell.reading.number = 6u32", "cell.reading.number = 8u32");
    let verdict = source_result(&invalid);
    assert!(
        matches!(&verdict, RouteVerdict::Rejected { unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::DataDomain { .. }))),
        "the replacement must fail its stored domain requirement: {verdict:?}"
    );
}

#[test]
fn a_mathematical_domain_cannot_form_its_own_embedded_partial_operand() {
    let verdict = source_result(
        r#"
domain u32::Impossible requires embed(self - 1u32) >= 0;
machine build_entry() {}
"#,
    );
    let RouteVerdict::Rejected { violations, .. } = verdict else {
        panic!("a mathematical comparison cannot authorize its embedded subtraction: {verdict:?}");
    };
    assert!(
        violations.iter().any(|row| row.statement.contains(
            "scalar domain: contract arithmetic is not proved total before its condition"
        )),
        "{violations:?}",
    );
}

#[test]
fn mathematical_domains_retain_scalar_record_and_nested_field_subjects() {
    const SOURCE: &str = r#"
data Reading [copy] requires embed(value) <= 8 { value: u32; }
domain Reading::Small requires embed(self.value) <= 4;
domain u32::Small requires embed(self) <= 4;
data Envelope { reading: Reading in Small; scalar: u32 in Small; neighbor: u32; }
machine observe(value: &Envelope)
requires embed(value.reading.value) <= 4, embed(value.scalar) <= 4, embed(value.neighbor) == 7
{ }
machine build_entry() {
    let reading: Reading = Reading { value: 3 };
    let checked: Reading in Small = reading as +Small;
    let scalar: u32 in Small = 4 as +Small;
    let value: Envelope = Envelope { reading: checked, scalar: scalar, neighbor: 7 };
    observe(&value);
}
"#;
    let accepted = accepted_source(SOURCE);
    let module = &accepted.elaboration.module;
    let [record_domain] = module.structural_domains.as_slice() else {
        panic!("the exact named record domain must remain in the catalog");
    };
    let predicate = record_domain.predicate.as_ref().unwrap();
    let reading = module
        .structural_types
        .iter()
        .find(|row| row.id == record_domain.carrier)
        .unwrap();
    let [field] = reading.shape.record_fields().unwrap() else {
        panic!("Reading has one exact declared field");
    };
    let embedded_field = |root| {
        semantic_vocabulary::IntegerMathTerm::embed(ScalarTerm::integer_field_path(
            root,
            vec![CanonicalStructuralPathSegment::Field(field.id)],
            integer_type(IntegerSign::Unsigned),
        ))
        .unwrap()
    };
    let limit =
        |value| semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(value));
    assert_eq!(
        predicate.proposition,
        Proposition::IntegerMathLessOrEqual(embedded_field(predicate.subject), limit(4)),
    );
    assert_eq!(
        reading.requires,
        [Proposition::IntegerMathLessOrEqual(
            embedded_field(id(1)),
            limit(8)
        )],
    );
    let [scalar_domain] = module.scalar_qualifications.domains.as_slice() else {
        panic!("the exact named scalar domain must remain in the catalog");
    };
    let predicate = scalar_domain.predicate.as_ref().unwrap();
    assert_eq!(
        predicate.proposition,
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::embed(ScalarTerm::Value {
                id: predicate.subject,
                scalar_type: scalar_domain.carrier,
            })
            .unwrap(),
            limit(4),
        ),
    );
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    assert_eq!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ),
    );
    // Each changes one value so an establishment or call requirement fails.
    // Missing predicates, wrong field substitution and ignored casts remain observable.
    rejected_source(&SOURCE.replace(
        "let scalar: u32 in Small = 4",
        "let scalar: u32 in Small = 5",
    ));
    rejected_source(&SOURCE.replace("Reading { value: 3 }", "Reading { value: 5 }"));
    rejected_source(&SOURCE.replace("neighbor: 7 }", "neighbor: 6 }"));
}

#[test]
fn mathematical_domain_and_data_predicates_form_their_embedded_partial_operands() {
    const SCALAR: &str = r#"
domain u32::Nonzero requires self >= 1, embed(self - 1) <= 4294967295;
machine build_entry() { let value: u32 in Nonzero = 7 as +Nonzero; }
"#;
    const RECORD: &str = r#"
data Count [copy] requires value >= 1, embed(value - 1) <= 4294967295 { value: u32; }
machine build_entry() { let value: Count = Count { value: 7 }; }
"#;
    executed_source(SCALAR);
    executed_source(RECORD);
    // The actual value 7 and the outer unbounded arithmetic cannot justify
    // partial subtraction for an arbitrary declaration input.
    rejected_source(&SCALAR.replace("self >= 1, ", ""));
    rejected_source(&RECORD.replace("value >= 1, ", ""));
}

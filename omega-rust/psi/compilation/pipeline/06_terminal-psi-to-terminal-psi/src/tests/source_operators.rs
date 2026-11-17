//! A primitive operation keeps its actual operand types during source admission.

use super::*;

fn enum_operator(text: &str) -> Result<RouteVerdict, String> {
    let typed = super::boundary_calls::try_typed(text).map_err(|error| error.message)?;
    operator_program(&typed)
}

fn operator_program(typed: &typed_trees::TypedTrees) -> Result<RouteVerdict, String> {
    let entry = super::boundary_calls::entry(typed);
    let elaboration = typed_trees_to_terminal_psi::elaborate_for(
        typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .map_err(|refusal| refusal.detail)?;
    judge(Ok(elaboration)).map_err(|error| format!("{error:?}"))
}

fn assert_enum_operator_value(source: &str, expected: bool) {
    assert_operator_value(enum_operator(source), expected);
}

fn assert_operator_value(verdict: Result<RouteVerdict, String>, expected: bool) {
    let verdict = verdict.unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("enum operator was not accepted");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Boolean(value)
                )
            ) if value == expected
        ),
        "{status:?}"
    );
}

#[test]
fn enum_case_comparisons_keep_their_validated_operand_types() {
    for (operator, case, expected) in [
        ("==", "On", true),
        ("==", "Off", false),
        ("!=", "On", false),
        ("!=", "Off", true),
        ("in", "On", true),
        ("in", "Off", false),
    ] {
        assert_enum_operator_value(
            &format!(
                "data Kind [copy] {{ case On; case Off; }}\n\
                 data Other [copy] {{ case On; case Off; }}\n\
                 machine == other_equal(left: Other, right: Other) -> bool {{ false }}\n\
                 machine entry() -> bool {{ let kind: Kind = Kind::On; kind {operator} Kind::{case} }}"
            ),
            expected,
        );
    }
}

#[test]
fn an_enum_case_test_cannot_use_a_case_of_another_enum() {
    for operator in ["==", "!=", "in"] {
        let source = format!(
            "data Kind [copy] {{ case On; case Off; }}\n\
             data Other [copy] {{ case On; case Off; }}\n\
             machine entry() -> bool {{ let kind: Kind = Kind::On; kind {operator} Other::On }}"
        );
        let Err(detail) = enum_operator(&source) else {
            panic!("a foreign enum case must be refused");
        };
        let expected = if operator == "in" {
            "case of another type than its subject's"
        } else {
            "the operands are different data types"
        };
        assert!(detail.contains(expected), "{detail}");
    }
}

#[test]
fn a_declared_enum_comparison_keeps_its_selected_meaning() {
    assert_enum_operator_value(
        "data Kind [copy] { case On; case Off; }\n\
         machine == same(left: Kind, right: Kind) -> bool { false }\n\
         machine entry() -> bool { let kind: Kind = Kind::On; kind == Kind::On }",
        false,
    );
}

#[test]
fn unrelated_operator_declarations_do_not_leave_scalar_operations_unresolved() {
    for (contract, accepted) in [("requires value <= 10", true), ("", false)] {
        let typed = super::boundary_calls::typed(&format!(
            r#"
data Pair [copy] {{ first: u32; second: u32; }}
machine + combine(left: Pair, right: Pair) -> Pair {{ left }}
machine <= ordered(left: Pair, right: Pair) -> bool {{ left.first <= right.first }}
machine entry(value: u32) -> u32 {contract} {{ value + 1u32 }}
"#
        ));
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("scalar operands exclude the unrelated Pair operator families");
        let verdict = judge(Ok(elaboration)).unwrap();
        if accepted {
            assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
        } else {
            assert!(
                matches!(verdict, RouteVerdict::Rejected { .. }),
                "{verdict:?}"
            );
        }
    }
}

#[test]
fn closed_type_disjunctions_complete_folded_operators_before_guard_pruning() {
    for (kind, expected) in [
        ("RoomHeader", true),
        ("FixedVec<i32, 4>", true),
        ("FixedVec<i32, 5>", false),
        ("u32", false),
    ] {
        let typed = super::boundary_calls::typed(&format!(
            r#"
data RoomHeader {{ room_id: u32; }}
data FixedVec<Element, const Capacity: u64> {{ items: [Element; Capacity]; length: u64; }}
machine == header_values(left: RoomHeader, right: RoomHeader) -> bool {{ false }}
machine policy_kind<Kind>() -> bool {{
    transition {{
        Kind == u32 || Kind == i32 || Kind == bool -> false
        Kind == RoomHeader -> true
        Kind == FixedVec<i32, 4> -> true
        _ -> false
    }}
}}
machine entry() -> bool {{ policy_kind<{kind}>() }}
"#
        ));
        let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("the actual closed application retains its complete nominal tuple");
        assert_operator_value(operator_program(&typed), expected);
    }
}

#[test]
fn folded_type_truth_values_and_clauses_keep_nested_boolean_operator_selections() {
    for source in [
        "data RoomHeader { room_id: u32; }\n\
         machine entry() -> bool { (RoomHeader == RoomHeader || RoomHeader == u32) == true }",
        "data RoomHeader { room_id: u32; }\n\
         machine entry() -> bool requires RoomHeader == RoomHeader || RoomHeader == u32 { true }",
    ] {
        assert_enum_operator_value(source, true);
    }
}

#[test]
fn declared_value_comparisons_under_disjunctive_guards_keep_their_selected_meaning() {
    assert_enum_operator_value(
        "data Kind [copy] { case On; case Off; }\n\
         machine == same(left: Kind, right: Kind) -> bool { false }\n\
         machine entry() -> bool {\n\
             let kind: Kind = Kind::On;\n\
             transition {\n\
                 kind == Kind::On || kind == Kind::Off -> true\n\
                 _ -> false\n\
             }\n\
         }",
        false,
    );
}

#[test]
fn mutable_boolean_locals_do_not_reuse_their_initial_static_truth() {
    assert_enum_operator_value(
        "machine entry() -> bool { let mut flag: bool = u32 == u32; flag = false; flag || u32 == u64 }",
        false,
    );
    assert_enum_operator_value(
        "machine entry() -> bool { let flag: bool = u32 == u32; flag || u32 == u64 }",
        true,
    );
}

#[test]
fn nested_indexed_record_stores_keep_the_actual_array_and_hoisted_selector_types() {
    let typed = super::boundary_calls::typed(
        r#"
data Cell [copy] { value: u32; }
data Schema [copy] { fields: [Cell; 4]; }
data Capture { schema: Schema; }
data Info { position: u64; }
machine replacement() -> u32 { 7u32 }
machine describe<Kind>(capture: &mut Capture, info: &Info)
requires info.position < 4u64
{
    put(capture, info)
    machine put(capture: &mut Capture, info: &Info)
    requires info.position < 4u64
    {
        capture.schema.fields[info.position] = Cell { value: replacement() };
    }
}
machine entry() -> bool {
    let mut capture: Capture;
    let info: Info = Info { position: 1u64 };
    describe<bool>(&mut capture, &info);
    capture.schema.fields[1u64].value == 7u32
}
"#,
    );
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .expect("the nested writer retains its actual enclosing application");
    assert_operator_value(operator_program(&typed), true);
}

#[test]
fn an_unselected_declared_value_index_is_explicitly_refused_for_reads_and_writes() {
    let declarations = "data Cell [copy] { value: u32; }\n\
        machine [] at(buffer: &[Cell; 4], index: u64) -> Cell { Cell { value: 99u32 } }";
    for body in [
        "let buffer: [Cell; 4]; buffer[0u64].value == 99u32",
        "let mut buffer: [Cell; 4]; buffer[0u64] = Cell { value: 7u32 }; true",
    ] {
        let typed = super::boundary_calls::typed(&format!(
            "{declarations}\nmachine entry() -> bool {{ {body} }}"
        ));
        let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("the ordinary preparation path retains the indexed expression");
        let refusal = operator_program(&typed)
            .expect_err("an unselected value index cannot silently acquire builtin semantics");
        assert_eq!(
            refusal,
            "admitted source selection Operator remains unresolved as CheckedOperator in `entry` at buffer[0]"
        );
    }
}

fn indexed_writer_source(requirement: &str) -> String {
    format!(
        r#"
trait Writer {{
    machine put(items: &mut [u32; 4], index: u64)
    requires index < 4u64 reaches none crashes none;
}}
machine supplied(items: &mut [u32; 4], index: u64)
{requirement} reaches none crashes none
{{ items[index] = 7u32; }}
machine entry() -> bool {{
    let mut items: [u32; 4];
    supplied(&mut items, 1u64);
    items[1u64] == 7u32
}}
"#
    )
}

#[test]
fn logical_callable_witnesses_capture_headers_without_reading_indexed_bodies() {
    for requirement in ["", "requires index < 4u64"] {
        let source = indexed_writer_source(requirement);
        // A finite authored ceiling supplies the public contract without
        // inferring entries from the uninvoked body, as in the ceiling tests.
        let complete = format!(
            "boundary trait HeaderCallback {{}}\n{}",
            source.replace(
                "reaches none crashes none",
                "reaches none crashes none invokes HeaderCallback",
            )
        );
        for (source, complete_header) in [(&source, false), (&complete, true)] {
            let typed = super::boundary_calls::typed(source);
            let required = typed
                .trait_machine_signatures
                .iter()
                .find(|(_, signature)| signature.name.as_str() == "put")
                .unwrap()
                .1;
            let supplied = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "supplied")
                .unwrap()
                .symbol;
            let witness = typed_trees_to_terminal_psi::elaborate_callable_witness(
                &typed,
                required,
                typed_trees_to_terminal_psi::CallableDeclaration::Machine(supplied),
            );
            if !complete_header {
                let Err(refused) = witness else {
                    panic!("an unread body cannot supply its inferred public entry row");
                };
                assert_eq!(
                    refused.reason,
                    terminal_check_tables::RefusalReason::Unsupported
                );
                assert_eq!(
                    refused.detail,
                    "machine invokes ceiling over a requirement that defers its entries"
                );
                continue;
            }
            let witness = witness
                .expect("logical refinement reads the complete header, not its uninvoked body");
            crate::prove_callable_refinement(witness, &mut crate::ProofSearchActivity::default())
                .expect("the public callable contract refines the requirement");
        }
    }
}

#[test]
fn demanded_indexed_bodies_still_need_their_own_bounds_proof() {
    assert_enum_operator_value(&indexed_writer_source("requires index < 4u64"), true);
    let verdict = enum_operator(&indexed_writer_source("requires true"))
        .expect("the body is well typed and reaches ordinary checking");
    let RouteVerdict::Rejected {
        formations,
        violations,
        unproved,
        unestablished,
        conflicts,
        exceeded,
    } = verdict
    else {
        panic!("the demanded body must be rejected");
    };
    assert!(formations.is_empty());
    assert!(violations.is_empty());
    assert!(unestablished.is_empty());
    assert!(conflicts.is_empty());
    assert!(exceeded.is_empty());
    assert_eq!(unproved.len(), 1);
    assert!(matches!(
        unproved[0].kind,
        ObligationKind::OperationSafety {
            site: terminal_psi::ExecutionSite::Operation(_)
        }
    ));
    assert_eq!(
        unproved[0].statement,
        "cannot prove operation safety in machine `supplied` state `entry`: cannot prove index is within length 4"
    );
}

#[test]
fn an_inlined_predicate_body_keeps_its_own_source_operator_owner() {
    let typed = super::boundary_calls::typed(
        "data Other [copy] { value: u32; }\n\
         machine < other_order(left: Other, right: Other) -> bool { false }\n\
         machine below_limit(value: u32) -> bool { value < 4u32 }\n\
         machine entry(value: u32) -> bool requires below_limit(value) { true }",
    );
    let verdict =
        operator_program(&typed).expect("the actual denotational body has its own source owner");
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn arithmetic_contract_terms_keep_their_checked_operand_types() {
    let source = r#"
data Pair [copy] { first: i64; second: i64; }
machine - subtract(left: Pair, right: Pair) -> Pair { left }
machine bounded(value: i64) -> i64
requires value >= (0i64 - 2147483648i64) && value <= 2147483647i64
{ value }
machine entry() -> bool { bounded(42i64) == 42i64 }
"#;
    assert_operator_value(enum_operator(source), true);
    let rejected = enum_operator(&source.replace("bounded(42i64)", "bounded(2147483648i64)"))
        .expect("the unchanged complete contract must still be read");
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row|
                matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{rejected:?}"
    );
}

#[test]
fn static_completion_keeps_later_guard_answers_without_assuming_the_result() {
    let source = r#"
        machine entry<Kind>() -> bool
        requires Kind == bool
        ensures result
        {
            transition {
                Kind == bool -> true
                Kind == u8 || Kind == u16 -> false
                _ -> false
            }
        }
    "#;
    for (source, accepts) in [
        (source.to_owned(), true),
        (
            source.replace("Kind == bool -> true", "Kind == bool -> false"),
            false,
        ),
    ] {
        let typed = super::boundary_calls::typed(&source);
        let forms = typed_trees_to_terminal_psi::elaborate_generic_forms(
            &typed,
            &[super::boundary_calls::entry(&typed)],
        )
        .expect("the closed type family retains every static guard selection");
        assert!(!forms.is_empty());
        for form in forms {
            let verdict = judge(Ok(form)).unwrap();
            if accepts {
                assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
            } else {
                assert!(
                    matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
                    if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
                    "{verdict:?}"
                );
            }
        }
    }
}

#[test]
fn mixed_contracts_keep_static_guard_answers_and_runtime_obligations() {
    let source = r#"
machine bounded<Kind>(value: u32) -> u32
requires value <= 10u32 && (Kind == u32 || Kind == u64)
{ value }
machine entry() -> bool { bounded<u32>(7u32) == 7u32 }
"#;
    let prepared = |text: &str| {
        typed_trees_to_typed_trees::prepare_typed_trees(
            super::boundary_calls::typed(text),
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("ordinary specialization must retain the mixed contract")
        .0
    };
    assert_operator_value(operator_program(&prepared(source)), true);
    let rejected = operator_program(&prepared(
        &source.replace("bounded<u32>(7u32)", "bounded<u32>(11u32)"),
    ))
    .expect("the closed type guard must not hide the remaining value contract");
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row|
                matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{rejected:?}"
    );
}

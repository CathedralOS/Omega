//! An impossible declared result changes continuation, not the value returned
//! by its sibling arm. An impossible postcondition keeps its declared type.

use super::*;
use terminal_semantics::inhabitation::{Inhabitation, Status, TerminalContracts};

fn program(text: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed = super::build_purpose::frontier_of(text, "machine build_entry() {}");
    let entry = super::build_purpose::symbol(&typed, "main");
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(super::build_purpose::package())],
                source::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

fn accepted(text: &str) -> Box<crate::verdict::AcceptedClosure> {
    let mut verdicts = program(text);
    let RouteVerdict::Accepted(accepted) = verdicts.remove(0).expect("source formation") else {
        panic!("the declared empty result must be judged: {verdicts:?}");
    };
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}",
    );
    accepted
}

fn check_no_normal(accepted: &crate::verdict::AcceptedClosure) -> MachineId {
    let module = &accepted.elaboration.module;
    let query =
        Inhabitation::prepare(&TerminalContracts::new(module.structural_types.iter())).unwrap();
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .filter_map(|block| match block.terminator {
            terminal_psi::Terminator::InvokeNoNormal { callee, .. }
                if module.machine(callee).is_some_and(|machine| {
                    machine.result.structural().is_some_and(|result| {
                        query.status(result.structural_type) == Status::ProvedEmpty
                    })
                }) =>
            {
                Some(callee)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [callee] = calls.as_slice() else {
        panic!("the actual empty-result call must remain a terminal invocation: {calls:?}");
    };
    let result = module
        .machine(*callee)
        .unwrap()
        .result
        .structural()
        .unwrap();
    assert_eq!(query.status(result.structural_type), Status::ProvedEmpty);
    // The call keeps its genuine Empty result declaration. No operation
    // materializes a receiving scalar or converts Empty into the sibling value.
    assert!(module.operations().all(|operation| {
        operation
            .declared_invocation()
            .is_none_or(|call| call.callee != (*callee).into())
    }));
    accepted
        .publish_invocation()
        .expect("actual accepted publication");
    let bytes = terminal_codec::encode_module(module).expect("the exact accepted module encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes)
            .expect("receiving admission retains the empty-result proof"),
        *module,
    );
    *callee
}

fn river(declarations: &str, result: &str) -> String {
    format!(
        r#"
{declarations}
public machine halt() -> {result} crashes Trap {{ crash Trap; }}
machine checked_value(value: u32, fatal: bool) -> u32
ensures result == value
crashes Trap {{
    transition fatal {{ true -> halt() false -> value }}
}}
machine main() crashes Trap {{
    let value: u32 = checked_value(7u32, false);
    transition value == 7u32 {{ true -> done() false -> wrong() }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}
"#
    )
}

#[test]
fn a_false_default_domain_preserves_the_actual_call_and_sibling_value() {
    let accepted = accepted(&river("public data Empty requires false {}", "Empty"));
    check_no_normal(&accepted);
}

#[test]
fn a_required_empty_field_and_every_empty_sum_case_exclude_their_result() {
    for (declarations, result) in [
        (
            "public data Empty requires false {} public data Wrapped { value: Empty; }",
            "Wrapped",
        ),
        (
            "public data Empty requires false {} public data Choice { case First(value: Empty); case Second(value: Empty); }",
            "Choice",
        ),
    ] {
        check_no_normal(&accepted(&river(declarations, result)));
    }
}

#[test]
fn an_empty_input_can_be_syntactically_returned_without_creating_a_normal_value() {
    let accepted = accepted(
        "public data Empty requires false {} \
         public machine identity(value: Empty) -> Empty crashes none { value } \
         machine main(value: Empty) -> u32 crashes none { identity(value) }",
    );
    let callee = check_no_normal(&accepted);
    assert!(
        terminal_semantics::control_graph::normal_completion_possible(
            &accepted.elaboration.module,
            callee,
        )
        .unwrap(),
        "the declared-type proof must work independently of syntactic non-return",
    );
}

fn type_refusal(text: &str) {
    let verdicts = program(text);
    match &verdicts[0] {
        Ok(RouteVerdict::FallBack(refusal)) => {
            assert!(refusal.detail.contains("type"), "{refusal:?}");
        }
        Ok(RouteVerdict::Rejected { violations, .. }) => {
            assert!(
                violations.iter().any(|violation| {
                    violation.statement.contains("scalar carrier `u32`")
                        && violation.statement.contains("requires `bool`")
                }),
                "the scalar result must keep its declared carrier: {verdicts:?}",
            );
        }
        _ => panic!("the declared types must remain incompatible: {verdicts:?}"),
    }
}

#[test]
fn an_ordinary_empty_record_is_inhabited_despite_a_private_crash_body() {
    type_refusal(&river("public data Ordinary {}", "Ordinary"));
}

#[test]
fn an_empty_element_in_a_zero_array_does_not_exclude_its_record() {
    type_refusal(&river(
        "public data Empty requires false {} public data Zero { values: [Empty; 0]; }",
        "Zero",
    ));
}

#[test]
fn an_impossible_postcondition_does_not_change_the_declared_result_type() {
    type_refusal(
        "public machine stop() -> u32 ensures false crashes Trap { crash Trap; } \
         machine main() crashes Trap { let value: bool = stop(); }",
    );
}

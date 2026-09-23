// The front-end pipeline these tests run, shared with the crate's unit tests
// and the other integration target through `src/lib.rs`; see its module
// documentation.
#[path = "support/front_end.rs"]
mod front_end;

use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};

#[test]
fn exact_symbol_structured_evaluation_never_reselects_a_sibling_by_name() {
    let typed = crate::front_end::typed_program(
        r#"
        data Left {}
        data Right {}

        machine Left::binding() -> i64 {
            11
        }

        machine Right::binding() -> i64 {
            22
        }
        "#,
    );
    let left = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Left::binding")
        .expect("left producer");
    let right = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Right::binding")
        .expect("right producer");
    assert_ne!(left.symbol, right.symbol);

    let admission = BuildTimeAdmissionPlan::infer(&typed, None);
    let evaluated = admission
        .evaluate_machine_symbol_for_invocation_measured(
            &typed,
            right.symbol,
            Vec::new(),
            BuildTimeInvocationCustody::Symbol(right.symbol),
        )
        .expect("evaluate the exact right-hand producer");

    assert_eq!(evaluated.value(), &BuildTimeValue::Int(22));
    assert_eq!(evaluated.usage().result_cells(), 1);
}

#[test]
fn exact_symbol_structured_evaluation_rejects_a_non_machine_symbol() {
    let typed = crate::front_end::typed_program(
        r#"
        data BindingLookalike {}

        machine binding() -> i64 {
            7
        }
        "#,
    );
    let data_symbol = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "BindingLookalike")
        .expect("data declaration")
        .symbol;
    let admission = BuildTimeAdmissionPlan::infer(&typed, None);
    let error = admission
        .evaluate_machine_symbol_for_invocation_measured(
            &typed,
            data_symbol,
            Vec::new(),
            BuildTimeInvocationCustody::Symbol(data_symbol),
        )
        .expect_err("an exact data symbol must not fall back to the machine spelling");
    assert!(error.contains("no machine with exact symbol"));
}

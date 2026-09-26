use super::TypedTrees;
use crate::facts::contract_plan_facts::build_closed_scalar_value_contract_plan;
use crate::tests::front_end::typed_program;
use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
};

mod parameter_predicates;

#[test]
fn retained_nonbuiltin_operator_status_cannot_be_replaced_by_literal_shape() {
    let program = typed_program(
        r#"
        machine value() -> u16
        requires 7u16 == 7u16
        ensures result == 7u16
        { 7u16 }
    "#,
    );
    let machine = program.machines().first().unwrap();
    for status in [
        CheckedOperatorResolutionStatus::Missing,
        CheckedOperatorResolutionStatus::Resolved,
        CheckedOperatorResolutionStatus::Ambiguous,
        CheckedOperatorResolutionStatus::DomainPending,
        CheckedOperatorResolutionStatus::Inadmissible,
        CheckedOperatorResolutionStatus::BuiltinFallback,
    ] {
        let mut operators = CheckedOperatorFacts::default();
        for contract in program.machine_contracts(machine) {
            let [typed_trees::domain::ProofFact::Expression(expression)] =
                program.proof_facts.span_or_empty(contract.facts)
            else {
                panic!("one contract expression");
            };
            operators.uses.append(CheckedOperatorUseFact {
                expression: *expression,
                status,
                ..Default::default()
            });
        }
        let plan = build_closed_scalar_value_contract_plan(&program, machine, &operators);
        let builtin = status == CheckedOperatorResolutionStatus::BuiltinFallback;
        assert_eq!(plan.requires()[0].is_some(), builtin, "{status:?}");
        assert_eq!(plan.ensures()[0].is_some(), builtin, "{status:?}");
    }
}

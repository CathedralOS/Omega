use super::{CheckedOperatorFacts, TypedTrees};
use crate::facts::contract_plan_facts::build_closed_scalar_value_contract_plan;
use crate::tests::front_end::typed_program;

fn plan(program: &TypedTrees) -> checked_trees::ClosedScalarValueContractPlan {
    build_closed_scalar_value_contract_plan(
        program,
        program.machines().first().unwrap(),
        &CheckedOperatorFacts::default(),
    )
}

#[test]
fn unsupported_present_ranges_are_not_silently_erased() {
    for parameter in ["input: u16 [0..=limit]", "input: u16 [0..=255] in Wrapping"] {
        let program = typed_program(&format!(
            "machine value(limit: u16, {parameter}) -> u16\nrequires 7u16 == 7u16\nensures result == input\n{{ input }}"
        ));
        let plan = plan(&program);
        assert_eq!(plan.requires().len(), 2, "{parameter}");
        assert!(plan.requires()[1].is_none(), "{parameter}");
    }
}

//! Closed contract predicates read field arithmetic with the operation a body
//! expression over the same carriers selects, and read members below any
//! borrowed structural parameter the caller names.
use super::{CheckedOperatorFacts, TypedTrees};
use crate::facts::contract_plan_facts::build_closed_scalar_value_contract_plan;
use crate::tests::front_end::typed_program;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedScalarExpression,
    ClosedScalarContractValue,
};

fn ensures_of(program: &TypedTrees, machine: &str) -> Vec<Option<ClosedScalarContractValue>> {
    let machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.name.as_str() == machine)
        .unwrap_or_else(|| panic!("missing machine `{machine}`"));
    build_closed_scalar_value_contract_plan(program, machine, &CheckedOperatorFacts::default())
        .ensures()
        .to_vec()
}

/// The arithmetic kind on the left of the one retained comparison.
fn left_arithmetic(row: &Option<ClosedScalarContractValue>) -> Option<CheckedIntegerBinaryKind> {
    let Some(ClosedScalarContractValue::Predicate(CheckedBooleanExpression::IntegerComparison {
        left,
        ..
    })) = row
    else {
        return None;
    };
    let CheckedScalarExpression::IntegerBinary { kind, left, .. } = left.as_ref() else {
        return None;
    };
    matches!(
        left.as_ref(),
        CheckedScalarExpression::StructuralParameterField { .. }
    )
    .then_some(*kind)
}

fn vec_with_policy(policy: &str) -> String {
    format!(
        "data Vec {{ items: [i32; 4]; length: u64{policy}; }}
machine Vec::fill(&mut self)
requires self.length < self.items.len
ensures self.length - 1 < self.items.len
{{ self.length = 3; }}"
    )
}

#[test]
fn field_arithmetic_selects_the_operation_its_policy_names() {
    for (policy, kind) in [
        ("", Some(CheckedIntegerBinaryKind::ExactSubtract)),
        (
            " in Wrapping",
            Some(CheckedIntegerBinaryKind::WrappingSubtract),
        ),
        (
            " in Saturating",
            Some(CheckedIntegerBinaryKind::SaturatingSubtract),
        ),
        // A Trapping subtraction owns a crash site and has no proposition.
        (" in Trapping", None),
    ] {
        let program = typed_program(&vec_with_policy(policy));
        let ensures = ensures_of(&program, "Vec::fill");
        assert_eq!(ensures.len(), 1, "{policy}");
        assert_eq!(left_arithmetic(&ensures[0]), kind, "{policy}");
        if kind.is_none() {
            assert!(ensures[0].is_none(), "{policy}");
        }
    }
}

#[test]
fn borrowed_parameter_members_join_the_namespace_and_owned_ones_do_not() {
    for (parameter, retained) in [
        ("picker: &mut Picker", true),
        ("picker: &Picker", true),
        ("picker: Picker", false),
    ] {
        let program = typed_program(&format!(
            "data Picker {{ limit: [i32; 4]; picks: u64; }}
machine pick({parameter}) -> u64
ensures picker.picks < picker.limit.len
{{ 0 }}"
        ));
        let ensures = ensures_of(&program, "pick");
        assert_eq!(ensures.len(), 1, "{parameter}");
        assert_eq!(ensures[0].is_some(), retained, "{parameter}");
    }
}

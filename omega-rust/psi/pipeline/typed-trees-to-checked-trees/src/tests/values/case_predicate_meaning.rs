use super::*;
use checked_trees::CheckedOperatorFacts;

#[test]
fn generated_case_equality_checks_single_tag_and_payload_expansions() {
    for value in [
        "choice == Choice::Empty {}",
        "choice.equals(Choice::Empty {})",
        "choice != Choice::Empty {}",
        "choice == Choice::Ready { value: 37 }",
    ] {
        let program = typed_trees(&format!(
            "trait Equatable {{ machine equals(&self, rhs: &Self) -> bool; }}
             data Choice {{ case Empty; case Ready(value: u64); }}
             ChoiceEquatable: Choice satisfies Equatable;
             machine equal(choice: Choice) -> bool {{ {value} }}"
        ));
        crate::lower_typed_trees(program)
            .unwrap_or_else(|diagnostics| panic!("{value}: {diagnostics:?}"));
    }
}

#[test]
fn case_tag_predicates_preserve_value_equality_and_membership_meaning() {
    for (members, operator, value, accepted, compared_field) in [
        (
            "case Success; case Failure;",
            "",
            "self.result == Outcome::Success",
            true,
            None,
        ),
        (
            "case Success; case Failure;",
            "operator == Outcome::equal(left: Outcome, right: Outcome) -> bool;",
            "self.result == Outcome::Success",
            false,
            None,
        ),
        (
            "common: u32; case Success;",
            "trait Equatable { machine equals(&self, rhs: &Self) -> bool; } OutcomeEquatable: Outcome satisfies Equatable;",
            "self.result == Outcome::Success { common: 0 }",
            true,
            Some("common"),
        ),
        (
            "case Success(value: u32);",
            "trait Equatable { machine equals(&self, rhs: &Self) -> bool; } OutcomeEquatable: Outcome satisfies Equatable;",
            "self.result == Outcome::Success { value: 0 }",
            true,
            Some("value"),
        ),
        (
            "common: u32; case Success(value: u32);",
            "",
            "self.result in Outcome::Success",
            true,
            None,
        ),
    ] {
        let program = typed_trees(&format!(
            "data Outcome [copy] {{ {members} }} {operator}
             data Root {{ result: Outcome; }}
             machine Root::predicate(&self) -> bool {{ {value} }}"
        ));
        let machine = program
            .machines()
            .iter()
            .find(|machine| program.symbols.display_path(machine.symbol, "::") == "Root::predicate")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let [StatementNode::Expression(expression)] =
            program.statement_table.statements(state.statement_nodes)
        else {
            panic!("one returned predicate");
        };
        let predicate = crate::values::lower_machine_parameter_boolean_expression(
            &program,
            &CheckedOperatorFacts::default(),
            machine,
            *expression,
            &[],
        );
        assert_eq!(predicate.is_some(), accepted, "{value}: {predicate:?}");
        if let Some(field) = compared_field {
            assert!(
                contains_field_equality(predicate.as_ref().unwrap(), field),
                "{value}: {predicate:?}"
            );
        }
    }
}

// A value equality that now lowers successfully must still compare the common
// or payload value. Accepting only its tag would weaken the original predicate.
fn contains_field_equality(
    predicate: &checked_trees::CheckedBooleanExpression,
    field: &str,
) -> bool {
    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
    };
    match predicate {
        CheckedBooleanExpression::IntegerComparison { left, .. } => {
            matches!(left.as_ref(), CheckedScalarExpression::StructuralParameterField { path, .. }
                if matches!(path.last(), Some(CheckedStructuralPredicatePathSegment::Field(name)) if name == field))
        }
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            contains_field_equality(left, field) || contains_field_equality(right, field)
        }
        _ => false,
    }
}

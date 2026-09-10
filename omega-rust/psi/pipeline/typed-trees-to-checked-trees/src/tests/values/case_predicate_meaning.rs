use super::*;
use checked_trees::CheckedOperatorFacts;

#[test]
fn case_tag_predicates_preserve_value_equality_and_membership_meaning() {
    for (members, operator, value, accepted) in [
        (
            "case Success; case Failure;",
            "",
            "self.result == Outcome::Success",
            true,
        ),
        (
            "case Success; case Failure;",
            "operator == Outcome::equal(left: Outcome, right: Outcome) -> bool;",
            "self.result == Outcome::Success",
            false,
        ),
        (
            "common: u32; case Success;",
            "trait Equatable { machine equals(&self, rhs: &Self) -> bool; } OutcomeEquatable: Outcome satisfies Equatable;",
            "self.result == Outcome::Success { common: 0 }",
            false,
        ),
        (
            "case Success(value: u32);",
            "trait Equatable { machine equals(&self, rhs: &Self) -> bool; } OutcomeEquatable: Outcome satisfies Equatable;",
            "self.result == Outcome::Success { value: 0 }",
            false,
        ),
        (
            "common: u32; case Success(value: u32);",
            "",
            "self.result in Outcome::Success",
            true,
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
    }
}

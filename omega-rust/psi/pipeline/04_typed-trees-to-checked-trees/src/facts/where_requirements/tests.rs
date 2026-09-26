use crate::tests::front_end::checked_program;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment,
};
use typed_trees::types::PrimitiveType;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program(source)
}

fn machine_requirements(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
) -> Vec<CheckedBooleanExpression> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == machine_name
                || machine
                    .name
                    .as_str()
                    .ends_with(&format!("::{machine_name}"))
        })
        .expect("machine");
    checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .expect("contract plan")
        .crash
        .structural_runtime_requirements()
        .expect("complete requirement package")
        .to_vec()
}

fn field_bound(path: &[&str], bound: i64) -> CheckedBooleanExpression {
    let literal = numerics::literals::IntegerLiteral::from_parts(
        bound < 0,
        numerics::literals::IntegerRadix::Decimal,
        &bound.unsigned_abs().to_string(),
    )
    .unwrap()
    .with_landing(numerics::literals::IntegerLanding {
        landed_type: numerics::literals::LandedIntegerType::I32,
        domain: numerics::arithmetic::ArithmeticDomain::Exact,
    });
    CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::LessOrEqual,
        left: Box::new(CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path: path
                .iter()
                .map(|segment| CheckedStructuralPredicatePathSegment::Field(segment.to_string()))
                .collect(),
            primitive_type: PrimitiveType::I32,
        }),
        right: Box::new(CheckedScalarExpression::IntegerLiteral { literal }),
    }
}

#[test]
fn entry_receiver_standing_bound_is_a_runtime_requirement() {
    let checked = checked(
        "boundary trait Console { machine exit_process(code: i32) reaches Console; }
         data Main<'s>
         where
             value <= 60,
             value <= limit,
         {
             console: &'s mut Console;
             value: i32;
             limit: i32;
         }
         machine Main::main(&mut self) reaches Console {
             let return_code: i32 = 70 + self.value;
             self.console.exit_process(return_code);
         }",
    );
    let requirements = machine_requirements(&checked, "main");
    assert!(
        requirements.contains(&field_bound(&["value"], 60)),
        "expected the standing bound in the runtime requirements, got {requirements:?}"
    );
}

#[test]
fn erased_field_facts_are_withheld() {
    let checked = checked(
        "boundary trait Console { machine exit_process(code: i32) reaches Console; }
         data Main<'s>
         where
             witness <= 60,
             value <= 60,
             value <= limit,
         {
             witness [erased]: i32;
             console: &'s mut Console;
             value: i32;
             limit: i32;
         }
         machine Main::main(&mut self) reaches Console {
             self.console.exit_process(70);
         }",
    );
    let requirements = machine_requirements(&checked, "main");
    assert!(
        requirements.contains(&field_bound(&["value"], 60)),
        "the retained field's bound should still be emitted: {requirements:?}"
    );
    assert!(
        !requirements
            .iter()
            .any(|requirement| format!("{requirement:?}").contains("witness")),
        "an erased field has no Terminal storage, so its facts cannot lower: {requirements:?}"
    );
}

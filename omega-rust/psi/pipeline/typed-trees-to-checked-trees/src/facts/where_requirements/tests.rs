use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment,
};
use typed_trees::types::PrimitiveType;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed).unwrap()
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
         data Main
         where
             value <= 60,
         {
             console: Console;
             value: i32;
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
fn nested_standing_bound_reaches_the_receiver_path() {
    let checked = checked(
        "boundary trait Console { machine exit_process(code: i32) reaches Console; }
         data Map
         where
             value <= 60,
         {
             value: i32;
         }
         data Main { console: Console; map: Map; }
         machine Main::main(&mut self) reaches Console {
             let return_code: i32 = 70 + self.map.value;
             self.console.exit_process(return_code);
         }",
    );
    let requirements = machine_requirements(&checked, "main");
    assert!(
        requirements.contains(&field_bound(&["map", "value"], 60)),
        "expected the nested standing bound, got {requirements:?}"
    );
}

#[test]
fn gated_definition_facts_are_withheld() {
    let checked = checked(
        "boundary trait Console { machine exit_process(code: i32) reaches Console; }
         data Main
         where
             health >= 1,
         {
             console: Console;
             health: i32;
         }
         machine Main::main(&mut self) reaches Console {
             self.health = 70;
             self.console.exit_process(self.health);
         }",
    );
    let requirements = machine_requirements(&checked, "main");
    assert!(
        requirements.is_empty(),
        "a zero-violating fact must not become an entry assumption: {requirements:?}"
    );
}

#[test]
fn field_to_field_facts_lower_both_sides() {
    let checked = checked(
        "boundary trait Console { machine exit_process(code: i32) reaches Console; }
         data Main
         where
             count <= len,
         {
             console: Console;
             count: i32;
             len: i32;
         }
         machine Main::main(&mut self) reaches Console {
             self.console.exit_process(70);
         }",
    );
    let requirements = machine_requirements(&checked, "main");
    let field = |path: &[&str]| {
        Box::new(CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path: path
                .iter()
                .map(|segment| CheckedStructuralPredicatePathSegment::Field(segment.to_string()))
                .collect(),
            primitive_type: PrimitiveType::I32,
        })
    };
    assert!(
        requirements.contains(&CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::LessOrEqual,
            left: field(&["count"]),
            right: field(&["len"]),
        }),
        "expected the field-to-field bound, got {requirements:?}"
    );
}

#[test]
fn erased_field_facts_are_withheld() {
    let checked = checked(
        "boundary trait Console { machine exit_process(code: i32) reaches Console; }
         data Main
         where
             witness <= 60,
             value <= 60,
         {
             witness [erased]: i32;
             console: Console;
             value: i32;
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

#[test]
fn non_entry_machines_keep_facts_as_write_obligations() {
    let checked = checked(
        "boundary trait Console { machine exit_process(code: i32) reaches Console; }
         data Meter
         where
             count <= 4096,
         {
             count: i32;
         }
         data Main { console: Console; meter: Meter; }
         machine Main::bump(&mut self, target: &mut Meter) {
             target.count = 3;
         }
         machine Main::main(&mut self) reaches Console {
             self.bump(&mut self.meter);
             self.console.exit_process(70);
         }",
    );
    let requirements = machine_requirements(&checked, "bump");
    assert!(
        requirements.is_empty(),
        "a callee's receiver facts would become caller-side obligations: {requirements:?}"
    );
    let requirements = machine_requirements(&checked, "main");
    assert!(
        requirements.iter().any(|requirement| {
            matches!(requirement,
                CheckedBooleanExpression::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::LessOrEqual,
                    left,
                    right,
                } if matches!(left.as_ref(),
                    CheckedScalarExpression::StructuralParameterField { path, .. }
                        if path == &[
                            CheckedStructuralPredicatePathSegment::Field("meter".to_string()),
                            CheckedStructuralPredicatePathSegment::Field("count".to_string()),
                        ]
                ) && matches!(right.as_ref(),
                    CheckedScalarExpression::IntegerLiteral { .. })
            )
        }),
        "the entry receiver's nested bound should be emitted: {requirements:?}"
    );
}

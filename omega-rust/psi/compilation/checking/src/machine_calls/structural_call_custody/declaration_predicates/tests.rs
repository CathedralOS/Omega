use super::{declaration_facts_carried, machine_declaration_predicates};
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedOperatorFacts,
    CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
};
use numerics::bignum::BigInt;
use typed_trees::TypedTrees;
use typed_trees::types::PrimitiveType;

/// A four-cell walk: every cell is 0 or 1, one ordinary state per cell.
const CELLS_VALID: &str = r#"
machine cells_valid(cells: &[i32; 4]) -> bool {
    transition cells[0] == 0 || cells[0] == 1 {
        true -> at_1(cells)
        _ -> (false)
    }
    state at_1(cells: &[i32; 4]) -> bool {
        transition cells[1] == 0 || cells[1] == 1 {
            true -> at_2(cells)
            _ -> (false)
        }
    }
    state at_2(cells: &[i32; 4]) -> bool {
        transition cells[2] == 0 || cells[2] == 1 {
            true -> at_3(cells)
            _ -> (false)
        }
    }
    state at_3(cells: &[i32; 4]) -> bool {
        transition cells[3] == 0 || cells[3] == 1 {
            true -> (true)
            _ -> (false)
        }
    }
}
"#;

/// True when either of the first two cells is zero: its elementwise
/// consequence bounds nothing, so the rows could never imply it.
const ONE_ZERO: &str = r#"
machine one_zero(cells: &[i32; 4]) -> bool {
    transition cells[0] == 0 || cells[1] == 0 {
        true -> (true)
        _ -> (false)
    }
}
"#;

/// A typed program whose zero-admitting data are no longer gated, as
/// checking refines them before any roster consumer runs.
fn program(source: &str) -> TypedTrees {
    let mut program = crate::front_end::typed_program(source);
    let admitted = program
        .data_definitions()
        .iter()
        .enumerate()
        .filter_map(|(index, definition)| {
            (definition.zero_gated && crate::data_predicates_admit_zero(&program, definition))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let definitions = program.roots.data_definitions;
    for index in admitted {
        program
            .tables
            .data_definitions
            .span_mut_or_empty(definitions)[index]
            .zero_gated = false;
    }
    program
}

fn machine<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> &'program typed_trees::machine::Machine {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("fixture machine")
}

fn data<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> &'program typed_trees::data::DataDefinition {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .expect("fixture data")
}

fn field(program: &TypedTrees, owner: &str, name: &str) -> CheckedStructuralPredicatePathSegment {
    let definition = data(program, owner);
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => Some(
                CheckedStructuralPredicatePathSegment::Field(field.path_identity()),
            ),
            _ => None,
        })
        .expect("fixture field")
}

fn literal(value: i64) -> Box<CheckedScalarExpression> {
    Box::new(CheckedScalarExpression::IntegerLiteral {
        literal: crate::land_integer_value(&BigInt::from_i64(value), PrimitiveType::I32)
            .expect("i32 literal"),
    })
}

fn cell_bit(path: Vec<CheckedStructuralPredicatePathSegment>) -> CheckedBooleanExpression {
    let leaf = || {
        Box::new(CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path: path.clone(),
            primitive_type: PrimitiveType::I32,
        })
    };
    CheckedBooleanExpression::And {
        left: Box::new(CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::LessOrEqual,
            left: literal(0),
            right: leaf(),
        }),
        right: Box::new(CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::LessOrEqual,
            left: leaf(),
            right: literal(1),
        }),
    }
}

#[test]
fn a_collection_predicate_carries_one_exact_interval_row_per_element() {
    let program = program(&format!(
        "{CELLS_VALID}
         data Grid where cells_valid(&board) == true {{ board: [i32; 4]; offset: i32; }}
         data Main {{ g: Grid; }}
         machine Main::seed(context: &mut Main) {{ }}
         machine Main::read(context: &Main) {{ }}"
    ));
    let operators = CheckedOperatorFacts::default();
    assert!(declaration_facts_carried(
        &program,
        &operators,
        data(&program, "Grid")
    ));
    let rows =
        machine_declaration_predicates(&program, &operators, machine(&program, "Main::seed"));
    let expected = (0..4)
        .map(|index| {
            cell_bit(vec![
                field(&program, "Main", "g"),
                field(&program, "Grid", "board"),
                CheckedStructuralPredicatePathSegment::FixedIndex(index),
            ])
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows.iter()
            .map(|row| row.predicate.clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert!(
        rows.iter()
            .all(|row| row.restored_on_return && row.parameter_position == 0)
    );
    // A shared loan carries the same entry rows and restores nothing.
    let shared =
        machine_declaration_predicates(&program, &operators, machine(&program, "Main::read"));
    assert_eq!(shared.len(), 4);
    assert!(shared.iter().all(|row| !row.restored_on_return));
}

#[test]
fn a_predicate_whose_consequence_does_not_imply_it_is_not_carried() {
    let program = program(&format!(
        "{ONE_ZERO}
         data Grid where one_zero(&board) == true {{ board: [i32; 4]; }}
         data Main {{ g: Grid; }}
         machine Main::seed(context: &mut Main) {{ }}"
    ));
    let operators = CheckedOperatorFacts::default();
    assert!(!declaration_facts_carried(
        &program,
        &operators,
        data(&program, "Grid")
    ));
    // Nothing bounds any single cell, so no row is sound to publish.
    assert!(
        machine_declaration_predicates(&program, &operators, machine(&program, "Main::seed"))
            .is_empty()
    );
}

#[test]
fn a_relational_fact_is_one_row_over_both_fields() {
    let program = program(
        "data Span where start <= end { start: i32; end: i32; }
         data Main { span: Span; }
         machine Main::widen(context: &mut Main) { }",
    );
    let operators = CheckedOperatorFacts::default();
    assert!(declaration_facts_carried(
        &program,
        &operators,
        data(&program, "Span")
    ));
    let leaf = |name: &str| {
        Box::new(CheckedScalarExpression::StructuralParameterField {
            parameter_position: 0,
            path: vec![
                field(&program, "Main", "span"),
                field(&program, "Span", name),
            ],
            primitive_type: PrimitiveType::I32,
        })
    };
    let rows =
        machine_declaration_predicates(&program, &operators, machine(&program, "Main::widen"));
    assert_eq!(
        rows.iter()
            .map(|row| row.predicate.clone())
            .collect::<Vec<_>>(),
        vec![CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::LessOrEqual,
            left: leaf("start"),
            right: leaf("end"),
        }]
    );
}

#[test]
fn interval_only_facts_ride_their_field_carriers() {
    let program = program(
        "data Gauge where level <= 9 { level: i32; }
         data Main { gauge: Gauge; }
         machine Main::fill(context: &mut Main) { }",
    );
    let operators = CheckedOperatorFacts::default();
    assert!(declaration_facts_carried(
        &program,
        &operators,
        data(&program, "Gauge")
    ));
    assert!(
        machine_declaration_predicates(&program, &operators, machine(&program, "Main::fill"))
            .is_empty()
    );
}

#[test]
fn an_ieee_range_fact_is_one_row_of_exact_format_comparisons() {
    let program = program(
        "data Gauge where level >= 0.0 && level <= 1.0 { level: f32; }
         data Main { gauge: Gauge; }
         machine Main::fill(context: &mut Main) { }",
    );
    let operators = CheckedOperatorFacts::default();
    assert!(declaration_facts_carried(
        &program,
        &operators,
        data(&program, "Gauge")
    ));
    let rows =
        machine_declaration_predicates(&program, &operators, machine(&program, "Main::fill"));
    let [row] = rows.as_slice() else {
        panic!("one IEEE range row: {rows:?}");
    };
    let CheckedBooleanExpression::And { left, right } = &row.predicate else {
        panic!("a conjunction of two comparisons: {row:?}");
    };
    for comparison in [left, right] {
        let CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } =
            comparison.as_ref()
        else {
            panic!("an IEEE comparison, never an integer bound: {comparison:?}");
        };
        let leaf_and_literal = [left.as_ref(), right.as_ref()];
        assert!(leaf_and_literal.iter().any(|operand| matches!(
            operand,
            CheckedScalarExpression::StructuralParameterField {
                primitive_type: PrimitiveType::F32,
                ..
            }
        )));
        assert!(leaf_and_literal.iter().any(|operand| matches!(
            operand,
            CheckedScalarExpression::IeeeFloatLiteral {
                value: semantic_vocabulary::IeeeFloatValue::Binary32(_)
            }
        )));
    }
}

#[test]
fn looping_machines_and_callers_retain_their_declared_restoration_obligations() {
    let program = program(&format!(
        "{CELLS_VALID}
         data Grid where cells_valid(&board) == true {{ board: [i32; 4]; }}
         data Main {{ g: Grid; }}
         machine Main::scan(context: &mut Main) {{
             transition {{ _ -> step(context) }}
             state step(context: &mut Main) {{ transition {{ _ -> step(context) }} }}
         }}
         machine Main::read(context: &Main) {{ }}
         machine Main::run(context: &mut Main) {{ Main::scan(context); }}
         machine Main::seed(context: &mut Main) {{ Main::read(context); }}"
    ));
    let operators = CheckedOperatorFacts::default();
    let rows = |name: &str| {
        machine_declaration_predicates(&program, &operators, machine(&program, name))
            .iter()
            .map(|row| row.restored_on_return)
            .collect::<Vec<_>>()
    };
    // Topology does not delete the contract: every actual arrival and normal
    // return must prove these obligations before the artifact is admitted.
    assert_eq!(rows("Main::scan"), vec![true; 4]);
    assert_eq!(rows("Main::run"), vec![true; 4]);
    // A caller of only shared-row callees restores its own.
    assert_eq!(rows("Main::seed"), vec![true; 4]);
}

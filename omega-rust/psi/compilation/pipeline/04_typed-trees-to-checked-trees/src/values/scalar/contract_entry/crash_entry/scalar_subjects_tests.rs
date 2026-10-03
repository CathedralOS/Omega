//! Invocation-relative scalar values reuse ordinary arithmetic lowering while
//! provenance decides whether each observed value still belongs to entry.
use super::lower_entry_scalar_with_subjects;
use checked_trees::{CheckedOperatorFacts, CheckedScalarExpression};
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

fn lower(source: &str) -> Option<CheckedScalarExpression> {
    let program = crate::tests::front_end::typed_program(source);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (ordinal, expression, primitive) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(ordinal, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.name.as_str() == "observed").then(|| {
                (
                    ordinal,
                    local.initial_value,
                    program
                        .primitive_type_reference(local.type_reference)
                        .expect("scalar destination"),
                )
            })
        })
        .expect("observation initializer");
    lower_entry_scalar_with_subjects(
        &program,
        &CheckedOperatorFacts::default(),
        machine,
        state,
        ordinal,
        expression,
        primitive,
        &|place| {
            crate::facts::entry_parameter_place_before_statement(
                &program,
                machine.symbol,
                state.symbol,
                ordinal,
                place,
            )
        },
    )
}

#[test]
fn entry_scalar_algebra_keeps_casts_and_dense_scalar_positions() {
    let value = lower(
        "data Padding { marker: bool; }
         machine probe(padding: Padding, input: i32 in Wrapping) {
             let saved: i32 in Wrapping = input;
             let observed: i64 in Wrapping = (saved as i64) + 1;
         }",
    )
    .expect("arithmetic over the saved entry value");
    let CheckedScalarExpression::IntegerBinary { left, .. } = value else {
        panic!("addition must retain the existing scalar operation");
    };
    let CheckedScalarExpression::IntegerWiden { operand, .. } = *left else {
        panic!("the total widening remains explicit");
    };
    assert_eq!(
        *operand,
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I32,
        }
    );
}

#[test]
fn entry_scalar_algebra_keeps_structural_subject_identity() {
    let value = lower(
        "data Input { value: i64 in Wrapping; }
         machine probe(unread: bool, input: Input, offset: i64 in Wrapping) {
             let observed: i64 in Wrapping = input.value + offset;
         }",
    )
    .expect("field and scalar operands share one arithmetic expression");
    let CheckedScalarExpression::IntegerBinary { left, right, .. } = value else {
        panic!("addition");
    };
    assert!(
        matches!(*left, CheckedScalarExpression::StructuralParameterField {
        parameter_position: 1, ref path, primitive_type: PrimitiveType::I64,
    } if path.len() == 1)
    );
    assert_eq!(
        *right,
        CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I64,
        }
    );
}

#[test]
fn entry_scalar_algebra_distinguishes_saved_value_from_overwritten_storage() {
    assert!(
        lower(
            "machine probe(mut input: i64 in Wrapping) {
             input = 4;
             let observed: i64 in Wrapping = input + 1;
         }",
        )
        .is_none(),
        "an overwritten value is not an invocation operand"
    );
    assert!(
        lower(
            "machine probe(mut input: i64 in Wrapping) {
             let saved: i64 in Wrapping = input;
             input = 4;
             let observed: i64 in Wrapping = saved + 1;
         }",
        )
        .is_some(),
        "the earlier stored copy keeps its own value"
    );
}

#[test]
fn entry_scalar_boolean_actual_keeps_existing_predicate_meaning() {
    assert!(matches!(
        lower(
            "data Input { ready: bool; }
         machine probe(flag: bool, input: Input) {
             let observed: bool = !flag || input.ready;
         }",
        ),
        Some(CheckedScalarExpression::Boolean(_))
    ));
}

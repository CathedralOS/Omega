//! Stage 05 must hand each written operand of a record or case construction
//! to the field it was written for, and the zero for an omitted field (`..`)
//! to that field's own slot with that field's own type. The lowered-form
//! checker then judges the data's `requires` clause on the operand in the
//! slot the clause reads. These tests lower constructions whose twins write
//! or omit another field, and change one field's operand in the lowered
//! program to require the checker's verdict to follow.
//!
//! The spec rules: "A final standalone `..` explicitly requests zero values
//! for omitted runtime fields. Each omitted value and the complete aggregate
//! must satisfy their default-domain ... obligations"
//! (`wiki/spec/language/data_and_literals.md`, "Explicit field omission");
//! a record or case literal evaluates in "Authored field-expression order"
//! (`wiki/spec/language/expressions.md`, "Evaluation schedule").

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, operation_mut, the_operation};
use semantic_vocabulary::{IntegerValue, ValueId};
use terminal_psi::{
    OperationKind, RecordFieldInitializer, RecordFieldValue, StructuralTypeShape, TerminalMachine,
};

/// The constant an operation of `machine` produced as `value`, written as
/// `<value>:<type>`, or the source text of the operation that produced it.
fn producer(program: &Program, machine: &TerminalMachine, value: ValueId) -> String {
    let operation = machine
        .operations()
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == value)
        })
        .unwrap_or_else(|| panic!("nothing produces {value:?}"));
    match &operation.kind {
        OperationKind::IntegerConstant { value: constant } => {
            let scalar = operation.result.scalar().unwrap();
            let constant = match constant {
                IntegerValue::Unsigned(value) => value.to_string(),
                IntegerValue::Signed(value) => value.to_string(),
            };
            format!("{constant}:{:?}", scalar.scalar_type)
        }
        OperationKind::BooleanConstant { value } => value.to_string(),
        _ => program.written(machine, operation.id),
    }
}

/// The name of each field the construction initializes, in the order it
/// lists them, with what produced its operand.
fn initialized(
    program: &Program,
    machine: &TerminalMachine,
    fields: &[RecordFieldInitializer],
    names: &[(semantic_vocabulary::StructuralFieldId, String)],
) -> Vec<(String, String)> {
    fields
        .iter()
        .map(|initializer| {
            let name = names
                .iter()
                .find(|(id, _)| *id == initializer.field)
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| panic!("unknown field {:?}", initializer.field));
            let RecordFieldValue::Scalar { value, .. } = initializer.value else {
                panic!("a scalar field: {initializer:?}")
            };
            (name, producer(program, machine, value))
        })
        .collect()
}

/// Every field stage 05 declared, by its identity and name.
fn field_names(program: &Program) -> Vec<(semantic_vocabulary::StructuralFieldId, String)> {
    let module = &program.elaboration().module;
    module
        .types
        .structural_types
        .iter()
        .flat_map(|declared| match &declared.shape {
            StructuralTypeShape::Record { fields, .. } => fields.clone(),
            StructuralTypeShape::Sum { .. } => declared
                .shape
                .cases()
                .iter()
                .flat_map(|case| case.fields.clone())
                .collect(),
            _ => Vec::new(),
        })
        .map(|field| {
            (
                field.id,
                module.identities.text(field.identity).unwrap().to_owned(),
            )
        })
        .collect()
}

fn record_fields(
    machine: &TerminalMachine,
) -> (semantic_vocabulary::OperationId, &[RecordFieldInitializer]) {
    let operation = the_operation(machine, |kind| {
        matches!(kind, OperationKind::EstablishRecord { .. })
    });
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        unreachable!()
    };
    (operation.id, fields)
}

const U8: &str = "Integer(IntegerType { carrier: Fixed, sign: Unsigned, width: Bits(8) })";
const U16: &str = "Integer(IntegerType { carrier: Fixed, sign: Unsigned, width: Bits(16) })";

/// `Options { count: 1, .. }` gives `count` the written 1 and `retries`,
/// a `u16`, its own zero. Writing `retries` instead moves the written value
/// to `retries` and the `u8` zero to `count`.
#[test]
fn an_omitted_field_gets_the_zero_of_its_own_slot_and_type() {
    let options = "data Options { count: u8; retries: u16; }";
    for (written, expected) in [
        (
            "count: 1",
            [
                ("count", format!("1:{U8}")),
                ("retries", format!("0:{U16}")),
            ],
        ),
        (
            "retries: 1",
            [
                ("count", format!("0:{U8}")),
                ("retries", format!("1:{U16}")),
            ],
        ),
    ] {
        let program = Program::of(&format!(
            "{options} machine build() -> Options {{ Options {{ {written}, .. }} }}"
        ));
        let build = program.machine("build");
        let names = field_names(&program);
        let (_, fields) = record_fields(build);
        let expected: Vec<_> = expected
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect();
        assert_eq!(
            initialized(&program, build, fields, &names),
            expected,
            "{written}"
        );
        program.judged(|_| {}).accepted();
    }
}

/// `Choice::First { y: right(), x: left(), .. }` calls `right` before
/// `left` because that is the order they are written, then hands `x` the
/// result of `left`, `flag` its Boolean zero and `y` the result of `right`,
/// in declaration order, on the constructed case.
#[test]
fn a_case_construction_runs_its_operands_in_written_order_and_fills_declared_fields() {
    let program = Program::of(
        "data Choice { case First(x: u8, flag: bool, y: u8); case Other(flag: bool); } \
         machine left() -> u8 { 1u8 } machine right() -> u8 { 2u8 } \
         machine build() -> Choice { Choice::First { y: right(), x: left(), .. } }",
    );
    let build = program.machine("build");
    let calls: Vec<_> = build
        .operations()
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .map(|operation| program.written(build, operation.id))
        .collect();
    assert_eq!(calls, ["right()", "left()"]);
    let operation = the_operation(build, |kind| {
        matches!(kind, OperationKind::EstablishStructuralCase { .. })
    });
    let OperationKind::EstablishStructuralCase {
        result_case,
        fields,
    } = &operation.kind
    else {
        unreachable!()
    };
    let names = field_names(&program);
    assert_eq!(
        initialized(&program, build, fields, &names),
        [
            ("x".to_owned(), "left()".to_owned()),
            ("flag".to_owned(), "false".to_owned()),
            ("y".to_owned(), "right()".to_owned()),
        ]
    );
    let module = &program.elaboration().module;
    let first = module
        .types
        .structural_types
        .iter()
        .flat_map(|declared| declared.shape.cases())
        .find(|case| case.id == *result_case)
        .expect("the constructed case is declared");
    assert_eq!(module.identities.text(first.identity), Some("First"));
    program.judged(|_| {}).accepted();
}

/// `Options requires self.n > 0` is judged on the operand in the `n` slot.
/// The zero `..` supplies for `n` does not satisfy it, and neither does a
/// written 0; a written 1 does. Moving the 0 written for `count` into the
/// `n` slot of the lowered construction makes the checker reject it.
#[test]
fn a_data_requirement_is_judged_on_the_operand_in_the_slot_it_reads() {
    let options = "data Options requires self.n > 0 { count: u8; n: u8; }";
    let build = |literal: &str| {
        Program::of(&format!(
            "{options} machine build() -> Options {{ {literal} }}"
        ))
    };
    let owed = |program: &Program| {
        let rejection = program.judged(|_| {}).rejected();
        assert!(
            rejection
                .unproved
                .iter()
                .all(|kind| kind.starts_with("DataDomain")),
            "{rejection:?}"
        );
        rejection.unproved.len()
    };
    assert_eq!(owed(&build("Options { count: 1, .. }")), 1);
    assert_eq!(owed(&build("Options { count: 1, n: 0 }")), 1);
    let lawful = build("Options { count: 0, n: 1 }");
    lawful.judged(|_| {}).accepted();

    let machine = lawful.machine("build");
    let names = field_names(&lawful);
    let (id, fields) = record_fields(machine);
    assert_eq!(
        initialized(&lawful, machine, fields, &names),
        [
            ("count".to_owned(), format!("0:{U8}")),
            ("n".to_owned(), format!("1:{U8}")),
        ]
    );
    // The checker judges the clause on the value in the `n` slot: making
    // that value 0 leaves the clause unproved.
    let RecordFieldValue::Scalar { value: n, .. } = fields[1].value else {
        unreachable!()
    };
    let constant = machine
        .operations()
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == n)
        })
        .unwrap()
        .id;
    let rejection = lawful
        .judged_in("build", |machine| {
            let OperationKind::IntegerConstant { value } =
                &mut operation_mut(machine, constant).kind
            else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(0);
        })
        .rejected();
    assert!(
        rejection.unproved.len() == 1 && rejection.unproved[0].starts_with("DataDomain"),
        "{rejection:?}"
    );

    // Handing `n` the operand written for `count` attaches the clause to
    // another value. The checker reads the slot's operand from the
    // construction itself, so `n` holds `count`'s 0 and the clause is
    // unproved.
    let count = fields[0].value.clone();
    let rejection = lawful
        .judged_in("build", |machine| {
            let OperationKind::EstablishRecord { fields } = &mut operation_mut(machine, id).kind
            else {
                unreachable!()
            };
            fields[1].value = count.clone();
        })
        .rejected();
    assert!(
        rejection.unproved.len() == 1 && rejection.unproved[0].starts_with("DataDomain"),
        "{rejection:?}"
    );
}

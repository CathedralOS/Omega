use super::program_with_storage;
use checked_trees::StoredIntegerWriteSite;
use semantic_vocabulary::IntegerSign;
use typed_trees::expression::ExpressionNode;

fn construction(
    source: &str,
) -> (
    typed_trees::TypedTrees,
    Vec<checked_trees::CheckedStoredIntegerWrite>,
    Vec<diagnostics::Diagnostic>,
) {
    let program = program_with_storage(source, 8, IntegerSign::Signed);
    let mut diagnostics = Vec::new();
    let writes = crate::value_custody::struct_literals::validate_struct_literal_fields(
        &program,
        &mut diagnostics,
    );
    (program, writes, diagnostics)
}

#[test]
fn stored_integer_construction_checks_physical_boundaries() {
    for (value, accepted) in [(-129, false), (-128, true), (127, true), (128, false)] {
        let source = format!(
            "data Cell {{ value: i64; }} machine make() -> Cell {{ Cell {{ value: {value} }} }}"
        );
        let (_, writes, diagnostics) = construction(&source);
        assert_eq!(diagnostics.is_empty(), accepted, "{value}: {diagnostics:?}");
        assert_eq!(writes.len(), usize::from(accepted), "{value}");
        if !accepted {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("IntegerAt store")),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn stored_integer_construction_uses_authored_bounds_without_narrowing_the_carrier() {
    for (bounds, accepted) in [
        ("requires value >= -128, value <= 127", true),
        ("requires value >= -128, value <= 128", false),
        ("", false),
    ] {
        let source = format!(
            "data Cell {{ value: i64; }} machine make(value: i64) -> Cell {bounds} {{ Cell {{ value: value }} }}"
        );
        let (_, writes, diagnostics) = construction(&source);
        assert_eq!(
            diagnostics.is_empty(),
            accepted,
            "{bounds}: {diagnostics:?}"
        );
        assert_eq!(writes.len(), usize::from(accepted));
    }
}

#[test]
fn stored_integer_construction_retains_the_nested_source_occurrence() {
    let (program, writes, diagnostics) = construction(
        "data Cell { value: i64; } data Envelope { cell: Cell; }
         machine make() -> Envelope { Envelope { cell: Cell { value: 127 } } }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let [write] = writes.as_slice() else {
        panic!("{writes:?}")
    };
    let StoredIntegerWriteSite::Construction { expression, .. } = write.site else {
        panic!("{write:?}")
    };
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        panic!("constructor")
    };
    assert_eq!(literal.type_name.as_str(), "Cell");
    let [field] = program.expression_table.struct_fields(literal.fields) else {
        panic!("one field")
    };
    assert_eq!(write.field, field.field_symbol);
    assert_eq!(write.value, field.value);
    assert_eq!(write.stored_type.bits(), 8);
}

#[test]
fn stored_integer_construction_uses_the_selected_immutable_guard() {
    for (upper, accepted) in [(127, true), (128, false)] {
        let source = format!(
            "data Cell {{ value: i64; }} machine make(value: i64) -> Cell {{
             transition value >= -128 && value <= {upper} {{
                true -> (Cell {{ value: value }})
                false -> (Cell {{ value: 0 }})
             }}
             }}"
        );
        let (_, writes, diagnostics) = construction(&source);
        assert_eq!(diagnostics.is_empty(), accepted, "{upper}: {diagnostics:?}");
        assert_eq!(writes.len(), if accepted { 2 } else { 1 });
    }
}

#[test]
fn stored_integer_construction_uses_the_value_at_its_evaluation_occurrence() {
    for (prefix, value, accepted) in [
        ("let value: i64 = 127;", "value", true),
        ("let mut value: i64 = 127; value = 128;", "value", false),
        ("let mut value: i64 = 128; value = 127;", "value", true),
    ] {
        let source = format!(
            "data Cell {{ value: i64; }} machine make() -> Cell {{ {prefix} Cell {{ value: {value} }} }}"
        );
        let (_, writes, diagnostics) = construction(&source);
        assert_eq!(
            diagnostics.is_empty(),
            accepted,
            "{prefix}: {diagnostics:?}"
        );
        assert_eq!(writes.len(), usize::from(accepted));
    }
}

#[test]
fn stored_integer_construction_respects_sibling_evaluation_order() {
    for (fields, accepted) in [
        (
            "cell: Cell { value: value }, other: mutate(&mut value)",
            true,
        ),
        (
            "other: mutate(&mut value), cell: Cell { value: value }",
            false,
        ),
    ] {
        let source = format!(
            "data Cell {{ value: i64; }} data Envelope {{ cell: Cell; other: i64; }}
             machine mutate(value: &mut i64) -> i64 {{ value = 128; 0 }}
             machine make() -> Envelope {{ let mut value: i64 = 127; Envelope {{ {fields} }} }}"
        );
        let (_, writes, diagnostics) = construction(&source);
        assert_eq!(
            diagnostics.is_empty(),
            accepted,
            "{fields}: {diagnostics:?}"
        );
        assert_eq!(writes.len(), usize::from(accepted));
    }
}

#[test]
fn stored_integer_construction_reports_arithmetic_failure_before_storage_fit() {
    for value in ["1 / input", "input + 1"] {
        let source = format!(
            "data Cell {{ value: i64; }} machine make(input: i64) -> Cell {{ Cell {{ value: {value} }} }}"
        );
        let (_, writes, diagnostics) = construction(&source);
        assert!(
            !diagnostics.is_empty(),
            "invalid arithmetic was silently dropped: {value}"
        );
        assert!(
            writes.is_empty(),
            "invalid arithmetic retained a store decision"
        );
    }
}

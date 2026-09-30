use super::data_float_predicate;
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn predicate(program: &typed_trees::TypedTrees) -> ExpressionHandle {
    let ProofFact::Expression(expression) = program
        .proof_facts
        .span_or_empty(program.data_definitions()[0].where_facts)[0]
    else {
        panic!("expression predicate");
    };
    expression
}

#[test]
fn ieee_field_predicates_use_the_current_snapshot() {
    let program = crate::front_end::typed_program(
        "data Reading where value >= 0.0f32 && value <= 1.0f32 { value: f32; }",
    );
    let definition = &program.data_definitions()[0];
    for (value, expected) in [
        (FloatMeaning::Zero { negative: false }, true),
        (FloatMeaning::Zero { negative: true }, true),
        (
            FloatSemantics::from_decimal(FloatFormat::BINARY32, "0.5").unwrap(),
            true,
        ),
        (
            FloatSemantics::from_decimal(FloatFormat::BINARY32, "2.0").unwrap(),
            false,
        ),
        (FloatMeaning::NaN, false),
        (FloatMeaning::Infinity { negative: false }, false),
        (FloatMeaning::Infinity { negative: true }, false),
    ] {
        assert_eq!(
            data_float_predicate(&program, definition, predicate(&program), &|_, _| Some(
                value.clone()
            )),
            Some(expected)
        );
    }
    assert_eq!(
        data_float_predicate(&program, definition, predicate(&program), &|_, _| None),
        None
    );
}

#[test]
fn literal_format_and_field_identity_are_not_guessed() {
    let mut program = crate::front_end::typed_program(
        "data Reading where value >= 0.0 { value: f32; } data Other { value: f32; }",
    );
    let expression = predicate(&program);
    let zero = |_, _| Some(FloatMeaning::Zero { negative: false });
    assert_eq!(
        data_float_predicate(&program, &program.data_definitions()[0], expression, &zero),
        None
    );
    crate::land_float_literal_destinations(&mut program);
    assert_eq!(
        data_float_predicate(&program, &program.data_definitions()[0], expression, &zero),
        Some(true)
    );
    let foreign = match &program.data_members(&program.data_definitions()[1])[0] {
        typed_trees::data::DataMember::Field(field) => field.symbol,
        _ => panic!("field"),
    };
    let name = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Name(_)).then_some(handle))
        .unwrap();
    let ExpressionNode::Name(path) = program.expression_table.expression_mut(name) else {
        panic!("name");
    };
    path.symbol = foreign;
    path.head_symbol = foreign;
    assert_eq!(
        data_float_predicate(&program, &program.data_definitions()[0], expression, &zero),
        None
    );
}

#[test]
fn authored_float_comparison_is_not_assigned_ieee_meaning() {
    let program = crate::front_end::typed_program(
        "boundary machine >= Other::greater(left: f32, right: f32) -> bool;
         data Reading where value >= 0.0f32 { value: f32; }",
    );
    assert_eq!(
        data_float_predicate(
            &program,
            &program.data_definitions()[0],
            predicate(&program),
            &|_, _| Some(FloatMeaning::Zero { negative: false })
        ),
        None
    );
}

#[test]
fn float_predicates_require_sealed_comparison_custody() {
    let text = "boundary machine >= Float::greater_or_equal(left: f32, right: f32) -> bool;
                data Reading where value >= 0.0f32 { value: f32; }";
    for (origin, relative, expected) in [
        (
            source::SourceOrigin::Toolchain,
            "float_operations.omg",
            Some(true),
        ),
        (source::SourceOrigin::Toolchain, "lookalike.omg", None),
        (source::SourceOrigin::User, "float_operations.omg", None),
    ] {
        let root = std::path::PathBuf::from("fixture");
        let mut sources = source::SourceMap::default();
        let source = sources
            .add_with_metadata(root.join(relative), text.to_owned(), root, None, origin)
            .source_id;
        let program = crate::front_end::typed_program_from_source_map(sources, &[(source, text)]);
        assert_eq!(
            data_float_predicate(
                &program,
                &program.data_definitions()[0],
                predicate(&program),
                &|_, _| Some(FloatMeaning::Zero { negative: false })
            ),
            expected,
            "{origin:?}/{relative}"
        );
    }
}

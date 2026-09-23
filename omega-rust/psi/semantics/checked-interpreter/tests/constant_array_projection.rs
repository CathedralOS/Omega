use checked_interpreter::evaluate_const_machine;
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;
use typed_trees::expression::ExpressionNode;

#[test]
fn typed_array_projection_cannot_execute_authored_indexing_as_builtin() {
    for (declaration, participates) in [
        ("", false),
        (
            "operator [] Indexing::index(items: &[u8; 2], index: u64) -> u8;",
            false,
        ),
        ("operator [] index(items: &[u8], index: u64) -> u8;", true),
    ] {
        let source = format!(
            "use settings; data Indexing {{}}
             {declaration}
             machine projected() -> u8 {{ settings::Sizes::VALUES[0] }}"
        );
        let settings =
            "module settings; pub data Sizes {} pub const Sizes::VALUES: [u8; 2] = [7, 9];";
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("projection/main.omg"),
                source.clone(),
                PathBuf::from("projection"),
                None,
                SourceOrigin::User,
            )
            .source_id;
        let settings_id = sources
            .add_with_metadata(
                PathBuf::from("projection/settings.omg"),
                settings.to_owned(),
                PathBuf::from("projection"),
                None,
                SourceOrigin::User,
            )
            .source_id;
        let typed = crate::front_end::typed_program_from_source_map(
            sources,
            &[(source_id, &source), (settings_id, settings)],
        );
        assert!(
            typed
                .expression_table
                .iter_expressions()
                .any(|(_, expression)| {
                    matches!(expression, ExpressionNode::Indexed(indexed)
                    if matches!(typed.expression_table.expression(indexed.collection),
                        ExpressionNode::ArrayLiteral(_)))
                }),
            "the interpreter must receive retained indexing over the substituted array"
        );
        // This public evaluator accepts typed input before checked selection
        // finalization. An unsupported authored operation must still fail at
        // execution rather than inherit the builtin array read's behavior.
        let result = evaluate_const_machine(&typed, "projected");
        if !participates {
            assert_eq!(result.expect("builtin projection evaluates"), 7);
        } else {
            let indexed = typed
                .expression_table
                .iter_expressions()
                .find_map(|(_, expression)| {
                    let ExpressionNode::Indexed(indexed) = expression else {
                        return None;
                    };
                    validation::declared_constant_array_type(&typed, indexed.collection)
                })
                .expect("selected collection type");
            assert!(
                !typed_trees::operator::resolve_indexed_spelling_for_operands(
                    &typed,
                    language_core::OperatorSpelling::Index,
                    &[Some(indexed), None],
                )
                .is_empty(),
                "authored operator must actually participate in this fixture"
            );
            let error = result.expect_err("authored indexing cannot execute as a builtin read");
            assert!(error.contains("builtin"), "{error}");
        }
    }
}

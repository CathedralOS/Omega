mod assembly_tests;
mod call_result_selection_tests;
mod constant_use_tests;
mod domain_lookup_tests;
mod nested_data_tests;
mod pattern_binding_name_tests;
mod resolved_name_handoff;
mod primitive_family_binding_tests;

/// Supply the exact authored file used both to parse and to resume body resolution.
pub(super) fn source_request(text: &str) -> super::ResolutionRequest {
    use crate::frontend_fixture::{Lexer, parse_syntax_trees_with_id};
    let mut files = crate::source_inputs::SourceMap::default();
    let id = files.add("test/main.omg".into(), text.to_owned()).source_id;
    let tokens = Lexer::new(text).tokenize().unwrap();
    super::ResolutionRequest {
        syntax: parse_syntax_trees_with_id(id, &tokens).unwrap(),
        sources: Some(std::sync::Arc::new(files)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    }
}

#[test]
fn computed_measures_require_one_subject_and_a_result_type() {
    let owner = "data Counter { value: i32; }";
    for declaration in [
        "measure Counter::Rank(subject: Counter) -> i32 { 0 }",
        "measure Counter::Rank lexicographic { value }",
    ] {
        let source = format!("{owner} {declaration}");
        let trees = super::resolve(source_request(&source))
            .unwrap_or_else(|diagnostics| panic!("{declaration}: {diagnostics:?}"));
        assert_eq!(trees.roots.measures.len(), 1);
    }

    for declaration in [
        "measure Counter::Rank() -> i32 { 0 }",
        "measure Counter::Rank(a: Counter, b: Counter) -> i32 { 0 }",
        "measure Counter::Rank(subject: Counter) { 0 }",
    ] {
        let source = format!("{owner} {declaration}");
        let diagnostics = match super::resolve(source_request(&source)) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("resolver accepted {declaration}"),
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("computed measure")),
            "{declaration}: {diagnostics:?}"
        );
    }
}

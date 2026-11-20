mod assembly_tests;
mod call_result_selection_tests;
mod domain_lookup_tests;
mod pattern_binding_name_tests;
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

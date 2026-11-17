use crate::token_input::cursor::*;
use source_files_to_tokens::Lexer;
use std::sync::Arc;

#[test]
fn escaping_text_and_literal_bytes_outlive_token_storage() {
    let source: Arc<str> = Arc::from(r#"name 1.5 "\xff""#);
    let tokens = Lexer::new(&source)
        .tokenize()
        .unwrap()
        .retain_source(source.clone());
    let mut syntax = syntax_trees::SyntaxTrees::new(SourceId(77));
    let input = Input::new(SourceId(77), &tokens, &source, &mut syntax);
    let (name, rest) = input.take_identifier().unwrap();
    let (float, rest) = rest.take_float_text().unwrap();
    let decoded_pointer = tokens.text(tokens.last().unwrap()).as_bytes().as_ptr();
    let (bytes, rest) = rest.take_string_bytes().unwrap();
    assert_eq!(bytes.as_ptr(), decoded_pointer);
    assert!(rest.tokens.is_empty());
    drop(tokens);
    drop(source);
    assert_eq!(name.as_str(&syntax), "name");
    assert_eq!(name.source_span().source_id, SourceId(77));
    assert_eq!(float.as_str(&syntax), "1.5");
    assert_eq!(bytes.as_ref(), &[255]);
}

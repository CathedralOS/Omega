use std::path::PathBuf;
use std::sync::Arc;

use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use typed_trees::TypedTrees;

fn typed(text: &str, relative: &str, origin: SourceOrigin) -> TypedTrees {
    let root = PathBuf::from("fixture");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(root.join(relative), text.to_owned(), root, None, origin)
        .source_id;
    let tokens = Lexer::new(text).tokenize().expect("tokenize");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        Arc::new(sources),
    )
    .expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

const CALLING: &str = r#"
trait CallingPolicy {}
trait Calling<C> {}
data Policy {}
boundary trait Native: Calling<Policy> {
    machine send(value: &[u8]);
}
machine send(value: &[u8]) satisfies Native::send via Binding::DllImport("foreign", "send");
"#;

#[test]
fn local_calling_lookalike_cannot_exempt_a_foreign_slice() {
    for (relative, origin) in [
        ("main.omg", SourceOrigin::User),
        ("calling.omg", SourceOrigin::User),
        ("other.omg", SourceOrigin::Toolchain),
    ] {
        let program = typed(CALLING, relative, origin);
        assert!(validation::standard_calling_traits(&program).is_none());
        let errors = validation::validate_program(&program).expect_err("no calling authority");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("cannot use safe slice")),
            "{errors:?}"
        );
    }
}

#[test]
fn exact_standard_calling_declarations_retain_the_custom_policy_route() {
    let program = typed(CALLING, "calling.omg", SourceOrigin::Toolchain);
    let (calling, policy) =
        validation::standard_calling_traits(&program).expect("exact declarations");
    assert_ne!(calling.symbol, policy.symbol);
    validation::validate_program(&program).expect("custom calling route remains available");
}

const VECTOR: &str = r#"
data Vec<T> { value: u64; }
boundary trait Native {
    machine send(value: &Vec<u8>);
}
machine send(value: &Vec<u8>) satisfies Native::send via Binding::DllImport("foreign", "send");
"#;

#[test]
fn public_calling_vocabulary_is_recognized_by_content_not_package_filename() {
    let vocabulary = include_str!("../../../../../source/library/std/calling.omg");
    let program = typed(vocabulary, "public_abi.omg", SourceOrigin::User);
    assert!(validation::standard_calling_traits(&program).is_some());
    let changed = vocabulary.replace("trait Calling<C>", "trait Calling<C, Extra>");
    assert_ne!(changed, vocabulary);
    let program = typed(&changed, "calling.omg", SourceOrigin::User);
    assert!(validation::standard_calling_traits(&program).is_none());
}

#[test]
fn user_vector_name_is_an_ordinary_record_not_a_private_descriptor() {
    for relative in ["main.omg", "vec.omg"] {
        validation::validate_program(&typed(VECTOR, relative, SourceOrigin::User))
            .expect("a local record name does not determine its native carrier");
    }
}

#[test]
fn exact_core_vector_still_requires_an_explicit_native_adapter() {
    let errors = validation::validate_program(&typed(VECTOR, "vec.omg", SourceOrigin::Toolchain))
        .expect_err("core descriptor cannot become a default foreign ABI record");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cannot use vector carrier")),
        "{errors:?}"
    );
}

#[test]
fn user_vector_name_does_not_hide_a_nested_private_carrier() {
    let text = VECTOR.replace("value: u64;", "value: &[u8];");
    let errors = validation::validate_program(&typed(&text, "main.omg", SourceOrigin::User))
        .expect_err("ordinary record fields are still checked");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("cannot use safe slice")),
        "{errors:?}"
    );
}

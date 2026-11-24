mod assembly_tests;
mod call_result_selection_tests;
mod constant_use_tests;
mod domain_lookup_tests;
mod equation_specialization_tests;
mod nested_data_tests;
mod pattern_binding_name_tests;
mod primitive_family_binding_tests;
mod resolved_name_handoff;

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

/// The symbols a definition's result type names at its arguments, beside the
/// definition's own binders.
fn definition_result_arguments(
    source: &str,
) -> (Vec<symbols::SymbolHandle>, Vec<symbols::SymbolHandle>) {
    use crate::source_resolved::mathematical::MathematicalType;
    use crate::source_resolved::types::TypeReference;
    let trees = super::resolve(source_request(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
    let declarations = &trees.tables.declarations;
    let definition = trees
        .roots
        .mathematical_definitions
        .iter()
        .next()
        .expect("the definition");
    let binders = declarations
        .data_type_parameters
        .span_or_empty(definition.binders)
        .iter()
        .map(|binder| binder.symbol)
        .collect();
    let MathematicalType::Ordinary(TypeReference::Generic(generic)) =
        declarations.mathematical_types.get(definition.result)
    else {
        panic!("the result applies data")
    };
    let named = generic
        .arguments
        .handles()
        .map(
            |argument| match declarations.child_type_references.get(argument) {
                TypeReference::Named { symbol, .. } => *symbol,
                other => panic!("a bare name: {other:?}"),
            },
        )
        .collect();
    (binders, named)
}

#[test]
fn a_definition_names_its_value_binders_inside_its_types() {
    // `Vector` takes a value at both positions, so `Length` and `Total`
    // there are the definition's own binders. Left unresolved, the package
    // review refuses the program for a stale declaration in a carried type.
    let (binders, named) = definition_result_arguments(
        "data Vector<Length: u64, Total: u64> { case Nil; } \
         let retain<Length: u64, Total: u64>(value: Vector<Length, Total>): \
         Vector<Length, Total> = value;",
    );
    assert_eq!(binders.len(), 2);
    assert_eq!(named, binders);
}

#[test]
fn a_value_binder_does_not_stand_where_data_takes_a_type() {
    // `Box` takes a type, and a value binder never hides one.
    let (binders, named) = definition_result_arguments(
        "data Box<T> { value: T; } \
         let keep<Length: u64>(value: Box<Length>): Box<Length> = value;",
    );
    assert_eq!(binders.len(), 1);
    assert_ne!(named, binders);
}

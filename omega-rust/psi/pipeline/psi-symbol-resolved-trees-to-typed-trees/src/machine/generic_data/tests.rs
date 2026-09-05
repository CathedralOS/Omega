use super::*;
use psi_source_files_to_tokens::Lexer;
use psi_symbols::SymbolHandle;

fn resolved_fixture() -> SymbolResolvedTrees {
    let source = r#"
        pub data Envelope<T> { value: T; }
        pub machine Envelope::stored<T>(&self) -> T { self.value }
        data Secret { value: i32; }
        machine use_private(value: Envelope<Secret>) {}
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize generic method");
    let mut sources = psi_source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("generic_method.omg"),
            source.to_owned(),
        )
        .source_id;
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse generic method with exact source occurrence ownership");
    let syntax =
        psi_generic_instances::normalize_pre_resolution(syntax).expect("synthesize generic method");
    psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve exact derivation")
}

fn derived(program: &SymbolResolvedTrees) -> Machine {
    program
        .machines
        .iter()
        .find(|machine| machine.generic_data_origin.template.is_valid())
        .expect("one synthesized method")
        .clone()
}

#[test]
fn exact_generated_method_and_authored_template_have_distinct_selection_contexts() {
    let program = resolved_fixture();
    let instance = derived(&program);
    let template = program
        .machines
        .iter()
        .find(|machine| machine.symbol == instance.generic_data_origin.template)
        .unwrap();
    assert!(is_derived(&program, &instance).unwrap());
    assert!(!is_derived(&program, template).unwrap());
    assert!(instance.is_public && template.is_public);
}

#[test]
fn missing_template_owner_or_source_token_cannot_authorize_suppression() {
    let program = resolved_fixture();
    let original = derived(&program);
    let mut missing_template = original.clone();
    missing_template.generic_data_origin.template = SymbolHandle::invalid();
    let mut missing_owner = original.clone();
    missing_owner.generic_data_origin.closed_owner = SymbolHandle::invalid();
    let mut missing_source = original;
    missing_source.generic_data_origin.template_source = Default::default();
    for malformed in [missing_template, missing_owner, missing_source] {
        assert!(is_derived(&program, &malformed).is_err());
    }
}

#[test]
fn wrong_template_and_cross_paired_closed_owner_reject() {
    let program = resolved_fixture();
    let original = derived(&program);
    let unrelated_machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "use_private")
        .unwrap();
    let unrelated_data = program
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == "Secret")
        .unwrap();
    let mut wrong_template = original.clone();
    wrong_template.generic_data_origin.template = unrelated_machine.symbol;
    let mut wrong_owner = original;
    wrong_owner.generic_data_origin.closed_owner = unrelated_data.symbol;
    wrong_owner.attached_data_symbol = unrelated_data.symbol;
    for malformed in [wrong_template, wrong_owner] {
        assert!(is_derived(&program, &malformed).is_err());
    }
}

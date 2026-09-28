use super::{Machine, SymbolResolvedTrees, is_derived};
use symbols::SymbolHandle;

fn resolved_fixture() -> SymbolResolvedTrees {
    let source = r#"
        pub data Envelope<T> { value: T; }
        pub machine Envelope::stored<T>(&self) -> T { self.value }
        data Secret { value: i32; }
        machine use_private(value: Envelope<Secret>) {}
    "#;
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("generic_method.omg"),
            source.to_owned(),
        )
        .source_id;
    crate::front_end::resolved_program_from_source_map_with_generic_data(
        sources,
        &[(source_id, source)],
    )
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

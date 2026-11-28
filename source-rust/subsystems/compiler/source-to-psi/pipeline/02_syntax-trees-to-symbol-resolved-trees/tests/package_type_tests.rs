use crate::resolution::resolve;
use crate::source_resolved::data::{DataMember, TypeParameterKind};
use crate::source_resolved::types::TypeReference;

#[test]
fn hidden_package_arguments_belong_to_the_declaration() {
    let source = "data Pair<'a, some S, T> { state: @'a S; tag: T; } \
                  data Other<'a, some S> { state: @'a S; } \
                  data Holder<'a> { left: Pair<'a, u8>; right: Other<'a>; }";
    let trees = resolve(super::source_request(source)).unwrap();
    let mut hidden = Vec::new();
    for name in ["Pair", "Other"] {
        let data = trees
            .roots
            .data_definitions
            .iter()
            .find(|d| d.name.as_str() == name)
            .unwrap();
        let binder = &trees.data_type_parameters(data.type_parameters)[0];
        assert!(matches!(binder.kind, TypeParameterKind::SomeType));
        assert!(binder.symbol.is_valid());
        assert_eq!(trees.symbols.get(binder.symbol).parent, data.symbol);
        let DataMember::Field(field) = &trees.data_members(data.members)[0] else {
            panic!("missing state field");
        };
        let TypeReference::Reference(reference) = &field.type_reference else {
            panic!("expected a borrow of hidden state");
        };
        let TypeReference::Named { symbol, .. } = trees.child_type_reference(reference.referee)
        else {
            panic!("expected a declaration-scoped type");
        };
        assert_eq!(*symbol, binder.symbol);
        hidden.push(binder.symbol);
    }
    assert_ne!(hidden[0], hidden[1]);
}

#[test]
fn package_type_arguments_include_only_public_binders() {
    let declaration = "data Package<some State, Tag> { state: @State; tag: Tag; }";
    for spelling in ["Package", "Package<u32, bool>"] {
        let source = format!("{declaration} machine take(item: {spelling}) {{}}");
        let diagnostics = resolve(super::source_request(&source)).expect_err("wrong public arity");
        assert!(
            diagnostics.iter().any(|error| error
                .message
                .contains("public argument count for package `Package` is 1")),
            "{diagnostics:?}"
        );
    }
}

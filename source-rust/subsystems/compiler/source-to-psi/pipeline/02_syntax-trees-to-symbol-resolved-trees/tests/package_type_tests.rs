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

#[test]
fn named_openings_belong_to_the_formal_or_local_that_lends_the_value() {
    use crate::source_resolved::statement::Statement;
    let source = "data X = u32; data Z = u16; data Packet<Tag, some S> { tag: Tag; item: @S; } \
        machine observe(a: @Packet<u8, some X>, b: @Packet<u8, some Y>) { \
            let left: @X = a.item; let right: @Y = b.item; \
            let local: @Packet<u8, some Z> = b; let value: @Z = local.item; }";
    let trees = resolve(super::source_request(source)).unwrap();
    let tables = &trees.tables.declarations;
    let types = &tables.child_type_references;
    let mut openings = std::collections::HashMap::new();
    let locals = || {
        tables
            .state_statements
            .iter()
            .filter_map(|(_, row)| match row {
                Statement::LocalData(local) => Some(local),
                _ => None,
            })
    };
    for (owner, reference) in tables
        .state_parameters
        .iter()
        .map(|(_, p)| (p.symbol, &p.type_reference))
        .chain(locals().map(|l| (l.symbol, &l.type_reference)))
    {
        for opening in types.annotation_openings(reference) {
            assert_eq!(opening.position, 1);
            assert!(opening.symbol.is_valid(), "{}", opening.name.as_str());
            assert_eq!(trees.symbols.get(opening.symbol).parent, owner);
            openings.insert(opening.name.as_str(), opening.symbol);
        }
    }
    assert_eq!(openings.len(), 3);
    for (local_name, opening_name) in [("left", "X"), ("right", "Y"), ("value", "Z")] {
        let binding = locals().find(|l| l.name.as_str() == local_name).unwrap();
        let TypeReference::Reference(reference) = &binding.type_reference else {
            panic!("expected borrow");
        };
        let TypeReference::Named { symbol, .. } = types.get(reference.referee) else {
            panic!("expected witness type");
        };
        assert_eq!(*symbol, openings[opening_name], "{local_name}");
    }
}

#[test]
fn opening_markers_select_hidden_slots_only() {
    for ty in [
        "Visible<some S>",
        "Tagged<some T, u8>",
        "Packet<some A, some B>",
    ] {
        let source = format!(
            "data Visible<T> {{ v: T; }} data Tagged<T, some S> {{ v: T; state: @S; }} \
            data Packet<some S> {{ state: @S; }} machine take(p: @{ty}) {{}}"
        );
        let errs = resolve(super::source_request(&source)).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| e.message.contains("declared `some` slot")
                    || e.message.contains("beyond its declared slots")),
            "{ty}: {errs:?}"
        );
    }
}

#[test]
fn two_visible_subjects_cannot_bind_the_same_witness_name() {
    let source = "data Packet<some S> { item: @S; } \
                  machine bad(a: @Packet<some X>, b: @Packet<some X>) {}";
    let errors = resolve(super::source_request(source)).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("type name already in scope")),
        "{errors:?}"
    );
}

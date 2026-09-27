use crate::lowerer::seeded_continuation::lower_seeded_extension;
use crate::lowerer::tests::seeded_plain_data_inputs;

#[test]
fn seeded_plain_data_continuation_appends_exact_erased_lifetime_data_graph() {
    let (mut base, extension) = seeded_plain_data_inputs(
        "pub data Main { value: u32; }",
        r#"
            pub data View<'buf> { body: &'buf Main; }
            pub data Envelope<'msg> { view: View<'msg>; tail: [u8; 2]; }
        "#,
    );
    base.typed_mut()
        .evidence_forwardings
        .push(typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: symbols::SymbolHandle::invalid(),
            state_symbol: symbols::SymbolHandle::invalid(),
            statement_index: 13,
            source_statement_index: 17,
            target: typed_trees::name::Identifier::generated_static("lifetime-target"),
            source: typed_trees::name::Identifier::generated_static("lifetime-source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let before_members = before
        .data_members
        .iter()
        .map(|(handle, member)| (handle, member.clone()))
        .collect::<Vec<_>>();
    let before_type_count = before.type_reference_table.type_reference_count();
    let before_symbols = before
        .symbols
        .symbols()
        .nodes()
        .iter()
        .map(|(handle, symbol)| (handle, symbol.clone()))
        .collect::<Vec<_>>();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed = lower_seeded_extension(extension, base)
        .expect("erased lifetime-only generated data should append");

    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    assert_eq!(
        typed
            .data_members
            .iter()
            .take(before_members.len())
            .map(|(handle, member)| (handle, member.clone()))
            .collect::<Vec<_>>(),
        before_members
    );
    for arena_index in 1..=u32::try_from(before_type_count).expect("type count") {
        let handle = arena::Handle::from_arena_index(arena_index);
        assert_eq!(
            typed.type_reference_table.type_reference(handle),
            before.type_reference_table.type_reference(handle)
        );
    }
    assert_eq!(
        typed
            .symbols
            .symbols()
            .nodes()
            .iter()
            .take(before_symbols.len())
            .map(|(handle, symbol)| (handle, symbol.clone()))
            .collect::<Vec<_>>(),
        before_symbols
    );
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );

    let main = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("retained Main data");
    let view = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "View")
        .expect("generated View data");
    let envelope = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Envelope")
        .expect("generated Envelope data");
    assert_eq!(
        view.lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["buf"]
    );
    assert_eq!(
        envelope
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["msg"]
    );

    let [typed_trees::data::DataMember::Field(body)] = typed.data_members(view) else {
        panic!("View has one body field")
    };
    let typed_trees::types::TypeReferenceNode::Reference {
        referee, lifetime, ..
    } = typed
        .type_reference_table
        .type_reference(body.type_reference)
    else {
        panic!("View.body remains a reference")
    };
    assert_eq!(lifetime.as_ref().map(|name| name.as_str()), Some("buf"));
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*referee)
    else {
        panic!("View.body referee remains nominal")
    };
    assert_eq!(*symbol, main.symbol);

    let [typed_trees::data::DataMember::Field(view_field), _] = typed.data_members(envelope) else {
        panic!("Envelope has view and tail fields")
    };
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = typed
        .type_reference_table
        .type_reference(view_field.type_reference)
    else {
        panic!("Envelope.view remains an erased lifetime application")
    };
    assert_eq!(*base_symbol, view.symbol);
    assert_eq!(
        lifetime_arguments
            .iter()
            .map(|argument| argument.as_str())
            .collect::<Vec<_>>(),
        ["msg"]
    );
    assert!(
        typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
    );
}

use crate::lowerer::seeded_continuation::lower_seeded_extension;
use crate::lowerer::tests::{seeded_normalized_plain_data_inputs, seeded_plain_data_inputs};

#[test]
fn seeded_plain_data_continuation_appends_named_data_and_preserves_typed_sidecars() {
    let (mut base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; }",
        "data Generated { base: Authored; }",
    );
    base.typed_mut()
        .evidence_forwardings
        .push(typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: symbols::SymbolHandle::invalid(),
            state_symbol: symbols::SymbolHandle::invalid(),
            statement_index: 7,
            source_statement_index: 11,
            target: typed_trees::name::Identifier::generated_static("target"),
            source: typed_trees::name::Identifier::generated_static("source"),
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

    let typed =
        lower_seeded_extension(extension, base).expect("plain generated data should append");

    assert_eq!(
        typed.data_definitions().len(),
        before.data_definitions().len() + 1
    );
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
    let generated = typed.data_definitions().last().expect("generated data");
    let [typed_trees::data::DataMember::Field(generated_field)] = typed.data_members(generated)
    else {
        panic!("one generated field")
    };
    assert!(generated_field.symbol.arena_index() > before.symbols.symbols().len() as u32);
    assert!(generated_field.type_reference.arena_index() > before_type_count as u32);
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );
    assert!(typed.authored_declaration_selections().len() > resolved_ledger.len());
}

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

#[test]
fn seeded_plain_data_continuation_appends_owner_local_type_parameter_data() {
    let (mut base, extension) = seeded_plain_data_inputs(
        "data Authored { value: u32; }",
        "data Generated<T> { value: T; pair: [T; 2]; }",
    );
    base.typed_mut()
        .evidence_forwardings
        .push(typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: symbols::SymbolHandle::invalid(),
            state_symbol: symbols::SymbolHandle::invalid(),
            statement_index: 19,
            source_statement_index: 23,
            target: typed_trees::name::Identifier::generated_static("generic-target"),
            source: typed_trees::name::Identifier::generated_static("generic-source"),
            source_conformance: None,
        });
    let before = base.typed().clone();
    let resolved_ledger = extension.trees().authored_declaration_selections().clone();

    let typed = lower_seeded_extension(extension, base)
        .expect("owner-local type-parameter data should append");

    assert_eq!(
        &typed.data_definitions()[..before.data_definitions().len()],
        before.data_definitions()
    );
    assert_eq!(typed.evidence_forwardings, before.evidence_forwardings);
    assert!(
        typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(resolved_ledger.as_slice())
    );

    let generated = typed.data_definitions().last().expect("generated data");
    let [parameter] = typed.data_type_parameters(generated) else {
        panic!("Generated has one type parameter")
    };
    assert!(matches!(
        parameter.kind,
        typed_trees::data::TypeParameterKind::Type
    ));
    assert_eq!(
        parameter.bounds,
        typed_trees::data::DataProperties::default()
    );
    let [
        typed_trees::data::DataMember::Field(value),
        typed_trees::data::DataMember::Field(pair),
    ] = typed.data_members(generated)
    else {
        panic!("Generated has value and pair fields")
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("Generated.value remains the owner-local type parameter")
    };
    assert_eq!(*symbol, parameter.symbol);
    let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } = typed
        .type_reference_table
        .type_reference(pair.type_reference)
    else {
        panic!("Generated.pair remains a fixed array")
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(*element_type)
    else {
        panic!("Generated.pair element remains the owner-local type parameter")
    };
    assert_eq!(*symbol, parameter.symbol);
}

#[test]
fn seeded_plain_data_continuation_appends_one_exact_local_primitive_instance() {
    let (mut base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        r#"
            data Cell<T> { value: T; }
            data Generated { first: Cell<u32>; second: Cell<u32>; base: Authored; }
        "#,
    );
    base.typed_mut()
        .evidence_forwardings
        .push(typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: symbols::SymbolHandle::invalid(),
            state_symbol: symbols::SymbolHandle::invalid(),
            statement_index: 29,
            source_statement_index: 31,
            target: typed_trees::name::Identifier::generated_static("instance-target"),
            source: typed_trees::name::Identifier::generated_static("instance-source"),
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
        .expect("one local primitive instance should append");

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
    let template = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell")
        .expect("local generic template");
    let instance = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.generic_instance.is_some())
        .expect("one synthesized instance");
    let wrapper = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated wrapper");
    let origin = instance.generic_instance.expect("instance origin");
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = typed.type_reference_table.type_reference(origin)
    else {
        panic!("instance retains its exact generic origin")
    };
    assert_eq!(*base_symbol, template.symbol);
    assert!(lifetime_arguments.is_empty());
    let [argument] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("one exact instance argument")
    };
    let typed_trees::types::TypeReferenceNode::Named {
        symbol: argument_symbol,
        ..
    } = typed.type_reference_table.type_reference(*argument)
    else {
        panic!("primitive instance argument remains nominal")
    };
    assert_eq!(
        typed.symbols.get(*argument_symbol).kind,
        symbols::SymbolKind::BuiltinType
    );
    let [typed_trees::data::DataMember::Field(value)] = typed.data_members(instance) else {
        panic!("instance has one substituted field")
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
        .type_reference_table
        .type_reference(value.type_reference)
    else {
        panic!("instance field is the primitive argument")
    };
    assert_eq!(*symbol, *argument_symbol);
    let wrapper_instance_uses = typed
        .data_members(wrapper)
        .iter()
        .filter(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return false;
            };
            matches!(
                typed.type_reference_table.type_reference(field.type_reference),
                typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                    if *symbol == instance.symbol
            )
        })
        .count();
    assert_eq!(
        wrapper_instance_uses, 2,
        "repeated uses deduplicate to one instance"
    );
}

use super::*;
use source::SourceId;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

#[test]
fn copied_nested_sum_context_uses_retained_application_not_generated_spelling() {
    let mut syntax = SyntaxTrees::new(SourceId::default());
    let tokens = Lexer::new("data Choice<T> { case Empty; case Full(value: T); } data Holder { value: Choice<Choice<u64>>; }").tokenize().unwrap();
    parse_syntax_trees_into_with_id(&mut syntax, SourceId(1), &tokens).unwrap();
    let mut syntax = normalize_generic_data(syntax).unwrap();
    let instances = syntax
        .root_item_handles()
        .iter()
        .copied()
        .filter_map(|declaration| {
            let Item::Data(data) = syntax.root_item(declaration) else {
                return None;
            };
            let origin = data.generic_instance?;
            let ClosedArgumentIdentity::Instance(template, argument_identity) =
                closed_argument_identity(&syntax, None, origin, false)?
            else {
                return None;
            };
            let TypeReferenceNode::Generic { arguments, .. } =
                syntax.type_references.type_reference(origin)
            else {
                return None;
            };
            Some(Instantiation {
                synthetic_name: data.name.as_str().to_owned(),
                declaration,
                template,
                argument_handles: syntax
                    .type_references
                    .type_reference_handles(*arguments)
                    .to_vec(),
                argument_identity,
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    let holder = syntax
        .root_items()
        .find(|item| matches!(item, Item::Data(data) if data.name.as_str() == "Holder"))
        .unwrap()
        .clone();
    let snapshot = syntax.clone();
    let Item::Data(copied) = syntax.copy_item_from(&snapshot, &holder) else {
        panic!("copied holder");
    };
    let [DataMember::Field(field)] = syntax.items.data_members(copied.members) else {
        panic!("copied field");
    };
    let copied_type = field.type_reference;
    let selected = expected_instance(&syntax, &instances, None, copied_type)
        .expect("copied application retains exact nested tuple");
    assert!(matches!(
        selected.argument_identity.as_slice(),
        [ClosedArgumentIdentity::Instance(_, _)]
    ));
    let selected_declaration = selected.declaration;
    let node_without_origin = syntax.type_references.type_reference(copied_type).clone();
    let absent_origin = syntax.type_references.insert(node_without_origin);
    assert!(
        expected_instance(&syntax, &instances, None, absent_origin).is_none(),
        "rendered name alone grants no instance identity"
    );
    let mut ambiguous = instances.clone();
    ambiguous.push(
        instances
            .iter()
            .find(|instance| instance.declaration == selected_declaration)
            .unwrap()
            .clone(),
    );
    assert!(
        expected_instance(&syntax, &ambiguous, None, copied_type).is_none(),
        "duplicate exact instantiation custody rejects"
    );
}

#[test]
fn attached_machine_substitution_retains_previously_closed_sum_argument() {
    let source = "data Choice<T> { case Empty; case Full(value: T); }
        data Cell<T> { value: T; }
        machine Cell::make<T>(&self) -> T { Choice::Empty }
        data Outer<T> { value: Cell<T>; }
        data Holder { value: Outer<Choice<u64>>; }";
    let mut syntax = SyntaxTrees::new(SourceId::default());
    let tokens = Lexer::new(source).tokenize().unwrap();
    parse_syntax_trees_into_with_id(&mut syntax, SourceId(1), &tokens).unwrap();
    let syntax = normalize_generic_data(syntax).unwrap();
    let machine = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine)
                if machine.type_parameters.is_empty()
                    && !machine.generic_data_template.as_str().is_empty() =>
            {
                Some(machine)
            }
            _ => None,
        })
        .expect("materialized root method");
    let state = syntax
        .items
        .state(*syntax.items.state_handles(machine.states).first().unwrap());
    let origin = syntax
        .type_references
        .generic_application_origin(state.return_type);
    assert!(
        origin.is_valid(),
        "substitution retains the original argument application"
    );
    assert!(matches!(
        closed_argument_identity(&syntax, None, state.return_type, false),
        Some(ClosedArgumentIdentity::Instance(_, _))
    ));
}

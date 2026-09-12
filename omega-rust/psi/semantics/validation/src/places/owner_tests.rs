use super::*;
use crate::symbols::MachineSymbols;
use symbols::SymbolHandle;
use typed_trees::name::Identifier;

fn fixture() -> TypedTrees {
    let source = "data First<T [copy]> { value: T; }
        machine First::read<T [copy]>(&self) -> T { self.value }
        data Second<T [copy]> { other: T; }
        machine Second::read<T [copy]>(&self) -> T { self.other }
        machine use_both(first: &First<u64>, second: &Second<u64>) {}";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let syntax = syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax)
        .expect("normalize the two closed carrier applications");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn closed_owner_fields_ignore_colliding_diagnostic_names() {
    let mut program = fixture();
    let owners = program
        .data_definitions()
        .iter()
        .filter(|data| data.generic_instance.is_some())
        .map(|data| {
            (
                data.symbol,
                program
                    .data_members(data)
                    .iter()
                    .find_map(|member| {
                        let typed_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        Some(field.name.as_str().to_owned())
                    })
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), 2);
    // Both closed applications have the same tuple. Diagnostic leaf collisions
    // model namespace-qualified declarations without changing their selections.
    let roots = program.roots.data_definitions;
    for data in program.tables.data_definitions.span_mut_or_empty(roots) {
        if data.generic_instance.is_some() {
            data.name = Identifier::generated("Envelope<u64>");
        }
    }
    for (owner, field) in owners {
        let mut machine = program
            .machines()
            .iter()
            .find(|machine| machine.attached_data_symbol == owner)
            .unwrap()
            .clone();
        machine.attached_data = Some(Identifier::generated("Envelope<u64>"));
        assert_eq!(
            machine_attached_data(&program, &machine).unwrap().symbol,
            owner
        );
        let mut diagnostics = Vec::new();
        let symbols = MachineSymbols::build(&program, &machine, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(symbols.has_member(&field));
        assert!(!symbols.has_member(if field == "value" { "other" } else { "value" }));
        let expected = exact_attached_field(&program, &machine, SymbolHandle::invalid(), &field)
            .unwrap()
            .type_reference;
        assert_eq!(
            declared_member_path_type(&program, &machine, None, &["self".to_owned(), field]),
            Some(expected)
        );
    }
}

#[test]
fn owner_selection_rejects_missing_stale_and_duplicate_declarations() {
    let program = fixture();
    let owner = program
        .data_definitions()
        .iter()
        .find(|data| data.generic_instance.is_some())
        .unwrap()
        .symbol;
    let original = program
        .machines()
        .iter()
        .find(|machine| machine.attached_data_symbol == owner)
        .unwrap();
    for symbol in [
        SymbolHandle::invalid(),
        original.symbol,
        SymbolHandle::from_parts(owner.arena_index(), owner.generation() + 1),
    ] {
        let mut machine = original.clone();
        machine.attached_data_symbol = symbol;
        assert!(machine_attached_data(&program, &machine).is_none());
    }
    let field = program
        .data_members(machine_attached_data(&program, original).unwrap())
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .unwrap();
    let mut foreign = original.clone();
    foreign.attached_data_symbol = program
        .data_definitions()
        .iter()
        .find(|data| data.generic_instance.is_some() && data.symbol != owner)
        .unwrap()
        .symbol;
    assert!(exact_attached_field(&program, &foreign, field.symbol, field.name.as_str()).is_none());
    let mut duplicate = program.clone();
    duplicate.push_data_definition(machine_attached_data(&program, original).unwrap().clone());
    assert!(machine_attached_data(&duplicate, original).is_none());
}

#[test]
fn place_type_uses_retained_nominal_symbol_without_name_fallback() {
    let mut program = fixture();
    let owner = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Second<u64>")
        .unwrap()
        .symbol;
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            name: Identifier::generated("First<u64>"),
            symbol: owner,
        });
    assert_eq!(
        data_definition_for_type(&program, reference)
            .unwrap()
            .symbol,
        owner
    );
    let instance = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == owner)
        .unwrap()
        .generic_instance
        .unwrap();
    let mut generic = program
        .type_reference_table
        .type_reference(instance)
        .clone();
    let TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        ..
    } = &mut generic
    else {
        panic!("retained application");
    };
    let template = *base_symbol;
    *base_name = Identifier::generated("First");
    let generic = program.type_reference_table.insert(generic);
    assert_eq!(
        data_definition_for_type(&program, generic).unwrap().symbol,
        template
    );
    for symbol in [
        SymbolHandle::invalid(),
        SymbolHandle::from_parts(owner.arena_index(), owner.generation() + 1),
    ] {
        program.type_reference_table.substitute_node(
            reference,
            TypeReferenceNode::Named {
                name: Identifier::generated("First<u64>"),
                symbol,
            },
        );
        assert!(data_definition_for_type(&program, reference).is_none());
    }
}

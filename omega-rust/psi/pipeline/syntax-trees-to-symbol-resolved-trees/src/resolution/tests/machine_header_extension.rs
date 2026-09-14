use super::*;
use symbols::SymbolKind;

#[test]
fn retained_extension_preserves_machine_children_and_appends_selected_inherited_slots() {
    let base_source = "data Anchor { retained: u64; }
        machine Anchor::read(&self) -> u64 { self.retained }";
    let extension_source = "data Box<T [copy]> { value: T; }
        machine Box::read<T [copy]>(&self) -> T { self.value }
        machine use_box(value: &Box<u64>) {}";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from(".omega/generated/methods.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_syntax =
        parse_syntax_trees_with_id(base_id, &Lexer::new(base_source).tokenize().unwrap()).unwrap();
    let base = lower_syntax_trees_with_sources(&base_syntax, Arc::new(sources.clone()))
        .expect("retained ordinary attached machine");
    let retained_machine = base.machines.iter().next().unwrap().symbol;
    let retained_children = base
        .symbols
        .child_handles(retained_machine)
        .unwrap()
        .map(|handle| (handle, base.symbols.get(handle).clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        retained_children
            .iter()
            .map(|(_, symbol)| symbol.kind)
            .collect::<Vec<_>>(),
        [SymbolKind::Field, SymbolKind::State]
    );
    let extension = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source).tokenize().unwrap(),
    )
    .unwrap();
    let sources = Arc::new(sources);
    let extension = crate::normalize_generic_data_with_retained_base(
        extension,
        sources.clone(),
        Vec::new(),
        Some(&base),
    )
    .expect("normalize current extension template and its closed application");
    let program =
        lower_syntax_extension_against_resolved_base(base, &extension, sources, Vec::new())
            .expect("extend retained machine headers");
    assert_eq!(
        program
            .symbols
            .child_handles(retained_machine)
            .unwrap()
            .map(|handle| (handle, program.symbols.get(handle).clone()))
            .collect::<Vec<_>>(),
        retained_children
    );
    let generated = program
        .machines
        .iter()
        .filter(|machine| machine.generic_data_origin.template.is_valid())
        .collect::<Vec<_>>();
    assert_eq!(generated.len(), 1);
    let machine = generated[0];
    let owner = program
        .data_definitions
        .iter()
        .find(|data| data.symbol == machine.attached_data_symbol)
        .unwrap();
    assert!(owner.generic_instance.is_some());
    assert_eq!(machine.generic_data_origin.closed_owner, owner.symbol);
    let children = program
        .symbols
        .child_handles(machine.symbol)
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 2);
    assert_eq!(program.symbols.get(children[0]).kind, SymbolKind::Field);
    assert_eq!(program.symbols.name(children[0]), "value");
    assert_eq!(program.symbols.get(children[1]).kind, SymbolKind::State);
    assert!(
        children
            .iter()
            .all(|child| program.symbols.get(*child).parent == machine.symbol)
    );
    let states = program.machine_state_handles(machine.states);
    assert_eq!(states.len(), 1);
    assert_eq!(program.machine_state(states[0]).symbol, children[1]);
}

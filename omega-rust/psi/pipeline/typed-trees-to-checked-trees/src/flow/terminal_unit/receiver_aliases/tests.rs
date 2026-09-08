use super::*;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    crate::lower_typed_trees(typed).expect("checked alias fixture")
}

fn fixture(access: &str, prefix: &str, body: &str) -> checked_trees::CheckedTrees {
    checked(&format!(
        "data Record [copy] {{ value: u16; }}
         machine Record::replace(&write self, value: u16) {{ self.value = value; }}
         machine consume(records: &write [Record; 2]) {{}}
         machine forward(records: &{access} [Record; 2], other: &write [Record; 2], value: u16) {{ {prefix} {body} }}"
    ))
}

fn aliases(checked: &checked_trees::CheckedTrees) -> Option<Vec<ReceiverAlias>> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")?;
    let state = checked.machine_states(machine).first()?;
    prefix(&checked.typed, &checked.facts, machine, state)
}

#[test]
fn direct_write_only_aliases_preserve_parameters_and_sequential_receivers() {
    for access in ["write", "mut"] {
        let checked = fixture(
            access,
            "let held: &write [Record; 2] = &write records;",
            "held[0].replace(value); held[1].replace(value);",
        );
        assert_eq!(aliases(&checked).expect("exact direct alias").len(), 1);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|plan| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == plan.machine && machine.name.as_str() == "forward"
                })
            })
            .expect("retained caller plan");
        assert_eq!(plan.operations.len(), 3);
        assert!(matches!(
            plan.operations[2],
            CheckedUnitEffectOperationPlan::ReturnUnit { .. }
        ));
        for (statement_index, operation) in plan.operations[..2].iter().enumerate() {
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                ..
            } = operation
            else {
                panic!("receiver call");
            };
            assert_eq!(coordinate.statement_index as usize, statement_index + 1);
            assert_eq!(structural_arguments.len(), 1);
            assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
            assert_eq!(
                structural_arguments[0].path,
                vec![CheckedUnitStructuralPathSegment::FixedIndex(
                    statement_index as u64
                )]
            );
        }
    }
}

#[test]
fn independent_alias_prefix_has_no_fixed_roster_limit() {
    let checked = fixture(
        "write",
        "let first: &write [Record; 2] = &write records; let second: &write [Record; 2] = &write other;",
        "first[1].replace(value); second[0].replace(value);",
    );
    let aliases = aliases(&checked).expect("two independent loans");
    assert_eq!(aliases.len(), 2);
    assert_ne!(aliases[0].root, aliases[1].root);
}

#[test]
fn alias_erasure_does_not_claim_escapes_or_nested_reborrow_carriers() {
    for (prefix, body) in [
        (
            "let held: &write [Record; 2] = &write records;",
            "consume(&write held); held[1].replace(value);",
        ),
        (
            "let held: &write [Record; 2] = &write records; let child: &write [Record; 2] = &write held;",
            "child[1].replace(value);",
        ),
    ] {
        let checked = fixture("write", prefix, body);
        assert!(aliases(&checked).is_none());
    }
}

#[test]
fn alias_erasure_rejoins_exact_resource_and_lifetime_custody() {
    let original = fixture(
        "write",
        "let held: &write [Record; 2] = &write records;",
        "held[1].replace(value);",
    );
    assert!(aliases(&original).is_some());
    let resource_handle = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, resource)| original.symbols.name(resource.owner_symbol) == "held")
        .expect("held resource")
        .0;
    for mutation in 0..5 {
        let mut checked = original.clone();
        let resource = checked
            .facts
            .borrow
            .direct_loan_resources
            .get_mut(resource_handle);
        match mutation {
            0 => resource.owner_symbol = SymbolHandle::invalid(),
            1 => resource.captured_place.root_symbol = SymbolHandle::invalid(),
            2 => resource.access = BorrowAccessKind::Mutable,
            3 => {
                resource.activation_source =
                    FlowInvalidationSource::Statement { statement_index: 1 }
            }
            _ => resource.weakening_reason = FlowBorrowWeakeningReason::LocalReassigned,
        }
        assert!(aliases(&checked).is_none(), "resource mutation {mutation}");
    }
    let mut checked = original.clone();
    let resource = checked
        .facts
        .borrow
        .direct_loan_resources
        .get(resource_handle);
    checked
        .facts
        .borrow
        .loans
        .get_mut(resource.loan)
        .source_owner_symbol = resource.owner_symbol;
    assert!(
        aliases(&checked).is_none(),
        "a derived local owner cannot impersonate a direct parameter loan"
    );
}

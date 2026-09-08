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
fn alias_erasure_does_not_claim_escapes_or_early_nested_closure() {
    for (prefix, body) in [
        (
            "let held: &write [Record; 2] = &write records;",
            "consume(&write held); held[1].replace(value);",
        ),
        (
            "let held: &write [Record; 2] = &write records; let child: &write [Record; 2] = &write held;",
            "child[1].replace(value); other[0].replace(value);",
        ),
    ] {
        let checked = fixture("write", prefix, body);
        assert!(aliases(&checked).is_none());
    }
}

#[test]
fn nested_aliases_preserve_every_parent_and_original_parameter() {
    for access in ["write", "mut"] {
        for depth in [2, 3, 6] {
            let mut prefix = String::from("let held0: &write [Record; 2] = &write records;");
            for child in 1..depth {
                prefix.push_str(&format!(
                    "let held{child}: &write [Record; 2] = &write held{};",
                    child - 1
                ));
            }
            let checked = fixture(
                access,
                &prefix,
                &format!(
                    "held{}[0].replace(value); held{}[1].replace(value);",
                    depth - 1,
                    depth - 1
                ),
            );
            let aliases = aliases(&checked).expect("exact nested chain");
            assert_eq!(aliases.len(), depth);
            assert!(aliases.iter().all(|alias| alias.root == aliases[0].root));
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "forward")
                .expect("caller");
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .find(|plan| plan.machine == machine.symbol)
                .expect("nested caller plan");
            for (call_position, operation) in plan.operations[..2].iter().enumerate() {
                let CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    structural_arguments,
                    ..
                } = operation
                else {
                    panic!("receiver call");
                };
                assert_eq!(coordinate.statement_index as usize, depth + call_position);
                assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
                assert_eq!(
                    structural_arguments[0].path,
                    vec![CheckedUnitStructuralPathSegment::FixedIndex(
                        call_position as u64
                    )]
                );
            }
        }
    }
}

fn nested_fixture() -> checked_trees::CheckedTrees {
    fixture(
        "write",
        "let held: &write [Record; 2] = &write records; let child: &write [Record; 2] = &write held; let leaf: &write [Record; 2] = &write child;",
        "leaf[1].replace(value);",
    )
}

#[test]
fn nested_receiver_alias_keeps_the_exact_self_attachment() {
    let checked = checked(
        "data Record [copy] { value: u16; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine Record::forward(&write self, value: u16) {
            let held: &write Record = &write self;
            let child: &write Record = &write held;
            child.replace(value);
        }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Record::forward")
        .expect("attached forward");
    let state = checked.machine_states(machine).first().expect("entry");
    let aliases =
        prefix(&checked.typed, &checked.facts, machine, state).expect("nested self receiver");
    assert_eq!(aliases.len(), 2);
    assert!(aliases.iter().all(|alias| alias.root == machine.symbol));
}

#[test]
fn nested_aliases_reject_forged_immediate_parent_resource_and_capture() {
    let original = nested_fixture();
    assert!(aliases(&original).is_some());
    let resource_handle = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "leaf")
        .expect("leaf resource")
        .0;
    for mutation in 0..12 {
        let mut checked = original.clone();
        let resource = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get_mut(resource_handle);
        match mutation {
            0 => resource.parent_loan = arena::Handle::invalid(),
            1 => resource.parent_resource = checked_trees::CheckedParentBorrowResource::default(),
            2 => resource.owner_symbol = SymbolHandle::invalid(),
            3 => resource.captured_place.root_symbol = SymbolHandle::invalid(),
            4 => resource.access = BorrowAccessKind::Mutable,
            5 => resource.parent_access = BorrowAccessKind::Read,
            6 => resource.parent_suspension.parent_entry_constraint = arena::Handle::invalid(),
            7 => resource.parent_suspension.child_activation = arena::Handle::invalid(),
            8 => {
                resource.parent_end_status.status =
                    checked_trees::ParentLexicalStatusAtChildEnd::LivePastChild
            }
            9 => resource.restoration.child_loan = arena::Handle::invalid(),
            10 => resource.weakening_reason = FlowBorrowWeakeningReason::LastUseExpired,
            _ => resource.parent_suspension.parent_loan = arena::Handle::invalid(),
        }
        assert!(aliases(&checked).is_none(), "resource mutation {mutation}");
    }
    for mutation in 0..4 {
        let mut checked = original.clone();
        let loan_handle = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get(resource_handle)
            .loan;
        let loan = checked.facts.borrow.loans.get_mut(loan_handle);
        match mutation {
            0 => loan.lineage = BorrowLoanLineage::DirectRoot,
            1 => loan.source_owner_symbol = SymbolHandle::invalid(),
            2 => loan.statement_index = 0,
            _ => loan.root_symbol = SymbolHandle::invalid(),
        }
        assert!(aliases(&checked).is_none(), "loan mutation {mutation}");
    }
}

#[test]
fn nested_aliases_reject_omitted_reordered_and_retargeted_closure() {
    let original = nested_fixture();
    let event_handle = original
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .find(|(_, row)| row.retired_parent_path.len() == 2)
        .expect("leaf closure")
        .0;
    for mutation in 0..8 {
        let mut checked = original.clone();
        let event = checked
            .facts
            .borrow
            .reborrow_disposition_events
            .get_mut(event_handle);
        match mutation {
            0 => {
                event.retired_parent_path.pop();
            }
            1 => event.retired_parent_path.reverse(),
            2 => event.retired_parent_path[0].weakening = arena::Handle::invalid(),
            3 => event.disposition = checked_trees::CheckedReborrowResourceDisposition::Reactivate,
            4 => event.child_activation = arena::Handle::invalid(),
            5 => {
                event.final_target =
                    checked_trees::CheckedBorrowResourceDispositionTarget::default()
            }
            6 => event.shared_cohort.push(event.child_resource),
            _ => event.parent_loan = arena::Handle::invalid(),
        }
        assert!(aliases(&checked).is_none(), "closure mutation {mutation}");
    }
    let containment_handle = original
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .find(|(_, row)| {
            row.child_loan
                == original
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .get(event_handle)
                    .child_loan
        })
        .expect("leaf containment")
        .0;
    for mutation in 0..6 {
        let mut checked = original.clone();
        let containment = checked
            .facts
            .borrow
            .reborrow_containment_certificates
            .get_mut(containment_handle);
        match mutation {
            0 => containment.parent_loan = arena::Handle::invalid(),
            1 => containment.parent_access = BorrowAccessKind::Mutable,
            2 => containment.parent_weakening = arena::Handle::invalid(),
            3 => containment.parent_place.root_symbol = SymbolHandle::invalid(),
            4 => containment
                .projection_remainder
                .push(facts::PlaceSegment::FixedIndex { index: 1 }),
            _ => {
                containment.containment =
                    checked_trees::CheckedReborrowContainmentKind::SharedFreeze
            }
        }
        assert!(
            aliases(&checked).is_none(),
            "containment mutation {mutation}"
        );
    }
}

#[test]
fn nested_aliases_require_unique_complete_custody_rosters() {
    let original = nested_fixture();
    for mutation in 0..6 {
        let mut checked = original.clone();
        match mutation {
            0 => checked.facts.borrow.reborrow_loan_resources.clear(),
            1 => checked
                .facts
                .borrow
                .reborrow_containment_certificates
                .clear(),
            2 => checked.facts.borrow.reborrow_disposition_events.clear(),
            3 => {
                let row = checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .iter()
                    .next()
                    .expect("resource")
                    .1
                    .clone();
                checked.facts.borrow.reborrow_loan_resources.append(row);
            }
            4 => {
                let row = checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .iter()
                    .next()
                    .expect("containment")
                    .1
                    .clone();
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .append(row);
            }
            _ => {
                let row = checked
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .iter()
                    .next()
                    .expect("disposition")
                    .1
                    .clone();
                checked.facts.borrow.reborrow_disposition_events.append(row);
            }
        }
        assert!(aliases(&checked).is_none(), "roster mutation {mutation}");
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

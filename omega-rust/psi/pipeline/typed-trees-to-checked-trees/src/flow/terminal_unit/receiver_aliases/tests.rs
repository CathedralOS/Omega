use super::*;

mod mutable;

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
            CheckedUnitEffectOperationPlan::Complete { .. }
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
fn projected_aliases_compose_capture_and_receiver_suffix_once() {
    for access in ["write", "mut"] {
        for (parameters, prefix, body, expected_path_length) in [
            (
                "records",
                "let held: &write Record = &write records[1];",
                "held.replace(value);",
                1,
            ),
            (
                "records",
                "let held: &write [Record; 2] = &write records; let child: &write Record = &write held[1];",
                "child.replace(value);",
                1,
            ),
            (
                "containers",
                "let held: &write Container = &write containers[1];",
                "held.records[1].replace(value);",
                3,
            ),
            (
                "containers",
                "let held: &write Container = &write containers[1]; let child: &write [Record; 2] = &write held.records;",
                "child[1].replace(value);",
                3,
            ),
            (
                "containers",
                "let held: &write Container = &write containers[1]; let child: &write [Record; 2] = &write held.records; let leaf: &write Record = &write child[1];",
                "leaf.replace(value);",
                3,
            ),
        ] {
            let element = if parameters == "records" {
                "Record"
            } else {
                "Container"
            };
            let checked = checked(&format!("data Record [copy] {{ value: u16; }}
                data Container [copy] {{ records: [Record; 2]; }}
                machine Record::replace(&write self, value: u16) {{ self.value = value; }}
                machine forward({parameters}: &{access} [{element}; 2], value: u16) {{ {prefix} {body} }}"));
            let aliases = aliases(&checked).expect("projected alias prefix");
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
                .expect("projected caller plan");
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                ..
            } = &plan.operations[0]
            else {
                panic!("receiver call");
            };
            assert_eq!(coordinate.statement_index as usize, aliases.len());
            let [argument] = structural_arguments.as_slice() else {
                panic!("one receiver");
            };
            assert_eq!(argument.source_parameter_index(), Some(0));
            assert_eq!(argument.path.len(), expected_path_length);
            assert_eq!(
                argument.path[0],
                CheckedUnitStructuralPathSegment::FixedIndex(1)
            );
            if expected_path_length == 3 {
                assert!(matches!(
                    argument.path[1],
                    CheckedUnitStructuralPathSegment::Field(_)
                ));
                assert_eq!(
                    argument.path[2],
                    CheckedUnitStructuralPathSegment::FixedIndex(1)
                );
            }
        }
    }
}

#[test]
fn projected_self_alias_retains_capture_and_normalized_receiver_paths() {
    let checked = checked(
        "data Record [copy] { value: u16; }
        data Container [copy] { records: [Record; 2]; }
        machine Record::replace(&write self, value: u16) { self.value = value; }
        machine Container::forward(&write self, value: u16) {
            let held: &write [Record; 2] = &write self.records;
            let leaf: &write Record = &write held[1];
            leaf.replace(value);
        }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Container::forward")
        .expect("attached caller");
    let state = checked.machine_states(machine).first().expect("entry");
    let aliases =
        prefix(&checked.typed, &checked.facts, machine, state).expect("projected self aliases");
    assert_eq!(aliases[1].root, machine.symbol);
    assert_eq!(aliases[1].segments.len(), 2);
    let resource = checked
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| row.owner_symbol == aliases[0].owner)
        .expect("direct resource")
        .1;
    assert_ne!(resource.captured_place.root_symbol, machine.symbol);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("projected attached caller plan");
    let CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &plan.operations[0]
    else {
        panic!("receiver call");
    };
    assert_eq!(structural_arguments[0].path.len(), 2);
    let StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("alias");
    };
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(local.initial_value)
    else {
        panic!("borrow");
    };
    let target = borrow.target;
    let wrong_field = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Record")
        .and_then(|definition| {
            checked
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) => Some(field.symbol),
                    _ => None,
                })
        })
        .expect("unrelated field");
    let mut forged = checked.clone();
    let ExpressionNode::Member(member) = forged.typed.expression_table.expression_mut(target)
    else {
        panic!("self member");
    };
    member.member_symbol = wrong_field;
    let machine = forged
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Container::forward")
        .expect("attached caller");
    let state = forged.machine_states(machine).first().expect("entry");
    assert!(
        prefix(&forged.typed, &forged.facts, machine, state).is_none(),
        "another declaration's field cannot replace inherited self field custody"
    );
}

#[test]
fn projected_aliases_reject_changed_capture_or_immediate_projection() {
    let original = fixture(
        "write",
        "let held: &write [Record; 2] = &write records; let child: &write Record = &write held[1];",
        "child.replace(value);",
    );
    assert!(aliases(&original).is_some());
    let resource_handle = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("child resource")
        .0;
    let certificate_handle = original
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .next()
        .expect("child containment")
        .0;
    for mutation in 0..5 {
        let mut checked = original.clone();
        match mutation {
            0 => checked
                .facts
                .borrow
                .reborrow_loan_resources
                .get_mut(resource_handle)
                .captured_place
                .segments
                .clear(),
            1 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(resource_handle)
                    .captured_place
                    .segments[0] = facts::PlaceSegment::FixedIndex { index: 0 }
            }
            2 => checked
                .facts
                .borrow
                .reborrow_containment_certificates
                .get_mut(certificate_handle)
                .projection_remainder
                .clear(),
            3 => checked
                .facts
                .borrow
                .reborrow_containment_certificates
                .get_mut(certificate_handle)
                .parent_place
                .segments
                .push(facts::PlaceSegment::FixedIndex { index: 1 }),
            _ => {
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .get_mut(certificate_handle)
                    .child_place
                    .segments[0] = facts::PlaceSegment::FixedIndex { index: 0 }
            }
        }
        assert!(
            aliases(&checked).is_none(),
            "projection mutation {mutation}"
        );
    }
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

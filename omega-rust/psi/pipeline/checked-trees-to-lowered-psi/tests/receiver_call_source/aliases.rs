//! Erased source-local carriers preserve the existing direct parameter subloan.

use super::*;

fn source(signature: &str, prefix: &str, calls: &str) -> String {
    format!(
        "data Record [copy] {{ value: u16; }}
         data Container {{ record: Record; records: [Record; 2]; }}
         data Nested {{ container: Container; }}
         machine Record::replace(&write self, value: u16) {{ self.value = value; }}
         machine {signature} {{ {prefix} {calls} }}"
    )
}

#[test]
fn direct_aliases_retain_whole_field_indexed_and_sequential_receivers() {
    for access in ["write", "mut"] {
        for (referent, calls) in [
            ("Record", "held.replace(value);"),
            ("Container", "held.record.replace(value);"),
            ("Nested", "held.container.record.replace(value);"),
            ("Container", "held.records[1].replace(value);"),
            (
                "[Record; 2]",
                "held[0].replace(value); held[1].replace(value);",
            ),
            ("[[Record; 2]; 2]", "held[1][0].replace(value);"),
        ] {
            let source = source(
                &format!("forward(value: u16, destination: &{access} {referent})"),
                &format!("let held: &write {referent} = &write destination;"),
                calls,
            );
            let checked = checked_from_source(&source);
            let _artifact = terminal_production::produce_terminal_artifact(&checked, "forward")
                .unwrap_or_else(|error| panic!("{source}: {error:?}"));
        }
    }
}

#[test]
fn independent_aliases_keep_distinct_parameter_roots() {
    let checked = checked_from_source(&source(
        "forward(first: &write Record, value: u16, second: &mut Record)",
        "let left: &write Record = &write first; let right: &write Record = &write second;",
        "left.replace(value); right.replace(value);",
    ));
    let artifact = terminal_production::produce_terminal_artifact(&checked, "forward").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(caller.blocks[0].operations.len(), 2);
    let places = caller.blocks[0]
        .operations
        .iter()
        .map(|operation| {
            let OperationKind::CallUnit {
                structural_arguments,
                ..
            } = &operation.kind
            else {
                panic!("receiver call");
            };
            assert_eq!(
                structural_arguments[0].access,
                StructuralAccess::WriteOnlyBorrow
            );
            structural_arguments[0].place
        })
        .collect::<Vec<_>>();
    assert_ne!(places[0], places[1]);
}

#[test]
fn attached_self_alias_retains_its_container() {
    for access in ["write", "mut"] {
        let checked = checked_from_source(&source(
            &format!("Container::forward(&{access} self, value: u16)"),
            "let held: &write Container = &write self;",
            "held.records[1].replace(value);",
        ));
        let _artifact =
            terminal_production::produce_terminal_artifact(&checked, "Container::forward").unwrap();
    }
}

#[test]
fn dotted_alias_replay_rejects_changed_field_path_and_endpoint() {
    use checked_trees::statement::StatementNode;
    let original = checked_from_source(&source(
        "forward(destination: &write Nested, value: u16)",
        "let held: &write Nested = &write destination;",
        "held.container.record.replace(value);",
    ));
    let caller = unit_plan(&original, "forward").machine;
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    for mutation in 0..3 {
        let mut checked = original.clone();
        if mutation == 0 {
            let StatementNode::Call(call) = &mut checked
                .typed
                .statement_table
                .statements_mut(state.statement_nodes)[1]
            else {
                panic!("dotted statement call");
            };
            call.receiver_symbol = call.receiver_root_symbol;
        } else {
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == caller)
                .unwrap();
            let CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            } = &mut plan.operations[0]
            else {
                panic!("call");
            };
            if mutation == 1 {
                structural_arguments[0].path.reverse();
            } else {
                structural_arguments[0].path.pop();
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "forward").is_err(),
            "field substitution {mutation}"
        );
    }
}

#[test]
fn alias_replay_rejects_initializer_and_operand_substitution() {
    use checked_trees::expression::ExpressionNode;
    use checked_trees::statement::StatementNode;
    let original = checked_from_source(&source(
        "forward(destination: &mut [Record; 2], other: &mut [Record; 2], value: u16)",
        "let held: &write [Record; 2] = &write destination;",
        "held[1].replace(value);",
    ));
    let caller = unit_plan(&original, "forward").machine;
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let other = original.state_parameters(state)[1].symbol;
    let StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("alias");
    };
    let initializer = local.initial_value;
    for mutation in 0..5 {
        let mut checked = original.clone();
        if mutation < 2 {
            let ExpressionNode::Borrow(borrow) =
                checked.typed.expression_table.expression_mut(initializer)
            else {
                panic!("borrow");
            };
            if mutation == 0 {
                borrow.access = language_semantics::ReferenceAccess::Mutable;
            } else {
                let target = borrow.target;
                let ExpressionNode::Name(name) =
                    checked.typed.expression_table.expression_mut(target)
                else {
                    panic!("root");
                };
                name.symbol = other;
                name.head_symbol = other;
            }
        } else {
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == caller)
                .unwrap();
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                structural_arguments,
                ..
            } = &mut plan.operations[0]
            else {
                panic!("call");
            };
            match mutation {
                2 => {
                    structural_arguments[0].path[0] =
                        checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0)
                }
                3 => {
                    structural_arguments[0].source =
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
                }
                _ => coordinate.statement_index = 0,
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "forward").is_err(),
            "substitution {mutation}"
        );
    }
}

#[test]
fn alias_replay_rejects_changed_source_loan_and_lifetime() {
    let original = checked_from_source(&source(
        "forward(destination: &write [Record; 2], value: u16)",
        "let held: &write [Record; 2] = &write destination;",
        "held[1].replace(value);",
    ));
    let _artifact = terminal_production::produce_terminal_artifact(&original, "forward").unwrap();
    let resource_handle = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, resource)| original.symbols.name(resource.owner_symbol) == "held")
        .unwrap()
        .0;
    for mutation in 0..9 {
        let mut checked = original.clone();
        let resource = checked
            .facts
            .borrow
            .direct_loan_resources
            .get_mut(resource_handle);
        match mutation {
            0 => resource.owner_symbol = symbols::SymbolHandle::invalid(),
            1 => resource.captured_place.root_symbol = symbols::SymbolHandle::invalid(),
            2 => resource.access = checked_trees::BorrowAccessKind::Mutable,
            3 => {
                resource.activation_source =
                    checked_trees::FlowInvalidationSource::Statement { statement_index: 1 }
            }
            4 => {
                resource.weakening_reason =
                    checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            }
            5 => resource.restoration.parent.root_symbol = symbols::SymbolHandle::invalid(),
            6 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(resource.loan)
                    .source_owner_symbol = resource.owner_symbol
            }
            7 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(resource.loan)
                    .last_use_statement_index = 0
            }
            _ => {
                checked.facts.borrow.loans.get_mut(resource.loan).kind =
                    checked_trees::BorrowAccessKind::Mutable
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "forward").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn nested_aliases_retain_immediate_parent_chains() {
    for access in ["mut", "write"] {
        for (prefix, depth) in [
            (
                "let held: &write [Record; 2] = &write destination; let child: &write [Record; 2] = &write held;",
                1,
            ),
            (
                "let held: &write [Record; 2] = &write destination; let middle: &write [Record; 2] = &write held; let child: &write [Record; 2] = &write middle;",
                2,
            ),
        ] {
            let text = source(
                &format!("forward(destination: &{access} [Record; 2], value: u16)"),
                prefix,
                "child[0].replace(value); child[1].replace(value);",
            );
            let checked = checked_from_source(&text);
            let artifact = terminal_production::produce_terminal_artifact(&checked, "forward")
                .unwrap_or_else(|error| panic!("{text}: {error:?}"));
            let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
            assert_eq!(module.reborrow_root_handoffs.len(), 1);
            assert_eq!(module.reborrow_root_handoffs[0].lineage.len(), depth);
        }
    }
}

#[test]
fn nested_alias_replay_rejects_changed_immediate_parent_and_lifecycle() {
    let original = checked_from_source(&source(
        "forward(destination: &write [Record; 2], value: u16)",
        "let held: &write [Record; 2] = &write destination; let middle: &write [Record; 2] = &write held; let child: &write [Record; 2] = &write middle;",
        "child[1].replace(value);",
    ));
    let artifact = terminal_production::produce_terminal_artifact(&original, "forward").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.reborrow_root_handoffs.len(), 1);
    assert_eq!(module.reborrow_root_handoffs[0].lineage.len(), 2);
    let (child_handle, child) = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, resource)| original.symbols.name(resource.owner_symbol) == "child")
        .unwrap();
    let (direct_handle, direct) = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, resource)| original.symbols.name(resource.owner_symbol) == "held")
        .unwrap();
    let certificate_handle = original
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .find(|(_, row)| row.child_resource == child_handle)
        .unwrap()
        .0;
    for mutation in 0..12 {
        let mut checked = original.clone();
        let resource = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get_mut(child_handle);
        match mutation {
            0 => resource.parent_loan = direct.loan,
            1 => {
                resource.parent_resource = checked_trees::CheckedParentBorrowResource::DirectRoot {
                    resource: direct_handle,
                }
            }
            2 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(child.loan)
                    .source_owner_symbol = direct.owner_symbol
            }
            3 => resource.access = checked_trees::BorrowAccessKind::Mutable,
            4 => resource.captured_place.root_symbol = symbols::SymbolHandle::invalid(),
            5 => {
                resource.parent_suspension.source =
                    checked_trees::FlowInvalidationSource::Statement { statement_index: 0 }
            }
            6 => resource.parent_suspension.child_activation = arena::Handle::invalid(),
            7 => resource.parent_suspension.parent_entry_constraint = arena::Handle::invalid(),
            8 => {
                checked
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .get_mut(child.parent_end_status.child_weakening)
                    .source =
                    checked_trees::FlowInvalidationSource::Statement { statement_index: 2 }
            }
            9 => {
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .get_mut(certificate_handle)
                    .child_resource = arena::Handle::invalid()
            }
            10 => {
                let duplicate = checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .get(certificate_handle)
                    .clone();
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .insert(duplicate);
            }
            _ => {
                checked.facts.borrow.loans.get_mut(child.loan).lineage =
                    checked_trees::BorrowLoanLineage::Reborrow {
                        parent_loan: direct.loan,
                    }
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "forward").is_err(),
            "nested mutation {mutation}"
        );
    }
}

#[test]
fn nested_alias_callee_retains_and_replays_its_handoff() {
    let mut text = source(
        "forward(destination: &write [Record; 2], value: u16)",
        "let held: &write [Record; 2] = &write destination; let middle: &write [Record; 2] = &write held; let child: &write [Record; 2] = &write middle;",
        "child[1].replace(value);",
    );
    text.push_str("machine wrapper(destination: &write [Record; 2], value: u16) { forward(&write destination, value); }");
    let original = checked_from_source(&text);
    let artifact = terminal_production::produce_terminal_artifact(&original, "wrapper").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.reborrow_root_handoffs.len(), 1);
    assert_ne!(module.reborrow_root_handoffs[0].machine, module.entry);
    assert_eq!(module.reborrow_root_handoffs[0].lineage.len(), 2);
    let (child_handle, _) = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, resource)| original.symbols.name(resource.owner_symbol) == "child")
        .unwrap();
    let certificate = original
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .find(|(_, row)| row.child_resource == child_handle)
        .unwrap()
        .0;
    let event = original
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .find(|(_, row)| row.child_resource == child_handle)
        .unwrap()
        .0;
    for mutation in 0..4 {
        let mut checked = original.clone();
        match mutation {
            0 => {
                checked
                    .facts
                    .borrow
                    .reborrow_containment_certificates
                    .get_mut(certificate)
                    .child_resource = arena::Handle::invalid()
            }
            1 => {
                checked
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .get_mut(event)
                    .child_resource = arena::Handle::invalid()
            }
            2 => checked
                .facts
                .borrow
                .reborrow_disposition_events
                .get_mut(event)
                .retired_parent_path
                .reverse(),
            _ => {
                checked
                    .facts
                    .borrow
                    .reborrow_disposition_events
                    .get_mut(event)
                    .retired_parent_path
                    .pop();
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "wrapper").is_err(),
            "callee custody mutation {mutation}"
        );
    }
}

//! Source and resource custody for statically captured alias projections.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

#[test]
fn projected_aliases_compose_formation_and_receiver_paths() {
    for access in ["write", "mut"] {
        for (signature, prefix, calls, path_length, lineage_depth) in [
            (
                format!("forward(destination: &{access} [Record; 2], value: u16)"),
                "let held: &write Record = &write destination[1];",
                "held.replace(value);",
                1,
                0,
            ),
            (
                format!("forward(destination: &{access} Container, value: u16)"),
                "let held: &write Record = &write destination.record;",
                "held.replace(value);",
                1,
                0,
            ),
            (
                format!("forward(destination: &{access} Nested, value: u16)"),
                "let held: &write Container = &write destination.container;",
                "held.records[1].replace(value);",
                3,
                0,
            ),
            (
                format!("forward(destination: &{access} Nested, value: u16)"),
                "let held: &write Container = &write destination.container; let child: &write Record = &write held.records[1];",
                "child.replace(value);",
                3,
                1,
            ),
            (
                format!("forward(destination: &{access} [[Record; 2]; 2], value: u16)"),
                "let held: &write [Record; 2] = &write destination[1]; let child: &write Record = &write held[0];",
                "child.replace(value);",
                2,
                1,
            ),
            (
                format!("Container::forward(&{access} self, value: u16)"),
                "let held: &write Record = &write self.records[1];",
                "held.replace(value);",
                2,
                0,
            ),
            (
                format!("Nested::forward(&{access} self, value: u16)"),
                "let held: &write Container = &write self.container; let child: &write Record = &write held.records[1];",
                "child.replace(value);",
                3,
                1,
            ),
            (
                format!("Container::forward(&{access} self, value: u16)"),
                "let held: &write Record = &write records[1];",
                "held.replace(value);",
                2,
                0,
            ),
            (
                format!("Nested::forward(&{access} self, value: u16)"),
                "let held: &write Container = &write container; let child: &write Record = &write held.records[1];",
                "child.replace(value);",
                3,
                1,
            ),
        ] {
            let text = source(&signature, prefix, calls);
            let checked = checked_from_source(&text);
            let caller_name = signature.split_once('(').unwrap().0;
            let plan = unit_plan(&checked, caller_name);
            let CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            } = &plan.operations[0]
            else {
                panic!("projected receiver call");
            };
            assert_eq!(structural_arguments[0].path.len(), path_length, "{text}");
            let artifact = terminal_production::produce_terminal_artifact(&checked, caller_name)
                .unwrap_or_else(|error| panic!("{text}: {error:?}"));
            let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
            assert_eq!(
                module.reborrow_root_handoffs.len(),
                usize::from(lineage_depth != 0)
            );
            if lineage_depth != 0 {
                assert_eq!(
                    module.reborrow_root_handoffs[0].lineage.len(),
                    lineage_depth
                );
            }
        }
    }
}

#[test]
fn bare_attached_alias_capture_keeps_its_authored_name_identity() {
    let original = checked_from_source(&source(
        "Container::forward(&write self, value: u16)",
        "let held: &write Record = &write records[1];",
        "held.replace(value);",
    ));
    let artifact =
        terminal_production::produce_terminal_artifact(&original, "Container::forward").unwrap();
    assert!(terminal_codec::decode_module(artifact.semantic_bytes()).is_ok());
    let (resource_handle, resource) = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "held")
        .unwrap();
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == resource.machine_symbol)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("alias");
    };
    let ExpressionNode::Borrow(borrow) = original.expression_table.expression(local.initial_value)
    else {
        panic!("borrow");
    };
    let ExpressionNode::Indexed(indexed) = original.expression_table.expression(borrow.target)
    else {
        panic!("index");
    };
    let name_handle = indexed.collection;
    let ExpressionNode::Name(name) = original.expression_table.expression(name_handle) else {
        panic!("bare field");
    };
    let field =
        validation::exact_attached_field(&original.typed, machine, name.symbol, "records").unwrap();
    assert_eq!(resource.captured_place.root_symbol, name.symbol);
    assert_ne!(name.symbol, field.symbol);
    for mutation in 0..3 {
        let mut checked = original.clone();
        match mutation {
            0 => {
                checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get_mut(resource_handle)
                    .captured_place
                    .root_symbol = field.symbol
            }
            1 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(resource.loan)
                    .root_symbol = field.symbol
            }
            _ => {
                let ExpressionNode::Name(name) =
                    checked.typed.expression_table.expression_mut(name_handle)
                else {
                    panic!("bare field");
                };
                name.symbol = field.symbol;
                name.head_symbol = field.symbol;
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "Container::forward").is_err(),
            "bare capture mutation {mutation}"
        );
    }
}

#[test]
fn projected_alias_replay_rejects_source_and_captured_place_substitution() {
    let original = checked_from_source(&source(
        "forward(destination: &write [Record; 2], value: u16)",
        "let held: &write Record = &write destination[1];",
        "held.replace(value);",
    ));
    let artifact = terminal_production::produce_terminal_artifact(&original, "forward").unwrap();
    assert!(terminal_codec::decode_module(artifact.semantic_bytes()).is_ok());
    let caller = unit_plan(&original, "forward").machine;
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("alias");
    };
    let initializer = local.initial_value;
    let ExpressionNode::Borrow(borrow) = original.expression_table.expression(initializer) else {
        panic!("borrow");
    };
    let projection = borrow.target;
    let ExpressionNode::Indexed(indexed) = original.expression_table.expression(projection) else {
        panic!("projection");
    };
    let collection = indexed.collection;
    let (resource_handle, resource) = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, resource)| resource.owner_symbol == local.symbol)
        .unwrap();
    for mutation in 0..6 {
        let mut checked = original.clone();
        match mutation {
            0 => {
                let ExpressionNode::Borrow(borrow) =
                    checked.typed.expression_table.expression_mut(initializer)
                else {
                    panic!("borrow");
                };
                borrow.target = collection;
            }
            1 => checked
                .facts
                .borrow
                .direct_loan_resources
                .get_mut(resource_handle)
                .captured_place
                .segments
                .clear(),
            2 => {
                checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get_mut(resource_handle)
                    .captured_place
                    .segments[0] = facts::PlaceSegment::FixedIndex { index: 0 }
            }
            3 => {
                let ExpressionNode::Indexed(indexed) =
                    checked.typed.expression_table.expression_mut(projection)
                else {
                    panic!("projection");
                };
                indexed.index = collection;
            }
            4 => {
                checked.facts.borrow.loans.get_mut(resource.loan).segments =
                    arena::HandleSpan::empty()
            }
            _ => {
                let CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } = &mut checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == caller)
                    .unwrap()
                    .operations[0]
                else {
                    panic!("call");
                };
                structural_arguments[0].path[0] =
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0);
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "forward").is_err(),
            "projected alias mutation {mutation}"
        );
    }
}

#[test]
fn projected_self_alias_replay_binds_field_capture_and_nested_suffix() {
    let original = checked_from_source(&source(
        "Nested::forward(&write self, value: u16)",
        "let held: &write Container = &write self.container; let child: &write Record = &write held.records[1];",
        "child.replace(value);",
    ));
    let artifact =
        terminal_production::produce_terminal_artifact(&original, "Nested::forward").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.reborrow_root_handoffs[0].lineage.len(), 1);
    let (direct_handle, direct) = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "held")
        .unwrap();
    assert_eq!(
        original.symbols.name(direct.captured_place.root_symbol),
        "container"
    );
    assert!(direct.captured_place.segments.is_empty());
    let (child_handle, child) = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "child")
        .unwrap();
    assert_eq!(
        child.captured_place.root_symbol,
        direct.captured_place.root_symbol
    );
    assert_eq!(child.captured_place.segments.len(), 2);
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == direct.machine_symbol)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("alias");
    };
    let ExpressionNode::Borrow(borrow) = original.expression_table.expression(local.initial_value)
    else {
        panic!("borrow");
    };
    let member = borrow.target;
    for mutation in 0..3 {
        let mut checked = original.clone();
        match mutation {
            0 => {
                let ExpressionNode::Member(member) =
                    checked.typed.expression_table.expression_mut(member)
                else {
                    panic!("member");
                };
                member.member_symbol = direct.owner_symbol;
            }
            1 => {
                checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get_mut(direct_handle)
                    .captured_place
                    .root_symbol = direct.machine_symbol
            }
            _ => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child_handle)
                    .captured_place
                    .segments
                    .pop()
                    .unwrap();
            }
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "Nested::forward").is_err(),
            "projected self alias mutation {mutation}"
        );
    }
}

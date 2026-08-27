use super::SyntaxTrees;
use crate::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableMemberExpression,
};
use crate::identifier::Identifier;
use crate::item::{
    DataDefinition, Item, Machine, State, StateSignature, TraitDefinition, WireDataDefinition,
};
use crate::snapshot::ItemSnapshot;
use crate::statement::{
    StatementNode, TableAssignment, TableCall, TableTransition, TransitionGuardNode,
    TransitionTargetNode,
};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_arena::HandleSpan;

#[test]
fn syntax_trees_extend_from_preserves_data_visibility() {
    let mut file = SyntaxTrees::new(Default::default());
    file.push_root_item(Item::Data(DataDefinition {
        name: Identifier::generated("PublicRecord"),
        is_public: true,
        supply_mode: psi_language_core::DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        generic_instance: None,
        properties: Default::default(),
        quotient: None,
        where_facts: HandleSpan::empty(),
        members: HandleSpan::empty(),
    }));

    let mut assembled = SyntaxTrees::new(Default::default());
    assembled.extend_from(&file);

    let Item::Data(data) = assembled.root_items().next().expect("data root") else {
        panic!("expected data root item");
    };
    assert!(data.is_public);
}

#[test]
fn syntax_copy_and_snapshot_preserve_generic_instance_origin() {
    let mut source = SyntaxTrees::new(Default::default());
    let argument = source
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("Message")));
    let arguments = source
        .type_references
        .insert_type_reference_handles([argument]);
    let origin = source.type_references.insert(TypeReferenceNode::Generic {
        base_name: Identifier::generated("Carrier"),
        lifetime_arguments: Vec::new(),
        arguments,
    });
    source.push_root_item(Item::Data(DataDefinition {
        name: Identifier::generated("irrelevant synthetic name"),
        is_public: false,
        supply_mode: psi_language_core::DataSupplyMode::CheckedShape,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        generic_instance: Some(origin),
        properties: Default::default(),
        quotient: None,
        where_facts: HandleSpan::empty(),
        members: HandleSpan::empty(),
    }));

    let mut copied = SyntaxTrees::new(Default::default());
    copied.extend_from(&source);
    let Item::Data(data) = copied.root_items().next().expect("copied data") else {
        panic!("copied root must remain data");
    };
    let origin = data.generic_instance.expect("copied generic origin");
    assert!(matches!(
        copied.type_references.type_reference(origin),
        TypeReferenceNode::Generic { base_name, .. } if base_name.as_str() == "Carrier"
    ));

    let snapshot = copied.snapshot();
    assert!(matches!(
        &snapshot.root_items[0],
        ItemSnapshot::Data {
            generic_instance: Some(crate::snapshot::TypeReferenceSnapshot::Generic { .. }),
            ..
        }
    ));
}

#[test]
fn syntax_trees_extend_from_preserves_trait_and_wire_visibility() {
    let mut file = SyntaxTrees::new(Default::default());
    file.push_root_item(Item::Trait(TraitDefinition {
        is_boundary: false,
        is_public: true,
        name: Identifier::generated("PublicTrait"),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        parents: HandleSpan::empty(),
        requires: HandleSpan::empty(),
        machines: HandleSpan::empty(),
    }));
    file.push_root_item(Item::WireData(WireDataDefinition {
        name: Identifier::generated("PublicWire"),
        is_public: true,
        encoding: None,
        members: HandleSpan::empty(),
    }));

    let mut assembled = SyntaxTrees::new(Default::default());
    assembled.extend_from(&file);

    let mut roots = assembled.root_items();
    let Item::Trait(trait_definition) = roots.next().expect("trait root") else {
        panic!("expected trait root item");
    };
    let Item::WireData(wire_data) = roots.next().expect("wire root") else {
        panic!("expected wire-data root item");
    };
    assert!(trait_definition.is_public);
    assert!(wire_data.is_public);

    let snapshot = assembled.snapshot();
    assert!(matches!(
        &snapshot.root_items[0],
        ItemSnapshot::Trait {
            is_public: true,
            ..
        }
    ));
    assert!(matches!(
        &snapshot.root_items[1],
        ItemSnapshot::WireData {
            is_public: true,
            ..
        }
    ));
}

#[test]
fn syntax_trees_collect_state_expression_and_type_payloads() {
    let mut syntax_trees = SyntaxTrees::new(Default::default());
    let guard = syntax_trees
        .expressions
        .insert(crate::expression::ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(1),
        ));
    let target = syntax_trees
        .statements
        .insert_transition_target(TransitionTargetNode::Terminal);
    let statement = syntax_trees
        .statements
        .insert(StatementNode::Transition(TableTransition {
            target,
            continuation: crate::statement::TransitionTargetHandle::invalid(),
            guard: TransitionGuardNode::When(guard),
            proof_selectors: HandleSpan::empty(),
            exit: Default::default(),
            source_span: Default::default(),
        }));
    let statement_handle = syntax_trees.items.append_statement_handle(statement);
    let statements = HandleSpan::from_parts(statement_handle, 1);
    let return_type = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
    let state = syntax_trees.items.insert_state(&State {
        name: Identifier::generated("entry"),
        parameters: HandleSpan::empty(),
        return_type,
        contracts: HandleSpan::empty(),
        statements,
    });
    let state_handle = syntax_trees.items.append_state_handle(state);

    syntax_trees.push_root_item(Item::Machine(Machine {
        name: Identifier::generated("Main"),
        attached_data: None,
        is_public: false,
        target: None,
        boundary: false,
        bodyless: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: crate::expression::ExpressionHandle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::from_parts(state_handle, 1),
    }));

    assert_eq!(syntax_trees.root_item_count(), 1);
    assert_eq!(syntax_trees.type_references.type_reference_count(), 1);
    assert_eq!(syntax_trees.expressions.expression_count(), 1);
    assert_eq!(syntax_trees.statements.statement_count(), 1);
    assert_eq!(syntax_trees.items.machine_count(), 1);
    assert_eq!(syntax_trees.items.state_count(), 1);
}

#[test]
fn syntax_trees_extend_from_preserves_root_payload_handles() {
    let mut file = SyntaxTrees::new(Default::default());
    let suspends_keyword_source_span =
        psi_source::SourceSpan::new(Default::default(), psi_source::Span::new(10, 18));
    let blocks_keyword_source_span =
        psi_source::SourceSpan::new(Default::default(), psi_source::Span::new(20, 26));
    let return_type = file
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
    let state = file.items.insert_state(&State {
        name: Identifier::generated("entry"),
        parameters: HandleSpan::empty(),
        return_type,
        contracts: HandleSpan::empty(),
        statements: HandleSpan::empty(),
    });
    let state = file.items.append_state_handle(state);
    file.push_root_item(Item::Machine(Machine {
        name: Identifier::generated("main"),
        attached_data: None,
        is_public: true,
        target: None,
        boundary: false,
        bodyless: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: crate::expression::ExpressionHandle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: vec![suspends_keyword_source_span],
        blocks_keyword_source_spans: vec![blocks_keyword_source_span],
        suspends: true,
        blocks: true,
        contracts: HandleSpan::empty(),
        states: HandleSpan::from_parts(state, 1),
    }));

    let mut assembled = SyntaxTrees::new(Default::default());
    assembled.extend_from(&file);

    let Item::Machine(machine) = assembled.root_items().next().expect("machine root") else {
        panic!("expected machine root item");
    };
    assert!(machine.is_public, "syntax assembly must retain visibility");
    assert_eq!(
        machine.suspends_keyword_source_spans,
        [suspends_keyword_source_span]
    );
    assert_eq!(
        machine.blocks_keyword_source_spans,
        [blocks_keyword_source_span]
    );
    let state_handle = assembled
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state handle");
    let state = assembled.items.state(state_handle);
    assert_eq!(state.name.as_str(), "entry");
    assert!(state.return_type.is_valid());
}

#[test]
fn syntax_signature_copy_preserves_operational_keyword_sources() {
    let mut source = SyntaxTrees::new(Default::default());
    let suspends_keyword_source_span =
        psi_source::SourceSpan::new(Default::default(), psi_source::Span::new(30, 38));
    let blocks_keyword_source_span =
        psi_source::SourceSpan::new(Default::default(), psi_source::Span::new(40, 46));
    let signature = source.items.insert_state_signature(&StateSignature {
        name: Identifier::generated("wait"),
        spelling: None,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        is_default: true,
        parameters: HandleSpan::empty(),
        return_type: TypeReferenceHandle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: vec![suspends_keyword_source_span],
        blocks_keyword_source_spans: vec![blocks_keyword_source_span],
        suspends: true,
        blocks: true,
        contracts: HandleSpan::empty(),
        default_body: HandleSpan::empty(),
        terminates_guarantee: false,
    });

    let mut copied_trees = SyntaxTrees::new(Default::default());
    let copied = copied_trees
        .copy_state_signature_node_from(&source, source.items.state_signature(signature));

    assert_eq!(
        copied.suspends_keyword_source_spans,
        [suspends_keyword_source_span]
    );
    assert_eq!(
        copied.blocks_keyword_source_spans,
        [blocks_keyword_source_span]
    );
    assert!(copied.suspends);
    assert!(copied.blocks);
}

#[test]
fn syntax_trees_extend_from_preserves_statement_call_arguments() {
    let mut file = SyntaxTrees::new(Default::default());
    let receiver = file
        .statements
        .append_identifier_path_member(Identifier::generated("self"));
    let receiver = HandleSpan::from_parts(receiver, 1);
    let argument = file
        .expressions
        .insert(crate::expression::ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(0),
        ));
    let argument = file.statements.append_expression_handle(argument);
    let call = file.statements.insert(StatementNode::Call(TableCall {
        receiver,
        receiver_starts_at_self: true,
        target: Identifier::generated("take_non_negative"),
        machine_arguments: Box::default(),
        arguments: HandleSpan::from_parts(argument, 1),
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
        discards_result: false,
    }));
    let call = file.items.append_statement_handle(call);
    let state = file.items.insert_state(&State {
        name: Identifier::generated("entry"),
        parameters: HandleSpan::empty(),
        return_type: TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        statements: HandleSpan::from_parts(call, 1),
    });
    let state = file.items.append_state_handle(state);
    file.push_root_item(Item::Machine(Machine {
        name: Identifier::generated("main"),
        attached_data: None,
        is_public: false,
        target: None,
        boundary: false,
        bodyless: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: crate::expression::ExpressionHandle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::from_parts(state, 1),
    }));

    let mut assembled = SyntaxTrees::new(Default::default());
    assembled.extend_from(&file);

    let Item::Machine(machine) = assembled.root_items().next().expect("machine root") else {
        panic!("expected machine root item");
    };
    let state_handle = assembled
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state handle");
    let state = assembled.items.state(state_handle);
    let statement_handle = assembled
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("call statement");
    let StatementNode::Call(call) = assembled.statements.statement(statement_handle) else {
        panic!("expected call statement");
    };
    assert_eq!(
        assembled
            .statements
            .expression_handles(call.arguments)
            .len(),
        1
    );
}

#[test]
fn syntax_trees_extend_from_preserves_nested_expression_argument_spans() {
    let mut file = SyntaxTrees::new(Default::default());
    let target_name = file
        .expressions
        .append_identifier_path_member(Identifier::generated("xp"));
    let target = file
        .expressions
        .insert(ExpressionNode::Name(HandleSpan::from_parts(target_name, 1)));

    let player_name = file
        .expressions
        .append_identifier_path_member(Identifier::generated("player"));
    let player = file
        .expressions
        .insert(ExpressionNode::Name(HandleSpan::from_parts(player_name, 1)));
    let player_level = file
        .expressions
        .insert(ExpressionNode::Member(TableMemberExpression {
            receiver: player,
            member: Identifier::generated("level"),
            case_variant: None,
        }));

    let self_value = file.expressions.insert(ExpressionNode::SelfValue);
    let nested_arguments = file.expressions.insert_expression_handles([player_level]);
    let nested_call = file
        .expressions
        .insert(ExpressionNode::Call(TableCallExpression {
            receiver: self_value,
            target: Identifier::generated("xp_required"),
            machine_arguments: Box::default(),
            arguments: nested_arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }));

    let zero = file.expressions.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::from_value(0),
    ));
    let max_arguments = file
        .expressions
        .insert_expression_handles([zero, nested_call]);
    let max_call = file
        .expressions
        .insert(ExpressionNode::Call(TableCallExpression {
            receiver: ExpressionHandle::invalid(),
            target: Identifier::generated("max"),
            machine_arguments: Box::default(),
            arguments: max_arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }));

    let statement = file
        .statements
        .insert(StatementNode::Assignment(TableAssignment {
            target,
            value: max_call,
        }));
    let statement = file.items.append_statement_handle(statement);
    let state = file.items.insert_state(&State {
        name: Identifier::generated("entry"),
        parameters: HandleSpan::empty(),
        return_type: TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        statements: HandleSpan::from_parts(statement, 1),
    });
    let state = file.items.append_state_handle(state);
    file.push_root_item(Item::Machine(Machine {
        name: Identifier::generated("main"),
        attached_data: None,
        is_public: false,
        target: None,
        boundary: false,
        bodyless: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: crate::expression::ExpressionHandle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::from_parts(state, 1),
    }));

    let mut assembled = SyntaxTrees::new(Default::default());
    assembled.extend_from(&file);

    let Item::Machine(machine) = assembled.root_items().next().expect("machine root") else {
        panic!("expected machine root item");
    };
    let state_handle = assembled
        .items
        .state_handles(machine.states)
        .first()
        .copied()
        .expect("entry state handle");
    let state = assembled.items.state(state_handle);
    let statement_handle = assembled
        .items
        .statements(state.statements)
        .first()
        .copied()
        .expect("assignment statement");
    let StatementNode::Assignment(assignment) = assembled.statements.statement(statement_handle)
    else {
        panic!("expected assignment statement");
    };

    assert_eq!(
        assembled.expressions.display_name(assignment.value),
        "max(0, self.xp_required(player.level))"
    );
}

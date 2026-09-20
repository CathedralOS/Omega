use super::{
    TheoremSchemaMutation, binary_expression, call_with_arguments, carrier_type,
    fixed_call_fixture, named_argument, push_generic_representative_application,
    push_representative, quotient_type, request_with_representative,
    selected_theorem_schema_fixture, static_argument, symbol,
};
use crate::proof_contracts::quotients::relation_plan::{
    RelationPlanError, RepresentativeContractFactLocation, RepresentativeContractOwner,
    complete_single_state_result_flow, complete_state_forwarding_result_flow,
    derive_define_precondition_correspondence, derive_direct_terminal_plan,
    derive_exact_representative_static_application, fallthrough_result_root,
    immutable_alias_fallthrough_root, render_failed_builtin_implication,
    render_implication_coordinate_diagnostic,
};
use arena::HandleSpan;
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, QuotientDefinition, TypeParameter, TypeParameterKind};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionNode, QuotientOperationKind, StaticSymbolApplication,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::proposition::PropositionDefinition;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData, TableTransition};
use typed_trees::types::TypeReferenceNode;

#[test]
fn define_fixed_preconditions_reject_arithmetic_weakening() {
    let fixture = fixed_call_fixture(false, |program, public, representative, _| {
        let public = named_argument(program, "fixed", public);
        let two = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let q = binary_expression(program, public, BinaryOperator::Greater, two);
        let representative = named_argument(program, "fixed", representative);
        let one = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let p = binary_expression(program, representative, BinaryOperator::Greater, one);
        (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
    });
    let positions = fixture
        .runtime
        .positions
        .iter()
        .map(|position| {
            let crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DirectLiftArgumentSource::PublicParameter(public_parameter) =
                position.source
            else {
                panic!("define fixture must use only direct public parameters")
            };
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
                public_parameter,
                representative_parameter: position.representative_parameter,
            }
        })
        .collect();
    assert_eq!(
        derive_define_precondition_correspondence(
            &fixture.program,
            &fixture.public_machine,
            &fixture.public_state,
            &fixture.representative,
            &fixture.public_partition,
            &fixture.representative_partition,
            &crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimeCorrespondence { positions },
        ),
        Err(RelationPlanError::DefinePreconditionMismatch),
        "define fixed Q<=>P remains an exact bijection, never arithmetic weakening",
    );
}

#[test]
fn define_correspondence_applies_closed_representative_type_substitution() {
    let mut program = TypedTrees::default();
    let mut request = push_generic_representative_application(&mut program);
    request.kind = QuotientOperationKind::Define;
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(600),
            name: Identifier::generated_static("StaticType"),
        });
    program.push_proposition(PropositionDefinition {
        symbol: symbol(2),
        name: Identifier::generated_static("ExactR"),
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        symbol: symbol(1),
        name: Identifier::generated_static("ExactQ"),
        quotient: Some(QuotientDefinition {
            carrier,
            relation: vec![Identifier::generated_static("ExactR")],
            relation_symbol: symbol(2),
            equivalence: None,
        }),
        ..Default::default()
    });
    let quotient = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(1),
            name: Identifier::generated_static("ExactQ"),
        });
    let public_symbol = symbol(3);
    let argument = named_argument(&mut program, "value", public_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([argument]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient,
            ..Default::default()
        },
    );

    let plan = derive_direct_terminal_plan(
        &program,
        &program,
        &Machine::default(),
        &state,
        &call,
        &request,
    )
    .expect("closed T := StaticType must instantiate the runtime telescope");
    assert_eq!(
        plan.representative_precondition,
        Some(crate::proof_contracts::quotients::relation_plan::precondition::RepresentativePreconditionPartition {
            dependent: Vec::new(),
            fixed: Vec::new(),
        })
    );
    assert_eq!(
        plan.public_precondition,
        Some(crate::proof_contracts::quotients::relation_plan::precondition::RepresentativePreconditionPartition {
            dependent: Vec::new(),
            fixed: Vec::new(),
        })
    );
    assert_eq!(
        plan.define_precondition_correspondence,
        Some(crate::proof_contracts::quotients::relation_plan::precondition::DefinePreconditionCorrespondence {
            dependent: Vec::new(),
            fixed: Vec::new(),
        })
    );
    assert_eq!(
        plan.define_correspondence
            .expect("define correspondence")
            .positions,
        vec![crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
            public_parameter: public_symbol,
            representative_parameter: symbol(625),
        }]
    );
}

#[test]
fn representative_static_application_rejects_const_category_near_miss() {
    let mut program = TypedTrees::default();
    let mut request = push_generic_representative_application(&mut program);
    let application = request
        .representative_operation
        .application
        .as_mut()
        .expect("generic application");
    application.arguments[1] = application.arguments[0].clone();

    assert_eq!(
        derive_exact_representative_static_application(&program, &request),
        Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(1))
    );
}

#[test]
fn define_runtime_correspondence_rejects_reordered_public_parameters() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let carrier_type = carrier_type(&mut program);
    let left_symbol = symbol(3);
    let right_symbol = symbol(4);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([right, left]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    for (parameter_symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference: quotient_type,
                ..Default::default()
            },
        );
    }
    let mut request = push_representative(
        &mut program,
        &[(carrier_type, false, false), (carrier_type, false, false)],
        carrier_type,
    );
    request.kind = QuotientOperationKind::Define;

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &program,
            &Machine::default(),
            &state,
            &call,
            &request,
        ),
        Err(RelationPlanError::DefineArgumentOrderMismatch(0))
    );
}

#[test]
fn derived_direct_terminal_plan_remains_non_executable() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let value_symbol = symbol(3);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let mut call = call_with_arguments(arguments);
    let carrier_type = carrier_type(&mut program);
    // Attached and free operations share the normalized positional form:
    // the representative receiver occupies position zero without forcing
    // the public wrapper parameter to be spelled `self`.
    let mut request =
        push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
    request.kind = QuotientOperationKind::Define;
    call.quotient_operation = Some(request);
    let call = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: value_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            ..Default::default()
        },
    );
    program
        .statement_table
        .push_statement(&mut state.statement_nodes, StatementNode::Expression(call));
    let mut machine = Machine::default();
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    crate::proof_contracts::quotients::formation_collection::reject_quotient_operation_requests(
        &program,
        &program,
        &mut diagnostics,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("compiler-derived direct-terminal relations RA=[0:")
    );
    assert!(diagnostics[0].message.contains("RR="));
    assert!(diagnostics[0].message.contains("define-runtime=[0]"));
    assert!(diagnostics[0].message.contains("Q=[dependent:0, fixed:0]"));
    assert!(diagnostics[0].message.contains("P=[dependent:0, fixed:0]"));
    assert!(
        diagnostics[0]
            .message
            .contains("Q<->P=[dependent:0, fixed:0]")
    );
    assert!(diagnostics[0].message.contains(
        "theorem-schema=[parameters:2, relations:1, legality:0, applications:2, conclusion:1]"
    ));
    assert!(
        diagnostics[0]
            .message
            .contains("exact selected theorem schema verification")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("checked pure representative effect summary")
    );
    assert!(!diagnostics[0].message.contains("the effect fence"));
    assert!(
        diagnostics[0]
            .message
            .contains("one unchanged state-fallthrough result edge through the exact result root")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("executable quotient operations are not admitted")
    );
}

#[test]
fn immutable_alias_fallthrough_requires_an_exact_immutable_chain() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let first_symbol = symbol(10);
    let second_symbol = symbol(11);
    let first_name = named_argument(&mut program, "first", first_symbol);
    let second_name = named_argument(&mut program, "second", second_symbol);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    for local in [
        TableLocalData {
            symbol: first_symbol,
            name: Identifier::generated_static("first"),
            type_reference: quotient_type,
            initial_value: request,
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
        TableLocalData {
            symbol: second_symbol,
            name: Identifier::generated_static("second"),
            type_reference: quotient_type,
            initial_value: first_name,
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    ] {
        program
            .statement_table
            .push_statement(&mut state.statement_nodes, StatementNode::LocalData(local));
    }
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(second_name),
    );

    assert_eq!(
        immutable_alias_fallthrough_root(&program, &state),
        Some(crate::proof_contracts::quotients::relation_plan::result_flow::ImmutableAliasFallthroughRoot {
            request_expression: request,
            alias_count: 2,
        })
    );

    let drifted_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    if let StatementNode::LocalData(first) = &mut program
        .statement_table
        .statements_mut(state.statement_nodes)[0]
    {
        first.type_reference = drifted_type;
    }
    assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);

    if let StatementNode::LocalData(first) = &mut program
        .statement_table
        .statements_mut(state.statement_nodes)[0]
    {
        first.type_reference = quotient_type;
        first.is_mutable = true;
    }
    assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);
}

#[test]
fn complete_result_flow_requires_one_exact_machine_state() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        symbol: symbol(31),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    let mut machine = Machine {
        symbol: symbol(30),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let root = {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let root = fallthrough_result_root(&program, state).expect("exact result root");
        assert_eq!(
            complete_single_state_result_flow(&program, machine, state, root),
            Some(crate::proof_contracts::quotients::relation_plan::result_flow::CompleteSingleStateResultFlow {
                machine_symbol: symbol(30),
                state_symbol: symbol(31),
                root,
            })
        );
        root
    };

    let mut machine = program.machines()[0].clone();
    program.push_machine_state(
        &mut machine,
        State {
            symbol: symbol(32),
            return_type,
            ..Default::default()
        },
    );
    program.machines_mut()[0] = machine;
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    assert_eq!(
        complete_single_state_result_flow(&program, machine, state, root),
        None
    );
}

#[test]
fn complete_result_flow_rejects_a_transition_before_fallthrough() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        symbol: symbol(41),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition::default()),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    let mut machine = Machine {
        symbol: symbol(40),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let root = fallthrough_result_root(&program, state).expect("fallthrough edge still exists");

    assert_eq!(
        complete_single_state_result_flow(&program, machine, state, root),
        None
    );
}

#[test]
fn complete_result_flow_accepts_exact_finite_state_forwarding() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));

    let mut result_state = State {
        symbol: symbol(52),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut result_state.statement_nodes,
        StatementNode::Expression(request),
    );

    let target = program.statement_table.insert_transition_target(
        typed_trees::statement::TransitionTargetNode::Named {
            static_machine_parameter: SymbolHandle::invalid(),
            path: typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(52),
                symbol: symbol(52),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        },
    );
    let mut forwarding_state = State {
        symbol: symbol(51),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut forwarding_state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            ..Default::default()
        }),
    );
    let mut machine = Machine {
        symbol: symbol(50),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, forwarding_state);
    program.push_machine_state(&mut machine, result_state);
    program.push_machine(machine);

    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    let root = fallthrough_result_root(&program, result_state).expect("exact result root");
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        Some(crate::proof_contracts::quotients::relation_plan::result_flow::CompleteStateForwardingResultFlow {
            machine_symbol: symbol(50),
            forwarding_edges: vec![crate::proof_contracts::quotients::relation_plan::result_flow::StateForwardingEdge {
                source_state_symbol: symbol(51),
                target_state_symbol: symbol(52),
            }],
            result_state_symbol: symbol(52),
            root,
        })
    );

    let intermediate_target = program.statement_table.insert_transition_target(
        typed_trees::statement::TransitionTargetNode::Named {
            static_machine_parameter: SymbolHandle::invalid(),
            path: typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(53),
                symbol: symbol(53),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        },
    );
    let forwarding_span = program.machine_states(&program.machines()[0])[0].statement_nodes;
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.target = intermediate_target;
    let mut intermediate_state = State {
        symbol: symbol(53),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut intermediate_state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            ..Default::default()
        }),
    );
    let mut expanded_machine = program.machines()[0].clone();
    program.push_machine_state(&mut expanded_machine, intermediate_state);
    program.machines_mut()[0] = expanded_machine;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        Some(crate::proof_contracts::quotients::relation_plan::result_flow::CompleteStateForwardingResultFlow {
            machine_symbol: symbol(50),
            forwarding_edges: vec![
                crate::proof_contracts::quotients::relation_plan::result_flow::StateForwardingEdge {
                    source_state_symbol: symbol(51),
                    target_state_symbol: symbol(53),
                },
                crate::proof_contracts::quotients::relation_plan::result_flow::StateForwardingEdge {
                    source_state_symbol: symbol(53),
                    target_state_symbol: symbol(52),
                },
            ],
            result_state_symbol: symbol(52),
            root,
        })
    );

    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.continuation = target;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );

    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.continuation = typed_trees::statement::TransitionTargetHandle::invalid();

    let cycle_target = program.statement_table.insert_transition_target(
        typed_trees::statement::TransitionTargetNode::Named {
            static_machine_parameter: SymbolHandle::invalid(),
            path: typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(51),
                symbol: symbol(51),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        },
    );
    let intermediate_span = program.machine_states(&program.machines()[0])[2].statement_nodes;
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(intermediate_span)
    else {
        panic!("one intermediate transition")
    };
    transition.target = cycle_target;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(intermediate_span)
    else {
        panic!("one intermediate transition")
    };
    transition.target = target;

    let mut duplicate_owner = Machine {
        symbol: symbol(60),
        ..Default::default()
    };
    program.push_machine_state(
        &mut duplicate_owner,
        State {
            symbol: symbol(52),
            return_type,
            ..Default::default()
        },
    );
    program.push_machine(duplicate_owner);
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );
}

#[test]
fn derived_immutable_alias_fallthrough_remains_non_executable() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let value_symbol = symbol(3);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let mut call = call_with_arguments(arguments);
    let carrier_type = carrier_type(&mut program);
    let mut request =
        push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
    request.kind = QuotientOperationKind::Define;
    call.quotient_operation = Some(request);
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let result_symbol = symbol(4);
    let result = named_argument(&mut program, "result", result_symbol);
    let mut state = State {
        symbol: symbol(5),
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: value_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            ..Default::default()
        },
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: result_symbol,
            name: Identifier::generated_static("result"),
            type_reference: quotient_type,
            initial_value: request,
            is_mutable: false,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(result),
    );
    let mut machine = Machine {
        symbol: symbol(6),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    crate::proof_contracts::quotients::formation_collection::reject_quotient_operation_requests(
        &program,
        &program,
        &mut diagnostics,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("compiler-derived immutable-alias fallthrough relations")
    );
    assert!(
            diagnostics[0]
                .message
                .contains("complete transition-free single-state normal-result coverage through 1 exact immutable result alias")
        );
    assert!(
        diagnostics[0]
            .message
            .contains("executable quotient operations are not admitted")
    );
}

#[test]
fn nonterminal_expression_request_cannot_claim_direct_result_flow() {
    let mut program = TypedTrees::default();
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let terminal = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let mut state = State::default();
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(terminal),
    );
    let mut machine = Machine::default();
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    crate::proof_contracts::quotients::formation_collection::reject_quotient_operation_requests(
        &program,
        &program,
        &mut diagnostics,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("retains its exact representative operation and selected theorem")
    );
    assert!(
        !diagnostics[0]
            .message
            .contains("compiler-derived direct-terminal relations")
    );
    assert!(
        !diagnostics[0]
            .message
            .contains("unchanged state-fallthrough result root")
    );
}

#[test]
fn failed_builtin_lift_implication_names_exact_fact_coordinates_and_transport_form() {
    let public = [
        RepresentativeContractFactLocation {
            owner: RepresentativeContractOwner::Machine,
            contract_position: 2,
            fact_position: 1,
        },
        RepresentativeContractFactLocation {
            owner: RepresentativeContractOwner::State,
            contract_position: 4,
            fact_position: 3,
        },
    ];
    let representative = RepresentativeContractFactLocation {
        owner: RepresentativeContractOwner::State,
        contract_position: 6,
        fact_position: 5,
    };

    assert_eq!(
        render_implication_coordinate_diagnostic(
            "left representative application",
            &public,
            representative,
        ),
        "built-in public Q => representative P implication failed for the left representative application: expected public fact coordinates [public-Q.machine-contract[2].fact[1], public-Q.state-contract[4].fact[3]] to imply representative fact coordinate representative-P.state-contract[6].fact[5]; use `Quotient::lift<F, Congruence, Transport>(...)` and select one exact checked forward-precondition transport theorem",
    );

    // The full selected-theorem request fixture below has only dependent P.
    // Fixed-call fixtures deliberately begin after relation/runtime
    // correspondence, so they cannot legitimately drive diagnostic
    // reconstruction. Pin the remaining exact fixed-call rendering here.
    assert_eq!(
        render_implication_coordinate_diagnostic(
            "runtime representative application",
            &public,
            representative,
        ),
        "built-in public Q => representative P implication failed for the runtime representative application: expected public fact coordinates [public-Q.machine-contract[2].fact[1], public-Q.state-contract[4].fact[3]] to imply representative fact coordinate representative-P.state-contract[6].fact[5]; use `Quotient::lift<F, Congruence, Transport>(...)` and select one exact checked forward-precondition transport theorem",
    );
}

#[test]
fn rejected_two_argument_lift_reports_reconstructed_q_and_p_coordinates() {
    let (mut program, representative, theorem, _) =
        selected_theorem_schema_fixture(TheoremSchemaMutation::Exact);

    let theorem_index = program
        .machines()
        .iter()
        .position(|machine| machine.symbol == theorem.machine_symbol)
        .expect("selected theorem machine");
    let mut theorem_machine = program.machines()[theorem_index].clone();
    program.push_machine_type_parameter(
        &mut theorem_machine,
        TypeParameter {
            symbol: symbol(848),
            name: Identifier::generated_static("TheoremCarrier"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        },
    );
    program.machines_mut()[theorem_index] = theorem_machine;

    let mut representative_machine = Machine {
        symbol: representative.machine_symbol,
        name: Identifier::generated_static("representative"),
        contracts: representative.machine_contracts,
        ..Default::default()
    };
    program.push_machine_type_parameter(
        &mut representative_machine,
        TypeParameter {
            symbol: symbol(849),
            name: Identifier::generated_static("RepresentativeCarrier"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        },
    );
    let mut representative_state = State {
        symbol: representative.state_symbol,
        name: Identifier::generated_static("apply"),
        return_type: representative.return_type,
        contracts: representative.state_contracts,
        ..Default::default()
    };
    for parameter in &representative.parameters {
        program.push_state_parameter(
            &mut representative_state,
            StateParameter {
                symbol: parameter.symbol,
                name: Identifier::generated("representative_parameter"),
                type_reference: parameter.type_reference,
                ..Default::default()
            },
        );
    }
    program.push_machine_state(&mut representative_machine, representative_state);
    program.push_machine(representative_machine);

    let quotient = quotient_type(
        &mut program,
        symbol(900),
        "ExactQuotient",
        symbol(850),
        "ExactRelation",
    );
    let public_value_symbol = symbol(901);
    let shared_symbol = symbol(902);
    let public_value = named_argument(&mut program, "value", public_value_symbol);
    let public_value_again = named_argument(&mut program, "value", public_value_symbol);
    let machine_q = binary_expression(
        &mut program,
        public_value,
        BinaryOperator::Equal,
        public_value_again,
    );
    let public_value = named_argument(&mut program, "value", public_value_symbol);
    let public_value_again = named_argument(&mut program, "value", public_value_symbol);
    let state_q = binary_expression(
        &mut program,
        public_value,
        BinaryOperator::Equal,
        public_value_again,
    );
    let machine_q = program
        .proof_facts
        .insert_many([ProofFact::Expression(machine_q)]);
    let state_q = program
        .proof_facts
        .insert_many([ProofFact::Expression(state_q)]);

    let mut public_machine = Machine {
        symbol: symbol(903),
        name: Identifier::generated_static("quotient_operation"),
        ..Default::default()
    };
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            ..Default::default()
        },
    );
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: machine_q,
            ..Default::default()
        },
    );
    let mut public_state = State {
        symbol: symbol(904),
        name: Identifier::generated_static("entry"),
        return_type: quotient,
        ..Default::default()
    };
    program.push_state_contract(
        &mut public_state,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: state_q,
            ..Default::default()
        },
    );
    for (parameter_symbol, name, type_reference) in [
        (public_value_symbol, "value", quotient),
        (
            shared_symbol,
            "shared",
            representative.parameters[1].type_reference,
        ),
    ] {
        program.push_state_parameter(
            &mut public_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }

    let value = named_argument(&mut program, "value", public_value_symbol);
    let shared = named_argument(&mut program, "shared", shared_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([value, shared]);
    let mut call = call_with_arguments(arguments);
    let mut selected_carrier = static_argument("Carrier");
    selected_carrier.symbol = symbol(500);
    let mut request = request_with_representative(representative.state_symbol);
    request.representative_operation.application = Some(Box::new(StaticSymbolApplication {
        lifetime_arguments: Box::default(),
        arguments: vec![selected_carrier.clone()].into_boxed_slice(),
    }));
    let mut selected_theorem = static_argument("congruence");
    selected_theorem.symbol = theorem.state_symbol;
    selected_theorem.application = Some(Box::new(StaticSymbolApplication {
        lifetime_arguments: Box::default(),
        arguments: vec![selected_carrier].into_boxed_slice(),
    }));
    request.theorem_evidence[0].application = selected_theorem;
    call.quotient_operation = Some(request);
    let right_context = render_failed_builtin_implication(
        &program,
        &public_machine,
        &public_state,
        &call,
        call.quotient_operation
            .as_ref()
            .expect("quotient request retained on call"),
        RelationPlanError::DirectLiftRightPreconditionNotImplied(0),
    )
    .expect("the right failure coordinate must replay the exact dependent partitions");
    assert!(
        right_context.contains("right representative application"),
        "{right_context}"
    );
    assert!(
        right_context.contains("public-Q.machine-contract[1].fact[0]"),
        "{right_context}"
    );
    assert!(right_context.contains("public-Q.state-contract[0].fact[0]"));
    assert!(
        right_context.contains("representative-P.machine-contract[0].fact[0]"),
        "{right_context}"
    );
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    program.statement_table.push_statement(
        &mut public_state.statement_nodes,
        StatementNode::Expression(request),
    );
    program.push_machine_state(&mut public_machine, public_state);
    program.push_machine(public_machine);

    let mut diagnostics = Vec::new();
    crate::proof_contracts::quotients::formation_collection::reject_quotient_operation_requests(
        &program,
        &program,
        &mut diagnostics,
    );

    assert_eq!(diagnostics.len(), 1);
    let message = &diagnostics[0].message;
    assert!(
        message.contains("public-Q.machine-contract[1].fact[0]"),
        "{message}"
    );
    assert!(message.contains("public-Q.state-contract[0].fact[0]"));
    assert!(message.contains("representative-P.machine-contract[0].fact[0]"));
    assert!(message.contains("`Quotient::lift<F, Congruence, Transport>(...)`"));
}

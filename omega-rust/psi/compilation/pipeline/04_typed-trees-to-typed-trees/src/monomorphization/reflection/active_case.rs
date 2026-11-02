//! Sum dispatch is ordinary guarded control; payload projections live only in
//! the corresponding state, whose case precondition checking must establish.
use super::*;
use typed_trees::expression::{BinaryOperator, StaticMachineArgument, TableBinaryExpression};
use typed_trees::statement::{TableTransition, TransitionGuardNode, TransitionTargetNode};
use typed_trees::{
    domain::ProofFact,
    machine::Machine,
    signature::{SignatureContract, SignatureContractKind, StateParameter, StateSignature},
    state::State,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    program: &mut TypedTrees,
    owner: &Machine,
    source_state: SymbolHandle,
    declaration: SymbolHandle,
    subject: TypeReferenceHandle,
    context: TypeReferenceHandle,
    subject_data: &DataDefinition,
    graph: &validation::reflection::SemanticSchemaGraph,
    field_info: &DataDefinition,
    info_type: TypeReferenceHandle,
    owner_identity: &str,
    owner_application: &str,
    access: ReferenceAccess,
    captured_subject: SymbolHandle,
    captured_context: SymbolHandle,
    field_callback: &StaticMachineArgument,
    case_callback: &StaticMachineArgument,
    case_signature: &StateSignature,
    query: typed_trees::statement::StatementHandle,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
    replacement: &mut Vec<StatementNode>,
) -> Result<(), Vec<Diagnostic>> {
    let parameter = program
        .state_signature_parameters(case_signature)
        .get(1)
        .ok_or_else(|| error("active visitation lost its exact case description"))?;
    let case_type = program
        .type_reference_table
        .dereferenced_type_reference(parameter.type_reference);
    let case_info = program
        .closed_data_definition(case_type)
        .filter(|data| {
            data.name.as_str() == "CaseInfo" && is_reflection_declaration(program, data.symbol)
        })
        .cloned()
        .ok_or_else(|| error("active visitation requires its exact CaseInfo vocabulary"))?;
    let case_info_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: case_info.symbol,
            name: case_info.name.clone(),
        });
    let helper_name = format!(
        "__reflect_active#{}#{}",
        source_state.arena_index(),
        query.arena_index()
    );
    let helper_symbol =
        program
            .symbols
            .insert_generated_root_from(source_state, SymbolKind::Machine, &helper_name);
    let variants = program
        .data_members(subject_data)
        .iter()
        .filter_map(|member| member.as_variant().cloned())
        .collect::<Vec<_>>();
    if variants.is_empty() {
        return Err(error("active visitation requires a sum subject"));
    }
    let state_names = std::iter::once("entry".to_owned())
        .chain(
            variants
                .iter()
                .enumerate()
                .map(|(index, _)| format!("case#{index}")),
        )
        .collect::<Vec<_>>();
    let symbols = program.symbols.insert_generated_children(
        helper_symbol,
        state_names
            .iter()
            .map(|name| (SymbolKind::State, name.as_str())),
    );
    let mut helper = Machine {
        symbol: helper_symbol,
        name: Identifier::generated(&helper_name),
        service_reach_row: owner.service_reach_row,
        service_reach_is_installation_bound: owner.service_reach_is_installation_bound,
        target: owner.target.clone(),
        authored_target: owner.authored_target.clone(),
        body_is_present: true,
        body_status: owner.body_status,
        ..Default::default()
    };
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let subject_type = reference(program, subject, access);
    let context_type = reference(program, context, ReferenceAccess::Mutable);
    let mut entry = State {
        symbol: symbols.handle_at(0),
        name: Identifier::generated("entry"),
        return_type: unit,
        ..Default::default()
    };
    let (entry_subject, entry_context) =
        parameters(program, &mut entry, subject_type, context_type);
    let mut dispatch = Vec::new();
    // A retained local marker keeps original query custody even for nullary
    // cases. It is descriptive origin; it grants no case or borrow evidence.
    // Custody is a descriptive marker, not another exclusive reference. A
    // scalar marker leaves the actual context as the sole state-edge loan.
    let marker_type = program
        .data_members(&case_info)
        .iter()
        .find_map(|member| {
            member
                .as_field()
                .filter(|field| field.name.as_str() == "stable_number")
                .map(|field| field.type_reference)
        })
        .ok_or_else(|| error("active visitation lost its marker type"))?;
    let marker_value = integer(program, 0)?;
    local(
        program,
        entry.symbol,
        declaration,
        "query_origin",
        marker_type,
        marker_value,
        &mut dispatch,
    )?;
    if let Some(StatementNode::LocalData(local)) = dispatch.last_mut() {
        local.reflected_query_origin = Some(query);
    }
    let mut cases = Vec::new();
    for (index, variant) in variants.iter().enumerate() {
        let symbol = symbols.handle_at(index as u32 + 1);
        let mut state = State {
            symbol,
            name: Identifier::generated(&state_names[index + 1]),
            return_type: unit,
            ..Default::default()
        };
        let (value, context) = parameters(program, &mut state, subject_type, context_type);
        let guard = case_guard(program, value, variant.symbol);
        let mut facts = HandleSpan::empty();
        program
            .proof_facts
            .append_to_span(&mut facts, ProofFact::Expression(guard));
        program.push_state_contract(
            &mut state,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts,
                ..Default::default()
            },
        );
        let description = graph
            .cases()
            .find(|(_, case)| case.name == variant.name.as_str())
            .map(|(_, case)| case.clone())
            .ok_or_else(|| error("active visitation lost its selected case description"))?;
        let mut body = Vec::new();
        let mut fields = Vec::new();
        for (key, text) in [
            ("member_identity", description.member_identity.as_str()),
            ("owner_identity", owner_identity),
            ("owner_application", owner_application),
            ("name", description.name.as_str()),
        ] {
            let value =
                expressions_of_mut(program).insert(ExpressionNode::String(text.as_bytes().into()));
            fields.push(info_field(program, &case_info, key, value)?);
        }
        for (key, number) in [
            (
                "declaration_position",
                description.declaration_position as u64,
            ),
            ("stable_number", description.stable_number.unwrap_or(0)),
        ] {
            let value = integer(program, number)?;
            fields.push(info_field(program, &case_info, key, value)?);
        }
        let has_number = expressions_of_mut(program)
            .insert(ExpressionNode::Boolean(description.stable_number.is_some()));
        fields.push(info_field(
            program,
            &case_info,
            "has_stable_number",
            has_number,
        )?);
        let fields = expressions_of_mut(program).insert_struct_fields(fields);
        let info =
            expressions_of_mut(program).insert(ExpressionNode::StructLiteral(TableStructLiteral {
                type_name: case_info.name.clone(),
                type_symbol: case_info.symbol,
                fields,
                ..Default::default()
            }));
        let info = local(
            program,
            state.symbol,
            declaration,
            "case_info",
            case_info_type,
            info,
            &mut body,
        )?;
        let context_value = name(program, context);
        let context_value = borrow(program, context_value, ReferenceAccess::Mutable);
        let info_value = name(program, info);
        let info_value = borrow(program, info_value, ReferenceAccess::Shared);
        let arguments = program
            .statement_table
            .insert_expression_handles(vec![context_value, info_value]);
        body.push(StatementNode::Call(TableCall {
            target_symbol: case_callback.symbol,
            target: case_callback
                .path
                .last()
                .cloned()
                .ok_or_else(|| error("active case callback lost its exact operation"))?,
            machine_arguments: case_callback
                .application
                .as_ref()
                .map_or_else(Vec::new, |application| application.arguments.to_vec())
                .into_boxed_slice(),
            arguments,
            operational_acknowledgement: acknowledgement,
            reflected_field_origin: Some(typed_trees::statement::ReflectedFieldCallOrigin {
                query,
                callback: case_callback.clone(),
            }),
            ..Default::default()
        }));
        let mut members = program
            .data_members(subject_data)
            .iter()
            .enumerate()
            .filter_map(|(position, member)| {
                let DataMember::Field(field) = member else {
                    return None;
                };
                Some(
                    graph
                        .fields()
                        .find(|(_, description)| {
                            description.declaration_position as usize == position
                        })
                        .map(|(_, description)| {
                            (position, field.clone(), description.clone(), None, "")
                        }),
                )
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| error("active visitation lost its common member"))?;
        let payload = program.data_payload_fields(variant).to_vec();
        for (position, field) in payload.into_iter().enumerate() {
            let description_field = description
                .payload_fields
                .iter()
                .find_map(|handle| {
                    let validation::reflection::SchemaNode::Field(field) = graph.node(*handle)
                    else {
                        return None;
                    };
                    (field.declaration_position as usize == position).then_some(field.clone())
                })
                .ok_or_else(|| error("active visitation lost its exact payload member"))?;
            members.push((
                position,
                field,
                description_field,
                Some(variant.name.clone()),
                description.member_identity.as_str(),
            ));
        }
        emit_fields(
            program,
            members,
            state.symbol,
            declaration,
            field_info,
            info_type,
            owner_identity,
            owner_application,
            false,
            access,
            Some(value),
            context,
            field_callback,
            query,
            acknowledgement,
            &mut body,
        )?;
        state.statement_nodes = program.statement_table.insert_statements(body);
        cases.push(state);
        let condition = case_guard(program, entry_subject, variant.symbol);
        let value = name(program, entry_subject);
        let value = borrow(program, value, access);
        let context = name(program, entry_context);
        let context = borrow(program, context, ReferenceAccess::Mutable);
        let arguments = program
            .statement_table
            .insert_expression_handles(vec![value, context]);
        let mut path = HandleSpan::empty();
        program
            .statement_table
            .push_name_path_member(&mut path, Identifier::generated(&state_names[index + 1]));
        let target =
            program
                .statement_table
                .insert_transition_target(TransitionTargetNode::Named {
                    operational_acknowledgement: acknowledgement,
                    path: typed_trees::statement::TableNamePath {
                        members: path,
                        head_symbol: symbol,
                        symbol,
                        ..Default::default()
                    },
                    static_machine_parameter: SymbolHandle::invalid(),
                    arguments,
                    evidence_arguments: Box::default(),
                    source_span: Default::default(),
                    authored_call_selection: None,
                });
        dispatch.push(StatementNode::Transition(TableTransition {
            guard: TransitionGuardNode::When(condition),
            target,
            ..Default::default()
        }));
    }
    entry.statement_nodes = program.statement_table.insert_statements(dispatch);
    // Entry must be first in the roster; use the ordinary machine insertion API.
    program.push_machine_state(&mut helper, entry.clone());
    for case in cases {
        program.push_machine_state(&mut helper, case);
    }
    program.push_machine(helper);
    let value = name(program, captured_subject);
    let value = borrow(program, value, access);
    let context = name(program, captured_context);
    let context = borrow(program, context, ReferenceAccess::Mutable);
    let arguments = program
        .statement_table
        .insert_expression_handles(vec![value, context]);
    replacement.push(StatementNode::Call(TableCall {
        target_symbol: entry.symbol,
        target: Identifier::generated("entry"),
        arguments,
        operational_acknowledgement: acknowledgement,
        reflected_field_origin: Some(typed_trees::statement::ReflectedFieldCallOrigin {
            query,
            callback: StaticMachineArgument {
                symbol: entry.symbol,
                path: vec![
                    Identifier::generated(&helper_name),
                    Identifier::generated("entry"),
                ]
                .into_boxed_slice(),
                ..Default::default()
            },
        }),
        ..Default::default()
    }));
    Ok(())
}
fn parameters(
    program: &mut TypedTrees,
    state: &mut State,
    subject_type: TypeReferenceHandle,
    context_type: TypeReferenceHandle,
) -> (SymbolHandle, SymbolHandle) {
    let symbols = program.symbols.insert_generated_children(
        state.symbol,
        [
            (SymbolKind::Parameter, "value"),
            (SymbolKind::Parameter, "context"),
        ],
    );
    for (symbol, parameter_type, label) in [
        (symbols.handle_at(0), subject_type, "value"),
        (symbols.handle_at(1), context_type, "context"),
    ] {
        program.push_state_parameter(
            state,
            StateParameter {
                symbol,
                name: Identifier::generated(label),
                type_reference: parameter_type,
                ..Default::default()
            },
        );
    }
    (symbols.handle_at(0), symbols.handle_at(1))
}
fn case_guard(
    program: &mut TypedTrees,
    subject: SymbolHandle,
    variant: SymbolHandle,
) -> ExpressionHandle {
    let left = name(program, subject);
    let right = name(program, variant);
    expressions_of_mut(program).insert(ExpressionNode::Binary(TableBinaryExpression {
        left,
        operator: BinaryOperator::CaseMembership,
        right,
    }))
}

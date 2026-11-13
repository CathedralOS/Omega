//! Construct the actual invocation as ordinary typed source. Only the
//! existing lowered checker can establish its argument domains and callee
//! requirements; this constructor supplies no acceptance or source authority.

use crate::BuildTimeValue;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::DataMemberIdentity;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableStructLiteral,
    TableStructLiteralField,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

fn entry_state<'a>(program: &'a TypedTrees, machine: &Machine) -> Option<&'a State> {
    match program.machine_states(machine) {
        [state] => Some(state),
        _ => None,
    }
}

// Authored inputs may name the declared member; returned values retain its
// stable schema identity. Both select only within the actual formal roster.
fn selects_member(spelling: &str, member: &DataMemberIdentity) -> bool {
    let supplied = DataMemberIdentity::of_spelling(spelling);
    if supplied.ordinal.is_some() {
        supplied == *member
    } else {
        supplied.name == member.name
    }
}

fn literal(
    program: &mut TypedTrees,
    destination: TypeReferenceHandle,
    value: &mut BuildTimeValue,
) -> Result<ExpressionHandle, String> {
    use numerics::literals::{IntegerLiteral, IntegerRadix};
    use typed_trees::data::DataMember;
    use typed_trees::types::PrimitiveType;
    if !program
        .type_reference_table
        .contains_type_reference(destination)
    {
        return Err("invocation value has no retained formal type".into());
    }
    let node = match program
        .type_reference_table
        .type_reference(destination)
        .clone()
    {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return literal(program, base_type, value);
        }
        TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared,
            ..
        } if matches!(
            program.type_reference_table.type_reference(referee),
            TypeReferenceNode::Slice { .. }
        ) =>
        {
            let BuildTimeValue::Text(bytes) = value else {
                return Err("a byte view requires actual immutable bytes".into());
            };
            ExpressionNode::String(bytes.clone().into())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err("expected an array value".into());
            };
            if elements.len() != length {
                return Err("array argument differs from its declared extent".into());
            }
            let elements = elements
                .iter_mut()
                .map(|value| literal(program, element_type, value))
                .collect::<Result<Vec<_>, _>>()?;
            ExpressionNode::ArrayLiteral(
                program.expression_table.insert_expression_handles(elements),
            )
        }
        TypeReferenceNode::Named { symbol, .. }
            if program.symbols.builtin_type_atom(symbol).is_some() =>
        {
            let primitive = program
                .symbols
                .builtin_type_atom(symbol)
                .and_then(|atom| match atom {
                    symbols::BuiltinTypeAtom::Bool => Some(PrimitiveType::Bool),
                    atom => PrimitiveType::from_builtin_atom(atom),
                })
                .ok_or("invocation primitive has no supported value representation")?;
            match (primitive, value) {
                (PrimitiveType::Bool, BuildTimeValue::Bool(value)) => {
                    ExpressionNode::Boolean(*value)
                }
                (primitive, BuildTimeValue::Int(value)) if primitive.accepts_integer_literal() => {
                    let digits = if primitive.is_signed_integer() {
                        value.unsigned_abs().to_string()
                    } else {
                        (*value as u64).to_string()
                    };
                    ExpressionNode::Integer(
                        IntegerLiteral::from_parts(
                            primitive.is_signed_integer() && *value < 0,
                            IntegerRadix::Decimal,
                            &digits,
                        )
                        .map_err(str::to_owned)?,
                    )
                }
                _ => return Err("invocation value differs from the exact primitive formal".into()),
            }
        }
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            let definition = program
                .closed_data_definition(destination)
                .cloned()
                .ok_or("invocation value has no exact closed data application")?;
            let (case_name, case_symbol, declared, supplied) = match value {
                BuildTimeValue::Struct { type_name, fields } => {
                    if type_name != definition.name.as_str()
                        && *type_name != program.normalized_type_identity_text(destination)
                    {
                        return Err("invocation record names another declaration".into());
                    }
                    (
                        None,
                        None,
                        program
                            .data_members(&definition)
                            .iter()
                            .filter_map(DataMember::as_field)
                            .cloned()
                            .collect::<Vec<_>>(),
                        fields,
                    )
                }
                BuildTimeValue::Case { variant, payload } => {
                    let mut cases = program
                        .data_members(&definition)
                        .iter()
                        .filter_map(DataMember::as_variant)
                        .filter(|case| selects_member(variant, &case.member_identity()));
                    let selected = cases
                        .next()
                        .cloned()
                        .ok_or("invocation case is not declared by its formal")?;
                    if cases.next().is_some() {
                        return Err("invocation case is ambiguous".into());
                    }
                    (
                        Some(selected.name.clone()),
                        Some(selected.symbol),
                        program.data_payload_fields(&selected).to_vec(),
                        payload,
                    )
                }
                _ => return Err("invocation data requires a complete record or case value".into()),
            };
            if declared.len() != supplied.len() {
                return Err("invocation value differs from its complete field roster".into());
            }
            let mut fields = Vec::with_capacity(declared.len());
            for field in declared {
                if field.relevance.is_erased() {
                    return Err("invocation values cannot manufacture erased evidence".into());
                }
                let mut matching = supplied
                    .iter_mut()
                    .filter(|(name, _)| selects_member(name, &field.member_identity()));
                let (_, value) = matching
                    .next()
                    .ok_or("invocation value is missing a declared field")?;
                if matching.next().is_some() {
                    return Err("invocation value repeats a declared field".into());
                }
                fields.push(TableStructLiteralField {
                    name: field.name,
                    field_symbol: field.symbol,
                    value: literal(program, field.type_reference, value)?,
                });
            }
            ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: destination,
                type_name: definition.name,
                type_symbol: definition.symbol,
                case_name,
                case_symbol,
                fields: program.expression_table.insert_struct_fields(fields),
            })
        }
        _ => return Err("invocation argument has no supported closed source construction".into()),
    };
    Ok(program.expression_table.insert(node))
}
pub(crate) fn direct_call(
    program: &TypedTrees,
    machine: &Machine,
) -> Result<TableCallExpression, String> {
    let entry =
        entry_state(program, machine).ok_or("constant invocation has no exact entry state")?;
    Ok(TableCallExpression {
        // This call names its entry directly; it has no runtime callee value.
        runtime_call: None,
        authored_call_selection: None,
        authored_call_binding: None,
        authored_machine_application: Default::default(),
        requirement_application: Default::default(),
        exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
        receiver: ExpressionHandle::invalid(),
        target_symbol: entry.symbol,
        static_machine_parameter: SymbolHandle::invalid(),
        target: entry.name.clone(),
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        installed_interrupt_operation: None,
        layout_query: None,
        native_code_layout: None,
        build_selection: Default::default(),
        arguments: Default::default(),
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement::default(),
    })
}

pub(crate) fn prepare_value_invocation(
    program: &TypedTrees,
    requester: SymbolHandle,
    machine: &Machine,
    request: &TableCallExpression,
    arguments: &[crate::BuildTimeValue],
    request_expression: ExpressionHandle,
) -> Result<(TypedTrees, SymbolHandle, ExpressionHandle), String> {
    if request.authored_machine_application.is_valid()
        && !program
            .expression_table
            .static_machine_application_is_valid(request.authored_machine_application)
    {
        return Err("source invocation has no retained static application".into());
    }
    let source_span = if request_expression.is_valid() {
        if !program
            .expression_table
            .expression_is_valid(request_expression)
            || program.expression_table.call_node(request_expression) != Some(request)
        {
            return Err(
                "source invocation call differs from its retained source occurrence".into(),
            );
        }
        program.expression_table.try_source_span(request_expression)
    } else {
        None
    };
    let entry =
        entry_state(program, machine).ok_or("constant invocation has no exact entry state")?;
    let parameters = program.state_parameters(entry);
    let declaration_argument =
        typed_trees::reflection::construction::declaration_query_argument_role(
            program,
            typed_trees::references::StaticArgumentCall::Expression(
                ExpressionHandle::invalid(),
                request,
            ),
        ) == typed_trees::references::StaticArgumentRole::DeclarationDescription;
    if declaration_argument {
        if !arguments.is_empty()
            || program
                .expression_table
                .expression_handles(request.arguments)
                .len()
                != parameters.len()
        {
            return Err(
                "a declaration query retains its authored operand, not a runtime argument".into(),
            );
        }
    } else if parameters.len() != arguments.len() {
        return Err("constant invocation arguments differ from its checked entry arity".into());
    }
    let static_call =
        typed_trees::references::StaticArgumentCall::Expression(request_expression, request);
    let mut supplied = static_call.arguments(program).to_vec();
    if !request.machine_arguments.is_empty()
        && request.machine_arguments.as_ref() != supplied.as_slice()
    {
        return Err(
            "source invocation static arguments differ from their retained application".into(),
        );
    }
    let type_parameters = program.machine_type_parameters(machine);
    if !machine.lifetime_parameters.is_empty() {
        return Err("source invocation lifetime arguments are not materialized".into());
    }
    if type_parameters
        .iter()
        .any(|parameter| !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type))
    {
        return Err("source invocation non-type static arguments are not materialized".into());
    }
    if type_parameters.len() != supplied.len() {
        return Err("source invocation lost its complete static argument list".into());
    }
    let mut probe = program.clone();
    let owner = static_call.lexical_owner(program);
    if !supplied.is_empty() && !program.symbols.symbols().nodes().is_valid(owner) {
        return Err("source invocation static arguments lost their supplying declaration".into());
    }
    let mut substitutions = Vec::with_capacity(type_parameters.len());
    let mut materialized = false;
    for (parameter, argument) in type_parameters.iter().zip(&mut supplied) {
        if argument.type_reference.is_valid() {
            // A stale supplied reference is malformed, not a missing operand
            // that can be rebuilt from its remaining symbol or spelling.
            if !program
                .type_reference_table
                .contains_type_reference(argument.type_reference)
            {
                return Err("source invocation type argument has no retained reference".into());
            }
        } else {
            // A bare type retains its declaration and full static application.
            // Form its reference through that same owner before substitution;
            // applied type expressions already have their retained reference.
            argument.type_reference = probe
                .static_operand_type_reference_in_scope(argument, owner)
                .map_err(|reason| format!("source invocation type argument: {reason}"))?;
            materialized = true;
        }
        substitutions.push((parameter.symbol, argument.type_reference));
    }
    if !substitutions.is_empty() {
        let references = substitutions
            .iter()
            .map(|(_, reference)| *reference)
            .collect::<Vec<_>>();
        probe
            .require_owned_application_operands(&references, owner)
            .map_err(|reason| format!("source invocation type argument ownership: {reason}"))?;
    }
    if materialized {
        let application = static_call.application();
        if application.is_valid() {
            let retained = probe
                .expression_table
                .static_machine_application_arguments_mut(application);
            if retained.len() != supplied.len() {
                return Err("source invocation static application lost its argument span".into());
            }
            retained.clone_from_slice(&supplied);
        }
        if request_expression.is_valid() && !request.machine_arguments.is_empty() {
            let ExpressionNode::Call(original) =
                probe.expression_table.expression_mut(request_expression)
            else {
                return Err(
                    "source invocation lost its original call during operand preparation".into(),
                );
            };
            original.machine_arguments = supplied.clone().into_boxed_slice();
        }
    }
    let result_type = probe
        .type_reference_table
        .instantiate(entry.return_type, &substitutions)?;
    let (probe_machine, mut state) =
        invocation_probe_declarations(&mut probe, requester, "@const-invocation", result_type);
    let mut actuals = Vec::with_capacity(arguments.len());
    for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if matches!(argument, crate::BuildTimeValue::LayoutTarget(_)) {
            return Err("source invocation requires an actually materialized layout target".into());
        }
        let destination = probe
            .type_reference_table
            .instantiate(parameter.type_reference, &substitutions)?;
        let mut carrier = destination;
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            probe.type_reference_table.type_reference(carrier)
        {
            carrier = *base_type;
        }
        let reference = match probe.type_reference_table.type_reference(carrier) {
            typed_trees::types::TypeReferenceNode::Reference {
                referee, access, ..
            } => Some((*referee, *access)),
            _ => None,
        };
        let byte_view = matches!(reference, Some((referee, language_core::ReferenceAccess::Shared))
                    if matches!(probe.type_reference_table.type_reference(referee), TypeReferenceNode::Slice { .. }));
        let mut snapshot = argument.clone();
        let value = literal(
            &mut probe,
            if byte_view {
                destination
            } else {
                reference.map_or(destination, |(referee, _)| referee)
            },
            &mut snapshot,
        )
        .map_err(|reason| format!("constant argument `{}`: {reason}", parameter.name))?;
        let actual = if byte_view {
            value
        } else if let Some((referee, access)) = reference {
            let name = format!("@argument-{position}");
            let symbol = probe
                .symbols
                .insert_generated_children(state.symbol, [(SymbolKind::Local, name.as_str())])
                .start();
            let name = typed_trees::name::Identifier::generated(&name);
            probe.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(typed_trees::statement::TableLocalData {
                    symbol,
                    name: name.clone(),
                    type_reference: referee,
                    initial_value: value,
                    is_mutable: access.is_exclusive(),
                    ..Default::default()
                }),
            );
            let path = probe_name_path(&mut probe, symbol, name);
            let target = probe.expression_table.insert(ExpressionNode::Name(path));
            probe.expression_table.insert(ExpressionNode::Borrow(
                typed_trees::expression::TableBorrowExpression { target, access },
            ))
        } else {
            value
        };
        actuals.push(actual);
    }
    let mut call = request.clone();
    if materialized && !call.machine_arguments.is_empty() {
        call.machine_arguments = supplied.into_boxed_slice();
    }
    if !declaration_argument {
        call.arguments = probe.expression_table.insert_expression_handles(actuals);
    }
    let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
    let call_span = source_span
        .or_else(|| probe.symbols.symbol_provenance_source_span(requester))
        .ok_or("source invocation lost its actual requesting source")?;
    probe.expression_table.set_source_span(concrete, call_span);
    let owner = finish_invocation_probe(&mut probe, probe_machine, state, concrete);
    probe.retain_call_static_argument_context(concrete, owner)?;
    Ok((probe, owner, concrete))
}

fn probe_name_path(
    probe: &mut TypedTrees,
    symbol: SymbolHandle,
    name: typed_trees::name::Identifier,
) -> typed_trees::expression::TableNamePath {
    let mut members = arena::HandleSpan::empty();
    probe
        .expression_table
        .push_name_path_member(&mut members, name);
    let mut member_symbols = arena::HandleSpan::empty();
    probe
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, symbol);
    typed_trees::expression::TableNamePath::new(members, member_symbols, symbol, symbol)
}

fn invocation_probe_declarations(
    probe: &mut TypedTrees,
    owner: SymbolHandle,
    name: &str,
    destination: typed_trees::types::TypeReferenceHandle,
) -> (Machine, State) {
    let symbol = probe
        .symbols
        .insert_generated_root_from(owner, SymbolKind::Machine, name);
    let children = probe
        .symbols
        .insert_generated_children(symbol, [(SymbolKind::State, name)]);
    (
        Machine {
            symbol,
            name: typed_trees::name::Identifier::generated(name),
            ..Default::default()
        },
        State {
            symbol: children.start(),
            name: typed_trees::name::Identifier::generated(name),
            return_type: destination,
            ..Default::default()
        },
    )
}

fn finish_invocation_probe(
    probe: &mut TypedTrees,
    mut machine: Machine,
    mut state: State,
    expression: ExpressionHandle,
) -> SymbolHandle {
    let target = probe
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(expression));
    let source_span = probe.expression_table.source_span(expression);
    probe.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span,
            ..Default::default()
        }),
    );
    let owner = machine.symbol;
    probe.push_machine_state(&mut machine, state);
    probe.push_machine(machine);
    owner
}

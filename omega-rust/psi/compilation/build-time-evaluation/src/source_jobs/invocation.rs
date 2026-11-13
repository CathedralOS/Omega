//! Construct the actual invocation as ordinary typed source. Only the
//! existing lowered checker can establish its argument domains and callee
//! requirements; this constructor supplies no acceptance or source authority.

use crate::BuildTimeValue;
use symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};
use typed_trees::TypedTrees;
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
                        .filter(|case| case.name.as_str() == variant);
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
                    .filter(|(name, _)| name == field.name.as_str());
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
pub(crate) fn prepare_value_invocation(
    program: &TypedTrees,
    requester: SymbolHandle,
    machine: &Machine,
    arguments: &[crate::BuildTimeValue],
    source_span: Option<source::SourceSpan>,
) -> Result<(TypedTrees, SymbolHandle), String> {
    let entry =
        entry_state(program, machine).ok_or("constant invocation has no exact entry state")?;
    let parameters = program.state_parameters(entry);
    if parameters.len() != arguments.len() {
        return Err("constant invocation arguments differ from its checked entry arity".into());
    }
    let mut probe = program.clone();
    let (probe_machine, mut state) = invocation_probe_declarations(
        &mut probe,
        requester,
        "@const-invocation",
        entry.return_type,
    );
    let mut actuals = Vec::with_capacity(arguments.len());
    for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if matches!(argument, crate::BuildTimeValue::LayoutTarget(_)) {
            return Err("source invocation requires an actually materialized layout target".into());
        }
        let mut carrier = parameter.type_reference;
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(carrier)
        {
            carrier = *base_type;
        }
        let reference = match program.type_reference_table.type_reference(carrier) {
            typed_trees::types::TypeReferenceNode::Reference {
                referee, access, ..
            } => Some((*referee, *access)),
            _ => None,
        };
        let byte_view = matches!(reference, Some((referee, language_core::ReferenceAccess::Shared))
                    if matches!(program.type_reference_table.type_reference(referee), TypeReferenceNode::Slice { .. }));
        let mut snapshot = argument.clone();
        let value = literal(
            &mut probe,
            if byte_view {
                parameter.type_reference
            } else {
                reference.map_or(parameter.type_reference, |(referee, _)| referee)
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
    let call = TableCallExpression {
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
        arguments: probe.expression_table.insert_expression_handles(actuals),
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement::default(),
    };
    let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
    let call_span = source_span
        .or_else(|| probe.symbols.symbol_provenance_source_span(requester))
        .ok_or("source invocation lost its actual requesting source")?;
    probe.expression_table.set_source_span(concrete, call_span);
    let owner = finish_invocation_probe(&mut probe, probe_machine, state, concrete);
    probe.retain_call_static_argument_context(concrete, owner)?;
    Ok((probe, owner))
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

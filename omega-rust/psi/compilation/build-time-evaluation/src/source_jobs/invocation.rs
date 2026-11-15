//! Construct the actual invocation as ordinary typed source. Only the
//! existing lowered checker can establish its argument domains and callee
//! requirements; this constructor supplies no acceptance or source authority.

use crate::BuildTimeValue;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataMemberIdentity};
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

struct DataArgument<'value> {
    definition: DataDefinition,
    case_name: Option<typed_trees::name::Identifier>,
    case_symbol: Option<SymbolHandle>,
    declared: Vec<DataField>,
    supplied: &'value [(String, BuildTimeValue)],
}

// Host contents lose source names after lowering. Select their exact nominal
// declaration before that boundary, using the same roster as source literals.
fn data_argument<'value>(
    program: &mut TypedTrees,
    destination: TypeReferenceHandle,
    value: &'value BuildTimeValue,
) -> Result<DataArgument<'value>, String> {
    let definition = program
        .closed_data_definition(destination)
        .cloned()
        .ok_or("invocation value has no exact closed data application")?;
    let (case_name, case_symbol, mut declared, supplied) = match value {
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
                    .collect(),
                fields.as_slice(),
            )
        }
        BuildTimeValue::Case { variant, payload } => {
            let mut cases = program
                .data_members(&definition)
                .iter()
                .filter_map(DataMember::as_variant)
                .filter(|case| selects_member(variant, &case.member_identity()));
            let selected = cases.next().ok_or_else(|| {
                format!(
                    "invocation case `{variant}` is not declared by formal `{}`",
                    program.normalized_type_identity_text(destination),
                )
            })?;
            if cases.next().is_some() {
                return Err("invocation case is ambiguous".into());
            }
            (
                Some(selected.name.clone()),
                Some(selected.symbol),
                program.data_payload_fields(selected).to_vec(),
                payload.as_slice(),
            )
        }
        _ => return Err("invocation data requires a complete record or case value".into()),
    };
    if declared.len() != supplied.len() {
        return Err("invocation value differs from its complete field roster".into());
    }
    // Validation and source construction inspect the same actual member
    // application. Template lifetime names cannot replace its supplying tuple.
    for field in &mut declared {
        field.type_reference =
            program.instantiate_data_member_type(field.type_reference, destination)?;
    }
    Ok(DataArgument {
        definition,
        case_name,
        case_symbol,
        declared,
        supplied,
    })
}

fn field_argument<'value>(
    field: &DataField,
    supplied: &'value [(String, BuildTimeValue)],
) -> Result<&'value BuildTimeValue, String> {
    if field.relevance.is_erased() {
        return Err("invocation values cannot manufacture erased evidence".into());
    }
    let mut matching = supplied
        .iter()
        .filter(|(name, _)| selects_member(name, &field.member_identity()));
    let (_, value) = matching
        .next()
        .ok_or("invocation value is missing a declared field")?;
    if matching.next().is_some() {
        return Err("invocation value repeats a declared field".into());
    }
    Ok(value)
}

fn validate_input_declarations(
    program: &mut TypedTrees,
    destination: TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<(), String> {
    if !program
        .type_reference_table
        .contains_type_reference(destination)
    {
        return Err("invocation value has no retained formal type".into());
    }
    match program
        .type_reference_table
        .type_reference(destination)
        .clone()
    {
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_input_declarations(program, base_type, value)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            if program.is_borrowed_byte_slice(destination) {
                if !matches!(value, BuildTimeValue::Text(_)) {
                    return Err("a byte view requires actual immutable bytes".into());
                }
                Ok(())
            } else {
                validate_input_declarations(program, referee, value)
            }
        }
        TypeReferenceNode::Slice { element_type }
        | TypeReferenceNode::FixedArray { element_type, .. } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err("a typed sequence requires its complete actual elements".into());
            };
            for element in elements {
                validate_input_declarations(program, element_type, element)?;
            }
            Ok(())
        }
        TypeReferenceNode::Named { symbol, .. }
            if program.symbols.builtin_type_atom(symbol).is_some() =>
        {
            Ok(())
        }
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
            let data = data_argument(program, destination, value)?;
            for field in &data.declared {
                validate_input_declarations(
                    program,
                    field.type_reference,
                    field_argument(field, data.supplied)?,
                )?;
            }
            Ok(())
        }
        _ => Err("invocation input has no supported closed declaration".into()),
    }
}

fn materialize_input(
    program: &mut TypedTrees,
    state: &mut State,
    input_values: &mut Vec<BuildTimeValue>,
    destination: TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<ExpressionHandle, String> {
    validate_input_declarations(program, destination, value)?;
    let name = format!("@input-{}", input_values.len());
    let symbol = program
        .symbols
        .append_generated_children_from(
            state.symbol,
            [(state.symbol, SymbolKind::Parameter, name.as_str())],
        )
        .start();
    let name = typed_trees::name::Identifier::generated(&name);
    program.push_state_parameter(
        state,
        typed_trees::signature::StateParameter {
            symbol,
            name: name.clone(),
            type_reference: destination,
            is_const: false,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    input_values.push(value.clone());
    let path = probe_name_path(program, symbol, name);
    Ok(program.expression_table.insert(ExpressionNode::Name(path)))
}

fn materialize_argument(
    program: &mut TypedTrees,
    state: &mut State,
    input_values: &mut Vec<BuildTimeValue>,
    destination: TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<ExpressionHandle, String> {
    use numerics::literals::{IntegerLiteral, IntegerRadix};
    use typed_trees::types::PrimitiveType;
    if !program
        .type_reference_table
        .contains_type_reference(destination)
    {
        return Err("invocation value has no retained formal type".into());
    }
    if matches!(value, BuildTimeValue::LayoutTarget(_)) {
        let reference = match program.type_reference_table.type_reference(destination) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => Some((*referee, *access)),
            _ => None,
        };
        let value_type = reference.map_or(destination, |(referee, _)| referee);
        let symbol = program
            .closed_data_definition(value_type)
            .map_or_else(SymbolHandle::invalid, |data| data.symbol);
        if !program.core_layout_declaration(
            symbol,
            language_core::sealed_intrinsics::SealedIntrinsic::LayoutTargetData,
        ) || program
            .type_reference_table
            .unconstrained_type_reference(value_type)
            != value_type
        {
            return Err(
                "issued layout context requires its exact unqualified core declaration".into(),
            );
        }
        // Opaque contents are supplied at execution, never constructed in
        // source. The same recursive argument walk handles a whole parameter
        // and a context stored in a field, case or array element.
        let name = format!("@context-{}", input_values.len());
        let name_text = typed_trees::name::Identifier::generated(&name);
        let symbol = program
            .symbols
            .append_generated_children_from(
                state.symbol,
                [(state.symbol, SymbolKind::Parameter, name.as_str())],
            )
            .start();
        program.push_state_parameter(
            state,
            typed_trees::signature::StateParameter {
                symbol,
                name: name_text.clone(),
                type_reference: value_type,
                is_const: false,
                is_mutable: reference.is_some_and(|(_, access)| access.is_exclusive()),
                relevance: language_core::BindingRelevance::Relevant,
            },
        );
        let path = probe_name_path(program, symbol, name_text);
        let actual = program.expression_table.insert(ExpressionNode::Name(path));
        input_values.push(value.clone());
        return Ok(if let Some((_, access)) = reference {
            program.expression_table.insert(ExpressionNode::Borrow(
                typed_trees::expression::TableBorrowExpression {
                    target: actual,
                    access,
                },
            ))
        } else {
            actual
        });
    }
    let node = match program
        .type_reference_table
        .type_reference(destination)
        .clone()
    {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return materialize_argument(program, state, input_values, base_type, value);
        }
        TypeReferenceNode::Reference { referee, .. }
            if matches!(
                program.type_reference_table.type_reference(referee),
                TypeReferenceNode::Slice { .. }
            ) =>
        {
            // A nested view has the same real host backing as a whole input.
            // A literal here would instead lend this generated caller's local
            // storage and lose the declared enclosing lifetime application.
            return materialize_input(program, state, input_values, destination, value);
        }
        TypeReferenceNode::Slice { element_type } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err("a typed sequence requires its complete actual elements".into());
            };
            let elements = elements
                .iter()
                .map(|value| {
                    materialize_argument(program, state, input_values, element_type, value)
                })
                .collect::<Result<Vec<_>, _>>()?;
            ExpressionNode::ArrayLiteral(
                program.expression_table.insert_expression_handles(elements),
            )
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
                .iter()
                .map(|value| {
                    materialize_argument(program, state, input_values, element_type, value)
                })
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
            let data = data_argument(program, destination, value)?;
            let mut fields = Vec::with_capacity(data.declared.len());
            for field in data.declared {
                let value = field_argument(&field, data.supplied)?;
                fields.push(TableStructLiteralField {
                    name: field.name,
                    field_symbol: field.symbol,
                    value: materialize_argument(
                        program,
                        state,
                        input_values,
                        field.type_reference,
                        value,
                    )?,
                });
            }
            ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: destination,
                type_name: data.definition.name,
                type_symbol: data.definition.symbol,
                case_name: data.case_name,
                case_symbol: data.case_symbol,
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
    Ok(declaration_call(entry.symbol, entry.name.clone()))
}

pub(crate) fn declaration_call(
    target: SymbolHandle,
    name: typed_trees::name::Identifier,
) -> TableCallExpression {
    TableCallExpression {
        authored_call_selection: None,
        authored_call_binding: None,
        authored_machine_application: Default::default(),
        requirement_application: Default::default(),
        exact_coercion_target: typed_trees::types::TypeReferenceHandle::invalid(),
        receiver: ExpressionHandle::invalid(),
        target_symbol: target,
        static_machine_parameter: SymbolHandle::invalid(),
        target: name,
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
    }
}

pub(crate) struct PreparedSourceInvocation {
    pub source: TypedTrees,
    pub caller: SymbolHandle,
    pub call: ExpressionHandle,
    /// Actual backing and issued opaque inputs of the checked probe. Constructed
    /// source values retain their ordinary literal premises.
    pub input_values: Vec<BuildTimeValue>,
}

pub(crate) fn prepare_value_invocation(
    program: &TypedTrees,
    requester: SymbolHandle,
    declaration: typed_trees::callable_signature_shapes::CallableDeclaration<'_>,
    request: &TableCallExpression,
    arguments: &[crate::BuildTimeValue],
    request_expression: ExpressionHandle,
    selected: Option<(
        &typed_trees::expression::StaticMachineArgument,
        SymbolHandle,
    )>,
) -> Result<PreparedSourceInvocation, String> {
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
    let mut probe = program.clone();
    // Allocate this call's real source owner before closing the selected
    // application. Its result below comes from that public signature before
    // the caller enters typed preparation or judgment.
    let original_result = declaration.shape(program)?.return_type;
    let (mut probe_machine, mut state) =
        invocation_probe_declarations(&mut probe, requester, "@const-invocation", original_result);
    let mut call = request.clone();
    if request_expression.is_valid()
        && matches!(
            declaration,
            typed_trees::callable_signature_shapes::CallableDeclaration::Signature(_)
        )
    {
        // This is another actual call of the same public requirement. Its
        // application belongs to this caller, while the original source row
        // and every supplied operand keep their existing owner and identity.
        let (_, captures, application) = probe.expression_table.copy_call_operands_from(
            &program.expression_table,
            &[],
            request.authored_machine_application,
        );
        if !captures.is_empty() || !application.is_valid() {
            return Err("copied public call has no complete source application".into());
        }
        probe
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = probe_machine.symbol;
        call.authored_machine_application = application;
    }
    let concrete = probe
        .expression_table
        .insert(ExpressionNode::Call(call.clone()));
    let selected_signature = if let Some((selection, selection_owner)) = selected {
        if request_expression.is_valid() || request.receiver.is_valid() {
            return Err("a selected invocation cannot replace an authored receiver".into());
        }
        if !program.symbols.symbols().nodes().is_valid(selection_owner) {
            return Err("selected invocation lost its supplying declaration".into());
        }
        let mut selection = probe
            .expression_table
            .copy_static_argument_from_self(selection);
        let name = typed_trees::name::Identifier::from(probe.symbols.name(selection.symbol));
        if selection.path.is_empty() {
            // The resolved declaration supplies diagnostic text only; its
            // symbol and complete source operand remain the selected identity.
            selection.path = vec![name.clone()].into_boxed_slice();
        }
        let application = if selection.type_reference.is_valid() {
            selection.type_reference
        } else {
            probe.static_operand_type_reference_in_scope(&selection, selection_owner)?
        };
        call.requirement_application = application;
        let mut path = probe_name_path(&mut probe, selection.symbol, name);
        path.static_application = probe.expression_table.insert_static_member(selection);
        call.receiver = probe.expression_table.insert(ExpressionNode::Name(path));
        *probe.expression_table.expression_mut(concrete) = ExpressionNode::Call(call);
        probe.retain_call_static_argument_context(concrete, probe_machine.symbol)?;
        call = probe
            .expression_table
            .call_node(concrete)
            .cloned()
            .ok_or("selected invocation lost its retained source call")?;
        probe.prepare_applied_callable_signatures_for_application(
            probe_machine.symbol,
            application,
        )?;
        Some(
            probe
                .named_conformance_call_member(&call)?
                .ok_or("selected invocation has no exact public requirement row")?
                .2
                .clone(),
        )
    } else {
        None
    };
    let declaration = selected_signature.as_ref().map_or(
        declaration,
        typed_trees::callable_signature_shapes::CallableDeclaration::Signature,
    );
    let shape = declaration.shape(&probe)?;
    let parameters = shape.parameters.to_vec();
    let declared_result = shape.return_type;
    let type_parameters = declaration.static_parameters(&probe)?.to_vec();
    // These are fresh caller binders, not the callee's region identities.
    // Ordinary call checking relates them through the actual formal inputs.
    probe_machine.lifetime_parameters = declaration.lifetime_parameters(&probe)?.to_vec();
    // Region substitution resolves declaration ordinals through the canonical
    // machine table, including inside nominal applications. Publish this
    // caller's telescope before constructing any such types.
    probe.push_machine(probe_machine.clone());
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
    if type_parameters
        .iter()
        .any(|parameter| !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type))
    {
        return Err("source invocation non-type static arguments are not materialized".into());
    }
    if type_parameters.len() != supplied.len() {
        return Err("source invocation lost its complete static argument list".into());
    }
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
        let application = call.authored_machine_application;
        if application.is_valid() {
            let retained = probe
                .expression_table
                .static_machine_application_arguments_mut(application);
            if retained.len() != supplied.len() {
                return Err("source invocation static application lost its argument span".into());
            }
            retained.clone_from_slice(&supplied);
        }
        if request_expression.is_valid()
            && !request.machine_arguments.is_empty()
            && call.authored_machine_application == request.authored_machine_application
        {
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
    let (input_types, result_type) = probe.instantiate_invocation_signature(
        &parameters
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect::<Vec<_>>(),
        declared_result,
        &substitutions,
        declaration,
        &mut probe_machine,
    )?;
    state.return_type = result_type;
    let mut actuals = Vec::with_capacity(arguments.len());
    let mut input_values = Vec::new();
    for (position, ((parameter, argument), destination)) in parameters
        .iter()
        .zip(arguments)
        .zip(input_types)
        .enumerate()
    {
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
        let sequence_input = matches!(reference, Some((referee, _))
                    if matches!(probe.type_reference_table.type_reference(referee), TypeReferenceNode::Slice { .. }));
        let value = if sequence_input {
            // A host view supplies its real backing as an entry input. A local
            // array literal would instead borrow storage owned by this caller.
            materialize_input(
                &mut probe,
                &mut state,
                &mut input_values,
                destination,
                argument,
            )
        } else {
            materialize_argument(
                &mut probe,
                &mut state,
                &mut input_values,
                reference.map_or(destination, |(referee, _)| referee),
                argument,
            )
        }
        .map_err(|reason| format!("constant argument `{}`: {reason}", parameter.name))?;
        let actual = if sequence_input {
            value
        } else if let Some((referee, access)) = reference {
            let name = format!("@argument-{position}");
            let symbol = probe
                .symbols
                .append_generated_children_from(
                    state.symbol,
                    [(state.symbol, SymbolKind::Local, name.as_str())],
                )
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
        // Retain the checked argument's actual type on its existing expression
        // row, so receipt consumers need not reconstruct the public signature.
        probe
            .expression_table
            .set_constructed_value_type_reference(actual, destination);
        actuals.push(actual);
    }
    if materialized && !call.machine_arguments.is_empty() {
        call.machine_arguments = supplied.into_boxed_slice();
    }
    if !declaration_argument {
        call.arguments = probe.expression_table.insert_expression_handles(actuals);
    }
    *probe.expression_table.expression_mut(concrete) = ExpressionNode::Call(call);
    let call_span = source_span
        .or_else(|| probe.symbols.symbol_provenance_source_span(requester))
        .ok_or("source invocation lost its actual requesting source")?;
    probe.expression_table.set_source_span(concrete, call_span);
    let owner = finish_invocation_probe(&mut probe, probe_machine, state, concrete)?;
    probe.retain_call_static_argument_context(concrete, owner)?;
    Ok(PreparedSourceInvocation {
        source: probe,
        caller: owner,
        call: concrete,
        input_values,
    })
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
) -> Result<SymbolHandle, String> {
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
    let positions = probe.machine_positions_for_symbol(owner);
    let [position] = positions.as_slice() else {
        return Err("source invocation lost its unique generated caller declaration".into());
    };
    probe.machines_mut()[*position as usize] = machine;
    Ok(owner)
}

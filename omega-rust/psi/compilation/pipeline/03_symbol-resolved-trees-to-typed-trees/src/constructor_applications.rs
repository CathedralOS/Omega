//! Constructors retain the complete type their payloads determine. This is
//! type formation only: the lowered checker still judges every field and all
//! generic eligibility and declaration requirements.

use diagnostics::Diagnostic;
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::name::Identifier;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{TypedTrees, data::TypeParameterKind};

pub(crate) fn prepare(program: &mut TypedTrees, frontier: usize) -> Result<(), Diagnostic> {
    let expressions: Vec<_> = program
        .expression_table
        .expression_entries()
        .filter(|(handle, _)| handle.arena_index() as usize > frontier)
        .map(|(handle, _)| handle)
        .collect();
    let owners = typed_trees::references::collect_program_operand_owners(program);
    for expression in expressions {
        infer(program, expression, &mut Vec::new())?;
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            continue;
        };
        let input = CallInput {
            target: call.target_symbol,
            name: call.target.clone(),
            application: call.authored_machine_application,
            explicit: !call.machine_arguments.is_empty(),
            arguments: program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec(),
        };
        let mut callers = owners.expression_owners(expression);
        let Some(caller) = callers.next() else {
            continue;
        };
        if !caller.is_valid() || callers.any(|other| other != caller) {
            continue;
        }
        if let Some(application) = infer_call(program, input, caller)?
            && let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression)
        {
            call.authored_machine_application = application;
        }
    }
    let statements: Vec<_> = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program.machine_states(machine).iter().flat_map(|state| {
                state.statement_nodes.handles().filter_map(|handle| {
                    let call = program.statement_table.statement(handle).as_call()?;
                    Some((
                        machine.symbol,
                        handle,
                        CallInput {
                            target: call.target_symbol,
                            name: call.target.clone(),
                            application: call.authored_machine_application,
                            explicit: !call.machine_arguments.is_empty(),
                            arguments: program
                                .statement_table
                                .expression_handles(call.arguments)
                                .to_vec(),
                        },
                    ))
                })
            })
        })
        .collect();
    for (caller, handle, input) in statements {
        if let Some(application) = infer_call(program, input, caller)?
            && let typed_trees::statement::StatementNode::Call(call) =
                program.statement_table.statement_mut(handle)
        {
            call.authored_machine_application = application;
        }
    }
    Ok(())
}

/// Both call positions ask the same application question; only their operand
/// arenas and the place where the answer is retained differ.
struct CallInput {
    target: SymbolHandle,
    name: Identifier,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    explicit: bool,
    arguments: Vec<ExpressionHandle>,
}

fn infer_call(
    program: &mut TypedTrees,
    call: CallInput,
    caller: SymbolHandle,
) -> Result<Option<typed_trees::static_applications::StaticMachineApplicationHandle>, Diagnostic> {
    if call.explicit
        || (call.application.is_valid()
            && !program
                .expression_table
                .static_machine_application_arguments(call.application)
                .is_empty())
    {
        return Ok(None);
    }
    let Some((owner, signature)) = program.trait_machine_signature(call.target) else {
        return Ok(None);
    };
    if !owner.type_parameters.is_empty()
        || !owner.lifetime_parameters.is_empty()
        || !signature.lifetime_parameters.is_empty()
    {
        return Ok(None);
    }
    let parameters = program.state_signature_type_parameters(signature).to_vec();
    if parameters.is_empty()
        || parameters
            .iter()
            .any(|parameter| parameter.kind != TypeParameterKind::Type)
    {
        return Ok(None);
    }
    let formals = program.state_signature_parameters(signature).to_vec();
    if formals.len() != call.arguments.len() {
        return Ok(None);
    }
    let mut inferred = vec![None; parameters.len()];
    for (formal, argument) in formals.iter().zip(call.arguments) {
        if let Some(actual) = infer(program, argument, &mut Vec::new())? {
            unify(
                program,
                formal.type_reference,
                actual,
                &parameters,
                &mut inferred,
            )?;
        }
    }
    let Some(inferred) = inferred.into_iter().collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    let arguments: Vec<_> = inferred
        .into_iter()
        .map(|type_reference| StaticMachineArgument {
            type_reference,
            ..Default::default()
        })
        .collect();
    let application = if call.application.is_valid() {
        program
            .expression_table
            .set_static_machine_application_arguments(call.application, &arguments);
        call.application
    } else {
        let application = program.expression_table.retain_static_machine_application(
            call.target,
            call.name,
            &arguments,
        );
        program
            .expression_table
            .static_machine_application_mut(application)
            .lexical_owner = caller;
        application
    };
    Ok(Some(application))
}

fn builtin(program: &mut TypedTrees, atom: BuiltinTypeAtom) -> Option<TypeReferenceHandle> {
    let symbol = program.symbols.builtin_atom_symbol(atom)?;
    let name = Identifier::generated(program.symbols.name(symbol).to_owned());
    Some(
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named { symbol, name }),
    )
}

fn infer(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    active: &mut Vec<ExpressionHandle>,
) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
    if !program.expression_table.expression_is_valid(value) {
        return Ok(None);
    }
    let retained = program
        .expression_table
        .constructed_value_type_reference(value);
    if retained.is_valid() {
        return Ok(Some(retained));
    }
    if active.contains(&value) {
        return Err(Diagnostic::error(
            "constructor type inference encountered a cyclic expression",
        ));
    }
    active.push(value);
    let result = match program.expression_table.expression(value).clone() {
        ExpressionNode::StructLiteral(literal) => {
            if literal.application_type.is_valid() {
                Some(literal.application_type)
            } else if let Some(data) = program
                .unique_data_definition_by_symbol(literal.type_symbol)
                .cloned()
            {
                let parameters = program.data_type_parameters(&data).to_vec();
                if parameters.is_empty() {
                    Some(
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::Named {
                                symbol: data.symbol,
                                name: data.name.clone(),
                            }),
                    )
                } else if data.lifetime_parameters.is_empty() {
                    let mut inferred = vec![None; parameters.len()];
                    let fields = program
                        .expression_table
                        .struct_fields(literal.fields)
                        .to_vec();
                    for field in fields {
                        if let Some(declared) =
                            crate::typing_queries::exact_declared_field(program, field.field_symbol)
                                .map(|field| field.type_reference)
                            && let Some(actual) = infer(program, field.value, active)?
                        {
                            unify(program, declared, actual, &parameters, &mut inferred)?;
                        }
                    }
                    // Only the canonical locator vocabulary defines inactive
                    // coordinates to be zero. Ordinary generic data must have
                    // every argument determined by its payload or destination.
                    if matches!(
                        program.core_vocabulary(data.symbol),
                        Some(language_core::core_vocabulary::CoreVocabulary::ExternalBinding(_))
                    ) {
                        let used: Vec<_> = program
                            .data_members(&data)
                            .iter()
                            .filter_map(|member| match member {
                                typed_trees::data::DataMember::Variant(case)
                                    if Some(case.symbol) == literal.case_symbol =>
                                {
                                    Some(case)
                                }
                                _ => None,
                            })
                            .flat_map(|case| program.data_payload_fields(case))
                            .flat_map(|field| {
                                typed_trees::references::collect_type_symbol_dependencies(
                                    program,
                                    field.type_reference,
                                )
                            })
                            .collect();
                        for (parameter, argument) in parameters.iter().zip(&mut inferred) {
                            if argument.is_none()
                                && !used.contains(&parameter.symbol)
                                && matches!(parameter.kind, TypeParameterKind::Const { .. })
                            {
                                *argument = Some(integer_argument(program, 0));
                            }
                        }
                    }
                    if let Some(arguments) = inferred.into_iter().collect::<Option<Vec<_>>>() {
                        let arguments = program
                            .type_reference_table
                            .insert_type_reference_handles(arguments);
                        Some(
                            program
                                .type_reference_table
                                .insert(TypeReferenceNode::Generic {
                                    base_symbol: data.symbol,
                                    base_name: data.name.clone(),
                                    arguments,
                                    lifetime_arguments: Vec::new(),
                                }),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        ExpressionNode::String(bytes) => {
            builtin(program, BuiltinTypeAtom::U8).map(|element_type| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(bytes.len()),
                    })
            })
        }
        ExpressionNode::Integer(literal) => literal.landing().and_then(|landing| {
            builtin(
                program,
                typed_trees::types::builtin_type_atom(landing.landed_type),
            )
        }),
        ExpressionNode::Boolean(_) => builtin(program, BuiltinTypeAtom::Bool),
        ExpressionNode::Name(path) => {
            let binder = path.resolved_symbol();
            binder.and_then(|binder| {
                let owner = program.symbols.get(binder).parent;
                let state = program.state_by_symbol(owner).or_else(|| {
                    program
                        .machine_by_symbol(owner)
                        .and_then(|machine| program.machine_states(machine).first())
                });
                state
                    .and_then(|state| {
                        crate::typing_queries::declared_place_type(program, state, value).ok()
                    })
                    .or_else(|| {
                        program
                            .const_declaration_by_symbol(binder)
                            .map(|declaration| declaration.declared_type)
                    })
            })
        }
        ExpressionNode::Call(call) => call_result(program, &call)?,
        ExpressionNode::ZeroValue(reference) | ExpressionNode::TypeExpression(reference) => {
            Some(reference)
        }
        _ => None,
    };
    active.pop();
    if let Some(reference) = result
        && matches!(
            program.expression_table.expression(value),
            ExpressionNode::StructLiteral(_)
        )
    {
        if let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression_mut(value)
        {
            literal.application_type = reference;
        }
        program
            .expression_table
            .set_constructed_value_type_reference(value, reference);
    }
    Ok(result)
}

fn integer_argument(program: &mut TypedTrees, value: usize) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(value.to_string()),
        })
}

fn unify(
    program: &mut TypedTrees,
    formal: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    parameters: &[typed_trees::data::TypeParameter],
    inferred: &mut [Option<TypeReferenceHandle>],
) -> Result<(), Diagnostic> {
    let formal_node = program.type_reference_table.type_reference(formal).clone();
    let actual_node = program.type_reference_table.type_reference(actual).clone();
    match (formal_node, actual_node) {
        (TypeReferenceNode::Named { symbol, .. }, _)
            if parameters.iter().any(|p| p.symbol == symbol) =>
        {
            bind(program, symbol, actual, parameters, inferred)?;
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: formal,
                arguments: left,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: actual,
                arguments: right,
                ..
            },
        ) if formal == actual => {
            let left = program
                .type_reference_table
                .type_reference_handles(left)
                .to_vec();
            let right = program
                .type_reference_table
                .type_reference_handles(right)
                .to_vec();
            if left.len() == right.len() {
                for (left, right) in left.into_iter().zip(right) {
                    unify(program, left, right, parameters, inferred)?;
                }
            }
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: left,
                length: FixedArrayLength::ConstParameter { symbol, .. },
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                length: FixedArrayLength::Literal(length),
            },
        ) => {
            let argument = integer_argument(program, length);
            bind(program, symbol, argument, parameters, inferred)?;
            unify(program, left, right, parameters, inferred)?;
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: left, ..
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                ..
            },
        )
        | (
            TypeReferenceNode::Reference { referee: left, .. },
            TypeReferenceNode::Reference { referee: right, .. },
        ) => {
            unify(program, left, right, parameters, inferred)?;
        }
        _ => {}
    }
    Ok(())
}

fn bind(
    program: &TypedTrees,
    symbol: SymbolHandle,
    actual: TypeReferenceHandle,
    parameters: &[typed_trees::data::TypeParameter],
    inferred: &mut [Option<TypeReferenceHandle>],
) -> Result<(), Diagnostic> {
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    else {
        return Ok(());
    };
    if let Some(previous) = inferred[position]
        && program.normalized_type_identity(previous) != program.normalized_type_identity(actual)
    {
        return Err(Diagnostic::error(
            "constructor or call arguments determine conflicting generic arguments",
        ));
    }
    inferred[position] = Some(actual);
    Ok(())
}

/// A call's result carries the same application as its declaration. Reading
/// that type does not claim the call satisfies its preconditions.
fn call_result(
    program: &mut TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
    let declaration =
        if let Some((_, signature)) = program.trait_machine_signature(call.target_symbol) {
            Some((
                signature.return_type,
                program.state_signature_type_parameters(signature).to_vec(),
            ))
        } else {
            let machine = program.machine_by_symbol(call.target_symbol).or_else(|| {
                program
                    .state_by_symbol(call.target_symbol)
                    .and_then(|state| {
                        program.machine_by_symbol(program.symbols.get(state.symbol).parent)
                    })
            });
            machine.and_then(|machine| {
                program.machine_states(machine).first().map(|state| {
                    (
                        state.return_type,
                        program.machine_type_parameters(machine).to_vec(),
                    )
                })
            })
        };
    let Some((result, parameters)) = declaration else {
        return Ok(None);
    };
    if !result.is_valid() {
        return Ok(None);
    }
    if parameters.is_empty() {
        return Ok(Some(result));
    }
    let arguments = if call.authored_machine_application.is_valid() {
        program
            .expression_table
            .static_machine_application_arguments(call.authored_machine_application)
            .to_vec()
    } else {
        call.machine_arguments.to_vec()
    };
    if arguments.len() != parameters.len() {
        return Ok(None);
    }
    let mut substitutions = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(&arguments) {
        if !matches!(
            parameter.kind,
            TypeParameterKind::Type | TypeParameterKind::Const { .. }
        ) {
            return Ok(None);
        }
        let Ok(reference) = program.static_operand_type_reference(argument) else {
            return Ok(None);
        };
        let reference = if matches!(parameter.kind, TypeParameterKind::Const { .. }) {
            let Some(value) = program
                .static_const_literal_from_type_reference(reference)
                .and_then(|literal| literal.value_u64())
                .and_then(|value| usize::try_from(value).ok())
            else {
                return Ok(None);
            };
            integer_argument(program, value)
        } else {
            reference
        };
        substitutions.push((parameter.symbol, reference));
    }
    program
        .type_reference_table
        .instantiate(result, &substitutions)
        .map(Some)
        .map_err(Diagnostic::error)
}

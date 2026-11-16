//! Constructed values retain the types their payloads determine. Ordinary
//! call families use those same types to select one declared parameter list.
//! This forms types and selections; the lowered checker still judges fields,
//! argument admission, generic eligibility and declaration requirements.

use diagnostics::Diagnostic;
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::name::Identifier;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{TypedTrees, data::TypeParameterKind};

pub(crate) fn prepare(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    frontier: usize,
) -> Result<(), Diagnostic> {
    let expressions: Vec<_> = program
        .expression_table
        .expression_entries()
        .filter(|(handle, _)| handle.arena_index() as usize > frontier)
        .map(|(handle, _)| handle)
        .collect();
    let owners = typed_trees::references::collect_program_operand_owners(program);
    for expression in expressions {
        infer(program, expression, &mut Vec::new())?;
        let needs_owner = match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(_) | ExpressionNode::Call(_) => true,
            ExpressionNode::Cast(cast) => cast.form.is_recast(),
            ExpressionNode::Indexed(indexed) => {
                program.expression_table.range_node(indexed.index).is_some()
            }
            _ => false,
        };
        if !needs_owner {
            continue;
        }
        let mut callers = owners.expression_owners(expression);
        let Some(first) = callers.next() else {
            continue;
        };
        let caller = if let Some(call) = program.expression_table.call_node(expression)
            && call.target_symbol.is_valid()
            && call.authored_machine_application.is_valid()
            && let owner = program
                .expression_table
                .static_machine_application(call.authored_machine_application)
                .lexical_owner
            && owner.is_valid()
            && (owner == first || callers.any(|other| other == owner))
        {
            // A constant and its generated entry may share an expression.
            // Its already selected source application supplies the scope.
            owner
        } else {
            if !first.is_valid()
                || owners
                    .expression_owners(expression)
                    .any(|other| other != first)
            {
                continue;
            }
            first
        };
        if let ExpressionNode::Cast(cast) = program.expression_table.expression(expression)
            && let Some(access) = cast.form.recast_access()
        {
            // A recast forms its written reference even without a destination.
            // Keep the occurrence and its lexical lifetime owner just as for
            // an explicit borrow; lowering must not invent its result type.
            let referent = cast.target_type;
            let reference = reference_formation_type(program, referent, access);
            program
                .type_reference_table
                .retain_operand_lifetime_owner(reference, caller)
                .map_err(Diagnostic::error)?;
            program
                .expression_table
                .set_constructed_value_type_reference(expression, reference);
            continue;
        }
        if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) {
            let collection = indexed.collection;
            // A bare range observes the actual collection's elements,
            // not one element. Retain its shared view before ordinary callers
            // and borrow formations ask for its type. A selected user operator
            // must supply its own result instead of acquiring a builtin type.
            let Some(collection) =
                crate::typing_queries::declared_expression_type(program, caller, collection)
            else {
                continue;
            };
            let Some(count) = builtin(program, BuiltinTypeAtom::U64) else {
                continue;
            };
            let operands = [Some(collection), Some(count), Some(count)];
            if !typed_trees::operator::resolve_indexed_spelling_for_operands(
                program,
                language_core::OperatorSpelling::Range,
                &operands,
                program.expression_table.source_span(expression),
            )
            .is_empty()
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    caller,
                    expression,
                    language_core::OperatorSpelling::Range,
                    &operands,
                )
            {
                continue;
            }
            let Ok(element_type) =
                crate::typing_queries::collection_element_type(program, collection)
            else {
                continue;
            };
            let slice = program
                .type_reference_table
                .insert(TypeReferenceNode::Slice { element_type });
            let view = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: slice,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: None,
                });
            program
                .type_reference_table
                .retain_operand_lifetime_owner(view, caller)
                .map_err(Diagnostic::error)?;
            program
                .expression_table
                .set_constructed_value_type_reference(expression, view);
            continue;
        }
        if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
            let (target, access) = (borrow.target, borrow.access);
            // The destination may attenuate this reference. Retain the type
            // of the written formation first, so lowering can form its actual
            // parent loan before lending a weaker child to that destination.
            let referent =
                match crate::typing_queries::declared_expression_type(program, caller, target) {
                    Some(reference) => Some(reference),
                    None => infer(program, target, &mut Vec::new())?,
                };
            if let Some(referent) = referent {
                let reference = reference_formation_type(program, referent, access);
                program
                    .type_reference_table
                    .retain_operand_lifetime_owner(reference, caller)
                    .map_err(Diagnostic::error)?;
                program
                    .expression_table
                    .set_constructed_value_type_reference(expression, reference);
            }
            continue;
        }
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            continue;
        };
        let mut input = CallInput {
            target: call.target_symbol,
            name: call.target.clone(),
            application: call.authored_machine_application,
            explicit: !call.machine_arguments.is_empty(),
            ordinary: call.is_ordinary_positional_call()
                && call.authored_call_binding.is_none()
                && !call.authored_machine_application.is_valid(),
            selection: call.authored_call_selection,
            source_span: program.expression_table.source_span(expression),
            arguments: program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec(),
        };
        let previous = (input.target, input.application);
        let application = infer_call(source, program, &mut input, caller)?;
        if input.target == previous.0 && application.is_none_or(|next| next == previous.1) {
            continue;
        }
        if let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) {
            call.target_symbol = input.target;
            if let Some(application) = application {
                call.authored_machine_application = application;
            }
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
                            ordinary: call.receiver.is_empty()
                                && !call.static_machine_parameter.is_valid()
                                && call.static_requirement_dispatch.is_none()
                                && call.installed_interrupt_operation.is_none()
                                && call.machine_arguments.is_empty()
                                && call.evidence_arguments.is_empty()
                                && !call.authored_machine_application.is_valid(),
                            selection: call.authored_call_selection,
                            source_span: call.source_span,
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
    for (caller, handle, mut input) in statements {
        let previous = (input.target, input.application);
        let application = infer_call(source, program, &mut input, caller)?;
        if input.target == previous.0 && application.is_none_or(|next| next == previous.1) {
            continue;
        }
        if let Some(call) = program.statement_table.call_mut(handle) {
            call.target_symbol = input.target;
            if let Some(application) = application {
                call.authored_machine_application = application;
            }
        }
    }
    let terminal_calls: Vec<_> = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program.machine_states(machine).iter().flat_map(|state| {
                state.statement_nodes.handles().flat_map(|statement| {
                    program
                        .statement_table
                        .statement(statement)
                        .as_transition()
                        .into_iter()
                        .flat_map(|transition| transition.targets())
                        .filter_map(|handle| {
                            let typed_trees::statement::TransitionTargetNode::Named {
                                path,
                                static_machine_parameter,
                                arguments,
                                evidence_arguments,
                                source_span,
                                authored_call_selection,
                                ..
                            } = program.statement_table.transition_target(handle)
                            else {
                                return None;
                            };
                            let [name] = program.statement_table.name_path_members(path.members)
                            else {
                                return None;
                            };
                            Some((
                                machine.symbol,
                                handle,
                                CallInput {
                                    target: path.symbol,
                                    name: name.clone(),
                                    application: Default::default(),
                                    explicit: false,
                                    ordinary: !path.receiver_symbol.is_valid()
                                        && !static_machine_parameter.is_valid()
                                        && evidence_arguments.is_empty(),
                                    selection: *authored_call_selection,
                                    source_span: *source_span,
                                    arguments: program
                                        .statement_table
                                        .expression_handles(*arguments)
                                        .to_vec(),
                                },
                            ))
                        })
                })
            })
        })
        .collect();
    for (caller, handle, mut input) in terminal_calls {
        let original = input.target;
        if original.is_valid() || !input.ordinary {
            continue;
        }
        infer_call(source, program, &mut input, caller)?;
        if input.target != original
            && let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target_mut(handle)
        {
            path.symbol = input.target;
            path.head_symbol = input.target;
        }
    }
    Ok(())
}

/// Every call position asks the same application question; only its operand
/// arenas and the place where the answer is retained differ.
struct CallInput {
    target: SymbolHandle,
    name: Identifier,
    application: typed_trees::static_applications::StaticMachineApplicationHandle,
    explicit: bool,
    ordinary: bool,
    selection: Option<typed_trees::AuthoredDeclarationSelectionOccurrenceId>,
    source_span: source::SourceSpan,
    arguments: Vec<ExpressionHandle>,
}

fn infer_call(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    call: &mut CallInput,
    caller: SymbolHandle,
) -> Result<Option<typed_trees::static_applications::StaticMachineApplicationHandle>, Diagnostic> {
    let source_owner = if program.symbols.get(caller).kind == symbols::SymbolKind::State {
        program.symbols.get(caller).parent
    } else {
        caller
    };
    // The declared input retains trait-default lexical context before typing
    // drops that template-only field. An unresolved bare call there still needs
    // its public requirement dispatcher, not an unrelated root declaration.
    let ordinary_scope = call.name.as_str().contains("::")
        || source.machine(source_owner).is_some_and(|owner| {
            owner.trait_default_origin.trait_source.is_empty()
                && !owner.trait_default_origin.declaring_trait.is_valid()
        });
    if !call.target.is_valid()
        && call.ordinary
        && ordinary_scope
        && let Some(target) = select_closed_call(program, call, caller)?
    {
        if let Some(occurrence) = call.selection {
            crate::call_and_product_selections::bind_generated_call_selection(
                program, occurrence, target,
            )?;
        }
        call.target = target;
    }
    let supplied = if call.application.is_valid() {
        program
            .expression_table
            .static_machine_application_arguments(call.application)
    } else if call.explicit {
        // A different source context owns arguments that were not retained
        // as an ordinary application.
        return Ok(None);
    } else {
        &[]
    };
    let (parameters, formals) =
        if let Some((owner, signature)) = program.trait_machine_signature(call.target) {
            if !supplied.is_empty()
                || !owner.type_parameters.is_empty()
                || !owner.lifetime_parameters.is_empty()
                || !signature.lifetime_parameters.is_empty()
            {
                return Ok(None);
            }
            (
                program.state_signature_type_parameters(signature).to_vec(),
                program.state_signature_parameters(signature).to_vec(),
            )
        } else {
            // The retained declaration owns the written prefix. Complete
            // that application before specialization asks for its bindings.
            if !call.application.is_valid() {
                return Ok(None);
            }
            let application = program
                .expression_table
                .static_machine_application(call.application);
            let Some(machine) = program.machine_by_symbol(application.binding_declaration) else {
                return Ok(None);
            };
            let state = if call.target == machine.symbol {
                program.machine_states(machine).first()
            } else {
                program
                    .state_by_symbol(call.target)
                    .filter(|state| program.symbols.get(state.symbol).parent == machine.symbol)
            };
            let Some(state) = state else {
                return Ok(None);
            };
            if machine.has_bodyless_signature() || machine.type_parameters.len() <= supplied.len() {
                return Ok(None);
            }
            (
                program.machine_callable_parameters(machine),
                program.state_parameters(state).to_vec(),
            )
        };
    let Some(omitted) = parameters.get(supplied.len()..) else {
        return Ok(None);
    };
    if omitted.is_empty()
        || omitted
            .iter()
            .any(|parameter| parameter.kind != TypeParameterKind::Type)
        || formals.len() != call.arguments.len()
    {
        return Ok(None);
    }
    let mut arguments = supplied.to_vec();
    let mut inferred = vec![None; omitted.len()];
    for (formal, argument) in formals.iter().zip(call.arguments.iter().copied()) {
        if let Some(actual) = infer(program, argument, &mut Vec::new())? {
            unify(
                program,
                formal.type_reference,
                actual,
                omitted,
                &mut inferred,
            )
            .map_err(|error| error.with_source_span(call.source_span))?;
        }
    }
    let Some(inferred) = inferred.into_iter().collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    arguments.extend(
        inferred
            .into_iter()
            .map(|type_reference| StaticMachineArgument {
                type_reference,
                ..Default::default()
            }),
    );
    let application = if call.application.is_valid() {
        program
            .expression_table
            .set_static_machine_application_arguments(call.application, &arguments);
        call.application
    } else {
        let application = program.expression_table.retain_static_machine_application(
            call.target,
            call.name.clone(),
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

/// The complete family allowed by this exact occurrence's ordinary source
/// scope. Existence distinguishes an unserved choice from an unknown name;
/// it never supplies a selected body, contract or invocation permission.
/// `None` retains an unserved required-contract scope; an empty family means
/// that no ordinary declaration has the name in this occurrence's scope.
pub(crate) fn source_call_candidates(
    program: &TypedTrees,
    owner: SymbolHandle,
    name: &str,
    source: source::SourceSpan,
) -> Option<Vec<SymbolHandle>> {
    use symbols::SymbolKind;
    if !program.symbols.symbols().nodes().is_valid(owner) {
        return None;
    }
    let kinds = &[
        SymbolKind::Machine,
        SymbolKind::Operator,
        SymbolKind::Proposition,
        SymbolKind::MathematicalDefinition,
    ];
    let owner = if program.symbols.get(owner).kind == SymbolKind::State {
        program.symbols.get(owner).parent
    } else {
        owner
    };
    let lexical = program
        .symbols
        .lexical_declaration_candidates(owner, name, kinds);
    if !lexical.is_empty() {
        return Some(lexical);
    }
    if !name.contains("::") {
        // A missing ordinary declaration in a required-contract scope is not
        // permission to borrow an unrelated root name. Its dispatch owner
        // must retain the original public requirement/application first.
        let mut scope = program.symbols.authored_origin(owner);
        for _ in 0..program.symbols.symbols().nodes().len() {
            if !program.symbols.symbols().nodes().is_valid(scope) {
                break;
            }
            let symbol = program.symbols.get(scope);
            if symbol.declaration_owner.is_valid()
                || matches!(symbol.kind, SymbolKind::Trait | SymbolKind::Conformance)
            {
                return None;
            }
            if !matches!(symbol.kind, SymbolKind::Machine | SymbolKind::State) {
                break;
            }
            scope = symbol.parent;
        }
    }
    Some(
        program
            .symbols
            .top_level_candidates_from_source(name, kinds, source),
    )
}

// A family is not an invocation target. Read every source-visible declaration
// and the whole actual operand list before retaining one exact public header.
// Unsupported members keep the family unresolved; they cannot be discarded to
// make another member win. The lowered checker still judges the selected call.
fn select_closed_call(
    program: &mut TypedTrees,
    call: &CallInput,
    caller: SymbolHandle,
) -> Result<Option<SymbolHandle>, Diagnostic> {
    let Some(candidates) =
        source_call_candidates(program, caller, call.name.as_str(), call.source_span)
    else {
        return Ok(None);
    };
    if candidates.is_empty() {
        return Ok(None);
    }
    let mut headers = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let Some(machine) = program.machine_by_symbol(candidate) else {
            return Ok(None);
        };
        // Lifetime names do not choose a carrier overload. The existing
        // carrier identity compares type, access and domains; ordinary call
        // formation must still instantiate and check the selected lifetimes.
        if !program.machine_type_parameters(machine).is_empty()
            || machine.attached_data.is_some()
            || program.symbols.get(candidate).declaration_owner.is_valid()
        {
            return Ok(None);
        }
        let identity = program.machine_callable_identity(machine);
        if !identity.is_valid()
            || !program.callable_result_dispatch_is_empty(
                program.callable_identity_result_dispatch(identity),
            )
        {
            return Ok(None);
        }
        let Some(state) = program.machine_states(machine).first() else {
            return Ok(None);
        };
        headers.push((
            candidate,
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect::<Vec<_>>(),
        ));
    }
    let mut selected = None;
    for (candidate, parameters) in headers {
        if parameters.len() != call.arguments.len() {
            continue;
        }
        let mut compatible = true;
        for (argument, expected) in call.arguments.iter().zip(&parameters) {
            let actual = infer_at_destination(program, *argument, *expected, &mut Vec::new())?;
            compatible &= actual.is_some_and(|actual| same_closed_type(program, actual, *expected));
        }
        if !compatible {
            continue;
        }
        if selected.is_some_and(|previous| previous != candidate) {
            return Err(Diagnostic::error(
                "the complete argument types select more than one declared machine",
            )
            .with_source_span(call.source_span));
        }
        selected = Some(candidate);
    }
    Ok(selected)
}

fn same_closed_type(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !program.type_reference_table.contains_type_reference(actual)
        || !program
            .type_reference_table
            .contains_type_reference(expected)
    {
        return false;
    }
    let actual = program.normalized_type_identity(actual);
    let expected = program.normalized_type_identity(expected);
    actual.is_valid() && expected.is_valid() && actual == expected
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

/// Borrowing an existing reference lends its referent again. Range views use
/// that same formation, preserving their element type and the written access.
fn reference_formation_type(
    program: &mut TypedTrees,
    mut referee: TypeReferenceHandle,
    access: language_core::ReferenceAccess,
) -> TypeReferenceHandle {
    let bare = program
        .type_reference_table
        .unconstrained_type_reference(referee);
    if let TypeReferenceNode::Reference {
        referee: backing, ..
    } = program.type_reference_table.type_reference(bare)
    {
        referee = *backing;
    }
    program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: None,
        })
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
        ExpressionNode::Borrow(borrow) => infer(program, borrow.target, active)?
            .map(|referee| reference_formation_type(program, referee, borrow.access)),
        ExpressionNode::ArrayLiteral(_) => infer_array(program, value, None, active)?,
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
            let reference = builtin(
                program,
                typed_trees::types::builtin_type_atom(landing.landed_type),
            )?;
            crate::type_reference::retain_arithmetic_result_type(
                &program.symbols,
                &mut program.tables.type_reference_table,
                reference,
                Some(landing.domain),
            );
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            program
                .type_reference_table
                .find_arithmetic_result_type_reference(*symbol, landing.domain)
        }),
        ExpressionNode::Float(literal) => literal
            .landing()
            .and_then(|format| builtin(program, crate::typing_queries::float_literal_atom(format))),
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

// A candidate may ask for a literal's type, but it must not install that
// candidate's destination on the expression. Only ordinary landing after a
// unique choice may commit the receiving context.
fn infer_at_destination(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
    active: &mut Vec<ExpressionHandle>,
) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
    if !program.expression_table.expression_is_valid(value) {
        return Ok(None);
    }
    if program
        .expression_table
        .constructed_value_type_reference(value)
        .is_valid()
    {
        return infer(program, value, active);
    }
    match program.expression_table.expression(value) {
        ExpressionNode::ArrayLiteral(_) => {
            if active.contains(&value) {
                return Err(Diagnostic::error(
                    "constructor type inference encountered a cyclic expression",
                ));
            }
            active.push(value);
            let result = infer_array(program, value, Some(destination), active);
            active.pop();
            result
        }
        // This older inference query reads quoted bytes as owned constructor
        // payloads. It cannot choose an ordinary overload from that answer:
        // the written byte view and contextual owned copy have distinct types.
        ExpressionNode::String(_) => Ok(None),
        _ => infer(program, value, active),
    }
}

fn infer_array(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    destination: Option<TypeReferenceHandle>,
    active: &mut Vec<ExpressionHandle>,
) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
    let receiving = destination.and_then(|destination| {
        crate::expressions::construction_destinations::array_literal_destination(
            program,
            value,
            destination,
        )
    });
    if destination.is_some() && receiving.is_none() {
        return Ok(None);
    }
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value) else {
        return Ok(None);
    };
    let elements = program
        .expression_table
        .expression_handles(*elements)
        .to_vec();
    let mut element_type = None;
    for element in &elements {
        let actual = if let Some((_, required)) = receiving {
            infer_at_destination(program, *element, required, active)?
        } else {
            infer(program, *element, active)?
        };
        let Some(actual) = actual else {
            return Ok(None);
        };
        if element_type.is_some_and(|previous| !same_closed_type(program, previous, actual)) {
            return Ok(None);
        }
        element_type = Some(actual);
    }
    let Some(element_type) = element_type.or_else(|| receiving.map(|(_, element)| element)) else {
        return Ok(None);
    };
    // A literal can form the received collection carrier; an existing array
    // or reference never reaches this path and keeps its written type.
    let node = if receiving.is_some_and(|(carrier, _)| {
        matches!(
            program.type_reference_table.type_reference(carrier),
            TypeReferenceNode::Slice { .. }
        )
    }) {
        TypeReferenceNode::Slice { element_type }
    } else {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(elements.len()),
        }
    };
    Ok(Some(program.type_reference_table.insert(node)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualified_operator_home_tests::checked;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    #[test]
    fn recasts_retain_their_written_reference_type_and_lifetime_owner() {
        let program = checked(&[(
            "main.omg",
            "machine observe(items: &[u32]) { let bytes: &[u8] = items as &[u8]; }",
        )])
        .expect("a recast has an exact written destination");
        let (expression, cast) = program
            .expression_table
            .expression_entries()
            .find_map(|(expression, node)| match node {
                ExpressionNode::Cast(cast) if cast.form.is_recast() => Some((expression, cast)),
                _ => None,
            })
            .unwrap();
        let reference = program
            .expression_table
            .constructed_value_type_reference(expression);
        assert!(matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                ..
            } if *referee == cast.target_type
        ));
        let owner = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "observe")
            .unwrap()
            .symbol;
        assert_eq!(
            program
                .type_reference_table
                .operand_lifetime_owner(reference),
            owner
        );
    }

    #[test]
    fn ordinary_call_inference_completes_the_existing_application() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../tests/fixtures/run/generics/inferred_static_call_types/main.omg"
        ));
        let program = checked(&[("main.omg", source)]).expect("the fixture types");
        let expressions = &program.expression_table;
        let mut completed = 0;
        for (_, node) in expressions.expression_entries() {
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            let [written] = call.machine_arguments.as_ref() else {
                continue;
            };
            if call.target.as_str() != "apply" {
                continue;
            }
            let [selected, inferred] =
                expressions.static_machine_application_arguments(call.authored_machine_application)
            else {
                panic!("the source application needs the written callback and its inferred type");
            };
            assert_eq!(selected, written);
            let inferred_type = program
                .type_reference_table
                .type_reference(inferred.type_reference);
            assert!(matches!(
                inferred_type,
                TypeReferenceNode::Named { symbol, .. }
                    if program.symbols.builtin_type_atom(*symbol) == Some(BuiltinTypeAtom::Bool)
            ));
            completed += 1;
        }
        assert!(completed >= 3, "constant and runtime applications");
    }

    #[test]
    fn prepared_statement_calls_keep_retained_body_storage_shared() {
        let mut symbols = symbols::SymbolTableBuilder::default();
        let caller = symbols.insert_root(
            symbols::SymbolKind::Machine,
            symbols::SymbolNameRef::Static("caller"),
        );
        let target = symbols.insert_root(
            symbols::SymbolKind::Machine,
            symbols::SymbolNameRef::Static("target"),
        );
        let symbols = symbols.finish();
        let source = symbol_resolved_trees::SymbolResolvedTrees {
            symbols: symbols.clone(),
            ..Default::default()
        };
        let mut program = TypedTrees {
            symbols,
            ..Default::default()
        };
        let statements = program
            .statement_table
            .insert_statements([StatementNode::Call(typed_trees::statement::TableCall {
                target_symbol: target,
                ..Default::default()
            })]);
        let states = program
            .tables
            .machine_states
            .insert_many([typed_trees::state::State {
                statement_nodes: statements,
                ..Default::default()
            }]);
        program.push_machine(typed_trees::machine::Machine {
            symbol: caller,
            states,
            ..Default::default()
        });
        let retained = program.clone();
        prepare(&source, &mut program, 0).unwrap();
        assert_eq!(program, retained);
        assert!(std::ptr::eq(
            program.statement_table.statement(statements.start()),
            retained.statement_table.statement(statements.start())
        ));
    }

    fn owned_call(
        program: &TypedTrees,
        owner: &str,
    ) -> (
        SymbolHandle,
        Option<AuthoredDeclarationSelectionOccurrenceId>,
    ) {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == owner)
            .expect("the actual caller declaration");
        let mut calls = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(value) => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(*value)
                        {
                            calls.push((call.target_symbol, call.authored_call_selection));
                        }
                    }
                    StatementNode::Call(call) => {
                        calls.push((call.target_symbol, call.authored_call_selection));
                    }
                    StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                        if let ExpressionNode::Call(call) =
                            program.expression_table.expression(local.initial_value)
                        {
                            calls.push((call.target_symbol, call.authored_call_selection));
                        }
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Named {
                                    path,
                                    authored_call_selection,
                                    ..
                                } => calls.push((path.symbol, *authored_call_selection)),
                                TransitionTargetNode::Value(value) => {
                                    if let ExpressionNode::Call(call) =
                                        program.expression_table.expression(*value)
                                    {
                                        calls.push((
                                            call.target_symbol,
                                            call.authored_call_selection,
                                        ));
                                    }
                                }
                                TransitionTargetNode::Terminal => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let [call] = calls.as_slice() else {
            panic!("one authored call in {owner}; found {}", calls.len())
        };
        *call
    }

    // These are real source calls, including their original selection rows.
    // The same declared argument types must choose the same public header in
    // value, discarded-result and terminal positions, without a body guess.
    #[test]
    fn ranges_retain_view_types_before_calls_and_exclusive_formations() {
        let program = checked(&[
            (
                "kit.omg",
                "module kit; public machine pick(value: u8) {} \
                 public machine pick(value: &[u8]) {}",
            ),
            (
                "client.omg",
                "machine ordinary(bytes: &mut [u8; 3]) { kit::pick(bytes[..]); } \
                 machine exclusive(bytes: &mut [u8; 3]) { sink(&mut bytes[..]); } \
                 machine sink(value: &mut [u8]) {}",
            ),
        ])
        .expect("the source retains its range and written borrow");
        let view_access = |reference| {
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(reference)
            else {
                panic!("the actual range result is a view")
            };
            let TypeReferenceNode::Slice { element_type } =
                program.type_reference_table.type_reference(*referee)
            else {
                panic!("the view observes the actual element slice")
            };
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(*element_type)
            else {
                panic!("the byte element keeps its declared carrier")
            };
            assert_eq!(
                program.symbols.builtin_type_atom(*symbol),
                Some(BuiltinTypeAtom::U8)
            );
            *access
        };
        let mut ranges = 0;
        let mut formations = 0;
        for (expression, node) in program.expression_table.expression_entries() {
            let expected = match node {
                ExpressionNode::Indexed(indexed)
                    if program.expression_table.range_node(indexed.index).is_some() =>
                {
                    ranges += 1;
                    language_core::ReferenceAccess::Shared
                }
                ExpressionNode::Borrow(_) => {
                    formations += 1;
                    language_core::ReferenceAccess::Mutable
                }
                _ => continue,
            };
            assert_eq!(
                view_access(
                    program
                        .expression_table
                        .constructed_value_type_reference(expression)
                ),
                expected
            );
        }
        assert_eq!((ranges, formations), (2, 1));
        let (target, _) = owned_call(&program, "ordinary");
        let callee = program
            .machine_by_symbol(target)
            .expect("the selected view overload");
        let state = program.machine_states(callee).first().unwrap();
        assert_eq!(
            view_access(program.state_parameters(state)[0].type_reference),
            language_core::ReferenceAccess::Shared
        );
    }

    #[test]
    fn closed_call_families_bind_complete_operands_in_every_call_position() {
        let library = r#"
            module kit;
            public machine pick(value: u32) -> u32 { value }
            public machine pick(value: u64) -> u64 { value }
            public machine borrow(value: &u32) {}
            public machine borrow(value: &mut u32) {}
            public machine array(values: [u32]) {}
            public machine array(values: [u64]) {}
            public machine view(values: &[u32]) {}
            public machine view(values: &[u64]) {}
        "#;
        let client = r#"
            use kit::pick;
            use kit::borrow;
            use kit::array;
            use kit::view;
            machine expression(value: u32) -> u32 { kit::pick(value) }
            machine statement(value: u64) { kit::pick(value); }
            machine terminal(value: u32) -> u32 { transition { _ -> kit::pick(value) } }
            machine shared(value: &u32) { kit::borrow(value); }
            machine exclusive(value: &mut u32) { kit::borrow(value); }
            machine array_value(value: u64) { kit::array([value]); }
            machine slice_view(value: &[u32]) { kit::view(value); }
            machine before_local(value: u32) { kit::pick(value); let kit: u32 = 0; }
            machine expression_before_local(value: u32) -> u32 {
                let result_value: u32 = kit::pick(value);
                let kit: u32 = 0;
                result_value
            }
            machine pick(value: u64) -> u64 { 0u64 }
            machine nested_precedence(value: u64) -> u64 {
                pick(value)
                machine pick(inner: u32) -> u32 { inner }
                machine pick(inner: u64) -> u64 { inner }
            }
        "#;
        let program = checked(&[("kit.omg", library), ("client.omg", client)])
            .expect("type actual source families");
        let u32_atom =
            BuiltinTypeAtom::Integer(numerics::integer_carrier::FixedInteger::unsigned(32));
        for (owner, atom, access) in [
            ("expression", u32_atom, None),
            ("before_local", u32_atom, None),
            ("expression_before_local", u32_atom, None),
            ("statement", BuiltinTypeAtom::U64, None),
            ("terminal", u32_atom, None),
            (
                "shared",
                u32_atom,
                Some(language_semantics::ReferenceAccess::Shared),
            ),
            (
                "exclusive",
                u32_atom,
                Some(language_semantics::ReferenceAccess::Mutable),
            ),
            ("array_value", BuiltinTypeAtom::U64, None),
            (
                "slice_view",
                u32_atom,
                Some(language_semantics::ReferenceAccess::Shared),
            ),
        ] {
            let (target, occurrence) = owned_call(&program, owner);
            let declaration = program
                .machine_by_own_or_state_symbol(target)
                .expect("one selected ordinary public declaration");
            assert!(declaration.is_public);
            let entry = &program.machine_states(declaration)[0];
            let Some([parameter]) = program.state_parameters(entry).as_array() else {
                panic!("one actual formal")
            };
            let mut reference = parameter.type_reference;
            if let Some(access) = access {
                let TypeReferenceNode::Reference {
                    referee,
                    access: actual,
                    ..
                } = program.type_reference_table.type_reference(reference)
                else {
                    panic!("reference formal")
                };
                assert_eq!(*actual, access);
                reference = *referee;
            }
            if matches!(owner, "array_value" | "slice_view") {
                let TypeReferenceNode::Slice { element_type } =
                    program.type_reference_table.type_reference(reference)
                else {
                    panic!("collection formal")
                };
                reference = *element_type;
            }
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                panic!("the selected declared carrier")
            };
            assert_eq!(
                program.symbols.builtin_type_atom(*symbol),
                Some(atom),
                "{owner}"
            );
            let selection = program
                .authored_declaration_selections()
                .get(occurrence.expect("authored call row"))
                .expect("same retained occurrence");
            assert_eq!(
                selection.target().as_resolved().unwrap().selected_symbol(),
                target
            );
        }
        let (target, _) = owned_call(&program, "nested_precedence");
        let owner = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "nested_precedence")
            .unwrap();
        assert_eq!(
            program.symbols.get(target).parent,
            owner.symbol,
            "the nearest nested family cannot be replaced by a root helper"
        );
    }

    #[test]
    fn closed_call_families_select_lifetime_bound_headers() {
        let library = r#"
            module kit;
            public data Payload<'text> [copy] { text: &'text [u8]; }
            public machine choose<'text>(value: Payload<'text>) -> u32 { 17u32 }
            public machine choose<'text>(value: &'text u32) -> u32 { 64u32 }
        "#;
        let client = r#"
            use kit;
            machine stored<'text>(value: kit::Payload<'text>) -> u32 { kit::choose(value) }
            machine shared(value: &u32) -> u32 { kit::choose(value) }
            machine incompatible(value: bool) -> u32 { kit::choose(value) }
            machine access_mismatch(value: &mut u32) -> u32 { kit::choose(value) }
        "#;
        let program = checked(&[("kit.omg", library), ("client.omg", client)])
            .expect("select actual lifetime-bound headers from their argument carriers");
        for owner in ["stored", "shared"] {
            let (target, occurrence) = owned_call(&program, owner);
            let declaration = program
                .machine_by_own_or_state_symbol(target)
                .expect("the complete actual selects one declared header");
            assert!(declaration.is_public);
            assert_eq!(declaration.lifetime_parameters.len(), 1);
            let Some([parameter]) = program
                .state_parameters(&program.machine_states(declaration)[0])
                .as_array()
            else {
                panic!("one retained formal")
            };
            assert_eq!(
                matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    TypeReferenceNode::Reference { .. }
                ),
                owner == "shared"
            );
            assert_eq!(
                program
                    .authored_declaration_selections()
                    .get(occurrence.expect("same source call occurrence"))
                    .unwrap()
                    .target()
                    .as_resolved()
                    .unwrap()
                    .selected_symbol(),
                target
            );
        }
        for owner in ["incompatible", "access_mismatch"] {
            assert!(!owned_call(&program, owner).0.is_valid(), "{owner}");
            let declaration = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == owner)
                .unwrap();
            assert_eq!(
                program
                    .deferred_refusal(declaration.symbol)
                    .unwrap()
                    .request,
                typed_trees::deferred_refusals::DeferredRequest::UnresolvedCallTarget
            );
        }
    }

    #[test]
    fn closed_call_families_refuse_missing_incompatible_access_domains_and_ambiguity() {
        let library = r#"
            module kit;
            public domain u32::Tagged;
            public domain u32::OtherTagged;
            public machine pick(value: u32) -> u32 { value }
            public machine pick(value: u64) -> u64 { value }
            public machine borrow(value: &u32) {}
            public machine borrow(value: &mut u32) {}
            public machine domain_value(value: u32 in Tagged) {}
            public machine domain_value(value: u32 in OtherTagged) {}
            public machine view(values: &[u32]) {}
            public machine view(values: &[u64]) {}
            public machine scoped(value: bool) -> bool { value }
            public machine scoped(value: u8) -> u8 { value }
            trait RequiredScope {
                machine scoped(value: u32) -> u32;
                machine scoped(value: u64) -> u64;
                machine forward(value: bool) -> bool { scoped(value) }
                machine broken() { undeclared_in_default(); }
            }
            ActualScope satisfies RequiredScope {
                machine scoped(value: u32) -> u32 { value }
                machine scoped(value: u64) -> u64 { value }
            }
        "#;
        let client = r#"
            use kit;
            machine incompatible(value: bool) -> u32 { kit::pick(value) }
            machine access_mismatch(value: u32) { kit::borrow(value); }
            machine domain_mismatch(value: u32) { kit::domain_value(value); }
            machine domain_match(value: u32 in Tagged) { kit::domain_value(value); }
            machine array_reference(value: &[u32; 1]) { kit::view(value); }
            machine parameter_shadow(kit: u32) { kit::pick(7u32); }
            machine local_shadow() { let kit: u32 = 0; kit::pick(7u32); }
            machine missing(value: u32) -> u32 { kit::absent(value) }
            machine pick(value: bool) -> bool { value }
            machine nested_mismatch(value: bool) -> bool {
                pick(value)
                machine pick(inner: u32) -> u32 { inner }
                machine pick(inner: u64) -> u64 { inner }
            }
        "#;
        let program = checked(&[("kit.omg", library), ("client.omg", client)])
            .expect("retain real unavailable source calls");
        // Defaults enter the ordinary machine path through an actual selected
        // implementation. An unused trait template has no typed body to inspect.
        let mut required_call = None;
        let mut unknown_call = None;
        for (handle, node) in program.expression_table.iter_expressions() {
            if let ExpressionNode::Call(call) = node {
                match call.target.as_str() {
                    "scoped" => required_call = Some(call.target_symbol),
                    "undeclared_in_default" => {
                        unknown_call = Some(program.expression_table.source_span(handle))
                    }
                    _ => {}
                }
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call()
                        && call.target.as_str() == "undeclared_in_default"
                    {
                        unknown_call = Some(call.source_span);
                    }
                }
            }
        }
        let required_target = required_call.expect("actual trait-default call");
        let implementation = program
            .conformances()
            .iter()
            .find(|declaration| {
                declaration
                    .alias
                    .as_ref()
                    .is_some_and(|name| name.as_str() == "ActualScope")
            })
            .expect("actual named implementation prepares the default");
        let selected = implementation
            .rows
            .iter()
            .find(|row| {
                row.realization_machine == required_target
                    || row.realization_state == required_target
            })
            .expect("the default retains its implementation's exact requirement row");
        let (owner, requirement) = program
            .trait_machine_signature(selected.requirement)
            .expect("the routed row retains its public header");
        assert_eq!(owner.symbol, implementation.trait_symbol);
        assert_eq!(selected.declaring_trait, owner.symbol);
        assert_eq!(requirement.name.as_str(), "scoped");
        assert!(
            !requirement.is_default,
            "the original scoped headers remain bodyless"
        );
        let unknown = unknown_call.expect("actual unknown name in a default body");
        assert!(
            program
                .deferred_refusals
                .iter()
                .any(|row| row.source_span == unknown
                    && row.request
                        == typed_trees::deferred_refusals::DeferredRequest::UndeclaredCallName),
            "an unserved requirement scope must not hide an absolutely unknown name"
        );
        assert!(
            owned_call(&program, "domain_match").0.is_valid(),
            "an exact named domain matches its actual declaration"
        );
        for owner in [
            "incompatible",
            "access_mismatch",
            "domain_mismatch",
            "array_reference",
            "parameter_shadow",
            "local_shadow",
            "nested_mismatch",
            "missing",
        ] {
            assert!(
                !owned_call(&program, owner).0.is_valid(),
                "{owner} has no matching declaration"
            );
            let declaration = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == owner)
                .unwrap();
            let request = program
                .deferred_refusal(declaration.symbol)
                .expect("actual call remains refused")
                .request;
            assert_eq!(
                request,
                if owner == "missing" {
                    typed_trees::deferred_refusals::DeferredRequest::UndeclaredCallName
                } else if matches!(owner, "parameter_shadow" | "local_shadow") {
                    typed_trees::deferred_refusals::DeferredRequest::ReceiverCallSelection
                } else {
                    typed_trees::deferred_refusals::DeferredRequest::UnresolvedCallTarget
                },
                "{owner}"
            );
        }
        let Err(error) = checked(&[
            (
                "left.omg",
                "module left; public machine pick(value: u32) -> u32 { value }",
            ),
            (
                "right.omg",
                "module right; public machine pick(value: u32) -> u32 { value }",
            ),
            (
                "client.omg",
                "use left::pick; use right::pick; machine ambiguous(value: u32) -> u32 { pick(value) }",
            ),
        ]) else {
            panic!("equal participating signatures cannot select a representative")
        };
        assert!(error.contains("more than one declared machine"), "{error}");
    }
}

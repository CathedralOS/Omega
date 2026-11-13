/*
These queries read retained declaration identities and already formed types.
They establish no contract, value inhabitation, or body judgment. A missing
required type is unavailable rather than reconstructed with the retired checker.
*/
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{
    TypedTrees,
    data::DataField,
    expression::{ExpressionHandle, TableStructLiteral},
    signature::StateParameter,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(crate) fn exact_declared_field(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&DataField> {
    if !symbol.is_valid() || program.symbols.get(symbol).kind != SymbolKind::Field {
        return None;
    }
    let mut fields = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_member_fields(program.data_members(data)))
        .filter(|field| field.symbol == symbol);
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}

pub(crate) fn call_target_parameters(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Result<&[StateParameter], Diagnostic> {
    if !program.symbols.symbols().nodes().is_valid(target) {
        return Err(crate::unavailable_judgment("selected call parameter types"));
    }
    if let Some(application) = program.closed_requirement_application(target) {
        return Ok(program.state_signature_parameters(application.signature));
    }
    if let Some(slot) = program.dynamic_slot_by_symbol(target) {
        return Ok(&slot.parameters);
    }
    if let Some(state) = program.state_by_symbol(target) {
        return Ok(program.state_parameters(state));
    }
    if let Some(machine) = program.machine_by_symbol(target) {
        return program
            .machine_states(machine)
            .first()
            .map(|state| program.state_parameters(state))
            .ok_or_else(|| crate::unavailable_judgment("call entry parameter types"));
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target) {
        return Ok(program.state_signature_parameters(signature));
    }
    if let Some(operator) = program.operator_definition_by_symbol(target) {
        return Ok(program.operator_parameters(operator));
    }
    let mut signatures = program
        .traits()
        .iter()
        .flat_map(|declaration| program.trait_machine_signatures(declaration))
        .filter(|signature| signature.symbol == target);
    if let Some(signature) = signatures.next()
        && signatures.next().is_none()
    {
        return Ok(program.state_signature_parameters(signature));
    }
    if let Some(signature) = program
        .evidence_requirement_declaration(target)
        .map_err(Diagnostic::error)?
    {
        return Ok(program.state_signature_parameters(signature));
    }
    Err(crate::unavailable_judgment("selected call parameter types"))
}

pub(crate) fn selected_construction_field_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    symbol: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let table = &program.expression_table;
    let application = [
        table.construction_required_type_reference(expression),
        table.constructed_value_type_reference(expression),
        literal.application_type,
    ]
    .into_iter()
    .find(|reference| reference.is_valid());
    if let Some(application) = application {
        if let Some(field) =
            program.data_constructor_field(application, literal.type_symbol, symbol)
        {
            let parent = program.symbols.get(field.symbol).parent;
            if program.symbols.get(parent).kind == SymbolKind::Variant {
                let case = literal.case_symbol.and_then(|case| {
                    program.data_constructor_case(application, literal.type_symbol, case)
                });
                if case.is_none_or(|case| case.symbol != parent) {
                    return Err(crate::unavailable_judgment(
                        "selected constructor case field type",
                    ));
                }
            }
            return Ok(field.type_reference);
        }
        return Err(crate::unavailable_judgment(
            "selected constructor field type",
        ));
    }
    let definition = program
        .unique_data_definition_by_symbol(literal.type_symbol)
        .ok_or_else(|| crate::unavailable_judgment("constructor declaration types"))?;
    if !program.data_type_parameters(definition).is_empty() || definition.generic_instance.is_some()
    {
        return Err(crate::unavailable_judgment(
            "applied constructor field type",
        ));
    }
    let field = exact_declared_field(program, symbol)
        .ok_or_else(|| crate::unavailable_judgment("constructor field type"))?;
    let parent = program.symbols.get(symbol).parent;
    let owner = if program.symbols.get(parent).kind == SymbolKind::Variant {
        if literal.case_symbol != Some(parent) {
            return Err(crate::unavailable_judgment("constructor case identity"));
        }
        program.symbols.get(parent).parent
    } else {
        parent
    };
    if owner != literal.type_symbol {
        return Err(crate::unavailable_judgment("constructor field owner"));
    }
    Ok(field.type_reference)
}

pub(crate) fn collection_element_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Ok(*element_type),
        _ => Err(crate::unavailable_judgment(
            "collection element type projection",
        )),
    }
}

pub(crate) fn collection_field_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    symbol: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let definition = program
        .closed_data_definition(reference)
        .ok_or_else(|| crate::unavailable_judgment("applied collection field type"))?;
    let field = program
        .data_constructor_field(reference, definition.symbol, symbol)
        .ok_or_else(|| crate::unavailable_judgment("collection field owner/type"))?;
    Ok(field.type_reference)
}

/// Read a storage destination's declared type so its literal can retain the
/// same applied constructor. Bounds, case selection, access and establishment
/// remain obligations of the consuming lowered operation.
pub(crate) fn declared_place_type(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    target: ExpressionHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let table = &program.expression_table;
    let mut spine = Vec::new();
    let mut cursor = target;
    // Scratch grows only with this expression spine. The arena's existing
    // count bounds malformed cycles without allocating a whole-arena bitmap.
    for _ in 0..table.expression_count() {
        if !table.expression_is_valid(cursor) {
            break;
        }
        match table.expression(cursor) {
            ExpressionNode::Name(path) => {
                let names = table.name_path_members(path.members);
                let selected = table.name_path_member_symbols(path.member_symbols);
                let binder = path
                    .resolved_symbol()
                    .ok_or_else(|| crate::unavailable_judgment("assignment storage binder"))?;
                if names.is_empty() || (!selected.is_empty() && selected.len() != names.len()) {
                    return Err(crate::unavailable_judgment(
                        "assignment storage path roster",
                    ));
                }
                if selected
                    .first()
                    .is_some_and(|symbol| symbol.is_valid() && *symbol != binder)
                {
                    return Err(crate::unavailable_judgment(
                        "assignment storage binder identity",
                    ));
                }
                let mut reference = program
                    .declared_binding_type_by_symbol(state, binder)
                    .ok_or_else(|| {
                        crate::unavailable_judgment("unique assignment storage declaration")
                    })?;
                for (position, name) in names.iter().enumerate().skip(1) {
                    reference = declared_field_type(
                        program,
                        reference,
                        name.as_str(),
                        selected.get(position).copied().unwrap_or_default(),
                    )?;
                }
                for expression in spine.into_iter().rev() {
                    reference = match table.expression(expression) {
                        ExpressionNode::Member(member) if member.case_variant.is_none() => {
                            declared_field_type(
                                program,
                                reference,
                                member.member.as_str(),
                                member.member_symbol,
                            )?
                        }
                        ExpressionNode::Indexed(_) => collection_element_type(program, reference)?,
                        _ => {
                            return Err(crate::unavailable_judgment("assignment storage selector"));
                        }
                    };
                    if !program
                        .type_reference_table
                        .contains_type_reference(reference)
                    {
                        return Err(crate::unavailable_judgment(
                            "assignment selected type occurrence",
                        ));
                    }
                }
                return Ok(reference);
            }
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                spine.push(cursor);
                cursor = member.receiver;
            }
            ExpressionNode::Indexed(index) => {
                spine.push(cursor);
                cursor = index.collection;
            }
            _ => break,
        }
    }
    Err(crate::unavailable_judgment("assignment storage expression"))
}

fn declared_field_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
    selected: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let carrier = program
        .type_reference_table
        .peeled_type_reference(reference);
    let data = program
        .closed_data_definition(carrier)
        .ok_or_else(|| crate::unavailable_judgment("assignment field application"))?;
    let mut fields = program
        .data_members(data)
        .iter()
        .filter_map(typed_trees::data::DataMember::as_field)
        .filter(|field| field.name.as_str() == name);
    let field = fields
        .next()
        .ok_or_else(|| crate::unavailable_judgment("assignment field declaration"))?;
    if fields.next().is_some() || (selected.is_valid() && selected != field.symbol) {
        return Err(crate::unavailable_judgment("assignment field identity"));
    }
    collection_field_type(program, carrier, field.symbol)
}

use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::{expression::ExpressionNode, types::PrimitiveType};
pub(crate) fn land_owned_fixed_byte_literal(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    let ExpressionNode::String(bytes) = program.expression_table.expression(value).clone() else {
        return Ok(());
    };
    let mut carrier = destination;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier)
    {
        carrier = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: _,
    } = program.type_reference_table.type_reference(carrier).clone()
    else {
        return Ok(());
    };

    if program.primitive_type_reference(element_type) != Some(PrimitiveType::U8) {
        return Err(Diagnostic::error(format!(
            "quoted byte literal cannot initialize owned fixed array `{}`: its element type must be `u8`",
            program.display_type_reference(destination),
        )));
    }
    let Some(width) = program.closed_fixed_array_length(carrier) else {
        return Err(Diagnostic::error(format!(
            "quoted byte literal cannot initialize owned fixed byte array `{}`: its width must be a compile-known resolved integer literal",
            program.display_type_reference(destination),
        )));
    };
    let actual = bytes.len();
    if actual != width {
        return Err(Diagnostic::error(format!(
            "quoted byte literal has {actual} source byte(s), but owned fixed byte array `{}` requires exactly {width}; fixed-array literal copying neither truncates nor pads",
            program.display_type_reference(destination),
        )));
    }

    let elements = bytes
        .iter()
        .map(|byte| {
            program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(i64::from(*byte)),
            ))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    *program.expression_table.expression_mut(value) = ExpressionNode::ArrayLiteral(elements);
    Ok(())
}

fn static_namespace(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && matches!(
            program.symbols.get(symbol).kind,
            SymbolKind::Module
                | SymbolKind::BuiltinType
                | SymbolKind::Data
                | SymbolKind::Domain
                | SymbolKind::TypeAlias
                | SymbolKind::Machine
                | SymbolKind::Trait
                | SymbolKind::Conformance
        )
}

/// What a call is written on.
#[derive(Clone, Copy)]
enum CallReceiver {
    Absent,
    /// A written path. Its symbol is invalid when the path names a value
    /// rather than a declaration.
    Named(SymbolHandle),
    /// Any other expression, which is always a value.
    Value,
}

/// Read a value's retained type in the exact formal scope. This operation
/// projects declarations and applied data views; it supplies no call verdict.
pub(crate) fn scoped_expression_type_reference(
    program: &TypedTrees,
    parameters: &[StateParameter],
    result: TypeReferenceHandle,
    expression: ExpressionHandle,
    state_scope: Option<&typed_trees::state::State>,
) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
    fn visit(
        program: &TypedTrees,
        parameters: &[StateParameter],
        result: TypeReferenceHandle,
        expression: ExpressionHandle,
        state_scope: Option<&typed_trees::state::State>,
        active: &mut Vec<ExpressionHandle>,
    ) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return Err(crate::unavailable_judgment(
                "stale or cyclic value type query",
            ));
        }
        if let Some(reference) = program
            .declared_callable_expression_type_reference(expression)
            .map_err(Diagnostic::error)?
        {
            return Ok(Some(reference));
        }
        active.push(expression);
        let reference = match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if program.expression_table.names_reserved_result(expression) {
                    result.is_valid().then_some(result)
                } else if let Some(state) = state_scope
                    && path.resolved_symbol().is_some_and(|symbol| {
                        program.symbols.symbols().nodes().is_valid(symbol)
                            && matches!(
                                program.symbols.get(symbol).kind,
                                SymbolKind::Parameter | SymbolKind::Local
                            )
                    })
                {
                    Some(declared_place_type(program, state, expression)?)
                } else if let Some(parameter) = parameters.iter().find(|parameter| {
                    parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
                }) {
                    Some(parameter.type_reference)
                } else if path.resolved_symbol().is_some_and(|symbol| {
                    program.symbols.symbols().nodes().is_valid(symbol)
                        && matches!(
                            program.symbols.get(symbol).kind,
                            SymbolKind::Parameter | SymbolKind::Local
                        )
                }) {
                    None
                } else {
                    program.first_named_value_declared_type(path.symbol, path.head_symbol)
                }
            }
            ExpressionNode::Member(member) => {
                let Some(receiver) = visit(
                    program,
                    parameters,
                    result,
                    member.receiver,
                    state_scope,
                    active,
                )?
                else {
                    active.pop();
                    return Ok(None);
                };
                let receiver = program.type_reference_table.peeled_type_reference(receiver);
                program
                    .closed_data_definition(receiver)
                    .and_then(|definition| {
                        program
                            .data_constructor_field(
                                receiver,
                                definition.symbol,
                                member.member_symbol,
                            )
                            .map(|field| field.type_reference)
                    })
            }
            ExpressionNode::Indexed(indexed) => {
                if matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    None
                } else {
                    match visit(
                        program,
                        parameters,
                        result,
                        indexed.collection,
                        state_scope,
                        active,
                    )? {
                        Some(collection) => Some(collection_element_type(program, collection)?),
                        None => None,
                    }
                }
            }
            ExpressionNode::Cast(cast) => Some(cast.target_type),
            ExpressionNode::SavedSubject(saved) => match program.saved_value_source(saved) {
                Some(value) => visit(program, parameters, result, value, state_scope, active)?,
                None => return Err(crate::unavailable_judgment("saved value source type")),
            },
            ExpressionNode::Atomic(atomic) => visit(
                program,
                parameters,
                result,
                atomic.value,
                state_scope,
                active,
            )?,
            ExpressionNode::Borrow(borrow) => visit(
                program,
                parameters,
                result,
                borrow.target,
                state_scope,
                active,
            )?,
            ExpressionNode::Match(dispatch) => {
                let arms = program.expression_table.match_arms(dispatch.arms);
                let mut joined = None;
                for arm in arms {
                    let Some(candidate) =
                        visit(program, parameters, result, arm.value, state_scope, active)?
                    else {
                        active.pop();
                        return Ok(None);
                    };
                    if joined.is_some_and(|previous| {
                        program.normalized_type_identity(previous)
                            != program.normalized_type_identity(candidate)
                    }) {
                        return Err(crate::unavailable_judgment(
                            "incompatible value type at match join",
                        ));
                    }
                    joined = Some(candidate);
                }
                joined
            }
            ExpressionNode::Call(call) => {
                if let Some(state) = program.state_by_symbol(call.target_symbol) {
                    Some(state.return_type)
                } else if let Some(machine) = program.machine_by_symbol(call.target_symbol) {
                    program
                        .machine_states(machine)
                        .first()
                        .map(|state| state.return_type)
                } else if let Some(operator) =
                    program.operator_definition_by_symbol(call.target_symbol)
                {
                    Some(operator.return_type)
                } else {
                    program
                        .trait_machine_signature(call.target_symbol)
                        .map(|(_, signature)| signature.return_type)
                }
            }
            _ => None,
        };
        active.pop();
        if let Some(reference) = reference
            && reference.is_valid()
            && !program
                .type_reference_table
                .contains_type_reference(reference)
        {
            return Err(crate::unavailable_judgment(
                "value type with stale source custody",
            ));
        }
        Ok(reference)
    }
    visit(
        program,
        parameters,
        result,
        expression,
        state_scope,
        &mut Vec::new(),
    )
}

/// The declarations a call may name directly.
struct DeclaredCallTargets<'program> {
    program: &'program TypedTrees,
    // Keys of the program's mathematical definitions, sorted.
    mathematical_definitions: Vec<(u32, u32)>,
}

impl<'program> DeclaredCallTargets<'program> {
    fn new(program: &'program TypedTrees) -> Self {
        let mut mathematical_definitions = program
            .mathematical_definitions()
            .iter()
            .map(|definition| crate::lowerer::symbol_key(definition.symbol))
            .collect::<Vec<_>>();
        mathematical_definitions.sort_unstable();
        Self {
            program,
            mathematical_definitions,
        }
    }

    /// Whether `symbol` is a declaration this call form names. Typing only
    /// says the call is bound; what the call must establish is judged where
    /// the declaration holding it is read.
    fn declares(&self, symbol: SymbolHandle, receiver: Option<SymbolHandle>) -> bool {
        let program = self.program;
        // A requirement has a declaration before it has a supplied body. Retain
        // its exact signature for lowering; availability and call obligations do
        // not become facts merely because preparation can identify the target.
        if !symbol.is_valid() || !program.symbols.symbols().nodes().is_valid(symbol) {
            return false;
        }
        if program.machine_by_own_or_state_symbol(symbol).is_some()
            || program.operator_definition_by_symbol(symbol).is_some()
            || program.trait_machine_signature(symbol).is_some()
        {
            return receiver.is_none_or(|receiver| static_namespace(program, receiver));
        }
        match program.symbols.get(symbol).kind {
            // mathematical_bindings.md, "Named mathematical definitions": a
            // top-level `let` with a parameter telescope is applied by name,
            // `core::Identity<T>(left, right)`. A module may qualify it; a
            // value never does.
            SymbolKind::MathematicalDefinition => {
                return receiver.is_none_or(|receiver| static_namespace(program, receiver))
                    && self
                        .mathematical_definitions
                        .binary_search(&crate::lowerer::symbol_key(symbol))
                        .is_ok();
            }
            // A compiler operation such as `embed(value)` has one bare name
            // and no qualified or receiver form.
            SymbolKind::BuiltinFunction => {
                return receiver.is_none()
                    && program
                        .symbols
                        .builtin_function_for_symbol(symbol)
                        .is_some();
            }
            // generics.md, "Callable parameter types": a static callable
            // parameter is called through its complete written callable
            // contract. One whose type is still to be inferred has no
            // contract to call through and stays a request.
            SymbolKind::MachineParameter => {
                return receiver.is_none()
                    && program.retained_static_machine_contract(symbol).is_some();
            }
            _ => {}
        }
        // An evidence child belongs to its actual bound namespace. Reuse the
        // applied-requirement query; a similarly named binder or a value receiver
        // cannot supply this declaration, and ambiguity remains unavailable.
        let binder = program.symbols.get(symbol).parent;
        binder.is_valid()
            && receiver.is_none_or(|receiver| receiver == binder)
            && matches!(
                program.evidence_bound_requirement(
                    program.symbols.get(binder).parent,
                    binder,
                    symbol,
                ),
                Ok(Some(_))
            )
    }

    /// What a call typing did not bind still asks of a later pass, or `None`
    /// for a call of a declared target.
    fn unserved(&self, target: SymbolHandle, receiver: CallReceiver) -> Option<DeferredRequest> {
        let receiver = match receiver {
            CallReceiver::Absent => None,
            CallReceiver::Named(symbol) => Some(symbol),
            CallReceiver::Value => return Some(DeferredRequest::ReceiverCallSelection),
        };
        if self.declares(target, receiver) {
            return None;
        }
        Some(
            if receiver.is_some_and(|receiver| !static_namespace(self.program, receiver)) {
                DeferredRequest::ReceiverCallSelection
            } else if !target.is_valid() {
                DeferredRequest::UnresolvedCallTarget
            } else {
                DeferredRequest::UncallableCallTarget
            },
        )
    }
}

/// Records every call typing left without a declared target on the
/// declaration that holds it (`typed_trees::deferred_refusals`).
///
/// Typing sees the program with every bundled library and does not know what
/// an entry reaches, so such a call is not an error of the program: it is a
/// request of the declaration, raised by whoever demands that declaration. A
/// terminal call is examined for the machines of this cohort only, as before.
/// A request in an expression no declaration owns has no reader to raise it
/// and still refuses here.
pub(crate) fn defer_unbound_calls(
    program: &mut TypedTrees,
    frontier: usize,
    completed: &[SymbolHandle],
) -> Result<(), Diagnostic> {
    use typed_trees::statement::TransitionTargetNode;
    let mut in_expressions = Vec::new();
    let mut in_declarations = Vec::new();
    {
        let program = &*program;
        let targets = DeclaredCallTargets::new(program);
        for (handle, expression) in program.expression_table.expression_entries() {
            let ExpressionNode::Call(call) = expression else {
                continue;
            };
            if call.runtime_call.is_some() {
                program
                    .declared_callable_expression_type_reference(handle)
                    .map_err(Diagnostic::error)?;
                continue;
            }
            let receiver = if !call.receiver.is_valid() {
                CallReceiver::Absent
            } else if let ExpressionNode::Name(path) =
                program.expression_table.expression(call.receiver)
            {
                CallReceiver::Named(path.symbol)
            } else {
                CallReceiver::Value
            };
            if let Some(request) = targets.unserved(call.target_symbol, receiver) {
                in_expressions.push((handle, request));
            }
        }
        for (ordinal, machine) in program.machines().iter().enumerate() {
            let in_cohort = ordinal >= frontier || completed.contains(&machine.symbol);
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call() {
                        if let Some(binding) = call.runtime_call {
                            program
                                .declared_callable_contract(binding.contract)
                                .map_err(Diagnostic::error)?;
                            if !program.expression_table.expression_is_valid(binding.callee)
                                || call.target_symbol.is_valid()
                                || call.receiver_symbol.is_valid()
                                || call.receiver_root_symbol.is_valid()
                                || !program
                                    .statement_table
                                    .name_path_members(call.receiver)
                                    .is_empty()
                                || call.static_machine_parameter.is_valid()
                                || call.static_requirement_dispatch.is_some()
                                || call.authored_machine_application.is_valid()
                                || !call.machine_arguments.is_empty()
                            {
                                return Err(crate::unavailable_judgment(
                                    "conflicting runtime statement-call metadata",
                                ));
                            }
                            continue;
                        }
                        let receiver = if program
                            .statement_table
                            .name_path_members(call.receiver)
                            .is_empty()
                        {
                            CallReceiver::Absent
                        } else {
                            CallReceiver::Named(call.receiver_symbol)
                        };
                        if let Some(request) = targets.unserved(call.target_symbol, receiver) {
                            in_declarations.push((machine.symbol, request, call.source_span));
                        }
                    }
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    if !in_cohort {
                        continue;
                    }
                    for target in transition.targets() {
                        let TransitionTargetNode::Named {
                            path, source_span, ..
                        } = program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        // Keep the exact named call for the lowered checker. It
                        // binds the arguments/result and emits every callee
                        // requirement at this terminal call. Unbound or value-based
                        // dispatch still needs its real selected correspondence.
                        let receiver = if path.receiver_symbol.is_valid() {
                            CallReceiver::Named(path.receiver_symbol)
                        } else {
                            CallReceiver::Absent
                        };
                        if let Some(request) = targets.unserved(path.symbol, receiver) {
                            in_declarations.push((machine.symbol, request, *source_span));
                        }
                    }
                }
            }
        }
        if !in_expressions.is_empty() {
            let owners = typed_trees::references::collect_program_operand_owners(program);
            for (handle, request) in in_expressions {
                let source_span = program.expression_table.source_span(handle);
                let before = in_declarations.len();
                in_declarations.extend(
                    owners
                        .expression_owners(handle)
                        .filter(|owner| owner.is_valid())
                        .map(|owner| (owner, request, source_span)),
                );
                if in_declarations.len() == before {
                    return Err(Diagnostic::error(format!(
                        "no product route yet: an expression outside every declaration is one {}",
                        request.construct()
                    ))
                    .with_source_span(source_span));
                }
            }
        }
    }
    for (declaration, request, source_span) in in_declarations {
        program.record_deferred_refusal(declaration, request, source_span);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn assignment_places_keep_applied_fields_and_refuse_foreign_or_duplicate_binders() {
        use symbols::{SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{
            data::{DataDefinition, DataField, DataMember, TypeParameter},
            expression::{TableIndexedExpression, TableMemberExpression, TableNamePath},
            machine::Machine,
            name::Identifier,
            state::State,
            types::FixedArrayLength,
        };
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let owners = builder.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("Cell")),
                (SymbolKind::Data, SymbolNameRef::Static("OtherCell")),
                (SymbolKind::Data, SymbolNameRef::Static("Row")),
                (SymbolKind::Machine, SymbolNameRef::Static("caller")),
            ],
        );
        let [cell, other, row, caller] = [0, 1, 2, 3].map(|index| owners.handle_at(index));
        let row_members = builder.insert_children(
            row,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::Field, SymbolNameRef::Static("items")),
            ],
        );
        let foreign_field = builder
            .insert_children(other, [(SymbolKind::Field, SymbolNameRef::Static("items"))])
            .start();
        let state = builder
            .insert_children(
                caller,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            )
            .start();
        let bindings = builder.insert_children(
            state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                (SymbolKind::Parameter, SymbolNameRef::Static("right")),
            ],
        );
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let mut named = |symbol, name| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
        };
        let cell_type = named(cell, "Cell");
        let other_type = named(other, "OtherCell");
        let parameter_type = named(row_members.handle_at(0), "T");
        program.push_data_definition(DataDefinition {
            symbol: cell,
            name: Identifier::generated("Cell"),
            ..Default::default()
        });
        let members = program
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: foreign_field,
                name: Identifier::generated("items"),
                type_reference: cell_type,
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: other,
            name: Identifier::generated("OtherCell"),
            members,
            ..Default::default()
        });
        let element_array = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: parameter_type,
                length: FixedArrayLength::Literal(2),
            });
        let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: row_members.handle_at(0),
            name: Identifier::generated("T"),
            ..Default::default()
        }]);
        let members = program
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: row_members.handle_at(1),
                name: Identifier::generated("items"),
                type_reference: element_array,
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: row,
            name: Identifier::generated("Row"),
            type_parameters,
            members,
            ..Default::default()
        });
        let applications = [cell_type, other_type].map(|argument| {
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles([argument]);
            program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol: row,
                    base_name: Identifier::generated("Row"),
                    arguments,
                    lifetime_arguments: Vec::new(),
                })
        });
        program
            .prepare_data_application_views(0)
            .expect("actual closed applications");
        let parameters = program
            .state_parameters
            .insert_many([0, 1].map(|index| StateParameter {
                symbol: bindings.handle_at(index),
                name: Identifier::generated(if index == 0 { "left" } else { "right" }),
                type_reference: applications[index as usize],
                ..Default::default()
            }));
        let states = program.machine_states.insert_many([State {
            symbol: state,
            parameters,
            ..Default::default()
        }]);
        program.push_machine(Machine {
            symbol: caller,
            states,
            ..Default::default()
        });
        let mut targets = Vec::new();
        let mut members = Vec::new();
        for index in [0, 1] {
            let mut names = Default::default();
            program.expression_table.push_name_path_member(
                &mut names,
                Identifier::generated(if index == 0 { "left" } else { "right" }),
            );
            let binder = bindings.handle_at(index);
            let receiver = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members: names,
                    head_symbol: binder,
                    symbol: binder,
                    ..Default::default()
                }));
            let member =
                program
                    .expression_table
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver,
                        member_symbol: row_members.handle_at(1),
                        member: Identifier::generated("items"),
                        case_variant: None,
                    }));
            let selector = program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(1),
            ));
            targets.push(program.expression_table.insert(ExpressionNode::Indexed(
                TableIndexedExpression {
                    collection: member,
                    index: selector,
                },
            )));
            members.push(member);
        }
        assert_eq!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[0]
            )
            .unwrap(),
            cell_type
        );
        assert_eq!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[1]
            )
            .unwrap(),
            other_type
        );
        let mut foreign_state = program.machine_states(&program.machines()[0])[0].clone();
        foreign_state.parameters = Default::default();
        assert!(declared_place_type(&program, &foreign_state, targets[0]).is_err());
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(members[0])
        else {
            panic!("member")
        };
        member.member_symbol = foreign_field;
        assert!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[0]
            )
            .unwrap_err()
            .message
            .contains("assignment field identity")
        );
        member_reset(&mut program, members[0], row_members.handle_at(1));
        let applied_field = program
            .data_application_definition(applications[0])
            .expect("retained actual application")
            .members
            .start();
        let DataMember::Field(field) = program.data_members.get_mut(applied_field) else {
            panic!("field")
        };
        let original = field.type_reference;
        field.type_reference =
            TypeReferenceHandle::from_parts(original.arena_index(), original.generation() + 1);
        let error = declared_place_type(
            &program,
            &program.machine_states(&program.machines()[0])[0],
            targets[0],
        )
        .unwrap_err();
        assert!(
            error.message.contains("collection field owner/type"),
            "{error:?}"
        );
        let DataMember::Field(field) = program.data_members.get_mut(applied_field) else {
            panic!("field")
        };
        field.type_reference = original;
        let array = program
            .type_reference_table
            .type_reference(original)
            .clone();
        let TypeReferenceNode::FixedArray {
            element_type,
            length,
        } = array
        else {
            panic!("array")
        };
        program.type_reference_table.substitute_node(
            original,
            TypeReferenceNode::FixedArray {
                element_type: TypeReferenceHandle::from_parts(
                    element_type.arena_index(),
                    element_type.generation() + 1,
                ),
                length: length.clone(),
            },
        );
        let error = declared_place_type(
            &program,
            &program.machine_states(&program.machines()[0])[0],
            targets[0],
        )
        .unwrap_err();
        assert!(
            error
                .message
                .contains("assignment selected type occurrence"),
            "{error:?}"
        );
        program.type_reference_table.substitute_node(
            original,
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            },
        );
        let mut duplicates = program
            .state_parameters(
                program
                    .machine_states(&program.machines()[0])
                    .first()
                    .unwrap(),
            )
            .to_vec();
        duplicates.push(duplicates[0].clone());
        let parameters = program.state_parameters.insert_many(duplicates);
        let machine = program.machines()[0].clone();
        program.machine_states_mut(&machine)[0].parameters = parameters;
        assert!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[0]
            )
            .unwrap_err()
            .message
            .contains("unique assignment storage declaration")
        );
        assert_eq!(
            declared_place_type(
                &program,
                &program.machine_states(&program.machines()[0])[0],
                targets[1]
            )
            .unwrap(),
            other_type
        );
    }

    fn member_reset(program: &mut TypedTrees, expression: ExpressionHandle, symbol: SymbolHandle) {
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression)
        else {
            panic!("member")
        };
        member.member_symbol = symbol;
    }

    use super::*;

    /// The rows the gate records on a copy of `program`.
    fn deferred(program: &TypedTrees) -> Result<Vec<(SymbolHandle, DeferredRequest)>, Diagnostic> {
        let mut program = program.clone();
        defer_unbound_calls(&mut program, 0, &[])?;
        Ok(program
            .deferred_refusals
            .iter()
            .map(|row| (row.declaration, row.request))
            .collect())
    }

    fn call(
        receiver: ExpressionHandle,
        target_symbol: SymbolHandle,
    ) -> typed_trees::expression::TableCallExpression {
        typed_trees::expression::TableCallExpression {
            runtime_call: None,
            authored_call_selection: None,
            authored_call_binding: None,
            build_selection: Default::default(),
            authored_machine_application: Default::default(),
            exact_coercion_target: Default::default(),
            requirement_application: Default::default(),
            receiver,
            target_symbol,
            static_machine_parameter: SymbolHandle::invalid(),
            target: typed_trees::name::Identifier::generated("call"),
            static_requirement_dispatch: None,
            machine_arguments: Box::new([]),
            quotient_operation: None,
            private_layout_operation: None,
            installed_interrupt_operation: None,
            layout_query: None,
            native_code_layout: None,
            arguments: Default::default(),
            evidence_arguments: Box::new([]),
            operational_acknowledgement: Default::default(),
        }
    }

    #[test]
    fn empty_call_and_transition_rosters_record_nothing() {
        assert_eq!(deferred(&TypedTrees::default()).unwrap(), Vec::new());
    }

    #[test]
    fn an_unbound_call_refuses_its_own_machine_and_leaves_the_others_served() {
        use typed_trees::expression::TableNamePath;
        use typed_trees::machine::{Machine, MachineBodyStatus};
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;
        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let machines = symbols.insert_children(
            root,
            [
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("served"),
                ),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("holder"),
                ),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("callee"),
                ),
            ],
        );
        let [served, holder, callee] = [0, 1, 2].map(|position| machines.handle_at(position));
        let states = [served, holder, callee].map(|machine| {
            symbols
                .insert_children(
                    machine,
                    [(SymbolKind::State, symbols::SymbolNameRef::Static("run"))],
                )
                .handle_at(0)
        });
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        // `served` calls `callee` by name; `holder` calls a method on a value
        // typing resolved to no declaration.
        let value = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath::default()));
        let bound = program
            .expression_table
            .insert(ExpressionNode::Call(call(Default::default(), states[2])));
        let unbound = program
            .expression_table
            .insert(ExpressionNode::Call(call(value, SymbolHandle::invalid())));
        for ((symbol, state), body) in
            [served, holder, callee]
                .into_iter()
                .zip(states)
                .zip([Some(bound), Some(unbound), None])
        {
            let statement_nodes = program
                .statement_table
                .insert_statements(body.map(StatementNode::Expression));
            let states = program.machine_states.insert_many([State {
                symbol: state,
                statement_nodes,
                ..Default::default()
            }]);
            program.push_machine(Machine {
                symbol,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
        }
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::ReceiverCallSelection)]
        );
        // The same call with no receiver is a name that resolves to nothing.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.receiver = Default::default();
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UnresolvedCallTarget)]
        );
        // The twin: once the call names a declared machine nothing is recorded.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target_symbol = states[2];
        assert_eq!(deferred(&program).unwrap(), Vec::new());
    }

    #[test]
    fn a_call_names_each_kind_of_declaration_only_in_its_own_call_form() {
        use symbols::{BuiltinFunction, builtin_function_symbols, builtin_type_symbols};
        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain([
                    (
                        SymbolKind::MathematicalDefinition,
                        symbols::SymbolNameRef::Static("Identity"),
                    ),
                    (
                        SymbolKind::MathematicalDefinition,
                        symbols::SymbolNameRef::Static("undeclared"),
                    ),
                    (
                        SymbolKind::MachineParameter,
                        symbols::SymbolNameRef::Static("Step"),
                    ),
                    (SymbolKind::Module, symbols::SymbolNameRef::Static("core")),
                    (SymbolKind::Local, symbols::SymbolNameRef::Static("value")),
                ]),
        );
        let declared = symbols::BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT;
        let [identity, undeclared, step, module, value] =
            [0, 1, 2, 3, 4].map(|offset| children.handles().nth(declared + offset).unwrap());
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        program.push_mathematical_definition(typed_trees::mathematical::MathematicalDefinition {
            symbol: identity,
            ..Default::default()
        });
        let embed = program
            .symbols
            .builtin_function_symbol(BuiltinFunction::IntegerEmbed)
            .unwrap();
        let targets = DeclaredCallTargets::new(&program);
        // A mathematical definition is applied by name, bare or qualified by
        // a module, and only where the program declares it.
        assert!(targets.declares(identity, None));
        assert!(targets.declares(identity, Some(module)));
        assert!(!targets.declares(identity, Some(value)));
        assert!(!targets.declares(undeclared, None));
        // A compiler operation has only its bare name.
        assert!(targets.declares(embed, None));
        assert!(!targets.declares(embed, Some(module)));
        // A callable parameter with no written contract is not yet callable.
        assert!(!targets.declares(step, None));
        assert_eq!(
            targets.unserved(step, CallReceiver::Absent),
            Some(DeferredRequest::UncallableCallTarget)
        );
        assert_eq!(
            targets.unserved(identity, CallReceiver::Named(value)),
            Some(DeferredRequest::ReceiverCallSelection)
        );
        assert_eq!(
            targets.unserved(identity, CallReceiver::Value),
            Some(DeferredRequest::ReceiverCallSelection)
        );
        assert_eq!(targets.unserved(embed, CallReceiver::Absent), None);
    }

    #[test]
    fn an_unbound_call_outside_every_declaration_still_refuses_the_program() {
        let mut program = TypedTrees::default();
        program.expression_table.insert(ExpressionNode::Call(call(
            Default::default(),
            SymbolHandle::invalid(),
        )));
        let error = deferred(&program).unwrap_err();
        assert!(error.message.starts_with("no product route yet:"));
        assert!(error.message.contains("outside every declaration"));
    }

    #[test]
    fn a_retained_requirement_needs_its_exact_signature_and_a_namespace_receiver() {
        use typed_trees::expression::TableNamePath;
        use typed_trees::signature::StateSignature;
        use typed_trees::trait_definition::TraitDefinition;

        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let owners = symbols.insert_children(
            root,
            [
                (SymbolKind::Trait, symbols::SymbolNameRef::Static("Api")),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("caller"),
                ),
            ],
        );
        let owner = owners.handle_at(0);
        let caller = owners.handle_at(1);
        let members = symbols.insert_children(
            owner,
            [
                (SymbolKind::State, symbols::SymbolNameRef::Static("call")),
                (SymbolKind::State, symbols::SymbolNameRef::Static("missing")),
            ],
        );
        let requirement = members.handle_at(0);
        let missing = members.handle_at(1);
        let receiver_value = symbols
            .insert_children(
                caller,
                [(SymbolKind::Local, symbols::SymbolNameRef::Static("value"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let signatures = program
            .trait_machine_signatures
            .insert_many([StateSignature {
                symbol: requirement,
                ..Default::default()
            }]);
        program.push_trait_definition(TraitDefinition {
            symbol: owner,
            is_boundary: true,
            machines: signatures,
            ..Default::default()
        });
        let receiver = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath::new(
                Default::default(),
                Default::default(),
                owner,
                owner,
            )));
        let expression = program
            .expression_table
            .insert(ExpressionNode::Call(call(receiver, requirement)));
        assert_eq!(
            deferred(&program).unwrap(),
            Vec::new(),
            "the retained requirement is a declared target"
        );
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("call expression")
        };
        call.target_symbol = missing;
        assert!(deferred(&program).is_err());
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("call expression")
        };
        call.target_symbol = requirement;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            panic!("receiver name")
        };
        path.symbol = receiver_value;
        path.head_symbol = receiver_value;
        assert!(deferred(&program).is_err());
    }
    #[test]
    fn bound_terminal_calls_keep_their_target_and_unbound_ones_refuse_their_machine() {
        use typed_trees::statement::{
            StatementNode, TableNamePath, TableTransition, TransitionTargetNode,
        };
        use typed_trees::{
            machine::{Machine, MachineBodyStatus},
            state::State,
        };
        let mut symbols = symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let machines = symbols.insert_children(
            root,
            [
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("caller"),
                ),
                (
                    SymbolKind::Machine,
                    symbols::SymbolNameRef::Static("callee"),
                ),
            ],
        );
        let caller = machines.handle_at(0);
        let callee = machines.handle_at(1);
        let caller_children = symbols.insert_children(
            caller,
            [
                (SymbolKind::State, symbols::SymbolNameRef::Static("entry")),
                (SymbolKind::Local, symbols::SymbolNameRef::Static("context")),
            ],
        );
        let state = caller_children.handle_at(0);
        let receiver = caller_children.handle_at(1);
        let target_state = symbols
            .insert_children(
                callee,
                [(SymbolKind::State, symbols::SymbolNameRef::Static("entry"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let mut members = Default::default();
        program.statement_table.push_name_path_member(
            &mut members,
            typed_trees::name::Identifier::generated("callee"),
        );
        let target =
            program
                .statement_table
                .insert_transition_target(TransitionTargetNode::Named {
                    path: TableNamePath {
                        symbol: callee,
                        head_symbol: callee,
                        members,
                        ..Default::default()
                    },
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    source_span: Default::default(),
                    authored_call_selection: None,
                    operational_acknowledgement: Default::default(),
                    static_machine_parameter: Default::default(),
                });
        let statements = program
            .statement_table
            .insert_statements([StatementNode::Transition(TableTransition {
                target,
                ..Default::default()
            })]);
        let states = program.machine_states.insert_many([State {
            symbol: state,
            statement_nodes: statements,
            ..Default::default()
        }]);
        let target_states = program.machine_states.insert_many([State {
            symbol: target_state,
            ..Default::default()
        }]);
        for (symbol, states) in [(caller, states), (callee, target_states)] {
            program.push_machine(Machine {
                symbol,
                states,
                body_is_present: true,
                body_status: MachineBodyStatus::Complete,
                ..Default::default()
            });
        }
        assert_eq!(
            deferred(&program).unwrap(),
            Vec::new(),
            "an exact declared terminal call is retained for judgment"
        );
        let mut unbound = program.clone();
        let TransitionTargetNode::Named { path, .. } =
            unbound.statement_table.transition_target_mut(target)
        else {
            panic!("named target")
        };
        path.symbol = SymbolHandle::invalid();
        assert_eq!(
            deferred(&unbound).unwrap(),
            vec![(caller, DeferredRequest::UnresolvedCallTarget)]
        );
        // A machine typed before this cohort is not examined again.
        let mut earlier = unbound.clone();
        defer_unbound_calls(&mut earlier, 2, &[]).unwrap();
        assert!(earlier.deferred_refusals.is_empty());
        let TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target_mut(target)
        else {
            panic!("named target")
        };
        path.receiver_symbol = receiver;
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(caller, DeferredRequest::ReceiverCallSelection)]
        );
    }
}

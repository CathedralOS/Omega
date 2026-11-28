/*
These queries read retained declaration identities and already formed types.
They establish no contract, value inhabitation, or body judgment. A missing
required type is unavailable rather than reconstructed with the retired checker.
*/
use crate::source_state::{
    TypedTrees,
    data::DataField,
    expression::{
        CollectionMeasure, ExpressionHandle, ExpressionNode, MemberSelection, TableNamePath,
        TableStructLiteral, UnaryOperator,
    },
    signature::StateParameter,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use diagnostics::Diagnostic;
use numeric_literals::FloatFormat;
use numerics::arithmetic::ArithmeticDomain;
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};

pub(crate) fn float_literal_atom(format: FloatFormat) -> BuiltinTypeAtom {
    match format {
        FloatFormat::F32 => BuiltinTypeAtom::F32,
        FloatFormat::F64 => BuiltinTypeAtom::F64,
    }
}

pub(crate) fn exact_declared_field(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&DataField> {
    program.unique_declared_field_by_symbol(symbol)
}

/// The parameters a call of `target` passes its arguments to, or `None` when
/// typing does not select them: a machine with no state, or a name that no
/// state, signature or operator answers alone. The caller records that on the
/// declaration holding the call rather than refusing the program.
pub(crate) fn call_target_parameters(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Result<Option<arena::RootRows<'_, StateParameter>>, Diagnostic> {
    if !program.symbols.symbols().nodes().is_valid(target) {
        return Ok(None);
    }
    if let Some(application) = program.closed_requirement_application(target) {
        return Ok(Some(
            program.state_signature_parameters(application.signature),
        ));
    }
    if let Some(state) = program.state_by_symbol(target) {
        return Ok(Some(program.state_parameters(state)));
    }
    if let Some(machine) = program.machine_by_symbol(target) {
        return Ok(program
            .machine_states(machine)
            .first()
            .map(|state| program.state_parameters(state)));
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target) {
        return Ok(Some(program.state_signature_parameters(signature)));
    }
    if let Some(contract) = program.proof_parameter_contract(target) {
        return Ok(Some(program.state_signature_parameters(contract)));
    }
    if let Some(operator) = program.operator_definition_by_symbol(target) {
        return Ok(Some(program.operator_parameters(operator)));
    }
    let mut signatures = program
        .traits()
        .iter()
        .flat_map(|declaration| program.trait_machine_signatures(declaration))
        .filter(|signature| signature.symbol == target);
    if let Some(signature) = signatures.next()
        && signatures.next().is_none()
    {
        return Ok(Some(program.state_signature_parameters(signature)));
    }
    if let Some(signature) = program
        .evidence_requirement_declaration(target)
        .map_err(Diagnostic::error)?
    {
        return Ok(Some(program.state_signature_parameters(signature)));
    }
    Ok(None)
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

// Selecting a field's type needs its applied declaration. The field owner
// may still be generic in the caller's binders, whose substitutions remain
// in its declared types. Concrete storage layout is checked separately.
fn projected_field_owner(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<&crate::source_state::data::DataDefinition> {
    program.closed_data_field_definition(reference).or_else(|| {
        let symbol = program.type_reference_table.type_head_symbol(reference)?;
        program.data_constructor_definition(reference, symbol)
    })
}

pub(crate) fn collection_field_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    symbol: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let definition = projected_field_owner(program, reference)
        .ok_or_else(|| crate::unavailable_judgment("applied collection field type"))?;
    let field = program
        .data_constructor_field(reference, definition.symbol, symbol)
        .ok_or_else(|| crate::unavailable_judgment("collection field owner/type"))?;
    Ok(field.type_reference)
}

// A selected builtin length has a plain count type; it is not a data field.
// Read only the retained member role and existing canonical type. This supplies
// no bounds, access or storage authority, and never resolves diagnostic spelling.
pub(crate) fn selected_member_value_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    selection: MemberSelection,
) -> Option<TypeReferenceHandle> {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return None;
    }
    match selection {
        MemberSelection::Field(symbol) => collection_field_type(program, reference, symbol).ok(),
        MemberSelection::CollectionMeasure(CollectionMeasure::Length) => {
            let receiver = program
                .type_reference_table
                .peeled_type_reference(reference);
            if !matches!(
                program.type_reference_table.type_reference(receiver),
                TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. }
            ) {
                return None;
            }
            let symbol = program.symbols.builtin_atom_symbol(BuiltinTypeAtom::U64)?;
            program
                .type_reference_table
                .find_arithmetic_result_type_reference(symbol, ArithmeticDomain::Exact)
        }
        MemberSelection::Unresolved
        | MemberSelection::CollectionMeasure(CollectionMeasure::Capacity) => None,
    }
}

/// Read a storage destination's declared type so its literal can retain the
/// same applied constructor. Bounds, case selection, access and establishment
/// remain obligations of the consuming lowered operation.
pub(crate) fn declared_place_type(
    program: &TypedTrees,
    state: &crate::source_state::state::State,
    target: ExpressionHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    declared_projection_type(program, target, |path| {
        if path.reserved_result_of.is_valid() {
            return Err(crate::unavailable_judgment(
                "reserved result is not storage",
            ));
        }
        let binder = path
            .resolved_symbol()
            .ok_or_else(|| crate::unavailable_judgment("assignment storage binder"))?;
        program
            .declared_binding_type_by_symbol(state, binder)
            .ok_or_else(|| crate::unavailable_judgment("unique assignment storage declaration"))
    })
}

/// A value parameter has the carrier declared by its enclosing
/// telescope. A sibling declaration's parameter cannot type this operand.
fn generic_value_type(
    program: &TypedTrees,
    mut scope: SymbolHandle,
    binder: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    use crate::source_state::data::TypeParameterKind;

    let nodes = program.symbols.symbols().nodes();
    if !nodes.is_valid(binder) || program.symbols.get(binder).kind != SymbolKind::TypeParameter {
        return None;
    }
    for _ in 0..nodes.len() {
        if !nodes.is_valid(scope) {
            break;
        }
        let parameters = program.declaration_type_parameters(scope);
        let mut matches = parameters
            .iter()
            .filter(|parameter| parameter.symbol == binder);
        if let Some(parameter) = matches.next() {
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                return None;
            };
            return (matches.next().is_none()
                && program
                    .type_reference_table
                    .contains_type_reference(type_reference))
            .then_some(type_reference);
        }
        scope = program.symbols.get(scope).parent;
    }
    None
}

/// Read the declared type of a value in its exact callable context. A
/// contract's reserved result is a value of its declared return type, never
/// writable storage or a name resolved again by spelling.
pub(crate) fn declared_callable_value_type(
    program: &TypedTrees,
    owner: SymbolHandle,
    target: ExpressionHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    if !program.symbols.symbols().nodes().is_valid(owner) {
        return Err(crate::unavailable_judgment("contract value owner"));
    }
    // Entry observation preserves the operand's complete declared type. This
    // is a value query, not a writable-place query or admission of an `old`
    // subject: the lowered reader still checks its input identity and phase.
    let table = &program.expression_table;
    if !table.expression_is_valid(target) {
        return Err(crate::unavailable_judgment("declared value expression"));
    }
    // A suffixed literal fixes its own carrier. Reading a receiving type here
    // could re-land it in a different carrier or silently drop its policy.
    let literal = if table.expression_is_valid(target) {
        match table.expression(target) {
            ExpressionNode::Integer(literal) => literal.landing().map(|landing| {
                (
                    crate::source_state::types::builtin_type_atom(landing.landed_type),
                    landing.domain,
                )
            }),
            ExpressionNode::Float(literal) => literal
                .landing()
                .map(|format| (float_literal_atom(format), ArithmeticDomain::Exact)),
            ExpressionNode::Boolean(_) => Some((BuiltinTypeAtom::Bool, ArithmeticDomain::Exact)),
            _ => None,
        }
    } else {
        None
    };
    if let Some((atom, policy)) = literal {
        let symbol = program
            .symbols
            .builtin_atom_symbol(atom)
            .ok_or_else(|| crate::unavailable_judgment("literal's retained carrier"))?;
        return program
            .type_reference_table
            .find_arithmetic_result_type_reference(symbol, policy)
            .ok_or_else(|| crate::unavailable_judgment("literal's retained qualified type"));
    }
    let mut target = target;
    for _ in 0..table.expression_count() {
        if !table.expression_is_valid(target) {
            break;
        }
        let crate::source_state::expression::ExpressionNode::Call(call) = table.expression(target)
        else {
            break;
        };
        if call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || !program
                .symbols
                .symbols()
                .nodes()
                .is_valid(call.target_symbol)
            || program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                != Some(symbols::BuiltinFunction::ContentOld)
        {
            break;
        }
        let [argument] = table.expression_handles(call.arguments) else {
            return Err(crate::unavailable_judgment("old input declared type arity"));
        };
        target = *argument;
    }
    if table.expression_is_valid(target)
        && let ExpressionNode::Name(path) = table.expression(target)
        && let Some(constant) = program.named_const_declaration(path)
    {
        return program
            .type_reference_table
            .contains_type_reference(constant.declared_type)
            .then_some(constant.declared_type)
            .ok_or_else(|| crate::unavailable_judgment("constant's declared type"));
    }
    let states = program
        .machine_by_symbol(owner)
        .map(|machine| program.machine_states(machine));
    let state = program.state_by_symbol(owner);
    // A structural callable alias's own clauses, and the copy a proof binder
    // retains, name the formals of that signature, as a trait machine's
    // clauses name its own. Only an owner that is no machine, state or trait
    // machine reads them, so a declaration keeps its own formals.
    let signature = program
        .trait_machine_signature(owner)
        .map(|(_, signature)| signature)
        .or_else(|| {
            if states.is_some() || state.is_some() {
                return None;
            }
            match program.symbols.get(owner).kind {
                symbols::SymbolKind::CallableContract => program
                    .runtime_callable_contracts
                    .iter()
                    .map(|(_, contract)| &contract.signature)
                    .find(|signature| signature.symbol == owner),
                symbols::SymbolKind::TypeParameter => program.proof_parameter_contract(owner),
                _ => None,
            }
        });
    if states.is_none() && state.is_none() && signature.is_none() {
        // A data declaration's own `requires` names its fields bare
        // (`requires (root as -Handle) != 0`), each a value of the type the
        // declaration gives it.
        let Some(definition) = program.unique_data_definition_by_symbol(owner) else {
            return Err(crate::unavailable_judgment("contract callable declaration"));
        };
        return declared_projection_type(program, target, |path| {
            if path.static_application.is_valid() || path.reserved_result_of.is_valid() {
                return Err(crate::unavailable_judgment("data clause value"));
            }
            if let Some(reference) = path
                .resolved_symbol()
                .and_then(|binder| generic_value_type(program, owner, binder))
            {
                return Ok(reference);
            }
            let field = path
                .resolved_symbol()
                .and_then(|binder| program.data_field(definition, binder))
                .ok_or_else(|| crate::unavailable_judgment("data clause field"))?;
            program
                .type_reference_table
                .contains_type_reference(field.type_reference)
                .then_some(field.type_reference)
                .ok_or_else(|| crate::unavailable_judgment("data clause field declared type"))
        });
    }
    declared_projection_type(program, target, |path| {
        if path.static_application.is_valid() {
            return Err(crate::unavailable_judgment("applied contract value"));
        }
        let reference = if path.reserved_result_of.is_valid() {
            if path.resolved_symbol().is_some()
                || program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
                || program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                    .iter()
                    .any(|symbol| symbol.is_valid())
            {
                return Err(crate::unavailable_judgment("reserved result identity"));
            }
            let result_owner = path.reserved_result_of;
            let mut results = states
                .into_iter()
                .flatten()
                .chain(state)
                .filter(|state| state.symbol == result_owner)
                .map(|state| state.return_type)
                .chain(
                    signature
                        .filter(|signature| signature.symbol == result_owner)
                        .map(|signature| signature.return_type),
                );
            let reference = results
                .next()
                .ok_or_else(|| crate::unavailable_judgment("result of another callable"))?;
            if results.next().is_some() {
                return Err(crate::unavailable_judgment(
                    "ambiguous contract result owner",
                ));
            }
            reference
        } else {
            let binder = path
                .resolved_symbol()
                .ok_or_else(|| crate::unavailable_judgment("contract value binder"))?;
            if !program.symbols.symbols().nodes().is_valid(binder) {
                return Err(crate::unavailable_judgment("contract value binder"));
            }
            let mut declarations = states
                .into_iter()
                .flatten()
                .chain(state)
                .filter_map(|state| program.declared_binding_type_by_symbol(state, binder))
                .chain(generic_value_type(program, owner, binder))
                .chain(signature.into_iter().flat_map(|signature| {
                    program
                        .state_signature_parameters(signature)
                        .iter()
                        .filter(move |parameter| parameter.symbol == binder)
                        .map(|parameter| parameter.type_reference)
                }));
            let reference = declarations
                .next()
                .ok_or_else(|| crate::unavailable_judgment("contract value declaration"))?;
            if declarations.next().is_some() {
                return Err(crate::unavailable_judgment(
                    "ambiguous contract value declaration",
                ));
            }
            reference
        };
        program
            .type_reference_table
            .contains_type_reference(reference)
            .then_some(reference)
            .ok_or_else(|| crate::unavailable_judgment("contract value declared type"))
    })
}

fn declared_projection_type(
    program: &TypedTrees,
    target: ExpressionHandle,
    mut root_type: impl FnMut(&TableNamePath) -> Result<TypeReferenceHandle, Diagnostic>,
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
                if names.is_empty() || (!selected.is_empty() && selected.len() != names.len()) {
                    return Err(crate::unavailable_judgment(
                        "assignment storage path roster",
                    ));
                }
                if let Some(binder) = path.resolved_symbol()
                    && selected
                        .first()
                        .is_some_and(|symbol| symbol.is_valid() && *symbol != binder)
                {
                    return Err(crate::unavailable_judgment(
                        "assignment storage binder identity",
                    ));
                }
                let mut reference = root_type(path)?;
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
                        ExpressionNode::Unary(unary)
                            if unary.operator == UnaryOperator::Dereference =>
                        {
                            let base = program
                                .type_reference_table
                                .unconstrained_type_reference(reference);
                            let TypeReferenceNode::Reference { referee, .. } =
                                program.type_reference_table.type_reference(base)
                            else {
                                return Err(crate::unavailable_judgment(
                                    "dereference of non-reference storage",
                                ));
                            };
                            *referee
                        }
                        ExpressionNode::Member(member) => declared_field_type(
                            program,
                            reference,
                            member.member.as_str(),
                            member.selection.field_symbol(),
                        )?,
                        ExpressionNode::Indexed(indexed)
                            if table.range_node(indexed.index).is_none() =>
                        {
                            collection_element_type(program, reference)?
                        }
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
            ExpressionNode::Member(member) => {
                spine.push(cursor);
                cursor = member.receiver;
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::Dereference => {
                spine.push(cursor);
                cursor = unary.operand;
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

pub(crate) fn declared_projection_field<'program>(
    program: &'program TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
    selected: SymbolHandle,
) -> Result<&'program DataField, Diagnostic> {
    let carrier = program
        .type_reference_table
        .peeled_type_reference(reference);
    let data = projected_field_owner(program, carrier)
        .ok_or_else(|| crate::unavailable_judgment("assignment field application"))?;
    let payload = if selected == SymbolHandle::invalid() {
        None
    } else {
        if !program.symbols.symbols().nodes().is_valid(selected)
            || program.symbols.get(selected).kind != SymbolKind::Field
        {
            return Err(crate::unavailable_judgment("assignment field identity"));
        }
        let parent = program.symbols.get(selected).parent;
        if program.symbols.get(parent).kind == SymbolKind::Variant {
            let case = program
                .data_constructor_case(carrier, data.symbol, parent)
                .ok_or_else(|| crate::unavailable_judgment("assignment field identity"))?;
            Some(program.data_payload_fields(case))
        } else {
            program
                .data_constructor_definition(carrier, parent)
                .ok_or_else(|| crate::unavailable_judgment("assignment field identity"))?;
            None
        }
    };
    // A case payload uses its exact selected namespace. Reading that
    // declaration supplies its type, not a fact that the case is active.
    let mut fields = program
        .data_members(data)
        .iter()
        .filter_map(crate::source_state::data::DataMember::as_field)
        .filter(|_| payload.is_none())
        .chain(
            payload
                .as_ref()
                .into_iter()
                .flat_map(|fields| fields.iter()),
        )
        .filter(|field| field.name.as_str() == name);
    let field = fields
        .next()
        .ok_or_else(|| crate::unavailable_judgment("assignment field declaration"))?;
    if fields.next().is_some() {
        return Err(crate::unavailable_judgment("assignment field identity"));
    }
    if selected != SymbolHandle::invalid() {
        let actual = program
            .data_constructor_field(carrier, data.symbol, selected)
            .ok_or_else(|| crate::unavailable_judgment("assignment field identity"))?;
        if actual.symbol != field.symbol {
            return Err(crate::unavailable_judgment("assignment field identity"));
        }
    }
    Ok(field)
}

pub(crate) fn declared_field_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
    selected: SymbolHandle,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let field = declared_projection_field(program, reference, name, selected)?;
    collection_field_type(program, reference, field.symbol)
}

use crate::source_state::deferred_refusals::DeferredRequest;
use crate::source_state::types::PrimitiveType;
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
                numeric_literals::IntegerLiteral::from_value(i64::from(*byte)),
            ))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    *program.expression_table.expression_mut(value) = ExpressionNode::ArrayLiteral(elements);
    program
        .expression_table
        .set_constructed_value_type_reference(value, carrier);
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
    parameters: arena::RootRows<'_, StateParameter>,
    result: TypeReferenceHandle,
    expression: ExpressionHandle,
    state_scope: Option<&crate::source_state::state::State>,
) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
    fn visit(
        program: &TypedTrees,
        parameters: arena::RootRows<'_, StateParameter>,
        result: TypeReferenceHandle,
        expression: ExpressionHandle,
        state_scope: Option<&crate::source_state::state::State>,
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
                selected_member_value_type(program, receiver, member.selection)
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
                None => {
                    return Err(crate::unavailable_judgment("saved value source type"));
                }
            },
            // The exchange's checked result is a structural outcome. Its receiver
            // is storage, so it cannot supply the result type of this call.
            ExpressionNode::Atomic(atomic) if atomic.exchange_operands.is_some() => None,
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
            // A block's type is its final value's.
            ExpressionNode::Block(block) if block.tail.is_valid() => {
                visit(program, parameters, result, block.tail, state_scope, active)?
            }
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
        if let Some(receiver) = receiver
            && receiver.is_valid()
            && program.symbols.symbols().nodes().is_valid(receiver)
            && matches!(
                program.static_record_bound_requirement(
                    program.symbols.get(receiver).parent,
                    receiver,
                    symbol,
                ),
                Ok(Some(_))
            )
        {
            return true;
        }
        if program.machine_by_own_or_state_symbol(symbol).is_some()
            || program.operator_definition_by_symbol(symbol).is_some()
            || program.trait_machine_signature(symbol).is_some()
        {
            return receiver.is_none_or(|receiver| static_namespace(program, receiver));
        }
        match program.symbols.get(symbol).kind {
            // Named mathematical definitions use their retained source row.
            // Sealed Identity has its own mathematical statement call form.
            // A module may qualify either; a value never does.
            SymbolKind::MathematicalDefinition => {
                return receiver.is_none_or(|receiver| static_namespace(program, receiver))
                    && match program.symbols.fixed_core_declaration_for_symbol(symbol) {
                        // Identity has a mathematical call form among the fixed
                        // declarations. Binding it judges neither its operands
                        // nor the statement it forms.
                        Some(symbols::FixedCoreDeclaration::Identity) => true,
                        Some(_) => false,
                        None => self
                            .mathematical_definitions
                            .binary_search(&crate::lowerer::symbol_key(symbol))
                            .is_ok(),
                    };
            }
            SymbolKind::Proposition => {
                return receiver.is_none_or(|receiver| static_namespace(program, receiver))
                    && program.proposition_definition_by_symbol(symbol).is_some();
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
            // A logical application keeps its proof binder and full contract;
            // identifying it grants no executable target or proven conclusion.
            SymbolKind::TypeParameter => {
                return receiver.is_none() && program.proof_parameter_contract(symbol).is_some();
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

    /// The same checked row identifies an inline body or an existing machine.
    /// Closing its original selection records that choice, not its validity.
    fn named_call(&self, target: SymbolHandle, receiver: CallReceiver) -> bool {
        let CallReceiver::Named(namespace) = receiver else {
            return false;
        };
        matches!(
            self.program.named_conformance_member(namespace, target),
            Ok(Some(_))
        ) || (namespace.is_valid()
            && self.program.symbols.symbols().nodes().is_valid(namespace)
            && matches!(
                self.program.static_record_bound_requirement(
                    self.program.symbols.get(namespace).parent,
                    namespace,
                    target,
                ),
                Ok(Some(_))
            ))
    }

    fn named_expression_call(
        &self,
        call: &crate::source_state::expression::TableCallExpression,
    ) -> bool {
        matches!(
            self.program.named_conformance_call_member(call),
            Ok(Some(_))
        ) || self
            .program
            .expression_table
            .name_path_node(call.receiver)
            .is_some_and(|receiver| {
                receiver.symbol.is_valid()
                    && self
                        .program
                        .symbols
                        .symbols()
                        .nodes()
                        .is_valid(receiver.symbol)
                    && matches!(
                        self.program.static_record_bound_requirement(
                            self.program.symbols.get(receiver.symbol).parent,
                            receiver.symbol,
                            call.target_symbol,
                        ),
                        Ok(Some(_))
                    )
            })
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

/// Closes exact named operation choices before recording calls typing left
/// without a declared target on their owning declaration. Selection closure
/// identifies a public requirement; it grants no body or contract judgment.
///
/// Keep a missing import's qualified name where ordinary lookup has already
/// ruled out a declaration. A receiver that denotes a value supplies no import.
fn unbound_call_path(
    result: crate::source_symbols::UnresolvedName,
    receiver_bound: bool,
    members: &[crate::source_state::name::Identifier],
    target: Option<&crate::source_state::name::Identifier>,
) -> Option<Box<str>> {
    if receiver_bound || result != crate::source_symbols::UnresolvedName::NoVisibleDeclaration {
        return None;
    }
    let mut names = members
        .iter()
        .chain(target)
        .flat_map(|name| name.as_str().split("::"));
    let (first, second) = (names.next()?, names.next()?);
    let mut path = String::from(first);
    for component in std::iter::once(second).chain(names) {
        path.push_str("::");
        path.push_str(component);
    }
    Some(path.into_boxed_str())
}

/// Typing sees the program with every bundled library and does not know what
/// an entry reaches, so such a call is not refused here: it is a row of the
/// declaration, raised by whoever demands that declaration. A call of a bare
/// name with no visible declaration is an error in every program and its row
/// says so (`DeferredRequest::UndeclaredCallName`); other unserved selections
/// stay requests. Resolution retains absence at each exact occurrence; typing
/// never asks unrelated declarations to reinterpret it. A missing value name
/// is the same error (`DeferredRequest::UndeclaredValueName`). A
/// terminal call is examined for the machines of this cohort only, as before.
/// A request in an expression no declaration owns has no reader to raise it
/// and still refuses here.
#[cfg(test)]
pub(crate) fn defer_unbound_calls(
    program: &mut TypedTrees,
    frontier: usize,
    completed: &[SymbolHandle],
) -> Result<(), Diagnostic> {
    let cohort = crate::lowerer::TypingCohort::for_machines(frontier, completed);
    let owners = crate::expressions::operand_owners::CohortOperandOwners::full(program);
    defer_unbound_calls_in_cohort(program, &cohort, &owners)
}

pub(crate) fn defer_unbound_calls_in_cohort(
    program: &mut TypedTrees,
    cohort: &crate::lowerer::TypingCohort<'_>,
    owners: &crate::expressions::operand_owners::CohortOperandOwners,
) -> Result<(), Diagnostic> {
    use crate::source_state::statement::TransitionTargetNode;
    let mut in_expressions = Vec::new();
    // Each row retains resolution's scoped outcome until its owner is known.
    let mut in_declarations = Vec::new();
    let mut closed_selections = Vec::new();
    let mut byte_domain_clauses;
    let rows: Vec<_>;
    {
        let program = &*program;
        let targets = DeclaredCallTargets::new(program);
        // These clauses already have the compiler's primitive byte meaning.
        // Keep every other unbound call for its ordinary declaration reader.
        byte_domain_clauses = program
            .domain_definitions()
            .iter()
            .skip(cohort.domain_definitions)
            .filter_map(|domain| {
                let predicate = crate::source_state::byte_predicates::primitive_byte_slice_domain(
                    program,
                    domain.symbol,
                )?;
                match program.proof_facts.span_or_empty(domain.facts).as_array() {
                    Some([crate::source_state::domain::ProofFact::Expression(expression)]) => {
                        Some((*expression, predicate))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        byte_domain_clauses
            .sort_unstable_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
        byte_domain_clauses.dedup();
        for (handle, expression) in program
            .expression_table
            .iter_expressions_from(cohort.typed_expressions + 1)
        {
            if let ExpressionNode::Name(path) = expression {
                if !path.symbol.is_valid()
                    && !path.reserved_result_of.is_valid()
                    && program.expression_table.unresolved_name(handle)
                        == crate::source_symbols::UnresolvedName::NoVisibleDeclaration
                {
                    in_expressions.push((
                        handle,
                        DeferredRequest::UndeclaredValueName,
                        crate::source_symbols::UnresolvedName::NoVisibleDeclaration,
                        None,
                    ));
                }
                continue;
            }
            let ExpressionNode::Call(call) = expression else {
                continue;
            };
            if byte_domain_clauses
                .binary_search_by_key(
                    &(handle.arena_index(), handle.generation()),
                    |(handle, _)| (handle.arena_index(), handle.generation()),
                )
                .is_ok()
            {
                continue;
            }
            if call.runtime_call.is_some() {
                program
                    .declared_callable_expression_type_reference(handle)
                    .map_err(Diagnostic::error)?;
                continue;
            }
            // `size_of::<T>()` and `align_of::<T>()` name no declaration by
            // construction: typing recognized the compiler-known query and
            // retained its operand, and Terminal Psi production reads that
            // request as a layout observation. It is not a call awaiting a
            // target.
            if call.layout_query.is_some() {
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
                let request = case_call(program, request, receiver, call.target.as_str());
                // A compiler operation the call was recognized as, or a
                // selection bound some other way, is not a bare name.
                let bare = if !call.static_machine_parameter.is_valid()
                    && call.static_requirement_dispatch.is_none()
                    && call.authored_call_binding.is_none()
                    && call.quotient_operation.is_none()
                    && call.private_layout_operation.is_none()
                    && call.installed_interrupt_operation.is_none()
                    && call.layout_query.is_none()
                    && call.native_code_layout.is_none()
                {
                    program.expression_table.unresolved_name(handle)
                } else {
                    Default::default()
                };
                let path = if !call.receiver.is_valid() {
                    unbound_call_path(bare, false, &[], Some(&call.target))
                } else if let ExpressionNode::Name(path) =
                    program.expression_table.expression(call.receiver)
                {
                    unbound_call_path(
                        bare,
                        path.head_symbol.is_valid() || path.symbol.is_valid(),
                        program.expression_table.name_path_members(path.members),
                        Some(&call.target),
                    )
                } else {
                    None
                };
                in_expressions.push((handle, request, bare, path));
            }
            if targets.named_expression_call(call)
                && let Some(occurrence) = call.authored_call_selection
            {
                closed_selections.push((occurrence, call.target_symbol));
            }
        }
        for (ordinal, machine) in program.machines().iter().enumerate() {
            if !cohort.owns_machine(ordinal, machine.symbol) {
                continue;
            }
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
                            let request =
                                case_call(program, request, receiver, call.target.as_str());
                            let bare = if call.installed_interrupt_operation.is_none()
                                && !call.static_machine_parameter.is_valid()
                            {
                                call.unresolved_name
                            } else {
                                Default::default()
                            };
                            let path = unbound_call_path(
                                bare,
                                call.receiver_root_symbol.is_valid()
                                    || call.receiver_symbol.is_valid(),
                                program.statement_table.name_path_members(call.receiver),
                                Some(&call.target),
                            );
                            in_declarations.push((
                                machine.symbol,
                                request,
                                call.source_span,
                                bare,
                                path,
                            ));
                        }
                        if targets.named_call(call.target_symbol, receiver)
                            && let Some(occurrence) = call.authored_call_selection
                        {
                            closed_selections.push((occurrence, call.target_symbol));
                        }
                    }
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for target in transition.targets() {
                        let TransitionTargetNode::Named {
                            path,
                            source_span,
                            static_machine_parameter,
                            ..
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
                            let bare = if !static_machine_parameter.is_valid() {
                                path.unresolved_name
                            } else {
                                Default::default()
                            };
                            let qualified = unbound_call_path(
                                bare,
                                path.head_symbol.is_valid() || path.receiver_symbol.is_valid(),
                                program.statement_table.name_path_members(path.members),
                                None,
                            );
                            in_declarations.push((
                                machine.symbol,
                                request,
                                *source_span,
                                bare,
                                qualified,
                            ));
                        }
                    }
                }
            }
        }
        if !in_expressions.is_empty() {
            for (handle, request, bare, path) in in_expressions {
                let source_span = program.expression_table.source_span(handle);
                let mut owned = false;
                for owner in owners
                    .expression_owners(handle)
                    .filter(|owner| owner.is_valid())
                {
                    owned = true;
                    in_declarations.push((owner, request, source_span, bare, path.clone()));
                }
                if !owned {
                    return Err(Diagnostic::error(format!(
                        "no product route yet: an expression outside every declaration is one {}",
                        request.construct()
                    ))
                    .with_source_span(source_span));
                }
            }
        }
        rows = in_declarations
            .into_iter()
            .map(|(declaration, request, source_span, bare, path)| {
                let undeclared = matches!(
                    request,
                    DeferredRequest::UnresolvedCallTarget | DeferredRequest::ReceiverCallSelection
                ) && bare
                    == crate::source_symbols::UnresolvedName::NoVisibleDeclaration;
                let request = if undeclared {
                    DeferredRequest::UndeclaredCallName
                } else {
                    request
                };
                (declaration, request, source_span, bare, path)
            })
            .collect();
    }
    for (expression, predicate) in byte_domain_clauses {
        complete_byte_domain_selection(program, expression, predicate)?;
    }
    if !closed_selections.is_empty() {
        use crate::source_state::{
            AuthoredDeclarationSelectionKind as Kind,
            AuthoredDeclarationSelectionLateBinding as Binding,
            AuthoredDeclarationSelectionTarget as Target,
        };
        for (occurrence, target) in closed_selections {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| {
                    Diagnostic::error("named operation call lost its authored selection")
                })?;
            let span = selection.source_span();
            if selection.kind() != Kind::Call {
                return Err(
                    Diagnostic::error("named operation call has a non-call selection")
                        .with_source_span(span),
                );
            }
            match selection.target() {
                Target::Resolved(actual) if actual.selected_symbol() == target => continue,
                Target::LateBound(Binding::CheckedCall) => {}
                _ => {
                    return Err(Diagnostic::error(
                        "named operation call has contradictory selection custody",
                    )
                    .with_source_span(span));
                }
            }
            program
                .finalize_late_bound_authored_declaration_selection(
                    occurrence,
                    Binding::CheckedCall,
                    target,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "named operation call lost its closed choice: {error:?}"
                    ))
                    .with_source_span(span)
                })?;
        }
    }
    for (declaration, request, source_span, lookup, path) in rows {
        if lookup == crate::source_symbols::UnresolvedName::NoVisibleDeclaration {
            program.record_missing_name(declaration, request, source_span, path);
        } else {
            program.record_deferred_refusal(declaration, request, source_span);
        }
    }
    Ok(())
}

/// Complete the source occurrence only after its exact domain clause has
/// acquired a primitive byte meaning. The ledger is output of that formation,
/// never the reason an unresolved call is treated as a primitive.
fn complete_byte_domain_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
    predicate: crate::source_state::byte_predicates::ByteSequencePredicate,
) -> Result<(), Diagnostic> {
    use crate::source_state::AuthoredDeclarationSelectionKind as Kind;
    let span = program.expression_table.source_span(expression);
    let fail = |reason: &str| {
        Diagnostic::error(format!(
            "primitive byte-domain call {expression:?}: {reason}"
        ))
        .with_source_span(span)
    };
    let call = program
        .expression_table
        .call_node(expression)
        .ok_or_else(|| fail("lost its formed call"))?;
    let mut occurrence = None;
    for attached in program
        .expression_table
        .authored_selection_occurrences(expression)
    {
        let row = program
            .authored_declaration_selections()
            .get(attached)
            .ok_or_else(|| fail("lost its attached authored occurrence"))?;
        if row.kind() != Kind::Call {
            continue;
        }
        if occurrence
            .replace(attached)
            .is_some_and(|previous| previous != attached)
        {
            return Err(fail("retains multiple authored Call occurrences"));
        }
    }
    if call.authored_call_selection != occurrence {
        return Err(fail("does not retain its own attached Call occurrence"));
    }
    // The primitive meaning belongs to the authenticated declaration, not an
    // unbound spelling. The ordinary resolved Call row remains its custody;
    // rewriting it to a synthetic intrinsic would erase the actual selection.
    let Some((actual, _)) =
        crate::source_state::byte_predicates::byte_predicate_call(program, expression)
    else {
        return Err(fail("changes its selected primitive or declaration"));
    };
    if actual != predicate {
        return Err(fail("changes its selected primitive or declaration"));
    }
    // byte_predicate_call checked this exact occurrence's Resolved declaration
    // against the sealed Core schema. Unknown or late-bound rows cannot acquire
    // primitive meaning here; ordinary selection must complete them first.
    Ok(())
}

/// A call typing left without a declared target, written `Sum::Case(..)`:
/// its qualifier is a data declaration and its name one of that
/// declaration's cases. That is an error in every program
/// (`DeferredRequest::CaseCalledAsMachine`); any other request is returned
/// as it came.
fn case_call(
    program: &TypedTrees,
    request: DeferredRequest,
    receiver: CallReceiver,
    written: &str,
) -> DeferredRequest {
    let CallReceiver::Named(owner) = receiver else {
        return request;
    };
    let names_case = matches!(
        request,
        DeferredRequest::UnresolvedCallTarget | DeferredRequest::UncallableCallTarget
    ) && owner.is_valid()
        && program.symbols.get(owner).kind == SymbolKind::Data
        && program
            .symbols
            .find_child_by_name_and_kind(owner, written, SymbolKind::Variant)
            .is_some();
    if names_case {
        DeferredRequest::CaseCalledAsMachine
    } else {
        request
    }
}

/*
    The declared type of an operand, for the forms whose type is written in a
    declaration or follows from one: a place (a parameter, an annotated local,
    a field or element of one), a cast, and builtin arithmetic over plain
    operands. `None` is "typing does not read this form", never "no domains":
    the caller defers the run. The callable-value query also retains the
    compiler's `old` projection. Exact call returns are read below; matches
    and unannotated locals are not read here. The checker of the lowered form
    types every expression and re-checks each entry against the operand it finds.
*/
pub(crate) fn declared_expression_type(
    program: &TypedTrees,
    owner: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    DeclaredExpressionTypes::default().get(program, owner, expression)
}

/// Scratch for one immutable typing frontier. Reuse it across operator sites so
/// nested operand trees are read once per actual owner, not once per parent.
#[derive(Default)]
pub(crate) struct DeclaredExpressionTypes {
    rows: std::collections::HashMap<
        (SymbolHandle, ExpressionHandle),
        Option<TypeReferenceHandle>,
        arena::BuildIndexHasher,
    >,
}

impl DeclaredExpressionTypes {
    pub(crate) fn get(
        &mut self,
        program: &TypedTrees,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let key = (owner, expression);
        if let Some(result) = self.rows.get(&key) {
            return *result;
        }
        // A malformed cyclic expression yields no type; it cannot recurse into
        // invented operand information. Storage grows only with queried pairs.
        self.rows.insert(key, None);
        let result = self.read(program, owner, expression);
        self.rows.insert(key, result);
        result
    }

    /// Selection supplies the exact declared return type of an operator
    /// declaration that needs no static application, before an enclosing cast
    /// is accounted. No contract or membership evidence is inferred.
    fn selected_operator_result(
        &mut self,
        program: &TypedTrees,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let span = program.expression_table.source_span(expression);
        for form in crate::expressions::operator_calls::forms(program, expression)
            .into_iter()
            .flatten()
        {
            let mut operands = Vec::with_capacity(form.operands().len());
            for operand in form.operands() {
                operands.push(
                    if crate::source_state::closed_numeric::has_anonymous_numeric_results(
                        program, *operand,
                    ) {
                        None
                    } else {
                        Some(self.get(program, owner, *operand)?)
                    },
                );
            }
            let Some(target) = crate::expressions::operator_calls::selection_for_operands(
                program, form, &operands, span,
            )
            .ok()
            .flatten() else {
                continue;
            };
            if !target.arguments.is_empty() {
                return None;
            }
            return program
                .machine_by_symbol(target.target)
                .and_then(|machine| program.machine_states(machine).first())
                .map(|state| state.return_type)
                .filter(|reference| {
                    program
                        .type_reference_table
                        .contains_type_reference(*reference)
                });
        }
        None
    }

    fn read(
        &mut self,
        program: &TypedTrees,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        let table = &program.expression_table;
        if !table.expression_is_valid(expression) {
            return None;
        }
        let retained = table.constructed_value_type_reference(expression);
        if program
            .type_reference_table
            .contains_type_reference(retained)
        {
            return Some(retained);
        }
        // A domain declaration binds its own self to the declared carrier.
        // Its refinement facts can therefore select indexed parents before
        // any machine binds or supplies a value of this domain.
        if matches!(table.expression(expression), ExpressionNode::Name(_))
            && let Some(domain) = program.domain_definition(owner)
            && crate::source_state::domain::domain_subject_reference(program, domain, expression)
        {
            return Some(domain.target_type);
        }
        // A requirement's namespace receiver is not a value argument. Read its
        // exact public signature before a surrounding cast accounts for the
        // result's domains; the receiving type cannot supply that information.
        if let ExpressionNode::Call(call) = table.expression(expression)
            && call.selects_only_nominal_route()
            && table.expression_is_valid(call.receiver)
            && let Ok(signature) = program.source_call_requirement_signature(
                owner,
                crate::source_state::references::StaticArgumentCall::Expression(expression, call),
            )
        {
            return program
                .type_reference_table
                .contains_type_reference(signature.return_type)
                .then_some(signature.return_type);
        }
        if let ExpressionNode::Call(call) = table.expression(expression)
            && call.selects_only_nominal_route()
            && !call.receiver.is_valid()
            && call.machine_arguments.is_empty()
            && call.evidence_arguments.is_empty()
            && program
                .symbols
                .symbols()
                .nodes()
                .is_valid(call.target_symbol)
            && program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                != Some(symbols::BuiltinFunction::ContentOld)
        {
            let result = program
                .state_by_symbol(call.target_symbol)
                .map(|state| state.return_type)
                .or_else(|| {
                    program
                        .machine_by_symbol(call.target_symbol)
                        .and_then(|machine| {
                            program
                                .machine_states(machine)
                                .first()
                                .map(|state| state.return_type)
                        })
                })
                .or_else(|| {
                    program
                        .trait_machine_signature(call.target_symbol)
                        .map(|(_, signature)| signature.return_type)
                })?;
            return program
                .type_reference_table
                .contains_type_reference(result)
                .then_some(result);
        }
        match table.expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                self.get(program, owner, program.saved_value_source(saved)?)
            }
            ExpressionNode::Member(member) => {
                let receiver = self.get(program, owner, member.receiver)?;
                selected_member_value_type(program, receiver, member.selection)
            }
            // A value `match` has the one type all its arms have. Arms that
            // disagree give no type here; the receiving judgment reports
            // them.
            ExpressionNode::Match(dispatch) => {
                let mut joined: Option<TypeReferenceHandle> = None;
                for arm in table.match_arms(dispatch.arms) {
                    let candidate = self.get(program, owner, arm.value)?;
                    if joined.is_some_and(|previous| {
                        program.normalized_type_identity(previous)
                            != program.normalized_type_identity(candidate)
                    }) {
                        return None;
                    }
                    joined.get_or_insert(candidate);
                }
                joined
            }
            ExpressionNode::Indexed(_)
                if let Some(result) = self.selected_operator_result(program, owner, expression) =>
            {
                Some(result)
            }
            ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Call(_) => {
                declared_callable_value_type(program, owner, expression).ok()
            }
            // An accounted entry and a settled cast carry their result. A plain
            // carrier cast results in the carrier it names.
            ExpressionNode::Cast(cast) if cast.result_type.is_valid() => Some(cast.result_type),
            ExpressionNode::Cast(cast)
                if cast.form == language_core::CastForm::Value
                    && cast.domain == ArithmeticDomain::Exact
                    && cast.semantic_domain.is_empty()
                    && cast.target_type.is_valid() =>
            {
                Some(cast.target_type)
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::Dereference => {
                let operand = self.get(program, owner, unary.operand)?;
                let bare = program
                    .type_reference_table
                    .unconstrained_type_reference(operand);
                match program.type_reference_table.type_reference(bare) {
                    TypeReferenceNode::Reference { referee, .. } => Some(*referee),
                    _ => None,
                }
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::BitwiseNot => {
                let operand = self.get(program, owner, unary.operand)?;
                crate::source_state::operator::core_integer_operand(program, operand)
                    .map(|_| operand)
            }
            // Selection supplies the exact declared return type before an enclosing
            // cast is accounted. No contract or membership evidence is inferred.
            ExpressionNode::Binary(binary) if binary.operator.is_arithmetic() => {
                let mut operands = Vec::with_capacity(2);
                for operand in [binary.left, binary.right] {
                    operands.push(
                        if crate::source_state::closed_numeric::has_anonymous_numeric_results(
                            program, operand,
                        ) {
                            None
                        } else {
                            Some(self.get(program, owner, operand)?)
                        },
                    );
                }
                if let Some(result) = self.selected_operator_result(program, owner, expression) {
                    return Some(result);
                }
                let [left, right] = operands.as_slice() else {
                    unreachable!("binary operator has two operands")
                };
                if let Some(result) = crate::source_state::operator::core_integer_result(
                    program,
                    binary.operator,
                    binary.policy,
                    *left,
                    *right,
                ) {
                    return Some(result);
                }
                // Keep the existing bare float and open-carrier queries.
                // A suffixed policy operation returns the same bare carrier; its
                // validity and arithmetic policy remain the checker's obligations.
                let mut result = None;
                for reference in operands.iter().flatten() {
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(*reference)
                    else {
                        return None;
                    };
                    let kind = program.symbols.get(*symbol).kind;
                    // A suffixed primitive fixes its result to the operand
                    // carrier even before a generic carrier is applied. This
                    // type query does not establish that its arithmetic is valid.
                    if !(kind == SymbolKind::BuiltinType
                        || (binary.policy != ArithmeticDomain::Exact
                            && kind == SymbolKind::TypeParameter))
                        || result.is_some_and(|earlier| earlier != *symbol)
                    {
                        return None;
                    }
                    result = Some(*symbol);
                }
                result.and_then(|_| operands.into_iter().flatten().next())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_computed_array_index_keeps_the_const_parameters_integer_type() {
        use crate::source_state::{expression::ExpressionNode, statement::StatementNode};
        let resolved = crate::resolve_fixture(
            "machine last<const Count: u64>(bytes: @[u8; Count]) -> u8 requires Count > 0 { bytes[Count - 1] }",
        );
        let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
        let state = &program.machine_states(&program.machines()[0])[0];
        let indices = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                (local.is_operand_capture
                    && matches!(
                        program.expression_table.expression(local.initial_value),
                        ExpressionNode::Binary(_)
                    ))
                .then_some(local.type_reference)
            })
            .collect::<Vec<_>>();
        let [reference] = indices.as_slice() else {
            panic!("expected one computed index")
        };
        assert_eq!(
            program.primitive_type_reference(*reference),
            Some(crate::source_state::types::PrimitiveType::U64)
        );
    }

    #[test]
    fn exact_constructor_fields_keep_case_and_constructor_owners() {
        use crate::source_state::data::{DataDefinition, DataField, DataMember, DataVariant};
        use crate::source_symbols::SymbolTableBuilder;
        use symbols::SymbolNameRef;
        let mut symbols = SymbolTableBuilder::default();
        let owner = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Choice"));
        let foreign = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Foreign"));
        let cases = symbols.insert_children(
            owner,
            [
                (SymbolKind::Variant, SymbolNameRef::Static("First")),
                (SymbolKind::Variant, SymbolNameRef::Static("Second")),
            ],
        );
        let field = symbols
            .insert_children(
                cases.start(),
                [(SymbolKind::Field, SymbolNameRef::Static("value"))],
            )
            .start();
        let foreign_field = symbols
            .insert_children(
                foreign,
                [(SymbolKind::Field, SymbolNameRef::Static("value"))],
            )
            .start();
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let payload = program.data_payload_fields.insert_many([DataField {
            symbol: field,
            type_reference: reference,
            ..Default::default()
        }]);
        let members = program.data_members.insert_many([
            DataMember::Variant(DataVariant {
                symbol: cases.start(),
                payload,
                ..Default::default()
            }),
            DataMember::Variant(DataVariant {
                symbol: cases.handle_at(1),
                ..Default::default()
            }),
        ]);
        program.push_data_definition(DataDefinition {
            symbol: owner,
            members,
            ..Default::default()
        });
        let members = program
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: foreign_field,
                type_reference: reference,
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: foreign,
            members,
            ..Default::default()
        });
        let mut literal = TableStructLiteral {
            type_symbol: owner,
            case_symbol: Some(cases.start()),
            ..Default::default()
        };
        let expression = program
            .expression_table
            .insert(ExpressionNode::StructLiteral(literal.clone()));
        assert_eq!(
            selected_construction_field_type(&program, expression, &literal, field).unwrap(),
            reference
        );
        assert!(
            selected_construction_field_type(&program, expression, &literal, foreign_field)
                .unwrap_err()
                .message
                .contains("constructor field owner")
        );
        literal.case_symbol = Some(cases.handle_at(1));
        assert!(
            selected_construction_field_type(&program, expression, &literal, field)
                .unwrap_err()
                .message
                .contains("constructor case identity")
        );
        literal.case_symbol = Some(cases.start());
        let snapshot = program.clone();
        program.data_payload_fields.get_mut(payload.start()).symbol = foreign_field;
        assert!(exact_declared_field(&program, field).is_none());
        assert!(exact_declared_field(&program, foreign_field).is_none());
        assert_eq!(
            selected_construction_field_type(&snapshot, expression, &literal, field).unwrap(),
            reference
        );
    }

    #[test]
    fn assignment_places_keep_applied_fields_and_refuse_foreign_or_duplicate_binders() {
        use crate::source_state::{
            data::{DataDefinition, DataField, DataMember, TypeParameter},
            expression::{TableIndexedExpression, TableMemberExpression, TableNamePath},
            machine::Machine,
            name::Identifier,
            state::State,
            types::FixedArrayLength,
        };
        use crate::source_symbols::SymbolTableBuilder;
        use symbols::SymbolNameRef;
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
        typed_trees::data_applications::prepare_from(&mut program, 0)
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
                        selection: crate::source_state::expression::MemberSelection::Field(
                            row_members.handle_at(1),
                        ),
                        member: Identifier::generated("items"),
                        case_variant: None,
                    }));
            let selector = program.expression_table.insert(ExpressionNode::Integer(
                numeric_literals::IntegerLiteral::from_value(1),
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
        member.selection = crate::source_state::expression::MemberSelection::Field(foreign_field);
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
        let stale = SymbolHandle::from_parts(foreign_field.arena_index(), u32::MAX);
        assert!(!program.symbols.symbols().nodes().is_valid(stale));
        member_reset(&mut program, members[0], stale);
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
        assert_eq!(
            error.message,
            "typing could not complete the check (assignment field identity)"
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
        program
            .machine_states
            .get_mut(machine.states.handle_at(0))
            .parameters = parameters;
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
        member.selection = crate::source_state::expression::MemberSelection::Field(symbol);
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

    #[test]
    fn applied_projection_accepts_only_its_original_or_actual_field() {
        let program = crate::qualified_operator_home_tests::checked(&[(
            "main.omg",
            r#"
            data Buffer<T> { storage: [T; 4]; }
            data Foreign { storage: [u8; 4]; }
            machine bytes(buffer: @Buffer<u8>) -> u8 { buffer.storage[0u64] }
        "#,
        )])
        .unwrap();
        let declaration = program
            .machines()
            .iter()
            .find(|row| row.name.as_str() == "bytes")
            .unwrap();
        let state = program.machine_states(declaration).first().unwrap();
        let reference = program.state_parameters(state)[0].type_reference;
        let carrier = program
            .type_reference_table
            .peeled_type_reference(reference);
        let applied = program.closed_data_field_definition(carrier).unwrap();
        let field = |data| {
            program
                .data_members(data)
                .iter()
                .filter_map(crate::source_state::data::DataMember::as_field)
                .next()
                .unwrap()
        };
        let actual = field(applied);
        let template = program
            .data_definitions()
            .iter()
            .find(|row| row.name.as_str() == "Buffer" && row.generic_instance.is_none())
            .unwrap();
        let original = field(template);
        let TypeReferenceNode::FixedArray {
            element_type,
            length,
        } = program
            .type_reference_table
            .type_reference(actual.type_reference)
        else {
            panic!("the actual generic field has a closed array carrier");
        };
        assert_eq!(
            program.primitive_type_reference(*element_type),
            Some(PrimitiveType::U8)
        );
        assert_eq!(
            *length,
            crate::source_state::types::FixedArrayLength::Literal(4)
        );
        for selected in [original.symbol, actual.symbol, SymbolHandle::invalid()] {
            assert_eq!(
                declared_field_type(&program, reference, "storage", selected).unwrap(),
                actual.type_reference
            );
        }
        let foreign = program
            .data_definitions()
            .iter()
            .find(|row| row.name.as_str() == "Foreign")
            .unwrap();
        for selected in [
            field(foreign).symbol,
            SymbolHandle::from_parts(1, 0),
            SymbolHandle::from_parts(0, 1),
            SymbolHandle::from_parts(actual.symbol.arena_index(), actual.symbol.generation() + 1),
        ] {
            assert!(
                declared_field_type(&program, reference, "storage", selected).is_err(),
                "an unrelated or malformed field cannot supply the applied projection"
            );
        }
    }

    #[test]
    fn primitive_byte_domain_calls_keep_their_exact_builtin_meaning() {
        use crate::source_state::byte_predicates::{
            ByteSequencePredicate, primitive_byte_slice_domain,
        };
        use crate::source_state::domain::ProofFact;
        use crate::source_state::expression::TableCallExpression;

        fn typed(text: &str, core: bool) -> TypedTrees {
            let mut sources = crate::source_inputs::SourceMap::default();
            let source = sources
                .add_with_metadata(
                    "package/main.omg".into(),
                    text.to_owned(),
                    "package".into(),
                    None,
                    source_admission::SourceOrigin::User,
                )
                .source_id;
            let tokens = crate::frontend_fixture::Lexer::new(text)
                .tokenize()
                .unwrap();
            let mut syntax =
                crate::frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
            let mut bindings = Vec::new();
            if core {
                let text = include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../../source/library/core/byte_observation/byte_observation.omg"
                ));
                let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../../../../source/library/core");
                let declaration = sources
                    .add_with_metadata(
                        root.join("byte_observation/byte_observation.omg"),
                        text.to_owned(),
                        root,
                        None,
                        source_admission::SourceOrigin::Toolchain,
                    )
                    .source_id;
                let tokens = crate::frontend_fixture::Lexer::new(text)
                    .tokenize()
                    .unwrap();
                crate::frontend_fixture::parse(&mut syntax, declaration, &tokens).unwrap();
                bindings.push(
                    crate::source_symbols::SourceScopedTopLevelBinding::file_import(
                        source,
                        declaration,
                        "byte_observation",
                    ),
                );
            }
            let mut request = crate::resolution::ResolutionRequest::new(syntax);
            request.sources = Some(std::sync::Arc::new(sources));
            request.top_level_bindings = bindings;
            let resolved = crate::resolution::resolve(request).unwrap();
            crate::lower_symbol_resolved_trees(&resolved).unwrap()
        }

        fn call_mut(
            program: &mut TypedTrees,
            expression: ExpressionHandle,
        ) -> &mut TableCallExpression {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression)
            else {
                panic!("the domain retains its actual call")
            };
            call
        }

        let program = typed(
            "use {omega}/language/core/byte_observation/byte_observation.omg;\npublic domain [u8]::Path requires |byte_observation|::no_nul(@self[..]);\ndomain [u8]::Ascii requires |byte_observation|::ascii_only(@self[..]);",
            true,
        );
        let domain = &program.domain_definitions()[0];
        let symbol = domain.symbol;
        let Some([ProofFact::Expression(expression)]) =
            program.proof_facts.span_or_empty(domain.facts).as_array()
        else {
            panic!("one authored predicate")
        };
        let expression = *expression;
        assert_eq!(
            primitive_byte_slice_domain(&program, symbol),
            Some(ByteSequencePredicate::NoNul)
        );
        let original = program
            .expression_table
            .call_node(expression)
            .unwrap()
            .clone();
        let occurrence = original
            .authored_call_selection
            .expect("actual authored Call");
        let selected = program
            .authored_declaration_selections()
            .get(occurrence)
            .unwrap()
            .target();
        assert!(matches!(
            selected,
            crate::source_state::AuthoredDeclarationSelectionTarget::Resolved(_)
        ));
        assert!(deferred(&program).unwrap().is_empty());
        let completed = program.clone();
        complete_byte_domain_selection(&completed, expression, ByteSequencePredicate::NoNul)
            .unwrap();
        assert_eq!(
            completed
                .authored_declaration_selections()
                .get(occurrence)
                .unwrap()
                .target(),
            selected,
            "a genuine declaration keeps its ordinary resolved custody"
        );
        assert!(
            complete_byte_domain_selection(
                &completed,
                expression,
                ByteSequencePredicate::AsciiOnly
            )
            .is_err(),
            "the authenticated no-nul observation cannot become ASCII"
        );

        let other_domain = &program.domain_definitions()[1];
        let Some([ProofFact::Expression(other_expression)]) = program
            .proof_facts
            .span_or_empty(other_domain.facts)
            .as_array()
        else {
            panic!("one sibling predicate")
        };
        let other = program
            .expression_table
            .call_node(*other_expression)
            .unwrap();
        let mut wrong = program.clone();
        call_mut(&mut wrong, expression).target_symbol = other.target_symbol;
        assert_eq!(primitive_byte_slice_domain(&wrong, symbol), None);
        assert!(
            complete_byte_domain_selection(&wrong, expression, ByteSequencePredicate::NoNul)
                .is_err()
        );
        for attached in [None, other.authored_call_selection] {
            let mut wrong = program.clone();
            call_mut(&mut wrong, expression).authored_call_selection = attached;
            assert!(
                complete_byte_domain_selection(&wrong, expression, ByteSequencePredicate::NoNul)
                    .unwrap_err()
                    .message
                    .contains("own attached Call occurrence")
            );
        }

        for (index, generation) in [(0, 1), (1, 0)] {
            for (case, malformed) in [
                TableCallExpression {
                    target_symbol: SymbolHandle::from_parts(index, generation),
                    ..original.clone()
                },
                TableCallExpression {
                    receiver: ExpressionHandle::from_parts(index, generation),
                    ..original.clone()
                },
                TableCallExpression {
                    build_selection: arena::Handle::from_parts(index, generation),
                    ..original.clone()
                },
                TableCallExpression {
                    authored_machine_application: arena::Handle::from_parts(index, generation),
                    ..original.clone()
                },
                TableCallExpression {
                    exact_coercion_target: TypeReferenceHandle::from_parts(index, generation),
                    ..original.clone()
                },
                TableCallExpression {
                    requirement_application: TypeReferenceHandle::from_parts(index, generation),
                    ..original.clone()
                },
                TableCallExpression {
                    static_machine_parameter: SymbolHandle::from_parts(index, generation),
                    ..original.clone()
                },
            ]
            .into_iter()
            .enumerate()
            {
                let mut wrong = program.clone();
                *call_mut(&mut wrong, expression) = malformed;
                assert_eq!(
                    primitive_byte_slice_domain(&wrong, symbol),
                    None,
                    "present malformed metadata is not exact absence: handle=({index},{generation}), field={case}"
                );
                assert!(
                    complete_byte_domain_selection(
                        &wrong,
                        expression,
                        ByteSequencePredicate::NoNul
                    )
                    .is_err()
                );
            }
        }
        let mut computed = program.clone();
        let callee = computed
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        call_mut(&mut computed, expression).runtime_call =
            Some(crate::source_state::callable_values::RuntimeCallBinding {
                callee,
                contract: Default::default(),
            });
        assert_eq!(primitive_byte_slice_domain(&computed, symbol), None);
        assert!(
            complete_byte_domain_selection(&computed, expression, ByteSequencePredicate::NoNul)
                .is_err()
        );

        let mut malformed = program.clone();
        call_mut(&mut malformed, expression).arguments = Default::default();
        assert_eq!(primitive_byte_slice_domain(&malformed, symbol), None);
        assert!(
            complete_byte_domain_selection(&malformed, expression, ByteSequencePredicate::NoNul)
                .is_err()
        );
        let mut malformed = program.clone();
        call_mut(&mut malformed, expression).machine_arguments =
            Box::new([crate::source_state::expression::StaticMachineArgument {
                type_reference: domain.target_type,
                ..Default::default()
            }]);
        assert_eq!(primitive_byte_slice_domain(&malformed, symbol), None);
        let mut malformed = program.clone();
        let (_, argument) =
            crate::source_state::byte_predicates::byte_predicate_call(&program, expression)
                .unwrap();
        let arguments =
            malformed
                .expression_table
                .insert_expression_handles([ExpressionHandle::from_parts(
                    argument.arena_index(),
                    argument.generation() + 1,
                )]);
        call_mut(&mut malformed, expression).arguments = arguments;
        assert_eq!(primitive_byte_slice_domain(&malformed, symbol), None);
        let mut wrong_carrier = program.clone();
        let slice = wrong_carrier.type_reference_table.insert(
            program
                .type_reference_table
                .type_reference(domain.target_type)
                .clone(),
        );
        wrong_carrier.type_reference_table.substitute_node(
            domain.target_type,
            TypeReferenceNode::Reference {
                referee: slice,
                access: language_core::ReferenceAccess::WriteOnly,
                lifetime: None,
            },
        );
        assert_eq!(primitive_byte_slice_domain(&wrong_carrier, symbol), None);

        let bare = typed("domain [u8]::Path requires no_nul(self);", false);
        let bare_domain = bare.domain_definitions()[0].symbol;
        assert_eq!(
            primitive_byte_slice_domain(&bare, bare_domain),
            None,
            "an undeclared spelling has no primitive privilege"
        );
        assert_eq!(
            deferred(&bare).unwrap(),
            vec![(bare_domain, DeferredRequest::UndeclaredCallName)]
        );
        let ordinary = typed(
            "machine no_nul(value: @[u8]) -> bool { true }\ndomain [u8]::Path requires no_nul(@self[..]);",
            false,
        );
        let ordinary_domain = ordinary.domain_definitions()[0].symbol;
        assert_eq!(
            primitive_byte_slice_domain(&ordinary, ordinary_domain),
            None,
            "a same-spelled ordinary body does not become a Core observation"
        );
        assert!(deferred(&ordinary).unwrap().is_empty());
    }

    fn call(
        receiver: ExpressionHandle,
        target_symbol: SymbolHandle,
    ) -> crate::source_state::expression::TableCallExpression {
        crate::source_state::expression::TableCallExpression {
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
            target: crate::source_state::name::Identifier::generated("call"),
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
        use crate::source_state::expression::TableNamePath;
        use crate::source_state::machine::{Machine, MachineBodyStatus};
        use crate::source_state::state::State;
        use crate::source_state::statement::StatementNode;
        let mut symbols = crate::source_symbols::SymbolTableBuilder::default();
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
        // The same call with no receiver is a name that resolves to nothing,
        // and nothing in the program is named `call`: an error wherever it
        // is written.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.receiver = Default::default();
        program.expression_table.set_unresolved_name(
            unbound,
            crate::source_symbols::UnresolvedName::NoVisibleDeclaration,
        );
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UndeclaredCallName)]
        );
        // A spelling alone grants no compiler meaning. This synthetic table
        // has no installed declaration for the query, so it still names nothing.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target = crate::source_state::name::Identifier::generated("address_of");
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UndeclaredCallName)]
        );
        // A layout query typing recognized is served by the compiler and
        // records no row; the same spelling without that recognition is
        // still a name that resolves to nothing.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target = crate::source_state::name::Identifier::generated("size_of");
        node.layout_query = Some(
            crate::source_state::expression::DataLayoutQueryRequest::Type {
                kind: language_core::data_layout_queries::DataLayoutQueryKind::Size,
                operand: Default::default(),
            },
        );
        assert_eq!(deferred(&program).unwrap(), Vec::new());
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.layout_query = None;
        assert_eq!(
            deferred(&program).unwrap(),
            vec![(holder, DeferredRequest::UndeclaredCallName)]
        );
        // The later package diagnostic consumes the exact path kept by typing.
        let mut qualified = program.clone();
        let ExpressionNode::Call(call) = qualified.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        call.target = crate::source_state::name::Identifier::generated("kit::value");
        defer_unbound_calls(&mut qualified, 0, &[]).unwrap();
        let row = qualified.deferred_refusal(holder).unwrap();
        assert_eq!(row.unresolved_call_path.as_deref(), Some("kit::value"));
        assert_eq!(
            row.source_span,
            qualified.expression_table.source_span(unbound)
        );

        // An ambiguous source lookup stays unserved, regardless of which
        // declarations elsewhere happen to carry its spelling.
        let ExpressionNode::Call(node) = program.expression_table.expression_mut(unbound) else {
            panic!("call expression")
        };
        node.target = crate::source_state::name::Identifier::generated("run");
        program
            .expression_table
            .set_unresolved_name(unbound, crate::source_symbols::UnresolvedName::Ambiguous);
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
        use crate::source_symbols::{
            builtin_function_symbols, builtin_type_symbols, fixed_core_declaration_symbols,
        };
        use symbols::{BuiltinFunction, FixedCoreDeclaration};
        let mut symbols = crate::source_symbols::SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, symbols::SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain(fixed_core_declaration_symbols())
                .chain([
                    (
                        SymbolKind::MathematicalDefinition,
                        symbols::SymbolNameRef::Static("Identity"),
                    ),
                    (
                        SymbolKind::MathematicalDefinition,
                        symbols::SymbolNameRef::Static("core::Identity"),
                    ),
                    (
                        SymbolKind::MachineParameter,
                        symbols::SymbolNameRef::Static("Step"),
                    ),
                    (SymbolKind::Module, symbols::SymbolNameRef::Static("core")),
                    (SymbolKind::Local, symbols::SymbolNameRef::Static("value")),
                ]),
        );
        let declared =
            symbols::BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT + FixedCoreDeclaration::COUNT;
        let [identity, undeclared, step, module, value] =
            [0, 1, 2, 3, 4].map(|offset| children.handles().nth(declared + offset).unwrap());
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        program.push_mathematical_definition(
            crate::source_state::mathematical::MathematicalDefinition {
                symbol: identity,
                ..Default::default()
            },
        );
        let embed = program
            .symbols
            .builtin_function_symbol(BuiltinFunction::IntegerEmbed)
            .unwrap();
        let targets = DeclaredCallTargets::new(&program);
        assert!(
            call_target_parameters(&program, embed).unwrap().is_none(),
            "a builtin has no authored parameter telescope to substitute"
        );
        // A mathematical definition is applied by name, bare or qualified by
        // a module, and only where the program declares it.
        assert!(targets.declares(identity, None));
        assert!(targets.declares(identity, Some(module)));
        assert!(!targets.declares(identity, Some(value)));
        // An unrelated same-spelling symbol is not the sealed declaration or
        // an authored definition merely because its label is core::Identity.
        assert!(!targets.declares(undeclared, None));
        let sealed_identity = program
            .symbols
            .fixed_core_declaration_symbol(FixedCoreDeclaration::Identity)
            .unwrap();
        assert!(targets.declares(sealed_identity, None));
        assert!(targets.declares(sealed_identity, Some(module)));
        assert!(!targets.declares(sealed_identity, Some(value)));
        assert_eq!(
            targets.unserved(sealed_identity, CallReceiver::Absent),
            None
        );
        assert_eq!(
            targets.unserved(sealed_identity, CallReceiver::Named(value)),
            Some(DeferredRequest::ReceiverCallSelection)
        );
        for declaration in FixedCoreDeclaration::ALL {
            if declaration == FixedCoreDeclaration::Identity {
                continue;
            }
            let symbol = program
                .symbols
                .fixed_core_declaration_symbol(declaration)
                .unwrap();
            assert!(!targets.declares(symbol, None));
            assert!(!targets.declares(symbol, Some(module)));
        }
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
        use crate::source_state::expression::TableNamePath;
        use crate::source_state::signature::StateSignature;
        use crate::source_state::trait_definition::TraitDefinition;

        let mut symbols = crate::source_symbols::SymbolTableBuilder::default();
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
        let receivers = symbols.insert_children(
            caller,
            [
                (SymbolKind::Local, symbols::SymbolNameRef::Static("value")),
                (
                    SymbolKind::TypeParameter,
                    symbols::SymbolNameRef::Static("Selected"),
                ),
            ],
        );
        let receiver_value = receivers.handle_at(0);
        let record_receiver = receivers.handle_at(1);
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
        let record_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: owner,
                name: crate::source_state::name::Identifier::generated("Api"),
            });
        let parameters =
            program
                .data_type_parameters
                .insert_many([crate::source_state::data::TypeParameter {
                    symbol: record_receiver,
                    name: crate::source_state::name::Identifier::generated("Selected"),
                    kind: crate::source_state::data::TypeParameterKind::Const {
                        type_reference: record_type,
                    },
                    ..Default::default()
                }]);
        program.push_machine(crate::source_state::machine::Machine {
            symbol: caller,
            type_parameters: parameters,
            ..Default::default()
        });
        assert!(
            DeclaredCallTargets::new(&program).declares(requirement, Some(record_receiver)),
            "the exact Const schema member is not unsupported runtime receiver dispatch"
        );
        assert!(
            !DeclaredCallTargets::new(&program).declares(missing, Some(record_receiver)),
            "an absent member does not acquire the schema's authority"
        );
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
        assert!(
            call_target_parameters(&program, requirement)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        assert!(call_target_parameters(&program, missing).unwrap().is_none());
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
        use crate::source_state::statement::{
            StatementNode, TableNamePath, TableTransition, TransitionTargetNode,
        };
        use crate::source_state::{
            machine::{Machine, MachineBodyStatus},
            state::State,
        };
        let mut symbols = crate::source_symbols::SymbolTableBuilder::default();
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
            crate::source_state::name::Identifier::generated("callee"),
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

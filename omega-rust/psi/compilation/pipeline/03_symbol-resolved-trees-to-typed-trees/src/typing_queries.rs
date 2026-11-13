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

pub(crate) fn require_prebound_calls(program: &TypedTrees) -> Result<(), Diagnostic> {
    use typed_trees::expression::ExpressionNode;
    let namespace = |symbol: SymbolHandle| {
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
    };
    let concrete = |symbol: SymbolHandle| {
        symbol.is_valid()
            && (program.machine_by_own_or_state_symbol(symbol).is_some()
                || program.operator_definition_by_symbol(symbol).is_some())
    };
    for (handle, expression) in program.expression_table.expression_entries() {
        let ExpressionNode::Call(call) = expression else {
            continue;
        };
        let receiver_is_namespace = !call.receiver.is_valid()
            || matches!(program.expression_table.expression(call.receiver), ExpressionNode::Name(path) if namespace(path.symbol));
        if !concrete(call.target_symbol) || !receiver_is_namespace {
            return Err(
                crate::unavailable_judgment("static or dynamic callable target binding")
                    .with_source_span(program.expression_table.source_span(handle)),
            );
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(call) = statement.as_call() else {
                    continue;
                };
                let receiver_is_namespace = program
                    .statement_table
                    .name_path_members(call.receiver)
                    .is_empty()
                    || namespace(call.receiver_symbol);
                if !concrete(call.target_symbol) || !receiver_is_namespace {
                    return Err(crate::unavailable_judgment(
                        "projected or dynamic statement-call target binding",
                    )
                    .with_source_span(call.source_span));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn require_prebound_transitions(
    program: &TypedTrees,
    frontier: usize,
    completed: &[SymbolHandle],
) -> Result<(), Diagnostic> {
    use typed_trees::statement::TransitionTargetNode;
    for (ordinal, machine) in program.machines().iter().enumerate() {
        if ordinal < frontier && !completed.contains(&machine.symbol) {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    let TransitionTargetNode::Named {
                        path, source_span, ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    let local = path.symbol == machine.symbol
                        || program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == path.symbol);
                    if !local {
                        return Err(crate::unavailable_judgment(
                            "terminal callable transition normalization",
                        )
                        .with_source_span(*source_span));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_call_and_transition_rosters_need_no_unavailable_binding() {
        let program = TypedTrees::default();
        require_prebound_calls(&program).unwrap();
        require_prebound_transitions(&program, 0, &[]).unwrap();
    }

    #[test]
    fn an_unresolved_call_refuses_instead_of_claiming_a_bound_target() {
        use typed_trees::expression::TableCallExpression;
        let mut program = TypedTrees::default();
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                authored_call_selection: None,
                authored_call_binding: None,
                build_selection: Default::default(),
                authored_machine_application: Default::default(),
                exact_coercion_target: Default::default(),
                requirement_application: Default::default(),
                receiver: Default::default(),
                target_symbol: SymbolHandle::invalid(),
                static_machine_parameter: SymbolHandle::invalid(),
                target: typed_trees::name::Identifier::generated("unresolved"),
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
            }));
        let error = require_prebound_calls(&program).unwrap_err();
        assert!(error.message.starts_with("no product route yet:"));
        assert!(error.message.contains("callable target binding"));
        assert_eq!(program.expression_table.expression_count(), 1);
    }
}

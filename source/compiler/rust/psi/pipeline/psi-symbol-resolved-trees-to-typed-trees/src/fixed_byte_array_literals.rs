//! Contextual quoted-byte literal landing for owned fixed byte arrays.
//!
//! A quoted literal remains the ordinary dynamically-sized byte/string value
//! everywhere else.  Only an owned `[u8; N]` destination with a resolved,
//! literal extent may copy it into an ordinary array value, and that copy is
//! exact-width: neither truncation nor zero padding is language semantics.

use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};

pub(crate) fn land_exact_fixed_byte_array_literals(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let mut destinations = Vec::<(ExpressionHandle, TypeReferenceHandle)>::new();

    // Aggregate fields and value-call arguments carry their destination types
    // in the typed expression graph itself.
    for (_, expression) in program.expression_table.expression_entries() {
        match expression {
            ExpressionNode::StructLiteral(literal) => {
                let Some(definition) = program.data_definitions().iter().find(|definition| {
                    if literal.type_symbol.is_valid() {
                        definition.symbol == literal.type_symbol
                    } else {
                        definition.name.as_str() == literal.type_name.as_str()
                    }
                }) else {
                    continue;
                };
                for field in program.expression_table.struct_fields(literal.fields) {
                    if let Some(field_type) = construction_field_type(
                        program,
                        definition,
                        literal.case_symbol,
                        literal.case_name.as_ref().map(|name| name.as_str()),
                        field.field_symbol,
                        field.name.as_str(),
                    ) {
                        destinations.push((field.value, field_type));
                    }
                }
            }
            ExpressionNode::Call(call) => collect_call_destinations(
                program,
                call.target_symbol,
                program.expression_table.expression_handles(call.arguments),
                &mut destinations,
            ),
            _ => {}
        }
    }

    // Machine/state declarations own the remaining result positions.
    for machine in program.machines() {
        for owned in program.machine_owned_data(machine) {
            if owned.initial_value.is_valid() && owned.type_reference.is_valid() {
                destinations.push((owned.initial_value, owned.type_reference));
            }
        }
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (statement_index, statement) in statements.iter().enumerate() {
                match statement {
                    StatementNode::LocalData(local)
                        if local.initial_value.is_valid() && local.type_reference.is_valid() =>
                    {
                        destinations.push((local.initial_value, local.type_reference));
                    }
                    StatementNode::Call(call) => collect_call_destinations(
                        program,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                        &mut destinations,
                    ),
                    StatementNode::Expression(value)
                        if state.return_type.is_valid()
                            && statement_index + 1 == statements.len() =>
                    {
                        destinations.push((*value, state.return_type));
                    }
                    StatementNode::Transition(transition) => {
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(value)
                                    if state.return_type.is_valid() =>
                                {
                                    destinations.push((*value, state.return_type));
                                }
                                TransitionTargetNode::Named {
                                    path, arguments, ..
                                } => {
                                    collect_call_destinations(
                                        program,
                                        path.symbol,
                                        program.statement_table.expression_handles(*arguments),
                                        &mut destinations,
                                    );
                                }
                                TransitionTargetNode::Value(_)
                                | TransitionTargetNode::SelfTarget
                                | TransitionTargetNode::Terminal => {}
                            }
                        }
                    }
                    StatementNode::AssemblyFact(_)
                    | StatementNode::Assignment(_)
                    | StatementNode::Expression(_)
                    | StatementNode::LocalData(_) => {}
                }
            }
        }
    }

    for (value, destination) in destinations {
        land_one(program, value, destination)?;
    }
    Ok(())
}

fn collect_call_destinations(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    arguments: &[ExpressionHandle],
    destinations: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) {
    if !target_symbol.is_valid() {
        return;
    }
    let Some(target) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_symbol)
    }) else {
        return;
    };
    let parameters = program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self);
    for (parameter, argument) in parameters.zip(arguments.iter().copied()) {
        if parameter.type_reference.is_valid() {
            destinations.push((argument, parameter.type_reference));
        }
    }
}

fn construction_field_type(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    case_symbol: Option<SymbolHandle>,
    case_name: Option<&str>,
    field_symbol: SymbolHandle,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    let matches_field = |field: &psi_typed_trees::data::DataField| {
        if field_symbol.is_valid() {
            field.symbol == field_symbol
        } else {
            field.name.as_str() == field_name
        }
    };
    if let Some(field) = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if matches_field(field) => Some(field),
            _ => None,
        })
    {
        return Some(field.type_reference);
    }
    let variant = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant)
                if if case_symbol.is_some_and(|symbol| symbol.is_valid()) {
                    case_symbol == Some(variant.symbol)
                } else {
                    case_name.is_some_and(|name| variant.name.as_str() == name)
                } =>
            {
                Some(variant)
            }
            _ => None,
        })?;
    program
        .data_payload_fields(variant)
        .iter()
        .find(|field| matches_field(field))
        .map(|field| field.type_reference)
}

fn land_one(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    if !value.is_valid() || !destination.is_valid() {
        return Ok(());
    }
    let ExpressionNode::String(bytes) = program.expression_table.expression(value).clone() else {
        return Ok(());
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program
        .type_reference_table
        .type_reference(destination)
        .clone()
    else {
        return Ok(());
    };

    if program.primitive_type_reference(element_type) != Some(PrimitiveType::U8) {
        return Err(Diagnostic::error(format!(
            "quoted byte literal cannot initialize owned fixed array `{}`: its element type must be `u8`",
            program.display_type_reference(destination),
        )));
    }
    let FixedArrayLength::Literal(width) = length else {
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
                psi_numerics::literals::IntegerLiteral::from_value(i64::from(*byte)),
            ))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    *program.expression_table.expression_mut(value) = ExpressionNode::ArrayLiteral(elements);
    Ok(())
}

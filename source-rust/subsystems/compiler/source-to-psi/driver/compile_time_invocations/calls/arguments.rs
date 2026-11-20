//! Supply each call operand as a typed source expression. Borrowed host
//! storage and issued values enter through parameters of the generated caller.

use crate::compile_time_invocations::BuildTimeValue;
use crate::source_state::TypedTrees;
use crate::source_state::data::{DataDefinition, DataField, DataMember, DataMemberIdentity};
use crate::source_state::expression::{
    ExpressionHandle, ExpressionNode, TableStructLiteral, TableStructLiteralField,
};
use crate::source_state::signature::StateParameter;
use crate::source_state::state::State;
use crate::source_state::statement::StatementNode;
use crate::source_state::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::data_applications;

pub(super) struct MaterializedArguments {
    pub(super) expressions: Vec<ExpressionHandle>,
    pub(super) input_values: Vec<BuildTimeValue>,
}

pub(super) fn materialize(
    probe: &mut TypedTrees,
    state: &mut State,
    parameters: &[StateParameter],
    arguments: &[BuildTimeValue],
    input_types: &[TypeReferenceHandle],
) -> Result<MaterializedArguments, String> {
    let mut expressions = Vec::with_capacity(arguments.len());
    let mut input_values = Vec::new();
    for (position, ((parameter, argument), destination)) in parameters
        .iter()
        .zip(arguments)
        .zip(input_types.iter().copied())
        .enumerate()
    {
        let mut carrier = destination;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            probe.type_reference_table.type_reference(carrier)
        {
            carrier = *base_type;
        }
        let reference = match probe.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => Some((*referee, *access)),
            _ => None,
        };
        let sequence_input = matches!(reference, Some((referee, _))
                    if matches!(probe.type_reference_table.type_reference(referee), TypeReferenceNode::Slice { .. }));
        let value = if sequence_input {
            // A host view supplies its real backing as an entry input. A local
            // array literal would instead borrow storage owned by this caller.
            materialize_input(probe, state, &mut input_values, destination, argument)
        } else {
            materialize_argument(
                probe,
                state,
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
            let name = crate::source_state::name::Identifier::generated(&name);
            probe.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(crate::source_state::statement::TableLocalData {
                    symbol,
                    name: name.clone(),
                    type_reference: referee,
                    initial_value: value,
                    is_mutable: access.is_exclusive(),
                    ..Default::default()
                }),
            );
            let path = name_path(probe, symbol, name);
            let target = probe.expression_table.insert(ExpressionNode::Name(path));
            probe.expression_table.insert(ExpressionNode::Borrow(
                crate::source_state::expression::TableBorrowExpression { target, access },
            ))
        } else {
            value
        };
        // Retain the checked argument's actual type on its existing expression
        // row, so receipt consumers need not reconstruct the public signature.
        probe
            .expression_table
            .set_constructed_value_type_reference(actual, destination);
        expressions.push(actual);
    }
    Ok(MaterializedArguments {
        expressions,
        input_values,
    })
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
    case_name: Option<crate::source_state::name::Identifier>,
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
    // A formal or member type can be a data application no preparation has
    // viewed yet, such as a parameter's `Shelf<u64>` or a member type copied
    // for its enclosing lifetimes. Form its view the way typing forms one for
    // its own references; an application that stays open is still refused.
    if matches!(
        program.type_reference_table.type_reference(destination),
        TypeReferenceNode::Generic { .. }
    ) && program.data_application_definition(destination).is_none()
    {
        data_applications::prepare(program, destination)?;
    }
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
    let name = crate::source_state::name::Identifier::generated(&name);
    program.push_state_parameter(
        state,
        crate::source_state::signature::StateParameter {
            symbol,
            name: name.clone(),
            type_reference: destination,
            is_const: false,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        },
    );
    input_values.push(value.clone());
    let path = name_path(program, symbol, name);
    Ok(program.expression_table.insert(ExpressionNode::Name(path)))
}

fn materialize_argument(
    program: &mut TypedTrees,
    state: &mut State,
    input_values: &mut Vec<BuildTimeValue>,
    destination: TypeReferenceHandle,
    value: &BuildTimeValue,
) -> Result<ExpressionHandle, String> {
    use crate::source_state::types::PrimitiveType;
    use numeric_literals::{IntegerLiteral, IntegerRadix};
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
        let name_text = crate::source_state::name::Identifier::generated(&name);
        let symbol = program
            .symbols
            .append_generated_children_from(
                state.symbol,
                [(state.symbol, SymbolKind::Parameter, name.as_str())],
            )
            .start();
        program.push_state_parameter(
            state,
            crate::source_state::signature::StateParameter {
                symbol,
                name: name_text.clone(),
                type_reference: value_type,
                is_const: false,
                is_mutable: reference.is_some_and(|(_, access)| access.is_exclusive()),
                relevance: language_core::BindingRelevance::Relevant,
            },
        );
        let path = name_path(program, symbol, name_text);
        let actual = program.expression_table.insert(ExpressionNode::Name(path));
        input_values.push(value.clone());
        return Ok(if let Some((_, access)) = reference {
            program.expression_table.insert(ExpressionNode::Borrow(
                crate::source_state::expression::TableBorrowExpression {
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
                zero_omitted_fields: false,
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

pub(super) fn name_path(
    probe: &mut TypedTrees,
    symbol: SymbolHandle,
    name: crate::source_state::name::Identifier,
) -> crate::source_state::expression::TableNamePath {
    let mut members = arena::HandleSpan::empty();
    probe
        .expression_table
        .push_name_path_member(&mut members, name);
    let mut member_symbols = arena::HandleSpan::empty();
    probe
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, symbol);
    crate::source_state::expression::TableNamePath::new(members, member_symbols, symbol, symbol)
}

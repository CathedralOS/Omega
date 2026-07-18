use crate::symbols::TopLevelSymbols;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_data_field_types(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        let data_members = program.data_members(data_definition);
        let type_parameters = program.data_type_parameters(data_definition);
        validate_data_member_names(data_definition, data_members, diagnostics);
        validate_data_shape(program, data_definition, data_members, diagnostics);

        for member in data_members {
            let payload_fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => {
                    validate_payload_field_names(data_definition, variant, program, diagnostics);
                    program.data_payload_fields(variant)
                }
            };

            for field in payload_fields {
                validate_type_reference_handle_with_type_parameters(
                    program,
                    field.type_reference,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::DataField {
                        data: data_definition.name.as_str(),
                        field: field.name.as_str(),
                        generic_depth: 0,
                    },
                    type_parameters,
                );

                // A field DEFAULT (`x: i32 = true`, `b: i8 = 300`) is EMITTED as the
                // field's initial value (verified: a scalar default reads back), so a
                // cross-class or narrowing default is a silent store miscompile -- the
                // same obligations as any value-binding slot, checked here because a
                // default has no machine/state context. Defaults are always literals/
                // consts, so the machine-free literal paths suffice.
                if field.initial_value.is_valid()
                    && let Some(primitive) = program.primitive_type_reference(field.type_reference)
                {
                    let owner = format!(
                        "data `{}` field `{}` default",
                        data_definition.name.as_str(),
                        field.name.as_str()
                    );
                    // Class first; a cross-class default is not also narrowing-checked.
                    if !crate::expression_types::report_cross_class_store(
                        program,
                        None,
                        None,
                        field.initial_value,
                        primitive,
                        &owner,
                        "field",
                        diagnostics,
                    ) {
                        crate::arithmetic_domains::check_literal_default_narrowing(
                            program,
                            field.initial_value,
                            primitive,
                            &owner,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn validate_data_default_domains(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        let facts = program
            .proof_facts
            .span_or_empty(data_definition.default_domain);
        crate::proof_facts::validate_proof_facts(
            program,
            facts,
            diagnostics,
            crate::proof_facts::ProofFactOwner::DataDefaultDomain(data_definition.name.as_str()),
        );
        for fact in facts {
            let expression = match fact {
                ProofFact::Expression(expression) => *expression,
                ProofFact::Membership(membership) => {
                    validate_default_domain_membership_type(
                        program,
                        data_definition,
                        membership,
                        diagnostics,
                    );
                    membership.value
                }
            };
            validate_default_domain_field_names(program, data_definition, expression, diagnostics);
        }
    }
}

fn validate_default_domain_membership_type(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    membership: &omega_typed_trees::domain::ProofMembershipFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Name(path) = program.expression_table.expression(membership.value) else {
        return;
    };
    let [field_name] = program.expression_table.name_path_members(path.members) else {
        return;
    };
    let Some(field_type) = program
        .data_members(data_definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == field_name.as_str() => {
                Some(field.type_reference)
            }
            _ => None,
        })
    else {
        return;
    };
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == membership.domain_symbol)
    else {
        return;
    };
    let carrier = membership_subject_carrier(program, field_type);
    if crate::type_references::type_references_match(program, carrier, domain.target_type) {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "data `{}` default-domain membership for field `{}` uses domain `{}` over `{}`, but the field carrier is `{}`",
        data_definition.name.as_str(),
        field_name.as_str(),
        domain.name.as_str(),
        crate::type_references::type_reference_label(program, domain.target_type),
        crate::type_references::type_reference_label(program, carrier),
    )));
}

fn membership_subject_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => membership_subject_carrier(program, *base_type),
        _ => type_reference,
    }
}

fn validate_default_domain_field_names(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let valid = members.len() == 1
                && program.data_members(data_definition).iter().any(|member| {
                    matches!(member, DataMember::Field(field) if field.name.as_str() == members[0].as_str())
                });
            if !valid {
                let name = members
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` default domain references `{name}`, but a data default-domain fact may name only a bare field of that data",
                    data_definition.name.as_str(),
                )));
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_default_domain_field_names(program, data_definition, *value, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            validate_default_domain_field_names(program, data_definition, binary.left, diagnostics);
            validate_default_domain_field_names(
                program,
                data_definition,
                binary.right,
                diagnostics,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                validate_default_domain_field_names(
                    program,
                    data_definition,
                    call.receiver,
                    diagnostics,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_default_domain_field_names(
                    program,
                    data_definition,
                    *argument,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Indexed(indexed) => {
            validate_default_domain_field_names(
                program,
                data_definition,
                indexed.collection,
                diagnostics,
            );
            validate_default_domain_field_names(
                program,
                data_definition,
                indexed.index,
                diagnostics,
            );
        }
        ExpressionNode::Member(member) => validate_default_domain_field_names(
            program,
            data_definition,
            member.receiver,
            diagnostics,
        ),
        ExpressionNode::Mutable(inner) => {
            validate_default_domain_field_names(program, data_definition, *inner, diagnostics)
        }
        ExpressionNode::Range(range) => {
            validate_default_domain_field_names(program, data_definition, range.start, diagnostics);
            validate_default_domain_field_names(program, data_definition, range.end, diagnostics);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                validate_default_domain_field_names(
                    program,
                    data_definition,
                    field.value,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Unary(unary) => validate_default_domain_field_names(
            program,
            data_definition,
            unary.operand,
            diagnostics,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefaultDomainValue {
    Boolean(bool),
    Integer(i64),
}

pub(crate) fn default_domain_zero_is_valid(
    program: &TypedTrees,
    data_definition: &DataDefinition,
) -> bool {
    default_domain_zero_is_valid_recursive(program, data_definition, &mut Vec::new())
}

fn default_domain_zero_is_valid_recursive(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    visiting: &mut Vec<String>,
) -> bool {
    if visiting
        .iter()
        .any(|name| name == data_definition.name.as_str())
    {
        return false;
    }
    visiting.push(data_definition.name.as_str().to_owned());
    let field_values = program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some((field.name.as_str(), 0i64)),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let field_lengths = program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => type_reference_zero_length(program, field.type_reference)
                .map(|length| (field.name.as_str(), length)),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let field_capacities = program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => type_reference_zero_capacity(program, field.type_reference)
                .map(|capacity| (field.name.as_str(), capacity)),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let facts_hold = program
        .proof_facts
        .span_or_empty(data_definition.default_domain)
        .iter()
        .all(|fact| match fact {
            ProofFact::Expression(expression) => {
                matches!(
                    evaluate_default_domain_expression(
                        program,
                        *expression,
                        &field_values,
                        &field_lengths,
                        &field_capacities,
                    ),
                    Some(DefaultDomainValue::Boolean(true))
                )
            }
            ProofFact::Membership(membership) => program
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
                .and_then(|domain| {
                    if !default_domain_byte_sequence_carrier(program, domain.target_type) {
                        return None;
                    }
                    omega_typed_trees::byte_predicates::domain_classifier_byte_predicate(
                        program,
                        domain.symbol,
                    )
                })
                .is_some_and(|predicate| predicate.holds_for(&[])),
        });
    let stored_fields_hold = program
        .data_members(data_definition)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .chain(
            program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) => Some(program.data_payload_fields(variant)),
                    DataMember::Field(_) => None,
                })
                .unwrap_or_default()
                .iter(),
        )
        .all(|field| type_reference_zero_is_valid(program, field.type_reference, visiting));
    visiting.pop();
    facts_hold && stored_fields_hold
}

fn default_domain_byte_sequence_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => default_domain_byte_sequence_carrier(program, *base_type),
        TypeReferenceNode::Slice { element_type }
        | TypeReferenceNode::FixedArray { element_type, .. } => {
            program.primitive_type_reference(*element_type)
                == Some(omega_typed_trees::types::PrimitiveType::U8)
        }
        TypeReferenceNode::Named { name, .. } => name.as_str() == "String",
        _ => false,
    }
}

pub(crate) fn data_field_type_zero_is_valid(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_reference_zero_is_valid(program, type_reference, &mut Vec::new())
}

pub(crate) fn type_reference_zero_length(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<i64> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => type_reference_zero_length(program, *base_type),
        TypeReferenceNode::Slice { .. } => Some(0),
        TypeReferenceNode::FixedArray {
            length: omega_typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } => i64::try_from(*length).ok(),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "String" => Some(0),
        _ => None,
    }
}

pub(crate) fn type_reference_zero_capacity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<i64> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => type_reference_zero_capacity(program, *base_type),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "String" => Some(0),
        TypeReferenceNode::Generic {
            base_name,
            arguments: _,
            ..
        } if base_name.as_str() == "Vec" => Some(0),
        _ => None,
    }
}

fn type_reference_zero_is_valid(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<String>,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    if program.primitive_type_reference(type_reference).is_some() {
        return crate::arithmetic_domains::range_constraint_interval(program, type_reference)
            .is_none_or(|range| {
                range.low().is_none_or(|low| low <= 0) && range.high().is_none_or(|high| high >= 0)
            });
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_zero_is_valid(program, *base_type, visiting)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            matches!(
                length,
                omega_typed_trees::types::FixedArrayLength::Literal(0)
            ) || type_reference_zero_is_valid(program, *element_type, visiting)
        }
        TypeReferenceNode::Named { name, .. } => {
            if name.as_str() == "String" {
                return true;
            }
            program
                .data_definitions()
                .iter()
                .find(|data| data.name.as_str() == name.as_str())
                .map(|data| default_domain_zero_is_valid_recursive(program, data, visiting))
                .unwrap_or(true)
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => true,
    }
}

pub(crate) fn attached_default_domain_zero_is_valid(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> bool {
    let Some(attached_name) = machine.attached_data.as_ref() else {
        return true;
    };
    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached_name.as_str())
    else {
        return true;
    };
    default_domain_zero_is_valid(program, data_definition)
}

pub(crate) fn assignment_default_domain_window(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    target: ExpressionHandle,
) -> Option<(String, String)> {
    let Some(path) = crate::arithmetic_domains::place_path(program, target) else {
        return None;
    };
    let parts = path.split('.').collect::<Vec<_>>();
    let (Some("self"), Some(field_name)) = (parts.first().copied(), parts.last().copied()) else {
        return None;
    };
    let Some(attached_name) = machine.attached_data.as_ref() else {
        return None;
    };
    let Some(mut data_definition) = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached_name.as_str())
    else {
        return None;
    };
    let mut base = "self".to_owned();
    for member_name in parts.iter().skip(1).take(parts.len().saturating_sub(2)) {
        let declaration_name = member_name.split('[').next().unwrap_or(member_name);
        let field_type = program
            .data_members(data_definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == declaration_name => {
                    Some(field.type_reference)
                }
                _ => None,
            })?;
        data_definition = named_data_definition_for_type(program, field_type)?;
        base.push('.');
        base.push_str(member_name);
    }

    let participates = program
        .proof_facts
        .span_or_empty(data_definition.default_domain)
        .iter()
        .any(|fact| {
            let expression = match fact {
                ProofFact::Expression(expression) => *expression,
                ProofFact::Membership(membership) => membership.value,
            };
            default_domain_expression_mentions_field(program, expression, field_name)
        })
        || program.data_members(data_definition).iter().any(|member| {
            matches!(member, DataMember::Field(field)
                if field.name.as_str() == field_name
                    && !data_field_type_zero_is_valid(program, field.type_reference))
        });
    participates.then(|| (base, data_definition.name.as_str().to_owned()))
}

pub(crate) fn named_data_definition_for_type<'program>(
    program: &'program TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&'program DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } => named_data_definition_for_type(program, *base_type),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            named_data_definition_for_type(program, *element_type)
        }
        TypeReferenceNode::Named { name, .. } => program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name.as_str()),
        TypeReferenceNode::Generic { base_name, .. } => program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == base_name.as_str()),
        _ => None,
    }
}

fn default_domain_expression_mentions_field(
    program: &TypedTrees,
    expression: ExpressionHandle,
    field_name: &str,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            members.len() == 1 && members[0].as_str() == field_name
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| default_domain_expression_mentions_field(program, *value, field_name)),
        ExpressionNode::Binary(binary) => {
            default_domain_expression_mentions_field(program, binary.left, field_name)
                || default_domain_expression_mentions_field(program, binary.right, field_name)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && default_domain_expression_mentions_field(program, call.receiver, field_name))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        default_domain_expression_mentions_field(program, *argument, field_name)
                    })
        }
        ExpressionNode::Indexed(indexed) => {
            default_domain_expression_mentions_field(program, indexed.collection, field_name)
                || default_domain_expression_mentions_field(program, indexed.index, field_name)
        }
        ExpressionNode::Member(member) => {
            default_domain_expression_mentions_field(program, member.receiver, field_name)
        }
        ExpressionNode::Mutable(inner) => {
            default_domain_expression_mentions_field(program, *inner, field_name)
        }
        ExpressionNode::Range(range) => {
            (range.start.is_valid()
                && default_domain_expression_mentions_field(program, range.start, field_name))
                || (range.end.is_valid()
                    && default_domain_expression_mentions_field(program, range.end, field_name))
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| {
                default_domain_expression_mentions_field(program, field.value, field_name)
            }),
        ExpressionNode::Unary(unary) => {
            default_domain_expression_mentions_field(program, unary.operand, field_name)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => false,
    }
}

fn evaluate_default_domain_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    field_values: &[(&str, i64)],
    field_lengths: &[(&str, i64)],
    field_capacities: &[(&str, i64)],
) -> Option<DefaultDomainValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(DefaultDomainValue::Boolean(*value)),
        ExpressionNode::Integer(value) => value.value_i64().map(DefaultDomainValue::Integer),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 1)
                .then(|| {
                    field_values
                        .iter()
                        .find(|(name, _)| *name == members[0].as_str())
                        .map(|(_, value)| DefaultDomainValue::Integer(*value))
                })
                .flatten()
        }
        ExpressionNode::Mutable(inner) => evaluate_default_domain_expression(
            program,
            *inner,
            field_values,
            field_lengths,
            field_capacities,
        ),
        ExpressionNode::Member(member) if member.member.as_str() == "len" => {
            let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let [field] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            field_lengths
                .iter()
                .find(|(name, _)| *name == field.as_str())
                .map(|(_, value)| DefaultDomainValue::Integer(*value))
        }
        ExpressionNode::Member(member) if member.member.as_str() == "capacity" => {
            let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            let [field] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            field_capacities
                .iter()
                .find(|(name, _)| *name == field.as_str())
                .map(|(_, value)| DefaultDomainValue::Integer(*value))
        }
        ExpressionNode::Unary(unary) => {
            let DefaultDomainValue::Boolean(value) = evaluate_default_domain_expression(
                program,
                unary.operand,
                field_values,
                field_lengths,
                field_capacities,
            )?
            else {
                return None;
            };
            Some(DefaultDomainValue::Boolean(!value))
        }
        ExpressionNode::Binary(binary) => {
            let left = evaluate_default_domain_expression(
                program,
                binary.left,
                field_values,
                field_lengths,
                field_capacities,
            )?;
            let right = evaluate_default_domain_expression(
                program,
                binary.right,
                field_values,
                field_lengths,
                field_capacities,
            )?;
            match (left, binary.operator, right) {
                (
                    DefaultDomainValue::Boolean(left),
                    BinaryOperator::And,
                    DefaultDomainValue::Boolean(right),
                ) => Some(DefaultDomainValue::Boolean(left && right)),
                (
                    DefaultDomainValue::Boolean(left),
                    BinaryOperator::Or,
                    DefaultDomainValue::Boolean(right),
                ) => Some(DefaultDomainValue::Boolean(left || right)),
                (
                    DefaultDomainValue::Boolean(left),
                    BinaryOperator::Equal,
                    DefaultDomainValue::Boolean(right),
                ) => Some(DefaultDomainValue::Boolean(left == right)),
                (
                    DefaultDomainValue::Boolean(left),
                    BinaryOperator::NotEqual,
                    DefaultDomainValue::Boolean(right),
                ) => Some(DefaultDomainValue::Boolean(left != right)),
                (
                    DefaultDomainValue::Integer(left),
                    operator,
                    DefaultDomainValue::Integer(right),
                ) => {
                    let value = match operator {
                        BinaryOperator::Add => {
                            DefaultDomainValue::Integer(left.checked_add(right)?)
                        }
                        BinaryOperator::Subtract => {
                            DefaultDomainValue::Integer(left.checked_sub(right)?)
                        }
                        BinaryOperator::Multiply => {
                            DefaultDomainValue::Integer(left.checked_mul(right)?)
                        }
                        BinaryOperator::Divide => {
                            DefaultDomainValue::Integer(left.checked_div(right)?)
                        }
                        BinaryOperator::Modulo => {
                            DefaultDomainValue::Integer(left.checked_rem(right)?)
                        }
                        BinaryOperator::BitwiseAnd => DefaultDomainValue::Integer(left & right),
                        BinaryOperator::BitwiseOr => DefaultDomainValue::Integer(left | right),
                        BinaryOperator::BitwiseXor => DefaultDomainValue::Integer(left ^ right),
                        BinaryOperator::ShiftLeft => DefaultDomainValue::Integer(
                            left.checked_shl(u32::try_from(right).ok()?)?,
                        ),
                        BinaryOperator::ShiftRight => DefaultDomainValue::Integer(
                            left.checked_shr(u32::try_from(right).ok()?)?,
                        ),
                        BinaryOperator::Equal => DefaultDomainValue::Boolean(left == right),
                        BinaryOperator::NotEqual => DefaultDomainValue::Boolean(left != right),
                        BinaryOperator::Greater => DefaultDomainValue::Boolean(left > right),
                        BinaryOperator::GreaterOrEqual => {
                            DefaultDomainValue::Boolean(left >= right)
                        }
                        BinaryOperator::Less => DefaultDomainValue::Boolean(left < right),
                        BinaryOperator::LessOrEqual => DefaultDomainValue::Boolean(left <= right),
                        BinaryOperator::And | BinaryOperator::Or => return None,
                    };
                    Some(value)
                }
                _ => None,
            }
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => None,
    }
}

// The zero-case-payload-free rule moved into `[zero_init]` verification
// (crate::properties): zero-VALIDITY is unconditional (a zeroed payload is
// itself zeroed, so the value stays valid), while zero-MEANS-EMPTY is the
// opt-in property that demands a payload-free zero case (frozen decision 8).

/// `[zero_init]` is an explicit claim that zero is a valid empty value, so its
/// reachable field ranges must contain zero. Ordinary machine-attached data is
/// no longer included here: R2 treats an excluding range as default-domain
/// sugar and gates access to the physically zeroed storage until establishment.
pub(crate) fn validate_zero_reachable_field_ranges(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut queue: Vec<&str> = Vec::new();
    for data_definition in program.data_definitions() {
        if data_definition.properties.zero_init {
            queue.push(data_definition.name.as_str());
        }
    }

    let mut seen: Vec<&str> = Vec::new();
    while let Some(name) = queue.pop() {
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let Some(data_definition) = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
        else {
            continue;
        };
        for member in program.data_members(data_definition) {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant),
            };
            for field in fields {
                if let Some(interval) = crate::arithmetic_domains::range_constraint_interval(
                    program,
                    field.type_reference,
                ) && (interval.low().is_some_and(|low| low > 0)
                    || interval.high().is_some_and(|high| high < 0))
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "field `{}` of `{name}` declares a range that excludes 0, but `{name}` \
                         declares `[zero_init]`: zero-means-empty requires every reachable field \
                         range to contain zero",
                        field.name.as_str(),
                    )));
                }
                // A FIXED-ARRAY field's ELEMENT range is the same invariant one
                // level down: ZII zeroes every element, so an element range
                // excluding 0 (`cells: [i32 [1..=7]; 4]`) is violated by the
                // initial state before the first write.
                if let Some(element_type) = fixed_array_element_type(program, field.type_reference)
                    && let Some(interval) =
                        crate::arithmetic_domains::range_constraint_interval(program, element_type)
                    && (interval.low().is_some_and(|low| low > 0)
                        || interval.high().is_some_and(|high| high < 0))
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "field `{}` of `{name}` declares an array ELEMENT range that excludes \
                         0, but `{name}` is zero-initialized (every element starts 0): a read \
                         before the first write would trust a bound the actual value 0 \
                         violates. Include 0 in the element range",
                        field.name.as_str(),
                    )));
                }
                if let Some(inner) = embedded_data_name(program, field.type_reference) {
                    queue.push(inner);
                }
            }
        }
    }
}

/// The ELEMENT type of a fixed-array field (through constraint shells,
/// recursing nested arrays): `[i32 [0..=7]; 4]` -> the constrained `i32`.
/// `None` for non-array types.
fn fixed_array_element_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            fixed_array_element_type(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            Some(fixed_array_element_type(program, *element_type).unwrap_or(*element_type))
        }
        _ => None,
    }
}

/// The Named data type a field EMBEDS (its bytes live inline, so ZII zeroes
/// them): through constraint shells and fixed-array elements. References are
/// NOT embedded -- a `&T` field is a zeroed pointer, not zeroed `T` bytes.
fn embedded_data_name<'program>(
    program: &'program TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&'program str> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => embedded_data_name(program, *base_type),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            embedded_data_name(program, *element_type)
        }
        TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn validate_payload_field_names(
    data_definition: &omega_typed_trees::data::DataDefinition,
    variant: &omega_typed_trees::data::DataVariant,
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let data_members = program.data_members(data_definition);
    let payload_fields = program.data_payload_fields(variant);
    for (field_index, field) in payload_fields.iter().enumerate() {
        if payload_fields[..field_index]
            .iter()
            .any(|previous| previous.name.as_str() == field.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` has duplicate payload field `{}`",
                data_definition.name, variant.name, field.name
            )));
        }

        // Mixed shapes: a payload field may not reuse a COMMON field's name.
        // Member access (`value.name`) searches common fields first and then
        // every case's payload, so a collision would silently rebind reads.
        if data_members.iter().any(|member| {
            matches!(member, DataMember::Field(common) if common.name.as_str() == field.name.as_str())
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` payload field `{}` collides with the common field of the same name",
                data_definition.name,
                variant.name,
                field.name
            )));
        }
    }
}

/// Mixed shapes (common fields + cases) are accepted with two honest-first-cut
/// restrictions, both rejected loudly here:
///
/// - COMMON fields must be scalar primitives (bool / integers / floats).
///   Case construction zero-initializes every common field not named in the
///   literal, and a scalar zero is one storage write in both backends; zeroing
///   nested aggregates / text / slices at construction is deferred.
/// - COMMON fields may not declare default initializers. Construction of a
///   mixed value is always the case-literal form, whose rule is
///   zero-unless-named (ZII keeps that valid); a default would silently not
///   apply, so it is rejected instead of ignored.
///
/// Payload-field/common-field name collisions are rejected separately in
/// `validate_payload_field_names` (member access searches both namespaces).
fn validate_data_shape(
    program: &TypedTrees,
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match omega_typed_trees::data::DataDefinition::shape_kind_from_members(data_members) {
        DataShapeKind::Empty | DataShapeKind::Enum | DataShapeKind::Record => {}
        DataShapeKind::Mixed => {
            for member in data_members {
                let DataMember::Field(field) = member else {
                    continue;
                };
                let scalar = matches!(
                    program.primitive_type_reference(field.type_reference),
                    Some(primitive) if primitive != omega_typed_trees::types::PrimitiveType::String
                );
                if !scalar {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` common field `{}` is not a scalar primitive; mixed data shape common fields support only bool, integer, and float types for now (case construction zero-initializes unnamed common fields)",
                        data_definition.name, field.name
                    )));
                }
                if field.initial_value.is_valid() {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` common field `{}` declares a default value; mixed data shape common fields are zero-initialized unless named in the case literal, so a default would never apply",
                        data_definition.name, field.name
                    )));
                }
            }
        }
    }
}

fn validate_data_member_names(
    data_definition: &omega_typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (member_index, member) in data_members.iter().enumerate() {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if data_members[..member_index]
            .iter()
            .any(|previous| data_member_name(previous) == member_name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }
    }
}

fn data_member_name(member: &DataMember) -> &str {
    match member {
        DataMember::Field(field) => field.name.as_str(),
        DataMember::Variant(variant) => variant.name.as_str(),
    }
}

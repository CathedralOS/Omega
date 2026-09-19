//! Lowering the Boolean expressions that machine parameters admit.

use crate::values::scalar::boolean_lowering::lower_boolean_expression;
use crate::values::scalar::expression_facts::{
    checked_integer_binary_kind, combine_arithmetic_domains, is_integer, operator_is_builtin,
    parameter_position, scalar_expression_type,
};
use crate::values::scalar::scalar_lowering::landed_for_primitive;
use crate::values::scalar::structural_fields;
use crate::values::scalar::structural_fields::{structural_data, structural_parameter_field_path};
use checked_trees::{
    CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedOperatorFacts, CheckedScalarExpression,
    CheckedStructuralParameterField, CheckedStructuralPredicatePathSegment,
};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::IntegerLanding;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Lower a contract predicate in the selected machine's entry-parameter
/// namespace. Crash contracts use this to retain the same checked scalar
/// meaning as executable guards without carrying typed-tree handles into the
/// terminal producer.
pub(crate) fn lower_machine_parameter_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);

    fn lower_structural_boolean_expression(
        program: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        operators: &CheckedOperatorFacts,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        fn field_type(
            program: &TypedTrees,
            receiver: TypeReferenceHandle,
            identity: &str,
        ) -> Option<TypeReferenceHandle> {
            let declaration = structural_data(program, receiver)?;
            program.data_members(declaration).iter().find_map(|member| {
                let typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                let matches_identity = match field.identity {
                    Some(field_identity) => identity == format!("#{field_identity}"),
                    None => field.name.as_str() == identity,
                };
                matches_identity.then_some(field.type_reference)
            })
        }

        fn path_type_reference(
            program: &TypedTrees,
            parameters: &[StateParameter],
            parameter_position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<TypeReferenceHandle> {
            let parameter = usize::try_from(parameter_position)
                .ok()
                .and_then(|position| parameters.get(position))?;
            let mut receiver = parameter.type_reference;
            let mut selected_case = None;
            for segment in path {
                match segment {
                    CheckedStructuralPredicatePathSegment::FixedIndex(element_index) => {
                        if selected_case.is_some() {
                            return None;
                        }
                        receiver = structural_fields::fixed_index_element_type(
                            program,
                            receiver,
                            *element_index,
                        )?;
                    }
                    CheckedStructuralPredicatePathSegment::Case(case) => {
                        if selected_case.is_some() {
                            return None;
                        }
                        let data = structural_data(program, receiver)?;
                        let variant = program.data_members(data).iter().find_map(|member| {
                            let typed_trees::data::DataMember::Variant(variant) = member else {
                                return None;
                            };
                            let identity = variant
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| variant.name.as_str().to_owned());
                            (identity == *case).then_some(variant)
                        })?;
                        selected_case = Some(variant);
                    }
                    CheckedStructuralPredicatePathSegment::Field(field) => {
                        receiver = if let Some(variant) = selected_case.take() {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find_map(|candidate| {
                                    let identity = candidate
                                        .identity
                                        .map(|identity| format!("#{identity}"))
                                        .unwrap_or_else(|| candidate.name.as_str().to_owned());
                                    (identity == *field).then_some(candidate.type_reference)
                                })?
                        } else {
                            field_type(program, receiver, field)?
                        };
                    }
                }
            }
            selected_case.is_none().then_some(receiver)
        }

        fn path_primitive_type(
            program: &TypedTrees,
            parameters: &[StateParameter],
            parameter_position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<PrimitiveType> {
            program.primitive_type_reference(path_type_reference(
                program,
                parameters,
                parameter_position,
                path,
            )?)
        }

        fn structural_record_fields(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Option<Vec<&typed_trees::data::DataField>> {
            let data = structural_data(program, type_reference)?;
            let mut fields = Vec::new();
            for member in program.data_members(data) {
                let typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                if field.relevance.is_erased() {
                    return None;
                }
                fields.push(field);
            }
            Some(fields)
        }

        fn payloadless_sum_cases(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Option<Vec<String>> {
            let data = structural_data(program, type_reference)?;
            let members = program.data_members(data);
            if !matches!(
                typed_trees::data::DataDefinition::shape_kind_from_members(members),
                typed_trees::data::DataShapeKind::Enum
            ) {
                return None;
            }
            members
                .iter()
                .map(|member| {
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        return None;
                    };
                    program.data_payload_fields(variant).is_empty().then(|| {
                        variant
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| variant.name.as_str().to_owned())
                    })
                })
                .collect()
        }

        fn append_direct_structural_leaf_equality(
            program: &TypedTrees,
            left_parameter: u32,
            right_parameter: u32,
            type_reference: TypeReferenceHandle,
            left: Vec<CheckedStructuralPredicatePathSegment>,
            right: Vec<CheckedStructuralPredicatePathSegment>,
            comparisons: &mut Vec<CheckedBooleanExpression>,
        ) -> Option<()> {
            match program.primitive_type_reference(type_reference) {
                Some(PrimitiveType::Bool) => {
                    comparisons.push(CheckedBooleanExpression::Equal {
                        left: Box::new(CheckedBooleanExpression::StructuralParameterField {
                            parameter_position: left_parameter,
                            path: left,
                        }),
                        right: Box::new(CheckedBooleanExpression::StructuralParameterField {
                            parameter_position: right_parameter,
                            path: right,
                        }),
                    });
                }
                Some(primitive_type)
                    if is_integer(primitive_type) && primitive_type != PrimitiveType::Addr =>
                {
                    comparisons.push(CheckedBooleanExpression::IntegerComparison {
                        kind: CheckedIntegerComparisonKind::Equal,
                        left: Box::new(CheckedScalarExpression::StructuralParameterField {
                            parameter_position: left_parameter,
                            path: left,
                            primitive_type,
                        }),
                        right: Box::new(CheckedScalarExpression::StructuralParameterField {
                            parameter_position: right_parameter,
                            path: right,
                            primitive_type,
                        }),
                    });
                }
                Some(primitive_type)
                    if matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) =>
                {
                    let mut left = CheckedStructuralParameterField {
                        parameter_position: left_parameter,
                        path: left,
                    };
                    let mut right = CheckedStructuralParameterField {
                        parameter_position: right_parameter,
                        path: right,
                    };
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    comparisons.push(CheckedBooleanExpression::IeeeFloatComparison {
                        kind: CheckedIeeeFloatComparisonKind::Equal,
                        primitive_type,
                        left,
                        right,
                    });
                }
                Some(_) => return None,
                None if crate::execution::terminal_unit::types::byte_sequence_carrier(
                    program,
                    type_reference,
                    &[],
                )
                .is_some() =>
                {
                    let mut left = CheckedStructuralParameterField {
                        parameter_position: left_parameter,
                        path: left,
                    };
                    let mut right = CheckedStructuralParameterField {
                        parameter_position: right_parameter,
                        path: right,
                    };
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    comparisons.push(CheckedBooleanExpression::ByteSequenceEqual { left, right });
                }
                None => return None,
            }
            Some(())
        }

        fn is_bounded_nested_mixed_field_path_pair(
            left: &[CheckedStructuralPredicatePathSegment],
            right: &[CheckedStructuralPredicatePathSegment],
        ) -> bool {
            const MAX_ENCLOSING_FIELDS: usize = 14;

            !left.is_empty()
                && left.len() == right.len()
                && left.len() <= MAX_ENCLOSING_FIELDS
                && left.iter().all(|segment| {
                    matches!(segment, CheckedStructuralPredicatePathSegment::Field(_))
                })
                && right.iter().all(|segment| {
                    matches!(segment, CheckedStructuralPredicatePathSegment::Field(_))
                })
        }

        fn append_acyclic_structural_equality(
            program: &TypedTrees,
            left_parameter: u32,
            right_parameter: u32,
            type_reference: TypeReferenceHandle,
            left_path: &[CheckedStructuralPredicatePathSegment],
            right_path: &[CheckedStructuralPredicatePathSegment],
            comparisons: &mut Vec<CheckedBooleanExpression>,
            visiting: &mut Vec<symbols::SymbolHandle>,
            nested_mixed_seen: &mut bool,
            allow_direct_nested_mixed: bool,
        ) -> Option<()> {
            if append_direct_structural_leaf_equality(
                program,
                left_parameter,
                right_parameter,
                type_reference,
                left_path.to_vec(),
                right_path.to_vec(),
                comparisons,
            )
            .is_some()
            {
                return Some(());
            }
            if let Some(cases) = payloadless_sum_cases(program, type_reference) {
                comparisons.push(CheckedBooleanExpression::PayloadlessSumEqual {
                    left: CheckedStructuralParameterField {
                        parameter_position: left_parameter,
                        path: left_path.to_vec(),
                    },
                    right: CheckedStructuralParameterField {
                        parameter_position: right_parameter,
                        path: right_path.to_vec(),
                    },
                    cases,
                });
                return Some(());
            }
            let data = structural_data(program, type_reference)?;
            if !data.symbol.is_valid() || visiting.contains(&data.symbol) {
                return None;
            }
            visiting.push(data.symbol);
            let result = (|| {
                let members = program.data_members(data);
                match typed_trees::data::DataDefinition::shape_kind_from_members(members) {
                    typed_trees::data::DataShapeKind::Empty => Some(()),
                    typed_trees::data::DataShapeKind::Record => {
                        for field in structural_record_fields(program, type_reference)? {
                            let field_identity = field
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| field.name.as_str().to_owned());
                            let mut left = left_path.to_vec();
                            left.push(CheckedStructuralPredicatePathSegment::Field(
                                field_identity.clone(),
                            ));
                            let mut right = right_path.to_vec();
                            right
                                .push(CheckedStructuralPredicatePathSegment::Field(field_identity));
                            append_acyclic_structural_equality(
                                program,
                                left_parameter,
                                right_parameter,
                                field.type_reference,
                                &left,
                                &right,
                                comparisons,
                                visiting,
                                nested_mixed_seen,
                                allow_direct_nested_mixed,
                            )?;
                        }
                        Some(())
                    }
                    typed_trees::data::DataShapeKind::Enum => {
                        let mut arms = Vec::new();
                        for member in members {
                            let typed_trees::data::DataMember::Variant(variant) = member else {
                                return None;
                            };
                            let case = variant
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| variant.name.as_str().to_owned());
                            let mut arm = vec![
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: left_parameter,
                                        path: left_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: right_parameter,
                                        path: right_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                            ];
                            for field in program.data_payload_fields(variant) {
                                if field.relevance.is_erased() {
                                    return None;
                                }
                                let field_identity = field
                                    .identity
                                    .map(|identity| format!("#{identity}"))
                                    .unwrap_or_else(|| field.name.as_str().to_owned());
                                let mut left = left_path.to_vec();
                                left.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                left.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity.clone(),
                                ));
                                let mut right = right_path.to_vec();
                                right.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                right.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity,
                                ));
                                append_acyclic_structural_equality(
                                    program,
                                    left_parameter,
                                    right_parameter,
                                    field.type_reference,
                                    &left,
                                    &right,
                                    &mut arm,
                                    visiting,
                                    nested_mixed_seen,
                                    allow_direct_nested_mixed,
                                )?;
                            }
                            let mut arm = arm.into_iter();
                            let first = arm.next()?;
                            arms.push(arm.fold(first, |left, right| {
                                CheckedBooleanExpression::And {
                                    left: Box::new(left),
                                    right: Box::new(right),
                                }
                            }));
                        }
                        let mut arms = arms.into_iter();
                        let first = arms.next()?;
                        comparisons.push(arms.fold(first, |left, right| {
                            CheckedBooleanExpression::Or {
                                left: Box::new(left),
                                right: Box::new(right),
                            }
                        }));
                        Some(())
                    }
                    typed_trees::data::DataShapeKind::Mixed => {
                        // The bounded nested mixed-shape slice permits one through
                        // fourteen direct record fields before the sole mixed occurrence.
                        // Deeper records, case payloads, and two mixed siblings retain
                        // their fail-closed fence until their independent path and replay
                        // canaries land.
                        if !left_path.is_empty() || !right_path.is_empty() {
                            if !is_bounded_nested_mixed_field_path_pair(left_path, right_path)
                                || !allow_direct_nested_mixed
                                || *nested_mixed_seen
                            {
                                return None;
                            }
                            *nested_mixed_seen = true;
                        }
                        for member in members {
                            let typed_trees::data::DataMember::Field(field) = member else {
                                continue;
                            };
                            if field.relevance.is_erased() {
                                return None;
                            }
                            let field_identity = field
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| field.name.as_str().to_owned());
                            let mut left = left_path.to_vec();
                            left.push(CheckedStructuralPredicatePathSegment::Field(
                                field_identity.clone(),
                            ));
                            let mut right = right_path.to_vec();
                            right
                                .push(CheckedStructuralPredicatePathSegment::Field(field_identity));
                            append_acyclic_structural_equality(
                                program,
                                left_parameter,
                                right_parameter,
                                field.type_reference,
                                &left,
                                &right,
                                comparisons,
                                visiting,
                                nested_mixed_seen,
                                allow_direct_nested_mixed,
                            )?;
                        }
                        let mut arms = Vec::new();
                        for member in members {
                            let typed_trees::data::DataMember::Variant(variant) = member else {
                                continue;
                            };
                            let case = variant
                                .identity
                                .map(|identity| format!("#{identity}"))
                                .unwrap_or_else(|| variant.name.as_str().to_owned());
                            let mut arm = vec![
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: left_parameter,
                                        path: left_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                                CheckedBooleanExpression::StructuralCaseMembership {
                                    subject: CheckedStructuralParameterField {
                                        parameter_position: right_parameter,
                                        path: right_path.to_vec(),
                                    },
                                    case: case.clone(),
                                },
                            ];
                            for field in program.data_payload_fields(variant) {
                                if field.relevance.is_erased() {
                                    return None;
                                }
                                let field_identity = field
                                    .identity
                                    .map(|identity| format!("#{identity}"))
                                    .unwrap_or_else(|| field.name.as_str().to_owned());
                                let mut left = left_path.to_vec();
                                left.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                left.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity.clone(),
                                ));
                                let mut right = right_path.to_vec();
                                right.push(CheckedStructuralPredicatePathSegment::Case(
                                    case.clone(),
                                ));
                                right.push(CheckedStructuralPredicatePathSegment::Field(
                                    field_identity,
                                ));
                                append_acyclic_structural_equality(
                                    program,
                                    left_parameter,
                                    right_parameter,
                                    field.type_reference,
                                    &left,
                                    &right,
                                    &mut arm,
                                    visiting,
                                    nested_mixed_seen,
                                    allow_direct_nested_mixed,
                                )?;
                            }
                            let mut arm = arm.into_iter();
                            let first = arm.next()?;
                            arms.push(arm.fold(first, |left, right| {
                                CheckedBooleanExpression::And {
                                    left: Box::new(left),
                                    right: Box::new(right),
                                }
                            }));
                        }
                        let mut arms = arms.into_iter();
                        let first = arms.next()?;
                        comparisons.push(arms.fold(first, |left, right| {
                            CheckedBooleanExpression::Or {
                                left: Box::new(left),
                                right: Box::new(right),
                            }
                        }));
                        Some(())
                    }
                }
            })();
            visiting.pop();
            result
        }

        fn lower_structural_equality(
            program: &TypedTrees,
            parameters: &[StateParameter],
            left: ExpressionHandle,
            right: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            fn collect_comparisons(
                program: &TypedTrees,
                left_parameter: u32,
                right_parameter: u32,
                type_reference: TypeReferenceHandle,
                left_path: &mut [CheckedStructuralPredicatePathSegment],
                right_path: &mut [CheckedStructuralPredicatePathSegment],
                output: &mut Vec<CheckedBooleanExpression>,
                visiting: &mut Vec<symbols::SymbolHandle>,
                allow_direct_nested_mixed: bool,
            ) -> Option<()> {
                let mut nested_mixed_seen = false;
                append_acyclic_structural_equality(
                    program,
                    left_parameter,
                    right_parameter,
                    type_reference,
                    left_path,
                    right_path,
                    output,
                    visiting,
                    &mut nested_mixed_seen,
                    allow_direct_nested_mixed,
                )
            }

            let mut left_path = Vec::new();
            let mut right_path = Vec::new();
            let left_parameter =
                structural_parameter_field_path(program, parameters, left, &mut left_path)?;
            let right_parameter =
                structural_parameter_field_path(program, parameters, right, &mut right_path)?;
            let left_type = path_type_reference(program, parameters, left_parameter, &left_path)?;
            let right_type =
                path_type_reference(program, parameters, right_parameter, &right_path)?;
            let left_data = structural_data(program, left_type)?;
            let right_data = structural_data(program, right_type)?;
            if left_data.symbol != right_data.symbol || left_data.name != right_data.name {
                return None;
            }
            let allow_direct_nested_mixed = left_path.is_empty()
                && right_path.is_empty()
                && matches!(
                    typed_trees::data::DataDefinition::shape_kind_from_members(
                        program.data_members(left_data)
                    ),
                    typed_trees::data::DataShapeKind::Record
                );
            let mut comparisons = Vec::new();
            collect_comparisons(
                program,
                left_parameter,
                right_parameter,
                left_type,
                &mut left_path,
                &mut right_path,
                &mut comparisons,
                &mut Vec::new(),
                allow_direct_nested_mixed,
            )?;
            let mut comparisons = comparisons.into_iter();
            let Some(first) = comparisons.next() else {
                return Some(CheckedBooleanExpression::Constant(true));
            };
            Some(
                comparisons.fold(first, |left, right| CheckedBooleanExpression::And {
                    left: Box::new(left),
                    right: Box::new(right),
                }),
            )
        }

        fn lower_structural_integer_expression(
            program: &TypedTrees,
            operators: &CheckedOperatorFacts,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
            let mut path = Vec::new();
            if let Some(parameter_position) =
                structural_parameter_field_path(program, parameters, expression, &mut path)
                && !path.is_empty()
                && let Some(type_reference) =
                    path_type_reference(program, parameters, parameter_position, &path)
                && let Some(primitive_type) = program.primitive_type_reference(type_reference)
                && is_integer(primitive_type)
                && primitive_type != PrimitiveType::Addr
            {
                return Some((
                    CheckedScalarExpression::StructuralParameterField {
                        parameter_position,
                        path,
                        primitive_type,
                    },
                    program.arithmetic_domain_for_type_reference(type_reference),
                ));
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(name) => {
                    let source_position = parameter_position(program, name, parameters)?;
                    let parameter = parameters.get(source_position)?;
                    // An erased binding has no scalar position to read.
                    if parameter.relevance.is_erased() {
                        return None;
                    }
                    let primitive_type =
                        program.primitive_type_reference(parameter.type_reference)?;
                    if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
                        return None;
                    }
                    // Scalar values use the dense primitive namespace, while
                    // structural member roots above retain authored positions.
                    let position = parameters[..source_position]
                        .iter()
                        .filter(|parameter| {
                            crate::values::scalar::occupies_scalar_position(program, parameter)
                        })
                        .count();
                    Some((
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type,
                        },
                        program.arithmetic_domain_for_type_reference(parameter.type_reference),
                    ))
                }
                ExpressionNode::Integer(literal) => Some((
                    CheckedScalarExpression::IntegerLiteral {
                        literal: literal.clone(),
                    },
                    literal
                        .landing()
                        .map(|landing| landing.domain)
                        .unwrap_or(ArithmeticDomain::Exact),
                )),
                ExpressionNode::Unary(unary)
                    if unary.operator == UnaryOperator::BitwiseNot
                        && operator_is_builtin(operators, expression) =>
                {
                    let (operand, domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        unary.operand,
                    )?;
                    let primitive_type = scalar_expression_type(&operand)?;
                    (is_integer(primitive_type) && primitive_type != PrimitiveType::Addr).then_some(
                        (
                            CheckedScalarExpression::IntegerBitwiseNot {
                                primitive_type,
                                operand: Box::new(operand),
                            },
                            domain,
                        ),
                    )
                }
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::BitwiseAnd
                            | BinaryOperator::BitwiseOr
                            | BinaryOperator::BitwiseXor
                    ) && operator_is_builtin(operators, expression) =>
                {
                    let (left, left_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    )?;
                    let (right, right_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    )?;
                    let primitive_type = scalar_expression_type(&left)?;
                    let domain = combine_arithmetic_domains(left_domain, right_domain)?;
                    let kind = match binary.operator {
                        BinaryOperator::BitwiseAnd => CheckedIntegerBinaryKind::BitwiseAnd,
                        BinaryOperator::BitwiseOr => CheckedIntegerBinaryKind::BitwiseOr,
                        BinaryOperator::BitwiseXor => CheckedIntegerBinaryKind::BitwiseXor,
                        _ => unreachable!("guarded structural bitwise operator"),
                    };
                    (is_integer(primitive_type)
                        && primitive_type != PrimitiveType::Addr
                        && scalar_expression_type(&right) == Some(primitive_type))
                    .then_some((
                        CheckedScalarExpression::IntegerBinary {
                            kind,
                            primitive_type,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        domain,
                    ))
                }
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                            | BinaryOperator::Modulo
                    ) && operator_is_builtin(operators, expression) =>
                {
                    let (left, left_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    )?;
                    let (right, right_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    )?;
                    let primitive_type = scalar_expression_type(&left)?;
                    let domain = combine_arithmetic_domains(left_domain, right_domain)?;
                    let kind = checked_integer_binary_kind(binary.operator, domain)?;
                    let supported = matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactAdd
                            | CheckedIntegerBinaryKind::ExactSubtract
                            | CheckedIntegerBinaryKind::ExactMultiply
                            | CheckedIntegerBinaryKind::ExactDivide
                            | CheckedIntegerBinaryKind::ExactRemainder
                    ) || (primitive_type != PrimitiveType::Addr
                        && matches!(
                            kind,
                            CheckedIntegerBinaryKind::WrappingAdd
                                | CheckedIntegerBinaryKind::SaturatingAdd
                                | CheckedIntegerBinaryKind::WrappingSubtract
                                | CheckedIntegerBinaryKind::SaturatingSubtract
                                | CheckedIntegerBinaryKind::WrappingMultiply
                                | CheckedIntegerBinaryKind::SaturatingMultiply
                                | CheckedIntegerBinaryKind::WrappingDivide
                                | CheckedIntegerBinaryKind::SaturatingDivide
                                | CheckedIntegerBinaryKind::WrappingRemainder
                                | CheckedIntegerBinaryKind::SaturatingRemainder
                        ));
                    (supported
                        && is_integer(primitive_type)
                        && scalar_expression_type(&right) == Some(primitive_type))
                    .then_some((
                        CheckedScalarExpression::IntegerBinary {
                            kind,
                            primitive_type,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        domain,
                    ))
                }
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                    ) && operator_is_builtin(operators, expression) =>
                {
                    let (left, left_domain) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    )?;
                    let (right, _) = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    )?;
                    let primitive_type = scalar_expression_type(&left)?;
                    let right_type = scalar_expression_type(&right)?;
                    let kind = checked_integer_binary_kind(binary.operator, left_domain)?;
                    (primitive_type != PrimitiveType::Addr
                        && right_type != PrimitiveType::Addr
                        && is_integer(primitive_type)
                        && is_integer(right_type)
                        && matches!(
                            kind,
                            CheckedIntegerBinaryKind::WrappingShiftLeft
                                | CheckedIntegerBinaryKind::WrappingShiftRight
                                | CheckedIntegerBinaryKind::ExactShiftLeft
                                | CheckedIntegerBinaryKind::ExactShiftRight
                        ))
                    .then_some((
                        CheckedScalarExpression::IntegerBinary {
                            kind,
                            primitive_type,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        left_domain,
                    ))
                }
                _ => None,
            }
        }

        fn lower_structural_float_field(
            program: &TypedTrees,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<(PrimitiveType, CheckedStructuralParameterField)> {
            let mut path = Vec::new();
            let parameter_position =
                structural_parameter_field_path(program, parameters, expression, &mut path)?;
            if path.is_empty() {
                return None;
            }
            let primitive_type =
                path_primitive_type(program, parameters, parameter_position, &path)?;
            matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64).then_some((
                primitive_type,
                CheckedStructuralParameterField {
                    parameter_position,
                    path,
                },
            ))
        }

        fn lower_structural_case_membership(
            program: &TypedTrees,
            machine: &typed_trees::machine::Machine,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            else {
                return None;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::CaseMembership
            ) {
                return None;
            }
            let state = program.machine_states(machine).first()?;
            if !validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return None;
            }
            let membership = validation::has_exact_case_membership_meaning(
                program,
                machine,
                Some(state),
                expression,
                binary,
            );
            let classifier = |candidate: ExpressionHandle| {
                let (case_symbol, literal_owner) =
                    match program.expression_table.expression(candidate) {
                        ExpressionNode::Name(name) => (name.symbol, None),
                        ExpressionNode::StructLiteral(literal)
                            if program
                                .expression_table
                                .struct_fields(literal.fields)
                                .is_empty() =>
                        {
                            (literal.case_symbol?, Some(literal.type_symbol))
                        }
                        _ => return None,
                    };
                program.data_definitions().iter().find_map(|data| {
                    if literal_owner.is_some_and(|owner| owner != data.symbol)
                        || (!membership
                            && program.data_members(data).iter().any(|member| {
                                matches!(member, typed_trees::data::DataMember::Field(_))
                            }))
                    {
                        return None;
                    }
                    program.data_members(data).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(variant) = member else {
                            return None;
                        };
                        (variant.symbol == case_symbol
                            && (membership || program.data_payload_fields(variant).is_empty()))
                        .then(|| {
                            (
                                data.symbol,
                                variant
                                    .identity
                                    .map(|identity| format!("#{identity}"))
                                    .unwrap_or_else(|| variant.name.as_str().to_owned()),
                            )
                        })
                    })
                })
            };
            let (subject_expression, data_symbol, case) =
                if let Some((data, case)) = classifier(binary.right) {
                    (binary.left, data, case)
                } else if let Some((data, case)) = classifier(binary.left) {
                    (binary.right, data, case)
                } else {
                    return None;
                };
            let mut path = Vec::new();
            let parameter_position = structural_parameter_field_path(
                program,
                parameters,
                subject_expression,
                &mut path,
            )?;
            let subject_type = path_type_reference(program, parameters, parameter_position, &path)?;
            (structural_data(program, subject_type)?.symbol == data_symbol).then_some(
                CheckedBooleanExpression::StructuralCaseMembership {
                    subject: CheckedStructuralParameterField {
                        parameter_position,
                        path,
                    },
                    case,
                },
            )
        }

        if let Some(membership) =
            lower_structural_case_membership(program, machine, parameters, expression)
        {
            return Some(membership);
        }

        let mut path = Vec::new();
        if let Some(parameter_position) =
            structural_parameter_field_path(program, parameters, expression, &mut path)
            && !path.is_empty()
            && path_primitive_type(program, parameters, parameter_position, &path)
                == Some(PrimitiveType::Bool)
        {
            return Some(CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            });
        }

        if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            )
            && let Some(equality) =
                lower_structural_equality(program, parameters, binary.left, binary.right)
        {
            return Some(if binary.operator == BinaryOperator::NotEqual {
                CheckedBooleanExpression::Not(Box::new(equality))
            } else {
                equality
            });
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Name(name) => {
                let name_symbol = name.symbol.is_valid().then_some(name.symbol).or_else(|| {
                    program
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        .iter()
                        .copied()
                        .find(|symbol| symbol.is_valid())
                });
                let name_text = program
                    .expression_table
                    .name_path_members(name.members)
                    .last();
                let source_position = parameters.iter().position(|parameter| {
                    name_symbol.is_some_and(|symbol| parameter.symbol == symbol)
                        || name_text.is_some_and(|text| parameter.name == *text)
                })?;
                let parameter = parameters.get(source_position)?;
                if parameter.relevance.is_erased() {
                    return None;
                }
                if parameter.is_mutable {
                    return (crate::values::mutable_scalar_parameter_type(program, parameter)
                        == Some(PrimitiveType::Bool)
                        && name.symbol == parameter.symbol
                        && name.head_symbol == parameter.symbol)
                        .then_some(CheckedBooleanExpression::StorageRead {
                            symbol: parameter.symbol,
                        });
                }
                (program.primitive_type_reference(parameter.type_reference)
                    == Some(PrimitiveType::Bool))
                .then(|| CheckedBooleanExpression::Parameter {
                    position: parameters[..source_position]
                        .iter()
                        .filter(|parameter| {
                            crate::values::scalar::occupies_scalar_position(program, parameter)
                        })
                        .count(),
                })
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(operators, expression) =>
            {
                Some(CheckedBooleanExpression::Not(Box::new(
                    lower_structural_boolean_expression(
                        program,
                        machine,
                        operators,
                        parameters,
                        unary.operand,
                    )?,
                )))
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) && operator_is_builtin(operators, expression) =>
            {
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) && let Some((primitive_type, left)) =
                    lower_structural_float_field(program, parameters, binary.left)
                    && let Some((right_type, right)) =
                        lower_structural_float_field(program, parameters, binary.right)
                    && primitive_type == right_type
                {
                    let (mut left, mut right) = (left, right);
                    if left > right {
                        std::mem::swap(&mut left, &mut right);
                    }
                    let comparison = CheckedBooleanExpression::IeeeFloatComparison {
                        kind: if binary.operator == BinaryOperator::Equal {
                            CheckedIeeeFloatComparisonKind::Equal
                        } else {
                            CheckedIeeeFloatComparisonKind::NotEqual
                        },
                        primitive_type,
                        left,
                        right,
                    };
                    return Some(comparison);
                }
                let integer_operands = (|| {
                    let left = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.left,
                    );
                    let right = lower_structural_integer_expression(
                        program,
                        operators,
                        parameters,
                        binary.right,
                    );
                    let contextual_literal = |expression: ExpressionHandle,
                                              primitive_type: PrimitiveType|
                     -> Option<(
                        CheckedScalarExpression,
                        ArithmeticDomain,
                    )> {
                        let ExpressionNode::Integer(literal) =
                            program.expression_table.expression(expression)
                        else {
                            return None;
                        };
                        literal.landing().is_none().then(|| {
                            (
                                CheckedScalarExpression::IntegerLiteral {
                                    literal: literal.with_landing(IntegerLanding {
                                        landed_type: landed_for_primitive(primitive_type)
                                            .expect("fixed integer comparison context lands"),
                                        domain: ArithmeticDomain::Exact,
                                    }),
                                },
                                ArithmeticDomain::Exact,
                            )
                        })
                    };
                    let mut left = left?;
                    let mut right = right?;
                    match (
                        scalar_expression_type(&left.0),
                        scalar_expression_type(&right.0),
                    ) {
                        (None, Some(primitive_type)) => {
                            left = contextual_literal(binary.left, primitive_type)?;
                        }
                        (Some(primitive_type), None) => {
                            right = contextual_literal(binary.right, primitive_type)?;
                        }
                        (Some(_), Some(_)) => {}
                        (None, None) => return None,
                    }
                    let left = left.0;
                    let right = right.0;
                    let left_type = scalar_expression_type(&left)?;
                    (is_integer(left_type) && scalar_expression_type(&right)? == left_type)
                        .then_some((left, right))
                })();
                if !matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) || integer_operands.is_some()
                {
                    let (mut left, mut right) = integer_operands?;
                    let (kind, negated) = match binary.operator {
                        BinaryOperator::Equal => (CheckedIntegerComparisonKind::Equal, false),
                        BinaryOperator::NotEqual => (CheckedIntegerComparisonKind::Equal, true),
                        BinaryOperator::Less => (CheckedIntegerComparisonKind::LessThan, false),
                        BinaryOperator::LessOrEqual => {
                            (CheckedIntegerComparisonKind::LessOrEqual, false)
                        }
                        BinaryOperator::Greater => {
                            std::mem::swap(&mut left, &mut right);
                            (CheckedIntegerComparisonKind::LessThan, false)
                        }
                        BinaryOperator::GreaterOrEqual => {
                            std::mem::swap(&mut left, &mut right);
                            (CheckedIntegerComparisonKind::LessOrEqual, false)
                        }
                        _ => return None,
                    };
                    let comparison = CheckedBooleanExpression::IntegerComparison {
                        kind,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                    return Some(if negated {
                        CheckedBooleanExpression::Not(Box::new(comparison))
                    } else {
                        comparison
                    });
                }
                let equality = CheckedBooleanExpression::Equal {
                    left: Box::new(lower_structural_boolean_expression(
                        program,
                        machine,
                        operators,
                        parameters,
                        binary.left,
                    )?),
                    right: Box::new(lower_structural_boolean_expression(
                        program,
                        machine,
                        operators,
                        parameters,
                        binary.right,
                    )?),
                };
                Some(if binary.operator == BinaryOperator::NotEqual {
                    CheckedBooleanExpression::Not(Box::new(equality))
                } else {
                    equality
                })
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                    && operator_is_builtin(operators, expression) =>
            {
                let left = Box::new(lower_structural_boolean_expression(
                    program,
                    machine,
                    operators,
                    parameters,
                    binary.left,
                )?);
                let right = Box::new(lower_structural_boolean_expression(
                    program,
                    machine,
                    operators,
                    parameters,
                    binary.right,
                )?);
                Some(if binary.operator == BinaryOperator::And {
                    CheckedBooleanExpression::And { left, right }
                } else {
                    CheckedBooleanExpression::Or { left, right }
                })
            }
            _ => None,
        }
    }

    if let Some(structural) =
        lower_structural_boolean_expression(program, machine, operators, parameters, expression)
    {
        return Some(structural);
    }
    // A machine parameter without a primitive carrier keeps the scalar path
    // unreachable: those predicates lower through
    // `lower_structural_boolean_expression` instead, and an address-typed
    // equality must never gain a fixed-integer crash meaning.
    parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    // `lower_boolean_expression` resolves `CheckedScalarExpression::Parameter`
    // positions against `parameters`, which lowering consumers read as the
    // dense retained-scalar roster; `[erased]` and structural bindings must
    // stay out of it so their references fall through to the erased/structural
    // branches that still see the authored roster.
    let scalar_parameters = parameters
        .iter()
        .filter(|parameter| crate::values::scalar::occupies_scalar_position(program, parameter))
        .cloned()
        .collect::<Vec<_>>();
    let parameter_types = scalar_parameters
        .iter()
        .map(|parameter| program.primitive_type_reference(parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    lower_boolean_expression(
        program,
        operators,
        expression,
        &scalar_parameters,
        parameters,
        &parameter_types,
        &[],
        exact_integer_casts,
    )
}

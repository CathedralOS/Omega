//! Constant evaluation: arguments.

use super::*;

/// If `type_reference` is a `Base<Args..>` spelling of a fully-monomorphizable
/// generic data definition, record the rewrite-to-plain-name and the (deduped)
/// instantiation. Anything Phase 1 cannot lower completely -- a non-generic base,
/// wrong arity, a non-sluggable argument, or a data shape whose members cannot
/// substitute every parameter occurrence exactly -- is left
/// UNTOUCHED for the existing type-check-only path (skip, never reject).
pub(in crate::generic_data) fn consider_generic_spelling(
    syntax: &mut SyntaxTrees,
    generic_data: &HashMap<String, GenericData>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    type_reference: TypeReferenceHandle,
    rewrites: &mut Vec<PendingRewrite>,
    instantiations: &mut Vec<Instantiation>,
) -> Result<(), Diagnostic> {
    let (base_name, lifetime_arguments, arguments) =
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => (base_name.clone(), lifetime_arguments.clone(), *arguments),
            _ => return Ok(()),
        };
    let base = base_name.as_str().to_string();
    let Some(base_info) = generic_data.get(&base) else {
        return Ok(()); // non-generic base: plan-laid / existing error paths
    };

    let argument_handles: Vec<TypeReferenceHandle> = syntax
        .tables
        .type_references
        .type_reference_handles(arguments)
        .to_vec();
    if argument_handles.len() != base_info.parameter_names.len() {
        return Ok(());
    }
    for ((parameter_name, parameter_type), argument) in base_info
        .parameter_names
        .iter()
        .zip(&base_info.const_parameter_types)
        .zip(&argument_handles)
    {
        let Some(parameter_type) = *parameter_type else {
            if matches!(
                syntax.tables.type_references.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            ) {
                return Err(Diagnostic::error(format!(
                    "generic argument expression for `{base}` is only valid for a const parameter"
                )));
            }
            continue;
        };
        match syntax
            .tables
            .type_references
            .type_reference(*argument)
            .clone()
        {
            TypeReferenceNode::Named(name) => {
                if CanonicalConstValue::from_atom(name.as_str()).is_some() {
                    continue;
                }
                if matches!(
                    syntax
                        .tables
                        .type_references
                        .type_reference(parameter_type),
                    TypeReferenceNode::Named(type_name) if type_name.as_str() == "bool"
                ) && matches!(name.as_str(), "true" | "false")
                {
                    let value = CanonicalConstValue::boolean(name.as_str() == "true");
                    syntax.tables.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(value.atom())),
                    );
                    continue;
                }
                if let Some(value) = const_values.get(name.as_str()) {
                    syntax.tables.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                    continue;
                }
                let Some(definition) = const_definitions.get(name.as_str()) else {
                    continue;
                };
                let value = canonicalize_const_definition(syntax, definition, parameter_type)
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                            "const argument for `{base}::{parameter_name}` is invalid at this index site: {reason}"
                        ))
                    })?;
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.atom())),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, parameter_type),
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "const argument expression for `{base}` is invalid: {reason}"
                    ))
                })?;
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            _ => continue,
        }
    }
    let Some(argument_handles) = canonicalize_monomorphizable_argument_handles(
        syntax,
        base_info,
        &lifetime_arguments,
        &argument_handles,
    ) else {
        return Ok(());
    };
    let Some(argument_names) = monomorphizable_argument_slugs(syntax, &argument_handles) else {
        return Ok(());
    };
    if !const_arguments_fit_declarations(syntax, base_info, &argument_handles) {
        // Leave malformed/out-of-range const applications intact so the normal
        // declaration-aware validator emits its precise diagnostic.
        return Ok(());
    }
    if !base_is_fully_monomorphizable(syntax, generic_data, base_info) {
        return Ok(());
    }

    let synthetic_name = format!("{base}<{}>", argument_names.join(", "));
    rewrites.push(PendingRewrite {
        type_reference,
        synthetic_name: synthetic_name.clone(),
        lifetime_arguments,
    });
    if !instantiations
        .iter()
        .any(|instance| instance.synthetic_name == synthetic_name)
    {
        instantiations.push(Instantiation {
            synthetic_name,
            base_name: base,
            argument_handles,
        });
    }
    Ok(())
}

pub(in crate::generic_data) fn const_arguments_fit_declarations(
    syntax: &SyntaxTrees,
    base_info: &GenericData,
    arguments: &[TypeReferenceHandle],
) -> bool {
    base_info
        .const_parameter_types
        .iter()
        .zip(arguments)
        .all(|(parameter_type, argument)| {
            let Some(parameter_type) = parameter_type else {
                return true;
            };
            let TypeReferenceNode::Named(value) =
                syntax.tables.type_references.type_reference(*argument)
            else {
                return false;
            };
            let TypeReferenceNode::Named(type_name) = syntax
                .tables
                .type_references
                .type_reference(*parameter_type)
            else {
                return false;
            };
            if let Some(value) = CanonicalConstValue::from_atom(value.as_str()) {
                return value.type_name == type_name.as_str();
            }
            let Ok(value) = value.as_str().parse::<i128>() else {
                return false;
            };
            let (minimum, maximum) = match type_name.as_str() {
                "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
                "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
                "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
                "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
                "u8" => (0, i128::from(u8::MAX)),
                "u16" => (0, i128::from(u16::MAX)),
                "u32" => (0, i128::from(u32::MAX)),
                "u64" | "addr" => (0, i128::from(u64::MAX)),
                _ => return false,
            };
            value >= minimum && value <= maximum
        })
}

/// Evaluate the symbolic integer subset retained in a const-generic argument.
/// Names resolve to literal scoped const declarations collected above.
/// Arithmetic deliberately matches the closed-expression parser fold over the
/// current signed/unsigned 64-bit envelope. Shifts and bitwise operations use
/// the matched const parameter's declared width and signedness.
pub(in crate::generic_data) fn evaluate_const_argument_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
    integer_type: Option<ConstIntegerType>,
) -> Result<EvaluatedConst, String> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(EvaluatedConst::Concrete)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Name(path) => {
            let name = syntax
                .expressions
                .identifier_path_members(*path)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if let Some(value) = parameter_values
                .get(&name)
                .or_else(|| const_values.get(&name))
            {
                Ok(EvaluatedConst::Concrete(*value))
            } else if symbolic_parameters.contains(&name) {
                Ok(EvaluatedConst::Symbolic(name))
            } else {
                Err(format!("`{name}` is not a scoped integer const"))
            }
        }
        ExpressionNode::Binary(binary) => {
            let left = evaluate_const_argument_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
            )?;
            let right = evaluate_const_argument_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
            )?;
            match (binary.operator, &right) {
                (BinaryOperator::Divide | BinaryOperator::Modulo, EvaluatedConst::Concrete(0)) => {
                    return Err(match binary.operator {
                        BinaryOperator::Divide => "division by zero is invalid".to_string(),
                        _ => "remainder by zero is invalid".to_string(),
                    });
                }
                (
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight,
                    EvaluatedConst::Concrete(amount),
                ) if *amount < 0 || *amount >= i128::from(u64::BITS) => {
                    return Err(match binary.operator {
                        BinaryOperator::ShiftLeft => {
                            "left shift exceeds the `u64` width".to_string()
                        }
                        _ => "right shift exceeds the `u64` width".to_string(),
                    });
                }
                (
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor,
                    _,
                ) => {}
                _ => {
                    return Err(
                        "only integer arithmetic, shifts, and bitwise operators are supported"
                            .to_string(),
                    );
                }
            }
            let (EvaluatedConst::Concrete(left), EvaluatedConst::Concrete(right)) = (&left, &right)
            else {
                return Ok(left.or_symbolic(right));
            };
            let (left, right) = (*left, *right);
            match binary.operator {
                BinaryOperator::Add => checked_evaluated_const(left.checked_add(right), "addition"),
                BinaryOperator::Subtract => {
                    checked_evaluated_const(left.checked_sub(right), "subtraction")
                }
                BinaryOperator::Multiply => {
                    checked_evaluated_const(left.checked_mul(right), "multiplication")
                }
                BinaryOperator::Divide => left
                    .checked_div(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "division by zero is invalid".to_string()),
                BinaryOperator::Modulo => left
                    .checked_rem(right)
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| "remainder by zero is invalid".to_string()),
                BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor => {
                    evaluate_declared_width_operation(binary.operator, left, right, integer_type)
                        .map(EvaluatedConst::Concrete)
                }
                _ => unreachable!("const integer operator was validated above"),
            }
        }
        _ => Err("expression is not a symbolic integer const expression".to_string()),
    }
}

#[derive(Clone, Copy)]
pub(in crate::generic_data) struct ConstIntegerType {
    name: &'static str,
    bits: u32,
    signed: bool,
}

pub(in crate::generic_data) fn const_integer_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Option<ConstIntegerType> {
    let TypeReferenceNode::Named(name) =
        syntax.tables.type_references.type_reference(type_reference)
    else {
        return None;
    };
    Some(match name.as_str() {
        "i8" => ConstIntegerType {
            name: "i8",
            bits: 8,
            signed: true,
        },
        "i16" => ConstIntegerType {
            name: "i16",
            bits: 16,
            signed: true,
        },
        "i32" => ConstIntegerType {
            name: "i32",
            bits: 32,
            signed: true,
        },
        "i64" => ConstIntegerType {
            name: "i64",
            bits: 64,
            signed: true,
        },
        "u8" => ConstIntegerType {
            name: "u8",
            bits: 8,
            signed: false,
        },
        "u16" => ConstIntegerType {
            name: "u16",
            bits: 16,
            signed: false,
        },
        "u32" => ConstIntegerType {
            name: "u32",
            bits: 32,
            signed: false,
        },
        "u64" => ConstIntegerType {
            name: "u64",
            bits: 64,
            signed: false,
        },
        "addr" => ConstIntegerType {
            name: "addr",
            bits: 64,
            signed: false,
        },
        _ => return None,
    })
}

pub(in crate::generic_data) fn generic_const_integer_types(
    syntax: &SyntaxTrees,
    generic_name: &str,
) -> Vec<Option<ConstIntegerType>> {
    syntax
        .root_items()
        .find_map(|item| {
            let Item::Data(definition) = item else {
                return None;
            };
            (definition.name.as_str() == generic_name).then(|| {
                syntax
                    .tables
                    .items
                    .type_parameters(definition.type_parameters)
                    .iter()
                    .map(|parameter| match parameter.kind {
                        TypeParameterKind::Const { type_reference } => {
                            const_integer_type(syntax, type_reference)
                        }
                        _ => None,
                    })
                    .collect()
            })
        })
        .unwrap_or_default()
}

pub(in crate::generic_data) fn evaluate_declared_width_operation(
    operator: BinaryOperator,
    left: i128,
    right: i128,
    integer_type: Option<ConstIntegerType>,
) -> Result<i128, String> {
    let Some(integer_type) = integer_type else {
        return Err(
            "shifts and bitwise operators require a declared integer const type".to_string(),
        );
    };
    let modulus = 1i128 << integer_type.bits;
    let maximum = if integer_type.signed {
        (modulus >> 1) - 1
    } else {
        modulus - 1
    };
    let minimum = if integer_type.signed {
        -(modulus >> 1)
    } else {
        0
    };
    if left < minimum || left > maximum {
        return Err(format!(
            "left operand `{left}` is outside the declared `{}` range",
            integer_type.name
        ));
    }

    match operator {
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            let amount = u32::try_from(right)
                .ok()
                .filter(|amount| *amount < integer_type.bits)
                .ok_or_else(|| {
                    format!(
                        "shift count must be non-negative and below the declared `{}` width",
                        integer_type.name
                    )
                })?;
            if operator == BinaryOperator::ShiftRight {
                // `i128 >>` sign-extends negative signed operands. Unsigned
                // operands were range-checked non-negative, for which the same
                // operation is the required logical shift.
                return Ok(left >> amount);
            }
            let shifted = left.checked_shl(amount).ok_or_else(|| {
                format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name
                )
            })?;
            if shifted < minimum || shifted > maximum {
                return Err(format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name
                ));
            }
            Ok(shifted)
        }
        BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor => {
            if right < minimum || right > maximum {
                return Err(format!(
                    "right operand `{right}` is outside the declared `{}` range",
                    integer_type.name
                ));
            }
            let mask = modulus - 1;
            let left_bits = left & mask;
            let right_bits = right & mask;
            let result_bits = match operator {
                BinaryOperator::BitwiseAnd => left_bits & right_bits,
                BinaryOperator::BitwiseOr => left_bits | right_bits,
                BinaryOperator::BitwiseXor => left_bits ^ right_bits,
                _ => unreachable!(),
            };
            if integer_type.signed && result_bits >= modulus >> 1 {
                Ok(result_bits - modulus)
            } else {
                Ok(result_bits)
            }
        }
        _ => unreachable!("caller provides only shifts and bitwise operators"),
    }
}

#[derive(Debug)]
pub(in crate::generic_data) enum EvaluatedConst {
    Concrete(i128),
    Symbolic(String),
}

pub(in crate::generic_data) fn checked_evaluated_const(
    value: Option<i128>,
    operation: &str,
) -> Result<EvaluatedConst, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(EvaluatedConst::Concrete)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}

impl EvaluatedConst {
    pub(in crate::generic_data) fn into_concrete(self) -> Result<i128, String> {
        match self {
            Self::Concrete(value) => Ok(value),
            Self::Symbolic(name) => Err(format!(
                "`{name}` is a const parameter that has no binding at this use"
            )),
        }
    }

    fn or_symbolic(self, other: Self) -> Self {
        match self {
            Self::Symbolic(_) => self,
            Self::Concrete(_) => other,
        }
    }
}

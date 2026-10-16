//! Constant evaluation: arguments.
use super::super::{
    BinaryOperator, CanonicalConstValue, ConstDefinition, Diagnostic, ExpressionHandle,
    ExpressionNode, HashMap, HashSet, Identifier, SyntaxTrees, TypeParameterKind,
    TypeReferenceHandle, TypeReferenceNode,
};
use crate::preparation::generic_data::eligibility::base_is_fully_monomorphizable;
use symbols::JoinedPath;

use crate::preparation::generic_data::ConstScalarSpelling;
use crate::preparation::generic_data::GenericData;
use crate::preparation::generic_data::discovery::ClosedApplication;

use crate::preparation::generic_data::canonicalize_monomorphizable_argument_handles;
use crate::preparation::generic_data::closed_argument_identity;
use crate::preparation::generic_data::const_evaluation::validate_anonymous_remainder;
use crate::preparation::generic_data::const_integer_in_envelope;
use crate::preparation::generic_data::integer_literal_value;
use crate::preparation::generic_data::monomorphizable_argument_slugs;
use crate::preparation::generic_data::selected_generic_data;
use crate::preparation::generic_data::syntax_type_identity;
use crate::preparation::generic_data::validate_syntax_integer_range;
use numerics::literals::LandedIntegerType;

/// If `type_reference` is a `Base<Args..>` spelling of a fully-monomorphizable
/// generic data definition, prepare its closed argument tuple. The synthesis
/// owner decides declaration demand and reuse; argument evaluation never owns
/// the instance roster. Anything Phase 1 cannot lower completely -- a non-generic base,
/// wrong arity, a non-sluggable argument, or a data shape whose members cannot
/// substitute every parameter occurrence exactly -- is left
/// UNTOUCHED for the existing type-check-only path (skip, never reject).
pub(in crate::preparation::generic_data) fn prepare_closed_application(
    syntax: &mut SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    type_reference: TypeReferenceHandle,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<ClosedApplication>, Diagnostic> {
    let (base_name, lifetime_arguments, arguments) =
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => (base_name.clone(), lifetime_arguments.clone(), *arguments),
            _ => return Ok(None),
        };
    let base = base_name.as_str().to_string();
    let Some(base_info) = selected_generic_data(syntax, generic_data, selection, &base_name) else {
        return Ok(None); // non-generic base: plan-laid / existing error paths
    };

    let mut argument_handles: Vec<TypeReferenceHandle> = syntax
        .tables
        .type_references
        .type_reference_handles(arguments)
        .to_vec();
    // Omitted trailing binders are recoverable only through the template's
    // structural type equations; an excess or an unsolvable shortfall stays
    // with the declaration-aware validator's arity diagnostic.
    let parameter_count = base_info.parameter_names.len();
    if argument_handles.len() > parameter_count
        || (argument_handles.len() < parameter_count && base_info.type_equations.is_empty())
    {
        return Ok(None);
    }
    if base_info
        .const_parameter_types
        .iter()
        .zip(&argument_handles)
        .any(|(parameter_type, argument)| {
            parameter_type.is_some()
                && matches!(syntax.tables.type_references.type_reference(*argument),
                    TypeReferenceNode::ConstExpression(expression)
                        if super::anonymous::requires_const_operator_selection(syntax, *expression))
        })
    {
        return Ok(None);
    }
    // Carrier admission precedes erasing a named argument. Its declared type
    // belongs to the caller, while a carrier such as `[Value; Count]` belongs
    // to this exact receiving telescope. The existing evaluator keeps those
    // scopes distinct while descending the original carrier structure.
    let application_scope = base_info
        .parameter_names
        .iter()
        .zip(&argument_handles)
        .map(|(parameter, argument)| (parameter.clone(), *argument))
        .collect::<crate::preparation::declaration_selection::GenericApplicationSubstitution>();
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
                match ConstScalarSpelling::from_bare_name(name.as_str()) {
                    Some(ConstScalarSpelling::Canonical(_) | ConstScalarSpelling::Integer(_)) => {
                        continue;
                    }
                    Some(ConstScalarSpelling::Boolean(value))
                        if matches!(
                            syntax
                                .tables
                                .type_references
                                .type_reference(syntax.type_references.type_reference(parameter_type).as_named()
                                    .and_then(|name| application_scope.get(name.as_str()).copied())
                                    .unwrap_or(parameter_type)),
                            TypeReferenceNode::Named(type_name) if type_name.as_str() == "bool"
                        ) =>
                    {
                        let value = CanonicalConstValue::boolean(value);
                        syntax.tables.type_references.replace_type_reference(
                            *argument,
                            TypeReferenceNode::Named(Identifier::generated(value.atom())),
                        );
                        continue;
                    }
                    // A boolean literal against a non-bool parameter is not a
                    // value here; it falls through to selection like any name.
                    Some(ConstScalarSpelling::Boolean(_)) | None => {}
                }
                if let Some(selection) = selection {
                    // These positions belong only to concrete owners; open
                    // generic binders are excluded by the owner traversal.
                    // An unresolved selection must not use the lexical map.
                    let Some(definition) = selection.select(syntax, &name)? else {
                        continue;
                    };
                    let value = super::values::canonicalize_selected_const_definition_in_application(syntax, &definition, parameter_type, Some(selection), &application_scope)
                        .map_err(|reason| {
                            Diagnostic::error(format!(
                                "const argument for `{base}::{parameter_name}` is invalid at this index site: {reason}"
                            )).with_source_span(name.source_span())
                        })?;
                    let replacement = match value.decode_encoding() {
                        Some(
                            language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                                value,
                                ..
                            },
                        ) => value.to_string(),
                        _ => value.atom(),
                    };
                    syntax
                        .tables
                        .type_references
                        .retain_const_argument_normalization(
                            *argument,
                            name.source_span(),
                            value.encoding.clone(),
                            [syntax_trees::types::ConstArgumentOrigin {
                                reference: name.source_span(),
                                declaration: definition.name.source_span(),
                                initializer: syntax.expressions.source_span(definition.value),
                                evaluated_value: value.encoding.into(),
                            }],
                            [],
                        );
                    syntax.tables.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(replacement)),
                    );
                    continue;
                }
                super::module_constant_selection_gate(
                    syntax,
                    &name,
                    const_values,
                    const_definitions,
                )?;
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
                let value = super::values::canonicalize_selected_const_definition_in_application(syntax, definition, parameter_type, None, &application_scope)
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
                if matches!(
                    syntax.expressions.expression(expression),
                    syntax_trees::expression::ExpressionNode::StructLiteral(_)
                        | syntax_trees::expression::ExpressionNode::ArrayLiteral(_)
                ) {
                    let value =
                        super::values::canonicalize_selected_index_expression_in_application(
                            syntax,
                            parameter_type,
                            expression,
                            selection,
                            &application_scope,
                        )
                        .map_err(|reason| {
                            Diagnostic::error(reason)
                                .with_source_span(syntax.expressions.source_span(expression))
                        })?;
                    let reference = syntax.expressions.source_span(expression);
                    syntax.type_references.retain_const_argument_normalization(
                        *argument,
                        reference,
                        value.encoding.clone(),
                        [],
                        [],
                    );
                    syntax
                        .type_references
                        .retain_const_argument_expression(*argument, expression);
                    syntax.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(value.atom())),
                    );
                    continue;
                }
                // A `Call` root surviving to synthesis is a pre-check
                // continuation: the probe already declined it because its
                // closure needs Omega's provider plan. Keep the authored
                // application for the semantic evaluation owner's deferred
                // fold rather than reporting it as a malformed argument.
                if matches!(
                    syntax.expressions.expression(expression),
                    syntax_trees::expression::ExpressionNode::Call(_)
                ) {
                    continue;
                }
                let destination =
                    syntax_type_identity(syntax, parameter_type).map_err(Diagnostic::error)?;
                let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, parameter_type),
                    warnings,
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "const argument expression for `{base}` is invalid for `{destination}`: {reason}"
                    )).with_source_span(syntax.expressions.source_span(expression))
                })?;
                if let Some(integer_type) = const_integer_type(syntax, parameter_type) {
                    validate_syntax_integer_range(integer_type.name(), value)
                        .map_err(Diagnostic::error)?;
                }
                syntax.tables.type_references.replace_type_reference(
                    *argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            _ => continue,
        }
    }
    if !base_info.type_equations.is_empty()
        && base_is_fully_monomorphizable(syntax, generic_data, selection, base_info)
    {
        // Every equation is decided against the complete tuple: omitted
        // trailing binders bind from the supplied arguments' retained
        // canonical ranges, explicit arguments are verified, and the
        // authored spelling grows to the complete application so the
        // retained origin and closed identity see every binder.
        let completed = super::super::complete_argument_tuple(
            syntax,
            base_info,
            &base_name,
            &argument_handles,
            const_values,
            selection,
            warnings,
        )?;
        if completed.len() != argument_handles.len() {
            let arguments = syntax
                .tables
                .type_references
                .insert_type_reference_handles(completed.iter().copied());
            syntax.tables.type_references.replace_type_reference(
                type_reference,
                TypeReferenceNode::Generic {
                    base_name: base_name.clone(),
                    lifetime_arguments: lifetime_arguments.clone(),
                    arguments,
                },
            );
            argument_handles = completed;
        }
    } else if argument_handles.len() < parameter_count {
        return Ok(None);
    }
    let Some(argument_handles) = canonicalize_monomorphizable_argument_handles(
        syntax,
        base_info,
        &lifetime_arguments,
        &argument_handles,
    ) else {
        return Ok(None);
    };
    let Some(argument_names) = monomorphizable_argument_slugs(syntax, &argument_handles, selection)
    else {
        return Ok(None);
    };
    if !const_arguments_fit_declarations(syntax, base_info, &argument_handles) {
        // Leave malformed/out-of-range const applications intact so the normal
        // declaration-aware validator emits its precise diagnostic.
        return Ok(None);
    }
    if !base_is_fully_monomorphizable(syntax, generic_data, selection, base_info) {
        return Ok(None);
    }

    let Some(argument_identity) = argument_handles
        .iter()
        .zip(&base_info.const_parameter_types)
        .map(|(argument, parameter)| {
            closed_argument_identity(syntax, selection, *argument, parameter.is_some())
        })
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    Ok(Some(ClosedApplication {
        base_name,
        lifetime_arguments,
        template: base_info.declaration,
        argument_handles,
        argument_identity,
        argument_names,
    }))
}

pub(in crate::preparation::generic_data) fn const_arguments_fit_declarations(
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
            let Some(value) = syntax
                .tables
                .type_references
                .type_reference(*argument)
                .as_named()
            else {
                return false;
            };
            let Some(type_name) = syntax
                .tables
                .type_references
                .type_reference(*parameter_type)
                .as_named()
            else {
                return false;
            };
            let value = match ConstScalarSpelling::from_bare_name(value.as_str()) {
                Some(ConstScalarSpelling::Canonical(value)) => {
                    return value.type_name == type_name.as_str();
                }
                Some(ConstScalarSpelling::Integer(value)) => value,
                Some(ConstScalarSpelling::Boolean(_)) | None => return false,
            };
            let Some(integer_type) = LandedIntegerType::from_name(type_name.as_str()) else {
                return false;
            };
            let (minimum, maximum) = integer_type.bounds();
            value >= minimum && value <= maximum
        })
}

/// Evaluate the symbolic integer subset retained in a const-generic argument.
/// Names resolve to literal scoped const declarations collected above.
/// Anonymous arithmetic retains rational intermediates until its integer
/// landing. Already-typed arithmetic keeps the current signed/unsigned 64-bit
/// envelope. Shifts and bitwise operations use the matched const parameter's
/// declared width and signedness.
pub(in crate::preparation::generic_data) fn evaluate_const_argument_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    const_values: &HashMap<String, i128>,
    parameter_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
    integer_type: Option<LandedIntegerType>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<EvaluatedConst, String> {
    if let Some(value) =
        super::anonymous::evaluate_anonymous_integer_argument(syntax, expression, warnings)?
    {
        return Ok(EvaluatedConst::Concrete(value));
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(EvaluatedConst::Concrete)
            .ok_or_else(|| {
                "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
            }),
        ExpressionNode::Name(path) => {
            let members = syntax.expressions.identifier_path_members(*path);
            let name = members.joined_path();
            if let Some(value) = parameter_values.get(&name) {
                Ok(EvaluatedConst::Concrete(*value))
            } else if symbolic_parameters.contains(&name) {
                Ok(EvaluatedConst::Symbolic(name))
            } else {
                crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                    syntax,
                    &name,
                    members
                        .first()
                        .map(|member| member.source_span())
                        .unwrap_or_default(),
                )?;
                const_values
                    .get(&name)
                    .copied()
                    .map(EvaluatedConst::Concrete)
                    .ok_or_else(|| format!("`{name}` is not a scoped integer const"))
            }
        }
        ExpressionNode::Binary(binary) => {
            if !super::anonymous::has_builtin_const_operator(syntax, binary) {
                return Err("const operator application requires declaration selection".to_owned());
            }
            validate_anonymous_remainder(syntax, binary)?;
            let left = evaluate_const_argument_expression(
                syntax,
                binary.left,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
                warnings,
            )?;
            let right = evaluate_const_argument_expression(
                syntax,
                binary.right,
                const_values,
                parameter_values,
                symbolic_parameters,
                integer_type,
                warnings,
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

pub(in crate::preparation::generic_data) fn const_integer_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> Option<LandedIntegerType> {
    let name = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .as_named()?;
    LandedIntegerType::from_name(name.as_str())
}

/// Reject a value outside its declared integer carrier's range.
pub(crate) fn validate_declared_integer(
    integer_type: LandedIntegerType,
    value: i128,
) -> Result<(), String> {
    let (minimum, maximum) = integer_type.bounds();
    if value < minimum || value > maximum {
        return Err(format!(
            "integer value `{value}` is outside the declared `{}` range",
            integer_type.name()
        ));
    }
    Ok(())
}

pub(in crate::preparation::generic_data) fn generic_const_integer_types(
    syntax: &SyntaxTrees,
    generic_name: &str,
) -> Vec<Option<LandedIntegerType>> {
    syntax
        .root_items()
        .find_map(|item| {
            let definition = item.as_data()?;
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

pub(in crate::preparation::generic_data) fn evaluate_declared_width_operation(
    operator: BinaryOperator,
    left: i128,
    right: i128,
    integer_type: Option<LandedIntegerType>,
) -> Result<i128, String> {
    let Some(integer_type) = integer_type else {
        return Err(
            "shifts and bitwise operators require a declared integer const type".to_string(),
        );
    };
    let modulus = 1i128 << integer_type.bit_width();
    let (minimum, maximum) = integer_type.bounds();
    if left < minimum || left > maximum {
        return Err(format!(
            "left operand `{left}` is outside the declared `{}` range",
            integer_type.name()
        ));
    }

    match operator {
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            let amount = u32::try_from(right)
                .ok()
                .filter(|amount| *amount < integer_type.bit_width())
                .ok_or_else(|| {
                    format!(
                        "shift count must be non-negative and below the declared `{}` width",
                        integer_type.name()
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
                    integer_type.name()
                )
            })?;
            if shifted < minimum || shifted > maximum {
                return Err(format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name()
                ));
            }
            Ok(shifted)
        }
        BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor => {
            if right < minimum || right > maximum {
                return Err(format!(
                    "right operand `{right}` is outside the declared `{}` range",
                    integer_type.name()
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
            if integer_type.is_signed() && result_bits >= modulus >> 1 {
                Ok(result_bits - modulus)
            } else {
                Ok(result_bits)
            }
        }
        _ => unreachable!("caller provides only shifts and bitwise operators"),
    }
}

#[derive(Debug)]
pub(in crate::preparation::generic_data) enum EvaluatedConst {
    Concrete(i128),
    Symbolic(String),
}

pub(in crate::preparation::generic_data) fn checked_evaluated_const(
    value: Option<i128>,
    operation: &str,
) -> Result<EvaluatedConst, String> {
    value
        .and_then(const_integer_in_envelope)
        .map(EvaluatedConst::Concrete)
        .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
}

impl EvaluatedConst {
    pub(in crate::preparation::generic_data) fn into_concrete(self) -> Result<i128, String> {
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

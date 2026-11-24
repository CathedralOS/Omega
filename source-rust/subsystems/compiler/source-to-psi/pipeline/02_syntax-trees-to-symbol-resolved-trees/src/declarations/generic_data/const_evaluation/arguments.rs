use super::BigInt;
/* Constant evaluation: arguments. */
use super::super::{
    BinaryOperator, CanonicalConstValue, Diagnostic, ExpressionHandle, ExpressionNode, HashMap,
    HashSet, Identifier, SyntaxTrees, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
};
use crate::resolution::declarations::generic_data::eligibility::base_is_fully_monomorphizable;
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;
use syntax_trees::identifier::JoinedSyntaxPath;

use crate::resolution::declarations::generic_data::ConstScalarSpelling;
use crate::resolution::declarations::generic_data::GenericData;
use crate::resolution::declarations::generic_data::discovery::ClosedApplication;

use super::anonymous::evaluate_anonymous_integer_argument;
use crate::resolution::declarations::generic_data::canonicalize_monomorphizable_argument_handles;
use crate::resolution::declarations::generic_data::closed_argument_identity;
use crate::resolution::declarations::generic_data::const_evaluation::validate_anonymous_remainder;
use crate::resolution::declarations::generic_data::const_integer_within_budget;
use crate::resolution::declarations::generic_data::integer_literal_value;
use crate::resolution::declarations::generic_data::monomorphizable_argument_slugs;
use crate::resolution::declarations::generic_data::selected_generic_data;
use crate::resolution::declarations::generic_data::syntax_type_identity;
use crate::resolution::declarations::generic_data::validate_syntax_integer_range;
use application_identity::const_value::DecodedCanonicalConstValue;
use numeric_literals::LandedIntegerType;

/*
    If `type_reference` is a `Base<Args..>` spelling of a fully-monomorphizable
    generic data definition, prepare its closed argument tuple. The synthesis
    owner decides declaration demand and reuse; argument evaluation never owns
    the instance roster. Anything Phase 1 cannot lower completely -- a non-generic base,
    wrong arity, a non-sluggable argument, or a data shape whose members cannot
    substitute every parameter occurrence exactly -- is left
    UNTOUCHED for the existing type-check-only path (skip, never reject).
*/
pub(in crate::resolution::declarations::generic_data) fn prepare_closed_application(
    syntax: &mut SyntaxTrees,
    generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
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
    let base = base_name.as_str(syntax).to_string();
    let Some(base_info) = selected_generic_data(syntax, generic_data, selection, &base_name) else {
        return Ok(None);
    };

    let mut argument_handles: Vec<TypeReferenceHandle> = syntax_type_references(syntax)
        .type_reference_handles(arguments)
        .to_vec();
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
    let application_scope = base_info
        .parameter_names
        .iter()
        .zip(&argument_handles)
        .map(|(parameter, argument)| (parameter.clone(), *argument))
        .collect::<crate::resolution::declarations::declaration_selection::GenericApplicationSubstitution>();
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
        match syntax_type_references(syntax)
            .type_reference(*argument)
            .clone()
        {
            TypeReferenceNode::Named(name) => {
                match ConstScalarSpelling::from_bare_name(name.as_str(syntax)) {
                    Some(ConstScalarSpelling::Canonical(_) | ConstScalarSpelling::Integer(_)) => {
                        continue;
                    }
                    Some(ConstScalarSpelling::Boolean(value))
                        if matches!(
                            syntax_type_references(syntax).type_reference(syntax.type_references.type_reference(parameter_type).as_named()
                                    .and_then(|name| application_scope.get(name.as_str(syntax)).copied())
                                    .unwrap_or(parameter_type)),
                            TypeReferenceNode::Named(type_name) if type_name.as_str(syntax) == "bool"
                        ) =>
                    {
                        let value = CanonicalConstValue::boolean(value);
                        syntax.tables.type_references.replace_type_reference(
                            *argument,
                            TypeReferenceNode::Named(Identifier::generated(value.atom())),
                        );
                        continue;
                    }
                    Some(ConstScalarSpelling::Boolean(_)) | None => {}
                }
                if let Some(selection) = selection {
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
                        Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                            value.to_string()
                        }
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
                                initializer: syntax.expressions.source_span(definition.value()),
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
                        .tables
                        .type_references
                        .retain_const_argument_expression(*argument, expression);
                    syntax.type_references.replace_type_reference(
                        *argument,
                        TypeReferenceNode::Named(Identifier::generated(value.atom())),
                    );
                    continue;
                }
                if matches!(
                    syntax.expressions.expression(expression),
                    syntax_trees::expression::ExpressionNode::Call(_)
                ) {
                    return Ok(None);
                }
                let destination =
                    syntax_type_identity(syntax, parameter_type).map_err(Diagnostic::error)?;
                let value = evaluate_const_argument_expression(
                syntax,
                expression,
                selection,
                &HashMap::new(),
                &HashSet::new(),
                const_integer_type(syntax, parameter_type, selection),
                warnings,
            )
            .and_then(EvaluatedConst::into_concrete)
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "const argument expression for `{base}` is invalid for `{destination}`: {reason}"
                )).with_source_span(syntax.expressions.source_span(expression))
            })?;
                if let Some(integer_type) = const_integer_type(syntax, parameter_type, selection) {
                    validate_syntax_integer_range(integer_type, &value)
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
        let completed = super::super::complete_argument_tuple(
            syntax,
            base_info,
            &base_name,
            &argument_handles,
            selection,
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
    if !const_arguments_fit_declarations(syntax, base_info, &argument_handles, selection) {
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

pub(in crate::resolution::declarations::generic_data) fn const_arguments_fit_declarations(
    syntax: &SyntaxTrees,
    base_info: &GenericData,
    arguments: &[TypeReferenceHandle],
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> bool {
    base_info
        .const_parameter_types
        .iter()
        .zip(arguments)
        .all(|(parameter_type, argument)| {
            let Some(parameter_type) = parameter_type else {
                return true;
            };
            let Some(value) = syntax_type_references(syntax)
                .type_reference(*argument)
                .as_named()
            else {
                return false;
            };
            let Some(type_name) = syntax_type_references(syntax)
                .type_reference(*parameter_type)
                .as_named()
            else {
                return false;
            };
            let value = match ConstScalarSpelling::from_bare_name(value.as_str(syntax)) {
                Some(ConstScalarSpelling::Canonical(value)) => {
                    return value.type_name == type_name.as_str(syntax);
                }
                Some(ConstScalarSpelling::Integer(value)) => value,
                Some(ConstScalarSpelling::Boolean(_)) | None => return false,
            };
            let Some(integer_type) = const_integer_type(syntax, *parameter_type, selection) else {
                return false;
            };
            integer_type.value_carrier().admits(&value)
        })
}

/*
    Evaluate the symbolic integer subset retained in a const-generic argument.
    Names use the source-scoped declaration selection shared with resolution;
    unrelated private constants never contribute a value.
    Anonymous arithmetic retains rational intermediates until its integer
    landing. Already-typed arithmetic retains its exact value and selected
    carrier range. Shifts and bitwise operations use the matched const parameter's
    declared width and signedness.
*/
pub(in crate::resolution::declarations::generic_data) fn evaluate_const_argument_expression(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    parameter_values: &HashMap<String, BigInt>,
    symbolic_parameters: &HashSet<String>,
    integer_type: Option<LandedIntegerType>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<EvaluatedConst, String> {
    if let Some(value) = evaluate_anonymous_integer_argument(syntax, expression, warnings)? {
        return Ok(EvaluatedConst::Concrete(value));
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Integer(value) => integer_literal_value(value)
            .map(EvaluatedConst::Concrete)
            .ok_or_else(|| "integer operand has no exact const value".to_string()),
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => {
            let members = syntax.expressions.identifier_path_members(path.to_owned());
            let name = members.joined_path_in(syntax);
            if let Some(value) = parameter_values.get(&name) {
                Ok(EvaluatedConst::Concrete(value.clone()))
            } else if symbolic_parameters.contains(&name) {
                Ok(EvaluatedConst::Symbolic(name))
            } else {
                let selection = selection
                    .ok_or_else(|| format!("`{name}` has no constant selection context"))?;
                let first = members.first().ok_or("constant name has no source")?;
                let mut span = first.source_span();
                span.span.end = members
                    .last()
                    .ok_or("constant name has no source")?
                    .source_span()
                    .span
                    .end;
                let identifier = Identifier::new(&name, span);
                let definition = selection
                    .select(syntax, &identifier)
                    .map_err(|diagnostic| diagnostic.message)?
                    .ok_or_else(|| format!("`{name}` is not a scoped integer const"))?;
                let value = super::values::canonicalize_selected_declaration_value(
                    syntax,
                    &definition,
                    Some(selection),
                )?;
                match value.decode_encoding() {
                    Some(DecodedCanonicalConstValue::Integer {
                        integer_type: actual,
                        value,
                    }) => {
                        if let Some(expected) = integer_type
                            && actual != expected
                        {
                            return Err(format!(
                                "constant `{name}` has carrier `{actual}`, expected `{}`",
                                expected.name(),
                            ));
                        }
                        Ok(EvaluatedConst::Concrete(value))
                    }
                    _ => Err(format!("`{name}` is not an integer const")),
                }
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
                selection,
                parameter_values,
                symbolic_parameters,
                integer_type,
                warnings,
            )?;
            let right = evaluate_const_argument_expression(
                syntax,
                binary.right,
                selection,
                parameter_values,
                symbolic_parameters,
                integer_type,
                warnings,
            )?;
            admit_integer_operator(
                binary.operator,
                match &right {
                    EvaluatedConst::Concrete(right) => Some(right),
                    EvaluatedConst::Symbolic(_) => None,
                },
            )?;
            let (EvaluatedConst::Concrete(left), EvaluatedConst::Concrete(right)) = (&left, &right)
            else {
                return Ok(left.or_symbolic(right));
            };
            if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
                return Err(
                    "policy-qualified const arithmetic requires its selected evaluator".to_owned(),
                );
            }
            apply_integer_operator(binary.operator, left, right, integer_type)
                .map(EvaluatedConst::Concrete)
        }
        _ => Err("expression is not a symbolic integer const expression".to_string()),
    }
}

/*
    Whether `operator` has a builtin integer meaning at all, and whether a
    right operand that is already known leaves it defined. The right operand
    is checked even while the left one is still a symbolic const parameter,
    so a template dividing by a literal zero is refused before it is bound.
*/
pub(in crate::resolution::declarations::generic_data) fn admit_integer_operator(
    operator: BinaryOperator,
    right: Option<&BigInt>,
) -> Result<(), String> {
    match (operator, right) {
        (BinaryOperator::Divide, Some(value)) if value.is_zero() => {
            Err("division by zero is invalid".to_string())
        }
        (BinaryOperator::Modulo, Some(value)) if value.is_zero() => {
            Err("remainder by zero is invalid".to_string())
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
        ) => Ok(()),
        _ => {
            Err("only integer arithmetic, shifts, and bitwise operators are supported".to_string())
        }
    }
}

/*
    The builtin integer meaning of one admitted operator over two known
    values. Every constant evaluator in this stage computes through here, so
    addition, typed division and the declared-width shifts and bitwise
    operators have one definition. Values stay exact until the selected
    carrier checks them; host scalar bounds never define their meaning.
*/
pub(in crate::resolution::declarations::generic_data) fn apply_integer_operator(
    operator: BinaryOperator,
    left: &BigInt,
    right: &BigInt,
    integer_type: Option<LandedIntegerType>,
) -> Result<BigInt, String> {
    admit_integer_operator(operator, Some(right))?;
    if let Some(integer_type) = integer_type {
        validate_declared_integer(integer_type, left)?;
        if !matches!(
            operator,
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        ) {
            validate_declared_integer(integer_type, right)?;
        }
    }
    let value = match operator {
        BinaryOperator::Add => left.add(right),
        BinaryOperator::Subtract => left.sub(right),
        BinaryOperator::Multiply => {
            if !left.is_zero()
                && !right.is_zero()
                && left
                    .bit_length()
                    .saturating_add(right.bit_length())
                    .saturating_sub(1)
                    > super::values::MAXIMUM_CONST_INTEGER_BITS
            {
                return Err(
                    "integer multiplication exceeds the const-evaluation resource budget"
                        .to_owned(),
                );
            }
            left.mul(right)
        }
        BinaryOperator::Divide | BinaryOperator::Modulo => {
            let (quotient, remainder) = left
                .div_rem(right)
                .ok_or("integer division has no defined result")?;
            // Remainder owes the common quotient's representability too:
            // signed MIN % -1 is invalid under Exact even though r is zero.
            if let Some(integer_type) = integer_type {
                validate_declared_integer(integer_type, &quotient)?;
            }
            if operator == BinaryOperator::Divide {
                quotient
            } else {
                remainder
            }
        }
        BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor => {
            evaluate_declared_width_operation(operator, left, right, integer_type)?
        }
        _ => {
            return Err(
                "only integer arithmetic, shifts, and bitwise operators are supported".to_owned(),
            );
        }
    };
    let value = const_integer_within_budget(value)
        .ok_or("integer evaluation exceeds the canonical-value resource budget")?;
    if let Some(integer_type) = integer_type {
        validate_declared_integer(integer_type, &value).map_err(|reason| {
            let operation = match operator {
                BinaryOperator::Add => "addition",
                BinaryOperator::Subtract => "subtraction",
                BinaryOperator::Multiply => "multiplication",
                BinaryOperator::Divide => "division",
                BinaryOperator::Modulo => "remainder",
                BinaryOperator::ShiftLeft => "left shift",
                BinaryOperator::ShiftRight => "right shift",
                _ => "bitwise operation",
            };
            format!("{operation} exceeds the declared `{integer_type}` range: {reason}")
        })?;
    }
    Ok(value)
}

pub(in crate::resolution::declarations::generic_data) fn const_integer_type(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> Option<LandedIntegerType> {
    let name = syntax_type_references(syntax)
        .type_reference(type_reference)
        .as_named()?;
    match selection?.builtin_type(syntax, name)? {
        symbols::BuiltinTypeAtom::Integer(carrier) => Some(LandedIntegerType::Fixed(carrier)),
        symbols::BuiltinTypeAtom::Address => Some(LandedIntegerType::Addr),
        _ => None,
    }
}

/* Reject a value outside its declared integer carrier's range. */
pub(crate) fn validate_declared_integer(
    integer_type: LandedIntegerType,
    value: &BigInt,
) -> Result<(), String> {
    if value.bit_length() > super::values::MAXIMUM_CONST_INTEGER_BITS {
        return Err("integer constant exceeds its canonical-value resource budget".to_owned());
    }
    if !integer_type.value_carrier().admits(value) {
        return Err(format!(
            "integer value `{value}` is outside the declared `{}` range",
            integer_type.name()
        ));
    }
    Ok(())
}

pub(in crate::resolution::declarations::generic_data) fn generic_const_integer_types(
    syntax: &SyntaxTrees,
    generic_name: &Identifier,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
) -> Vec<Option<LandedIntegerType>> {
    selection
        .and_then(|selection| selection.data(syntax, generic_name).ok())
        .map(|definition| {
            syntax_items(syntax)
                .type_parameters(definition.type_parameters)
                .iter()
                .map(|parameter| match parameter.kind {
                    TypeParameterKind::Const { type_reference } => {
                        const_integer_type(syntax, type_reference, selection)
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(in crate::resolution::declarations::generic_data) fn evaluate_declared_width_operation(
    operator: BinaryOperator,
    left: &BigInt,
    right: &BigInt,
    integer_type: Option<LandedIntegerType>,
) -> Result<BigInt, String> {
    let Some(integer_type) = integer_type else {
        return Err(
            "shifts and bitwise operators require a declared integer const type".to_string(),
        );
    };
    if !integer_type.value_carrier().admits(left) {
        return Err(format!(
            "left operand `{left}` is outside the declared `{}` range",
            integer_type.name()
        ));
    }

    match operator {
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            let amount = right
                .to_u64()
                .filter(|amount| *amount < integer_type.bit_width())
                .ok_or_else(|| {
                    format!(
                        "shift count must be non-negative and below the declared `{}` width",
                        integer_type.name()
                    )
                })?;
            if left.is_zero() {
                return Ok(BigInt::zero());
            }
            if operator == BinaryOperator::ShiftRight {
                if amount >= left.bit_length() as u64 {
                    return Ok(BigInt::from(if left.is_negative() { -1 } else { 0 }));
                }
                let amount = usize::try_from(amount)
                    .map_err(|_| "right shift exceeds host indexing resources")?;
                return Ok(left.shr_bits(amount));
            }
            let amount = usize::try_from(amount)
                .ok()
                .filter(|amount| {
                    left.bit_length().saturating_add(*amount)
                        <= super::values::MAXIMUM_CONST_INTEGER_BITS
                })
                .ok_or("left shift exceeds the const-evaluation resource budget")?;
            let shifted = left.shl_bits(amount);
            if !integer_type.value_carrier().admits(&shifted) {
                return Err(format!(
                    "left shift exceeds the declared `{}` range",
                    integer_type.name()
                ));
            }
            Ok(shifted)
        }
        BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor => {
            if !integer_type.value_carrier().admits(right) {
                return Err(format!(
                    "right operand `{right}` is outside the declared `{}` range",
                    integer_type.name()
                ));
            }
            use bignum::IntegerBitwiseOperation;
            let operation = match operator {
                BinaryOperator::BitwiseAnd => IntegerBitwiseOperation::And,
                BinaryOperator::BitwiseOr => IntegerBitwiseOperation::Or,
                BinaryOperator::BitwiseXor => IntegerBitwiseOperation::Xor,
                _ => return Err("not a bitwise integer operation".to_owned()),
            };
            Ok(left.bitwise(right, operation))
        }
        _ => Err("not a shift or bitwise integer operation".to_owned()),
    }
}

#[derive(Debug)]
pub(in crate::resolution::declarations::generic_data) enum EvaluatedConst {
    Concrete(BigInt),
    Symbolic(String),
}

impl EvaluatedConst {
    pub(in crate::resolution::declarations::generic_data) fn into_concrete(
        self,
    ) -> Result<BigInt, String> {
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

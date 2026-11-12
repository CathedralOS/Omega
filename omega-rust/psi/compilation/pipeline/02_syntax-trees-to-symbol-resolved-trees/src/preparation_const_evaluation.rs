/* Exact source-constant evaluation used by declaration retention and synthesis. */

use super::{ConstDefinition, Diagnostic, HashMap, Identifier, SyntaxTrees};

mod anonymous {
    /* Exact anonymous values, independent of their eventual const landing. */

    use super::fact_values::ConstFactValue;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use diagnostics::Diagnostic;
    use numerics::bignum::BigRational;
    use numerics::literals::LandedIntegerType;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::BinaryOperator;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::expression::TableBinaryExpression;
    use syntax_trees::item::Item;
    use syntax_trees::item::Machine;
    use syntax_trees::operator_spelling::OperatorSpelling;

    pub(crate) struct AnonymousNumericValue {
        pub(super) value: BigRational,
        fractional: Option<(ExpressionHandle, BigRational)>,
    }

    /*
        Named values and prior landings stay with the declared integer evaluator.
        Wholly anonymous arithmetic needs no declaration selection: it denotes an
        exact rational (wiki/spec/language/numeric_values.md), and operator
        spellings resolve by operand carrier, which anonymous operands lack.
    */
    pub(super) fn evaluate_anonymous_integer_argument(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<i128>, String> {
        evaluate_anonymous_numeric_expression(syntax, expression)?
            .map(|value| value.into_integer(syntax, warnings))
            .transpose()
    }

    pub(super) fn evaluate_anonymous_numeric_expression(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
    ) -> Result<Option<AnonymousNumericValue>, String> {
        if !anonymous_numeric_expression(syntax, expression) {
            return Ok(None);
        }
        enum Step {
            Enter(ExpressionHandle),
            Binary(ExpressionHandle, BinaryOperator),
        }
        let mut pending = vec![Step::Enter(expression)];
        let mut values: Vec<BigRational> = Vec::new();
        let mut fractional = None;
        while let Some(step) = pending.pop() {
            match step {
                Step::Enter(expression) => match syntax.expressions.expression(expression) {
                    ExpressionNode::Integer(literal) => {
                        let value = literal.value_bignum().ok_or_else(|| {
                            "anonymous integer literal has no exact value".to_owned()
                        })?;
                        values.push(BigRational::from_integer(value));
                    }
                    ExpressionNode::Float(text) => {
                        let value = text
                            .literal(syntax)
                            .and_then(|literal| BigRational::from_decimal_str(literal.text()))
                            .ok_or_else(|| {
                                "anonymous decimal literal has no exact value".to_owned()
                            })?;
                        if fractional.is_none() && value.to_integer_exact().is_none() {
                            fractional = Some((expression, value.clone()));
                        }
                        values.push(value);
                    }
                    ExpressionNode::Binary(binary) => {
                        pending.push(Step::Binary(expression, binary.operator));
                        pending.push(Step::Enter(binary.right));
                        pending.push(Step::Enter(binary.left));
                    }
                    _ => {
                        return Err(
                            "anonymous const expression changed during evaluation".to_owned()
                        );
                    }
                },
                Step::Binary(expression, operator) => {
                    let right = values
                        .pop()
                        .ok_or_else(|| "missing right const operand".to_owned())?;
                    let left = values
                        .pop()
                        .ok_or_else(|| "missing left const operand".to_owned())?;
                    let value = match operator {
                        BinaryOperator::Add => left.add(&right),
                        BinaryOperator::Subtract => left.sub(&right),
                        BinaryOperator::Multiply => left.mul(&right),
                        BinaryOperator::Divide => left
                            .div(&right)
                            .ok_or_else(|| "division by zero is invalid".to_owned())?,
                        _ => return Err("operator has no anonymous const meaning".to_owned()),
                    };
                    if fractional.is_none() && value.to_integer_exact().is_none() {
                        fractional = Some((expression, value.clone()));
                    }
                    values.push(value);
                }
            }
        }
        if values.len() != 1 {
            return Err("anonymous const expression has no single value".to_owned());
        }
        let value = values
            .pop()
            .ok_or_else(|| "missing anonymous const value".to_owned())?;
        Ok(Some(AnonymousNumericValue { value, fractional }))
    }

    impl AnonymousNumericValue {
        pub(super) fn into_integer(
            self,
            syntax: &SyntaxTrees,
            warnings: &mut Vec<Diagnostic>,
        ) -> Result<i128, String> {
            let value = self.value;
            let integer = value.to_integer_exact().ok_or_else(|| format!(
        "exact anonymous value `{value}` cannot land in an integer const argument; type an operand if typed integer division was intended"
    ))?;
            let value = integer
                .to_i64()
                .map(i128::from)
                .or_else(|| integer.to_u64().map(i128::from))
                .ok_or_else(|| {
                    "integer const argument exceeds the signed/unsigned 64-bit envelope".to_owned()
                })?;
            if let Some((origin, fractional_value)) = self.fractional {
                // Implicit const-argument landing after a fractional
                // intermediate rejects (numeric_values.md); an explicit
                // `as` operand is not an anonymous argument at this layer.
                warnings.push(Diagnostic::error(format!(
                    "anonymous arithmetic with the exact fractional intermediate `{fractional_value}` cannot land implicitly as integer `{integer}`; cast the complete anonymous expression with `as` to land its exact value, or type an operand if typed integer division was intended"
                )).with_source_span(syntax.expressions.source_span(origin)));
            }
            Ok(value)
        }

        pub(super) fn binary(
            self,
            right: Self,
            operator: BinaryOperator,
            expression: ExpressionHandle,
        ) -> Result<Self, String> {
            let value = match operator {
                BinaryOperator::Add => self.value.add(&right.value),
                BinaryOperator::Subtract => self.value.sub(&right.value),
                BinaryOperator::Multiply => self.value.mul(&right.value),
                BinaryOperator::Divide => self
                    .value
                    .div(&right.value)
                    .ok_or_else(|| "division by zero is invalid".to_owned())?,
                _ => return Err("operator has no anonymous const meaning".to_owned()),
            };
            let fractional = self.fractional.or(right.fractional).or_else(|| {
                value
                    .to_integer_exact()
                    .is_none()
                    .then(|| (expression, value.clone()))
            });
            Ok(Self { value, fractional })
        }
    }

    /* Keep authored operator applications intact until declaration selection. */
    pub(super) fn requires_const_operator_selection(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
    ) -> bool {
        match syntax.expressions.expression(expression) {
            ExpressionNode::Binary(binary) => {
                !has_builtin_const_operator(syntax, binary)
                    || requires_const_operator_selection(syntax, binary.left)
                    || requires_const_operator_selection(syntax, binary.right)
            }
            ExpressionNode::Unary(unary) => {
                requires_const_operator_selection(syntax, unary.operand)
            }
            _ => false,
        }
    }

    /*
        Whether this pre-resolution pass may apply the builtin meaning of one
        binary node. Operator spellings resolve by operand carrier
        (wiki/spec/language/numeric_values.md), so a node whose operands are both
        wholly anonymous selects no declaration: it is exact builtin arithmetic or
        comparison, the rule checked-tree operator selection also applies.
    */
    pub(super) fn has_builtin_const_operator(
        syntax: &SyntaxTrees,
        binary: &TableBinaryExpression,
    ) -> bool {
        if anonymous_numeric_expression(syntax, binary.left)
            && anonymous_numeric_expression(syntax, binary.right)
        {
            return true;
        }
        let Some(spelling) = binary_operator_spelling(binary.operator) else {
            return true;
        };
        has_no_authored_spelling(syntax, spelling)
    }

    fn binary_operator_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
        Some(match operator {
            BinaryOperator::Add => OperatorSpelling::Add,
            BinaryOperator::Subtract => OperatorSpelling::Subtract,
            BinaryOperator::Multiply => OperatorSpelling::Multiply,
            BinaryOperator::Divide => OperatorSpelling::Divide,
            BinaryOperator::Modulo => OperatorSpelling::Modulo,
            BinaryOperator::Equal => OperatorSpelling::Equal,
            BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
            BinaryOperator::Less => OperatorSpelling::Less,
            BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
            BinaryOperator::Greater => OperatorSpelling::Greater,
            BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
            BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight => return None,
        })
    }

    fn has_no_authored_spelling(syntax: &SyntaxTrees, spelling: OperatorSpelling) -> bool {
        !syntax
            .root_items()
            .any(|item| authored_spelling_parameters(syntax, item, spelling).is_some())
    }

    /*
        The declared parameter carriers of one item's authored `spelling`, or
        `None` when the item does not author it. Each parameter yields the spelled
        name of its type (`u64`, `Nat`, `f32`); a parameter whose carrier cannot
        be read — a binder, a borrow, an uninspectable lane — yields `None`, which
        may bind any operand.
    */
    fn authored_spelling_parameters(
        syntax: &SyntaxTrees,
        item: &Item,
        spelling: OperatorSpelling,
    ) -> Option<Vec<Option<String>>> {
        match item {
            Item::Machine(machine) if machine.spelling == Some(spelling) => {
                Some(machine_parameter_names(syntax, machine))
            }
            Item::Operator(operator) if operator.spelling == Some(spelling) => {
                Some(vec![None, None])
            }
            Item::Domain(domain) => syntax_items(syntax)
                .operators(domain.operators)
                .iter()
                .any(|operator| operator.spelling == Some(spelling))
                .then(|| vec![None, None]),
            Item::Trait(trait_definition) => syntax_items(syntax)
                .state_signatures(trait_definition.machines)
                .iter()
                .any(|handle| syntax.items.state_signature(*handle).spelling == Some(spelling))
                .then(|| vec![None, None]),
            _ => None,
        }
    }

    /*
        The parameter type names a spelled machine declares on its entry state,
        with the machine's own type binders left unread: `T` can carry any
        operand, so it stays `None`.
    */
    fn machine_parameter_names(syntax: &SyntaxTrees, machine: &Machine) -> Vec<Option<String>> {
        let binder_names: Vec<&str> = syntax_items(syntax)
            .type_parameters(machine.type_parameters)
            .iter()
            .map(|parameter| parameter.name.as_str(syntax))
            .collect();
        let Some(entry) = syntax.items.state_handles(machine.states).first() else {
            return Vec::new();
        };
        let state = syntax.items.state(*entry);
        syntax_items(syntax)
            .state_parameters(state.parameters)
            .iter()
            .map(|handle| {
                let parameter = syntax.items.state_parameter(*handle);
                let name = syntax_type_references(syntax)
                    .type_reference(parameter.type_reference)
                    .as_named()?;
                (!binder_names.contains(&name.as_str(syntax)))
                    .then(|| name.as_str(syntax).to_string())
            })
            .collect()
    }

    /*
        Whether one authored `spelling` occurrence could bind these closed operand
        values. `has_no_authored_spelling` answers before operand carriers are
        known, so the core `Nat` `<=`/`-` machines make every `x <= y` fact
        decline even where the operands could never reach a `Nat` parameter.
        Once both operands landed, a spelled machine applies only when each
        declared parameter accepts the operand's carrier; a fact between `u64`
        values cannot reach `Nat` or `f32` parameters and keeps the builtin
        meaning. Callers whose subject binding loses the carrier (domain replay
        on a scalar `self`) must keep `has_builtin_const_operator`'s answer.
    */
    pub(super) fn authored_operator_can_bind(
        syntax: &SyntaxTrees,
        operator: BinaryOperator,
        left: &ConstFactValue,
        right: &ConstFactValue,
    ) -> bool {
        let Some(spelling) = binary_operator_spelling(operator) else {
            return false;
        };
        syntax.root_items().any(|item| {
            authored_spelling_parameters(syntax, item, spelling).is_some_and(|parameters| {
                parameters.len() != 2
                    || parameter_binds(parameters[0].as_deref(), left)
                        && parameter_binds(parameters[1].as_deref(), right)
            })
        })
    }

    /*
        Whether a parameter declared with the named carrier accepts this operand.
        A declared-integer operand binds only its own carrier (`u64` reaches `u64`
        parameters, never `Nat`); an anonymous or bare integer binds only builtin
        integer carriers — never a nominal `Nat`, `f32` or `Quantity`; a Boolean
        binds `bool`.
    */
    fn parameter_binds(parameter_name: Option<&str>, operand: &ConstFactValue) -> bool {
        let Some(name) = parameter_name else {
            return true;
        };
        use super::fact_values::ConstFactValue;
        match operand {
            ConstFactValue::DeclaredInteger { integer_type, .. } => name == integer_type.name(),
            ConstFactValue::Integer(_) | ConstFactValue::Anonymous(_) => {
                LandedIntegerType::from_name(name).is_some()
            }
            ConstFactValue::Boolean(_) => name == "bool",
        }
    }

    pub(super) fn anonymous_numeric_expression(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
    ) -> bool {
        enum Step {
            Enter(ExpressionHandle),
            Leave,
        }
        let mut pending = vec![Step::Enter(expression)];
        let mut active = Vec::new();
        while let Some(step) = pending.pop() {
            let Step::Enter(expression) = step else {
                active.pop();
                continue;
            };
            if !expression.is_valid() || active.contains(&expression) {
                return false;
            }
            match syntax.expressions.expression(expression) {
                ExpressionNode::Integer(literal) if literal.landing().is_none() => {}
                ExpressionNode::Float(text)
                    if text
                        .literal(syntax)
                        .is_some_and(|literal| literal.landing().is_none()) => {}
                ExpressionNode::Binary(binary) => {
                    if !matches!(
                        binary.operator,
                        BinaryOperator::Add
                            | BinaryOperator::Subtract
                            | BinaryOperator::Multiply
                            | BinaryOperator::Divide
                    ) {
                        return false;
                    }
                    active.push(expression);
                    pending.push(Step::Leave);
                    pending.push(Step::Enter(binary.right));
                    pending.push(Step::Enter(binary.left));
                }
                _ => return false,
            }
        }
        true
    }
}
mod arguments {
    /* Constant evaluation: arguments. */
    use super::super::{
        BinaryOperator, CanonicalConstValue, ConstDefinition, Diagnostic, ExpressionHandle,
        ExpressionNode, HashMap, HashSet, Identifier, SyntaxTrees, TypeParameterKind,
        TypeReferenceHandle, TypeReferenceNode,
    };
    use crate::preparation::generic_data::eligibility::base_is_fully_monomorphizable;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use syntax_trees::identifier::JoinedSyntaxPath;

    use crate::preparation::generic_data::ConstScalarSpelling;
    use crate::preparation::generic_data::GenericData;
    use crate::preparation::generic_data::discovery::ClosedApplication;

    use super::anonymous::evaluate_anonymous_integer_argument;
    use super::module_constant_selection_gate;
    use crate::preparation::generic_data::canonicalize_monomorphizable_argument_handles;
    use crate::preparation::generic_data::closed_argument_identity;
    use crate::preparation::generic_data::const_evaluation::validate_anonymous_remainder;
    use crate::preparation::generic_data::const_integer_in_envelope;
    use crate::preparation::generic_data::integer_literal_value;
    use crate::preparation::generic_data::monomorphizable_argument_slugs;
    use crate::preparation::generic_data::selected_generic_data;
    use crate::preparation::generic_data::syntax_type_identity;
    use crate::preparation::generic_data::validate_syntax_integer_range;
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use numerics::literals::LandedIntegerType;

    /*
        If `type_reference` is a `Base<Args..>` spelling of a fully-monomorphizable
        generic data definition, prepare its closed argument tuple. The synthesis
        owner decides declaration demand and reuse; argument evaluation never owns
        the instance roster. Anything Phase 1 cannot lower completely -- a non-generic base,
        wrong arity, a non-sluggable argument, or a data shape whose members cannot
        substitute every parameter occurrence exactly -- is left
        UNTOUCHED for the existing type-check-only path (skip, never reject).
    */
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
        let base = base_name.as_str(syntax).to_string();
        let Some(base_info) = selected_generic_data(syntax, generic_data, selection, &base_name)
        else {
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
            match syntax_type_references(syntax)
                .type_reference(*argument)
                .clone()
            {
                TypeReferenceNode::Named(name) => {
                    match ConstScalarSpelling::from_bare_name(name.as_str(syntax)) {
                        Some(
                            ConstScalarSpelling::Canonical(_) | ConstScalarSpelling::Integer(_),
                        ) => {
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
                    module_constant_selection_gate(syntax, &name, const_values, const_definitions)?;
                    if let Some(value) = const_values.get(name.as_str(syntax)) {
                        syntax.tables.type_references.replace_type_reference(
                            *argument,
                            TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                        );
                        continue;
                    }
                    let Some(definition) = const_definitions.get(name.as_str(syntax)) else {
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
        let Some(argument_names) =
            monomorphizable_argument_slugs(syntax, &argument_handles, selection)
        else {
            return Ok(None);
        };
        if !const_arguments_fit_declarations(syntax, base_info, &argument_handles) {
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
                let Some(integer_type) = LandedIntegerType::from_name(type_name.as_str(syntax))
                else {
                    return false;
                };
                let (minimum, maximum) = integer_type.bounds();
                value >= minimum && value <= maximum
            })
    }

    /*
        Evaluate the symbolic integer subset retained in a const-generic argument.
        Names resolve to literal scoped const declarations collected above.
        Anonymous arithmetic retains rational intermediates until its integer
        landing. Already-typed arithmetic keeps the current signed/unsigned 64-bit
        envelope. Shifts and bitwise operations use the matched const parameter's
        declared width and signedness.
    */
    pub(in crate::preparation::generic_data) fn evaluate_const_argument_expression(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        const_values: &HashMap<String, i128>,
        parameter_values: &HashMap<String, i128>,
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
                .ok_or_else(|| {
                    "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
                }),
            ExpressionNode::Name(path) => {
                let members = syntax.expressions.identifier_path_members(*path);
                let name = members.joined_path_in(syntax);
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
                    return Err(
                        "const operator application requires declaration selection".to_owned()
                    );
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
                    (
                        BinaryOperator::Divide | BinaryOperator::Modulo,
                        EvaluatedConst::Concrete(0),
                    ) => {
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
                let (EvaluatedConst::Concrete(left), EvaluatedConst::Concrete(right)) =
                    (&left, &right)
                else {
                    return Ok(left.or_symbolic(right));
                };
                let (left, right) = (*left, *right);
                match binary.operator {
                    BinaryOperator::Add => {
                        checked_evaluated_const(left.checked_add(right), "addition")
                    }
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
                    | BinaryOperator::BitwiseXor => evaluate_declared_width_operation(
                        binary.operator,
                        left,
                        right,
                        integer_type,
                    )
                    .map(EvaluatedConst::Concrete),
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
        let name = syntax_type_references(syntax)
            .type_reference(type_reference)
            .as_named()?;
        LandedIntegerType::from_name(name.as_str(syntax))
    }

    /* Reject a value outside its declared integer carrier's range. */
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
                (definition.name.as_str(syntax) == generic_name).then(|| {
                    syntax_items(syntax)
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
}
mod domains {
    /* Constant evaluation: domains. */
    use super::super::{
        CanonicalConstValue, ConstDefinition, Diagnostic, ExpressionNode, HashMap, HashSet,
        Identifier, IntegerLiteral, SyntaxTrees, TypeParameterKind, TypeReferenceHandle,
        TypeReferenceNode,
    };
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use syntax_trees::identifier::JoinedSyntaxPath;

    use crate::preparation::generic_data::ConstScalarSpelling;
    use crate::preparation::generic_data::EvaluatedConst;
    use crate::preparation::generic_data::canonicalize_selected_const_definition;
    use crate::preparation::generic_data::collect_data_type_reference_positions;
    use crate::preparation::generic_data::const_integer_type;
    use crate::preparation::generic_data::evaluate_const_argument_expression;
    use crate::preparation::generic_data::module_constants::module_path;
    use crate::preparation::generic_data::syntax_type_identity;
    use crate::preparation::generic_data::validate_const_index_type;
    use crate::preparation::generic_data::validate_syntax_integer_range;
    use crate::preparation::generic_data::work_domain::TypePositionPurpose;
    use language_semantics::const_value::DecodedCanonicalConstValue;

    pub(in crate::preparation::generic_data) fn integer_literal_value(
        value: &IntegerLiteral,
    ) -> Option<i128> {
        value
            .value_i64()
            .map(i128::from)
            .or_else(|| value.value_u64().map(i128::from))
    }

    pub(in crate::preparation::generic_data) fn qualified_const_name(
        syntax_trees: &SyntaxTrees,
        definition: &ConstDefinition,
    ) -> String {
        crate::constant::semantic_const_name(syntax_trees, definition)
    }

    #[derive(Clone)]
    pub(in crate::preparation::generic_data) struct ClosedDomainFamily {
        path: String,
        declaration: Identifier,
        parameters: Vec<ClosedDomainParameter>,
    }

    #[derive(Clone)]
    enum ClosedDomainParameter {
        Type {
            name: String,
        },
        Const {
            name: String,
            type_reference: TypeReferenceHandle,
        },
    }

    /*
        The family's index telescope, excluding only a directly named first
        type binder used as its carrier. Fixed carriers retain every parameter.
    */
    pub(in crate::preparation::generic_data) fn domain_index_parameters<'syntax>(
        syntax: &'syntax SyntaxTrees,
        definition: &syntax_trees::item::DomainDefinition,
    ) -> Option<&'syntax [syntax_trees::item::TypeParameter]> {
        let target_name =
            match syntax_type_references(syntax).type_reference(definition.target_type) {
                TypeReferenceNode::Named(target) => Some(target.as_str(syntax)),
                _ => None,
            };
        let parameters = syntax.items.type_parameters(definition.type_parameters);
        let generic_carrier = parameters.first().is_some_and(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Type)
                && target_name == Some(parameter.name.as_str(syntax))
        });
        Some(if generic_carrier {
            &parameters[1..]
        } else {
            parameters
        })
    }

    /*
        PDI2's closed-index precursor runs beside generic-data canonicalization but
        does not monomorphize the domain: the family remains nominal and erased.
        Only its const arguments are rewritten to the same canonical leaves used by
        PDI1 generic identity.
    */
    pub(in crate::preparation::generic_data) fn canonicalize_closed_domain_indices(
        syntax: &mut SyntaxTrees,
        const_values: &HashMap<String, i128>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
        work: &super::super::work_domain::SynthesisWork<'_>,
    ) -> Result<(), Diagnostic> {
        let mut families = Vec::<ClosedDomainFamily>::new();

        for item in syntax.root_items() {
            let Some(definition) = item.as_domain() else {
                continue;
            };
            let parameters = syntax_items(syntax).type_parameters(definition.type_parameters);
            let header_arguments =
                syntax_type_references(syntax).type_reference_handles(definition.index_arguments);

            if parameters.is_empty() {
                if !header_arguments.is_empty() {
                    return Err(Diagnostic::error(format!(
                        "domain `{}` supplies index arguments but declares no generic carrier/const telescope",
                        definition.name.as_str(syntax)
                    )));
                }
                continue;
            }

            let Some(index_parameters) = domain_index_parameters(syntax, definition) else {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must use its carrier binder directly before `::{}`",
                    definition.name.as_str(syntax),
                    definition.name.as_str(syntax)
                )));
            };
            if index_parameters.is_empty() {
                continue;
            }
            let mut family_parameters = Vec::with_capacity(index_parameters.len());
            for parameter in index_parameters {
                match parameter.kind {
                    TypeParameterKind::Type => {
                        family_parameters.push(ClosedDomainParameter::Type {
                            name: parameter.name.as_str(syntax).to_owned(),
                        })
                    }
                    TypeParameterKind::Const { type_reference } => {
                        validate_const_index_type(syntax, type_reference, &mut HashSet::new())
                        .map_err(|reason| {
                            Diagnostic::error(format!(
                                "indexed domain `{}::{}` has an ineligible index type: {reason}",
                                definition.name.as_str(syntax),
                                parameter.name.as_str(syntax)
                            ))
                        })?;
                        family_parameters.push(ClosedDomainParameter::Const {
                            name: parameter.name.as_str(syntax).to_owned(),
                            type_reference,
                        });
                    }
                    _ => {
                        return Err(Diagnostic::error(format!(
                            "indexed domain `{}` indices must be type or proof-static const parameters",
                            definition.name.as_str(syntax)
                        )));
                    }
                }
            }
            if header_arguments.len() != family_parameters.len() {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` declares {} index parameter(s) but selects {} index argument(s) in its family header",
                    definition.name.as_str(syntax),
                    family_parameters.len(),
                    header_arguments.len()
                )));
            }
            for (parameter, argument) in family_parameters.iter().zip(header_arguments) {
                let parameter_name = match parameter {
                    ClosedDomainParameter::Type { name }
                    | ClosedDomainParameter::Const { name, .. } => name,
                };
                let Some(argument_name) = syntax_type_references(syntax)
                    .type_reference(*argument)
                    .as_named()
                else {
                    return Err(Diagnostic::error(format!(
                        "indexed domain `{}` must select each index binder directly in its family header",
                        definition.name.as_str(syntax)
                    )));
                };
                if argument_name.as_str(syntax) != parameter_name {
                    return Err(Diagnostic::error(format!(
                        "indexed domain `{}` must select index binder `{parameter_name}` in declaration order, not `{argument_name}`",
                        definition.name.as_str(syntax),
                        argument_name = argument_name.as_str(syntax)
                    )));
                }
            }
            let family_path = match module_path(syntax, definition.name.source_span().source_id) {
                Some(module) => format!("{module}::{}", definition.name.as_str(syntax)),
                None => definition.name.as_str(syntax).to_owned(),
            };
            let declaration = definition.name.source_span();
            if families.iter().any(|family| {
                family.path == family_path
                    && selection.is_none_or(|selection| {
                        selection
                            .same_checked_package(family.declaration.source_span(), declaration)
                    })
            }) {
                return Err(Diagnostic::error(format!(
                    "indexed domain family `{}` is declared more than once",
                    definition.name.as_str(syntax)
                )));
            }
            families.push(ClosedDomainFamily {
                path: family_path,
                declaration: definition.name.clone(),
                parameters: family_parameters,
            });
        }

        let constraints = if work.is_all() {
            syntax.tables.type_references.domain_constraints()
        } else {
            work.type_positions(syntax, TypePositionPurpose::Identity, true)
                .into_iter()
                .flat_map(|reference| {
                    let TypeReferenceNode::Constrained { constraints, .. } =
                        syntax.type_references.type_reference(reference)
                    else {
                        return Vec::new();
                    };
                    syntax_type_references(syntax)
                        .constraints(*constraints)
                        .iter()
                        .filter_map(|constraint| {
                            if let syntax_trees::types::TypeConstraintNode::Domain(domain) =
                                constraint
                            {
                                Some(domain.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        let declarations = work.declarations(syntax);
        let expressions =
            super::super::uses::selected_machine_expression_handles(syntax, &declarations);
        let mut applications = constraints
            .into_iter()
            .map(|constraint| {
                (
                    constraint.name.as_str(syntax).to_owned(),
                    constraint.arguments,
                    constraint.name.source_span(),
                )
            })
            .collect::<Vec<_>>();
        applications.extend(syntax.expressions.iter_expressions().filter_map(
            |(handle, expression)| {
                if !work.is_all()
                    && !work.owns_new_expression(handle)
                    && !expressions.contains(handle)
                {
                    return None;
                }
                let ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                if cast.semantic_domain.is_empty() {
                    return None;
                }
                let members =
                    syntax_expressions(syntax).identifier_path_members(cast.semantic_domain);
                let name = members.joined_path_in(syntax);
                let mut reference = members.first()?.source_span();
                for member in members.iter().skip(1) {
                    if member.source_span().source_id != reference.source_id {
                        break;
                    }
                    reference.span.end = member.source_span().span.end;
                }
                Some((name, cast.semantic_domain_arguments, reference))
            },
        ));

        let concrete_data_positions = collect_data_type_reference_positions(syntax, false);
        for (name, argument_span, reference) in applications {
            let family = match selection {
                Some(selection) => selection
                    .domain_family(syntax, &name, reference, None)
                    .and_then(|(_, definition)| {
                        families.iter().find(|family| {
                            family.declaration.source_span() == definition.name.source_span()
                                && family.declaration.as_str(syntax)
                                    == definition.name.as_str(syntax)
                        })
                    }),
                None => families.iter().find(|family| family.path == name),
            };
            let Some(family) = family else {
                continue;
            };
            let arguments = syntax_type_references(syntax)
                .type_reference_handles(argument_span)
                .to_vec();
            canonicalize_closed_domain_application(
                syntax,
                &name,
                family,
                arguments,
                const_values,
                selection,
                &concrete_data_positions,
                warnings,
            )?;
        }
        Ok(())
    }

    pub(in crate::preparation::generic_data) fn canonicalize_closed_domain_application(
        syntax: &mut SyntaxTrees,
        family_name: &str,
        family: &ClosedDomainFamily,
        arguments: Vec<TypeReferenceHandle>,
        const_values: &HashMap<String, i128>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        concrete_data_positions: &[TypeReferenceHandle],
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<(), Diagnostic> {
        if arguments.len() != family.parameters.len() {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` requires {} closed index argument(s), but {} were supplied",
                family_name,
                family.parameters.len(),
                arguments.len()
            )));
        }
        for (parameter, argument) in family.parameters.iter().zip(arguments) {
            let ClosedDomainParameter::Const {
                name: parameter_name,
                type_reference: parameter_type,
            } = parameter
            else {
                continue;
            };
            let node = syntax_type_references(syntax)
                .type_reference(argument)
                .clone();
            match node {
                TypeReferenceNode::Named(name) => {
                    match ConstScalarSpelling::from_bare_name(name.as_str(syntax)) {
                        Some(ConstScalarSpelling::Canonical(value)) => {
                            let required = syntax_type_identity(syntax, *parameter_type)
                                .map_err(Diagnostic::error)?;
                            if value.type_name != required {
                                return Err(Diagnostic::error(format!(
                                    "index argument for `{}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                                    family_name, value.type_name
                                )));
                            }
                            continue;
                        }
                        Some(ConstScalarSpelling::Integer(_)) => continue,
                        Some(ConstScalarSpelling::Boolean(value)) => {
                            let required = syntax_type_identity(syntax, *parameter_type)
                                .map_err(Diagnostic::error)?;
                            if required != "bool" {
                                return Err(Diagnostic::error(format!(
                                "index argument for `{family_name}::{parameter_name}` has canonical type `bool`, expected `{required}`"
                            )).with_source_span(name.source_span()));
                            }
                            let value = CanonicalConstValue::boolean(value);
                            syntax.type_references.replace_type_reference(
                                argument,
                                TypeReferenceNode::Named(Identifier::new(
                                    value.atom(),
                                    name.source_span(),
                                )),
                            );
                            continue;
                        }
                        None => {}
                    }
                    if let Some(selection) = selection
                        && concrete_data_positions.contains(&argument)
                    {
                        let Some(definition) = selection.select(syntax, &name)? else {
                            continue;
                        };
                        let value = canonicalize_selected_const_definition(syntax, &definition, *parameter_type, Some(selection))
                        .map_err(|reason| Diagnostic::error(format!(
                            "index argument for `{family_name}::{parameter_name}` is invalid: {reason}"
                        )).with_source_span(name.source_span()))?;
                        let replacement = match value.decode_encoding() {
                            Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                                value.to_string()
                            }
                            _ => value.atom(),
                        };
                        let initializer = syntax.expressions.source_span(definition.value);
                        syntax.type_references.retain_const_argument_normalization(
                            argument,
                            name.source_span(),
                            value.encoding.clone(),
                            [syntax_trees::types::ConstArgumentOrigin {
                                reference: name.source_span(),
                                declaration: definition.name.source_span(),
                                initializer,
                                evaluated_value: value.encoding.into(),
                            }],
                            [],
                        );
                        syntax.type_references.replace_type_reference(
                            argument,
                            TypeReferenceNode::Named(Identifier::generated(replacement)),
                        );
                        continue;
                    }
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    if syntax.const_expression_contains_name(expression)
                        || super::anonymous::requires_const_operator_selection(syntax, expression)
                    {
                        continue;
                    }
                    let destination =
                        syntax_type_identity(syntax, *parameter_type).map_err(Diagnostic::error)?;
                    let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, *parameter_type),
                    warnings,
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "index argument expression for `{}` is invalid for `{destination}`: {reason}",
                        family_name
                    )).with_source_span(syntax.expressions.source_span(expression))
                })?;
                    if const_integer_type(syntax, *parameter_type).is_some() {
                        let required = syntax_type_identity(syntax, *parameter_type)
                            .map_err(Diagnostic::error)?;
                        validate_syntax_integer_range(&required, value)
                            .map_err(Diagnostic::error)?;
                    }
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                }
                _ => {
                    return Err(Diagnostic::error(format!(
                        "indexed domain `{}::{parameter_name}` requires a closed const value or direct const binder",
                        family_name
                    )));
                }
            }
        }
        Ok(())
    }
}
mod fact_values {
    /* Values used while discharging closed generic facts, not a proof IR. */

    use crate::preparation::generic_data::checked_fact_integer;
    use crate::preparation::generic_data::const_integer_in_envelope;

    use super::super::{
        BinaryOperator, CanonicalConstValue, Diagnostic, ExpressionHandle, SyntaxTrees,
    };
    use super::anonymous::AnonymousNumericValue;
    use super::arguments::{evaluate_declared_width_operation, validate_declared_integer};
    use numerics::literals::LandedIntegerType;
    use std::cmp::Ordering;

    pub(crate) enum ConstFactValue {
        Anonymous(AnonymousNumericValue),
        Integer(i128),
        DeclaredInteger {
            value: i128,
            integer_type: LandedIntegerType,
        },
        Boolean(bool),
    }

    /*
        The concrete scalar bound to `self` while a selected domain's facts replay
        at declaration site, its closed index binders, and the payload a nested
        membership carries into the next selected domain. Unlike `ConstFactValue`
        there is no anonymous rational here: declaration-site discharge binds only
        completed canonical values.
    */
    #[derive(Clone, Copy)]
    pub(crate) enum ConstScalarValue {
        Integer(i128),
        DeclaredInteger {
            value: i128,
            integer_type: LandedIntegerType,
        },
        Boolean(bool),
    }

    impl ConstScalarValue {
        pub(crate) fn from_canonical(value: &super::super::CanonicalConstValue) -> Option<Self> {
            use language_semantics::const_value::DecodedCanonicalConstValue;
            match value.decode_encoding()? {
                DecodedCanonicalConstValue::Integer { value, type_name } => {
                    let integer_type = LandedIntegerType::from_name(&type_name)?;
                    validate_declared_integer(integer_type, value).ok()?;
                    Some(Self::DeclaredInteger {
                        value,
                        integer_type,
                    })
                }
                DecodedCanonicalConstValue::Boolean(value) => Some(Self::Boolean(value)),
                _ => None,
            }
        }

        pub(crate) fn into_fact_value(self) -> ConstFactValue {
            match self {
                Self::Integer(value) => ConstFactValue::Integer(value),
                Self::DeclaredInteger {
                    value,
                    integer_type,
                } => ConstFactValue::DeclaredInteger {
                    value,
                    integer_type,
                },
                Self::Boolean(value) => ConstFactValue::Boolean(value),
            }
        }
    }

    impl From<i128> for ConstScalarValue {
        fn from(value: i128) -> Self {
            Self::Integer(value)
        }
    }

    /*
        A bare generic-argument name that spells a const scalar value rather than
        naming a declaration: an already-canonical const atom, an integer literal,
        or a boolean literal. Recognition stops at the spelling. The canonical atom
        is handed back undecoded because each site keeps its own required-type
        check and decoding rule (a non-scalar or out-of-range atom must still be
        reported against the parameter type, not silently dropped).
    */
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ConstScalarSpelling {
        Canonical(CanonicalConstValue),
        Integer(i128),
        Boolean(bool),
    }

    impl ConstScalarSpelling {
        pub(crate) fn from_bare_name(name: &str) -> Option<Self> {
            if let Some(value) = CanonicalConstValue::from_atom(name) {
                return Some(Self::Canonical(value));
            }
            if let Ok(value) = name.parse::<i128>() {
                return Some(Self::Integer(value));
            }
            language_semantics::const_value::boolean_literal_spelling(name).map(Self::Boolean)
        }
    }

    impl ConstFactValue {
        pub(crate) fn into_integer(
            self,
            syntax: &SyntaxTrees,
            warnings: &mut Vec<Diagnostic>,
        ) -> Result<Option<i128>, String> {
            match self {
                Self::Anonymous(value) => value.into_integer(syntax, warnings).map(Some),
                Self::Integer(value) => Ok(Some(value)),
                Self::DeclaredInteger {
                    value,
                    integer_type,
                } => {
                    validate_declared_integer(integer_type, value)?;
                    Ok(Some(value))
                }
                Self::Boolean(_) => Ok(None),
            }
        }
    }

    pub(crate) fn evaluate_const_fact_binary(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        operator: BinaryOperator,
        left: ConstFactValue,
        right: ConstFactValue,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<ConstFactValue, String> {
        use BinaryOperator::*;
        if matches!(&left, ConstFactValue::DeclaredInteger { .. })
            || matches!(&right, ConstFactValue::DeclaredInteger { .. })
        {
            return evaluate_declared_integer_fact(syntax, operator, left, right, warnings);
        }
        let (left, right) = match (left, right) {
            (ConstFactValue::Anonymous(left), ConstFactValue::Anonymous(right)) => {
                if matches!(operator, Add | Subtract | Multiply | Divide) {
                    return left
                        .binary(right, operator, expression)
                        .map(ConstFactValue::Anonymous);
                }
                let ordering = left.value.cmp_value(&right.value);
                let comparison = match operator {
                    Equal => Some(ordering == Ordering::Equal),
                    NotEqual => Some(ordering != Ordering::Equal),
                    Greater => Some(ordering == Ordering::Greater),
                    GreaterOrEqual => Some(ordering != Ordering::Less),
                    Less => Some(ordering == Ordering::Less),
                    LessOrEqual => Some(ordering != Ordering::Greater),
                    _ => None,
                };
                if let Some(value) = comparison {
                    return Ok(ConstFactValue::Boolean(value));
                }
                (
                    ConstFactValue::Anonymous(left),
                    ConstFactValue::Anonymous(right),
                )
            }
            operands => operands,
        };
        if let (ConstFactValue::Boolean(left), ConstFactValue::Boolean(right)) = (&left, &right) {
            return match operator {
                And => Ok(ConstFactValue::Boolean(*left && *right)),
                Or => Ok(ConstFactValue::Boolean(*left || *right)),
                Equal => Ok(ConstFactValue::Boolean(left == right)),
                NotEqual => Ok(ConstFactValue::Boolean(left != right)),
                _ => Err("arithmetic and ordering operators require integer operands".to_string()),
            };
        }
        let Some(left) = left.into_integer(syntax, warnings)? else {
            return Err("const fact operands have incompatible types".to_owned());
        };
        let Some(right) = right.into_integer(syntax, warnings)? else {
            return Err("const fact operands have incompatible types".to_owned());
        };
        match operator {
            Add => checked_fact_integer(left.checked_add(right), "addition"),
            Subtract => checked_fact_integer(left.checked_sub(right), "subtraction"),
            Multiply => checked_fact_integer(left.checked_mul(right), "multiplication"),
            Divide => left
                .checked_div(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "division by zero is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "remainder by zero is invalid".to_string()),
            ShiftLeft if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shl(amount))
                .and_then(const_integer_in_envelope)
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "left shift exceeds the `u64` width".to_string()),
            ShiftRight if left >= 0 => u32::try_from(right)
                .ok()
                .filter(|amount| *amount < u64::BITS)
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstFactValue::Integer)
                .ok_or_else(|| "right shift exceeds the `u64` width".to_string()),
            BitwiseAnd if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left & right)),
            BitwiseOr if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left | right)),
            BitwiseXor if left >= 0 && right >= 0 => Ok(ConstFactValue::Integer(left ^ right)),
            Equal => Ok(ConstFactValue::Boolean(left == right)),
            NotEqual => Ok(ConstFactValue::Boolean(left != right)),
            Greater => Ok(ConstFactValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstFactValue::Boolean(left >= right)),
            Less => Ok(ConstFactValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstFactValue::Boolean(left <= right)),
            And | Or => Err("logical operators require boolean operands".to_string()),
            ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => Err(
                "signed shifts and bitwise operators require declared-width semantics".to_string(),
            ),
        }
    }

    fn evaluate_declared_integer_fact(
        syntax: &SyntaxTrees,
        operator: BinaryOperator,
        left: ConstFactValue,
        right: ConstFactValue,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<ConstFactValue, String> {
        use BinaryOperator::*;
        let shift = matches!(operator, ShiftLeft | ShiftRight);
        let integer_type = match (&left, &right) {
            (
                ConstFactValue::DeclaredInteger {
                    integer_type: left, ..
                },
                ConstFactValue::DeclaredInteger {
                    integer_type: right,
                    ..
                },
            ) => {
                if !shift && left != right {
                    return Err(format!(
                        "const fact operands have incompatible declared carriers `{}` and `{}`",
                        left.name(),
                        right.name()
                    ));
                }
                *left
            }
            (
                ConstFactValue::DeclaredInteger { integer_type, .. },
                ConstFactValue::Anonymous(_) | ConstFactValue::Integer(_),
            ) => *integer_type,
            (
                ConstFactValue::Anonymous(_) | ConstFactValue::Integer(_),
                ConstFactValue::DeclaredInteger { integer_type, .. },
            ) if !shift => *integer_type,
            _ => {
                return Err(
                    "const fact integer operation requires retained declared carriers".to_owned(),
                );
            }
        };
        let mut operand = |value: ConstFactValue| -> Result<i128, String> {
            let (value, operand_type) = match value {
                ConstFactValue::DeclaredInteger {
                    value,
                    integer_type,
                } => (value, integer_type),
                ConstFactValue::Anonymous(value) => {
                    (value.into_integer(syntax, warnings)?, integer_type)
                }
                ConstFactValue::Integer(value) => (value, integer_type),
                _ => {
                    return Err("const fact integer operation lost its declared carrier".to_owned());
                }
            };
            validate_declared_integer(operand_type, value)?;
            Ok(value)
        };
        let left = operand(left)?;
        let right = operand(right)?;
        if right == 0 {
            match operator {
                Divide => return Err("division by zero is invalid".to_owned()),
                Modulo => return Err("remainder by zero is invalid".to_owned()),
                _ => {}
            }
        }
        let comparison = match operator {
            Equal => Some(left == right),
            NotEqual => Some(left != right),
            Greater => Some(left > right),
            GreaterOrEqual => Some(left >= right),
            Less => Some(left < right),
            LessOrEqual => Some(left <= right),
            _ => None,
        };
        if let Some(value) = comparison {
            return Ok(ConstFactValue::Boolean(value));
        }
        let value = match operator {
            Add => left.checked_add(right),
            Subtract => left.checked_sub(right),
            Multiply => left.checked_mul(right),
            Divide => left.checked_div(right),
            Modulo => {
                let quotient = left
                    .checked_div(right)
                    .ok_or_else(|| "const fact division is undefined".to_owned())?;
                validate_declared_integer(integer_type, quotient)?;
                left.checked_rem(right)
            }
            ShiftLeft | ShiftRight | BitwiseAnd | BitwiseOr | BitwiseXor => Some(
                evaluate_declared_width_operation(operator, left, right, Some(integer_type))?,
            ),
            _ => return Err("logical operators require boolean operands".to_owned()),
        }
        .ok_or_else(|| {
            "const fact integer operation is undefined or exceeds its declared range".to_owned()
        })?;
        validate_declared_integer(integer_type, value)?;
        Ok(ConstFactValue::DeclaredInteger {
            value,
            integer_type,
        })
    }
}
mod facts {
    /*
        Declaration-site domain discharge shares the closed fact evaluator used by
        generic data. Initializer probes determine values, not membership: ordinary
        constant validation still relies on this gate before publishing identity.
        Scalar replay carries declaration selection through every recursive operand;
        lexical subjects and binders precede selected constants, whose canonical
        values retain their carrier and policy before arithmetic.
        Exact declaration selection, complete index binding and establishment
        obligations therefore precede fact replay. A generic carrier does not supply
        a selected algebra for the subject; only carrier-independent facts can close here.
    */
    use super::super::{
        ConstDefinition, Diagnostic, ExpressionHandle, ExpressionNode, HashMap, HashSet, Item,
        ProofFact, SyntaxTrees, TypeConstraintNode, TypeParameterKind, TypeReferenceHandle,
        TypeReferenceNode, canonicalize_selected_declared_const_definition,
    };
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use arena::HandleSpan;
    use syntax_trees::expression::UnaryOperator;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::identifier::JoinedSyntaxPath;

    use crate::preparation::generic_data::const_evaluation::validate_anonymous_remainder;
    use crate::preparation::generic_data::evaluate_const_fact_binary;
    use crate::preparation::generic_data::integer_literal_value;
    use crate::preparation::generic_data::{
        ConstFactValue, ConstScalarSpelling, ConstScalarValue, canonicalize_const_definition,
        domain_index_parameters, syntax_type_identity,
    };

    use super::anonymous::{
        authored_operator_can_bind, evaluate_anonymous_numeric_expression,
        has_builtin_const_operator,
    };
    use super::arguments::validate_declared_integer;
    use super::qualified_const_name;
    use super::values::CanonicalConstNode;
    use super::values::canonicalize_selected_declaration_value;
    use crate::preparation::generic_data::module_constants::lexical_integer_const_values;
    use crate::preparation::generic_data::module_constants::module_path;
    use crate::preparation::generic_data::module_constants::reject_module_constant_selection;
    use language_core::CarryPermission;
    use language_core::CarryPolicy;
    use language_core::Multiplicity;
    use numerics::literals::LandedIntegerType;
    use syntax_trees::item::DataMember;
    use syntax_trees::item::DomainDefinition;

    /*
        Evaluate a proof expression exactly when every operand is known at generic
        instantiation time. `None` means the fact still depends on a runtime field
        and must remain on the synthesized record.
        Integer synthesis callers and mixed domain binders share the same fact
        operations without copying their environments or encoding Booleans as 0/1.
    */
    pub(crate) fn evaluate_const_fact_expression(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        const_values: &HashMap<String, i128>,
        parameter_values: &HashMap<String, impl Copy + Into<ConstScalarValue>>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<ConstFactValue>, String> {
        evaluate_const_fact_expression_at(
            syntax,
            expression,
            const_values,
            parameter_values,
            selection,
            None,
            warnings,
        )
    }

    /*
        Replay facts with the selected domain's lexical subject, if any. Aggregate
        subjects expose scalar-decodable fields or elements, not whole-value scalars.
    */
    fn evaluate_const_fact_expression_at(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        const_values: &HashMap<String, i128>,
        parameter_values: &HashMap<String, impl Copy + Into<ConstScalarValue>>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        subject: Option<ConstFactSubject<'_>>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<ConstFactValue>, String> {
        macro_rules! evaluate_child {
            ($expression:expr $(,)?) => {
                evaluate_const_fact_expression_at(
                    syntax,
                    $expression,
                    const_values,
                    parameter_values,
                    selection,
                    subject,
                    warnings,
                )
            };
        }
        if let Some(ConstFactSubject::Fields(fields)) = subject
            && let Some(value) = const_field_projection(syntax, expression, fields, &mut |index| {
                evaluate_child!(index)
            })?
        {
            return Ok(value
                .and_then(node_subject_leaf)
                .map(ConstScalarValue::into_fact_value));
        }
        if let Some(value) = evaluate_anonymous_numeric_expression(syntax, expression)? {
            return Ok(Some(ConstFactValue::Anonymous(value)));
        }
        let warning_start = warnings.len();
        let result = (|| match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(literal) => {
                let value = integer_literal_value(literal).ok_or_else(|| {
                    "integer operand must fit the signed/unsigned 64-bit envelope".to_string()
                })?;
                let Some(landing) = literal.landing() else {
                    return Ok(Some(ConstFactValue::Integer(value)));
                };
                if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact {
                    return Ok(None);
                }
                let integer_type = landing.landed_type;
                validate_declared_integer(integer_type, value)?;
                Ok(Some(ConstFactValue::DeclaredInteger {
                    value,
                    integer_type,
                }))
            }
            ExpressionNode::Boolean(value) => Ok(Some(ConstFactValue::Boolean(*value))),
            ExpressionNode::Name(path) => {
                let members = syntax.expressions.identifier_path_members(*path);
                let name = members.joined_path_in(syntax);
                if let Some(subject) = subject.filter(|subject| subject.matches(syntax, expression))
                {
                    return Ok(subject.scalar().map(ConstScalarValue::into_fact_value));
                }
                if let Some(value) = parameter_values.get(&name) {
                    return Ok(Some((*value).into().into_fact_value()));
                }
                if let Some(selection) = selection {
                    let Some(first) = members.first() else {
                        return Ok(None);
                    };
                    let mut span = first.source_span();
                    span.span.end = members
                        .last()
                        .expect("name has a first member")
                        .source_span()
                        .span
                        .end;
                    let identifier = Identifier::new(&name, span);
                    let Some(definition) = selection
                        .select(syntax, &identifier)
                        .map_err(|diagnostic| diagnostic.message)?
                    else {
                        return Ok(None);
                    };
                    let Ok(value) = canonicalize_selected_declaration_value(
                        syntax,
                        &definition,
                        Some(selection),
                    ) else {
                        return Ok(None);
                    };
                    return Ok(ConstScalarValue::from_canonical(&value)
                        .map(ConstScalarValue::into_fact_value));
                }
                reject_module_constant_selection(
                    syntax,
                    &name,
                    members
                        .first()
                        .map(|member| member.source_span())
                        .unwrap_or_default(),
                )?;
                let Some(value) = const_values.get(&name).copied() else {
                    return Ok(None);
                };
                let mut definitions = syntax.root_items().filter_map(|item| match item {
                    Item::Const(definition) if qualified_const_name(syntax, definition) == name => {
                        Some(definition)
                    }
                    _ => None,
                });
                let Some(definition) = definitions.next() else {
                    return Ok(None);
                };
                if definitions.next().is_some() {
                    return Ok(None);
                }
                let Some(integer_type) =
                    super::arguments::const_integer_type(syntax, definition.type_reference)
                else {
                    return Ok(None);
                };
                validate_declared_integer(integer_type, value)?;
                Ok(Some(ConstFactValue::DeclaredInteger {
                    value,
                    integer_type,
                }))
            }
            ExpressionNode::Member(member) => {
                let Some(fields) = subject.and_then(ConstFactSubject::fields) else {
                    return Ok(None);
                };
                if member.case_variant.is_some()
                    || !subject.is_some_and(|subject| subject.matches(syntax, member.receiver))
                {
                    return Ok(None);
                }
                Ok(fields
                    .iter()
                    .find(|(name, _)| name.as_str() == member.member.as_str(syntax))
                    .map(|(_, value)| value.into_fact_value()))
            }
            ExpressionNode::Indexed(indexed) => {
                let Some(ConstSubjectBinding::Elements(elements)) =
                    subject.and_then(ConstFactSubject::binding)
                else {
                    return Ok(None);
                };
                if !subject.is_some_and(|subject| subject.matches(syntax, indexed.collection)) {
                    return Ok(None);
                }
                let Some(index_value) = evaluate_child!(indexed.index)? else {
                    return Ok(None);
                };
                let Some(index) = index_value.into_integer(syntax, warnings)? else {
                    return Ok(None);
                };
                let Ok(index) = usize::try_from(index) else {
                    return Ok(None);
                };
                Ok(elements
                    .get(index)
                    .copied()
                    .flatten()
                    .map(|value| value.into_fact_value()))
            }
            ExpressionNode::Unary(unary) => match unary.operator {
                UnaryOperator::LogicalNot => Ok(evaluate_child!(unary.operand)?.and_then(
                    |value| match value {
                        ConstFactValue::Boolean(value) => Some(ConstFactValue::Boolean(!value)),
                        _ => None,
                    },
                )),
                UnaryOperator::BitwiseNot => Ok(None),
            },
            ExpressionNode::Binary(binary) => {
                let builtin = has_builtin_const_operator(syntax, binary);
                if !builtin
                    && matches!(
                        subject.and_then(ConstFactSubject::scalar),
                        Some(ConstScalarValue::Integer(_))
                    )
                {
                    return Ok(None);
                }
                validate_anonymous_remainder(syntax, binary)?;
                let Some(left) = evaluate_child!(binary.left)? else {
                    return Ok(None);
                };
                let Some(right) = evaluate_child!(binary.right)? else {
                    return Ok(None);
                };
                if !builtin && authored_operator_can_bind(syntax, binary.operator, &left, &right) {
                    return Ok(None);
                }
                evaluate_const_fact_binary(
                    syntax,
                    expression,
                    binary.operator,
                    left,
                    right,
                    warnings,
                )
                .map(Some)
            }
            _ => Ok(None),
        })();
        if !matches!(result, Ok(Some(_))) {
            warnings.truncate(warning_start);
        }
        result
    }

    /*
        Discharge `N in Domain` when `N` is a concrete const parameter and the
        domain is defined by evaluable boolean facts over the subject. Machine-call facts
        stay on the concrete record for typed build-time evaluation.
    */
    pub(crate) fn evaluate_const_membership_fact(
        syntax: &SyntaxTrees,
        membership: &syntax_trees::item::ProofMembershipFact,
        const_values: &HashMap<String, i128>,
        parameter_values: &HashMap<String, i128>,
        parameter_type_names: &HashMap<String, String>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<bool>, String> {
        if !membership.domain_arguments.is_empty() {
            return Ok(None);
        }
        let ExpressionNode::Name(value_path) = syntax.expressions.expression(membership.value)
        else {
            return Ok(None);
        };
        let [parameter_name] = syntax.expressions.identifier_path_members(*value_path) else {
            return Ok(None);
        };
        let Some(value) = parameter_values.get(parameter_name.as_str(syntax)).copied() else {
            return Ok(None);
        };
        let Some(parameter_type) = parameter_type_names.get(parameter_name.as_str(syntax)) else {
            return Ok(None);
        };
        let members = syntax.items.identifier_path_members(membership.domain);
        let domain_path = members.joined_path_in(syntax);
        evaluate_named_const_domain(
            syntax,
            &domain_path,
            parameter_type,
            ConstSubjectBinding::Scalar(ConstScalarValue::Integer(value)),
            const_values,
            &mut Vec::new(),
            members
                .first()
                .map(|member| member.source_span())
                .unwrap_or_default(),
            selection,
            warnings,
        )
        .map(|proof| proof.map(|proof| proof.holds()))
    }

    /*
        The subject operand one selected domain replay binds for its carrier: a
        scalar payload for whole-value operands, a record or variant carrier's
        scalar-decodable field ledger for `subject.<name>` projection, or an array
        carrier's scalar-decodable element ledger for `subject[<index>]` projection.
        Aggregate carriers never bind a whole-subject scalar — whole-aggregate
        comparisons, deeper member chains, and slots without a scalar leaf stay
        undecided and their facts keep the declaration fenced.
    */
    #[derive(Clone, Copy)]
    enum ConstSubjectBinding<'a> {
        Scalar(ConstScalarValue),
        Record(&'a [(String, ConstScalarValue)]),
        Elements(&'a [Option<ConstScalarValue>]),
        /*
            The scalar-decodable payload field ledger of a variant carrier's
            statically selected case.
        */
        Variant(&'a [(String, ConstScalarValue)]),
    }

    /*
        The lexical value scope of one predicate. Unavailable fields or generic
        subjects still shadow outer constants and index bindings with the same name.
    */
    #[derive(Clone, Copy)]
    enum ConstFactSubject<'a> {
        Value {
            binding: Option<ConstSubjectBinding<'a>>,
        },
        /*
            Bare lexical fields in a data or active-case predicate. The complete
            nodes retain shadowing and compositional member/index projections.
        */
        Fields(&'a [(&'a str, &'a CanonicalConstNode)]),
    }

    impl<'a> ConstFactSubject<'a> {
        fn matches(self, syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
            let Self::Value { .. } = self else {
                return false;
            };
            let ExpressionNode::Name(path) = syntax.expressions.expression(expression) else {
                return false;
            };
            matches!(syntax.expressions.identifier_path_members(*path), [name] if name.as_str(syntax) == "self")
        }

        fn binding(self) -> Option<ConstSubjectBinding<'a>> {
            match self {
                Self::Value { binding, .. } => binding,
                Self::Fields(_) => None,
            }
        }

        fn scalar(self) -> Option<ConstScalarValue> {
            match self.binding() {
                Some(ConstSubjectBinding::Scalar(value)) => Some(value),
                _ => None,
            }
        }

        fn fields(self) -> Option<&'a [(String, ConstScalarValue)]> {
            match self.binding() {
                Some(
                    ConstSubjectBinding::Record(fields) | ConstSubjectBinding::Variant(fields),
                ) => Some(fields),
                _ => None,
            }
        }
    }

    fn node_subject_leaf(value: &CanonicalConstNode) -> Option<ConstScalarValue> {
        match value {
            CanonicalConstNode::Integer { type_name, value } => {
                LandedIntegerType::from_name(type_name).map(|integer_type| {
                    ConstScalarValue::DeclaredInteger {
                        value: *value,
                        integer_type,
                    }
                })
            }
            CanonicalConstNode::Boolean(value) => Some(ConstScalarValue::Boolean(*value)),
            _ => None,
        }
    }

    /*
        `Some(None)` is a lexical field with an unavailable projection, not an
        unbound name. Keeping that distinction prevents a same-spelled constant
        from proving a predicate about an aggregate field or invalid array slot.
    */
    fn const_field_projection<'value>(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        fields: &[(&str, &'value CanonicalConstNode)],
        evaluate_index: &mut impl FnMut(ExpressionHandle) -> Result<Option<ConstFactValue>, String>,
    ) -> Result<Option<Option<&'value CanonicalConstNode>>, String> {
        use super::values::CanonicalConstNode;
        match syntax.expressions.expression(expression) {
            ExpressionNode::Name(path) => {
                let [name] = syntax.expressions.identifier_path_members(*path) else {
                    return Ok(None);
                };
                Ok(fields
                    .iter()
                    .find(|(field, _)| *field == name.as_str(syntax))
                    .map(|(_, value)| Some(*value)))
            }
            ExpressionNode::Member(member) => {
                let logical_subject = match syntax.expressions.expression(member.receiver) {
                    ExpressionNode::Name(path) => {
                        matches!(syntax.expressions.identifier_path_members(*path), [name]
                        if name.as_str(syntax) == "self")
                    }
                    _ => false,
                };
                if logical_subject {
                    return Ok(Some(if member.case_variant.is_none() {
                        fields
                            .iter()
                            .find(|(field, _)| *field == member.member.as_str(syntax))
                            .map(|(_, value)| *value)
                    } else {
                        None
                    }));
                }
                let Some(receiver) =
                    const_field_projection(syntax, member.receiver, fields, evaluate_index)?
                else {
                    return Ok(None);
                };
                let projected = match receiver {
                    Some(CanonicalConstNode::Record { fields, .. })
                        if member.case_variant.is_none() =>
                    {
                        fields
                            .iter()
                            .find(|(name, _)| name == member.member.as_str(syntax))
                            .map(|(_, value)| value)
                    }
                    _ => None,
                };
                Ok(Some(projected))
            }
            ExpressionNode::Indexed(indexed) => {
                let Some(collection) =
                    const_field_projection(syntax, indexed.collection, fields, evaluate_index)?
                else {
                    return Ok(None);
                };
                let Some(CanonicalConstNode::Array { values, .. }) = collection else {
                    return Ok(Some(None));
                };
                let mut warnings = Vec::new();
                let index = evaluate_index(indexed.index)?
                    .map(|value| value.into_integer(syntax, &mut warnings))
                    .transpose()?
                    .flatten();
                // An index is an implicit integer landing too.
                if let Some(error) = warnings.iter().find(|warning| warning.is_error()) {
                    return Err(error.message.clone());
                }
                let selected = index
                    .and_then(|value| usize::try_from(value).ok())
                    .and_then(|index| values.get(index));
                Ok(Some(selected))
            }
            _ => Ok(None),
        }
    }

    /*
        Default-domain and active-case facts belong to the selected value, not its
        ability to serve as a canonical static index. Close them with the existing
        exact fact evaluator before ordinary declaration encoding. Recursion through
        canonical construction establishes nested values independently; unsupported
        facts remain fenced, and inactive cases contribute no establishment duty.
    */
    pub(super) fn prove_const_data_predicates(
        syntax: &SyntaxTrees,
        definition: &syntax_trees::item::DataDefinition,
        value: &CanonicalConstNode,
        substitution: &crate::preparation::declaration_selection::GenericApplicationSubstitution,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> Result<(), String> {
        use super::values::CanonicalConstNode;
        let (fields, case_name) = match value {
            CanonicalConstNode::Record { fields, .. } => (fields, None),
            CanonicalConstNode::Variant {
                fields, case_name, ..
            } => (fields, Some(case_name.as_str())),
            _ => return Err("data constant lost its completed structural value".to_owned()),
        };
        let variant = case_name.and_then(|case_name| {
            syntax_items(syntax)
                .data_members(definition.members)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str(syntax) == case_name => {
                        Some(variant)
                    }
                    _ => None,
                })
        });
        if definition.requires_facts.is_empty()
            && variant.is_none_or(|variant| variant.requires_facts.is_empty())
        {
            return Ok(());
        }
        let const_values = lexical_integer_const_values(syntax);
        let mut parameter_values = HashMap::<String, ConstScalarValue>::new();
        let mut warnings = Vec::new();
        for parameter in syntax.items.type_parameters(definition.type_parameters) {
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                continue;
            };
            let argument = substitution
                .get(parameter.name.as_str(syntax))
                .ok_or_else(|| {
                    "data constant predicates require closed index bindings".to_owned()
                })?;
            let bound = evaluate_domain_index_argument(
                syntax,
                definition.name.as_str(syntax),
                parameter.name.as_str(syntax),
                type_reference,
                *argument,
                &const_values,
                &parameter_values,
                syntax.items.type_parameters(definition.type_parameters),
                selection,
                &mut warnings,
            )?
            .ok_or_else(|| {
                "data constant predicates require decidable index bindings".to_owned()
            })?;
            parameter_values.insert(parameter.name.as_str(syntax).to_owned(), bound);
        }
        let active_fields = fields
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect::<Vec<_>>();
        let members = syntax.items.data_members(definition.members);
        let common_fields = active_fields
            .iter()
            .copied()
            .filter(|(name, _)| {
                members.iter().any(|member| match member {
                    DataMember::Field(field) => field.name.as_str(syntax) == *name,
                    _ => false,
                })
            })
            .collect::<Vec<_>>();
        for (facts, fields, label) in [
            (
                definition.requires_facts,
                common_fields.as_slice(),
                "default-domain",
            ),
            (
                variant.map_or(HandleSpan::empty(), |variant| variant.requires_facts),
                active_fields.as_slice(),
                "case",
            ),
        ] {
            let subject = Some(ConstFactSubject::Fields(fields));
            for fact in syntax.items.proof_facts(facts) {
                let proved = evaluate_selected_const_fact(
                    syntax,
                    fact,
                    &const_values,
                    &parameter_values,
                    syntax.items.type_parameters(definition.type_parameters),
                    subject,
                    definition.name.as_str(syntax),
                    &mut Vec::new(),
                    selection,
                    &mut warnings,
                )?;
                match proved {
                    Some(ConstFactValue::Boolean(true)) => {}
                    Some(ConstFactValue::Boolean(false)) => {
                        return Err(format!(
                            "data constant `{}` violates its {label} predicate",
                            definition.name.as_str(syntax)
                        ));
                    }
                    _ => {
                        return Err(format!(
                            "data constant `{}` requires declaration-site {label} proof checking",
                            definition.name.as_str(syntax)
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /*
        Data construction and named-domain establishment evaluate the same proof
        facts with their actual lexical bindings and selected membership owners.
    */
    fn evaluate_selected_const_fact(
        syntax: &SyntaxTrees,
        fact: &ProofFact,
        const_values: &HashMap<String, i128>,
        parameter_values: &HashMap<String, ConstScalarValue>,
        parameters: &[syntax_trees::item::TypeParameter],
        subject: Option<ConstFactSubject<'_>>,
        carrier: &str,
        visiting: &mut Vec<(source::SourceSpan, String)>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<ConstFactValue>, String> {
        let membership = match fact {
            ProofFact::Membership(membership) => membership,
            ProofFact::Expression(expression) => {
                return evaluate_const_domain_expression(
                    syntax,
                    *expression,
                    const_values,
                    parameter_values,
                    subject,
                    carrier,
                    visiting,
                    selection,
                    warnings,
                );
            }
        };
        let Some(nested_value) = evaluate_const_fact_expression_at(
            syntax,
            membership.value,
            const_values,
            parameter_values,
            selection,
            subject,
            warnings,
        )?
        else {
            return Ok(None);
        };
        let Some((nested_carrier, nested_value)) =
            nested_membership_operand(syntax, nested_value, carrier, warnings)?
        else {
            return Ok(None);
        };
        let members = syntax.items.identifier_path_members(membership.domain);
        let path = members.joined_path_in(syntax);
        let reference = members
            .first()
            .map(|member| member.source_span())
            .unwrap_or_default();
        let holds = if membership.domain_arguments.is_empty() {
            evaluate_named_const_domain(
                syntax,
                &path,
                nested_carrier,
                ConstSubjectBinding::Scalar(nested_value),
                const_values,
                visiting,
                reference,
                selection,
                warnings,
            )?
        } else {
            evaluate_indexed_const_domain(
                syntax,
                &path,
                membership.domain_arguments,
                nested_carrier,
                ConstSubjectBinding::Scalar(nested_value),
                const_values,
                parameter_values,
                parameters,
                visiting,
                reference,
                selection,
                warnings,
            )?
        };
        Ok(holds.map(|proof| ConstFactValue::Boolean(proof.holds())))
    }

    /*
        A refuted membership is still a Boolean operand. Keep its selected atom
        separately so direct constant establishment can explain the failed alias
        without turning false into an evaluation error inside a larger predicate.
    */
    enum DomainFactProof {
        Holds,
        Refuted { domain: String },
    }

    impl DomainFactProof {
        fn holds(&self) -> bool {
            matches!(self, Self::Holds)
        }
    }

    /*
        Evaluate `value in <authored>` against one declared domain. With a constant
        selection the authored path selects its exact owner under module name law —
        the same selection full resolution later assigns the retained fact — so
        same-spelled domains in sibling modules no longer collide and an ambiguous
        or unauthorized occurrence declines instead of guessing. Without a
        selection (header-free probes) a leaf expands against the value carrier
        exactly as the original name-only lookup did.
    */
    fn evaluate_const_domain_constraint(
        syntax: &SyntaxTrees,
        domain: &syntax_trees::types::DomainConstraint,
        carrier: &str,
        subject_binding: ConstSubjectBinding<'_>,
        const_values: &HashMap<String, i128>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<DomainFactProof>, String> {
        if domain.arguments.is_empty() {
            evaluate_named_const_domain(
                syntax,
                domain.name.as_str(syntax),
                carrier,
                subject_binding,
                const_values,
                &mut Vec::new(),
                domain.name.source_span(),
                selection,
                warnings,
            )
        } else {
            evaluate_indexed_const_domain(
                syntax,
                domain.name.as_str(syntax),
                domain.arguments,
                carrier,
                subject_binding,
                const_values,
                &HashMap::new(),
                &[],
                &mut Vec::new(),
                domain.name.source_span(),
                selection,
                warnings,
            )
        }
    }

    fn evaluate_named_const_domain(
        syntax: &SyntaxTrees,
        authored: &str,
        carrier: &str,
        subject_binding: ConstSubjectBinding<'_>,
        const_values: &HashMap<String, i128>,
        visiting: &mut Vec<(source::SourceSpan, String)>,
        reference: source::SourceSpan,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<DomainFactProof>, String> {
        let domain = match selection {
            Some(selection) => selection
                .domain(syntax, authored, reference, Some(carrier))
                .or_else(|| {
                    selection
                        .domain_family(syntax, authored, reference, Some(carrier))
                        .map(|(_, definition)| definition)
                }),
            None => {
                let expanded = format!("{carrier}::{authored}");
                let mut matches = syntax.root_items().filter_map(|item| {
                    let domain = item.as_domain()?;
                    (domain.name.as_str(syntax) == authored
                        || domain.name.as_str(syntax) == expanded)
                        .then_some(domain)
                });
                matches.next().filter(|_| matches.next().is_none())
            }
        };
        let Some(domain) = domain else {
            if authored == "Carry::Portable" || CarryPermission::from_name(authored).is_some() {
                return Err(format!(
                    "domain constraint `{authored}` names a compiler carry permission; const declarations cannot supply its boundary-grant evidence",
                ));
            }
            return Ok(None);
        };
        let Some(index_parameters) = domain_index_parameters(syntax, domain) else {
            return Ok(None);
        };
        if !index_parameters.is_empty() {
            return Err(format!(
                "indexed domain `{authored}` requires {} closed index argument(s), but 0 were supplied",
                index_parameters.len(),
            ));
        }
        let domain_key = module_domain_key(syntax, domain);
        evaluate_selected_domain_facts(
            syntax,
            domain,
            domain_key,
            carrier,
            subject_binding,
            const_values,
            &HashMap::new(),
            visiting,
            selection,
            warnings,
        )
    }

    /*
        The selected declaration's complete logical path — module prefix plus the
        declared name. This label alone is not declaration identity: distinct
        packages can publish identical logical paths.
    */
    fn module_domain_key(syntax: &SyntaxTrees, domain: &DomainDefinition) -> String {
        match module_path(syntax, domain.name.source_span().source_id) {
            Some(module) => format!("{module}::{}", domain.name.as_str(syntax)),
            None => domain.name.as_str(syntax).to_owned(),
        }
    }

    /*
        Whether the concrete const carrier satisfies every requirement a domain
        declares on its abstract carrier parameter — the checked application
        evidence the bound calls for. Builtin scalar carriers are `copy` and ride
        the permissive carry policy; a named `data` carrier answers from its
        authored `properties`. Any other carrier declines rather than guessing.
    */
    fn carrier_satisfies_declared_bounds(
        syntax: &SyntaxTrees,
        carrier: &str,
        bounds: &syntax_trees::item::DataProperties,
    ) -> bool {
        let properties = crate::constant::is_builtin_scalar_carrier(carrier)
            .then_some(syntax_trees::item::DataProperties {
                multiplicity: Multiplicity::Unrestricted,
                carry: Some(CarryPolicy::PERMISSIVE),
            })
            .or_else(|| {
                syntax.root_items().find_map(|item| match item {
                    Item::Data(definition) if definition.name.as_str(syntax) == carrier => {
                        Some(definition.properties)
                    }
                    _ => None,
                })
            });
        let Some(properties) = properties else {
            return false;
        };
        match bounds.multiplicity {
            Multiplicity::Unrestricted if properties.multiplicity != Multiplicity::Unrestricted => {
                return false;
            }
            Multiplicity::Linear if properties.multiplicity != Multiplicity::Linear => {
                return false;
            }
            _ => {}
        }
        if let Some(required) = bounds.carry {
            let effective = properties.carry.unwrap_or(CarryPolicy::STRICT);
            if !effective.permits(required) {
                return false;
            }
        }
        true
    }

    /*
        Replay one already-selected domain declaration's facts against `value`
        with the subject bound and the application's closed index binders mapped in
        `parameter_values` (empty for a monomorphic domain). Unindexed and closed
        indexed membership share this body so both apply the same carrier check,
        recursion guard, and fact order. Nested indexed memberships reuse the
        same application evaluator with the enclosing binder values; their own
        authored spans select declarations in the fact author's source context.
    */
    fn evaluate_selected_domain_facts(
        syntax: &SyntaxTrees,
        domain: &DomainDefinition,
        domain_key: String,
        carrier: &str,
        subject_binding: ConstSubjectBinding<'_>,
        const_values: &HashMap<String, i128>,
        parameter_values: &HashMap<String, ConstScalarValue>,
        visiting: &mut Vec<(source::SourceSpan, String)>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<DomainFactProof>, String> {
        if visiting
            .iter()
            .any(|(span, name)| *span == domain.name.source_span() && name == &domain_key)
        {
            return Ok(None);
        }
        if !domain.authored_routes.is_empty() {
            return Ok(None);
        }
        let domain_target_name = match syntax.type_references.type_reference(domain.target_type) {
            TypeReferenceNode::Named(name) => Some(name.as_str(syntax)),
            _ => None,
        };
        let carrier_parameter = syntax_items(syntax)
            .type_parameters(domain.type_parameters)
            .first()
            .filter(|parameter| {
                matches!(parameter.kind, TypeParameterKind::Type)
                    && domain_target_name == Some(parameter.name.as_str(syntax))
            });
        let binding = if let Some(parameter) = carrier_parameter {
            if parameter.bounds != syntax_trees::item::DataProperties::default()
                && !carrier_satisfies_declared_bounds(syntax, carrier, &parameter.bounds)
            {
                return Ok(None);
            }
            None
        } else {
            let domain_carrier = match syntax.type_references.type_reference(domain.target_type) {
                TypeReferenceNode::Named(name) => name.as_str(syntax).to_owned(),
                TypeReferenceNode::FixedArray { .. } => {
                    match syntax_type_identity(syntax, domain.target_type) {
                        Ok(identity) => identity,
                        Err(_) => return Ok(None),
                    }
                }
                _ => return Ok(None),
            };
            if domain_carrier != carrier {
                return Err(format!(
                    "domain `{domain_key}` has carrier `{domain_carrier}`, but the const value has carrier `{carrier}`",
                ));
            }
            Some(subject_binding)
        };
        let subject = Some(ConstFactSubject::Value { binding });
        let warning_start = warnings.len();
        visiting.push((domain.name.source_span(), domain_key));
        let result = (|| {
            if let Some(alias) = &domain.alias {
                if alias.constituents.is_empty() {
                    return Ok(None);
                }
                for constituent in &alias.constituents {
                    let members = syntax.items.identifier_path_members(*constituent);
                    let Some(first) = members.first() else {
                        return Ok(None);
                    };
                    let path = members.joined_path_in(syntax);
                    match evaluate_named_const_domain(
                        syntax,
                        &path,
                        carrier,
                        subject_binding,
                        const_values,
                        visiting,
                        first.source_span(),
                        selection,
                        warnings,
                    )? {
                        Some(DomainFactProof::Holds) => {}
                        Some(refuted) => return Ok(Some(refuted)),
                        None => return Ok(None),
                    }
                }
            }
            for fact in syntax.items.proof_facts(domain.facts) {
                let holds = evaluate_selected_const_fact(
                    syntax,
                    fact,
                    const_values,
                    parameter_values,
                    syntax.items.type_parameters(domain.type_parameters),
                    subject,
                    carrier,
                    visiting,
                    selection,
                    warnings,
                )?;
                let Some(ConstFactValue::Boolean(holds)) = holds else {
                    return Ok(None);
                };
                if !holds {
                    return Ok(Some(DomainFactProof::Refuted {
                        domain: module_domain_key(syntax, domain),
                    }));
                }
            }
            Ok(Some(DomainFactProof::Holds))
        })();
        visiting.pop();
        if !matches!(result, Ok(Some(_))) {
            warnings.truncate(warning_start);
        }
        result
    }

    /*
        Bind one evaluated nested-membership operand as the subject payload the next
        selected domain replays. A Boolean operand always selects the `bool`
        carrier; an integer operand keeps the enclosing carrier, which
        the subject-derived expressions inherit. `Ok(None)` leaves the membership on the
        checked-record path rather than binding a guessed payload.
    */
    fn nested_membership_operand<'a>(
        syntax: &SyntaxTrees,
        value: ConstFactValue,
        carrier: &'a str,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<(&'a str, ConstScalarValue)>, String> {
        match value {
            ConstFactValue::Boolean(value) => Ok(Some(("bool", ConstScalarValue::Boolean(value)))),
            ConstFactValue::DeclaredInteger {
                value,
                integer_type,
            } => Ok(Some((
                integer_type.name(),
                ConstScalarValue::DeclaredInteger {
                    value,
                    integer_type,
                },
            ))),
            value => Ok(value
                .into_integer(syntax, warnings)?
                .map(|value| (carrier, ConstScalarValue::Integer(value)))),
        }
    }

    /*
        Evaluate `value in <authored><args>` — a closed index application of a
        generic domain family — at declaration site. The authored spelling selects
        the family under the same module name law `domain` applies to monomorphic
        domains (a qualified path names its exact owner, module-local precedence
        ranks same-leaf candidates, and relative spellings stay import-gated);
        contested, unreachable, or non-generic owners decline. Every index
        parameter must be an integer or Boolean const binder and every argument
        must close to that exact carrier against the enclosing scope's bindings; the
        family's facts then replay through the same evaluator monomorphic domains
        use, with the subject bound to `value` and each binder mapped to its argument.
        A non-scalar or non-const index parameter,
        an argument that stays open (enclosing binders, module-constant spellings,
        names still owned by resolved selection), and an unprovable fact all
        decline with `None` so the declaration keeps its fence instead of
        discharging against a guessed binding.
    */
    fn evaluate_indexed_const_domain(
        syntax: &SyntaxTrees,
        authored: &str,
        argument_span: HandleSpan<TypeReferenceHandle>,
        carrier: &str,
        subject_binding: ConstSubjectBinding<'_>,
        const_values: &HashMap<String, i128>,
        argument_values: &HashMap<String, ConstScalarValue>,
        argument_parameters: &[syntax_trees::item::TypeParameter],
        visiting: &mut Vec<(source::SourceSpan, String)>,
        reference: source::SourceSpan,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<DomainFactProof>, String> {
        let domain = match selection {
            Some(selection) => selection
                .domain_family(syntax, authored, reference, Some(carrier))
                .map(|(_, definition)| definition),
            None => {
                let expanded = format!("{carrier}::{authored}");
                let mut matches = syntax.root_items().filter_map(|item| match item {
                    Item::Domain(domain)
                        if domain.name.as_str(syntax) == authored
                            || domain.name.as_str(syntax) == expanded =>
                    {
                        Some(domain)
                    }
                    _ => None,
                });
                match matches.next() {
                    Some(domain)
                        if matches.next().is_none()
                            && !domain.type_parameters.is_empty()
                            && module_path(syntax, domain.name.source_span().source_id)
                                .is_none() =>
                    {
                        Some(domain)
                    }
                    _ => None,
                }
            }
        };
        let Some(domain) = domain else {
            return Ok(None);
        };
        let domain_key = module_domain_key(syntax, domain);
        let Some(index_parameters) = domain_index_parameters(syntax, domain) else {
            return Ok(None);
        };
        let arguments = syntax.type_references.type_reference_handles(argument_span);
        if arguments.len() != index_parameters.len() {
            return Err(format!(
                "indexed domain `{authored}` requires {} closed index argument(s), but {} were supplied",
                index_parameters.len(),
                arguments.len(),
            ));
        }
        let mut parameter_values = HashMap::new();
        for (parameter, argument) in index_parameters.iter().zip(arguments.iter()) {
            let (TypeParameterKind::Const {
                type_reference: parameter_type,
            }
            | TypeParameterKind::Value {
                type_reference: parameter_type,
            }) = parameter.kind
            else {
                return Ok(None);
            };
            let Some(bound) = evaluate_domain_index_argument(
                syntax,
                authored,
                parameter.name.as_str(syntax),
                parameter_type,
                *argument,
                const_values,
                argument_values,
                argument_parameters,
                selection,
                warnings,
            )?
            else {
                return Ok(None);
            };
            let required = syntax_type_identity(syntax, parameter_type)?;
            let bound = match bound {
                ConstScalarValue::Integer(bound) => {
                    if required == "bool" {
                        return Err(format!(
                            "index argument for `{authored}::{}` is an integer, expected `bool`",
                            parameter.name.as_str(syntax),
                        ));
                    }
                    crate::preparation::generic_data::validate_syntax_integer_range(
                        &required, bound,
                    )?;
                    let Some(integer_type) = LandedIntegerType::from_name(&required) else {
                        return Ok(None);
                    };
                    ConstScalarValue::DeclaredInteger {
                        value: bound,
                        integer_type,
                    }
                }
                ConstScalarValue::DeclaredInteger {
                    value,
                    integer_type,
                } => {
                    if integer_type.name() != required {
                        return Err(format!(
                            "index argument for `{authored}::{}` has canonical type `{}`, expected `{required}`",
                            parameter.name.as_str(syntax),
                            integer_type.name(),
                        ));
                    }
                    validate_declared_integer(integer_type, value)?;
                    bound
                }
                ConstScalarValue::Boolean(_) if required != "bool" => {
                    return Err(format!(
                        "index argument for `{authored}::{}` has canonical type `bool`, expected `{required}`",
                        parameter.name.as_str(syntax),
                    ));
                }
                ConstScalarValue::Boolean(_) => bound,
            };
            parameter_values.insert(parameter.name.as_str(syntax).to_owned(), bound);
        }
        evaluate_selected_domain_facts(
            syntax,
            domain,
            domain_key,
            carrier,
            subject_binding,
            const_values,
            &parameter_values,
            visiting,
            selection,
            warnings,
        )
    }

    /*
        Evaluate one supplied index argument of a domain-family application to the
        scalar its const binder receives, under the same admissible forms the
        closed-index canonicalizer accepts: literal and canonical leaves, the
        caller's bound index parameters, exactly selected constant declarations,
        and the scoped integer ledger. `Ok(None)` means the argument cannot be
        decided at this point — an unresolved name, a spelling ordinary resolution
        still owns, or a non-scalar canonical value — so the application stays
        fenced rather than binding a guessed value.
    */
    fn evaluate_domain_index_argument(
        syntax: &SyntaxTrees,
        family_name: &str,
        parameter_name: &str,
        parameter_type: TypeReferenceHandle,
        argument: TypeReferenceHandle,
        const_values: &HashMap<String, i128>,
        argument_values: &HashMap<String, ConstScalarValue>,
        argument_parameters: &[syntax_trees::item::TypeParameter],
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<ConstScalarValue>, String> {
        let integer_type =
            crate::preparation::generic_data::const_integer_type(syntax, parameter_type);
        let boolean_type = matches!(
            syntax.type_references.type_reference(parameter_type),
            TypeReferenceNode::Named(name) if name.as_str(syntax) == "bool"
        );
        if integer_type.is_none() && !boolean_type {
            return Ok(None);
        }
        match syntax.type_references.type_reference(argument) {
            TypeReferenceNode::Named(name) => {
                match ConstScalarSpelling::from_bare_name(name.as_str(syntax)) {
                    Some(ConstScalarSpelling::Canonical(atom)) => {
                        let required = syntax_type_identity(syntax, parameter_type)?;
                        if atom.type_name != required {
                            return Err(format!(
                                "index argument for `{family_name}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                                atom.type_name,
                            ));
                        }
                        return Ok(ConstScalarValue::from_canonical(&atom));
                    }
                    Some(ConstScalarSpelling::Integer(value)) => {
                        if let Some(integer_type) = integer_type {
                            validate_declared_integer(integer_type, value)?;
                            return Ok(Some(ConstScalarValue::DeclaredInteger {
                                value,
                                integer_type,
                            }));
                        }
                        return Ok(Some(ConstScalarValue::Integer(value)));
                    }
                    Some(ConstScalarSpelling::Boolean(value)) => {
                        return Ok(Some(ConstScalarValue::Boolean(value)));
                    }
                    None => {}
                }
                if let Some(value) = argument_values.get(name.as_str(syntax)) {
                    let Some(syntax_trees::item::TypeParameter { kind, .. }) = argument_parameters
                        .iter()
                        .find(|parameter| parameter.name.as_str(syntax) == name.as_str(syntax))
                    else {
                        return Ok(None);
                    };
                    let type_reference = match kind {
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference } => *type_reference,
                        _ => return Ok(None),
                    };
                    let actual = syntax_type_identity(syntax, type_reference)?;
                    let required = syntax_type_identity(syntax, parameter_type)?;
                    if actual != required {
                        return Err(format!(
                            "index argument for `{family_name}::{parameter_name}` has carrier `{actual}`, expected `{required}`"
                        ));
                    }
                    return Ok(Some(*value));
                }
                if let Some(selection) = selection
                    && let Some(definition) = selection
                        .select(syntax, name)
                        .map_err(|diagnostic| diagnostic.message)?
                {
                    let value = crate::preparation::generic_data::canonicalize_selected_const_definition(
                        syntax,
                        &definition,
                        parameter_type,
                        Some(selection),
                    )
                    .map_err(|reason| {
                        format!(
                            "index argument for `{family_name}::{parameter_name}` is invalid: {reason}"
                        )
                    })?;
                    return Ok(ConstScalarValue::from_canonical(&value));
                }
                if let Some(value) = const_values.get(name.as_str(syntax)) {
                    reject_module_constant_selection(
                        syntax,
                        name.as_str(syntax),
                        name.source_span(),
                    )?;
                    return Ok(Some(ConstScalarValue::Integer(*value)));
                }
                let mut definitions = syntax.root_items().filter_map(|item| match item {
                    Item::Const(definition)
                        if !crate::preparation::generic_data::module_constants::is_module_constant(
                            syntax, definition,
                        )
                            && qualified_const_name(syntax, definition) == name.as_str(syntax) =>
                    {
                        Some(definition)
                    }
                    _ => None,
                });
                let Some(definition) = definitions.next() else {
                    return Ok(None);
                };
                if definitions.next().is_some() {
                    return Ok(None);
                }
                reject_module_constant_selection(syntax, name.as_str(syntax), name.source_span())?;
                let value = canonicalize_const_definition(syntax, definition, parameter_type).map_err(
                |reason| {
                    format!(
                        "index argument for `{family_name}::{parameter_name}` is invalid: {reason}"
                    )
                },
            )?;
                Ok(ConstScalarValue::from_canonical(&value))
            }
            TypeReferenceNode::ConstExpression(expression) => {
                if syntax.const_expression_contains_name(*expression)
                    || super::anonymous::requires_const_operator_selection(syntax, *expression)
                {
                    return Ok(None);
                }
                if boolean_type {
                    return Ok(None);
                }
                let value = crate::preparation::generic_data::evaluate_const_argument_expression(
                    syntax,
                    *expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    integer_type,
                    warnings,
                )
                .and_then(crate::preparation::generic_data::EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    format!("index argument expression for `{family_name}` is invalid: {reason}")
                })?;
                let required = syntax_type_identity(syntax, parameter_type)?;
                crate::preparation::generic_data::validate_syntax_integer_range(&required, value)?;
                Ok(Some(ConstScalarValue::Integer(value)))
            }
            _ => Ok(None),
        }
    }

    /*
        The shared fence for constrained const declarations whose carrier's
        constraints cannot be proved at declaration site.
    */
    const CONSTRAINED_CONST_FENCE: &str = "constrained const declarations require declaration-site proof checking before they can publish compatibility identity";

    /*
        One Domain constraint discharged over a bound const subject: `Holds`
        passes silently, `Refuted` fails naming the caller's subject, and an
        undecided fact fences rather than guessing the binding.
    */
    fn prove_const_domain_constraint(
        syntax: &SyntaxTrees,
        constraint: &TypeConstraintNode,
        carrier: &str,
        subject_binding: ConstSubjectBinding<'_>,
        const_values: &HashMap<String, i128>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
        subject_label: &str,
    ) -> Result<(), String> {
        let TypeConstraintNode::Domain(domain) = constraint else {
            return Ok(());
        };
        let holds = evaluate_const_domain_constraint(
            syntax,
            domain,
            carrier,
            subject_binding,
            const_values,
            selection,
            warnings,
        )?;
        match holds {
            Some(DomainFactProof::Holds) => Ok(()),
            Some(DomainFactProof::Refuted {
                domain: failed_domain,
            }) => Err(format!(
                "domain constraint `{}` for {subject_label} is false; failed domain `{failed_domain}`",
                domain.name.as_str(syntax),
            )),
            None => Err(CONSTRAINED_CONST_FENCE.to_owned()),
        }
    }

    /*
        Discharge the `Domain` constraints a `Constrained` leaf carrier declares —
        applied to each leaf a constrained field or
        element type names as an initializer canonicalizes through
        `canonicalize_const_expression`. Aggregate leaves bind the subject through their
        scalar-decodable members exactly as the declaration path does; a leaf
        without a bindable payload, an unselected owner, or an undecided fact stays
        fenced rather than publishing a guessed identity. Constraints of other
        kinds keep their own validation sites.
    */
    pub(in crate::preparation::generic_data) fn prove_const_leaf_domain_constraints(
        syntax: &SyntaxTrees,
        constraints: &[TypeConstraintNode],
        value: &CanonicalConstNode,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> Result<(), String> {
        use super::values::CanonicalConstNode;
        if !constraints
            .iter()
            .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
        {
            return Ok(());
        }
        let record_fields: Vec<(String, ConstScalarValue)>;
        let array_elements: Vec<Option<ConstScalarValue>>;
        let variant_fields: Vec<(String, ConstScalarValue)>;
        let (carrier, subject_binding) = match value {
            CanonicalConstNode::Integer { type_name, value } => {
                let Some(integer_type) = LandedIntegerType::from_name(type_name) else {
                    return Err(CONSTRAINED_CONST_FENCE.to_owned());
                };
                (
                    type_name.clone(),
                    ConstSubjectBinding::Scalar(ConstScalarValue::DeclaredInteger {
                        value: *value,
                        integer_type,
                    }),
                )
            }
            CanonicalConstNode::Boolean(value) => (
                "bool".to_owned(),
                ConstSubjectBinding::Scalar(ConstScalarValue::Boolean(*value)),
            ),
            CanonicalConstNode::Record { type_name, fields } => {
                record_fields = fields
                    .iter()
                    .filter_map(|(name, field)| {
                        node_subject_leaf(field).map(|value| (name.clone(), value))
                    })
                    .collect();
                (
                    type_name.clone(),
                    ConstSubjectBinding::Record(&record_fields),
                )
            }
            CanonicalConstNode::Array { type_name, values } => {
                array_elements = values.iter().map(node_subject_leaf).collect();
                (
                    type_name.clone(),
                    ConstSubjectBinding::Elements(&array_elements),
                )
            }
            CanonicalConstNode::Variant {
                type_name, fields, ..
            } => {
                variant_fields = fields
                    .iter()
                    .filter_map(|(name, field)| {
                        node_subject_leaf(field).map(|value| (name.clone(), value))
                    })
                    .collect();
                (
                    type_name.clone(),
                    ConstSubjectBinding::Variant(&variant_fields),
                )
            }
            _ => return Err(CONSTRAINED_CONST_FENCE.to_owned()),
        };
        let const_values = lexical_integer_const_values(syntax);
        let mut warnings = Vec::new();
        for constraint in constraints {
            prove_const_domain_constraint(
                syntax,
                constraint,
                &carrier,
                subject_binding,
                &const_values,
                selection,
                &mut warnings,
                "the const leaf value",
            )?;
        }
        Ok(())
    }

    /*
        Find declaration predicates through the exact selected member types. This
        only selects a required check; it grants no value or membership. Recursion
        rejoins source-owned declarations rather than guessing from a leaf spelling.
    */
    pub(crate) fn const_type_requires_declaration_proof(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        visiting: &mut Vec<source::SourceSpan>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> bool {
        use syntax_trees::item::DataMember;
        let origin = syntax_type_references(syntax).generic_application_origin(type_reference);
        if origin.is_valid() && origin != type_reference {
            return const_type_requires_declaration_proof(syntax, origin, visiting, selection);
        }
        fn named_data(
            syntax: &SyntaxTrees,
            name: &Identifier,
            visiting: &mut Vec<source::SourceSpan>,
            selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        ) -> bool {
            let Ok(data) = super::values::selected_data(syntax, name, selection) else {
                return false;
            };
            if visiting.contains(&data.name.source_span()) {
                return false;
            }
            visiting.push(data.name.source_span());
            let value_facts = super::super::template_type_equation_offsets(syntax, data, selection)
                .map_or(true, |offsets| offsets.len() != data.requires_facts.len());
            let requires_proof = value_facts
                || syntax.tables.items.data_members(data.members).iter().any(
                    |member| match member {
                        DataMember::Field(field) => const_type_requires_declaration_proof(
                            syntax,
                            field.type_reference,
                            visiting,
                            selection,
                        ),
                        DataMember::Variant(variant) => {
                            !variant.requires_facts.is_empty()
                                || syntax_items(syntax)
                                    .data_payload_fields(variant.payload)
                                    .iter()
                                    .any(|field| {
                                        const_type_requires_declaration_proof(
                                            syntax,
                                            field.type_reference,
                                            visiting,
                                            selection,
                                        )
                                    })
                        }
                        _ => false,
                    },
                );
            visiting.pop();
            requires_proof
        }
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                syntax_type_references(syntax)
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
                    || const_type_requires_declaration_proof(
                        syntax, *base_type, visiting, selection,
                    )
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                const_type_requires_declaration_proof(syntax, *element_type, visiting, selection)
            }
            TypeReferenceNode::Named(name) => named_data(syntax, name, visiting, selection),
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                syntax_type_references(syntax)
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| {
                        const_type_requires_declaration_proof(
                            syntax, *argument, visiting, selection,
                        )
                    })
                    || named_data(syntax, base_name, visiting, selection)
            }
            _ => false,
        }
    }

    /*
        A pure literal does not request initializer evaluation. Its selected data,
        active-case and qualified-member predicates still owe the same construction
        discharge as an evaluated value, even in an unused private declaration.
    */
    pub(crate) fn prove_const_literal_predicates(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    ) -> Result<(), String> {
        if crate::constant::requires_const_initializer_evaluation(syntax, definition) {
            return Ok(());
        }
        if super::super::qualification_names_itself(definition) {
            return canonicalize_selected_declared_const_definition(syntax, definition, selection)
                .map(|_| ());
        }
        if !const_type_requires_declaration_proof(
            syntax,
            definition.type_reference,
            &mut Vec::new(),
            selection,
        ) {
            return Ok(());
        }
        super::canonicalize_const_expression(
            syntax,
            definition.type_reference,
            definition.value,
            selection,
            &crate::preparation::declaration_selection::GenericApplicationSubstitution::new(),
        )
        .map(|_| ())
    }

    /*
        Evaluate one fact expression inside a selected domain declaration with
        the subject bound to the checked value only for a fixed carrier. A generic
        carrier leaves it unavailable rather than assuming an instance's algebra.
        `parameter_values` carries the
        application's closed index bindings so a family fact like `subject < N`
        resolves its binder exactly; a monomorphic domain passes an empty map.
    */
    fn evaluate_const_domain_expression(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        const_values: &HashMap<String, i128>,
        parameter_values: &HashMap<String, ConstScalarValue>,
        subject: Option<ConstFactSubject<'_>>,
        carrier: &str,
        visiting: &mut Vec<(source::SourceSpan, String)>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<ConstFactValue>, String> {
        if let Some(value) = evaluate_anonymous_numeric_expression(syntax, expression)? {
            return Ok(Some(ConstFactValue::Anonymous(value)));
        }
        let warning_start = warnings.len();
        let result = (|| match syntax.expressions.expression(expression) {
            ExpressionNode::Membership(membership) => {
                let Some(value) = evaluate_const_fact_expression_at(
                    syntax,
                    membership.value,
                    const_values,
                    parameter_values,
                    selection,
                    subject,
                    warnings,
                )?
                else {
                    return Ok(None);
                };
                let Some((nested_carrier, value)) =
                    nested_membership_operand(syntax, value, carrier, warnings)?
                else {
                    return Ok(None);
                };
                let members = syntax_expressions(syntax).identifier_path_members(membership.domain);
                let path = members.joined_path_in(syntax);
                evaluate_named_const_domain(
                    syntax,
                    &path,
                    nested_carrier,
                    ConstSubjectBinding::Scalar(value),
                    const_values,
                    visiting,
                    members
                        .first()
                        .map(|member| member.source_span())
                        .unwrap_or_default(),
                    selection,
                    warnings,
                )
                .map(|result| result.map(|proof| ConstFactValue::Boolean(proof.holds())))
            }
            ExpressionNode::Binary(binary) => {
                let builtin = has_builtin_const_operator(syntax, binary);
                if !builtin
                    && matches!(
                        subject.and_then(ConstFactSubject::scalar),
                        Some(ConstScalarValue::Integer(_))
                    )
                {
                    return Ok(None);
                }
                validate_anonymous_remainder(syntax, binary)?;
                let Some(left) = evaluate_const_domain_expression(
                    syntax,
                    binary.left,
                    const_values,
                    parameter_values,
                    subject,
                    carrier,
                    visiting,
                    selection,
                    warnings,
                )?
                else {
                    return Ok(None);
                };
                let Some(right) = evaluate_const_domain_expression(
                    syntax,
                    binary.right,
                    const_values,
                    parameter_values,
                    subject,
                    carrier,
                    visiting,
                    selection,
                    warnings,
                )?
                else {
                    return Ok(None);
                };
                if !builtin && authored_operator_can_bind(syntax, binary.operator, &left, &right) {
                    return Ok(None);
                }
                evaluate_const_fact_binary(
                    syntax,
                    expression,
                    binary.operator,
                    left,
                    right,
                    warnings,
                )
                .map(Some)
            }
            _ => evaluate_const_fact_expression_at(
                syntax,
                expression,
                const_values,
                parameter_values,
                selection,
                subject,
                warnings,
            ),
        })();
        if !matches!(result, Ok(Some(_))) {
            warnings.truncate(warning_start);
        }
        result
    }
}
mod remainder {
    /*
        Formation checks before closed const arguments or facts lose their source
        operands. A destination's const parameter type cannot type anonymous `%`.
    */

    use super::super::{BinaryOperator, SyntaxTrees};

    use super::anonymous::anonymous_numeric_expression;
    use syntax_trees::expression::TableBinaryExpression;

    pub(super) fn validate_anonymous_remainder(
        syntax: &SyntaxTrees,
        binary: &TableBinaryExpression,
    ) -> Result<(), String> {
        if binary.operator == BinaryOperator::Modulo
            && anonymous_numeric_expression(syntax, binary.left)
            && anonymous_numeric_expression(syntax, binary.right)
        {
            return Err("builtin `%` requires an integer-typed operand; type an operand (for example, `7i32 % 2`), not just the destination".to_owned());
        }
        Ok(())
    }
}
mod templates {
    /* Constant evaluation: templates. */
    use super::super::{
        DataMember, Diagnostic, ExpressionNode, HashMap, HashSet, Identifier, Item, StatementNode,
        SyntaxTrees, TypeConstraintNode, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
    };
    use crate::preparation::generic_data::uses::{
        ConstructorFrontier, collect_expression_handles, collect_statement_expression_handles,
        visit_expression_type_inputs, visit_static_argument_type_inputs,
    };
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;

    use crate::preparation::generic_data::EvaluatedConst;

    use super::super::uses::concrete_machine_declarations;
    use crate::preparation::generic_data::evaluate_const_argument_expression;
    use crate::preparation::generic_data::generic_const_integer_types;
    use arena::HandleSpan;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::item::CapabilityContract;
    use syntax_trees::item::Machine;
    use syntax_trees::item::ProofFact;

    pub(in crate::preparation::generic_data) fn replace_const_expression_names_from(
        syntax: &mut SyntaxTrees,
        expression_watermark: syntax_trees::expression::ExpressionFrontier,
        const_expressions: &HashMap<String, ExpressionNode>,
    ) {
        replace_uncaptured_const_names(
            syntax,
            expression_watermark,
            const_expressions,
            &HashSet::new(),
        );
    }

    /*
        Runtime bindings keep their authored selection. A local enters the frontier
        only after its initializer; machine-header expressions use entry parameters.
    */
    #[derive(Default)]
    pub(in crate::preparation::generic_data) struct RuntimeTemplateCaptures {
        expressions: HashSet<ExpressionHandle>,
        type_references: HashSet<TypeReferenceHandle>,
    }

    impl RuntimeTemplateCaptures {
        pub fn captures_type_reference(&self, reference: TypeReferenceHandle) -> bool {
            self.type_references.contains(&reference)
        }
    }

    pub(in crate::preparation::generic_data) fn capture_machine_runtime_template_names(
        syntax: &SyntaxTrees,
        machine: &Machine,
        type_watermark: syntax_trees::types::TypeReferenceFrontier,
        expression_watermark: syntax_trees::expression::ExpressionFrontier,
    ) -> RuntimeTemplateCaptures {
        let states = syntax.items.state_handles(machine.states);
        let parameters = states
            .first()
            .map(|state| syntax.items.state(*state).parameters)
            .unwrap_or_default();
        let mut captured = RuntimeTemplateCaptures::default();
        let header_frontier = ConstructorFrontier {
            parameters,
            prior_statements: &[],
        };
        for (handle, expression) in syntax.expressions.iter_expressions() {
            if expression_watermark.contains(handle)
                && let ExpressionNode::Name(path) = expression
                && header_frontier.captures(syntax, *path)
            {
                captured.expressions.insert(handle);
            }
        }
        for (handle, _) in syntax_type_references(syntax).named_nodes_from(type_watermark, syntax) {
            if let Some(name) = syntax.type_references.type_reference(handle).as_named()
                && header_frontier.captures_name(syntax, name)
            {
                captured.type_references.insert(handle);
            }
        }
        for state in states {
            let state = syntax.items.state(*state);
            let mut signature = TypeExpressionInputs::default();
            collect_type_expression_handles(syntax, state.return_type, &mut signature);
            for parameter in syntax.items.state_parameters(state.parameters) {
                collect_type_expression_handles(
                    syntax,
                    syntax.items.state_parameter(*parameter).type_reference,
                    &mut signature,
                );
            }
            collect_contract_expression_handles(syntax, state.contracts, &mut signature);
            let statements = syntax.items.statements(state.statements);
            update_captured_names(
                syntax,
                &mut signature,
                &ConstructorFrontier {
                    parameters: state.parameters,
                    prior_statements: &[],
                },
                &mut captured,
            );
            for (ordinal, statement) in statements.iter().enumerate() {
                let mut inputs = TypeExpressionInputs::default();
                collect_statement_expression_handles(syntax, *statement, &mut inputs.expressions);
                match syntax.statements.statement(*statement) {
                    StatementNode::LocalData(local) => {
                        collect_type_expression_handles(syntax, local.type_reference, &mut inputs);
                    }
                    StatementNode::Call(call) => {
                        visit_static_argument_type_inputs(
                            syntax,
                            &call.machine_arguments,
                            &mut |reference| {
                                collect_type_expression_handles(syntax, reference, &mut inputs);
                            },
                        );
                    }
                    _ => {}
                }
                update_captured_names(
                    syntax,
                    &mut inputs,
                    &ConstructorFrontier {
                        parameters: state.parameters,
                        prior_statements: &statements[..ordinal],
                    },
                    &mut captured,
                );
            }
        }
        captured
    }

    pub(in crate::preparation::generic_data) fn replace_machine_const_expression_names_from(
        syntax: &mut SyntaxTrees,
        expression_watermark: syntax_trees::expression::ExpressionFrontier,
        const_expressions: &HashMap<String, ExpressionNode>,
        captured: &RuntimeTemplateCaptures,
    ) {
        replace_uncaptured_const_names(
            syntax,
            expression_watermark,
            const_expressions,
            &captured.expressions,
        );
    }

    fn update_captured_names(
        syntax: &SyntaxTrees,
        inputs: &mut TypeExpressionInputs,
        frontier: &ConstructorFrontier<'_>,
        captured: &mut RuntimeTemplateCaptures,
    ) {
        let mut visited = HashSet::new();
        loop {
            let pending = inputs
                .expressions
                .difference(&visited)
                .copied()
                .collect::<Vec<_>>();
            if pending.is_empty() {
                break;
            }
            for handle in pending {
                visited.insert(handle);
                visit_expression_type_inputs(syntax, handle, &mut |reference| {
                    collect_type_expression_handles(syntax, reference, inputs);
                });
            }
        }
        for handle in &inputs.expressions {
            if let ExpressionNode::Name(path) = syntax.expressions.expression(*handle) {
                if inputs.bound_expressions.contains(handle) || frontier.captures(syntax, *path) {
                    captured.expressions.insert(*handle);
                } else {
                    captured.expressions.remove(handle);
                }
            }
        }
        for reference in &inputs.types {
            if let Some(name) = syntax.type_references.type_reference(*reference).as_named() {
                if inputs.bound_types.contains(reference) || frontier.captures_name(syntax, name) {
                    captured.type_references.insert(*reference);
                } else {
                    captured.type_references.remove(reference);
                }
            }
        }
    }

    #[derive(Default)]
    struct TypeExpressionInputs {
        expressions: HashSet<ExpressionHandle>,
        types: HashSet<TypeReferenceHandle>,
        bound_expressions: HashSet<ExpressionHandle>,
        bound_types: HashSet<TypeReferenceHandle>,
    }

    impl TypeExpressionInputs {
        fn extend(&mut self, other: Self) {
            self.expressions.extend(other.expressions);
            self.types.extend(other.types);
            self.bound_expressions.extend(other.bound_expressions);
            self.bound_types.extend(other.bound_types);
        }
    }

    struct TypeInputWalk<'syntax> {
        syntax: &'syntax SyntaxTrees,
        entered: HashSet<TypeReferenceHandle>,
    }

    impl<'syntax> TypeInputWalk<'syntax> {
        fn new(syntax: &'syntax SyntaxTrees) -> Self {
            Self {
                syntax,
                entered: HashSet::new(),
            }
        }

        fn expression(&mut self, expression: ExpressionHandle, inputs: &mut TypeExpressionInputs) {
            let syntax = self.syntax;
            let mut expressions = HashSet::new();
            collect_expression_handles(syntax, expression, &mut expressions);
            for &expression in &expressions {
                visit_expression_type_inputs(syntax, expression, &mut |reference| {
                    self.reference(reference, inputs);
                });
            }
            inputs.expressions.extend(expressions);
        }

        fn contracts(
            &mut self,
            contracts: HandleSpan<CapabilityContract>,
            inputs: &mut TypeExpressionInputs,
        ) {
            let syntax = self.syntax;
            for contract in syntax.items.capability_contracts(contracts) {
                for fact in syntax.items.proof_facts(contract.facts) {
                    match fact {
                        ProofFact::Expression(expression) => self.expression(*expression, inputs),
                        ProofFact::Membership(membership) => {
                            self.expression(membership.value, inputs);
                            for &argument in syntax
                                .type_references
                                .type_reference_handles(membership.domain_arguments)
                            {
                                self.reference(argument, inputs);
                            }
                        }
                    }
                }
            }
        }

        fn protect_signature_scope(
            &self,
            signature: &crate::preparation::callable_signatures::SignatureInputs,
            scope: usize,
            inputs: &mut TypeExpressionInputs,
        ) {
            for &expression in &inputs.expressions {
                if let ExpressionNode::Name(path) = self.syntax.expressions.expression(expression)
                    && let Some(name) = self
                        .syntax
                        .expressions
                        .identifier_path_members(*path)
                        .first()
                    && signature.hides_value(scope, name.as_str(self.syntax))
                {
                    inputs.bound_expressions.insert(expression);
                }
            }
            for &reference in &inputs.types {
                if let Some(name) = self
                    .syntax
                    .type_references
                    .type_reference(reference)
                    .as_named()
                    && signature.hides_type(scope, name.as_str(self.syntax))
                {
                    inputs.bound_types.insert(reference);
                }
            }
        }

        fn reference(&mut self, reference: TypeReferenceHandle, inputs: &mut TypeExpressionInputs) {
            if !reference.is_valid() || !self.entered.insert(reference) {
                return;
            }
            inputs.types.insert(reference);
            let syntax = self.syntax;
            match syntax.type_references.type_reference(reference) {
                TypeReferenceNode::ConstExpression(expression) => {
                    self.expression(*expression, inputs)
                }
                TypeReferenceNode::Reference { referee, .. } => self.reference(*referee, inputs),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    self.reference(*element_type, inputs);
                    if let syntax_trees::types::FixedArrayLength::Expression(value) = length {
                        self.reference(*value, inputs);
                    }
                }
                TypeReferenceNode::Slice { element_type } => self.reference(*element_type, inputs),
                TypeReferenceNode::Generic { arguments, .. }
                | TypeReferenceNode::DynamicTrait {
                    trait_arguments: arguments,
                    ..
                } => {
                    for &argument in syntax.type_references.type_reference_handles(*arguments) {
                        self.reference(argument, inputs);
                    }
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    self.reference(*base_type, inputs);
                    for constraint in syntax.type_references.constraints(*constraints) {
                        if let TypeConstraintNode::Domain(domain) = constraint {
                            for &argument in syntax
                                .type_references
                                .type_reference_handles(domain.arguments)
                            {
                                self.reference(argument, inputs);
                            }
                        }
                    }
                }
                TypeReferenceNode::Callable { signature } => {
                    let signature = crate::preparation::callable_signatures::SignatureInputs::of(
                        syntax,
                        syntax.items.state_signature(*signature),
                    );
                    for &(scope, reference) in &signature.types {
                        let mut nested = TypeExpressionInputs::default();
                        self.reference(reference, &mut nested);
                        self.protect_signature_scope(&signature, scope, &mut nested);
                        inputs.extend(nested);
                    }
                    for &(scope, expression) in &signature.expressions {
                        let mut nested = TypeExpressionInputs::default();
                        self.expression(expression, &mut nested);
                        self.protect_signature_scope(&signature, scope, &mut nested);
                        inputs.extend(nested);
                    }
                }
                TypeReferenceNode::StaticSelection(selection) => {
                    visit_static_argument_type_inputs(
                        syntax,
                        std::slice::from_ref(selection),
                        &mut |reference| {
                            self.reference(reference, inputs);
                        },
                    );
                }
                TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
            }
        }
    }

    fn collect_contract_expression_handles(
        syntax: &SyntaxTrees,
        contracts: HandleSpan<CapabilityContract>,
        inputs: &mut TypeExpressionInputs,
    ) {
        TypeInputWalk::new(syntax).contracts(contracts, inputs);
    }

    fn collect_type_expression_handles(
        syntax: &SyntaxTrees,
        reference: TypeReferenceHandle,
        inputs: &mut TypeExpressionInputs,
    ) {
        TypeInputWalk::new(syntax).reference(reference, inputs);
    }

    fn replace_uncaptured_const_names(
        syntax: &mut SyntaxTrees,
        expression_watermark: syntax_trees::expression::ExpressionFrontier,
        const_expressions: &HashMap<String, ExpressionNode>,
        captured: &HashSet<ExpressionHandle>,
    ) {
        let replacements = syntax_expressions(syntax)
            .iter_expressions()
            .filter(|(handle, _)| expression_watermark.contains(*handle))
            .filter(|(handle, _)| !captured.contains(handle))
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Name(path) = expression else {
                    return None;
                };
                let [name] = syntax.expressions.identifier_path_members(*path) else {
                    return None;
                };
                const_expressions
                    .get(name.as_str(syntax))
                    .cloned()
                    .map(|value| (handle, value))
            })
            .collect::<Vec<_>>();
        for (handle, value) in replacements {
            syntax.expressions.replace_expression(handle, value);
        }
    }

    /*
        Generic definitions remain in the tree after their concrete records are
        synthesized so the normal frontend can validate the template. A symbolic
        const expression cannot cross that boundary yet, so reduce each template
        expression to either its concrete value or one declared const-parameter
        dependency. The concrete clones already carry the fully evaluated value;
        this placeholder exists only to preserve the established generic type/kind
        checks on the source template.
    */
    pub(in crate::preparation::generic_data) fn normalize_generic_template_const_expressions(
        syntax: &mut SyntaxTrees,
        const_values: &HashMap<String, i128>,
        warnings: &mut Vec<Diagnostic>,
        declarations: &[syntax_trees::item::ItemHandle],
    ) -> Result<(), Diagnostic> {
        let templates: Vec<(String, HashSet<String>, Vec<TypeReferenceHandle>)> = declarations
            .iter()
            .map(|declaration| syntax.root_item(*declaration))
            .filter_map(|item| {
                let definition = item.as_data()?;
                if definition.type_parameters.is_empty() {
                    return None;
                }
                let symbolic_parameters = syntax_items(syntax)
                    .type_parameters(definition.type_parameters)
                    .iter()
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
                    .map(|parameter| parameter.name.as_str(syntax).to_string())
                    .collect();
                let fields = syntax_items(syntax)
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field.type_reference),
                        DataMember::Variant(_) => None,
                        DataMember::Retired(_) => None,
                    })
                    .collect();
                Some((
                    definition.name.as_str(syntax).to_string(),
                    symbolic_parameters,
                    fields,
                ))
            })
            .collect();

        for (template, symbolic_parameters, fields) in templates {
            for field in fields {
                normalize_template_type_reference(
                syntax,
                field,
                const_values,
                &symbolic_parameters,
                warnings,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "const argument expression in generic data `{template}` is invalid: {reason}"
                ))
            })?;
            }
        }
        Ok(())
    }

    pub(in crate::preparation::generic_data) fn normalize_template_type_reference(
        syntax: &mut SyntaxTrees,
        type_reference: TypeReferenceHandle,
        const_values: &HashMap<String, i128>,
        symbolic_parameters: &HashSet<String>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<(), String> {
        let node = syntax_type_references(syntax)
            .type_reference(type_reference)
            .clone();
        match node {
            TypeReferenceNode::Callable { signature } => {
                let inputs = crate::preparation::callable_signatures::SignatureInputs::of(
                    syntax,
                    syntax.items.state_signature(signature),
                );
                let scopes = (0..inputs.scope_count())
                    .map(|scope| {
                        let constants = const_values
                            .iter()
                            .filter(|(name, _)| !inputs.hides_value(scope, name))
                            .map(|(name, value)| (name.clone(), *value))
                            .collect::<HashMap<_, _>>();
                        let mut symbolic = symbolic_parameters
                            .iter()
                            .filter(|name| !inputs.hides_value(scope, name))
                            .cloned()
                            .collect::<HashSet<_>>();
                        symbolic.extend(inputs.bound_values(scope));
                        (constants, symbolic)
                    })
                    .collect::<Vec<_>>();
                for &(scope, reference) in &inputs.types {
                    normalize_template_type_reference(
                        syntax,
                        reference,
                        &scopes[scope].0,
                        &scopes[scope].1,
                        warnings,
                    )?;
                }
                for &(scope, expression) in &inputs.expressions {
                    let mut expressions = HashSet::new();
                    collect_expression_handles(syntax, expression, &mut expressions);
                    let mut types = Vec::new();
                    for expression in expressions {
                        visit_expression_type_inputs(syntax, expression, &mut |reference| {
                            types.push(reference)
                        });
                    }
                    for reference in types {
                        normalize_template_type_reference(
                            syntax,
                            reference,
                            &scopes[scope].0,
                            &scopes[scope].1,
                            warnings,
                        )?;
                    }
                }
                Ok(())
            }
            TypeReferenceNode::Reference { referee, .. } => normalize_template_type_reference(
                syntax,
                referee,
                const_values,
                symbolic_parameters,
                warnings,
            ),
            TypeReferenceNode::Constrained { base_type, .. } => normalize_template_type_reference(
                syntax,
                base_type,
                const_values,
                symbolic_parameters,
                warnings,
            ),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => normalize_template_type_reference(
                syntax,
                element_type,
                const_values,
                symbolic_parameters,
                warnings,
            ),
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let arguments = syntax_type_references(syntax)
                    .type_reference_handles(arguments)
                    .to_vec();
                let integer_types = generic_const_integer_types(syntax, base_name.as_str(syntax));
                for (index, argument) in arguments.into_iter().enumerate() {
                    let node = syntax_type_references(syntax)
                        .type_reference(argument)
                        .clone();
                    if let TypeReferenceNode::ConstExpression(expression) = node {
                        let placeholder = evaluate_const_argument_expression(
                            syntax,
                            expression,
                            const_values,
                            &HashMap::new(),
                            symbolic_parameters,
                            integer_types.get(index).copied().flatten(),
                            warnings,
                        )?;
                        let name = match placeholder {
                            EvaluatedConst::Concrete(value) => value.to_string(),
                            EvaluatedConst::Symbolic(name) => name,
                        };
                        syntax.tables.type_references.replace_type_reference(
                            argument,
                            TypeReferenceNode::Named(Identifier::generated(name)),
                        );
                    } else {
                        normalize_template_type_reference(
                            syntax,
                            argument,
                            const_values,
                            symbolic_parameters,
                            warnings,
                        )?;
                    }
                }
                Ok(())
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let placeholder = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    symbolic_parameters,
                    None,
                    warnings,
                )?;
                let name = match placeholder {
                    EvaluatedConst::Concrete(value) => value.to_string(),
                    EvaluatedConst::Symbolic(name) => name,
                };
                syntax.tables.type_references.replace_type_reference(
                    type_reference,
                    TypeReferenceNode::Named(Identifier::generated(name)),
                );
                Ok(())
            }
            TypeReferenceNode::StaticSelection(_) => Err(
                "a scoped type selection must resolve before scalar template normalization"
                    .to_owned(),
            ),
            TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named(_)
            | TypeReferenceNode::Unit => Ok(()),
        }
    }

    /*
        Declared type inputs for closed data specialization: fields, constants and
        concrete callable signatures/bodies, including bodyless requirements.
        Revisit newly synthesized declarations on each completion round.
    */
    pub(in crate::preparation::generic_data) fn collect_selected_type_reference_positions(
        syntax: &SyntaxTrees,
        declarations: &[syntax_trees::item::ItemHandle],
    ) -> Vec<TypeReferenceHandle> {
        collect_owned_type_reference_positions(
            syntax,
            declarations,
            true,
            false,
            TypePositionTraversal::Identity,
        )
    }

    /*
        A substituted declaration adds inline storage, not demands to instantiate
        every referent it describes. The synthesis owner adds declaration-obligation
        demands independently; identity traversal alone does not discharge them.
    */
    pub(in crate::preparation::generic_data) fn collect_stored_type_reference_positions(
        syntax: &SyntaxTrees,
        declarations: &[syntax_trees::item::ItemHandle],
    ) -> Vec<TypeReferenceHandle> {
        collect_owned_type_reference_positions(
            syntax,
            declarations,
            true,
            false,
            TypePositionTraversal::Storage,
        )
    }

    pub(in crate::preparation::generic_data) fn collect_data_type_reference_positions(
        syntax: &SyntaxTrees,
        public_only: bool,
    ) -> Vec<TypeReferenceHandle> {
        collect_owned_type_reference_positions(
            syntax,
            syntax.root_item_handles(),
            false,
            public_only,
            TypePositionTraversal::Identity,
        )
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TypePositionTraversal {
        Identity,
        Storage,
    }

    pub(in crate::preparation) fn collect_type_positions(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        positions: &mut Vec<TypeReferenceHandle>,
        include_domain_arguments: bool,
    ) {
        visit_type_positions(
            syntax,
            type_reference,
            positions,
            include_domain_arguments,
            TypePositionTraversal::Identity,
        );
    }

    fn visit_type_positions(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        positions: &mut Vec<TypeReferenceHandle>,
        include_domain_arguments: bool,
        traversal: TypePositionTraversal,
    ) {
        positions.push(type_reference);
        macro_rules! visit_child {
            ($child:expr $(,)?) => {
                visit_type_positions(
                    syntax,
                    $child,
                    positions,
                    include_domain_arguments,
                    traversal,
                )
            };
        }
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Callable { signature } => {
                if traversal == TypePositionTraversal::Storage {
                    return;
                }
                let inputs = crate::preparation::callable_signatures::SignatureInputs::of(
                    syntax,
                    syntax.items.state_signature(*signature),
                );
                for &(_, reference) in &inputs.types {
                    visit_child!(reference);
                }
                for &(_, expression) in &inputs.expressions {
                    let mut expressions = HashSet::new();
                    collect_expression_handles(syntax, expression, &mut expressions);
                    for expression in expressions {
                        visit_expression_type_inputs(syntax, expression, &mut |reference| {
                            visit_child!(reference)
                        });
                    }
                }
            }
            TypeReferenceNode::Reference { referee, .. } => {
                if traversal == TypePositionTraversal::Identity {
                    visit_child!(*referee);
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                visit_child!(*base_type);
                if !include_domain_arguments || traversal == TypePositionTraversal::Storage {
                    return;
                }
                for constraint in syntax.type_references.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        for argument in
                            syntax_type_references(syntax).type_reference_handles(domain.arguments)
                        {
                            visit_child!(*argument);
                        }
                    }
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                visit_child!(*element_type);
                if let syntax_trees::types::FixedArrayLength::Expression(value) = length {
                    visit_child!(*value);
                }
            }
            TypeReferenceNode::Slice { element_type } => visit_child!(*element_type),
            TypeReferenceNode::Generic { arguments, .. }
            | TypeReferenceNode::DynamicTrait {
                trait_arguments: arguments,
                ..
            } => {
                for argument in syntax_type_references(syntax).type_reference_handles(*arguments) {
                    visit_child!(*argument);
                }
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let mut expressions = HashSet::new();
                super::super::uses::collect_expression_handles(
                    syntax,
                    *expression,
                    &mut expressions,
                );
                for expression in expressions {
                    visit_expression_type_inputs(syntax, expression, &mut |reference| {
                        visit_child!(reference);
                    });
                }
            }
            TypeReferenceNode::StaticSelection(selection) => {
                super::super::uses::visit_static_argument_type_inputs(
                    syntax,
                    std::slice::from_ref(selection),
                    &mut |reference| {
                        visit_child!(reference);
                    },
                );
            }
            TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
        }
    }

    fn collect_owned_type_reference_positions(
        syntax: &SyntaxTrees,
        declarations: &[syntax_trees::item::ItemHandle],
        include_callables: bool,
        public_only: bool,
        traversal: TypePositionTraversal,
    ) -> Vec<TypeReferenceHandle> {
        let mut positions: Vec<TypeReferenceHandle> = Vec::new();
        for &declaration in declarations {
            let item = syntax.root_item(declaration);
            match item {
                Item::Data(definition)
                    if definition.type_parameters.is_empty()
                        && (!public_only || definition.is_public) =>
                {
                    for member in syntax.tables.items.data_members(definition.members) {
                        match member {
                            DataMember::Field(field) => visit_type_positions(
                                syntax,
                                field.type_reference,
                                &mut positions,
                                true,
                                traversal,
                            ),
                            DataMember::Variant(variant) => {
                                for field in
                                    syntax.tables.items.data_payload_fields(variant.payload)
                                {
                                    visit_type_positions(
                                        syntax,
                                        field.type_reference,
                                        &mut positions,
                                        true,
                                        traversal,
                                    );
                                }
                            }
                            DataMember::Retired(_) => {}
                        }
                    }
                }
                Item::Const(definition) if !public_only || definition.is_public => {
                    visit_type_positions(
                        syntax,
                        definition.type_reference,
                        &mut positions,
                        true,
                        traversal,
                    )
                }
                _ => {}
            }
        }
        if !include_callables {
            return positions;
        }
        visit_callable_type_inputs(syntax, declarations, |reference, _| {
            visit_type_positions(syntax, reference, &mut positions, false, traversal);
        });
        positions
    }

    /*
        Collect the same declared callable inputs used by data specialization,
        retaining which inputs form a public signature for constant evaluation.
    */
    pub(in crate::preparation::generic_data) fn collect_callable_type_reference_positions(
        syntax: &SyntaxTrees,
    ) -> (Vec<TypeReferenceHandle>, Vec<TypeReferenceHandle>) {
        let mut positions = Vec::new();
        let mut public_positions = Vec::new();
        visit_callable_type_inputs(
            syntax,
            syntax.root_item_handles(),
            |reference, is_public_signature| {
                let start = positions.len();
                collect_type_positions(syntax, reference, &mut positions, true);
                if is_public_signature {
                    public_positions.extend_from_slice(&positions[start..]);
                }
            },
        );
        (positions, public_positions)
    }

    /*
        Every concrete callable publishes its declared type inputs here. Constant
        evaluation and closed data specialization consume this same list, including
        bodyless requirements. Open declarations remain templates in both routes.
    */
    fn visit_callable_type_inputs(
        syntax: &SyntaxTrees,
        declarations: &[syntax_trees::item::ItemHandle],
        mut visit: impl FnMut(TypeReferenceHandle, bool),
    ) {
        for machine in concrete_machine_declarations(syntax, declarations) {
            for conformance in syntax.items.satisfies_clauses(machine.satisfies) {
                for &argument in
                    syntax_type_references(syntax).type_reference_handles(conformance.arguments)
                {
                    visit(argument, machine.is_public);
                }
            }
            for (ordinal, state_handle) in syntax_items(syntax)
                .state_handles(machine.states)
                .iter()
                .enumerate()
            {
                let state = syntax.items.state(*state_handle);
                let is_public_signature = ordinal == 0 && machine.is_public;
                visit(state.return_type, is_public_signature);
                for parameter in syntax.items.state_parameters(state.parameters) {
                    visit(
                        syntax.items.state_parameter(*parameter).type_reference,
                        is_public_signature,
                    );
                }
                for statement in syntax.items.statements(state.statements) {
                    for reference in super::super::uses::statement_type_inputs(syntax, *statement) {
                        visit(reference, false);
                    }
                }
            }
        }
        for &declaration in declarations {
            let Item::Trait(definition) = syntax.root_item(declaration) else {
                continue;
            };
            if !definition.type_parameters.is_empty() {
                continue;
            }
            for handle in syntax.items.state_signatures(definition.machines) {
                let signature = syntax.items.state_signature(*handle);
                if !signature.type_parameters.is_empty() {
                    continue;
                }
                visit(signature.return_type, definition.is_public);
                for parameter in syntax.items.state_parameters(signature.parameters) {
                    visit(
                        syntax.items.state_parameter(*parameter).type_reference,
                        definition.is_public,
                    );
                }
            }
        }
    }
}
mod values {
    /*
        Structural const values under exact declaration-site carrier selection.

        The header resolver selects each nominal type and constructor in its own
        authored source, including nested field carriers. Compare those declarations
        before erasing names into the existing canonical atom. Field order and leaf
        landing still determine the encoded value; final resolution independently
        checks the receiving parameter's nominal carrier.

        Structural encoding recursively lands anonymous scalar leaves at the declared
        component type. A suffixed leaf has already landed: erasing that carrier or
        arithmetic domain would equate a differently typed value with this component,
        even when its numeric payload fits. Check the landing before encoding it.
        Array eligibility, element rosters and encoded carrier labels close geometry
        through one source-selected integer extent. Generic bindings retain priority;
        a completed named constant is read from its declaration, never a root-name map.
        Ordinary declaration materialization does not request const-index eligibility.
        Its recursively selected data and active-case predicates still discharge over
        the completed value before encoding. Index requests share that membership
        proof while retaining their stronger structural/canonical carrier judgment.
    */
    use super::super::{
        CanonicalConstValue, ConstDefinition, DataDefinition, DataMember, ExpressionHandle,
        ExpressionNode, FixedArrayLength, HandleSpan, HashSet, Identifier, Item, SyntaxTrees,
        TypeReferenceHandle, TypeReferenceNode,
    };
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use syntax_trees::identifier::JoinedSyntaxPath;

    use super::facts::prove_const_data_predicates;
    use crate::preparation::declaration_selection::{
        ConstantSelection, GenericApplicationSubstitution, resolved_generic_argument,
        substituted_generic_arguments,
    };
    use crate::preparation::generic_data::ClosedArgumentIdentity;
    use crate::preparation::generic_data::ConstFactValue;
    use crate::preparation::generic_data::arguments::{
        closed_argument_identity, closed_parameter_argument_identity,
    };
    use crate::preparation::generic_data::integer_literal_value;
    use crate::preparation::generic_data::qualified_const_name;
    use crate::preparation::generic_data::type_reference_slug;
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use numerics::literals::LandedIntegerType;
    use symbols::SymbolKind;
    use syntax_trees::item::TypeParameterKind;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(in crate::preparation::generic_data) enum CanonicalConstNode {
        Integer {
            type_name: String,
            value: i128,
        },
        Boolean(bool),
        Float {
            format: numerics::literals::FloatFormat,
            bits: u64,
        },
        Array {
            type_name: String,
            values: Vec<CanonicalConstNode>,
        },
        Record {
            type_name: String,
            fields: Vec<(String, CanonicalConstNode)>,
        },
        Variant {
            type_name: String,
            case_name: String,
            fields: Vec<(String, CanonicalConstNode)>,
        },
    }

    impl CanonicalConstNode {
        pub(in crate::preparation::generic_data) fn encoding(&self) -> String {
            match self {
                Self::Integer { type_name, value } => {
                    framed("integer", [type_name.clone(), value.to_string()])
                }
                Self::Boolean(value) => framed("boolean", [if *value { "true" } else { "false" }]),
                Self::Float { format, bits } => match format {
                    numerics::literals::FloatFormat::F32 => format!("float:f32:{bits:08x}"),
                    numerics::literals::FloatFormat::F64 => format!("float:f64:{bits:016x}"),
                },
                Self::Array { type_name, values } => framed(
                    "array",
                    std::iter::once(type_name.as_str().to_owned())
                        .chain(values.iter().map(Self::encoding)),
                ),
                Self::Record { type_name, fields } => framed(
                    "record",
                    std::iter::once(type_name.clone()).chain(
                        fields
                            .iter()
                            .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                    ),
                ),
                Self::Variant {
                    type_name,
                    case_name,
                    fields,
                } => framed(
                    "variant",
                    [type_name.clone(), case_name.clone()].into_iter().chain(
                        fields
                            .iter()
                            .flat_map(|(name, value)| [name.clone(), value.encoding()]),
                    ),
                ),
            }
        }

        pub(in crate::preparation::generic_data) fn display(&self) -> String {
            match self {
                Self::Integer { value, .. } => value.to_string(),
                Self::Boolean(value) => value.to_string(),
                Self::Float { format, bits } => match format {
                    numerics::literals::FloatFormat::F32 => {
                        format!("{:?}f32", f32::from_bits(*bits as u32))
                    }
                    numerics::literals::FloatFormat::F64 => {
                        format!("{:?}f64", f64::from_bits(*bits))
                    }
                },
                Self::Array { values, .. } => format!(
                    "[{}]",
                    values
                        .iter()
                        .map(Self::display)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Self::Record { type_name, fields } => format!(
                    "{type_name} {{ {} }}",
                    fields
                        .iter()
                        .map(|(name, value)| format!("{name}: {}", value.display()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Self::Variant {
                    type_name,
                    case_name,
                    fields,
                } if fields.is_empty() => format!("{type_name}::{case_name}"),
                Self::Variant {
                    type_name,
                    case_name,
                    fields,
                } => format!(
                    "{type_name}::{case_name} {{ {} }}",
                    fields
                        .iter()
                        .map(|(name, value)| format!("{name}: {}", value.display()))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        }
    }

    pub(in crate::preparation::generic_data) fn framed(
        tag: &str,
        pieces: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> String {
        let mut encoded = tag.to_owned();
        for piece in pieces {
            let piece = piece.as_ref();
            encoded.push_str(&piece.len().to_string());
            encoded.push(':');
            encoded.push_str(piece);
        }
        encoded
    }

    pub(in crate::preparation::generic_data) fn canonicalize_const_definition(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        parameter_type: TypeReferenceHandle,
    ) -> Result<CanonicalConstValue, String> {
        canonicalize_selected_const_definition(syntax, definition, parameter_type, None)
    }

    pub(in crate::preparation) fn canonicalize_selected_const_definition(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        parameter_type: TypeReferenceHandle,
        selection: Option<&ConstantSelection>,
    ) -> Result<CanonicalConstValue, String> {
        canonicalize_selected_const_definition_in_application(
            syntax,
            definition,
            parameter_type,
            selection,
            &GenericApplicationSubstitution::new(),
        )
    }

    pub(in crate::preparation::generic_data) fn canonicalize_selected_const_definition_in_application(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        parameter_type: TypeReferenceHandle,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<CanonicalConstValue, String> {
        let declared = syntax_type_identity(syntax, definition.type_reference)?;
        let required = substituted_type_label(syntax, parameter_type, substitution, selection)?;
        if !same_carrier_in_application(
            syntax,
            definition.type_reference,
            parameter_type,
            selection,
            substitution,
        )? {
            return Err(format!(
                "const `{}` declares type `{declared}`, but the parameter requires `{required}`",
                qualified_const_name(syntax, definition)
            ));
        }
        canonicalize_scoped_index_expression(
            syntax,
            definition.type_reference,
            &GenericApplicationSubstitution::new(),
            parameter_type,
            substitution,
            definition.value,
            selection,
        )
    }

    pub(in crate::preparation::generic_data) fn canonicalize_selected_index_expression(
        syntax: &SyntaxTrees,
        value_type: TypeReferenceHandle,
        parameter_type: TypeReferenceHandle,
        expression: ExpressionHandle,
        selection: Option<&ConstantSelection>,
    ) -> Result<CanonicalConstValue, String> {
        canonicalize_scoped_index_expression(
            syntax,
            value_type,
            &GenericApplicationSubstitution::new(),
            parameter_type,
            &GenericApplicationSubstitution::new(),
            expression,
            selection,
        )
    }

    pub(in crate::preparation::generic_data) fn canonicalize_selected_index_expression_in_application(
        syntax: &SyntaxTrees,
        parameter_type: TypeReferenceHandle,
        expression: ExpressionHandle,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<CanonicalConstValue, String> {
        canonicalize_scoped_index_expression(
            syntax,
            parameter_type,
            substitution,
            parameter_type,
            substitution,
            expression,
            selection,
        )
    }

    fn canonicalize_scoped_index_expression(
        syntax: &SyntaxTrees,
        value_type: TypeReferenceHandle,
        value_scope: &GenericApplicationSubstitution,
        parameter_type: TypeReferenceHandle,
        parameter_scope: &GenericApplicationSubstitution,
        expression: ExpressionHandle,
        selection: Option<&ConstantSelection>,
    ) -> Result<CanonicalConstValue, String> {
        validate_selected_const_index_type(
            syntax,
            parameter_type,
            &mut Vec::new(),
            selection,
            parameter_scope,
        )?;
        let node =
            canonicalize_const_expression(syntax, value_type, expression, selection, value_scope)?;
        canonical_const_result_in_application(
            syntax,
            parameter_type,
            node,
            selection,
            parameter_scope,
        )
    }

    pub(in crate::preparation::generic_data) fn syntax_type_identity(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
    ) -> Result<String, String> {
        Ok(
            match syntax.tables.type_references.type_reference(type_reference) {
                TypeReferenceNode::Named(name) => name.as_str(syntax).to_owned(),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } => format!(
                    "[{}; {length}]",
                    syntax_type_identity(syntax, *element_type)?
                ),
                TypeReferenceNode::Constrained { base_type, .. } => {
                    syntax_type_identity(syntax, *base_type)?
                }
                TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                    ..
                } => {
                    let argument_handles =
                        syntax_type_references(syntax).type_reference_handles(*arguments);
                    if argument_handles.is_empty() {
                        base_name.as_str(syntax).to_owned()
                    } else {
                        let mut parts = Vec::with_capacity(argument_handles.len());
                        for argument in argument_handles {
                            parts.push(syntax_type_identity(syntax, *argument)?);
                        }
                        format!("{}<{}>", base_name.as_str(syntax), parts.join(", "))
                    }
                }
                TypeReferenceNode::Unit => "()".to_owned(),
                TypeReferenceNode::ConstExpression(expression) => {
                    match syntax.expressions.expression(*expression) {
                        syntax_trees::expression::ExpressionNode::Integer(literal) => {
                            super::integer_literal_value(literal)
                                .map(|value| value.to_string())
                                .ok_or_else(|| {
                                    "integer literal exceeds const envelope".to_owned()
                                })?
                        }
                        syntax_trees::expression::ExpressionNode::Name(path) => {
                            let [member] = syntax.expressions.identifier_path_members(*path) else {
                                return Err("unsupported path in const expression".to_owned());
                            };
                            member.as_str(syntax).to_owned()
                        }
                        _ => {
                            return Err(
                            "structured const parameter types must be a canonical scalar, fixed array, or declared data value"
                                .to_owned(),
                        );
                        }
                    }
                }
                _ => {
                    return Err(
                "structured const parameter types must be a canonical scalar, fixed array, or declared data value"
                    .to_owned(),
            );
                }
            },
        )
    }

    /*
        Encoded labels are consistency claims. Nominal equality is established from
        the selected declarations before values lose their authored spelling; the
        receiving generic slot independently rejoins that declared carrier later.
    */
    pub(super) fn selected_data<'syntax>(
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
        selection: Option<&ConstantSelection>,
    ) -> Result<&'syntax DataDefinition, String> {
        if let Some(selection) = selection {
            return selection.data(syntax, name);
        }
        let mut definitions = syntax.root_items().filter_map(|item| match item {
            Item::Data(definition) if definition.name.as_str(syntax) == name.as_str(syntax) => {
                Some(definition)
            }
            _ => None,
        });
        let definition = definitions.next().ok_or_else(|| {
            format!(
                "`{name}` is not a declared canonical data type",
                name = name.as_str(syntax)
            )
        })?;
        if definitions.next().is_some() {
            return Err(format!(
                "`{name}` has ambiguous nominal declarations",
                name = name.as_str(syntax)
            ));
        }
        Ok(definition)
    }

    /*
        Select a synthesized closed instance by its exact display name. Synthesized
        declarations are pushed after the header symbol pass, so name law lookup
        cannot reach them; their `generic_instance` evidence is what distinguishes
        one from an authored declaration that happens to share the spelling.
    */
    fn selected_synthesized_instance<'syntax>(
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
    ) -> Result<&'syntax DataDefinition, String> {
        let mut candidates = syntax.root_items().filter_map(|item| match item {
            Item::Data(definition)
                if definition.generic_instance.is_some()
                    && definition.name.as_str(syntax) == name.as_str(syntax) =>
            {
                Some(definition)
            }
            _ => None,
        });
        let instance = candidates.next().ok_or_else(|| {
            format!(
                "`{name}` does not select one declared canonical data type",
                name = name.as_str(syntax)
            )
        })?;
        if candidates.next().is_some() {
            return Err(format!(
                "`{name}` has ambiguous nominal declarations",
                name = name.as_str(syntax)
            ));
        }
        Ok(instance)
    }

    fn selected_constructor<'syntax>(
        syntax: &'syntax SyntaxTrees,
        name: &Identifier,
        selection: Option<&ConstantSelection>,
    ) -> Result<(&'syntax DataDefinition, Option<Identifier>), String> {
        if let Some(selection) = selection {
            return selection.constructor(syntax, name);
        }
        let record = selected_data(syntax, name, None).ok();
        let case = name.as_str(syntax).rsplit_once("::").and_then(|(owner, case)| {
        let owner = Identifier::new(owner, name.source_span());
        selected_data(syntax, &owner, None)
            .ok()
            .filter(|definition| {
                syntax.items.data_members(definition.members).iter().any(|member| {
                    matches!(member, DataMember::Variant(variant) if variant.name.as_str(syntax) == case)
                })
            })
            .map(|definition| (definition, Identifier::new(case, name.source_span())))
    });
        match (record, case) {
            (Some(definition), None) => Ok((definition, None)),
            (None, Some((definition, case))) => Ok((definition, Some(case))),
            _ => Err(format!(
                "`{name}` does not select one declared constructor carrier",
                name = name.as_str(syntax)
            )),
        }
    }

    /*
        The shared canonical-leaf packaging: re-label the selected type, gate a
        `Rat` carrier through its canonical-form check, and wrap the node.
    */
    fn canonical_const_result(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        node: CanonicalConstNode,
        selection: Option<&ConstantSelection>,
    ) -> Result<CanonicalConstValue, String> {
        canonical_const_result_in_application(
            syntax,
            type_reference,
            node,
            selection,
            &GenericApplicationSubstitution::new(),
        )
    }

    fn canonical_const_result_in_application(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        node: CanonicalConstNode,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<CanonicalConstValue, String> {
        let required = substituted_type_label(syntax, type_reference, substitution, selection)?;
        if required == "Rat" {
            validate_canonical_rat(&node)?;
        }
        Ok(CanonicalConstValue::new(
            required,
            node.encoding(),
            node.display(),
        ))
    }

    fn selected_type_label(
        syntax: &SyntaxTrees,
        reference: TypeReferenceHandle,
        selection: Option<&ConstantSelection>,
    ) -> Result<String, String> {
        match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Named(name)
                if !crate::constant::is_builtin_scalar_carrier(name.as_str(syntax)) =>
            {
                let origin = syntax.type_references.generic_application_origin(reference);
                if origin.is_valid() && origin != reference {
                    return selected_type_label(syntax, origin, selection);
                }
                Ok(selected_data(syntax, name, selection)?
                    .name
                    .as_str(syntax)
                    .to_owned())
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let argument_handles = syntax.type_references.type_reference_handles(*arguments);
                if argument_handles.is_empty() {
                    return Ok(selected_synthesized_instance(syntax, base_name)?
                        .name
                        .as_str(syntax)
                        .to_owned());
                }
                let definition = selected_data(syntax, base_name, selection)?;
                generic_application_label(syntax, definition, argument_handles, selection)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => Ok(format!(
                "[{}; {length}]",
                selected_type_label(syntax, *element_type, selection)?
            )),
            TypeReferenceNode::Constrained { base_type, .. } => {
                selected_type_label(syntax, *base_type, selection)
            }
            _ => syntax_type_identity(syntax, reference),
        }
    }

    /*
        The canonical carrier label of one closed generic application: the exact
        name the synthesized instance owns (`synthetic_name`), matching the
        downstream `definition.name` label contract. When the instance already
        exists its declaration name is authoritative — the deduplicated `__n`
        spellings can only be reproduced that way. Before synthesis completes, the
        same leaf-plus-slugs construction predicts it.
    */
    fn generic_application_label(
        syntax: &SyntaxTrees,
        definition: &DataDefinition,
        argument_handles: &[TypeReferenceHandle],
        selection: Option<&ConstantSelection>,
    ) -> Result<String, String> {
        if let Some(instance) =
            synthesized_instance(syntax, selection, definition, argument_handles)
        {
            return Ok(instance.name.as_str(syntax).to_owned());
        }
        let parameters = syntax.items.type_parameters(definition.type_parameters);
        let argument_names =
            closed_application_argument_labels(syntax, parameters, argument_handles, selection)?;
        Ok(format!(
            "{}<{}>",
            definition.name.as_str(syntax),
            argument_names.join(", ")
        ))
    }

    /*
        Render only an independently closed tuple under its selected receiving
        telescope. Code and data select different namespaces; labels are metadata,
        while the exact declaration and recursive argument keys above own identity.
    */
    fn closed_application_argument_labels(
        syntax: &SyntaxTrees,
        parameters: &[syntax_trees::item::TypeParameter],
        arguments: &[TypeReferenceHandle],
        selection: Option<&ConstantSelection>,
    ) -> Result<Vec<String>, String> {
        if parameters.len() != arguments.len() {
            return Err("generic const carrier has an incomplete argument tuple".to_owned());
        }
        parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                closed_parameter_argument_identity(syntax, selection, *argument, &parameter.kind)
                    .ok_or("generic const carrier is not a closed monomorphizable application")?;
                if matches!(parameter.kind, TypeParameterKind::Implementation) {
                    return selected_implementation_label(syntax, *argument, selection);
                }
                if let Some(slug) = type_reference_slug(syntax, *argument, selection) {
                    return Ok(slug);
                }
                let label = selected_type_label(syntax, *argument, selection)?;
                qualify_application_label(syntax, *argument, label, SymbolKind::Data, selection)
            })
            .collect()
    }

    fn selected_implementation_label(
        syntax: &SyntaxTrees,
        reference: TypeReferenceHandle,
        selection: Option<&ConstantSelection>,
    ) -> Result<String, String> {
        let origin = syntax.type_references.generic_application_origin(reference);
        let reference = if origin.is_valid() { origin } else { reference };
        let (name, arguments) = match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Named(name) => (name, &[][..]),
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => (
                base_name,
                syntax.type_references.type_reference_handles(*arguments),
            ),
            _ => return Err("closed implementation argument lost its application".to_owned()),
        };
        let selection =
            selection.ok_or("closed implementation argument has no declaration selection")?;
        let declaration = selection.implementation_item(syntax, name)?;
        let Item::Conformance(implementation) = syntax.root_item(declaration) else {
            return Err("closed implementation argument lost its declaration".to_owned());
        };
        let alias = implementation
            .alias
            .as_ref()
            .ok_or("closed implementation declaration has no name")?;
        let parameters = syntax.items.type_parameters(implementation.type_parameters);
        let arguments =
            closed_application_argument_labels(syntax, parameters, arguments, Some(selection))?;
        let label = if arguments.is_empty() {
            alias.as_str(syntax).to_owned()
        } else {
            format!("{}<{}>", alias.as_str(syntax), arguments.join(", "))
        };
        qualify_application_label(
            syntax,
            reference,
            label,
            SymbolKind::Conformance,
            Some(selection),
        )
    }

    fn qualify_application_label(
        syntax: &SyntaxTrees,
        reference: TypeReferenceHandle,
        label: String,
        kind: SymbolKind,
        selection: Option<&ConstantSelection>,
    ) -> Result<String, String> {
        let name = match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Named(name)
            | TypeReferenceNode::Generic {
                base_name: name, ..
            } => name,
            _ => return Ok(label),
        };
        if let Some(selection) = selection
            && let Some(path) = selection.declaration_lookup_path(syntax, name, kind)
            && let Some((prefix, _)) = path.rsplit_once("::")
        {
            Ok(format!("{prefix}::{label}"))
        } else {
            Ok(label)
        }
    }

    /*
        The already-synthesized instance for one selected template and closed
        argument tuple, found by the same `Instance` identity deduplication uses —
        never by rendered-name equality.
    */
    fn synthesized_instance<'syntax>(
        syntax: &'syntax SyntaxTrees,
        selection: Option<&ConstantSelection>,
        template: &DataDefinition,
        argument_handles: &[TypeReferenceHandle],
    ) -> Option<&'syntax DataDefinition> {
        let parameters = syntax.items.type_parameters(template.type_parameters);
        if parameters.len() != argument_handles.len() {
            return None;
        }
        let identity = parameters
            .iter()
            .zip(argument_handles)
            .map(|(parameter, argument)| {
                closed_parameter_argument_identity(syntax, selection, *argument, &parameter.kind)
            })
            .collect::<Option<Vec<_>>>()?;
        let template_handle = syntax.root_item_handles().iter().copied().find(|handle| {
        matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, template))
    })?;
        syntax.root_items().find_map(|item| match item {
            Item::Data(data) => {
                let origin = data.generic_instance?;
                match closed_argument_identity(syntax, selection, origin, false) {
                    Some(ClosedArgumentIdentity::Instance(declaration, identities))
                        if declaration == template_handle && identities == identity =>
                    {
                        Some(data)
                    }
                    _ => None,
                }
            }
            _ => None,
        })
    }

    /*
        Receiving formals substitute once. Inserted caller types are then compared
        outside this telescope; original array children keep the receiving scope.
    */
    fn same_carrier_in_application(
        syntax: &SyntaxTrees,
        declared: TypeReferenceHandle,
        required: TypeReferenceHandle,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<bool, String> {
        if let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(required)
            && let Some(actual) = substitution.get(name.as_str(syntax))
        {
            return same_carrier(syntax, declared, *actual, selection);
        }
        match (
            syntax.type_references.type_reference(declared),
            syntax.type_references.type_reference(required),
        ) {
            (
                TypeReferenceNode::FixedArray {
                    element_type: actual,
                    length: actual_length,
                },
                TypeReferenceNode::FixedArray {
                    element_type: demanded,
                    length: demanded_length,
                },
            ) => Ok(closed_fixed_array_length(
                syntax,
                actual_length,
                &GenericApplicationSubstitution::new(),
                selection,
            )? == closed_fixed_array_length(
                syntax,
                demanded_length,
                substitution,
                selection,
            )? && same_carrier_in_application(
                syntax,
                *actual,
                *demanded,
                selection,
                substitution,
            )?),
            (TypeReferenceNode::Constrained { base_type, .. }, _) => {
                same_carrier_in_application(syntax, *base_type, required, selection, substitution)
            }
            (_, TypeReferenceNode::Constrained { base_type, .. }) => {
                same_carrier_in_application(syntax, declared, *base_type, selection, substitution)
            }
            _ => same_carrier(syntax, declared, required, selection),
        }
    }

    fn same_carrier(
        syntax: &SyntaxTrees,
        declared: TypeReferenceHandle,
        required: TypeReferenceHandle,
        selection: Option<&ConstantSelection>,
    ) -> Result<bool, String> {
        if declared == required {
            return Ok(true);
        }
        if syntax_type_references(syntax)
            .generic_application_origin(declared)
            .is_valid()
            || syntax_type_references(syntax)
                .generic_application_origin(required)
                .is_valid()
        {
            return Ok(
                closed_argument_identity(syntax, selection, declared, false).is_some()
                    && closed_argument_identity(syntax, selection, declared, false)
                        == closed_argument_identity(syntax, selection, required, false),
            );
        }
        match (
            syntax.type_references.type_reference(declared),
            syntax.type_references.type_reference(required),
        ) {
            (TypeReferenceNode::Named(left), TypeReferenceNode::Named(right)) => {
                let primitive = |name: &str| crate::constant::is_builtin_scalar_carrier(name);
                if primitive(left.as_str(syntax)) || primitive(right.as_str(syntax)) {
                    return Ok(left.as_str(syntax) == right.as_str(syntax));
                }
                Ok(std::ptr::eq(
                    selected_data(syntax, left, selection)?,
                    selected_data(syntax, right, selection)?,
                ))
            }
            (
                TypeReferenceNode::FixedArray {
                    element_type: left,
                    length: left_length,
                },
                TypeReferenceNode::FixedArray {
                    element_type: right,
                    length: right_length,
                },
            ) => Ok(left_length.same_value(right_length, syntax)
                && same_carrier(syntax, *left, *right, selection)?),
            (TypeReferenceNode::Constrained { base_type, .. }, _) => {
                same_carrier(syntax, *base_type, required, selection)
            }
            (_, TypeReferenceNode::Constrained { base_type, .. }) => {
                same_carrier(syntax, declared, *base_type, selection)
            }
            (TypeReferenceNode::Generic { .. }, _) | (_, TypeReferenceNode::Generic { .. }) => {
                match (
                    closed_argument_identity(syntax, selection, declared, false),
                    closed_argument_identity(syntax, selection, required, false),
                ) {
                    (Some(declared_identity), Some(required_identity)) => {
                        Ok(declared_identity == required_identity)
                    }
                    _ => Ok(false),
                }
            }
            (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => Ok(true),
            _ => Ok(false),
        }
    }

    pub(in crate::preparation::generic_data) fn validate_const_index_type(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        _visiting: &mut HashSet<String>,
    ) -> Result<(), String> {
        validate_selected_const_index_type(
            syntax,
            type_reference,
            &mut Vec::new(),
            None,
            &GenericApplicationSubstitution::new(),
        )
    }

    /*
        Payload encoding and destination establishment are different judgments.
        Only the compile-time owner may keep a pending payload private or consume
        an exact checked destination receipt. Neither encoding mode proves a fact.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(in crate::preparation::generic_data) enum ConstPredicateAdmission {
        ProveSyntax,
        PendingDestination,
        CheckedDestination,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ConstValuePurpose {
        Declaration,
        Index,
    }

    /*
        Representation choice remains on the declaration and its canonical carrier
        label. Only value eligibility and payload replay inspect semantic data. The
        selected, sealed Core declaration supplies this relationship; user data,
        including same-spelled opaque data, retains ordinary nominal checking.
    */
    fn representation_semantic_type(
        syntax: &SyntaxTrees,
        reference: TypeReferenceHandle,
        selection: Option<&ConstantSelection>,
    ) -> Result<Option<TypeReferenceHandle>, String> {
        let TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } = syntax.type_references.type_reference(reference)
        else {
            return Ok(None);
        };
        if arguments.is_empty() {
            return Ok(None);
        }
        let Some(selection) = selection else {
            return Ok(None);
        };
        let definition = selection.data(syntax, base_name)?;
        if !selection.is_core_layout_storage(syntax, definition) {
            return Ok(None);
        }
        let [semantic_type, _policy] = syntax.type_references.type_reference_handles(*arguments)
        else {
            return Err(
                "Core representation requires its semantic type and explicit policy".to_owned(),
            );
        };
        if !lifetime_arguments.is_empty() {
            return Err("Core representation has no lifetime parameters".to_owned());
        }
        Ok(Some(*semantic_type))
    }

    fn validate_selected_const_index_type(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        visiting: &mut Vec<source::SourceSpan>,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<(), String> {
        validate_selected_const_type(
            syntax,
            type_reference,
            visiting,
            selection,
            substitution,
            ConstValuePurpose::Index,
        )
    }

    pub(in crate::preparation::generic_data) fn canonicalize_selected_declaration_value(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        selection: Option<&ConstantSelection>,
    ) -> Result<CanonicalConstValue, String> {
        if definition.pending_qualification.is_some() {
            return Err("constant destination qualification remains pending".to_owned());
        }
        canonicalize_selected_declaration_value_with_admission(
            syntax,
            definition,
            selection,
            ConstPredicateAdmission::ProveSyntax,
        )
    }

    pub(in crate::preparation::generic_data) fn canonicalize_selected_declaration_value_with_admission(
        syntax: &SyntaxTrees,
        definition: &ConstDefinition,
        selection: Option<&ConstantSelection>,
        admission: ConstPredicateAdmission,
    ) -> Result<CanonicalConstValue, String> {
        let substitution = GenericApplicationSubstitution::new();
        validate_selected_const_type(
            syntax,
            definition.type_reference,
            &mut Vec::new(),
            selection,
            &substitution,
            ConstValuePurpose::Declaration,
        )?;
        let node = canonicalize_const_expression_with_admission(
            syntax,
            definition.type_reference,
            definition.value,
            selection,
            &substitution,
            admission,
        )?;
        canonical_const_result(syntax, definition.type_reference, node, selection)
    }

    /*
        Close structural array geometry through the same selected declaration used
        for its value. A spelling is not an extent: source custody selects the owner,
        canonical scalar validation establishes its carrier, and only a completed
        literal is observable here. Calls and unresolved values remain with the
        evaluation scheduler; reading them here would invent dependency readiness.
    */
    pub(crate) fn closed_fixed_array_length(
        syntax: &SyntaxTrees,
        length: &FixedArrayLength,
        substitution: &GenericApplicationSubstitution,
        selection: Option<&ConstantSelection>,
    ) -> Result<usize, String> {
        match length {
            FixedArrayLength::Literal(length) => Ok(*length),
            FixedArrayLength::ConstParameter(name) => {
                if let Some(argument) = substitution.get(name.as_str(syntax)) {
                    return syntax_type_references(syntax)
                        .type_reference(*argument)
                        .as_named()
                        .ok_or_else(|| {
                            "array extent binding is not a completed integer literal".to_owned()
                        })?
                        .as_str(syntax)
                        .parse::<usize>()
                        .map_err(|_| {
                            "array extent binding is not a nonnegative host-sized integer"
                                .to_owned()
                        });
                }
                let selection = selection.ok_or_else(|| {
                    "array extent requires an exact selected constant declaration".to_owned()
                })?;
                let definition = selection
                    .select(syntax, name)
                    .map_err(|diagnostic| diagnostic.message.to_string())?
                    .ok_or_else(|| {
                        "array extent requires an exact selected constant declaration".to_owned()
                    })?;
                let mut carrier = definition.type_reference;
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    syntax.type_references.type_reference(carrier)
                {
                    carrier = *base_type;
                }
                if !matches!(syntax.type_references.type_reference(carrier), TypeReferenceNode::Named(name)
                if matches!(selection.builtin_type(syntax, name), Some(symbols::BuiltinTypeAtom::I8 | symbols::BuiltinTypeAtom::I16 | symbols::BuiltinTypeAtom::I32 | symbols::BuiltinTypeAtom::I64 | symbols::BuiltinTypeAtom::U8 | symbols::BuiltinTypeAtom::U16 | symbols::BuiltinTypeAtom::U32 | symbols::BuiltinTypeAtom::U64)))
                {
                    return Err("array extent constant must have an integer carrier".to_owned());
                }
                if !matches!(
                    syntax.expressions.expression(definition.value),
                    ExpressionNode::Integer(_)
                ) {
                    return Err(
                        "array extent constant is not a completed integer literal".to_owned()
                    );
                }
                let value = super::super::canonicalize_selected_declared_const_definition(
                    syntax,
                    &definition,
                    Some(selection),
                )?;
                let Some(DecodedCanonicalConstValue::Integer { value, .. }) =
                    value.decode_encoding()
                else {
                    return Err("array extent constant must have an integer carrier".to_owned());
                };
                usize::try_from(value).map_err(|_| {
                    "array extent constant is not a nonnegative host-sized integer".to_owned()
                })
            }
            FixedArrayLength::Expression(value) => syntax_type_references(syntax)
                .type_reference(*value)
                .as_named()
                .and_then(|value| value.as_str(syntax).parse::<usize>().ok())
                .ok_or_else(|| {
                    "array extent expression requires completed constant evaluation".to_owned()
                }),
            FixedArrayLength::ConstCall(_) => {
                Err("array extent call requires completed constant evaluation".to_owned())
            }
        }
    }

    fn validate_selected_const_type(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        visiting: &mut Vec<source::SourceSpan>,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
        purpose: ConstValuePurpose,
    ) -> Result<(), String> {
        if let Some(name) = syntax_type_references(syntax)
            .type_reference(type_reference)
            .as_named()
            && let Some(argument) = substitution.get(name.as_str(syntax))
        {
            return validate_selected_const_type(
                syntax,
                *argument,
                visiting,
                selection,
                &GenericApplicationSubstitution::new(),
                purpose,
            );
        }
        let origin = syntax_type_references(syntax).generic_application_origin(type_reference);
        if origin.is_valid() && origin != type_reference {
            return validate_selected_const_type(
                syntax,
                origin,
                visiting,
                selection,
                substitution,
                purpose,
            );
        }
        if purpose == ConstValuePurpose::Declaration
            && let Some(semantic_type) =
                representation_semantic_type(syntax, type_reference, selection)?
        {
            return validate_selected_const_type(
                syntax,
                semantic_type,
                visiting,
                selection,
                substitution,
                purpose,
            );
        }
        let no_bindings = GenericApplicationSubstitution::new();
        match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Named(name) => {
            if crate::constant::is_structural_scalar_carrier(name.as_str(syntax)) {
                return Ok(());
            }
            if purpose == ConstValuePurpose::Declaration
                && matches!(name.as_str(syntax), "f32" | "f64")
            {
                return Ok(());
            }
            if matches!(name.as_str(syntax), "f32" | "f64") {
                return Err(format!(
                    "`{name}` is not eligible as a const index: runtime floating/text identity is not canonical structural data"
                , name = name.as_str(syntax)));
            }
            let definition = selected_data(syntax, name, selection)?;
            let declaration = definition.name.source_span();
            if visiting.contains(&declaration) {
                return Ok(());
            }
            if !definition.type_parameters.is_empty() || !definition.lifetime_parameters.is_empty() {
                return Err("generic data const carriers require closed template normalization".to_owned());
            }
            visiting.push(declaration);
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!(
                    "boundary-opaque data `{name}` is not eligible as a const index"
                , name = name.as_str(syntax)));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` is not eligible as a structural const index until quotient-backed canonical representatives land"
                , name = name.as_str(syntax)));
            }
            for member in syntax.tables.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => validate_selected_const_type(syntax,
                        field.type_reference,
                        visiting,
                        selection,
                        &no_bindings, purpose)?,
                    DataMember::Variant(variant) => {
                        for field in syntax.tables.items.data_payload_fields(variant.payload) {
                            validate_selected_const_type(syntax, field.type_reference, visiting, selection, &no_bindings, purpose)?;
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
            visiting.pop();
            Ok(())
        }
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let argument_handles = syntax_type_references(syntax).type_reference_handles(*arguments);
            if argument_handles.is_empty() {
                let definition = selected_synthesized_instance(syntax, base_name)?;
                let declaration = definition.name.source_span();
                if visiting.contains(&declaration) {
                    return Ok(());
                }
                visiting.push(declaration);
                for member in syntax.tables.items.data_members(definition.members) {
                    match member {
                        DataMember::Field(field) => validate_selected_const_type(syntax,
                            field.type_reference,
                            visiting,
                            selection,
                            &no_bindings, purpose)?,
                        DataMember::Variant(variant) => {
                            for field in
                                syntax.tables.items.data_payload_fields(variant.payload)
                            {
                                validate_selected_const_type(syntax,
                                    field.type_reference,
                                    visiting,
                                    selection,
                                    &no_bindings, purpose)?;
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
                visiting.pop();
                return Ok(());
            }
            let definition = selected_data(syntax, base_name, selection)?;
            let parameters = syntax.items.type_parameters(definition.type_parameters);
            if !lifetime_arguments.is_empty()
                || !definition.lifetime_parameters.is_empty()
                || argument_handles.len() != arguments.len()
                || parameters.len() != definition.type_parameters.len()
                || parameters.len() != argument_handles.len()
            {
                return Err(
                    "generic const carriers require closed lifetime-free applications"
                        .to_owned(),
                );
            }
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!(
                    "boundary-opaque data `{base_name}` is not eligible as a const index"
                , base_name = base_name.as_str(syntax)));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{base_name}` is not eligible as a structural const index until quotient-backed canonical representatives land"
                , base_name = base_name.as_str(syntax)));
            }
            let declaration = definition.name.source_span();
            if visiting.contains(&declaration) {
                return Ok(());
            }
            visiting.push(declaration);
            let next_substitution = substituted_generic_arguments(syntax, parameters, argument_handles, substitution)?;
            for member in syntax.tables.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => validate_selected_const_type(syntax,
                        field.type_reference,
                        visiting,
                        selection,
                        &next_substitution, purpose)?,
                    DataMember::Variant(variant) => {
                        for field in syntax.tables.items.data_payload_fields(variant.payload) {
                            validate_selected_const_type(syntax, field.type_reference, visiting, selection, &next_substitution, purpose)?;
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
            visiting.pop();
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            closed_fixed_array_length(syntax, length, substitution, selection)?;
            validate_selected_const_type(syntax,
                *element_type,
                visiting,
                selection,
                substitution, purpose)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_selected_const_type(syntax, *base_type, visiting, selection, substitution, purpose)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Callable { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::StaticSelection(_)
 => Err(
            "const index types require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
    }
    }

    pub(in crate::preparation::generic_data) fn canonicalize_const_expression(
        syntax: &SyntaxTrees,
        expected_type: TypeReferenceHandle,
        expression: ExpressionHandle,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
    ) -> Result<CanonicalConstNode, String> {
        canonicalize_const_expression_with_admission(
            syntax,
            expected_type,
            expression,
            selection,
            substitution,
            ConstPredicateAdmission::ProveSyntax,
        )
    }

    pub(in crate::preparation::generic_data) fn canonicalize_const_expression_with_admission(
        syntax: &SyntaxTrees,
        expected_type: TypeReferenceHandle,
        expression: ExpressionHandle,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
        admission: ConstPredicateAdmission,
    ) -> Result<CanonicalConstNode, String> {
        if let Some(name) = syntax_type_references(syntax)
            .type_reference(expected_type)
            .as_named()
            && let Some(argument) = substitution.get(name.as_str(syntax))
        {
            return canonicalize_const_expression_with_admission(
                syntax,
                *argument,
                expression,
                selection,
                &GenericApplicationSubstitution::new(),
                admission,
            );
        }
        let origin = syntax_type_references(syntax).generic_application_origin(expected_type);
        if origin.is_valid() && origin != expected_type {
            return canonicalize_const_expression_with_admission(
                syntax,
                origin,
                expression,
                selection,
                substitution,
                admission,
            );
        }
        if let Some(semantic_type) = representation_semantic_type(syntax, expected_type, selection)?
        {
            return canonicalize_const_expression_with_admission(
                syntax,
                semantic_type,
                expression,
                selection,
                substitution,
                admission,
            );
        }
        match syntax.tables.type_references.type_reference(expected_type) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let node = canonicalize_const_expression_with_admission(
                    syntax,
                    *base_type,
                    expression,
                    selection,
                    substitution,
                    admission,
                )?;
                if admission == ConstPredicateAdmission::ProveSyntax {
                    crate::preparation::generic_data::prove_const_leaf_domain_constraints(
                        syntax,
                        syntax.tables.type_references.constraints(*constraints),
                        &node,
                        selection,
                    )?;
                }
                Ok(node)
            }
            TypeReferenceNode::Named(type_name)
                if LandedIntegerType::from_name(type_name.as_str(syntax)).is_some() =>
            {
                let ExpressionNode::Integer(literal) = syntax.expressions.expression(expression)
                else {
                    return Err(format!(
                        "expected an integer literal for `{type_name}`",
                        type_name = type_name.as_str(syntax)
                    ));
                };
                if literal.landing().is_some_and(|landing| {
                    landing.landed_type.name() != type_name.as_str(syntax)
                        || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                }) {
                    return Err(format!(
                        "integer literal landing conflicts with declared const component `{type_name}`",
                        type_name = type_name.as_str(syntax)
                    ));
                }
                let value = integer_literal_value(literal)
                    .ok_or_else(|| "integer literal exceeds the const-value envelope".to_owned())?;
                validate_syntax_integer_range(type_name.as_str(syntax), value)?;
                Ok(CanonicalConstNode::Integer {
                    type_name: type_name.as_str(syntax).to_owned(),
                    value,
                })
            }
            TypeReferenceNode::Named(type_name) if type_name.as_str(syntax) == "bool" => {
                let ExpressionNode::Boolean(value) = syntax.expressions.expression(expression)
                else {
                    return Err("expected a boolean literal for `bool`".to_owned());
                };
                Ok(CanonicalConstNode::Boolean(*value))
            }
            TypeReferenceNode::Named(type_name)
                if matches!(type_name.as_str(syntax), "f32" | "f64") =>
            {
                use numerics::literals::{FloatFormat, FloatLiteral};
                let format = if type_name.as_str(syntax) == "f32" {
                    FloatFormat::F32
                } else {
                    FloatFormat::F64
                };
                let literal = match syntax.expressions.expression(expression) {
                    ExpressionNode::Float(text) => text.literal(syntax),
                    ExpressionNode::Integer(integer) if integer.landing().is_none() => integer
                        .value_bignum()
                        .and_then(|value| FloatLiteral::parse(&value.to_string())),
                    _ => None,
                }
                .ok_or_else(|| {
                    format!(
                        "initializer conflicts with declared floating carrier `{type_name}`",
                        type_name = type_name.as_str(syntax)
                    )
                })?;
                if literal.landing().is_some_and(|landing| landing != format) {
                    return Err(format!(
                        "initializer conflicts with declared floating carrier `{type_name}`",
                        type_name = type_name.as_str(syntax)
                    ));
                }
                let bits = match format {
                    FloatFormat::F32
                        if literal.presented_bits().is_some() || !literal.value_f32().is_nan() =>
                    {
                        u64::from(literal.value_f32().to_bits())
                    }
                    FloatFormat::F64
                        if literal.presented_bits().is_some() || !literal.value_f64().is_nan() =>
                    {
                        literal.value_f64().to_bits()
                    }
                    _ => {
                        return Err(
                        "floating declaration identity requires explicit NaN representation bits"
                            .into(),
                    );
                    }
                };
                Ok(CanonicalConstNode::Float { format, bits })
            }
            TypeReferenceNode::Named(type_name) => {
                canonicalize_data_const_expression_with_admission(
                    syntax, type_name, expression, selection, admission,
                )
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                let argument_handles = syntax_type_references(syntax)
                    .type_reference_handles(*arguments)
                    .to_vec();
                if argument_handles.is_empty() {
                    let definition = selected_synthesized_instance(syntax, base_name)?;
                    return canonicalize_selected_data_const_expression_with_admission(
                        syntax,
                        definition,
                        definition.name.as_str(syntax),
                        expression,
                        selection,
                        &GenericApplicationSubstitution::new(),
                        admission,
                    );
                }
                let definition = selected_data(syntax, base_name, selection)?;
                let parameters = syntax.items.type_parameters(definition.type_parameters);
                if !lifetime_arguments.is_empty()
                    || !definition.lifetime_parameters.is_empty()
                    || argument_handles.len() != arguments.len()
                    || parameters.len() != definition.type_parameters.len()
                    || parameters.len() != argument_handles.len()
                {
                    return Err(
                        "generic const carrier is not a closed monomorphizable application"
                            .to_owned(),
                    );
                }
                let mut next_substitution = GenericApplicationSubstitution::new();
                let mut resolved_arguments = Vec::with_capacity(argument_handles.len());
                for (parameter, argument) in parameters.iter().zip(argument_handles.iter()) {
                    let resolved = resolved_generic_argument(syntax, *argument, substitution)?;
                    next_substitution.insert(parameter.name.as_str(syntax).to_owned(), resolved);
                    resolved_arguments.push(resolved);
                }
                let label =
                    generic_application_label(syntax, definition, &resolved_arguments, selection)?;
                if let Some(instance) =
                    synthesized_instance(syntax, selection, definition, &resolved_arguments)
                {
                    return canonicalize_selected_data_const_expression_with_admission(
                        syntax,
                        instance,
                        &label,
                        expression,
                        selection,
                        &GenericApplicationSubstitution::new(),
                        admission,
                    );
                }
                canonicalize_selected_data_const_expression_with_admission(
                    syntax,
                    definition,
                    &label,
                    expression,
                    selection,
                    &next_substitution,
                    admission,
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let length = closed_fixed_array_length(syntax, length, substitution, selection)?;
                let ExpressionNode::ArrayLiteral(values) =
                    syntax.expressions.expression(expression)
                else {
                    return Err("expected an array literal for fixed-array const value".to_owned());
                };
                let values = syntax.expressions.expression_handles(*values);
                if values.len() != length {
                    return Err(format!(
                        "fixed-array const value requires {length} elements but has {}",
                        values.len()
                    ));
                }
                let values = values
                    .iter()
                    .map(|value| {
                        canonicalize_const_expression_with_admission(
                            syntax,
                            *element_type,
                            *value,
                            selection,
                            substitution,
                            admission,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(CanonicalConstNode::Array {
                    type_name: substituted_type_label(
                        syntax,
                        expected_type,
                        substitution,
                        selection,
                    )?,
                    values,
                })
            }
            TypeReferenceNode::Unit => Err(
                "unit const values do not yet have a source literal; use an empty declared record"
                    .to_owned(),
            ),
            _ => Err("const value expression has an ineligible parameter type".to_owned()),
        }
    }

    /*
        The canonical carrier label after the enclosing application's bindings have
        been applied: a member spelled `T` or `Pair<T>` labels as its resolved
        closed carrier, never its template spelling.
    */
    fn substituted_type_label(
        syntax: &SyntaxTrees,
        reference: TypeReferenceHandle,
        substitution: &GenericApplicationSubstitution,
        selection: Option<&ConstantSelection>,
    ) -> Result<String, String> {
        match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Named(name) if substitution.contains_key(name.as_str(syntax)) => {
                selected_type_label(syntax, substitution[name.as_str(syntax)], selection)
            }
            TypeReferenceNode::Named(_)
                if syntax_type_references(syntax)
                    .generic_application_origin(reference)
                    .is_valid() =>
            {
                selected_type_label(
                    syntax,
                    syntax.type_references.generic_application_origin(reference),
                    selection,
                )
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let argument_handles = syntax_type_references(syntax)
                    .type_reference_handles(*arguments)
                    .to_vec();
                if argument_handles.is_empty() {
                    return selected_type_label(syntax, reference, selection);
                }
                let definition = selected_data(syntax, base_name, selection)?;
                let resolved = argument_handles
                    .iter()
                    .map(|argument| resolved_generic_argument(syntax, *argument, substitution))
                    .collect::<Result<Vec<_>, _>>()?;
                generic_application_label(syntax, definition, &resolved, selection)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let length = closed_fixed_array_length(syntax, length, substitution, selection)?;
                Ok(format!(
                    "[{}; {length}]",
                    substituted_type_label(syntax, *element_type, substitution, selection)?
                ))
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                substituted_type_label(syntax, *base_type, substitution, selection)
            }
            _ => selected_type_label(syntax, reference, selection),
        }
    }

    pub(in crate::preparation::generic_data) fn canonicalize_data_const_expression_with_admission(
        syntax: &SyntaxTrees,
        type_name: &Identifier,
        expression: ExpressionHandle,
        selection: Option<&ConstantSelection>,
        admission: ConstPredicateAdmission,
    ) -> Result<CanonicalConstNode, String> {
        let definition = selected_data(syntax, type_name, selection)?;
        canonicalize_selected_data_const_expression_with_admission(
            syntax,
            definition,
            definition.name.as_str(syntax),
            expression,
            selection,
            &GenericApplicationSubstitution::new(),
            admission,
        )
    }

    fn canonicalize_selected_data_const_expression_with_admission(
        syntax: &SyntaxTrees,
        definition: &DataDefinition,
        type_name: &str,
        expression: ExpressionHandle,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
        admission: ConstPredicateAdmission,
    ) -> Result<CanonicalConstNode, String> {
        let value = match syntax.expressions.expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                let (constructed, case_name) =
                    selected_constructor(syntax, &literal.constructor_name, selection)?;
                if !std::ptr::eq(constructed, definition) {
                    let instance_template = definition
                        .generic_instance
                        .and_then(
                            |origin| match syntax.type_references.type_reference(origin) {
                                TypeReferenceNode::Generic { base_name, .. } => {
                                    Some(base_name.clone())
                                }
                                _ => None,
                            },
                        )
                        .and_then(|base_name| selected_data(syntax, &base_name, selection).ok());
                    if !instance_template
                        .is_some_and(|template| std::ptr::eq(template, constructed))
                    {
                        return Err(format!(
                            "const constructor `{}` selects a different nominal carrier than `{type_name}`",
                            literal.constructor_name.as_str(syntax)
                        ));
                    }
                }
                if let Some(case_name) = &case_name {
                    let variant = syntax_items(syntax)
                        .data_members(definition.members)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant)
                                if variant.name.as_str(syntax) == case_name.as_str(syntax) =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })
                        .ok_or_else(|| {
                            format!("`{type_name}` has no case `{}`", case_name.as_str(syntax))
                        })?;
                    let declared_fields = syntax_items(syntax)
                        .data_payload_fields(variant.payload)
                        .iter()
                        .collect::<Vec<_>>();
                    let common_fields = syntax
                        .tables
                        .items
                        .data_members(definition.members)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        });
                    let declared_fields = common_fields.chain(declared_fields).collect::<Vec<_>>();
                    let fields = canonicalize_named_fields_with_admission(
                        syntax,
                        &declared_fields,
                        literal.fields,
                        selection,
                        substitution,
                        admission,
                    )?;
                    Ok(CanonicalConstNode::Variant {
                        type_name: type_name.to_owned(),
                        case_name: case_name.as_str(syntax).to_owned(),
                        fields,
                    })
                } else {
                    if syntax_items(syntax)
                        .data_members(definition.members)
                        .iter()
                        .any(|member| matches!(member, DataMember::Variant(_)))
                    {
                        return Err(format!(
                            "`{type_name}` is case data; its const value must name one case"
                        ));
                    }
                    let declared_fields = syntax_items(syntax)
                        .data_members(definition.members)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let fields = canonicalize_named_fields_with_admission(
                        syntax,
                        &declared_fields,
                        literal.fields,
                        selection,
                        substitution,
                        admission,
                    )?;
                    Ok(CanonicalConstNode::Record {
                        type_name: type_name.to_owned(),
                        fields,
                    })
                }
            }
            ExpressionNode::Name(path) => {
                let path = syntax.expressions.identifier_path_members(*path);
                let Some((case_name, owner)) = path.split_last() else {
                    return Err(format!("expected a `{type_name}` structural literal"));
                };
                let Some(first) = owner.first() else {
                    return Err(format!("expected a `{type_name}` case owner"));
                };
                let head = Identifier::new(owner.joined_path_in(syntax), first.source_span());
                let selected = selected_data(syntax, &head, selection)?;
                let same_carrier = std::ptr::eq(selected, definition)
                    || definition
                        .generic_instance
                        .and_then(
                            |origin| match syntax.type_references.type_reference(origin) {
                                TypeReferenceNode::Generic { base_name, .. } => {
                                    Some(base_name.clone())
                                }
                                _ => None,
                            },
                        )
                        .and_then(|base_name| selected_data(syntax, &base_name, selection).ok())
                        .is_some_and(|template| std::ptr::eq(template, selected));
                if !same_carrier {
                    return Err(format!(
                        "expected a `{type_name}` value, got `{}`",
                        head.as_str(syntax)
                    ));
                }
                let variant = syntax_items(syntax)
                    .data_members(definition.members)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.name.as_str(syntax) == case_name.as_str(syntax) =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        format!(
                            "`{type_name}` has no case `{case_name}`",
                            case_name = case_name.as_str(syntax)
                        )
                    })?;
                if !variant.payload.is_empty() {
                    return Err(format!(
                        "case `{type_name}::{case_name}` requires named payload fields",
                        case_name = case_name.as_str(syntax)
                    ));
                }
                Ok(CanonicalConstNode::Variant {
                    type_name: type_name.to_owned(),
                    case_name: case_name.as_str(syntax).to_owned(),
                    fields: Vec::new(),
                })
            }
            _ => Err(format!("expected a `{type_name}` structural literal")),
        }?;
        if admission == ConstPredicateAdmission::ProveSyntax {
            prove_const_data_predicates(syntax, definition, &value, substitution, selection)?;
        }
        Ok(value)
    }

    fn canonicalize_named_fields_with_admission(
        syntax: &SyntaxTrees,
        declared_fields: &[&syntax_trees::item::DataField],
        literal_fields: HandleSpan<syntax_trees::expression::TableStructLiteralField>,
        selection: Option<&ConstantSelection>,
        substitution: &GenericApplicationSubstitution,
        admission: ConstPredicateAdmission,
    ) -> Result<Vec<(String, CanonicalConstNode)>, String> {
        let authored = syntax.expressions.struct_fields(literal_fields);
        for declared in declared_fields {
            let matches = authored
                .iter()
                .filter(|field| field.name.as_str(syntax) == declared.name.as_str(syntax))
                .count();
            if matches != 1 {
                return Err(if matches == 0 {
                    format!("missing const field `{}`", declared.name.as_str(syntax))
                } else {
                    format!("duplicate const field `{}`", declared.name.as_str(syntax))
                });
            }
        }
        let selected_fields = authored
            .iter()
            .map(|field| {
                declared_fields
                    .iter()
                    .enumerate()
                    .find(|(_, declared)| declared.name.as_str(syntax) == field.name.as_str(syntax))
                    .map(|(position, declared)| (position, *declared, field))
                    .ok_or_else(|| format!("unknown const field `{}`", field.name.as_str(syntax)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut canonical = vec![None; declared_fields.len()];
        for (position, declared, field) in selected_fields {
            canonical[position] = Some((
                declared.name.as_str(syntax).to_owned(),
                canonicalize_const_expression_with_admission(
                    syntax,
                    declared.type_reference,
                    field.value,
                    selection,
                    substitution,
                    admission,
                )?,
            ));
        }
        canonical
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "const construction lost a validated active field".to_owned())
    }

    pub(in crate::preparation::generic_data) fn validate_syntax_integer_range(
        type_name: &str,
        value: i128,
    ) -> Result<(), String> {
        let Some(integer_type) = LandedIntegerType::from_name(type_name) else {
            return Err(format!("`{type_name}` is not an integer const type"));
        };
        let (minimum, maximum) = integer_type.bounds();
        if value < minimum || value > maximum {
            Err(format!("const value `{value}` does not fit `{type_name}`"))
        } else {
            Ok(())
        }
    }

    pub(in crate::preparation::generic_data) fn validate_canonical_rat(
        value: &CanonicalConstNode,
    ) -> Result<(), String> {
        let CanonicalConstNode::Record { fields, .. } = value else {
            return Err("`Rat` index value must be a structural record".to_owned());
        };
        let numerator = fields
            .iter()
            .find(|(name, _)| name == "num")
            .map(|(_, value)| value)
            .ok_or_else(|| "`Rat` index value is missing `num`".to_owned())?;
        let denominator = fields
            .iter()
            .find(|(name, _)| name == "den")
            .map(|(_, value)| nat_value(value))
            .transpose()?
            .ok_or_else(|| "`Rat` index value is missing `den`".to_owned())?;
        let CanonicalConstNode::Record { fields, .. } = numerator else {
            return Err("`Rat.num` must be an `IntPair` record".to_owned());
        };
        let negative = fields
            .iter()
            .find(|(name, _)| name == "neg")
            .map(|(_, value)| nat_value(value))
            .transpose()?
            .ok_or_else(|| "`Rat.num` is missing `neg`".to_owned())?;
        let positive = fields
            .iter()
            .find(|(name, _)| name == "pos")
            .map(|(_, value)| nat_value(value))
            .transpose()?
            .ok_or_else(|| "`Rat.num` is missing `pos`".to_owned())?;
        if denominator == 0 {
            return Err("`Rat` index denominator must be positive".to_owned());
        }
        if negative != 0 && positive != 0 {
            return Err(
            "`Rat` index signed coordinates must be cancelled (at least one of `num.neg` and `num.pos` must be zero)"
                .to_owned(),
        );
        }
        let magnitude = negative.max(positive);
        if gcd_usize(magnitude, denominator) != 1 {
            return Err(
                "`Rat` index numerator magnitude and denominator must be gcd-reduced".to_owned(),
            );
        }
        Ok(())
    }

    pub(in crate::preparation::generic_data) fn nat_value(
        value: &CanonicalConstNode,
    ) -> Result<usize, String> {
        match value {
            CanonicalConstNode::Variant {
                type_name,
                case_name,
                fields,
            } if type_name == "Nat" && case_name == "Zero" && fields.is_empty() => Ok(0),
            CanonicalConstNode::Variant {
                type_name,
                case_name,
                fields,
            } if type_name == "Nat" && case_name == "Succ" => {
                let previous = fields
                    .iter()
                    .find(|(name, _)| name == "prev")
                    .map(|(_, value)| nat_value(value))
                    .transpose()?
                    .ok_or_else(|| "`Nat::Succ` is missing `prev`".to_owned())?;
                previous
                    .checked_add(1)
                    .ok_or_else(|| "`Nat` const value is too large".to_owned())
            }
            _ => Err("`Rat` canonicality requires structural core `Nat` fields".to_owned()),
        }
    }

    pub(in crate::preparation::generic_data) fn gcd_usize(
        mut left: usize,
        mut right: usize,
    ) -> usize {
        while right != 0 {
            let remainder = left % right;
            left = right;
            right = remainder;
        }
        left
    }

    pub(in crate::preparation::generic_data) fn const_integer_in_envelope(
        value: i128,
    ) -> Option<i128> {
        (value >= i128::from(i64::MIN) && value <= i128::from(u64::MAX)).then_some(value)
    }

    pub(in crate::preparation::generic_data) fn checked_fact_integer(
        value: Option<i128>,
        operation: &str,
    ) -> Result<ConstFactValue, String> {
        value
            .and_then(const_integer_in_envelope)
            .map(ConstFactValue::Integer)
            .ok_or_else(|| format!("{operation} exceeds the signed/unsigned 64-bit envelope"))
    }
}

use crate::preparation::generic_data::module_constants::reject_module_constant_selection;
pub(super) use arguments::{
    EvaluatedConst, const_integer_type, evaluate_const_argument_expression,
    generic_const_integer_types, prepare_closed_application,
};
pub(super) use domains::{
    canonicalize_closed_domain_indices, domain_index_parameters, integer_literal_value,
    qualified_const_name,
};
pub(crate) use fact_values::{
    ConstFactValue, ConstScalarSpelling, ConstScalarValue, evaluate_const_fact_binary,
};
pub(in crate::preparation::generic_data) use facts::prove_const_leaf_domain_constraints;
pub(crate) use facts::{
    const_type_requires_declaration_proof, evaluate_const_fact_expression,
    evaluate_const_membership_fact, prove_const_literal_predicates,
};
use remainder::validate_anonymous_remainder;
pub(in crate::preparation) use templates::collect_type_positions;
pub(super) use templates::{
    capture_machine_runtime_template_names, collect_callable_type_reference_positions,
    collect_data_type_reference_positions, collect_selected_type_reference_positions,
    collect_stored_type_reference_positions, normalize_generic_template_const_expressions,
    replace_const_expression_names_from, replace_machine_const_expression_names_from,
};
pub(in crate::preparation) use values::canonicalize_selected_const_definition;
pub(crate) use values::closed_fixed_array_length;
pub(super) use values::{
    ConstPredicateAdmission, canonicalize_const_definition, canonicalize_const_expression,
    canonicalize_selected_declaration_value,
    canonicalize_selected_declaration_value_with_admission, canonicalize_selected_index_expression,
    checked_fact_integer, const_integer_in_envelope, syntax_type_identity,
    validate_const_index_type, validate_syntax_integer_range,
};

/*
    A `name` selection that names a module-scoped const value or const
    declaration is rejected up front — the shared module-constant gate both
    evaluation walkers apply before their own normalization.
*/
fn module_constant_selection_gate(
    syntax: &SyntaxTrees,
    name: &Identifier,
    const_values: &HashMap<String, i128>,
    const_definitions: &HashMap<String, ConstDefinition>,
) -> Result<(), Diagnostic> {
    if const_values.contains_key(name.as_str(syntax))
        || const_definitions.contains_key(name.as_str(syntax))
    {
        reject_module_constant_selection(syntax, name.as_str(syntax), name.source_span())
            .map_err(|reason| Diagnostic::error(reason).with_source_span(name.source_span()))?;
    }
    Ok(())
}

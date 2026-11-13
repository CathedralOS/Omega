/*
We validate structural type references and array-extent index expressions
before computing package-review type identities.

A type argument carries a value only in a const or value parameter of its own
declaration (domain applications use that declaration's index parameters, and
generic-carrier families omit the carrier parameter). Declaration selection
alone does not freeze arithmetic or call arguments: retained array recipes must
preserve their exact original expression node, handle roster, and use edges,
while builtin operator selection licenses only the exact operation without
manufacturing conformance laws or algebraic rewrites.
*/

use crate::capture::SourceDeclarationInput;
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::wire::is_layout_domain_name;

pub(crate) fn validate_package_type_identity_input(
    compilation: &SourceDeclarationInput<'_>,
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<(), Vec<Diagnostic>> {
    validate_package_type_identity_input_inner(
        compilation,
        program,
        type_reference,
        binders,
        false,
        None,
    )
}

pub(crate) fn validate_package_type_identity_input_inner(
    compilation: &SourceDeclarationInput<'_>,
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    allow_const_value: bool,
    extent_exposure: Option<AuthoredDeclarationSelectionExposure>,
) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::data::TypeParameter;
    use typed_trees::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};

    macro_rules! validate_child {
        ($child:expr, $allow_const:expr $(,)?) => {
            validate_package_type_identity_input_inner(
                compilation,
                program,
                $child,
                binders,
                $allow_const,
                extent_exposure,
            )
        };
    }
    let validate_argument = |argument, parameters: Option<&[TypeParameter]>, index: usize| {
        let accepts_value = parameters
            .and_then(|parameters| parameters.get(index))
            .is_some_and(|parameter| parameter.kind.is_const_or_value());
        validate_child!(argument, accepts_value)
    };

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Callable { contract } => program
            .runtime_callable_contract_identity(*contract, binders)
            .map(|_| ())
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "package review callable type has no complete scoped contract: {reason}"
                ))]
            }),
        TypeReferenceNode::Reference { referee, .. } => {
            validate_child!(*referee, false)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_child!(*base_type, false)?;
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Domain(domain) => {
                        use typed_trees::types::DomainConstraintSubject;

                        match domain.subject {
                            DomainConstraintSubject::Declared => {
                                if domain.name.as_str() == "OmegaLayout"
                                    || is_layout_domain_name(domain.name.as_str())
                                {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects an unclassified or legacy flattened OmegaLayout constraint",
                                    )]);
                                }
                                if !domain.symbol.is_valid() {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects a declared domain without an exact symbol",
                                    )]);
                                }
                            }
                            DomainConstraintSubject::Carry(_)
                            | DomainConstraintSubject::Value(_) => {
                                if domain.symbol.is_valid() || !domain.arguments.is_empty() {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects a malformed compiler-owned scalar domain constraint",
                                    )]);
                                }
                            }
                            DomainConstraintSubject::OmegaLayout { .. } => {
                                if domain.symbol.is_valid() || domain.arguments.len() != 1 {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects a malformed compiler-owned OmegaLayout constraint",
                                    )]);
                                }
                            }
                        }
                        let declared_parameters = (domain.subject
                            == typed_trees::types::DomainConstraintSubject::Declared)
                            .then(|| program.domain_definition(domain.symbol))
                            .flatten()
                            .map(|definition| {
                                typed_trees::domain::index_parameters(program, definition)
                            });
                        for (index, argument) in domain.arguments.iter().enumerate() {
                            validate_argument(*argument, declared_parameters, index)?;
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            validate_child!(*element_type, false)?;
            match length {
                FixedArrayLength::Literal(_) => Ok(()),
                FixedArrayLength::ConstParameter { symbol, name } => {
                    validate_package_const_binder(program, *symbol, name.as_str(), binders)
                }
                FixedArrayLength::Expression(value) => {
                    let typed_trees::types::TypeReferenceNode::ConstExpression(expression) =
                        program.type_reference_table.type_reference(*value)
                    else {
                        return Err(vec![Diagnostic::error(
                            "package review rejects a malformed array extent recipe",
                        )]);
                    };
                    validate_package_index_expression(
                        compilation,
                        program,
                        *expression,
                        binders,
                        extent_exposure,
                    )
                }
                FixedArrayLength::ConstCall { .. } => Err(vec![Diagnostic::error(
                    "package review rejects an unevaluated const call in structural type identity",
                )]),
            }
        }
        TypeReferenceNode::Slice { element_type } => validate_child!(*element_type, false),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let parameters = program
                .data_definition_by_symbol(*base_symbol)
                .map(|definition| program.data_type_parameters(definition))
                .or_else(|| {
                    program
                        .conformance_by_symbol(*base_symbol)
                        .map(|declaration| program.conformance_type_parameters(declaration))
                });
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            for (index, argument) in arguments.iter().enumerate() {
                validate_argument(*argument, parameters, index)?;
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if !allow_const_value {
                return Err(vec![Diagnostic::error(
                    "package review rejects a value expression outside one exact declared const or value parameter",
                )]);
            }
            validate_package_index_expression(compilation, program, *expression, binders, None)
        }
        TypeReferenceNode::Named { symbol, name } => validate_package_named_type_leaf(
            program,
            *symbol,
            name.as_str(),
            binders,
            allow_const_value,
        ),
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Unit => Ok(()),
    }
}

fn validate_package_named_type_leaf(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
    spelling: &str,
    binders: &[(SymbolHandle, String)],
    allow_const_value: bool,
) -> Result<(), Vec<Diagnostic>> {
    if symbol.is_valid() {
        if program.symbols.get(symbol).kind == symbols::SymbolKind::Const {
            return Err(vec![Diagnostic::error(
                "package review rejects a residual const declaration in structural type identity",
            )]);
        }
        return Ok(());
    }
    if allow_const_value
        && (CanonicalConstValue::from_atom(spelling).is_some() || spelling.parse::<i128>().is_ok())
    {
        return Ok(());
    }
    let mut matches = binders.iter().filter(|(candidate, _)| {
        candidate.is_valid() && program.symbols.name(*candidate) == spelling
    });
    if matches.next().is_some() && matches.next().is_none() {
        return Ok(());
    }
    Err(vec![Diagnostic::error(
        "package review rejects a source-spelled type or const leaf without exact semantic identity",
    )])
}

fn validate_package_const_binder(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
    spelling: &str,
    binders: &[(SymbolHandle, String)],
) -> Result<(), Vec<Diagnostic>> {
    if symbol.is_valid() && binders.iter().any(|(candidate, _)| *candidate == symbol) {
        return Ok(());
    }
    let mut matches = binders.iter().filter(|(candidate, _)| {
        !symbol.is_valid() && candidate.is_valid() && program.symbols.name(*candidate) == spelling
    });
    if matches.next().is_some() && matches.next().is_none() {
        return Ok(());
    }
    Err(vec![Diagnostic::error(
        "package review rejects a const binder without one exact telescope identity",
    )])
}

fn validate_package_index_expression(
    compilation: &SourceDeclarationInput<'_>,
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
    builtin_exposure: Option<AuthoredDeclarationSelectionExposure>,
) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::expression::ExpressionNode;
    macro_rules! index_expr {
        ($value:expr => $exposure:expr) => {
            validate_package_index_expression(compilation, program, $value, binders, $exposure)
        };
    }

    if builtin_exposure.is_some()
        && (!compilation
            .original_program
            .typed
            .expression_table
            .expression_is_valid(expression)
            || compilation
                .original_program
                .typed
                .expression_table
                .expression(expression)
                != program.expression_table.expression(expression))
    {
        return Err(vec![Diagnostic::error(
            "package review array extent differs from its retained original expression and use edges",
        )]);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::MachineValue(value) => {
            let original = &compilation.original_program.typed;
            if !original.expression_table.expression_is_valid(expression)
                || !matches!(original.expression_table.expression(expression),
                    ExpressionNode::MachineValue(retained) if retained == value)
            {
                return Err(vec![Diagnostic::error(
                    "package review machine index lost its exact original code selection",
                )]);
            }
            let current = program
                .runtime_machine_value_identity(value, binders)
                .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
            let retained = original
                .runtime_machine_value_identity(value, binders)
                .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
            if current != retained {
                return Err(vec![Diagnostic::error(
                    "package review machine index changed its complete contract or selected declaration",
                )]);
            }
            // The typed index identity retains MachineValue as an opaque code
            // selection, including its static application; it is no invocation
            // and supplies no mathematical address or numeric literal.
            Ok(())
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let spelling = members.joined_path();
            if !path.symbol.is_valid()
                && (CanonicalConstValue::from_atom(&spelling).is_some()
                    || spelling.parse::<i128>().is_ok())
            {
                return Ok(());
            }
            if members.len() == 1 {
                validate_package_const_binder(program, path.symbol, &spelling, binders)
            } else {
                Err(vec![Diagnostic::error(
                    "package review rejects an index name without one exact const-binder or compiler-const identity",
                )])
            }
        }
        ExpressionNode::Integer(_) => Ok(()),
        ExpressionNode::Unary(unary) => index_expr!(unary.operand => builtin_exposure),
        ExpressionNode::Binary(binary) => {
            if let Some(selection) = program.expression_table.open_index_operation(expression) {
                if !selection.operator.is_valid()
                    || !selection.provider.is_valid()
                    || !selection.algebra_trait.is_valid()
                {
                    return Err(vec![Diagnostic::error(
                        "package review rejects an open index operation with incomplete semantic authority",
                    )]);
                }
            } else {
                let Some(exposure) = builtin_exposure else {
                    return Err(vec![Diagnostic::error(
                        "package review rejects an open index operation without exact checked algebra selection",
                    )]);
                };
                use language_semantics::declaration_selection::{
                    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionTarget,
                };
                let selection = crate::capture::contracts::expressions::operators::exact_checked_operator_selection(
                    compilation, program, expression, Some(exposure),
                )?;
                if selection.target()
                    != AuthoredDeclarationSelectionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                    )
                {
                    return Err(vec![Diagnostic::error(
                        "package review rejects an open index operation without exact checked builtin or algebra selection",
                    )]);
                }
            }
            index_expr!(binary.left => builtin_exposure)?;
            index_expr!(binary.right => builtin_exposure)
        }
        ExpressionNode::Call(call) if builtin_exposure.is_some() => {
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
            };
            if call.authored_machine_application.is_valid()
                || call.exact_coercion_target.is_valid()
                || call.static_machine_parameter.is_valid()
                || call.static_requirement_dispatch.is_some()
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.quotient_operation.is_some()
                || call.private_layout_operation.is_some()
                || call.installed_interrupt_operation.is_some()
                || call.layout_query.is_some()
            {
                return Err(vec![Diagnostic::error(
                    "package review array extent call requires an exact represented ordinary application identity",
                )]);
            }
            let selection = crate::capture::contracts::expressions::operators::exact_checked_expression_selection(
                compilation, program, expression, AuthoredDeclarationSelectionKind::Call, builtin_exposure,
            )?;
            let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
                return Err(vec![Diagnostic::error(
                    "package review array extent call has no exact resolved declaration",
                )]);
            };
            if target.selected_symbol() != call.target_symbol || call.receiver.is_valid() {
                return Err(vec![Diagnostic::error(
                    "package review array extent call differs from its retained exact declaration",
                )]);
            }
            if program.expression_table.expression_handles(call.arguments)
                != compilation
                    .original_program
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)
            {
                return Err(vec![Diagnostic::error(
                    "package review array extent call arguments differ from the retained original roster",
                )]);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                index_expr!(*argument => builtin_exposure)?;
            }
            Ok(())
        }
        ExpressionNode::Cast(cast) if builtin_exposure.is_some() => {
            if cast.form != language_core::cast_form::CastForm::Value
                || cast.semantic_domain_symbol.is_valid()
                || !cast.semantic_domain_arguments.is_empty()
                || cast.semantic_domain.count() != 0
                || validation::arithmetic_result_type_reference(program, cast.result_type).is_none()
                || program.primitive_type_reference(cast.target_type)
                    != program.primitive_type_reference(cast.result_type)
                || program.arithmetic_domain_for_type_reference(cast.result_type) != cast.domain
            {
                return Err(vec![Diagnostic::error(
                    "package review array extent cast requires its exact represented scalar carrier and policy identity",
                )]);
            }
            validate_package_type_identity_input_inner(
                compilation,
                program,
                cast.target_type,
                binders,
                false,
                builtin_exposure,
            )?;
            index_expr!(cast.value => builtin_exposure)
        }
        ExpressionNode::SavedSubject(_)
        | ExpressionNode::Match(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => Err(vec![Diagnostic::error(
            "package review rejects an unsupported structural index expression",
        )]),
    }
}

pub(crate) fn missing_exact_toolchain_type_owner() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "package review structural type identity has unresolved nominal ownership or is missing exact source-backed toolchain ownership",
    )]
}

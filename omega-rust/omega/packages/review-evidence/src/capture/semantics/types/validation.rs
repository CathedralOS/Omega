use crate::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use symbols::JoinedPath;
use symbols::SymbolHandle;

pub(crate) fn validate_package_type_identity_input(
    compilation: &PackageReviewInput<'_>,
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
    compilation: &PackageReviewInput<'_>,
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    allow_const_value: bool,
    extent_exposure: Option<AuthoredDeclarationSelectionExposure>,
) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => validate_package_type_identity_input_inner(
            compilation,
            program,
            *referee,
            binders,
            false,
            extent_exposure,
        ),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_package_type_identity_input_inner(
                compilation,
                program,
                *base_type,
                binders,
                false,
                extent_exposure,
            )?;
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        validate_package_index_expression(
                            compilation,
                            program,
                            *minimum,
                            binders,
                            None,
                        )?;
                        validate_package_index_expression(
                            compilation,
                            program,
                            *maximum,
                            binders,
                            None,
                        )?;
                    }
                    TypeConstraintNode::Domain(domain) => {
                        use typed_trees::types::DomainConstraintSubject;

                        match domain.subject {
                            DomainConstraintSubject::Declared => {
                                if domain.name.as_str() == "OmegaLayout"
                                    || typed_trees::wire::is_layout_domain_name(
                                        domain.name.as_str(),
                                    )
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
                            .map(|definition| program.domain_type_parameters(definition));
                        for (index, argument) in domain.arguments.iter().enumerate() {
                            let is_const = declared_parameters
                                .and_then(|parameters| parameters.get(index + 1))
                                .is_some_and(|parameter| {
                                    matches!(
                                        parameter.kind,
                                        typed_trees::data::TypeParameterKind::Const { .. }
                                    )
                                });
                            validate_package_type_identity_input_inner(
                                compilation,
                                program,
                                *argument,
                                binders,
                                is_const,
                                extent_exposure,
                            )?;
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
            validate_package_type_identity_input_inner(
                compilation,
                program,
                *element_type,
                binders,
                false,
                extent_exposure,
            )?;
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
        TypeReferenceNode::Slice { element_type } => validate_package_type_identity_input_inner(
            compilation,
            program,
            *element_type,
            binders,
            false,
            extent_exposure,
        ),
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
            for (index, argument) in program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .enumerate()
            {
                let is_const = parameters
                    .and_then(|parameters| parameters.get(index))
                    .is_some_and(|parameter| {
                        matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Const { .. }
                        )
                    });
                validate_package_type_identity_input_inner(
                    compilation,
                    program,
                    *argument,
                    binders,
                    is_const,
                    extent_exposure,
                )?;
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if !allow_const_value {
                return Err(vec![Diagnostic::error(
                    "package review rejects a const expression outside one exact declared const-parameter slot",
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
        && (language_semantics::const_value::CanonicalConstValue::from_atom(spelling).is_some()
            || spelling.parse::<i128>().is_ok())
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
    compilation: &PackageReviewInput<'_>,
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
    builtin_exposure: Option<AuthoredDeclarationSelectionExposure>,
) -> Result<(), Vec<Diagnostic>> {
    use typed_trees::expression::ExpressionNode;

    // Declaration selection alone does not freeze arithmetic or call arguments.
    // Retained array recipes must preserve their exact original node and edges;
    // projection-local type substitutions are handled by the identity context.
    if builtin_exposure.is_some()
        && (!compilation
            .custody
            .typed
            .expression_table
            .expression_is_valid(expression)
            || compilation
                .custody
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
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let spelling = members.joined_path();
            if !path.symbol.is_valid()
                && (language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
                    .is_some()
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
        ExpressionNode::Unary(unary) => validate_package_index_expression(
            compilation,
            program,
            unary.operand,
            binders,
            builtin_exposure,
        ),
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
                // Builtin selection licenses this exact operation only. No
                // conformance laws or algebraic rewriting are manufactured.
            }
            validate_package_index_expression(
                compilation,
                program,
                binary.left,
                binders,
                builtin_exposure,
            )?;
            validate_package_index_expression(
                compilation,
                program,
                binary.right,
                binders,
                builtin_exposure,
            )
        }
        ExpressionNode::Call(call) if builtin_exposure.is_some() => {
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
            };
            // The canonical call production carries an ordinary monomorphic
            // target and value arguments. Other semantic applications must not
            // collapse into that identity by dropping their static components.
            if call.authored_machine_application.is_valid()
                || call.exact_coercion_target.is_valid()
                || call.static_machine_parameter.is_valid()
                || call.static_requirement_dispatch.is_some()
                || !call.machine_arguments.is_empty()
                || !call.evidence_arguments.is_empty()
                || call.quotient_operation.is_some()
                || call.private_layout_operation.is_some()
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
            // A HandleSpan is a roster address, not the roster's contents.
            if program.expression_table.expression_handles(call.arguments)
                != compilation
                    .custody
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)
            {
                return Err(vec![Diagnostic::error(
                    "package review array extent call arguments differ from the retained original roster",
                )]);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_package_index_expression(
                    compilation,
                    program,
                    *argument,
                    binders,
                    builtin_exposure,
                )?;
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
            validate_package_index_expression(
                compilation,
                program,
                cast.value,
                binders,
                builtin_exposure,
            )
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

use super::{framed_identity, review_lifetime_topology_with_substitutions};
use crate::evidence::{PackageReviewDataField, PackageReviewTypeIdentity};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_data_field(
    compilation: &CheckedCompilation,
    field: &psi_typed_trees::data::DataField,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewDataField, Vec<Diagnostic>> {
    Ok(PackageReviewDataField {
        identity: field.identity,
        name: field.name.as_str().to_owned(),
        relevance: field.relevance,
        type_identity: review_signature_type_identity_with_binders(
            compilation,
            field.type_reference,
            binders,
            lifetime_binders,
        )?,
    })
}

pub(crate) fn review_type_identity_with_binders(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .package_qualified_type_identity_with_binders_and_toolchain_sources(
            type_reference,
            binders,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: identity.into_string(),
    })
}

pub(crate) fn review_type_identity_with_binders_and_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .package_qualified_type_identity_with_binders_substitutions_and_toolchain_sources(
            type_reference,
            binders,
            substitutions,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: identity.into_string(),
    })
}

pub(crate) fn validate_package_type_identity_input(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<(), Vec<Diagnostic>> {
    validate_package_type_identity_input_inner(program, type_reference, binders, false)
}

pub(crate) fn validate_package_type_identity_input_inner(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    allow_const_value: bool,
) -> Result<(), Vec<Diagnostic>> {
    use psi_typed_trees::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            validate_package_type_identity_input_inner(program, *referee, binders, false)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_package_type_identity_input_inner(program, *base_type, binders, false)?;
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range { minimum, maximum } => {
                        validate_package_index_expression(program, *minimum, binders)?;
                        validate_package_index_expression(program, *maximum, binders)?;
                    }
                    TypeConstraintNode::Domain(domain) => {
                        use psi_typed_trees::types::DomainConstraintSubject;

                        match domain.subject {
                            DomainConstraintSubject::Declared => {
                                if domain.name.as_str() == "OmegaLayout"
                                    || psi_typed_trees::wire::is_layout_domain_name(
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
                            == psi_typed_trees::types::DomainConstraintSubject::Declared)
                            .then(|| {
                                program
                                    .domain_definitions()
                                    .iter()
                                    .find(|definition| definition.symbol == domain.symbol)
                            })
                            .flatten()
                            .map(|definition| program.domain_type_parameters(definition));
                        for (index, argument) in domain.arguments.iter().enumerate() {
                            let is_const = declared_parameters
                                .and_then(|parameters| parameters.get(index + 1))
                                .is_some_and(|parameter| {
                                    matches!(
                                        parameter.kind,
                                        psi_typed_trees::data::TypeParameterKind::Const { .. }
                                    )
                                });
                            validate_package_type_identity_input_inner(
                                program, *argument, binders, is_const,
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
            validate_package_type_identity_input_inner(program, *element_type, binders, false)?;
            match length {
                FixedArrayLength::Literal(_) => Ok(()),
                FixedArrayLength::ConstParameter { symbol, name } => {
                    validate_package_const_binder(program, *symbol, name.as_str(), binders)
                }
                FixedArrayLength::ConstCall { .. } => Err(vec![Diagnostic::error(
                    "package review rejects an unevaluated const call in structural type identity",
                )]),
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            validate_package_type_identity_input_inner(program, *element_type, binders, false)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let parameters = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *base_symbol)
                .map(|definition| program.data_type_parameters(definition));
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
                            psi_typed_trees::data::TypeParameterKind::Const { .. }
                        )
                    });
                validate_package_type_identity_input_inner(program, *argument, binders, is_const)?;
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if !allow_const_value {
                return Err(vec![Diagnostic::error(
                    "package review rejects a const expression outside one exact declared const-parameter slot",
                )]);
            }
            validate_package_index_expression(program, *expression, binders)
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

pub(crate) fn validate_package_named_type_leaf(
    program: &psi_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    spelling: &str,
    binders: &[(SymbolHandle, String)],
    allow_const_value: bool,
) -> Result<(), Vec<Diagnostic>> {
    if symbol.is_valid() {
        if program.symbols.get(symbol).kind == psi_symbols::SymbolKind::Const {
            return Err(vec![Diagnostic::error(
                "package review rejects a residual const declaration in structural type identity",
            )]);
        }
        return Ok(());
    }
    if allow_const_value
        && (psi_language_semantics::const_value::CanonicalConstValue::from_atom(spelling).is_some()
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

pub(crate) fn validate_package_const_binder(
    program: &psi_typed_trees::TypedTrees,
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

pub(crate) fn validate_package_index_expression(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<(), Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let spelling = members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if !path.symbol.is_valid()
                && (psi_language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
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
        ExpressionNode::Unary(unary) => {
            validate_package_index_expression(program, unary.operand, binders)
        }
        ExpressionNode::Binary(binary) => {
            let mut selections = program
                .open_index_normalizations
                .iter()
                .flat_map(|normalization| &normalization.operations)
                .filter(|selection| selection.expression == expression);
            let Some(selection) = selections.next() else {
                return Err(vec![Diagnostic::error(
                    "package review rejects an open index operation without exact checked selection",
                )]);
            };
            if selections.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "package review rejects an open index operation with duplicate checked selections",
                )]);
            }
            if !selection.operator.is_valid()
                || !selection.provider.is_valid()
                || !selection.algebra_trait.is_valid()
            {
                return Err(vec![Diagnostic::error(
                    "package review rejects an open index operation with incomplete semantic authority",
                )]);
            }
            validate_package_index_expression(program, binary.left, binders)?;
            validate_package_index_expression(program, binary.right, binders)
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
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

/// Public signature identity layers erased borrow-region relationships over
/// the ordinary package-qualified runtime type identity. General structural
/// type identity intentionally erases these tags; package compatibility may
/// not, because changing which input owns an output loan changes the callable
/// contract without changing layout or monomorphization.
pub(crate) fn review_signature_type_identity_with_binders(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    review_signature_type_identity_with_binders_and_substitutions(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        &[],
    )
}

pub(crate) fn review_signature_type_identity_with_binders_and_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        &[],
    )
}

pub(crate) fn review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let runtime = compilation
        .package_qualified_type_identity_with_binders_substitutions_and_toolchain_sources(
            type_reference,
            binders,
            substitutions,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?
        .into_string();
    let lifetime = review_lifetime_topology_with_substitutions(
        compilation,
        type_reference,
        lifetime_binders,
        substitutions,
        lifetime_substitutions,
        &mut Vec::new(),
    )?;
    Ok(PackageReviewTypeIdentity {
        canonical: framed_identity("signature-type", &[runtime, lifetime]),
    })
}

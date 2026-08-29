use super::super::exact_identity::*;
use super::super::public_api::*;
use super::*;
use crate::model::*;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_contract_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    project_contract_expression_with_substitutions(
        compilation,
        context,
        binders,
        expression,
        &[],
        &[],
        checked_fact,
        depth,
    )
}

pub(crate) fn exact_fact_call_projection<'compilation>(
    compilation: &'compilation CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    projection_expression: psi_typed_trees::expression::ExpressionHandle,
    call_expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Result<&'compilation psi_checked_trees::CheckedFactCallProjection, Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    let ExpressionNode::Call(call) = compilation.expression_table.expression(call_expression)
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection does not rejoin a call expression",
            context.subject_kind, context.subject_name
        ))]);
    };
    let matching = compilation
        .facts
        .fact_call_projections
        .iter()
        .filter(|projection| {
            projection.projection_expression == projection_expression
                && projection.call_expression == call_expression
                && projection.target_state == call.target_symbol
                && projection.machine_arguments == call.machine_arguments
        })
        .collect::<Vec<_>>();
    let [projection] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection rejoins {} exact eligibility certificates; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    let target = compilation
        .machines()
        .iter()
        .find(|machine| machine.symbol == projection.target_machine)
        .and_then(|machine| {
            compilation
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == projection.target_state)
        });
    if target.is_none_or(|state| state.return_type != projection.result_type) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection certificate no longer rejoins its exact result type",
            context.subject_kind, context.subject_name
        ))]);
    }
    if member.case_variant.is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection is not a direct record field",
            context.subject_kind, context.subject_name
        ))]);
    }
    let data_symbol = match compilation
        .type_reference_table
        .type_reference(projection.result_type)
    {
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
            Some(*base_symbol)
        }
        _ => None,
    };
    let field_rejoins = data_symbol
        .and_then(|symbol| {
            compilation
                .data_definitions()
                .iter()
                .find(|data| data.symbol == symbol)
        })
        .and_then(|data| {
            compilation.data_members(data).iter().find_map(|candidate| {
                let psi_typed_trees::data::DataMember::Field(field) = candidate else {
                    return None;
                };
                (field.name.as_str() == member.member.as_str()).then_some(field.symbol)
            })
        })
        .is_some_and(|field| field == projection.field);
    if !field_rejoins {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection certificate no longer rejoins its exact field",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(projection)
}

pub(crate) fn contract_parameter_field_symbol(
    compilation: &CheckedCompilation,
    parameter: &psi_typed_trees::signature::StateParameter,
    field_name: &str,
) -> Option<SymbolHandle> {
    use psi_typed_trees::types::TypeReferenceNode;

    let mut type_reference = parameter.type_reference;
    let data_symbol = loop {
        match compilation
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => break *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => break *base_symbol,
            _ => return None,
        }
    };
    compilation
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)
        .and_then(|data| {
            compilation.data_members(data).iter().find_map(|member| {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.name.as_str() == field_name).then_some(field.symbol)
            })
        })
}

pub(crate) fn project_contract_expression_with_substitutions(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    projection_substitutions: &[(SymbolHandle, psi_typed_trees::expression::ExpressionHandle)],
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    if depth >= 256 {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract expression exceeds the package-review depth limit",
            context.subject_kind, context.subject_name
        ))]);
    }
    let child = |expression| {
        project_contract_expression_with_substitutions(
            compilation,
            context,
            binders,
            expression,
            substitutions,
            projection_substitutions,
            checked_fact,
            depth + 1,
        )
    };
    match compilation.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Ok(PackageReviewContractExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => Ok(PackageReviewContractExpression::Integer(
            value.text().to_owned(),
        )),
        ExpressionNode::ArrayLiteral(values) => Ok(PackageReviewContractExpression::Array(
            compilation
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| child(*value))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        ExpressionNode::StructLiteral(literal) => {
            project_contract_constructor_expression(compilation, context, literal, &child)
        }
        ExpressionNode::Indexed(indexed) => Ok(PackageReviewContractExpression::Indexed {
            meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
            collection: Box::new(child(indexed.collection)?),
            index: Box::new(child(indexed.index)?),
        }),
        ExpressionNode::Range(range) => Ok(PackageReviewContractExpression::Range {
            start: range
                .start
                .is_valid()
                .then(|| child(range.start))
                .transpose()?
                .map(Box::new),
            end: range
                .end
                .is_valid()
                .then(|| child(range.end))
                .transpose()?
                .map(Box::new),
            end_inclusive: range.end_inclusive,
        }),
        ExpressionNode::String(value) => Ok(PackageReviewContractExpression::ByteSequence(
            value.to_vec(),
        )),
        ExpressionNode::ZeroValue(type_reference) => {
            Ok(PackageReviewContractExpression::ZeroValue(
                review_signature_type_identity_with_binders(
                    compilation,
                    *type_reference,
                    binders,
                    context.lifetime_binders,
                )?,
            ))
        }
        ExpressionNode::Binary(binary) => Ok(PackageReviewContractExpression::Binary {
            meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
            operator: project_contract_binary_operator(binary.operator),
            left: Box::new(child(binary.left)?),
            right: Box::new(child(binary.right)?),
        }),
        ExpressionNode::Unary(unary) => {
            if exact_checked_contract_operator_meaning(compilation, context, expression)?
                != PackageReviewContractOperatorMeaning::Builtin
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` unary contract operator is not one closed compiler-owned meaning",
                    context.subject_kind, context.subject_name
                ))]);
            }
            Ok(PackageReviewContractExpression::Unary {
                operator: project_contract_unary_operator(unary.operator),
                operand: Box::new(child(unary.operand)?),
            })
        }
        ExpressionNode::Call(call) => {
            let target =
                exact_checked_contract_call_target(compilation, context, expression, call)?;
            let static_parameter_kinds = match &target {
                PackageReviewContractCallTarget::Nominal(_) => {
                    contract_call_static_parameter_kinds(
                        compilation,
                        context,
                        call.target_symbol,
                        call.machine_arguments.len(),
                    )?
                }
                PackageReviewContractCallTarget::BuiltinFunction(_) => {
                    if !call.machine_arguments.is_empty() {
                        return Err(vec![Diagnostic::error(format!(
                            "reviewed {} `{}` supplies static arguments to a compiler-owned builtin function",
                            context.subject_kind, context.subject_name
                        ))]);
                    }
                    Vec::new()
                }
                PackageReviewContractCallTarget::ByteSequencePredicate(_) => {
                    if !call.machine_arguments.is_empty() {
                        return Err(vec![Diagnostic::error(format!(
                            "reviewed {} `{}` supplies static arguments to a compiler-owned byte-sequence predicate",
                            context.subject_kind, context.subject_name
                        ))]);
                    }
                    Vec::new()
                }
            };
            if call.quotient_operation.is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a quotient contract call not yet represented by package review",
                    context.subject_kind, context.subject_name
                ))]);
            }
            if !call.evidence_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a contract call with evidence arguments not yet represented by package review",
                    context.subject_kind, context.subject_name
                ))]);
            }
            // Call-site suspend/block acknowledgement is diagnostic audit
            // metadata, explicitly outside contract identity. Fact-position
            // calls have already been checked as total and pure.
            Ok(PackageReviewContractExpression::Call {
                receiver: call
                    .receiver
                    .is_valid()
                    .then(|| child(call.receiver))
                    .transpose()?
                    .map(Box::new),
                target,
                static_arguments: call
                    .machine_arguments
                    .iter()
                    .zip(static_parameter_kinds)
                    .map(|(argument, parameter_kind)| {
                        project_contract_static_argument(
                            compilation,
                            context,
                            binders,
                            argument,
                            parameter_kind,
                            0,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                arguments: compilation
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| child(*argument))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        ExpressionNode::Name(path) => project_contract_name_expression(
            compilation,
            context,
            binders,
            expression,
            path,
            substitutions,
            checked_fact,
        ),
        ExpressionNode::Member(member)
            if contract_member_has_exact_collection_length(compilation, expression) =>
        {
            require_exact_checked_contract_collection_length(
                compilation,
                context,
                expression,
                member,
            )?;
            Ok(PackageReviewContractExpression::CollectionLength {
                collection: Box::new(child(member.receiver)?),
            })
        }
        ExpressionNode::Member(member)
            if matches!(
                compilation.expression_table.expression(member.receiver),
                ExpressionNode::Name(path)
                    if projection_substitutions
                        .iter()
                        .any(|(symbol, actual)| {
                            *symbol == path.symbol
                                && matches!(
                                    compilation.expression_table.expression(*actual),
                                    ExpressionNode::Call(_)
                                )
                        })
            ) =>
        {
            let ExpressionNode::Name(path) =
                compilation.expression_table.expression(member.receiver)
            else {
                unreachable!()
            };
            let actual = projection_substitutions
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)
                .map(|(_, actual)| *actual)
                .expect("guarded projection substitution");
            let projection =
                exact_fact_call_projection(compilation, context, expression, actual, member)?;
            require_exact_checked_contract_nominal_member(
                compilation,
                context,
                expression,
                projection.field,
            )?;
            project_contract_member_expression(
                compilation,
                context,
                child(actual)?,
                projection.field,
                None,
            )
        }
        ExpressionNode::Member(member)
            if matches!(
                compilation.expression_table.expression(member.receiver),
                ExpressionNode::Name(path)
                    if substitutions.iter().any(|(symbol, _)| *symbol == path.symbol)
            ) =>
        {
            require_exact_checked_contract_nominal_member(
                compilation,
                context,
                expression,
                member.member_symbol,
            )?;
            project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                member.member_symbol,
                None,
            )
        }
        ExpressionNode::Member(member)
            if checked_fact.is_none()
                && matches!(
                    compilation.expression_table.expression(member.receiver),
                    ExpressionNode::Name(path)
                        if context.parameters.iter().any(|parameter| {
                            parameter.symbol == path.symbol && member.case_variant.is_none()
                        })
                ) =>
        {
            let ExpressionNode::Name(path) =
                compilation.expression_table.expression(member.receiver)
            else {
                unreachable!()
            };
            let parameter = context
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)
                .expect("guarded proposition parameter member");
            let field = contract_parameter_field_symbol(
                compilation,
                parameter,
                member.member.as_str(),
            )
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition parameter member does not resolve through its declared carrier",
                    context.subject_kind, context.subject_name
                    ))]
                })?;
            require_exact_checked_contract_nominal_member(compilation, context, expression, field)?;
            project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                field,
                None,
            )
        }
        ExpressionNode::Member(member)
            if matches!(
                compilation.expression_table.expression(member.receiver),
                ExpressionNode::Call(_)
            ) =>
        {
            let projection = exact_fact_call_projection(
                compilation,
                context,
                expression,
                member.receiver,
                member,
            )?;
            require_exact_checked_contract_nominal_member(
                compilation,
                context,
                expression,
                projection.field,
            )?;
            project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                projection.field,
                None,
            )
        }
        ExpressionNode::Member(_) => {
            let Some(checked_fact) = checked_fact else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a proposition-argument member expression without an exact checked place join",
                    context.subject_kind, context.subject_name
                ))]);
            };
            let Some((root_expression, mut source_members)) =
                contract_member_path_source(compilation, expression)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a computed member expression not yet represented by package review",
                    context.subject_kind, context.subject_name
                ))]);
            };
            let data_subject_root = context.data_symbol.is_some_and(|data_symbol| {
                is_data_subject_field_expression(compilation, data_symbol, root_expression)
            });
            let root = contract_member_path_root(compilation, context, root_expression)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contract member path has no exact semantic root",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            let receiver = if data_subject_root {
                let psi_typed_trees::expression::ExpressionNode::Name(path) =
                    compilation.expression_table.expression(root_expression)
                else {
                    unreachable!("guarded data-subject name root")
                };
                let [field_name] = compilation.expression_table.name_path_members(path.members)
                else {
                    unreachable!("guarded single data-subject field")
                };
                source_members.insert(0, field_name.clone());
                PackageReviewContractExpression::DomainSubject
            } else {
                child(root_expression)?
            };
            let member_path = checked_contract_member_path(
                compilation,
                context,
                checked_fact,
                expression,
                root,
                &source_members,
            )?;
            let selected_member = member_path.last().ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed {} `{}` contract member path has no checked member coordinate",
                    context.subject_kind, context.subject_name
                ))]
            })?;
            require_exact_checked_contract_nominal_member(
                compilation,
                context,
                expression,
                selected_member.1,
            )?;
            member_path
                .into_iter()
                .try_fold(receiver, |receiver, (case_variant, member_symbol)| {
                    project_contract_member_expression(
                        compilation,
                        context,
                        receiver,
                        member_symbol,
                        case_variant,
                    )
                })
        }
        ExpressionNode::Cast(cast) => {
            let semantic_domain = if cast.semantic_domain_symbol.is_valid() {
                let domain = compilation
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == cast.semantic_domain_symbol)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "reviewed {} `{}` cast refers to an unresolved semantic domain",
                            context.subject_kind, context.subject_name
                        ))]
                    })?;
                let identity = nominal_identity(compilation, domain.symbol)?;
                let reviewed_package = compilation.package_identity().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review requires package-aware checked compilation",
                    )]
                })?;
                if reviewed_package_owns(&identity, reviewed_package)? && !domain.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed {} `{}` exposes non-public semantic domain `{}` in a cast",
                        context.subject_kind, context.subject_name, domain.name
                    ))]);
                }
                Some(identity)
            } else {
                None
            };
            Ok(PackageReviewContractExpression::Cast {
                value: Box::new(child(cast.value)?),
                target: review_type_identity_with_binders(compilation, cast.target_type, binders)?,
                arithmetic_domain: match cast.domain {
                    psi_numerics::arithmetic::ArithmeticDomain::Exact => {
                        PackageReviewArithmeticDomain::Exact
                    }
                    psi_numerics::arithmetic::ArithmeticDomain::Wrapping => {
                        PackageReviewArithmeticDomain::Wrapping
                    }
                    psi_numerics::arithmetic::ArithmeticDomain::Saturating => {
                        PackageReviewArithmeticDomain::Saturating
                    }
                    psi_numerics::arithmetic::ArithmeticDomain::Trapping => {
                        PackageReviewArithmeticDomain::Trapping
                    }
                },
                semantic_domain,
                semantic_domain_arguments: compilation
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments)
                    .iter()
                    .map(|argument| {
                        review_type_identity_with_binders(compilation, *argument, binders)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                form: match cast.form {
                    psi_language_core::cast_form::CastForm::Value => PackageReviewCastForm::Value,
                    psi_language_core::cast_form::CastForm::RecastShared => {
                        PackageReviewCastForm::RecastShared
                    }
                    psi_language_core::cast_form::CastForm::RecastMutable => {
                        PackageReviewCastForm::RecastMutable
                    }
                },
            })
        }
        _ => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` uses a contract expression form not yet represented by package review",
            context.subject_kind, context.subject_name
        ))]),
    }
}

pub(crate) fn project_contract_constructor_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    literal: &psi_typed_trees::expression::TableStructLiteral,
    child: &impl Fn(
        psi_typed_trees::expression::ExpressionHandle,
    ) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    use psi_typed_trees::data::DataMember;

    let matching_data = compilation
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == literal.type_symbol)
        .collect::<Vec<_>>();
    let [data] = matching_data.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` constructor resolves its data symbol to {} declarations; expected one",
            context.subject_kind,
            context.subject_name,
            matching_data.len()
        ))]);
    };
    let data_identity = nominal_identity(compilation, data.symbol)?;
    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    if reviewed_package_owns(&data_identity, reviewed_package)? && !data.is_public {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` exposes non-public data `{}` through a constructor",
            context.subject_kind, context.subject_name, data.name
        ))]);
    }

    let selected_variant = match literal.case_symbol {
        Some(case_symbol) => {
            let matching = compilation
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) if variant.symbol == case_symbol => Some(variant),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [variant] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` constructor case resolves to {} variants in `{}`; expected one",
                    context.subject_kind,
                    context.subject_name,
                    matching.len(),
                    data.name
                ))]);
            };
            Some(*variant)
        }
        None => None,
    };
    if literal.case_name.is_some() != selected_variant.is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` constructor has inconsistent checked case identity",
            context.subject_kind, context.subject_name
        ))]);
    }

    let mut allowed_fields = compilation
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field.symbol),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    if let Some(variant) = selected_variant {
        allowed_fields.extend(
            compilation
                .data_payload_fields(variant)
                .iter()
                .map(|field| field.symbol),
        );
    }

    let mut fields = compilation
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .map(|field| {
            if !field.field_symbol.is_valid()
                || !allowed_fields.contains(&field.field_symbol)
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` constructor field `{}` does not rejoin its selected data/case",
                    context.subject_kind, context.subject_name, field.name
                ))]);
            }
            Ok(PackageReviewConstructorField {
                field: nominal_identity(compilation, field.field_symbol)?,
                value: child(field.value)?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    fields.sort();
    if fields.windows(2).any(|pair| pair[0].field == pair[1].field) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` constructor repeats one exact field",
            context.subject_kind, context.subject_name
        ))]);
    }

    Ok(PackageReviewContractExpression::Constructor {
        data: data_identity,
        case: selected_variant
            .map(|variant| nominal_identity(compilation, variant.symbol))
            .transpose()?,
        fields,
    })
}

pub(crate) fn exact_checked_contract_operator_meaning(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Result<PackageReviewContractOperatorMeaning, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionIntrinsic,
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator has {} exact checked selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        ) => Ok(PackageReviewContractOperatorMeaning::Builtin),
        AuthoredDeclarationSelectionTarget::Resolved(target) => {
            let symbol = target.selected_symbol();
            let declaration = psi_typed_trees::operator::declaration_by_symbol(compilation, symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contract selected an operator without one retained declaration",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            Ok(PackageReviewContractOperatorMeaning::Declared(
                project_operator_coordinate(compilation, declaration)?,
            ))
        }
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator selected a non-operator intrinsic",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::LateBound(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator remains late-bound after checked lowering",
            context.subject_kind, context.subject_name
        ))]),
    }
}

pub(crate) fn exact_checked_contract_call_target(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Result<PackageReviewContractCallTarget, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call has {} exact checked call-selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == call.target_symbol =>
        {
            if let Some(function) = compilation
                .typed
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
            {
                Ok(PackageReviewContractCallTarget::BuiltinFunction(function))
            } else {
                Ok(PackageReviewContractCallTarget::Nominal(nominal_identity(
                    compilation,
                    call.target_symbol,
                )?))
            }
        }
        AuthoredDeclarationSelectionTarget::Resolved(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call target disagrees with its exact checked call-selection row",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::Intrinsic(
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(
                predicate,
            ),
        ) if !call.target_symbol.is_valid()
            && !call.receiver.is_valid()
            && psi_language_semantics::byte_predicates::ByteSequencePredicate::from_name(
                call.target.as_str(),
            ) == Some(predicate) => Ok(PackageReviewContractCallTarget::ByteSequencePredicate(
                match predicate {
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::ValidUtf8 => {
                        PackageReviewByteSequencePredicate::ValidUtf8
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::NoNul => {
                        PackageReviewByteSequencePredicate::NoNul
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::AsciiOnly => {
                        PackageReviewByteSequencePredicate::AsciiOnly
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::NonEmpty => {
                        PackageReviewByteSequencePredicate::NonEmpty
                    }
                },
            )),
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract-call intrinsic identity disagrees with its exact checked call-selection row or is not yet represented by package review",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::LateBound(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` retains an unresolved contract call selection",
            context.subject_kind, context.subject_name
        ))]),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractCallStaticParameterKind {
    Type,
    Const,
    Machine,
    Proposition,
}

pub(crate) fn contract_call_static_parameter_kind(
    parameter: &psi_typed_trees::data::TypeParameter,
) -> ContractCallStaticParameterKind {
    match parameter.kind {
        psi_typed_trees::data::TypeParameterKind::Type => ContractCallStaticParameterKind::Type,
        psi_typed_trees::data::TypeParameterKind::Const { .. } => {
            ContractCallStaticParameterKind::Const
        }
        psi_typed_trees::data::TypeParameterKind::Machine { .. } => {
            ContractCallStaticParameterKind::Machine
        }
        psi_typed_trees::data::TypeParameterKind::Proposition { .. } => {
            ContractCallStaticParameterKind::Proposition
        }
    }
}

pub(crate) fn contract_call_static_parameter_kinds(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    target: SymbolHandle,
    supplied_count: usize,
) -> Result<Vec<ContractCallStaticParameterKind>, Vec<Diagnostic>> {
    let project = |parameters: &[psi_typed_trees::data::TypeParameter]| {
        parameters
            .iter()
            .map(contract_call_static_parameter_kind)
            .collect::<Vec<_>>()
    };
    let mut candidates = compilation
        .machines()
        .iter()
        .filter(|machine| {
            compilation
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == target)
        })
        .map(|machine| project(compilation.machine_type_parameters(machine)))
        .collect::<Vec<_>>();
    if let Some((_, signature)) = compilation.machine_parameter_signature(target) {
        candidates.push(project(
            compilation.state_signature_type_parameters(signature),
        ));
    }
    candidates.extend(compilation.traits().iter().flat_map(|definition| {
        compilation
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == target)
            .map(|signature| project(compilation.state_signature_type_parameters(signature)))
    }));
    let [parameter_kinds] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call target rejoins {} static telescopes; expected exactly one",
            context.subject_kind,
            context.subject_name,
            candidates.len()
        ))]);
    };
    if parameter_kinds.len() != supplied_count {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call supplies {supplied_count} static arguments for a checked telescope of {} parameters",
            context.subject_kind,
            context.subject_name,
            parameter_kinds.len()
        ))]);
    }
    Ok(parameter_kinds.clone())
}

pub(crate) fn project_contract_static_argument(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    project_static_argument(
        compilation,
        context.subject_kind,
        context.subject_name,
        binders,
        context.lifetime_binders,
        argument,
        parameter_kind,
        depth,
    )
}

pub(crate) fn project_static_argument(
    compilation: &CheckedCompilation,
    subject_kind: &str,
    subject_name: &str,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed {subject_kind} `{subject_name}` uses a static argument {reason}",
        ))]
    };
    if depth >= 64 {
        return Err(rejected(
            "whose nested application exceeds the package-review depth limit",
        ));
    }
    if argument.evidence_projection.is_some() {
        return Err(rejected(
            "from an evidence projection not yet represented by package review",
        ));
    }
    if parameter_kind == ContractCallStaticParameterKind::Proposition {
        return Err(rejected(
            "for a proposition parameter not yet represented by package review",
        ));
    }
    if let Some(application) = argument.application.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Type
            || !argument.symbol.is_valid()
            || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::Data
        {
            return Err(rejected(
                "with a non-data nested static application not yet represented by package review",
            ));
        }
        let definitions = compilation
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == argument.symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(rejected(
                "whose generic data base does not rejoin exactly one checked declaration",
            ));
        };
        if definition.lifetime_parameters.len() != application.lifetime_arguments.len() {
            return Err(rejected(
                "whose lifetime argument count differs from its checked data declaration",
            ));
        }
        let parameters = compilation.data_type_parameters(definition);
        if parameters.len() != application.arguments.len() {
            return Err(rejected(
                "whose generic data argument count differs from its checked telescope",
            ));
        }
        let base = compilation
            .package_qualified_nominal_type_identity_with_toolchain_sources(
                argument.symbol,
                compilation.exact_toolchain_sources(),
            )
            .ok_or_else(missing_exact_toolchain_type_owner)?;
        let arguments = application
            .arguments
            .iter()
            .zip(parameters)
            .map(|(argument, parameter)| {
                project_static_argument(
                    compilation,
                    subject_kind,
                    subject_name,
                    binders,
                    lifetime_binders,
                    argument,
                    contract_call_static_parameter_kind(parameter),
                    depth + 1,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let lifetime_arguments = application
            .lifetime_arguments
            .iter()
            .map(|lifetime| {
                lifetime_binder_ordinal(lifetime, lifetime_binders, "contract-call nested type")
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(PackageReviewContractStaticArgument::GenericType {
            base: PackageReviewTypeIdentity {
                canonical: base.into_string(),
            },
            lifetime_arguments,
            arguments,
        });
    }
    if let Some(literal) = argument.const_literal.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Const {
            return Err(rejected(
                "whose category differs from its checked telescope slot",
            ));
        }
        return Ok(PackageReviewContractStaticArgument::ConstInteger(
            literal.text().to_owned(),
        ));
    }
    if let Some(position) = binders
        .iter()
        .position(|(symbol, _)| *symbol == argument.symbol)
    {
        let position = portable_parameter_position(position)?;
        return match compilation.typed.symbols.get(argument.symbol).kind {
            psi_symbols::SymbolKind::MachineParameter
                if parameter_kind == ContractCallStaticParameterKind::Machine =>
            {
                Ok(PackageReviewContractStaticArgument::GenericMachineBinder(
                    position,
                ))
            }
            psi_symbols::SymbolKind::TypeParameter => {
                let matching = compilation
                    .typed
                    .data_type_parameters
                    .iter()
                    .map(|(_, parameter)| parameter)
                    .filter(|parameter| parameter.symbol == argument.symbol)
                    .collect::<Vec<_>>();
                let [parameter] = matching.as_slice() else {
                    return Err(rejected(
                        "that does not rejoin exactly one checked caller parameter",
                    ));
                };
                match (&parameter.kind, parameter_kind) {
                    (
                        psi_typed_trees::data::TypeParameterKind::Type,
                        ContractCallStaticParameterKind::Type,
                    ) => Ok(PackageReviewContractStaticArgument::GenericTypeBinder(
                        position,
                    )),
                    (
                        psi_typed_trees::data::TypeParameterKind::Const { .. },
                        ContractCallStaticParameterKind::Const,
                    ) => Ok(PackageReviewContractStaticArgument::GenericConstBinder(
                        position,
                    )),
                    _ => Err(rejected(
                        "whose category differs from its checked caller and callee telescope slots",
                    )),
                }
            }
            _ => Err(rejected(
                "whose category differs from its checked caller and callee telescope slots",
            )),
        };
    }
    if parameter_kind == ContractCallStaticParameterKind::Type {
        if !argument.symbol.is_valid()
            || !matches!(
                compilation.typed.symbols.get(argument.symbol).kind,
                psi_symbols::SymbolKind::BuiltinType | psi_symbols::SymbolKind::Data
            )
        {
            return Err(rejected(
                "whose category differs from its checked type slot",
            ));
        }
        let identity = compilation
            .package_qualified_nominal_type_identity_with_toolchain_sources(
                argument.symbol,
                compilation.exact_toolchain_sources(),
            )
            .ok_or_else(missing_exact_toolchain_type_owner)?;
        return Ok(PackageReviewContractStaticArgument::Type(
            PackageReviewTypeIdentity {
                canonical: identity.into_string(),
            },
        ));
    }
    if parameter_kind == ContractCallStaticParameterKind::Const {
        return Err(rejected(
            "from a forwarded or symbolic const not yet represented by package review",
        ));
    }
    if !argument.symbol.is_valid()
        || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::State
    {
        return Err(rejected(
            "whose category differs from its checked machine slot",
        ));
    }
    let matching_states = compilation
        .machines()
        .iter()
        .filter_map(|machine| compilation.machine_states(machine).first())
        .filter(|entry| entry.symbol == argument.symbol)
        .count();
    if matching_states != 1 {
        return Err(rejected(
            "that does not rejoin exactly one checked concrete machine entry",
        ));
    }
    Ok(PackageReviewContractStaticArgument::ConcreteMachine(
        nominal_identity(compilation, argument.symbol)?,
    ))
}

pub(crate) fn project_contract_name_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    let members = compilation.expression_table.name_path_members(path.members);
    let data_binder_position = context.data_symbol.and_then(|data_symbol| {
        data_subject_binder_position(compilation, data_symbol, expression, binders)
    });
    if data_binder_position.is_none()
        && context.data_symbol.is_some_and(|data_symbol| {
            is_data_subject_field_expression(compilation, data_symbol, expression)
        })
    {
        let Some(checked_fact) = checked_fact else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` uses a data-invariant field without an exact checked place join",
                context.subject_kind, context.subject_name
            ))]);
        };
        return checked_contract_member_path(
            compilation,
            context,
            checked_fact,
            expression,
            psi_facts::PlaceRoot::Symbol(context.data_symbol.expect("guarded data subject")),
            members,
        )?
        .into_iter()
        .try_fold(
            PackageReviewContractExpression::DomainSubject,
            |receiver, (case_variant, member_symbol)| {
                project_contract_member_expression(
                    compilation,
                    context,
                    receiver,
                    member_symbol,
                    case_variant,
                )
            },
        );
    }
    let root_symbol = path.head_symbol;
    let root_name = members.first();
    let substitution_root = substitutions
        .iter()
        .rev()
        .find_map(|(symbol, substitution)| {
            (*symbol == root_symbol || (members.len() == 1 && *symbol == path.symbol))
                .then(|| substitution.clone())
        });
    let parameter_position = context.parameters.iter().position(|parameter| {
        if root_symbol.is_valid() {
            parameter.symbol == root_symbol
        } else {
            root_name.is_some_and(|name| name == &parameter.name)
        }
    });
    let is_domain_subject =
        context.domain_symbol.is_some() && root_name.is_some_and(|name| name.as_str() == "self");
    let binder_position = binders
        .iter()
        .position(|(symbol, _)| *symbol == root_symbol)
        .or(data_binder_position);
    let root = if let Some(substitution) = substitution_root {
        Some(substitution)
    } else if is_domain_subject {
        Some(PackageReviewContractExpression::DomainSubject)
    } else if let Some(position) = parameter_position {
        Some(PackageReviewContractExpression::Parameter(
            portable_parameter_position(position)?,
        ))
    } else if root_name.is_some_and(|name| name.as_str() == "result") {
        Some(PackageReviewContractExpression::Result)
    } else if let Some(position) = binder_position {
        Some(PackageReviewContractExpression::GenericBinder(
            portable_parameter_position(position)?,
        ))
    } else {
        None
    };

    let Some(projected) = root else {
        if root_symbol.is_valid()
            && root_name.is_some_and(|name| {
                context
                    .parameters
                    .iter()
                    .any(|parameter| name == &parameter.name)
            })
        {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract parameter spelling does not match its exact resolved symbol",
                context.subject_kind, context.subject_name
            ))]);
        }
        if !path.symbol.is_valid() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract contains an unresolved name expression",
                context.subject_kind, context.subject_name
            ))]);
        }
        return nominal_identity(compilation, path.symbol)
            .map(PackageReviewContractExpression::Nominal);
    };
    if members.len() == 1 {
        return Ok(projected);
    }
    let Some(checked_fact) = checked_fact else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` uses a proposition-argument name-path member without an exact checked place join",
            context.subject_kind, context.subject_name
        ))]);
    };
    let semantic_root = is_domain_subject
        .then_some(psi_facts::PlaceRoot::Expression(expression))
        .or_else(|| {
            parameter_position
                .map(|position| psi_facts::PlaceRoot::Symbol(context.parameters[position].symbol))
        })
        .or_else(|| {
            root_symbol
                .is_valid()
                .then_some(psi_facts::PlaceRoot::Symbol(root_symbol))
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract name-path member has no exact semantic root",
                context.subject_kind, context.subject_name
            ))]
        })?;
    checked_contract_member_path(
        compilation,
        context,
        checked_fact,
        expression,
        semantic_root,
        &members[1..],
    )?
    .into_iter()
    .try_fold(projected, |receiver, (case_variant, member_symbol)| {
        project_contract_member_expression(
            compilation,
            context,
            receiver,
            member_symbol,
            case_variant,
        )
    })
}

pub(crate) fn contract_member_has_exact_collection_length(
    compilation: &CheckedCompilation,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .any(|selection| {
            selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess
                && selection.target()
                    == AuthoredDeclarationSelectionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                    )
        })
}

pub(crate) fn require_exact_checked_contract_nominal_member(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    expected_member: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member has {} exact checked member-selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member does not retain one exact declaration target",
            context.subject_kind, context.subject_name
        ))]);
    };
    if !expected_member.is_valid() || target.selected_symbol() != expected_member {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` nominal member disagrees with its exact checked member-selection row",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(())
}

pub(crate) fn require_exact_checked_contract_collection_length(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Result<(), Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionIntrinsic,
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` collection-length projection has {} exact checked member-selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` collection-length projection is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    if member.member.as_str() != "len"
        || member.member_symbol.is_valid()
        || member.case_variant.is_some()
        || selection.target()
            != AuthoredDeclarationSelectionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::CollectionLength,
            )
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` collection-length syntax disagrees with its exact checked intrinsic selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(())
}

pub(crate) fn project_contract_member_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    receiver: PackageReviewContractExpression,
    member_symbol: SymbolHandle,
    case_variant_symbol: Option<SymbolHandle>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    if !member_symbol.is_valid() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract contains an unresolved member expression",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(PackageReviewContractExpression::Member {
        receiver: Box::new(receiver),
        member: nominal_identity(compilation, member_symbol)?,
        case_variant: case_variant_symbol
            .map(|symbol| nominal_identity(compilation, symbol))
            .transpose()?,
    })
}

pub(crate) fn contract_member_path_source(
    compilation: &CheckedCompilation,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<(
    psi_typed_trees::expression::ExpressionHandle,
    Vec<psi_typed_trees::name::Identifier>,
)> {
    use psi_typed_trees::expression::ExpressionNode;

    match compilation.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let (root, mut members) = contract_member_path_source(compilation, member.receiver)?;
            members.push(member.member.clone());
            Some((root, members))
        }
        ExpressionNode::Name(path)
            if compilation
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1 =>
        {
            Some((expression, Vec::new()))
        }
        _ => None,
    }
}

pub(crate) fn contract_member_path_root(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_facts::PlaceRoot> {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return None;
    };
    if context.data_symbol.is_some_and(|data_symbol| {
        is_data_subject_field_expression(compilation, data_symbol, expression)
    }) {
        return context.data_symbol.map(psi_facts::PlaceRoot::Symbol);
    }
    let resolved = path
        .head_symbol
        .is_valid()
        .then_some(path.head_symbol)
        .or_else(|| path.symbol.is_valid().then_some(path.symbol));
    if let Some(symbol) = resolved {
        return Some(psi_facts::PlaceRoot::Symbol(symbol));
    }
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return None;
    };
    if context.domain_symbol.is_some() && name.as_str() == "self" {
        return Some(psi_facts::PlaceRoot::Expression(expression));
    }
    context
        .parameters
        .iter()
        .find(|parameter| parameter.name == *name)
        .map(|parameter| psi_facts::PlaceRoot::Symbol(parameter.symbol))
}

pub(crate) fn is_data_subject_field_expression(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return false;
    };
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return false;
    };
    let [member_symbol] = compilation
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    else {
        return false;
    };
    if !path.head_symbol.is_valid()
        || path.symbol != path.head_symbol
        || *member_symbol != path.head_symbol
    {
        return false;
    }
    let selected = path.head_symbol;
    let Some(data) = compilation
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == data_symbol)
    else {
        return false;
    };
    compilation.data_members(data).iter().any(|member| {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            return false;
        };
        field.symbol == selected && field.name == *name
    })
}

pub(crate) fn data_subject_binder_position(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<usize> {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return None;
    };
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return None;
    };
    let [member_symbol] = compilation
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    else {
        return None;
    };
    if !path.head_symbol.is_valid()
        || path.symbol != path.head_symbol
        || *member_symbol != path.head_symbol
    {
        return None;
    }
    let selected = path.head_symbol;
    let data = compilation
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == data_symbol)?;
    let parameter = compilation
        .data_type_parameters(data)
        .iter()
        .find(|parameter| parameter.symbol == selected && parameter.name == *name)?;
    binders
        .iter()
        .position(|(symbol, _)| *symbol == parameter.symbol)
}

pub(crate) fn checked_contract_member_path(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    checked_fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    root: psi_facts::PlaceRoot,
    source_members: &[psi_typed_trees::name::Identifier],
) -> Result<Vec<(Option<SymbolHandle>, SymbolHandle)>, Vec<Diagnostic>> {
    use psi_facts::{FactPayload, FactPlace};

    if let Some(domain_symbol) = context.domain_symbol {
        let mut candidates = Vec::new();
        for (_, record) in compilation
            .facts
            .semantic
            .domain_definition_facts
            .iter()
            .filter(|(_, record)| {
                record.domain_symbol == domain_symbol && record.fact == checked_fact
            })
        {
            for dependency in record
                .dependencies
                .iter()
                .filter(|dependency| dependency.expression == expression)
            {
                let Some((_, place)) = compilation
                    .facts
                    .semantic
                    .places
                    .iter()
                    .find(|(handle, _)| *handle == dependency.place)
                else {
                    continue;
                };
                if place.root != root {
                    continue;
                }
                if let Some(selected) =
                    checked_member_segments(compilation, place.segments, source_members)
                {
                    candidates.push(selected);
                }
            }
        }
        let [selected] = candidates.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract member path resolves to {} exact checked dependency records; expected one",
                context.subject_kind,
                context.subject_name,
                candidates.len()
            ))]);
        };
        return Ok(selected.clone());
    }

    if let Some(data_symbol) = context.data_symbol {
        let mut candidates = Vec::new();
        for (_, record) in compilation
            .facts
            .semantic
            .data_definition_facts
            .iter()
            .filter(|(_, record)| record.data_symbol == data_symbol && record.fact == checked_fact)
        {
            for dependency in record
                .dependencies
                .iter()
                .filter(|dependency| dependency.expression == expression)
            {
                let Some((_, place)) = compilation
                    .facts
                    .semantic
                    .places
                    .iter()
                    .find(|(handle, _)| *handle == dependency.place)
                else {
                    continue;
                };
                if place.root != root {
                    continue;
                }
                if let Some(selected) =
                    checked_member_segments(compilation, place.segments, source_members)
                {
                    candidates.push(selected);
                }
            }
        }
        let [selected] = candidates.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract member path resolves to {} exact checked dependency records; expected one",
                context.subject_kind,
                context.subject_name,
                candidates.len()
            ))]);
        };
        return Ok(selected.clone());
    }

    let mut candidates = Vec::new();
    for (_, semantic_fact) in compilation.facts.semantic.facts.iter() {
        let contract_fact_matches = matches!(
            semantic_fact.payload,
            FactPayload::ContractBooleanExpression { fact, .. }
                | FactPayload::ContractDomainMembership { fact, .. }
                if fact == checked_fact
        );
        if semantic_fact.point != context.point || !contract_fact_matches {
            continue;
        }
        let FactPlace::Place(place_handle) = semantic_fact.place else {
            continue;
        };
        let place = compilation.facts.semantic.places.get(place_handle);
        if place.root != root {
            continue;
        }
        let Some(selected) = checked_member_segments(compilation, place.segments, source_members)
        else {
            continue;
        };
        if !candidates.contains(&selected) {
            candidates.push(selected);
        }
    }
    let [selected] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract member path resolves to {} exact checked place rows; expected one",
            context.subject_kind,
            context.subject_name,
            candidates.len()
        ))]);
    };
    Ok(selected.clone())
}

pub(crate) fn checked_member_segments(
    compilation: &CheckedCompilation,
    segments: psi_arena::HandleSpan<psi_facts::PlaceSegment>,
    source_members: &[psi_typed_trees::name::Identifier],
) -> Option<Vec<(Option<SymbolHandle>, SymbolHandle)>> {
    use psi_facts::PlaceSegment;

    let mut selected = Vec::new();
    let mut pending_case = None;
    for segment in compilation
        .facts
        .semantic
        .place_segments
        .span_or_empty(segments)
    {
        match *segment {
            PlaceSegment::Case { variant } if pending_case.is_none() => {
                pending_case = Some(variant);
            }
            PlaceSegment::Field { symbol } if symbol.is_valid() => {
                selected.push((pending_case.take(), symbol));
            }
            _ => return None,
        }
    }
    if pending_case.is_some() || selected.len() != source_members.len() {
        return None;
    }
    if selected
        .iter()
        .zip(source_members)
        .any(|((_, symbol), name)| compilation.symbols.name(*symbol) != name.as_str())
    {
        return None;
    }
    Some(selected)
}

pub(crate) fn portable_parameter_position(position: usize) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(position).map_err(|_| {
        vec![Diagnostic::error(
            "package review contract parameter ordinal exceeds the portable identity range",
        )]
    })
}

pub(crate) const fn project_contract_binary_operator(
    operator: psi_typed_trees::expression::BinaryOperator,
) -> PackageReviewContractBinaryOperator {
    use psi_typed_trees::expression::BinaryOperator;
    match operator {
        BinaryOperator::Add => PackageReviewContractBinaryOperator::Add,
        BinaryOperator::And => PackageReviewContractBinaryOperator::And,
        BinaryOperator::BitwiseAnd => PackageReviewContractBinaryOperator::BitwiseAnd,
        BinaryOperator::BitwiseOr => PackageReviewContractBinaryOperator::BitwiseOr,
        BinaryOperator::BitwiseXor => PackageReviewContractBinaryOperator::BitwiseXor,
        BinaryOperator::Divide => PackageReviewContractBinaryOperator::Divide,
        BinaryOperator::Equal => PackageReviewContractBinaryOperator::Equal,
        BinaryOperator::Greater => PackageReviewContractBinaryOperator::Greater,
        BinaryOperator::GreaterOrEqual => PackageReviewContractBinaryOperator::GreaterOrEqual,
        BinaryOperator::Less => PackageReviewContractBinaryOperator::Less,
        BinaryOperator::LessOrEqual => PackageReviewContractBinaryOperator::LessOrEqual,
        BinaryOperator::Modulo => PackageReviewContractBinaryOperator::Modulo,
        BinaryOperator::Multiply => PackageReviewContractBinaryOperator::Multiply,
        BinaryOperator::NotEqual => PackageReviewContractBinaryOperator::NotEqual,
        BinaryOperator::Or => PackageReviewContractBinaryOperator::Or,
        BinaryOperator::ShiftLeft => PackageReviewContractBinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight => PackageReviewContractBinaryOperator::ShiftRight,
        BinaryOperator::Subtract => PackageReviewContractBinaryOperator::Subtract,
    }
}

pub(crate) const fn project_contract_unary_operator(
    operator: psi_typed_trees::expression::UnaryOperator,
) -> PackageReviewContractUnaryOperator {
    match operator {
        psi_typed_trees::expression::UnaryOperator::BitwiseNot => {
            PackageReviewContractUnaryOperator::BitwiseNot
        }
        psi_typed_trees::expression::UnaryOperator::LogicalNot => {
            PackageReviewContractUnaryOperator::LogicalNot
        }
    }
}

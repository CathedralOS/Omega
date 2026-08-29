use super::calls::{
    contract_call_value_receiver, exact_checked_contract_call_target, exact_fact_call_projection,
    require_exact_contract_call_reference_arguments, resolved_contract_call_symbol,
};
use super::constructors::project_contract_constructor_expression;
use super::members::{
    checked_contract_member_path, contract_member_has_exact_collection_length,
    contract_member_path_root, contract_member_path_source, exact_checked_contract_nominal_member,
    is_data_subject_field_expression, project_contract_member_expression,
    require_exact_checked_contract_collection_length,
    require_exact_checked_contract_nominal_member,
};
use super::names::{contract_parameter_field_symbol, project_contract_name_expression};
use super::operators::{
    exact_checked_contract_operator_meaning, project_contract_binary_operator,
    project_contract_unary_operator,
};
use super::static_arguments::{
    contract_call_static_parameter_kinds, project_contract_static_argument,
};
use crate::evidence::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering, PackageReviewCastForm,
    PackageReviewContractCallTarget, PackageReviewContractExpression,
    PackageReviewContractOperatorMeaning, PackageReviewFloatLiteral, PackageReviewReferenceAccess,
};
use crate::projection::contracts::checked::facts::ContractProjectionContext;
use crate::projection::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::projection::semantics::types::{
    review_signature_type_identity_with_binders, review_type_identity_with_binders,
};
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
        ExpressionNode::Float(value) => Ok(PackageReviewContractExpression::Float(
            match value.landing() {
                Some(psi_numerics::literals::FloatFormat::F32) => {
                    PackageReviewFloatLiteral::F32(value.f32_bits())
                }
                Some(psi_numerics::literals::FloatFormat::F64) => {
                    PackageReviewFloatLiteral::F64(value.landed_f64().to_bits())
                }
                None => {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contains a float literal without an exact checked width landing",
                        context.subject_kind, context.subject_name
                    ))]);
                }
            },
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
        ExpressionNode::Atomic(atomic) => {
            if !atomic.value.is_valid() || atomic.result.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` contains an inconsistent atomic-load expression",
                    context.subject_kind, context.subject_name
                ))]);
            }
            let ordering = match atomic.ordering {
                psi_language_core::atomic::AtomicOrderingPlan::Load(
                    psi_language_core::atomic::MemoryOrdering::NoOrdering,
                ) => PackageReviewAtomicLoadOrdering::NoOrdering,
                psi_language_core::atomic::AtomicOrderingPlan::Load(
                    psi_language_core::atomic::MemoryOrdering::Receive,
                ) => PackageReviewAtomicLoadOrdering::Receive,
                psi_language_core::atomic::AtomicOrderingPlan::Load(
                    psi_language_core::atomic::MemoryOrdering::GlobalOrder,
                ) => PackageReviewAtomicLoadOrdering::GlobalOrder,
                psi_language_core::atomic::AtomicOrderingPlan::Load(_) => {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contains an atomic load with an invalid ordering",
                        context.subject_kind, context.subject_name
                    ))]);
                }
                _ => {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contains a mutation-bearing atomic contract expression",
                        context.subject_kind, context.subject_name
                    ))]);
                }
            };
            Ok(PackageReviewContractExpression::AtomicLoad {
                value: Box::new(child(atomic.value)?),
                ordering,
            })
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
        ExpressionNode::Borrow(reference) => Ok(PackageReviewContractExpression::Reference {
            access: match reference.access {
                psi_language_core::ReferenceAccess::Shared => PackageReviewReferenceAccess::Shared,
                psi_language_core::ReferenceAccess::Mutable => {
                    PackageReviewReferenceAccess::Mutable
                }
                psi_language_core::ReferenceAccess::WriteOnly => {
                    PackageReviewReferenceAccess::WriteOnly
                }
            },
            target: Box::new(child(reference.target)?),
        }),
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
            let resolved_symbol = resolved_contract_call_symbol(compilation, call);
            let static_parameter_kinds = match &target {
                PackageReviewContractCallTarget::Nominal(_) => {
                    let target_symbol = resolved_symbol.ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "reviewed {} `{}` contract call has no exact resolved target symbol",
                            context.subject_kind, context.subject_name
                        ))]
                    })?;
                    require_exact_contract_call_reference_arguments(
                        compilation,
                        context,
                        target_symbol,
                        call,
                    )?;
                    contract_call_static_parameter_kinds(
                        compilation,
                        context,
                        target_symbol,
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
                receiver: contract_call_value_receiver(compilation, call)
                    .map(child)
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
        ExpressionNode::Member(member) => {
            let Some(checked_fact) = checked_fact else {
                let selected =
                    exact_checked_contract_nominal_member(compilation, context, expression)?;
                if member.member_symbol.is_valid() && member.member_symbol != selected {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed {} `{}` computed member disagrees with its exact checked declaration selection",
                        context.subject_kind, context.subject_name
                    ))]);
                }
                let selected_parent = compilation.symbols.get(selected).parent;
                let case_variant = (compilation.symbols.get(selected_parent).kind
                    == psi_symbols::SymbolKind::Variant)
                    .then_some(selected_parent);
                return project_contract_member_expression(
                    compilation,
                    context,
                    child(member.receiver)?,
                    selected,
                    case_variant,
                );
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
    }
}

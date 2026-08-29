use super::members::{
    checked_contract_member_path, data_subject_binder_position, is_data_subject_field_expression,
    project_contract_member_expression,
};
use crate::evidence::PackageReviewContractExpression;
use crate::projection::checked_semantics::declarations::nominal_identity;
use crate::projection::contracts::checked::facts::ContractProjectionContext;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

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

pub(crate) fn portable_parameter_position(position: usize) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(position).map_err(|_| {
        vec![Diagnostic::error(
            "package review contract parameter ordinal exceeds the portable identity range",
        )]
    })
}

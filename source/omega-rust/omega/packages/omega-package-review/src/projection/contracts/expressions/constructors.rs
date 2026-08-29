use crate::evidence::{PackageReviewConstructorField, PackageReviewContractExpression};
use crate::projection::checked_semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::projection::contracts::checked::facts::ContractProjectionContext;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

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

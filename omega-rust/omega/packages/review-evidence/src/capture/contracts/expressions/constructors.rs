use crate::capture::PackageReviewInput;
use crate::capture::contracts::facts::{ContractProjectionContext, reviewed_package};
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::{PackageReviewConstructorField, PackageReviewContractExpression};
use diagnostics::Diagnostic;

pub(crate) fn project_contract_constructor_expression(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    literal: &typed_trees::expression::TableStructLiteral,
    child: &impl Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    use typed_trees::data::DataMember;

    let matching_data = compilation
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == literal.type_symbol)
        .collect::<Vec<_>>();
    let [data] = matching_data.as_slice() else {
        return Err(context.rejected(format!(
            "constructor resolves its data symbol to {} declarations; expected one",
            matching_data.len()
        )));
    };
    let data_identity = nominal_identity(compilation, data.symbol)?;
    let reviewed_package = reviewed_package(compilation, "package")?;
    if context.requires_public_nominals()
        && reviewed_package_owns(&data_identity, reviewed_package)?
        && !data.is_public
    {
        return Err(context.rejected(format!(
            "exposes non-public data `{}` through a constructor",
            data.name
        )));
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
                return Err(context.rejected(format!(
                    "constructor case resolves to {} variants in `{}`; expected one",
                    matching.len(),
                    data.name
                )));
            };
            Some(*variant)
        }
        None => None,
    };
    if literal.case_name.is_some() != selected_variant.is_some() {
        return Err(context.rejected("constructor has inconsistent checked case identity"));
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
            if !field.field_symbol.is_valid() || !allowed_fields.contains(&field.field_symbol) {
                return Err(context.rejected(format!(
                    "constructor field `{}` does not rejoin its selected data/case",
                    field.name
                )));
            }
            Ok(PackageReviewConstructorField {
                field: nominal_identity(compilation, field.field_symbol)?,
                value: child(field.value)?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    fields.sort();
    if fields.windows(2).any(|pair| pair[0].field == pair[1].field) {
        return Err(context.rejected("constructor repeats one exact field"));
    }

    Ok(PackageReviewContractExpression::Constructor {
        data: data_identity,
        case: selected_variant
            .map(|variant| nominal_identity(compilation, variant.symbol))
            .transpose()?,
        fields,
    })
}

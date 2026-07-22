use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    data_definition: &resolved::data::DataDefinition,
) -> Result<typed::data::DataDefinition, Diagnostic> {
    let mut typed_data_definition = typed::data::DataDefinition {
        symbol: data_definition.symbol,
        name: crate::name::lower_name(&data_definition.name),
        supply_mode: data_definition.supply_mode,
        type_parameters: omega_core::arena::HandleSpan::empty(),
        properties: typed::data::DataProperties {
            copy: data_definition.properties.copy,
            zero_init: data_definition.properties.zero_init,
            carry: data_definition.properties.carry,
            multiplicity: data_definition.properties.multiplicity,
        },
        quotient: data_definition
            .quotient
            .as_ref()
            .map(|quotient| {
                Ok::<_, Diagnostic>(typed::data::QuotientDefinition {
                    carrier: lower_type_reference_into_table(lowerer, &quotient.carrier)?,
                    relation: quotient
                        .relation
                        .iter()
                        .map(crate::name::lower_name)
                        .collect(),
                    relation_symbol: quotient.relation_symbol,
                })
            })
            .transpose()?,
        // R2 rung 2 slice 2: copied (re-lowered) from the resolved record;
        // inert until rung 3's atomic consumer.
        where_facts: crate::domain::lower_proof_facts(lowerer, data_definition.where_facts)?,
        zero_gated: data_definition.zero_gated,
        members: omega_core::arena::HandleSpan::empty(),
    };

    for parameter in lowerer
        .source_trees
        .data_type_parameters(data_definition.type_parameters)
    {
        let type_parameter = lower_type_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_data_type_parameter(&mut typed_data_definition, type_parameter);
    }

    for member in lowerer.source_trees.data_members(data_definition.members) {
        let member = lower_data_member(lowerer, member)?;
        lowerer
            .typed_trees
            .push_data_member(&mut typed_data_definition, member);
    }

    Ok(typed_data_definition)
}

pub(crate) fn lower_type_parameter(
    lowerer: &mut Lowerer,
    parameter: &resolved::data::TypeParameter,
) -> Result<typed::data::TypeParameter, Diagnostic> {
    Ok(typed::data::TypeParameter {
        symbol: parameter.symbol,
        name: crate::name::lower_name(&parameter.name),
        kind: lower_type_parameter_kind(lowerer, &parameter.kind)?,
        bounds: typed::data::DataProperties {
            copy: parameter.bounds.copy,
            zero_init: parameter.bounds.zero_init,
            carry: parameter.bounds.carry,
            multiplicity: parameter.bounds.multiplicity,
        },
    })
}

pub(crate) fn lower_type_parameter_kind(
    lowerer: &mut Lowerer,
    kind: &resolved::data::TypeParameterKind,
) -> Result<typed::data::TypeParameterKind, Diagnostic> {
    match kind {
        resolved::data::TypeParameterKind::Type => Ok(typed::data::TypeParameterKind::Type),
        resolved::data::TypeParameterKind::Const { type_reference } => {
            Ok(typed::data::TypeParameterKind::Const {
                type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
            })
        }
        resolved::data::TypeParameterKind::Machine { contract } => {
            Ok(typed::data::TypeParameterKind::Machine {
                contract: crate::state::lower_state_signature(lowerer, contract)?,
            })
        }
    }
}

fn lower_data_member(
    lowerer: &mut Lowerer,
    member: &resolved::data::DataMember,
) -> Result<typed::data::DataMember, Diagnostic> {
    match member {
        resolved::data::DataMember::Field(field) => {
            Ok(typed::data::DataMember::Field(typed::data::DataField {
                symbol: field.symbol,
                name: crate::name::lower_name(&field.name),
                type_reference: lower_type_reference_into_table(lowerer, &field.type_reference)?,
            }))
        }
        resolved::data::DataMember::Variant(variant) => {
            let mut typed_variant = typed::data::DataVariant {
                symbol: variant.symbol,
                name: crate::name::lower_name(&variant.name),
                payload: omega_core::arena::HandleSpan::empty(),
            };
            let payload_fields = lowerer
                .source_trees
                .data_payload_fields(variant.payload)
                .to_vec();
            for field in &payload_fields {
                let lowered = typed::data::DataField {
                    symbol: field.symbol,
                    name: crate::name::lower_name(&field.name),
                    type_reference: lower_type_reference_into_table(
                        lowerer,
                        &field.type_reference,
                    )?,
                };
                lowerer
                    .typed_trees
                    .push_data_payload_field(&mut typed_variant, lowered);
            }
            Ok(typed::data::DataMember::Variant(typed_variant))
        }
    }
}

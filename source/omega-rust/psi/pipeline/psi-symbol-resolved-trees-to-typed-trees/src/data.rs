use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    data_definition: &resolved::data::DataDefinition,
) -> Result<typed::data::DataDefinition, Diagnostic> {
    let mut typed_data_definition = typed::data::DataDefinition {
        symbol: data_definition.symbol,
        name: crate::name::lower_name(&data_definition.name),
        is_public: data_definition.is_public,
        supply_mode: data_definition.supply_mode,
        lifetime_parameters: data_definition
            .lifetime_parameters
            .iter()
            .map(crate::name::lower_name)
            .collect(),
        type_parameters: psi_arena::HandleSpan::empty(),
        generic_instance: data_definition
            .generic_instance
            .as_ref()
            .map(|origin| lower_type_reference_into_table(lowerer, origin))
            .transpose()?,
        properties: typed::data::DataProperties {
            carry: data_definition.properties.carry,
            multiplicity: data_definition.properties.multiplicity,
        },
        quotient: data_definition
            .quotient
            .as_ref()
            .map(|quotient| {
                crate::type_reference::retain_static_path_selection(
                    &mut lowerer.typed_trees,
                    &quotient.relation,
                    quotient.relation_symbol,
                    lowerer.type_reference_exposure,
                    "quotient relation",
                )?;
                if let Some(selection) = &quotient.equivalence {
                    crate::type_reference::retain_static_path_selection(
                        &mut lowerer.typed_trees,
                        &selection.relation,
                        selection.relation_symbol,
                        lowerer.type_reference_exposure,
                        "quotient equivalence subject",
                    )?;
                    crate::type_reference::retain_type_reference_selection(
                        lowerer.source_trees,
                        &mut lowerer.typed_trees,
                        &selection.trait_name,
                        selection.trait_symbol,
                        lowerer.type_reference_exposure,
                        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
                    )?;
                }
                Ok::<_, Diagnostic>(typed::data::QuotientDefinition {
                    carrier: lower_type_reference_into_table(lowerer, &quotient.carrier)?,
                    relation: quotient
                        .relation
                        .iter()
                        .map(crate::name::lower_name)
                        .collect(),
                    relation_symbol: quotient.relation_symbol,
                    equivalence: quotient
                        .equivalence
                        .as_ref()
                        .map(|selection| {
                            let trait_arguments = lowerer
                                .source_trees
                                .child_type_references(selection.trait_arguments)
                                .iter()
                                .map(|argument| lower_type_reference_into_table(lowerer, argument))
                                .collect::<Result<Vec<_>, _>>()?;
                            Ok::<_, Diagnostic>(typed::data::QuotientEquivalenceSelection {
                                relation: selection
                                    .relation
                                    .iter()
                                    .map(crate::name::lower_name)
                                    .collect(),
                                relation_symbol: selection.relation_symbol,
                                trait_name: crate::name::lower_name(&selection.trait_name),
                                trait_symbol: selection.trait_symbol,
                                trait_arguments: lowerer
                                    .typed_trees
                                    .type_reference_table
                                    .insert_type_reference_handles(trait_arguments),
                                conformance_name: crate::name::lower_name(
                                    &selection.conformance_name,
                                ),
                                conformance_symbol: selection.conformance_symbol,
                            })
                        })
                        .transpose()?,
                })
            })
            .transpose()?,
        // R2 rung 2 slice 2: copied (re-lowered) from the resolved record;
        // inert until rung 3's atomic consumer.
        where_facts: crate::domain::lower_proof_facts(lowerer, data_definition.where_facts)?,
        zero_gated: data_definition.zero_gated,
        retired_identities: data_definition.retired_identities.clone(),
        members: psi_arena::HandleSpan::empty(),
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
            let contract = match contract {
                resolved::data::MachineParameterContract::RequirementIdentity => {
                    typed::data::MachineParameterContract::RequirementIdentity
                }
                resolved::data::MachineParameterContract::Structural(signature) => {
                    typed::data::MachineParameterContract::Structural(
                        crate::state::lower_state_signature(lowerer, signature)?,
                    )
                }
                resolved::data::MachineParameterContract::AuthoredNominal { .. } => {
                    return Err(Diagnostic::error(
                        "an unresolved nominal machine-parameter requirement reached typed lowering",
                    ));
                }
                resolved::data::MachineParameterContract::Nominal {
                    trait_definition,
                    requirement,
                } => typed::data::MachineParameterContract::Nominal {
                    trait_definition: *trait_definition,
                    requirement: *requirement,
                },
            };
            Ok(typed::data::TypeParameterKind::Machine { contract })
        }
        resolved::data::TypeParameterKind::Proposition { contract } => {
            let mut parameters = psi_arena::HandleSpan::empty();
            for parameter in lowerer.source_trees.state_parameters(contract.parameters) {
                let parameter = crate::state::lower_state_parameter(lowerer, parameter)?;
                lowerer
                    .typed_trees
                    .state_parameters
                    .append_to_span(&mut parameters, parameter);
            }
            Ok(typed::data::TypeParameterKind::Proposition {
                contract: typed::data::PropositionParameterSignature {
                    name: crate::name::lower_name(&contract.name),
                    parameters,
                },
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
                identity: field.identity,
                symbol: field.symbol,
                name: crate::name::lower_name(&field.name),
                relevance: field.relevance,
                type_reference: lower_type_reference_into_table(lowerer, &field.type_reference)?,
            }))
        }
        resolved::data::DataMember::Variant(variant) => {
            let mut typed_variant = typed::data::DataVariant {
                identity: variant.identity,
                symbol: variant.symbol,
                name: crate::name::lower_name(&variant.name),
                payload: psi_arena::HandleSpan::empty(),
                retired_payload_identities: variant.retired_payload_identities.clone(),
            };
            let payload_fields = lowerer
                .source_trees
                .data_payload_fields(variant.payload)
                .to_vec();
            for field in &payload_fields {
                let lowered = typed::data::DataField {
                    identity: field.identity,
                    symbol: field.symbol,
                    name: crate::name::lower_name(&field.name),
                    relevance: field.relevance,
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

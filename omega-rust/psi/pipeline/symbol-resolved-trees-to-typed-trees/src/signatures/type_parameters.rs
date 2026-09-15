use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

/// Nested callable contracts allocate their own telescopes in the same arena.
/// Finish those children before publishing the complete sibling span; appending
/// each parent immediately would interleave later siblings with nested binders.
pub(crate) fn lower_type_parameters(
    lowerer: &mut Lowerer,
    parameters: arena::HandleSpan<resolved::data::TypeParameter>,
) -> Result<arena::HandleSpan<typed::data::TypeParameter>, Diagnostic> {
    let parameters = lowerer
        .source_trees
        .data_type_parameters(parameters)
        .iter()
        .map(|parameter| lower_type_parameter(lowerer, parameter))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lowerer
        .typed_trees
        .data_type_parameters
        .insert_many(parameters))
}

fn lower_type_parameter(
    lowerer: &mut Lowerer,
    parameter: &resolved::data::TypeParameter,
) -> Result<typed::data::TypeParameter, Diagnostic> {
    Ok(typed::data::TypeParameter {
        symbol: parameter.symbol,
        name: crate::lowerer::name::lower_name(&parameter.name),
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
        resolved::data::TypeParameterKind::Value { type_reference } => {
            Ok(typed::data::TypeParameterKind::Value {
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
                        crate::signatures::callable_signature::lower_state_signature(
                            lowerer, signature,
                        )?,
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
                    authored_path,
                } => {
                    let [trait_path @ .., requirement_name] = authored_path.as_slice() else {
                        return Err(Diagnostic::error(
                            "a nominal machine-parameter requirement lost its authored `Trait::requirement` path before typed lowering",
                        ));
                    };
                    if trait_path.is_empty() {
                        return Err(Diagnostic::error(
                            "a nominal machine-parameter requirement lost its authored trait path before typed lowering",
                        ));
                    }
                    crate::type_reference::retain_path_selection(
                        &mut lowerer.typed_trees,
                        trait_path,
                        *trait_definition,
                        lowerer.type_reference_exposure,
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
                        "nominal machine-parameter trait requirement",
                    )?;
                    crate::type_reference::retain_static_path_selection(
                        &mut lowerer.typed_trees,
                        std::slice::from_ref(requirement_name),
                        *requirement,
                        lowerer.type_reference_exposure,
                        "nominal machine-parameter requirement",
                    )?;
                    typed::data::MachineParameterContract::Nominal {
                        trait_definition: *trait_definition,
                        requirement: *requirement,
                    }
                }
            };
            Ok(typed::data::TypeParameterKind::Machine { contract })
        }
        resolved::data::TypeParameterKind::Proposition { contract } => {
            let mut parameters = arena::HandleSpan::empty();
            for parameter in lowerer.source_trees.state_parameters(contract.parameters) {
                let parameter =
                    crate::signatures::parameters::lower_state_parameter(lowerer, parameter)?;
                lowerer
                    .typed_trees
                    .state_parameters
                    .append_to_span(&mut parameters, parameter);
            }
            Ok(typed::data::TypeParameterKind::Proposition {
                contract: typed::data::PropositionParameterSignature {
                    name: crate::lowerer::name::lower_name(&contract.name),
                    parameters,
                },
            })
        }
    }
}

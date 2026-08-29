use super::contracts::*;
use super::operational::*;
use crate::model::*;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_type_parameters(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    declaration_kind: &str,
    declaration_path: &str,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_type_parameters_after(
        compilation,
        parameters,
        declaration_kind,
        declaration_path,
        &[],
        0,
        lifetime_binders,
        0,
    )
}

pub(crate) fn project_type_parameters_after(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    declaration_kind: &str,
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    depth: usize,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    if depth >= 64 {
        return Err(vec![Diagnostic::error(format!(
            "public {declaration_kind} `{declaration_path}` static-machine contract exceeds the package-review depth limit",
        ))]);
    }
    let mut binders = preceding_binders.to_vec();
    binders.extend(parameters.iter().enumerate().map(|(ordinal, parameter)| {
        (
            parameter.symbol,
            format!("type-parameter:{}", ordinal_offset + ordinal),
        )
    }));
    let mut projected = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let kind = match &parameter.kind {
            psi_typed_trees::data::TypeParameterKind::Type => PackageReviewTypeParameterKind::Type,
            psi_typed_trees::data::TypeParameterKind::Const { type_reference } => {
                PackageReviewTypeParameterKind::Const(review_signature_type_identity_with_binders(
                    compilation,
                    *type_reference,
                    &binders,
                    lifetime_binders,
                )?)
            }
            psi_typed_trees::data::TypeParameterKind::Machine { contract } => {
                PackageReviewTypeParameterKind::Machine(project_machine_parameter_contract(
                    compilation,
                    parameter.symbol,
                    contract,
                    declaration_kind,
                    declaration_path,
                    &binders,
                    ordinal_offset + parameters.len(),
                    lifetime_binders,
                    depth + 1,
                )?)
            }
            psi_typed_trees::data::TypeParameterKind::Proposition { contract } => {
                let mut projected_parameters = Vec::new();
                for value_parameter in compilation
                    .typed
                    .state_parameters
                    .span_or_empty(contract.parameters)
                {
                    if value_parameter.is_const
                        || value_parameter.is_mutable
                        || value_parameter.is_self
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "public {declaration_kind} `{declaration_path}` proposition parameter uses a non-default value-parameter mode not yet certified by package review",
                        ))]);
                    }
                    projected_parameters.push(PackageReviewPropositionParameterValue {
                        type_identity: review_signature_type_identity_with_binders(
                            compilation,
                            value_parameter.type_reference,
                            &binders,
                            lifetime_binders,
                        )?,
                    });
                }
                PackageReviewTypeParameterKind::Proposition(
                    PackageReviewPropositionParameterSignature {
                        parameters: projected_parameters,
                    },
                )
            }
        };
        projected.push(PackageReviewTypeParameter {
            kind,
            bounds: parameter.bounds,
        });
    }
    Ok((binders, projected))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn project_machine_parameter_contract(
    compilation: &CheckedCompilation,
    parameter_symbol: SymbolHandle,
    contract: &psi_typed_trees::data::MachineParameterContract,
    declaration_kind: &str,
    declaration_path: &str,
    outer_binders: &[(SymbolHandle, String)],
    nested_ordinal_offset: usize,
    outer_lifetime_binders: &[psi_typed_trees::name::Identifier],
    depth: usize,
) -> Result<PackageReviewMachineParameterContract, Vec<Diagnostic>> {
    match contract {
        psi_typed_trees::data::MachineParameterContract::RequirementIdentity => {
            Ok(PackageReviewMachineParameterContract::RequirementIdentity)
        }
        psi_typed_trees::data::MachineParameterContract::Structural(signature) => {
            if signature.spelling.is_some() || signature.is_default {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` has a structural static-machine contract with trait-only requirement metadata",
                ))]);
            }
            let mut lifetime_binders = outer_lifetime_binders.to_vec();
            lifetime_binders.extend(signature.lifetime_parameters.iter().cloned());
            let (binders, type_parameters) = project_type_parameters_after(
                compilation,
                compilation.state_signature_type_parameters(signature),
                declaration_kind,
                declaration_path,
                outer_binders,
                nested_ordinal_offset,
                &lifetime_binders,
                depth,
            )?;
            let parameters = compilation.state_signature_parameters(signature);
            let context = ContractProjectionContext {
                subject_kind: "public static-machine parameter",
                subject_name: declaration_path,
                owner: psi_checked_trees::ContractProofFactOwner::StateSignature {
                    owner_symbol: parameter_symbol,
                    state_symbol: signature.symbol,
                },
                point: psi_facts::ProgramPoint::State {
                    machine_symbol: parameter_symbol,
                    state_symbol: signature.symbol,
                },
                parameters,
                domain_symbol: None,
                data_symbol: None,
                lifetime_binders: &lifetime_binders,
            };
            let contracts = project_contracts(
                compilation,
                compilation.state_signature_contracts(signature),
                &context,
                &binders,
            )?;
            let published_crash = project_signature_crash_routes(
                compilation,
                parameter_symbol,
                signature.symbol,
                "public static-machine parameter",
                declaration_path,
            )?;
            Ok(PackageReviewMachineParameterContract::Structural(
                PackageReviewMachineParameterSignature {
                    lifetime_parameter_count: signature.lifetime_parameters.len(),
                    type_parameters,
                    parameters: parameters
                        .iter()
                        .map(|parameter| {
                            Ok(PackageReviewMachineParameterValue {
                                name: parameter.name.as_str().to_owned(),
                                type_identity: review_signature_type_identity_with_binders(
                                    compilation,
                                    parameter.type_reference,
                                    &binders,
                                    &lifetime_binders,
                                )?,
                                is_const: parameter.is_const,
                                is_mutable: parameter.is_mutable,
                                is_self: parameter.is_self,
                            })
                        })
                        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
                    return_type: review_signature_type_identity_with_binders(
                        compilation,
                        signature.return_type,
                        &binders,
                        &lifetime_binders,
                    )?,
                    contracts,
                    published_crash,
                    service_reach: project_service_row(compilation, signature.service_reach_row)?,
                    service_reach_is_installation_bound: signature
                        .service_reach_is_installation_bound,
                    synchronous_invocations: project_synchronous_invocations(
                        compilation,
                        &psi_effects::declared_signature_invocations(compilation, signature),
                    )?,
                    suspends: signature.suspends,
                    blocks: signature.blocks,
                    termination: project_machine_parameter_termination(
                        compilation,
                        signature,
                        declaration_path,
                    )?,
                },
            ))
        }
        psi_typed_trees::data::MachineParameterContract::Nominal {
            trait_definition,
            requirement,
        } => {
            let matching_traits = compilation
                .traits()
                .iter()
                .filter(|candidate| candidate.symbol == *trait_definition)
                .collect::<Vec<_>>();
            let [trait_definition] = matching_traits.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` static-machine contract resolves its nominal trait to {} declarations; expected exactly one",
                    matching_traits.len(),
                ))]);
            };
            let matching_requirements = compilation
                .trait_machine_signatures(trait_definition)
                .iter()
                .filter(|candidate| candidate.symbol == *requirement)
                .collect::<Vec<_>>();
            let [requirement] = matching_requirements.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` static-machine contract resolves its nominal requirement to {} declarations in trait `{}`; expected exactly one",
                    matching_requirements.len(),
                    trait_definition.name,
                ))]);
            };
            if !trait_definition.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` exposes non-public trait `{}` through a static-machine contract",
                    trait_definition.name,
                ))]);
            }
            let trait_identity = nominal_identity(compilation, trait_definition.symbol)?;
            let requirement_identity =
                trait_requirement_identity(compilation, trait_definition, requirement)?;
            if trait_identity.owner != requirement_identity.owner {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` static-machine contract has mismatched trait and requirement ownership",
                ))]);
            }
            Ok(PackageReviewMachineParameterContract::Nominal {
                trait_identity,
                requirement_identity,
            })
        }
    }
}

pub(crate) fn project_signature_crash_routes(
    compilation: &CheckedCompilation,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    subject_kind: &str,
    subject_name: &str,
) -> Result<Vec<PackageReviewCrashRoute>, Vec<Diagnostic>> {
    let matching = compilation
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .filter(|capsule| {
            capsule.target_machine() == target_machine && capsule.target_state() == target_state
        })
        .collect::<Vec<_>>();
    let [capsule] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{subject_kind} `{subject_name}` has {} exact checked crash capsules; expected one",
            matching.len(),
        ))]);
    };
    Ok(project_crash_routes(capsule.published_buckets()))
}

pub(crate) struct ProjectedSelectedConformanceApplication {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewContractStaticArgument>,
    pub(crate) subject: PackageReviewContractStaticArgument,
    pub(crate) trait_symbol: SymbolHandle,
    pub(crate) trait_arguments: Vec<PackageReviewTypeIdentity>,
}

pub(crate) fn selected_conformance_application_type_reference(
    compilation: &mut CheckedCompilation,
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    subject_kind: &str,
    subject_name: &str,
    depth: usize,
) -> Result<psi_typed_trees::types::TypeReferenceHandle, Vec<Diagnostic>> {
    use psi_typed_trees::types::TypeReferenceNode;

    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed {subject_kind} `{subject_name}` selected conformance has {reason}",
        ))]
    };
    if depth >= 64 {
        return Err(rejected(
            "an application deeper than the portable review limit",
        ));
    }
    if argument.evidence_projection.is_some()
        || parameter_kind == ContractCallStaticParameterKind::Proposition
    {
        return Err(rejected(
            "a proposition or evidence-projection argument not represented by package review",
        ));
    }
    if let Some(literal) = argument.const_literal.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Const {
            return Err(rejected("a literal in a non-const telescope slot"));
        }
        return Ok(compilation
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: psi_typed_trees::name::Identifier::generated(literal.text()),
            }));
    }
    if let Some(application) = argument.application.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Type
            || !argument.symbol.is_valid()
            || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::Data
        {
            return Err(rejected(
                "a nested non-data application in its declaration telescope",
            ));
        }
        let definition = compilation
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == argument.symbol)
            .cloned()
            .ok_or_else(|| rejected("a nested data application without one exact declaration"))?;
        if definition.lifetime_parameters.len() != application.lifetime_arguments.len() {
            return Err(rejected(
                "a nested data application with the wrong lifetime arity",
            ));
        }
        let parameters = compilation.data_type_parameters(&definition).to_vec();
        if parameters.len() != application.arguments.len() {
            return Err(rejected(
                "a nested data application with the wrong static arity",
            ));
        }
        let mut children = Vec::with_capacity(parameters.len());
        for (child, parameter) in application.arguments.iter().zip(&parameters) {
            children.push(selected_conformance_application_type_reference(
                compilation,
                child,
                contract_call_static_parameter_kind(parameter),
                subject_kind,
                subject_name,
                depth + 1,
            )?);
        }
        let arguments = compilation
            .typed
            .type_reference_table
            .insert_type_reference_handles(children);
        return Ok(compilation
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: definition.symbol,
                base_name: definition.name,
                lifetime_arguments: application.lifetime_arguments.to_vec(),
                arguments,
            }));
    }
    if !argument.symbol.is_valid() {
        return Err(rejected("an unresolved declaration argument"));
    }
    let name = argument.path.last().cloned().unwrap_or_else(|| {
        psi_typed_trees::name::Identifier::generated(
            compilation.typed.symbols.name(argument.symbol),
        )
    });
    Ok(compilation
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: argument.symbol,
            name,
        }))
}

pub(crate) fn project_selected_conformance_application(
    compilation: &CheckedCompilation,
    selected: &psi_typed_trees::expression::StaticMachineArgument,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    declaration_kind: &str,
    declaration_path: &str,
) -> Result<ProjectedSelectedConformanceApplication, Vec<Diagnostic>> {
    use psi_typed_trees::trait_definition::ConformanceSubject;

    let closed = psi_typed_trees_to_checked_trees::close_conformance_application(
        &compilation.typed,
        selected,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let declarations = compilation
        .conformances()
        .iter()
        .filter(|declaration| declaration.symbol == selected.symbol)
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` resolves its selected conformance application to {} declarations; expected exactly one",
            declarations.len()
        ))]);
    };
    if !declaration.is_public {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` exposes non-public selected conformance `{}`",
            declaration
                .alias
                .as_ref()
                .map_or("<unnamed>", |name| name.as_str())
        ))]);
    }
    if closed.declaration != declaration.symbol {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` selected conformance closure changed declaration identity"
        ))]);
    }
    let parameters = compilation.conformance_type_parameters(declaration);
    let supplied = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| application.arguments.as_ref());
    if parameters.len() != supplied.len() {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` selected conformance application has inconsistent checked arity"
        ))]);
    }
    let arguments = supplied
        .iter()
        .zip(parameters)
        .map(|(argument, parameter)| {
            project_static_argument(
                compilation,
                declaration_kind,
                declaration_path,
                binders,
                lifetime_binders,
                argument,
                contract_call_static_parameter_kind(parameter),
                0,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lifetime_arguments = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| {
            application.lifetime_arguments.as_ref()
        })
        .iter()
        .map(|lifetime| {
            lifetime_binder_ordinal(
                lifetime,
                lifetime_binders,
                "selected conformance application",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let subject = match &declaration.subject {
        ConformanceSubject::Subjectless => {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` selects a subjectless conformance for a type-parameter bound"
            ))]);
        }
        ConformanceSubject::Carrier(_) => {
            if let Some(position) = parameters
                .iter()
                .position(|parameter| parameter.symbol == declaration.carrier_symbol)
            {
                let subject = arguments[position].clone();
                if !matches!(
                    subject,
                    PackageReviewContractStaticArgument::Type(_)
                        | PackageReviewContractStaticArgument::GenericTypeBinder(_)
                        | PackageReviewContractStaticArgument::GenericType { .. }
                ) {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` selected conformance instantiates its subject from a non-type argument"
                    ))]);
                }
                subject
            } else {
                let mut projected = compilation.clone();
                let carrier = projected
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == declaration.carrier_symbol)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "{declaration_kind} `{declaration_path}` selected conformance has no exact nominal subject"
                        ))]
                    })?;
                if !carrier.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` exposes non-public selected-conformance subject `{}`",
                        carrier.name
                    ))]);
                }
                let carrier_name = carrier.name.clone();
                let carrier = projected.typed.type_reference_table.insert(
                    psi_typed_trees::types::TypeReferenceNode::Named {
                        symbol: declaration.carrier_symbol,
                        name: carrier_name,
                    },
                );
                PackageReviewContractStaticArgument::Type(
                    review_signature_type_identity_with_binders(
                        &projected,
                        carrier,
                        binders,
                        lifetime_binders,
                    )?,
                )
            }
        }
    };

    let mut instantiated = compilation.clone();
    let mut substitutions = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(supplied) {
        substitutions.push((
            parameter.symbol,
            selected_conformance_application_type_reference(
                &mut instantiated,
                argument,
                contract_call_static_parameter_kind(parameter),
                declaration_kind,
                declaration_path,
                0,
            )?,
        ));
    }
    let selected_lifetimes = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| {
            application.lifetime_arguments.as_ref()
        });
    let lifetime_substitutions = declaration
        .lifetime_parameters
        .iter()
        .cloned()
        .zip(selected_lifetimes.iter().cloned())
        .collect::<Vec<_>>();
    let trait_arguments = compilation
        .type_reference_table
        .type_reference_handles(declaration.arguments)
        .iter()
        .map(|argument| {
            review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                &instantiated,
                *argument,
                binders,
                lifetime_binders,
                &substitutions,
                &lifetime_substitutions,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if closed.trait_definition != declaration.trait_symbol
        || closed.trait_arguments.len() != trait_arguments.len()
    {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` selected conformance closure disagrees with its exact instantiated trait application"
        ))]);
    }
    Ok(ProjectedSelectedConformanceApplication {
        declaration: nominal_identity(compilation, declaration.symbol)?,
        lifetime_arguments,
        arguments,
        subject,
        trait_symbol: declaration.trait_symbol,
        trait_arguments,
    })
}

pub(crate) fn project_conformance_bounds(
    compilation: &CheckedCompilation,
    bounds: &[psi_typed_trees::machine::GenericConformanceBound],
    parameters: &[psi_typed_trees::data::TypeParameter],
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    declaration_kind: &str,
    declaration_path: &str,
) -> Result<Vec<PackageReviewConformanceBound>, Vec<Diagnostic>> {
    let mut projected = Vec::with_capacity(bounds.len());
    let mut next_binder_ordinal = 0usize;
    for bound in bounds {
        let binder_ordinal = if let Some(binder) = bound.binder {
            if !binder.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` has an unresolved conformance evidence binder"
                ))]);
            }
            let ordinal = u32::try_from(next_binder_ordinal).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` has too many conformance binders for portable review evidence"
                ))]
            })?;
            next_binder_ordinal += 1;
            Some(ordinal)
        } else {
            None
        };
        let Some(subject_parameter) = parameters
            .iter()
            .position(|parameter| parameter.symbol == bound.subject)
        else {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` has a conformance subject outside its type-parameter telescope"
            ))]);
        };
        let (
            selected_conformance,
            selected_lifetime_arguments,
            selected_arguments,
            selected_subject,
            trait_symbol,
            trait_arguments,
        ) = match bound.selected_conformance.as_ref() {
            None => (
                None,
                Vec::new(),
                Vec::new(),
                None,
                bound.carrier,
                bound
                    .arguments
                    .iter()
                    .map(|argument| {
                        review_signature_type_identity_with_binders(
                            compilation,
                            *argument,
                            binders,
                            lifetime_binders,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Some(selected) => {
                let selected = project_selected_conformance_application(
                    compilation,
                    selected,
                    binders,
                    lifetime_binders,
                    declaration_kind,
                    declaration_path,
                )?;
                (
                    Some(selected.declaration),
                    selected.lifetime_arguments,
                    selected.arguments,
                    Some(selected.subject),
                    selected.trait_symbol,
                    selected.trait_arguments,
                )
            }
        };
        let matching_traits = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == trait_symbol)
            .collect::<Vec<_>>();
        let [trait_definition] = matching_traits.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` conformance bound resolves to {} traits; expected exactly one",
                matching_traits.len()
            ))]);
        };
        if !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` exposes non-public conformance trait `{}`",
                trait_definition.name
            ))]);
        }
        if !trait_definition.lifetime_parameters.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` uses lifetime-parameterized conformance trait `{}` without retained lifetime arguments",
                trait_definition.name
            ))]);
        }
        projected.push(PackageReviewConformanceBound {
            binder_ordinal,
            subject_parameter: u32::try_from(subject_parameter).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` conformance subject exceeds the portable review parameter range"
                ))]
            })?,
            selected_conformance,
            selected_lifetime_arguments,
            selected_arguments,
            selected_subject,
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            arguments: trait_arguments,
        });
    }
    Ok(projected)
}

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

pub(crate) fn review_domain_lifetime_label(
    compilation: &CheckedCompilation,
    domain: &psi_typed_trees::types::DomainConstraint,
) -> Result<String, Vec<Diagnostic>> {
    use psi_typed_trees::types::{DomainConstraintSubject, OmegaLayoutGrammar};

    match domain.subject {
        DomainConstraintSubject::Declared => {
            let identity = nominal_identity(compilation, domain.symbol)?;
            let owner = match identity.owner {
                PackageReviewNominalOwner::Package(package) => {
                    canonical_digest_label("package", package.digest())
                }
                PackageReviewNominalOwner::ToolchainSource(source) => {
                    canonical_digest_label("toolchain-source", source.digest())
                }
                PackageReviewNominalOwner::Unresolved => {
                    return Err(vec![Diagnostic::error(
                        "package review rejects a declared domain without exact nominal ownership",
                    )]);
                }
            };
            Ok(framed_identity("declared-domain", &[owner, identity.path]))
        }
        DomainConstraintSubject::Carry(permission) => Ok(framed_identity(
            "compiler-domain",
            &[
                "carry".to_owned(),
                match permission {
                    psi_language_semantics::CarryPermission::AcrossSuspend => "across-suspend",
                    psi_language_semantics::CarryPermission::AnyCpu => "any-cpu",
                    psi_language_semantics::CarryPermission::AnyThread => "any-thread",
                    psi_language_semantics::CarryPermission::MovableAddress => "movable-address",
                }
                .to_owned(),
            ],
        )),
        DomainConstraintSubject::Value(value_domain) => Ok(framed_identity(
            "compiler-domain",
            &[
                "value".to_owned(),
                match value_domain {
                    psi_language_semantics::value_domain::ValueDomain::Finite => "finite",
                }
                .to_owned(),
            ],
        )),
        DomainConstraintSubject::OmegaLayout { grammar } => Ok(framed_identity(
            "compiler-domain",
            &[
                "omega-layout".to_owned(),
                match grammar {
                    OmegaLayoutGrammar::Derived => "derived",
                }
                .to_owned(),
            ],
        )),
    }
}

pub(crate) fn canonical_digest_label(kind: &str, digest: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut label = String::with_capacity(kind.len() + 1 + digest.len() * 2);
    label.push_str(kind);
    label.push(':');
    for byte in digest {
        let _ = write!(label, "{byte:02x}");
    }
    label
}

pub(crate) fn review_lifetime_topology_with_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
    active_substitutions: &mut Vec<SymbolHandle>,
) -> Result<String, Vec<Diagnostic>> {
    use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    let topology = match compilation
        .type_reference_table
        .type_reference(type_reference)
    {
        TypeReferenceNode::Reference {
            referee, lifetime, ..
        } => {
            let lifetime = match lifetime {
                Some(lifetime) => format!(
                    "binder:{}",
                    substituted_lifetime_binder_ordinal(
                        lifetime,
                        lifetime_binders,
                        lifetime_substitutions,
                        "public type",
                    )?
                ),
                None => "elided".to_owned(),
            };
            framed_identity(
                "reference",
                &[
                    lifetime,
                    review_lifetime_topology_with_substitutions(
                        compilation,
                        *referee,
                        lifetime_binders,
                        substitutions,
                        lifetime_substitutions,
                        active_substitutions,
                    )?,
                ],
            )
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut constraint_topologies = compilation
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    TypeConstraintNode::Domain(domain) if !domain.arguments.is_empty() => {
                        Some((|| {
                            let label = review_domain_lifetime_label(compilation, domain)?;
                            let arguments = domain
                                .arguments
                                .iter()
                                .map(|argument| {
                                    review_lifetime_topology_with_substitutions(
                                        compilation,
                                        *argument,
                                        lifetime_binders,
                                        substitutions,
                                        lifetime_substitutions,
                                        active_substitutions,
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            Ok::<String, Vec<Diagnostic>>(framed_identity(&label, &arguments))
                        })())
                    }
                    _ => None,
                })
                .collect::<Result<Vec<_>, _>>()?;
            constraint_topologies.sort();
            constraint_topologies.dedup();
            let mut children = vec![review_lifetime_topology_with_substitutions(
                compilation,
                *base_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?];
            children.extend(constraint_topologies);
            framed_identity("constrained", &children)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => framed_identity(
            "array",
            &[review_lifetime_topology_with_substitutions(
                compilation,
                *element_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?],
        ),
        TypeReferenceNode::Slice { element_type } => framed_identity(
            "slice",
            &[review_lifetime_topology_with_substitutions(
                compilation,
                *element_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?],
        ),
        TypeReferenceNode::Generic {
            lifetime_arguments,
            arguments,
            ..
        } => {
            let mut children = lifetime_arguments
                .iter()
                .map(|lifetime| {
                    substituted_lifetime_binder_ordinal(
                        lifetime,
                        lifetime_binders,
                        lifetime_substitutions,
                        "public type",
                    )
                    .map(|ordinal| format!("binder:{ordinal}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            children.extend(
                compilation
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| {
                        review_lifetime_topology_with_substitutions(
                            compilation,
                            *argument,
                            lifetime_binders,
                            substitutions,
                            lifetime_substitutions,
                            active_substitutions,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            framed_identity("generic", &children)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            let Some((_, replacement)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            else {
                return Ok("named".to_owned());
            };
            if active_substitutions.contains(symbol) {
                return Err(vec![Diagnostic::error(
                    "package review rejects a cyclic inherited type substitution",
                )]);
            }
            active_substitutions.push(*symbol);
            let topology = review_lifetime_topology_with_substitutions(
                compilation,
                *replacement,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            );
            active_substitutions.pop();
            topology?
        }
        TypeReferenceNode::DynamicTrait { .. } => "dynamic-trait".to_owned(),
        TypeReferenceNode::ConstExpression(_) => "const-expression".to_owned(),
        TypeReferenceNode::Unit => "unit".to_owned(),
    };
    Ok(topology)
}

pub(crate) fn substituted_lifetime_binder_ordinal(
    lifetime: &psi_typed_trees::name::Identifier,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
    context: &str,
) -> Result<u32, Vec<Diagnostic>> {
    let lifetime = substitutions
        .iter()
        .rev()
        .find_map(|(parameter, argument)| (parameter == lifetime).then_some(argument))
        .unwrap_or(lifetime);
    lifetime_binder_ordinal(lifetime, lifetime_binders, context)
}

pub(crate) fn lifetime_binder_ordinal(
    lifetime: &psi_typed_trees::name::Identifier,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    context: &str,
) -> Result<u32, Vec<Diagnostic>> {
    let Some(ordinal) = lifetime_binders
        .iter()
        .position(|candidate| candidate == lifetime)
    else {
        return Err(vec![Diagnostic::error(format!(
            "{context} refers to unresolved lifetime `'{}'",
            lifetime.as_str()
        ))]);
    };
    u32::try_from(ordinal).map_err(|_| {
        vec![Diagnostic::error(format!(
            "{context} lifetime binder ordinal exceeds the portable package-review limit"
        ))]
    })
}

pub(crate) fn framed_identity(label: &str, children: &[String]) -> String {
    use std::fmt::Write as _;

    let mut identity = String::new();
    let _ = write!(identity, "{}:{label}", label.len());
    for child in children {
        let _ = write!(identity, "{}:{child}", child.len());
    }
    identity
}

pub(crate) fn reviewed_package_owns(
    identity: &PackageReviewNominalIdentity,
    package: PackageKeyIdentity,
) -> Result<bool, Vec<Diagnostic>> {
    match identity.owner {
        PackageReviewNominalOwner::Package(owner) => Ok(owner == package),
        PackageReviewNominalOwner::ToolchainSource(_) => Ok(false),
        PackageReviewNominalOwner::Unresolved => Err(vec![Diagnostic::error(format!(
            "reviewed public declaration `{}` has no managed package owner",
            identity.path
        ))]),
    }
}

pub(crate) fn nominal_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner = nominal_owner(compilation, symbol)?;
    let path = compilation.typed.symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review encountered a symbol without a stable declaration path",
        )]);
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

pub(crate) fn trait_requirement_identity(
    compilation: &CheckedCompilation,
    owner: &psi_typed_trees::trait_definition::TraitDefinition,
    requirement: &psi_typed_trees::signature::StateSignature,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner_identity = nominal_identity(compilation, owner.symbol)?;
    let requirement_owner = nominal_owner(compilation, requirement.symbol)?;
    if owner_identity.owner != requirement_owner {
        return Err(vec![Diagnostic::error(format!(
            "package review trait `{}` and requirement `{}` have mismatched exact ownership",
            owner.name, requirement.name
        ))]);
    }
    Ok(PackageReviewNominalIdentity {
        owner: requirement_owner,
        path: compilation
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity(),
    })
}

pub(crate) fn trait_requirement_identity_from_symbols(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    context: &str,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owners = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its declaring trait to {} declarations; expected exactly one",
            owners.len()
        ))]);
    };
    let requirements = compilation
        .trait_machine_signatures(owner)
        .iter()
        .filter(|candidate| candidate.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its requirement to {} overload declarations under its exact trait; expected exactly one",
            requirements.len()
        ))]);
    };
    trait_requirement_identity(compilation, owner, requirement)
}

pub(crate) fn provider_requirement_identity(
    compilation: &CheckedCompilation,
    schema: omega_provider_planning::plans::ProviderSchemaDeclaration,
    requirement_symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    match schema {
        omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryTrait(trait_symbol) => {
            trait_requirement_identity_from_symbols(
                compilation,
                trait_symbol,
                requirement_symbol,
                "selected provider row",
            )
        }
        omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(_) => {
            let operators = compilation.operators().iter().chain(
                compilation
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| compilation.domain_operators(domain)),
            );
            let matches = operators
                .filter(|candidate| candidate.symbol == requirement_symbol && candidate.is_boundary)
                .collect::<Vec<_>>();
            let [operator] = matches.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider row resolves its boundary operator requirement to {} declarations; expected exactly one",
                    matches.len()
                ))]);
            };
            let nominal = nominal_identity(compilation, requirement_symbol)?;
            Ok(PackageReviewNominalIdentity {
                owner: nominal.owner,
                path: psi_typed_trees::operator::boundary_operator_requirement_identity(
                    &compilation.typed,
                    operator,
                ),
            })
        }
    }
}

pub(crate) fn nominal_owner(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    nominal_owner_from_symbols(&compilation.typed.symbols, symbol)
}

pub(crate) fn nominal_owner_from_symbols(
    symbols: &psi_symbols::SymbolTable,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    if let Some(package) = symbols.symbol_package_identity(symbol) {
        return Ok(PackageReviewNominalOwner::Package(package));
    }
    let Some(source_file) = symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| symbols.source_file(span))
    else {
        return Ok(PackageReviewNominalOwner::Unresolved);
    };
    match source_file.origin {
        psi_source::SourceOrigin::Toolchain => Ok(PackageReviewNominalOwner::ToolchainSource(
            toolchain_source_identity(source_file)?,
        )),
        psi_source::SourceOrigin::User => Ok(PackageReviewNominalOwner::Unresolved),
    }
}

pub(crate) fn toolchain_source_identity(
    source_file: &psi_source::SourceFile,
) -> Result<PackageReviewToolchainSourceIdentity, Vec<Diagnostic>> {
    Ok(PackageReviewToolchainSourceIdentity {
        digest: omega_package_compilation::toolchain_source_identity_digest(source_file)?,
    })
}

pub(crate) fn is_canonical_virtual_toolchain_path(path: &std::path::Path) -> bool {
    let mut components = path.components();
    let Some(std::path::Component::Normal(component)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    component.to_str().is_some_and(|component| {
        component.len() >= 3 && component.starts_with('<') && component.ends_with('>')
    })
}

pub(crate) fn exactly_one<'item, Item>(
    mut matches: impl Iterator<Item = &'item Item>,
    subject: &str,
    fact_kind: &str,
) -> Result<&'item Item, Vec<Diagnostic>> {
    let first = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no exact checked {fact_kind} row"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has duplicate checked {fact_kind} rows"
        ))]);
    }
    Ok(first)
}

#[cfg(test)]
mod tests;

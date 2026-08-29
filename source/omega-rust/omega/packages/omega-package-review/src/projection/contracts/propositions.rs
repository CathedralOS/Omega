use super::super::exact_identity::*;
use super::*;
use crate::model::*;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_contract_proposition(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    callable_binders: &[(SymbolHandle, String)],
    application: &psi_typed_trees::proposition::PropositionApplication,
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    binder_substitutions: &[(SymbolHandle, PackageReviewPropositionBinderArgument)],
    value_substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    visiting: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};

    if depth >= 64 {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition expansion exceeds the package-review depth limit",
            context.subject_kind, context.subject_name
        ))]);
    }
    if visiting.contains(&application.proposition) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition expansion is cyclic",
            context.subject_kind, context.subject_name
        ))]);
    }
    if application.proposition.is_valid()
        && compilation.typed.symbols.get(application.proposition).kind
            == psi_symbols::SymbolKind::PropositionParameter
    {
        if !application.binder_arguments.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint has unexpected static arguments",
                context.subject_kind, context.subject_name
            ))]);
        }
        let matching_parameters = compilation
            .typed
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .filter(|parameter| parameter.symbol == application.proposition)
            .collect::<Vec<_>>();
        let [parameter] = matching_parameters.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint rejoins {} declaration parameters; expected exactly one",
                context.subject_kind,
                context.subject_name,
                matching_parameters.len()
            ))]);
        };
        let psi_typed_trees::data::TypeParameterKind::Proposition { contract } = &parameter.kind
        else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint does not rejoin a proposition-family signature",
                context.subject_kind, context.subject_name
            ))]);
        };
        let argument_handles = compilation
            .expression_table
            .expression_handles(application.arguments);
        let parameter_count = compilation
            .typed
            .state_parameters
            .span_or_empty(contract.parameters)
            .len();
        if argument_handles.len() != parameter_count {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint has inconsistent checked arity",
                context.subject_kind, context.subject_name
            ))]);
        }
        let mut static_ordinal = 0usize;
        let mut matching_ordinals = Vec::new();
        for (symbol, _) in callable_binders {
            if matches!(
                compilation.typed.symbols.get(*symbol).kind,
                psi_symbols::SymbolKind::TypeParameter
                    | psi_symbols::SymbolKind::MachineParameter
                    | psi_symbols::SymbolKind::PropositionParameter
            ) {
                if *symbol == application.proposition {
                    matching_ordinals.push(static_ordinal);
                }
                static_ordinal += 1;
            }
        }
        let [binder_ordinal] = matching_ordinals.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint rejoins {} callable static binders; expected exactly one",
                context.subject_kind,
                context.subject_name,
                matching_ordinals.len()
            ))]);
        };
        let arguments = argument_handles
            .iter()
            .map(|argument| {
                project_contract_expression_with_substitutions(
                    compilation,
                    context,
                    callable_binders,
                    *argument,
                    value_substitutions,
                    &[],
                    checked_fact,
                    0,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(PackageReviewContractFact::PropositionParameter(
            PackageReviewPropositionParameterApplication {
                binder_ordinal: portable_parameter_position(*binder_ordinal)?,
                arguments,
            },
        ));
    }
    let declaration = compilation
        .propositions()
        .iter()
        .find(|candidate| candidate.symbol == application.proposition)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract refers to an unresolved or generic proposition endpoint",
                context.subject_kind, context.subject_name
            ))]
        })?;
    let declaration_binders = compilation.proposition_binders(declaration);
    let declaration_parameters = compilation.proposition_parameters(declaration);
    if declaration_binders.len() != application.binder_arguments.len()
        || declaration_parameters.len()
            != compilation
                .expression_table
                .expression_handles(application.arguments)
                .len()
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition `{}` has inconsistent checked arity",
            context.subject_kind, context.subject_name, declaration.name
        ))]);
    }
    let binder_arguments = declaration_binders
        .iter()
        .zip(&application.binder_arguments)
        .map(|(binder, argument)| {
            let expected = match binder.kind {
                psi_typed_trees::proposition::PropositionBinderKind::Type => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Type
                }
                psi_typed_trees::proposition::PropositionBinderKind::Const { .. } => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Const
                }
                psi_typed_trees::proposition::PropositionBinderKind::Machine => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine
                }
            };
            if argument.kind != expected {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition `{}` binder kind changed during typing",
                    context.subject_kind, context.subject_name, declaration.name
                ))]);
            }
            project_proposition_binder_argument(
                compilation,
                context,
                callable_binders,
                argument,
                binder_substitutions,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = compilation
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            project_contract_expression_with_substitutions(
                compilation,
                context,
                callable_binders,
                *argument,
                value_substitutions,
                &[],
                checked_fact,
                0,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    match &declaration.body {
        PropositionBody::Primitive | PropositionBody::Witness { .. } => Ok(
            PackageReviewContractFact::Proposition(project_proposition_endpoint(
                compilation,
                declaration,
                binder_arguments,
                arguments,
            )?),
        ),
        PropositionBody::Transparent { proposition } => {
            visiting.push(declaration.symbol);
            let mut nested_binders = binder_substitutions.to_vec();
            nested_binders.extend(
                declaration_binders
                    .iter()
                    .zip(&binder_arguments)
                    .map(|(binder, argument)| (binder.symbol, argument.clone())),
            );
            let mut nested_values = value_substitutions.to_vec();
            nested_values.extend(
                declaration_parameters
                    .iter()
                    .zip(&arguments)
                    .map(|(parameter, argument)| (parameter.symbol, argument.clone())),
            );
            for (binder, argument) in declaration_binders.iter().zip(&binder_arguments) {
                if let Some(value) = proposition_binder_value_expression(argument) {
                    nested_values.push((binder.symbol, value));
                }
            }
            let projected = match proposition {
                PropositionFormula::Application(expansion) => project_contract_proposition(
                    compilation,
                    context,
                    callable_binders,
                    expansion,
                    checked_fact,
                    &nested_binders,
                    &nested_values,
                    visiting,
                    depth + 1,
                ),
                PropositionFormula::BooleanExpression(expression) => {
                    let projection_substitutions = declaration_parameters
                        .iter()
                        .zip(
                            compilation
                                .expression_table
                                .expression_handles(application.arguments),
                        )
                        .map(|(parameter, argument)| (parameter.symbol, *argument))
                        .collect::<Vec<_>>();
                    project_contract_expression_with_substitutions(
                        compilation,
                        context,
                        callable_binders,
                        *expression,
                        &nested_values,
                        &projection_substitutions,
                        checked_fact,
                        0,
                    )
                    .map(PackageReviewContractFact::Expression)
                }
            };
            visiting.pop();
            projected
        }
    }
}

pub(crate) fn project_proposition_binder_argument(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    callable_binders: &[(SymbolHandle, String)],
    argument: &psi_typed_trees::proposition::PropositionBinderArgument,
    substitutions: &[(SymbolHandle, PackageReviewPropositionBinderArgument)],
) -> Result<PackageReviewPropositionBinderArgument, Vec<Diagnostic>> {
    if let Some((_, substitution)) = substitutions
        .iter()
        .rev()
        .find(|(symbol, _)| *symbol == argument.symbol)
    {
        if substitution.kind != argument.kind {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` proposition binder substitution changes kind",
                context.subject_kind, context.subject_name
            ))]);
        }
        return Ok(substitution.clone());
    }
    let value = if let Some(projection) = argument.evidence_projection.as_ref() {
        project_proposition_evidence_projection(compilation, context, projection)?
    } else if let Some(literal) = &argument.const_literal {
        PackageReviewPropositionBinderValue::Integer(literal.text().to_owned())
    } else if let Some(position) = callable_binders
        .iter()
        .position(|(symbol, _)| *symbol == argument.symbol)
    {
        PackageReviewPropositionBinderValue::GenericBinder(portable_parameter_position(position)?)
    } else if argument.symbol.is_valid() {
        match argument.kind {
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                let identity = compilation
                    .package_qualified_nominal_type_identity_with_toolchain_sources(
                        argument.symbol,
                        compilation.exact_toolchain_sources(),
                    )
                    .ok_or_else(missing_exact_toolchain_type_owner)?;
                PackageReviewPropositionBinderValue::Type(PackageReviewTypeIdentity {
                    canonical: identity.into_string(),
                })
            }
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                PackageReviewPropositionBinderValue::Machine(nominal_identity(
                    compilation,
                    argument.symbol,
                )?)
            }
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition contains a non-literal const binder argument without an exact caller binder",
                    context.subject_kind, context.subject_name
                ))]);
            }
        }
    } else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition contains an unresolved binder argument",
            context.subject_kind, context.subject_name
        ))]);
    };
    Ok(PackageReviewPropositionBinderArgument {
        kind: argument.kind,
        value,
    })
}

pub(crate) fn project_proposition_evidence_projection(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    projection: &psi_typed_trees::expression::EvidenceProjection,
) -> Result<PackageReviewPropositionBinderValue, Vec<Diagnostic>> {
    let matching_terms = compilation
        .facts
        .proof
        .evidence_terms
        .iter()
        .filter_map(|(handle, term)| {
            (term.owner == context.owner
                && term.kind == psi_checked_trees::ContractProofFactKind::Requires
                && term.name == projection.term.as_str())
            .then_some((handle, term))
        })
        .collect::<Vec<_>>();
    let [(term_handle, term)] = matching_terms.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} checked source terms; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_terms.len()
        ))]);
    };
    let Some(checked_interface) = term.evidence_interface.as_ref() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` has no exact checked source interface",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    };
    let matching_requirements = checked_interface
        .requirements
        .iter()
        .filter(|requirement| {
            compilation.symbols.name(requirement.requirement) == projection.member.as_str()
        })
        .collect::<Vec<_>>();
    let [checked_requirement] = matching_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} checked requirement rows; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_requirements.len()
        ))]);
    };
    if !compilation
        .facts
        .proof
        .proposition_vocabulary
        .applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .filter_map(|argument| argument.evidence_projection.as_ref())
        .any(|retained| {
            retained.term == *term_handle
                && retained.declaring_trait == checked_requirement.declaring_trait
                && retained.requirement == checked_requirement.requirement
        })
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` has no retained checked projection row",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    }

    let declaration = compilation
        .propositions()
        .iter()
        .find(|candidate| candidate.symbol == term.proposition.declaration)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` evidence projection `{}.{}` has an unresolved source proposition endpoint",
                context.subject_kind, context.subject_name, projection.term, projection.member
            ))]
        })?;
    let psi_typed_trees::proposition::PropositionBody::Witness { evidence } = &declaration.body
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` does not originate from witness evidence",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    };
    let proposition_binders = compilation
        .proposition_binders(declaration)
        .iter()
        .enumerate()
        .map(|(position, binder)| (binder.symbol, format!("proposition-binder:{position}")))
        .collect::<Vec<_>>();
    let interface = project_evidence_interface(compilation, *evidence, &proposition_binders)?;
    if nominal_identity(compilation, checked_interface.trait_symbol)? != interface.trait_identity {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` changed source interface during checked lowering",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    }
    let declaring_trait = nominal_identity(compilation, checked_requirement.declaring_trait)?;
    let requirement = trait_requirement_identity_from_symbols(
        compilation,
        checked_requirement.declaring_trait,
        checked_requirement.requirement,
        "checked evidence projection",
    )?;
    let matching_projected = interface
        .requirements
        .iter()
        .filter(|candidate| {
            candidate.declaring_trait == declaring_trait && candidate.requirement == requirement
        })
        .collect::<Vec<_>>();
    let [projected_requirement] = matching_projected.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} structural interface rows; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_projected.len()
        ))]);
    };
    Ok(PackageReviewPropositionBinderValue::EvidenceProjection {
        source_kind: PackageReviewContractKind::Requires,
        source_lane_position: portable_parameter_position(term.lane_position)?,
        declaring_trait,
        declaring_trait_arguments: projected_requirement.declaring_trait_arguments.clone(),
        requirement,
    })
}

pub(crate) fn proposition_binder_value_expression(
    argument: &PackageReviewPropositionBinderArgument,
) -> Option<PackageReviewContractExpression> {
    match &argument.value {
        PackageReviewPropositionBinderValue::Machine(identity) => {
            Some(PackageReviewContractExpression::Nominal(identity.clone()))
        }
        PackageReviewPropositionBinderValue::Type(_) => None,
        PackageReviewPropositionBinderValue::GenericBinder(position) => {
            Some(PackageReviewContractExpression::GenericBinder(*position))
        }
        PackageReviewPropositionBinderValue::Integer(value) => {
            Some(PackageReviewContractExpression::Integer(value.clone()))
        }
        PackageReviewPropositionBinderValue::EvidenceProjection { .. } => None,
    }
}

pub(crate) fn project_proposition_endpoint(
    compilation: &CheckedCompilation,
    declaration: &psi_typed_trees::proposition::PropositionDefinition,
    binder_arguments: Vec<PackageReviewPropositionBinderArgument>,
    arguments: Vec<PackageReviewContractExpression>,
) -> Result<PackageReviewPropositionApplication, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::PropositionBody;

    let (binders, parameter_types) = project_proposition_signature(compilation, declaration)?;
    let binder_symbols = compilation
        .proposition_binders(declaration)
        .iter()
        .enumerate()
        .map(|(position, binder)| (binder.symbol, format!("proposition-binder:{position}")))
        .collect::<Vec<_>>();
    let evidence = match declaration.body {
        PropositionBody::Primitive => PackageReviewPropositionEvidence::FactOnly,
        PropositionBody::Witness { evidence } => PackageReviewPropositionEvidence::Witness(
            project_evidence_interface(compilation, evidence, &binder_symbols)?,
        ),
        PropositionBody::Transparent { .. } => unreachable!("transparent endpoint was expanded"),
    };
    Ok(PackageReviewPropositionApplication {
        declaration: nominal_identity(compilation, declaration.symbol)?,
        binders,
        parameter_types,
        binder_arguments,
        arguments,
        evidence,
    })
}

pub(crate) fn project_proposition_signature(
    compilation: &CheckedCompilation,
    declaration: &psi_typed_trees::proposition::PropositionDefinition,
) -> Result<
    (
        Vec<PackageReviewPropositionBinder>,
        Vec<PackageReviewTypeIdentity>,
    ),
    Vec<Diagnostic>,
> {
    use psi_typed_trees::proposition::PropositionBinderKind;

    let declaration_binders = compilation.proposition_binders(declaration);
    let binder_symbols = declaration_binders
        .iter()
        .enumerate()
        .map(|(position, binder)| (binder.symbol, format!("proposition-binder:{position}")))
        .collect::<Vec<_>>();
    let binders = declaration_binders
        .iter()
        .map(|binder| {
            Ok(PackageReviewPropositionBinder {
                kind: match binder.kind {
                    PropositionBinderKind::Type => PackageReviewPropositionBinderKind::Type,
                    PropositionBinderKind::Const { type_reference } => {
                        PackageReviewPropositionBinderKind::Const(
                            review_type_identity_with_binders(
                                compilation,
                                type_reference,
                                &binder_symbols,
                            )?,
                        )
                    }
                    PropositionBinderKind::Machine => PackageReviewPropositionBinderKind::Machine,
                },
                bounds: binder.bounds,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let parameter_types = compilation
        .proposition_parameters(declaration)
        .iter()
        .map(|parameter| {
            review_type_identity_with_binders(
                compilation,
                parameter.type_reference,
                &binder_symbols,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((binders, parameter_types))
}

pub(crate) fn project_evidence_interface(
    compilation: &CheckedCompilation,
    evidence: psi_typed_trees::types::TypeReferenceHandle,
    proposition_binders: &[(SymbolHandle, String)],
) -> Result<PackageReviewEvidenceInterface, Vec<Diagnostic>> {
    use psi_typed_trees::types::TypeReferenceNode;

    let (trait_symbol, arguments) = match compilation.type_reference_table.type_reference(evidence)
    {
        TypeReferenceNode::Named { symbol, .. } => (*symbol, Vec::new()),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => (
            *base_symbol,
            compilation
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec(),
        ),
        _ => {
            return Err(vec![Diagnostic::error(
                "reviewed witness proposition uses a non-nominal evidence interface",
            )]);
        }
    };
    let definition = compilation
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "reviewed witness proposition has an unresolved evidence trait",
            )]
        })?;
    if !definition.lifetime_parameters.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed witness proposition uses lifetime-parameterized evidence trait `{}` without retained lifetime arguments",
            definition.name
        ))]);
    }
    let projected_arguments = arguments
        .iter()
        .map(|argument| {
            review_type_identity_with_binders_and_substitutions(
                compilation,
                *argument,
                proposition_binders,
                &[],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut requirements = Vec::new();
    collect_evidence_requirements(
        compilation,
        trait_symbol,
        &arguments,
        proposition_binders,
        None,
        &[],
        &mut Vec::new(),
        &mut requirements,
    )?;
    requirements.sort();
    requirements.dedup();
    Ok(PackageReviewEvidenceInterface {
        trait_identity: nominal_identity(compilation, trait_symbol)?,
        arguments: projected_arguments,
        requirements,
    })
}

pub(crate) fn collect_evidence_requirements(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    trait_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    proposition_binders: &[(SymbolHandle, String)],
    lifetime_binders: Option<&[psi_typed_trees::name::Identifier]>,
    inherited_substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    visited: &mut Vec<(PackageReviewNominalIdentity, Vec<PackageReviewTypeIdentity>)>,
    requirements: &mut Vec<PackageReviewEvidenceRequirement>,
) -> Result<(), Vec<Diagnostic>> {
    let definition = compilation
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "reviewed evidence interface inherits an unresolved trait",
            )]
        })?;
    if !definition.lifetime_parameters.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence interface inherits lifetime-parameterized trait `{}`",
            definition.name
        ))]);
    }
    let type_parameters = compilation.trait_type_parameters(definition);
    if type_parameters.len() != trait_arguments.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence trait `{}` has inconsistent instantiated arity",
            definition.name
        ))]);
    }
    if type_parameters.iter().any(|parameter| {
        !matches!(
            &parameter.kind,
            psi_typed_trees::data::TypeParameterKind::Type
                | psi_typed_trees::data::TypeParameterKind::Const { .. }
                | psi_typed_trees::data::TypeParameterKind::Machine {
                    contract: psi_typed_trees::data::MachineParameterContract::RequirementIdentity
                }
        )
    }) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence trait `{}` uses a structural/nominal machine or proposition parameter not represented by package review",
            definition.name
        ))]);
    }
    let argument_identities = trait_arguments
        .iter()
        .map(|argument| match lifetime_binders {
            Some(lifetime_binders) => {
                review_signature_type_identity_with_binders_and_substitutions(
                    compilation,
                    *argument,
                    proposition_binders,
                    lifetime_binders,
                    inherited_substitutions,
                )
            }
            None => review_type_identity_with_binders_and_substitutions(
                compilation,
                *argument,
                proposition_binders,
                inherited_substitutions,
            ),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let visit = (
        nominal_identity(compilation, trait_symbol)?,
        argument_identities.clone(),
    );
    if visited.contains(&visit) {
        return Ok(());
    }
    visited.push(visit);

    for requirement in compilation.trait_machine_signatures(definition) {
        requirements.push(PackageReviewEvidenceRequirement {
            declaring_trait: nominal_identity(compilation, trait_symbol)?,
            declaring_trait_arguments: argument_identities.clone(),
            requirement: trait_requirement_identity(compilation, definition, requirement)?,
        });
    }

    let mut substitutions = inherited_substitutions.to_vec();
    substitutions.extend(
        type_parameters
            .iter()
            .zip(trait_arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument)),
    );
    for parent in compilation.trait_requirements(definition) {
        if !parent.lifetime_arguments.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed evidence trait `{}` has a parent with lifetime arguments not yet represented by package review",
                definition.name
            ))]);
        }
        let parent_arguments = compilation
            .type_reference_table
            .type_reference_handles(parent.arguments);
        collect_evidence_requirements(
            compilation,
            parent.symbol,
            parent_arguments,
            proposition_binders,
            lifetime_binders,
            &substitutions,
            visited,
            requirements,
        )?;
    }
    Ok(())
}

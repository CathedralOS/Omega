use super::model::ProjectedSelectedConformanceApplication;
use crate::evidence::PackageReviewContractStaticArgument;
use crate::projection::checked_semantics::declarations::nominal_identity;
use crate::projection::checked_semantics::types::lifetimes::lifetime_binder_ordinal;
use crate::projection::checked_semantics::types::{
    review_signature_type_identity_with_binders,
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes,
};
use crate::projection::contracts::expressions::static_arguments::{
    contract_call_static_parameter_kind, project_static_argument, ContractCallStaticParameterKind,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

fn selected_conformance_application_type_reference(
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

pub(super) fn project_selected_conformance_application(
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
    let selected_trait_lifetimes = declaration
        .trait_lifetime_arguments
        .iter()
        .map(|ordinal| {
            let ordinal = usize::try_from(*ordinal).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` selected conformance retains an invalid target-trait lifetime ordinal"
                ))]
            })?;
            let selected = selected_lifetimes.get(ordinal).ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` selected conformance target-trait lifetime falls outside its checked application"
                ))]
            })?;
            Ok::<_, Vec<Diagnostic>>(selected.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let trait_lifetime_arguments = selected_trait_lifetimes
        .iter()
        .map(|selected| {
            lifetime_binder_ordinal(
                selected,
                lifetime_binders,
                "selected conformance target trait application",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
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
        || closed.trait_lifetime_arguments
            != selected_trait_lifetimes
                .iter()
                .map(|lifetime| lifetime.as_str().to_owned())
                .collect::<Vec<_>>()
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
        trait_lifetime_arguments,
        trait_arguments,
    })
}

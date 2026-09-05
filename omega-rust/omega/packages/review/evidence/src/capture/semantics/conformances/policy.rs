//! Policy projection from exact retained selections, including private slots.

use super::policy_arguments::{argument_context, argument_type_reference, rejected};
use super::policy_callables::{callable_identity, caller_binder_identity};
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::declarations::trait_requirement_identity_from_symbols;
use crate::capture::semantics::types::review_signature_type_identity_with_binders_and_substitutions_and_lifetimes;
use crate::record::{
    PackagePolicyClosedConformanceApplication, PackagePolicyConformanceConstArgument,
    PackagePolicyConformanceRow,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::data::TypeParameterKind;
use psi_typed_trees::expression::{StaticMachineArgument, StaticSymbolApplication};
use psi_typed_trees::name::Identifier;
use psi_typed_trees::trait_definition::ConformanceSubject;
use psi_typed_trees::typed_trees::{ClosedConformanceApplication, ClosedConformanceConstArgument};
use psi_typed_trees::types::TypeReferenceNode;

/// Retain the complete source-qualified conformance meaning without historical
/// report/commitment fields. This component is inert, not complete package
/// policy, a proof reconstruction, or acceptance. Private declarations remain
/// eligible: callback layout slots need not be part of the public API.
/// Lifetime ordinals refer to the explicitly supplied containing telescope;
/// the containing policy must retain that context. An unbound name is rejected.
pub fn project_checked_conformance_policy(
    compilation: &CheckedCompilation,
    application: &ClosedConformanceApplication,
    lifetime_binders: &[Identifier],
) -> Result<PackagePolicyClosedConformanceApplication, Vec<Diagnostic>> {
    let declaration = compilation
        .conformances()
        .iter()
        .find(|declaration| declaration.symbol == application.declaration)
        .ok_or_else(|| rejected("an application without its exact declaration"))?;
    let parameters = compilation
        .conformance_type_parameters(declaration)
        .to_vec();
    let lifetimes = lifetime_binders;
    if lifetimes
        .iter()
        .enumerate()
        .any(|(index, lifetime)| lifetimes[..index].contains(lifetime))
    {
        return Err(rejected(
            "a containing telescope with duplicate lifetime binders",
        ));
    }
    if declaration.trait_symbol != application.trait_definition {
        return Err(rejected("a target trait without exact declaration custody"));
    }
    let mut lifetime_arguments = Vec::with_capacity(application.lifetime_arguments.len());
    for name in &application.lifetime_arguments {
        let name = Identifier::generated(name);
        let position = lifetimes
            .iter()
            .position(|prior| prior == &name)
            .ok_or_else(|| rejected("a lifetime outside the containing telescope"))?;
        lifetime_arguments
            .push(u32::try_from(position).map_err(|_| rejected("too many lifetime arguments"))?);
    }
    let mut binders = Vec::new();
    argument_context(
        compilation,
        &application.arguments,
        lifetimes,
        &mut binders,
        0,
    )?;
    // Bound retained trees before reclosure clones or recursively renders them.
    validate_retained_application(compilation, application)?;
    let lifetime_substitutions = declaration
        .lifetime_parameters
        .iter()
        .cloned()
        .zip(
            application
                .lifetime_arguments
                .iter()
                .map(Identifier::generated),
        )
        .collect::<Vec<_>>();
    let mut instantiated = compilation.clone();
    let mut substitutions = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(application.arguments.iter()) {
        substitutions.push((
            parameter.symbol,
            argument_type_reference(&mut instantiated, argument, &parameter.kind, 0)?,
        ));
    }
    let subject_reference = match declaration.subject {
        ConformanceSubject::Subjectless => None,
        ConformanceSubject::Carrier(_) => {
            if !declaration.carrier_symbol.is_valid() {
                return Err(rejected("a carrier without exact declaration custody"));
            }
            Some(
                instantiated
                    .typed
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: declaration.carrier_symbol,
                        name: Identifier::generated(
                            compilation.symbols.name(declaration.carrier_symbol),
                        ),
                    }),
            )
        }
    };
    let project_type = |reference| {
        review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
            &instantiated,
            reference,
            &binders,
            lifetimes,
            &substitutions,
            &lifetime_substitutions,
        )
    };
    let mut type_arguments = Vec::with_capacity(application.type_arguments.len());
    for (parameter, (_, reference)) in parameters.iter().zip(&substitutions) {
        if matches!(parameter.kind, TypeParameterKind::Type) {
            type_arguments.push(project_type(*reference)?);
        }
    }
    let const_arguments = application
        .const_arguments
        .iter()
        .map(|argument| {
            Ok(match argument {
                ClosedConformanceConstArgument::Evaluated {
                    parameter_carrier,
                    declared_carrier,
                    value,
                } => {
                    psi_validation::validate_exact_const_value_encoding(
                        &compilation.typed,
                        *declared_carrier,
                        &value.encoding,
                    )
                    .map_err(|reason| {
                        rejected(&format!("an invalid exact const value: {reason}"))
                    })?;
                    PackagePolicyConformanceConstArgument::Evaluated {
                        parameter_carrier: project_type(*parameter_carrier)?,
                        declared_carrier: project_type(*declared_carrier)?,
                        canonical_value_encoding: value.encoding.clone(),
                    }
                }
                ClosedConformanceConstArgument::CallerBinder {
                    parameter_carrier,
                    binder,
                    binder_carrier,
                } => PackagePolicyConformanceConstArgument::CallerBinder {
                    parameter_carrier: project_type(*parameter_carrier)?,
                    binder: caller_binder_identity(compilation, *binder)?,
                    binder_carrier: project_type(*binder_carrier)?,
                },
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let trait_lifetime_arguments = application
        .trait_lifetime_arguments
        .iter()
        .map(|name| {
            lifetimes
                .iter()
                .position(|lifetime| lifetime.as_str() == name)
                .and_then(|position| u32::try_from(position).ok())
                .ok_or_else(|| rejected("a target-trait lifetime outside the exact application"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let trait_arguments = compilation
        .type_reference_table
        .type_reference_handles(declaration.arguments)
        .iter()
        .map(|reference| project_type(*reference))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PackagePolicyClosedConformanceApplication {
        declaration: nominal_identity(compilation, application.declaration)?,
        lifetime_arguments,
        type_arguments,
        const_arguments,
        machine_arguments: application
            .machine_arguments
            .iter()
            .map(|symbol| callable_identity(compilation, *symbol))
            .collect::<Result<Vec<_>, _>>()?,
        subject: subject_reference.map(project_type).transpose()?,
        trait_identity: nominal_identity(compilation, application.trait_definition)?,
        trait_lifetime_arguments,
        trait_arguments,
        rows: application
            .rows
            .iter()
            .map(|row| {
                Ok(PackagePolicyConformanceRow {
                    declaring_trait: nominal_identity(compilation, row.declaring_trait)?,
                    requirement: trait_requirement_identity_from_symbols(
                        compilation,
                        row.declaring_trait,
                        row.requirement,
                        "conformance policy row",
                    )?,
                    realization_machine: callable_identity(compilation, row.realization_machine)?,
                    realization_state: callable_identity(compilation, row.realization_state)?,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
    })
}

fn validate_retained_application(
    compilation: &CheckedCompilation,
    application: &ClosedConformanceApplication,
) -> Result<(), Vec<Diagnostic>> {
    let selected = StaticMachineArgument {
        path: Box::new([]),
        application: Some(Box::new(StaticSymbolApplication {
            lifetime_arguments: application
                .lifetime_arguments
                .iter()
                .map(Identifier::generated)
                .collect(),
            arguments: application.arguments.clone(),
        })),
        const_literal: None,
        evidence_projection: None,
        symbol: application.declaration,
    };
    // This reclassifies retained arguments against the same checked declaration
    // telescope. It executes no source code and consumes no proof certificate.
    let closed = psi_typed_trees_to_checked_trees::close_conformance_application(
        &compilation.typed,
        &selected,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    if closed.declaration != application.declaration
        || closed.lifetime_arguments != application.lifetime_arguments
        || closed.type_arguments != application.type_arguments
        || closed.const_arguments != application.const_arguments
        || closed.machine_arguments != application.machine_arguments
        || closed.subject_identity != application.subject_identity
        || closed.trait_definition != application.trait_definition
        || closed.trait_lifetime_arguments != application.trait_lifetime_arguments
        || closed.trait_arguments != application.trait_arguments
        || closed.rows != application.rows
    {
        return Err(rejected("stale declaration or argument custody"));
    }
    Ok(())
}

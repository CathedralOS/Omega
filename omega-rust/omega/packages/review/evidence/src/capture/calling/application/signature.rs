//! Exact source-side application of one published calling requirement.

mod arguments;
mod declaration;
mod inheritance;
mod parameters;
mod types;
pub(crate) use declaration::{declaration_parameters, project_declaration};
pub(crate) use parameters::instantiate as instantiate_static_parameters;

use super::rejected;
use crate::capture::semantics::declarations::{nominal_identity, trait_requirement_identity};
use crate::capture::semantics::signatures::policy::project_type_parameters;
use crate::capture::semantics::types::review_signature_type_identity_with_binders_and_substitutions_and_lifetimes;
use crate::record::{
    PackagePolicyTypeParameter, PackageReviewNominalIdentity,
    PackageReviewTraitRequirementParameter, PackageReviewTypeIdentity,
};
use omega_compiler::CheckedCompilation;
use omega_provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::name::Identifier;

pub(crate) struct CallingSignatureProjection {
    pub boundary_trait: PackageReviewNominalIdentity,
    pub boundary_arguments: Vec<PackageReviewTypeIdentity>,
    pub boundary_lifetime_parameter_count: u32,
    pub requirement: PackageReviewNominalIdentity,
    pub requirement_trait: PackageReviewNominalIdentity,
    pub requirement_arguments: Vec<PackageReviewTypeIdentity>,
    pub requirement_lifetime_arguments: Vec<u32>,
    pub requirement_lifetime_parameter_count: u32,
    pub static_parameters: Vec<PackagePolicyTypeParameter>,
    pub semantic_parameters: Vec<PackageReviewTraitRequirementParameter>,
    pub semantic_result: Option<PackageReviewTypeIdentity>,
    pub lifetime_binders: Vec<Identifier>,
}

pub(super) fn project(
    compilation: &CheckedCompilation,
    realization: &BoundaryCallingPlanRealization,
) -> Result<CallingSignatureProjection, Vec<Diagnostic>> {
    let projected = project_application(
        compilation,
        realization.boundary_trait,
        &realization.boundary_arguments,
        realization.requirement_machine,
    )?;
    if projected.requirement.path
        != realization
            .materialized_signature()
            .owner_requirement_identity()
    {
        return Err(rejected(
            "calling requirement overload differs from the materialized signature",
        ));
    }
    if projected.semantic_parameters.len()
        != realization.materialized_signature().parameters().len()
        || projected.semantic_result.is_some()
            != realization.materialized_signature().result().is_some()
    {
        return Err(rejected(
            "calling semantic signature differs from its materialized parameter or result telescope",
        ));
    }
    Ok(projected)
}

/// Project the semantic application without requiring any physical realization.
pub(crate) fn project_application(
    compilation: &CheckedCompilation,
    boundary_trait: psi_symbols::SymbolHandle,
    boundary_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    requirement_machine: psi_symbols::SymbolHandle,
) -> Result<CallingSignatureProjection, Vec<Diagnostic>> {
    project_with_binders(
        compilation,
        boundary_trait,
        boundary_arguments,
        requirement_machine,
        &[],
    )
}

fn project_with_binders(
    compilation: &CheckedCompilation,
    boundary_trait: psi_symbols::SymbolHandle,
    boundary_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    requirement_machine: psi_symbols::SymbolHandle,
    root_binders: &[(psi_symbols::SymbolHandle, String)],
) -> Result<CallingSignatureProjection, Vec<Diagnostic>> {
    let roots = compilation
        .traits()
        .iter()
        .filter(|owner| owner.symbol == boundary_trait && owner.is_boundary)
        .cloned()
        .collect::<Vec<_>>();
    let [root] = roots.as_slice() else {
        return Err(rejected(
            "calling application has no unique exact boundary trait",
        ));
    };
    let mut projected = compilation.clone();
    let root_lifetimes = root
        .lifetime_parameters
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    if root
        .lifetime_parameters
        .iter()
        .enumerate()
        .any(|(index, name)| root.lifetime_parameters[..index].contains(name))
    {
        return Err(rejected("boundary trait repeats a lifetime binder"));
    }
    let root_arguments = boundary_arguments
        .iter()
        .map(|argument| types::instantiate(&mut projected, *argument, &[], &root_lifetimes, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let mut inherited = Vec::new();
    inheritance::collect(
        &mut projected,
        inheritance::Application {
            owner: root.clone(),
            arguments: root_arguments.clone(),
            lifetime_arguments: root.lifetime_parameters.clone(),
            inherited_substitutions: Vec::new(),
        },
        requirement_machine,
        &mut Vec::new(),
        &mut inherited,
        root_binders,
    )?;
    if inherited.is_empty() {
        return Err(rejected(
            "calling requirement is not inherited by its selected boundary trait",
        ));
    }
    // Equal diamonds may reach the same declaration application repeatedly.
    // Compare structural semantic arguments, never arena allocation positions.
    let identities = inherited
        .iter()
        .map(|application| {
            Ok((
                application.owner.symbol,
                application.lifetime_arguments.clone(),
                arguments::project(
                    &projected,
                    &application.owner,
                    &application.arguments,
                    &application.inherited_substitutions,
                    &root.lifetime_parameters,
                    root_binders,
                )?,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    if identities.iter().any(|identity| identity != &identities[0]) {
        return Err(rejected(
            "calling requirement has differently instantiated inheritance paths",
        ));
    }
    let application = &inherited[0];
    let signatures = compilation
        .trait_machine_signatures(&application.owner)
        .iter()
        .filter(|signature| signature.symbol == requirement_machine)
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(rejected(
            "calling requirement has no exact declared signature",
        ));
    };
    let requirement = trait_requirement_identity(compilation, &application.owner, signature)?;
    let requirement_lifetime_arguments = application
        .lifetime_arguments
        .iter()
        .map(|name| {
            root.lifetime_parameters
                .iter()
                .position(|binder| binder == name)
                .and_then(|ordinal| u32::try_from(ordinal).ok())
                .ok_or_else(|| {
                    rejected("inherited requirement lifetime has no root telescope coordinate")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut substitutions = application.inherited_substitutions.clone();
    substitutions.extend(
        compilation
            .trait_type_parameters(&application.owner)
            .iter()
            .zip(&application.arguments)
            .filter(|(parameter, _)| {
                !root_binders
                    .iter()
                    .any(|(symbol, _)| *symbol == parameter.symbol)
            })
            .map(|(parameter, actual)| (parameter.symbol, *actual))
            .collect::<Vec<_>>(),
    );
    let mut lifetime_binders = root.lifetime_parameters.clone();
    let mut lifetimes = Vec::new();
    for (ordinal, name) in signature.lifetime_parameters.iter().enumerate() {
        if signature.lifetime_parameters[..ordinal].contains(name) {
            return Err(rejected("calling requirement repeats a method lifetime"));
        }
        let normalized = if lifetime_binders.contains(name) {
            Identifier::generated(format!("$calling-method-lifetime-{ordinal}"))
        } else {
            name.clone()
        };
        lifetime_binders.push(normalized.clone());
        lifetimes.push((name.clone(), normalized));
    }
    // Method lifetime names shadow trait names in the method's own signature.
    lifetimes.extend(
        application
            .owner
            .lifetime_parameters
            .iter()
            .cloned()
            .zip(application.lifetime_arguments.iter().cloned()),
    );
    let mut parameters = compilation
        .state_signature_type_parameters(signature)
        .to_vec();
    let mut contract_scopes = Vec::new();
    parameters::instantiate(
        &mut projected,
        &mut parameters,
        &substitutions,
        &lifetimes,
        &lifetime_binders,
        &mut contract_scopes,
        0,
    )?;
    let mut inherited_binders = root_binders.to_vec();
    inherited_binders.extend(
        substitutions
            .iter()
            .enumerate()
            .map(|(ordinal, (symbol, _))| (*symbol, format!("inherited-parameter:{ordinal}")))
            .collect::<Vec<_>>(),
    );
    let (binders, static_parameters) = project_type_parameters(
        &projected,
        compilation,
        &parameters,
        compilation.state_signature_type_parameters(signature),
        &requirement.path,
        &inherited_binders,
        0,
        &lifetime_binders,
        &substitutions,
        &contract_scopes,
        false,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    )?;
    let source_parameters = compilation
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut semantic_parameters = Vec::new();
    for parameter in source_parameters {
        let reference = types::instantiate(
            &mut projected,
            parameter.type_reference,
            &substitutions,
            &lifetimes,
            0,
        )?;
        semantic_parameters.push(PackageReviewTraitRequirementParameter {
            name: parameter.name.as_str().to_owned(),
            type_identity:
                review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                    &projected,
                    reference,
                    &binders,
                    &lifetime_binders,
                    &substitutions,
                    &[],
                )?,
            is_mutable: parameter.is_mutable,
            is_const: parameter.is_const,
            is_self: parameter.is_self,
        });
    }
    let semantic_result = if signature.return_type.is_valid() {
        let reference = types::instantiate(
            &mut projected,
            signature.return_type,
            &substitutions,
            &lifetimes,
            0,
        )?;
        Some(
            review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                &projected,
                reference,
                &binders,
                &lifetime_binders,
                &substitutions,
                &[],
            )?,
        )
    } else {
        None
    };
    Ok(CallingSignatureProjection {
        boundary_trait: nominal_identity(compilation, root.symbol)?,
        boundary_arguments: arguments::project(
            &projected,
            root,
            &root_arguments,
            &[],
            &root.lifetime_parameters,
            root_binders,
        )?,
        boundary_lifetime_parameter_count: count(root.lifetime_parameters.len())?,
        requirement,
        requirement_trait: nominal_identity(compilation, application.owner.symbol)?,
        requirement_arguments: identities[0].2.clone(),
        requirement_lifetime_arguments,
        requirement_lifetime_parameter_count: count(signature.lifetime_parameters.len())?,
        static_parameters,
        semantic_parameters,
        semantic_result,
        lifetime_binders,
    })
}

fn count(value: usize) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(value).map_err(|_| rejected("calling lifetime count exceeds u32"))
}

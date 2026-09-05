//! Scope-preserving public static telescopes and typed policy values.

pub(crate) mod values;
use crate::capture::calling::application::signature::instantiate_static_parameters;
use crate::capture::semantics::signatures::parameters::project_policy_type_parameters_after;
use crate::record::*;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::{
    data::{MachineParameterContract, TypeParameter, TypeParameterKind},
    name::Identifier,
};

/// Project the prepared signature but obtain semantic absence and proof-source
/// associations from the original checked telescope, never a dummy type string.
pub(crate) fn project_type_parameters(
    compilation: &CheckedCompilation,
    checked_source: &CheckedCompilation,
    parameters: &[TypeParameter],
    source_parameters: &[TypeParameter],
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    contract_scopes: &[super::parameters::CallingContractScope],
    public_nominals: bool,
    selection_exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackagePolicyTypeParameter>), Vec<Diagnostic>> {
    let (binders, projected) = super::parameters::project_policy_type_parameters(
        compilation,
        checked_source,
        parameters,
        declaration_path,
        preceding_binders,
        ordinal_offset,
        lifetime_binders,
        substitutions,
        contract_scopes,
        public_nominals,
        selection_exposure,
    )?;
    Ok((
        binders,
        convert_parameters(checked_source, source_parameters, projected, 0)?,
    ))
}

pub(crate) struct PreparedRequirement {
    pub compilation: CheckedCompilation,
    pub signature: psi_typed_trees::signature::StateSignature,
    pub lifetimes: Vec<Identifier>,
    pub scopes: Vec<crate::capture::semantics::signatures::parameters::CallingContractScope>,
}

pub(crate) fn requirement(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    source: &psi_typed_trees::signature::StateSignature,
    subject: &str,
    outer_binders: &[(SymbolHandle, String)],
    offset: usize,
    outer_lifetimes: &[Identifier],
) -> Result<
    (
        PreparedRequirement,
        Vec<(SymbolHandle, String)>,
        Vec<PackagePolicyTypeParameter>,
    ),
    Vec<Diagnostic>,
> {
    let mut projected = compilation.clone();
    let mut wrapper = [TypeParameter {
        symbol: owner,
        name: source.name.clone(),
        kind: TypeParameterKind::Machine {
            contract: MachineParameterContract::Structural(source.clone()),
        },
        bounds: Default::default(),
    }];
    let substitutions = outer_lifetimes
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    let mut scopes = Vec::new();
    instantiate_static_parameters(
        &mut projected,
        &mut wrapper,
        &[],
        &substitutions,
        outer_lifetimes,
        &mut scopes,
        0,
    )?;
    let [
        TypeParameter {
            kind:
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::Structural(signature),
                },
            ..
        },
    ] = wrapper
    else {
        return Err(rejected("requirement scope lost its structural signature"));
    };
    let mut lifetimes = outer_lifetimes.to_vec();
    lifetimes.extend(signature.lifetime_parameters.iter().cloned());
    let (binders, values) = project_policy_type_parameters_after(
        &projected,
        compilation,
        projected.state_signature_type_parameters(&signature),
        subject,
        outer_binders,
        offset,
        &lifetimes,
        &scopes,
    )?;
    let parameters = convert_parameters(
        compilation,
        compilation.state_signature_type_parameters(source),
        values,
        0,
    )?;
    Ok((
        PreparedRequirement {
            compilation: projected,
            signature,
            lifetimes,
            scopes,
        },
        binders,
        parameters,
    ))
}

pub(crate) fn parameters(
    compilation: &CheckedCompilation,
    source: &[TypeParameter],
    subject: &str,
    outer_binders: &[(SymbolHandle, String)],
    offset: usize,
    lifetimes: &[Identifier],
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackagePolicyTypeParameter>), Vec<Diagnostic>> {
    if source.is_empty() {
        return Ok((outer_binders.to_vec(), Vec::new()));
    }
    let mut projected = compilation.clone();
    let mut parameters = source.to_vec();
    let substitutions = lifetimes
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    let mut scopes = Vec::new();
    instantiate_static_parameters(
        &mut projected,
        &mut parameters,
        &[],
        &substitutions,
        lifetimes,
        &mut scopes,
        0,
    )?;
    let (binders, values) = project_policy_type_parameters_after(
        &projected,
        compilation,
        &parameters,
        subject,
        outer_binders,
        offset,
        lifetimes,
        &scopes,
    )?;
    Ok((binders, convert_parameters(compilation, source, values, 0)?))
}

fn convert_parameters(
    compilation: &CheckedCompilation,
    source: &[TypeParameter],
    values: Vec<PackageReviewTypeParameter>,
    depth: usize,
) -> Result<Vec<PackagePolicyTypeParameter>, Vec<Diagnostic>> {
    if depth >= 64 || source.len() != values.len() {
        return Err(rejected(
            "static policy telescope loses its exact source shape",
        ));
    }
    source
        .iter()
        .zip(values)
        .map(|(parameter, value)| {
            let kind = match (&parameter.kind, value.kind) {
                (TypeParameterKind::Type, PackageReviewTypeParameterKind::Type) => {
                    PackagePolicyTypeParameterKind::Type
                }
                (TypeParameterKind::Const { .. }, PackageReviewTypeParameterKind::Const(value)) => {
                    PackagePolicyTypeParameterKind::Const(value)
                }
                (
                    TypeParameterKind::Proposition { .. },
                    PackageReviewTypeParameterKind::Proposition(value),
                ) => PackagePolicyTypeParameterKind::Proposition(value),
                (
                    TypeParameterKind::Machine { contract },
                    PackageReviewTypeParameterKind::Machine(value),
                ) => PackagePolicyTypeParameterKind::Machine(machine(
                    compilation,
                    contract,
                    value,
                    depth + 1,
                )?),
                _ => {
                    return Err(rejected(
                        "static policy parameter changes its checked category",
                    ));
                }
            };
            Ok(PackagePolicyTypeParameter {
                kind,
                bounds: value.bounds,
            })
        })
        .collect()
}

fn machine(
    compilation: &CheckedCompilation,
    source: &MachineParameterContract,
    value: PackageReviewMachineParameterContract,
    depth: usize,
) -> Result<PackagePolicyMachineParameterContract, Vec<Diagnostic>> {
    match (source, value) {
        (
            MachineParameterContract::Structural(source),
            PackageReviewMachineParameterContract::Structural(value),
        ) => Ok(PackagePolicyMachineParameterContract::Structural(
            PackagePolicyMachineParameterSignature {
                lifetime_parameter_count: value.lifetime_parameter_count,
                type_parameters: convert_parameters(
                    compilation,
                    compilation.state_signature_type_parameters(source),
                    value.type_parameters,
                    depth,
                )?,
                parameters: value.parameters,
                return_type: source.return_type.is_valid().then_some(value.return_type),
                contracts: value.contracts,
                published_crash: values::crashes(value.published_crash)?,
                service_reach: value.service_reach,
                service_reach_is_installation_bound: value.service_reach_is_installation_bound,
                synchronous_invocations: value.synchronous_invocations,
                suspends: value.suspends,
                blocks: value.blocks,
                termination: values::termination(
                    compilation,
                    &source.termination_guarantee,
                    value.termination,
                )?,
            },
        )),
        (
            MachineParameterContract::Nominal { .. },
            PackageReviewMachineParameterContract::Nominal {
                trait_identity,
                requirement_identity,
            },
        ) => Ok(PackagePolicyMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        }),
        (
            MachineParameterContract::RequirementIdentity,
            PackageReviewMachineParameterContract::RequirementIdentity,
        ) => Ok(PackagePolicyMachineParameterContract::RequirementIdentity),
        _ => Err(rejected(
            "static machine policy loses its checked structural signature",
        )),
    }
}

fn rejected(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("policy signature: {message}"))]
}

use super::policy_crashes;

mod policy;
pub(crate) use policy::{project_policy_type_parameters, project_policy_type_parameters_after};

use super::super::super::behavior::{
    project_crash_routes, project_machine_parameter_termination, project_service_row,
    project_synchronous_invocations,
};
use super::super::super::contracts::facts::{ContractProjectionContext, project_contracts};
use super::super::declarations::{nominal_identity, trait_requirement_identity};
use super::super::types::project_data_properties;
use crate::capture::PackageReviewInput;
use crate::record::{
    PackageReviewCrashRoute, PackageReviewMachineParameterContract,
    PackageReviewMachineParameterSignature, PackageReviewMachineParameterValue,
    PackageReviewPropositionParameterSignature, PackageReviewPropositionParameterValue,
    PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
struct Projection<'a> {
    public_nominals: bool,
    policy_crash_guards: bool,
    selection_exposure: AuthoredDeclarationSelectionExposure,
    substitutions: &'a [(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    contract_scopes: Option<&'a [CallingContractScope]>,
}

/// Temporary source custody for expressions in a clone-instantiated contract.
pub(crate) struct CallingContractScope {
    pub parameter_symbol: SymbolHandle,
    pub signature_symbol: SymbolHandle,
    /// Outer-first; the last matching source name is the lexical binder.
    pub lifetime_substitutions: Vec<(typed_trees::name::Identifier, typed_trees::name::Identifier)>,
    pub parameters: Vec<typed_trees::signature::StateParameter>,
    pub original_signature: typed_trees::signature::StateSignature,
}

impl Projection<'_> {
    fn value_type(
        self,
        typed: &typed_trees::TypedTrees,
        compilation: &PackageReviewInput<'_>,
        reference: typed_trees::types::TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        lifetimes: &[typed_trees::name::Identifier],
    ) -> Result<crate::record::PackageReviewTypeIdentity, Vec<Diagnostic>> {
        super::super::types::signature_type_identity(
            typed,
            compilation.custody.exact_toolchain_sources(),
            reference,
            binders,
            lifetimes,
            self.substitutions,
            &[],
            false,
        )
    }
}

pub(crate) fn project_type_parameters(
    compilation: &PackageReviewInput<'_>,
    parameters: &[typed_trees::data::TypeParameter],
    declaration_kind: &str,
    declaration_path: &str,
    lifetime_binders: &[typed_trees::name::Identifier],
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
    compilation: &PackageReviewInput<'_>,
    parameters: &[typed_trees::data::TypeParameter],
    declaration_kind: &str,
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[typed_trees::name::Identifier],
    depth: usize,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_type_parameters_inner(
        &compilation.typed,
        compilation,
        parameters,
        declaration_kind,
        declaration_path,
        preceding_binders,
        ordinal_offset,
        lifetime_binders,
        depth,
        Projection {
            public_nominals: true,
            policy_crash_guards: false,
            selection_exposure: AuthoredDeclarationSelectionExposure::PublicInterface,
            substitutions: &[],
            contract_scopes: None,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn project_type_parameters_inner(
    typed: &typed_trees::TypedTrees,
    compilation: &PackageReviewInput<'_>,
    parameters: &[typed_trees::data::TypeParameter],
    declaration_kind: &str,
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
    lifetime_binders: &[typed_trees::name::Identifier],
    depth: usize,
    projection: Projection<'_>,
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
            typed_trees::data::TypeParameterKind::Type => PackageReviewTypeParameterKind::Type,
            typed_trees::data::TypeParameterKind::Const { type_reference } => {
                PackageReviewTypeParameterKind::Const(projection.value_type(
                    typed,
                    compilation,
                    *type_reference,
                    &binders,
                    lifetime_binders,
                )?)
            }
            typed_trees::data::TypeParameterKind::Value { type_reference } => {
                PackageReviewTypeParameterKind::Value(projection.value_type(
                    typed,
                    compilation,
                    *type_reference,
                    &binders,
                    lifetime_binders,
                )?)
            }
            typed_trees::data::TypeParameterKind::Machine { contract } => {
                PackageReviewTypeParameterKind::Machine(project_machine_parameter_contract_inner(
                    typed,
                    compilation,
                    parameter.symbol,
                    contract,
                    declaration_kind,
                    declaration_path,
                    &binders,
                    ordinal_offset + parameters.len(),
                    lifetime_binders,
                    depth + 1,
                    projection,
                )?)
            }
            typed_trees::data::TypeParameterKind::Proposition { contract } => {
                let mut projected_parameters = Vec::new();
                for value_parameter in typed.state_parameters.span_or_empty(contract.parameters) {
                    projected_parameters.push(PackageReviewPropositionParameterValue {
                        type_identity: projection.value_type(
                            typed,
                            compilation,
                            value_parameter.type_reference,
                            &binders,
                            lifetime_binders,
                        )?,
                        is_const: value_parameter.is_const,
                        is_mutable: value_parameter.is_mutable,
                        is_self: value_parameter.is_self,
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
            bounds: project_data_properties(parameter.bounds),
        });
    }
    Ok((binders, projected))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn project_machine_parameter_contract(
    compilation: &PackageReviewInput<'_>,
    parameter_symbol: SymbolHandle,
    contract: &typed_trees::data::MachineParameterContract,
    declaration_kind: &str,
    declaration_path: &str,
    outer_binders: &[(SymbolHandle, String)],
    nested_ordinal_offset: usize,
    outer_lifetime_binders: &[typed_trees::name::Identifier],
    depth: usize,
) -> Result<PackageReviewMachineParameterContract, Vec<Diagnostic>> {
    project_machine_parameter_contract_inner(
        &compilation.typed,
        compilation,
        parameter_symbol,
        contract,
        declaration_kind,
        declaration_path,
        outer_binders,
        nested_ordinal_offset,
        outer_lifetime_binders,
        depth,
        Projection {
            public_nominals: true,
            policy_crash_guards: false,
            selection_exposure: AuthoredDeclarationSelectionExposure::PublicInterface,
            substitutions: &[],
            contract_scopes: None,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn project_machine_parameter_contract_inner(
    typed: &typed_trees::TypedTrees,
    compilation: &PackageReviewInput<'_>,
    parameter_symbol: SymbolHandle,
    contract: &typed_trees::data::MachineParameterContract,
    declaration_kind: &str,
    declaration_path: &str,
    outer_binders: &[(SymbolHandle, String)],
    nested_ordinal_offset: usize,
    outer_lifetime_binders: &[typed_trees::name::Identifier],
    depth: usize,
    projection: Projection<'_>,
) -> Result<PackageReviewMachineParameterContract, Vec<Diagnostic>> {
    match contract {
        typed_trees::data::MachineParameterContract::RequirementIdentity => {
            Ok(PackageReviewMachineParameterContract::RequirementIdentity)
        }
        typed_trees::data::MachineParameterContract::Structural(signature) => {
            if signature.spelling.is_some() || signature.is_default {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` has a structural static-machine contract with trait-only requirement metadata",
                ))]);
            }
            let mut lifetime_binders = outer_lifetime_binders.to_vec();
            lifetime_binders.extend(signature.lifetime_parameters.iter().cloned());
            let (binders, type_parameters) = project_type_parameters_inner(
                typed,
                compilation,
                typed.state_signature_type_parameters(signature),
                declaration_kind,
                declaration_path,
                outer_binders,
                nested_ordinal_offset,
                &lifetime_binders,
                depth,
                projection,
            )?;
            let parameters = typed.state_signature_parameters(signature);
            let mut scopes = projection
                .contract_scopes
                .unwrap_or(&[])
                .iter()
                .filter(|scope| {
                    scope.parameter_symbol == parameter_symbol
                        && scope.signature_symbol == signature.symbol
                });
            let scope = scopes.next();
            if scopes.next().is_some() || (projection.contract_scopes.is_some() && scope.is_none())
            {
                return Err(vec![Diagnostic::error(
                    "calling static contract has no unique exact source scope",
                )]);
            }
            let checked_source = compilation;
            let source_signature = scope.map_or(signature, |scope| &scope.original_signature);
            let context = ContractProjectionContext {
                subject_kind: "public static-machine parameter",
                subject_name: declaration_path,
                owner: checked_trees::ContractProofFactOwner::StateSignature {
                    owner_symbol: parameter_symbol,
                    state_symbol: signature.symbol,
                },
                point: facts::ProgramPoint::State {
                    machine_symbol: parameter_symbol,
                    state_symbol: signature.symbol,
                },
                parameters: scope.map_or(parameters, |scope| scope.parameters.as_slice()),
                domain_symbol: None,
                data_symbol: None,
                lifetime_binders: &lifetime_binders,
                lifetime_substitutions: scope
                    .map_or(&[], |scope| scope.lifetime_substitutions.as_slice()),
                selection_exposure: projection.selection_exposure,
            };
            let contracts = project_contracts(
                checked_source,
                checked_source.state_signature_contracts(source_signature),
                &context,
                &binders,
            )?;
            let published_crash = if projection.policy_crash_guards {
                policy_crashes::project(
                    checked_source,
                    parameter_symbol,
                    source_signature,
                    &context,
                    &binders,
                )?
            } else {
                project_signature_crash_routes(
                    compilation,
                    parameter_symbol,
                    signature.symbol,
                    "public static-machine parameter",
                    declaration_path,
                )?
            };
            Ok(PackageReviewMachineParameterContract::Structural(
                PackageReviewMachineParameterSignature {
                    lifetime_parameter_count: signature.lifetime_parameters.len(),
                    type_parameters,
                    parameters: parameters
                        .iter()
                        .map(|parameter| {
                            Ok(PackageReviewMachineParameterValue {
                                name: parameter.name.as_str().to_owned(),
                                type_identity: projection.value_type(
                                    typed,
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
                    return_type: projection.value_type(
                        typed,
                        compilation,
                        signature.return_type,
                        &binders,
                        &lifetime_binders,
                    )?,
                    contracts,
                    published_crash,
                    service_reach: project_service_row(
                        compilation,
                        source_signature.service_reach_row,
                    )?,
                    service_reach_is_installation_bound: source_signature
                        .service_reach_is_installation_bound,
                    synchronous_invocations: project_synchronous_invocations(
                        compilation,
                        &validation::declared_signature_invocations(compilation, source_signature),
                    )?,
                    suspends: source_signature.suspends,
                    blocks: source_signature.blocks,
                    termination: project_machine_parameter_termination(
                        compilation,
                        source_signature,
                        declaration_path,
                    )?,
                },
            ))
        }
        typed_trees::data::MachineParameterContract::Nominal {
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
            if projection.public_nominals && !trait_definition.is_public {
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
    compilation: &PackageReviewInput<'_>,
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

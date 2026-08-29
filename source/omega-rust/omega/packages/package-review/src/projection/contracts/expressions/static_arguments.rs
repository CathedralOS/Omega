use super::names::portable_parameter_position;
use crate::evidence::{PackageReviewContractStaticArgument, PackageReviewTypeIdentity};
use crate::projection::contracts::metadata::contracts::ContractProjectionContext;
use crate::projection::exact_identity::lifetime_identities::lifetime_binder_ordinal;
use crate::projection::exact_identity::nominal_identities::nominal_identity;
use crate::projection::exact_identity::type_identities::missing_exact_toolchain_type_owner;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractCallStaticParameterKind {
    Type,
    Const,
    Machine,
    Proposition,
}

pub(crate) fn contract_call_static_parameter_kind(
    parameter: &psi_typed_trees::data::TypeParameter,
) -> ContractCallStaticParameterKind {
    match parameter.kind {
        psi_typed_trees::data::TypeParameterKind::Type => ContractCallStaticParameterKind::Type,
        psi_typed_trees::data::TypeParameterKind::Const { .. } => {
            ContractCallStaticParameterKind::Const
        }
        psi_typed_trees::data::TypeParameterKind::Machine { .. } => {
            ContractCallStaticParameterKind::Machine
        }
        psi_typed_trees::data::TypeParameterKind::Proposition { .. } => {
            ContractCallStaticParameterKind::Proposition
        }
    }
}

pub(crate) fn contract_call_static_parameter_kinds(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    target: SymbolHandle,
    supplied_count: usize,
) -> Result<Vec<ContractCallStaticParameterKind>, Vec<Diagnostic>> {
    let project = |parameters: &[psi_typed_trees::data::TypeParameter]| {
        parameters
            .iter()
            .map(contract_call_static_parameter_kind)
            .collect::<Vec<_>>()
    };
    let mut candidates = compilation
        .machines()
        .iter()
        .filter(|machine| {
            compilation
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == target)
        })
        .map(|machine| project(compilation.machine_type_parameters(machine)))
        .collect::<Vec<_>>();
    if let Some((_, signature)) = compilation.machine_parameter_signature(target) {
        candidates.push(project(
            compilation.state_signature_type_parameters(signature),
        ));
    }
    candidates.extend(compilation.traits().iter().flat_map(|definition| {
        compilation
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == target)
            .map(|signature| project(compilation.state_signature_type_parameters(signature)))
    }));
    let [parameter_kinds] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call target rejoins {} static telescopes; expected exactly one",
            context.subject_kind,
            context.subject_name,
            candidates.len()
        ))]);
    };
    if parameter_kinds.len() != supplied_count {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call supplies {supplied_count} static arguments for a checked telescope of {} parameters",
            context.subject_kind,
            context.subject_name,
            parameter_kinds.len()
        ))]);
    }
    Ok(parameter_kinds.clone())
}

pub(crate) fn project_contract_static_argument(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    project_static_argument(
        compilation,
        context.subject_kind,
        context.subject_name,
        binders,
        context.lifetime_binders,
        argument,
        parameter_kind,
        depth,
    )
}

pub(crate) fn project_static_argument(
    compilation: &CheckedCompilation,
    subject_kind: &str,
    subject_name: &str,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed {subject_kind} `{subject_name}` uses a static argument {reason}",
        ))]
    };
    if depth >= 64 {
        return Err(rejected(
            "whose nested application exceeds the package-review depth limit",
        ));
    }
    if argument.evidence_projection.is_some() {
        return Err(rejected(
            "from an evidence projection not yet represented by package review",
        ));
    }
    if parameter_kind == ContractCallStaticParameterKind::Proposition {
        return Err(rejected(
            "for a proposition parameter not yet represented by package review",
        ));
    }
    if let Some(application) = argument.application.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Type
            || !argument.symbol.is_valid()
            || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::Data
        {
            return Err(rejected(
                "with a non-data nested static application not yet represented by package review",
            ));
        }
        let definitions = compilation
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == argument.symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(rejected(
                "whose generic data base does not rejoin exactly one checked declaration",
            ));
        };
        if definition.lifetime_parameters.len() != application.lifetime_arguments.len() {
            return Err(rejected(
                "whose lifetime argument count differs from its checked data declaration",
            ));
        }
        let parameters = compilation.data_type_parameters(definition);
        if parameters.len() != application.arguments.len() {
            return Err(rejected(
                "whose generic data argument count differs from its checked telescope",
            ));
        }
        let base = compilation
            .package_qualified_nominal_type_identity_with_toolchain_sources(
                argument.symbol,
                compilation.exact_toolchain_sources(),
            )
            .ok_or_else(missing_exact_toolchain_type_owner)?;
        let arguments = application
            .arguments
            .iter()
            .zip(parameters)
            .map(|(argument, parameter)| {
                project_static_argument(
                    compilation,
                    subject_kind,
                    subject_name,
                    binders,
                    lifetime_binders,
                    argument,
                    contract_call_static_parameter_kind(parameter),
                    depth + 1,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let lifetime_arguments = application
            .lifetime_arguments
            .iter()
            .map(|lifetime| {
                lifetime_binder_ordinal(lifetime, lifetime_binders, "contract-call nested type")
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(PackageReviewContractStaticArgument::GenericType {
            base: PackageReviewTypeIdentity {
                canonical: base.into_string(),
            },
            lifetime_arguments,
            arguments,
        });
    }
    if let Some(literal) = argument.const_literal.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Const {
            return Err(rejected(
                "whose category differs from its checked telescope slot",
            ));
        }
        return Ok(PackageReviewContractStaticArgument::ConstInteger(
            literal.text().to_owned(),
        ));
    }
    if let Some(position) = binders
        .iter()
        .position(|(symbol, _)| *symbol == argument.symbol)
    {
        let position = portable_parameter_position(position)?;
        return match compilation.typed.symbols.get(argument.symbol).kind {
            psi_symbols::SymbolKind::MachineParameter
                if parameter_kind == ContractCallStaticParameterKind::Machine =>
            {
                Ok(PackageReviewContractStaticArgument::GenericMachineBinder(
                    position,
                ))
            }
            psi_symbols::SymbolKind::TypeParameter => {
                let matching = compilation
                    .typed
                    .data_type_parameters
                    .iter()
                    .map(|(_, parameter)| parameter)
                    .filter(|parameter| parameter.symbol == argument.symbol)
                    .collect::<Vec<_>>();
                let [parameter] = matching.as_slice() else {
                    return Err(rejected(
                        "that does not rejoin exactly one checked caller parameter",
                    ));
                };
                match (&parameter.kind, parameter_kind) {
                    (
                        psi_typed_trees::data::TypeParameterKind::Type,
                        ContractCallStaticParameterKind::Type,
                    ) => Ok(PackageReviewContractStaticArgument::GenericTypeBinder(
                        position,
                    )),
                    (
                        psi_typed_trees::data::TypeParameterKind::Const { .. },
                        ContractCallStaticParameterKind::Const,
                    ) => Ok(PackageReviewContractStaticArgument::GenericConstBinder(
                        position,
                    )),
                    _ => Err(rejected(
                        "whose category differs from its checked caller and callee telescope slots",
                    )),
                }
            }
            _ => Err(rejected(
                "whose category differs from its checked caller and callee telescope slots",
            )),
        };
    }
    if parameter_kind == ContractCallStaticParameterKind::Type {
        if !argument.symbol.is_valid()
            || !matches!(
                compilation.typed.symbols.get(argument.symbol).kind,
                psi_symbols::SymbolKind::BuiltinType | psi_symbols::SymbolKind::Data
            )
        {
            return Err(rejected(
                "whose category differs from its checked type slot",
            ));
        }
        let identity = compilation
            .package_qualified_nominal_type_identity_with_toolchain_sources(
                argument.symbol,
                compilation.exact_toolchain_sources(),
            )
            .ok_or_else(missing_exact_toolchain_type_owner)?;
        return Ok(PackageReviewContractStaticArgument::Type(
            PackageReviewTypeIdentity {
                canonical: identity.into_string(),
            },
        ));
    }
    if parameter_kind == ContractCallStaticParameterKind::Const {
        return Err(rejected(
            "from a forwarded or symbolic const not yet represented by package review",
        ));
    }
    if !argument.symbol.is_valid()
        || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::State
    {
        return Err(rejected(
            "whose category differs from its checked machine slot",
        ));
    }
    let matching_states = compilation
        .machines()
        .iter()
        .filter_map(|machine| compilation.machine_states(machine).first())
        .filter(|entry| entry.symbol == argument.symbol)
        .count();
    if matching_states != 1 {
        return Err(rejected(
            "that does not rejoin exactly one checked concrete machine entry",
        ));
    }
    Ok(PackageReviewContractStaticArgument::ConcreteMachine(
        nominal_identity(compilation, argument.symbol)?,
    ))
}

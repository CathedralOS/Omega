mod conformance;
mod const_values;

use super::names::portable_parameter_position;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::lifetimes::lifetime_binder_ordinal;
use crate::capture::semantics::types::missing_exact_toolchain_type_owner;
use crate::record::{PackageReviewContractStaticArgument, PackageReviewTypeIdentity};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

use conformance::project_contract_conformance_application;
pub(crate) use conformance::require_exact_conformance_static_argument_selections;
use const_values::project_named_const_static_argument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractCallStaticParameterKind {
    Type,
    Const,
    Machine,
    Conformance,
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
    let project_machine = |machine: &psi_typed_trees::machine::Machine| {
        let mut kinds = project(compilation.machine_type_parameters(machine));
        kinds.extend(
            machine
                .conformance_bounds
                .iter()
                .filter(|bound| bound.binder.is_some())
                .map(|_| ContractCallStaticParameterKind::Conformance),
        );
        kinds
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
        .map(project_machine)
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
    candidates.extend(
        compilation
            .operators()
            .iter()
            .chain(
                compilation
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| compilation.domain_operators(domain)),
            )
            .filter(|operator| operator.symbol == target)
            .map(|operator| project(compilation.operator_type_parameters(operator))),
    );
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
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    static_argument_position: usize,
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    if argument.symbol.is_valid()
        && compilation.typed.symbols.get(argument.symbol).kind
            == psi_symbols::SymbolKind::Conformance
    {
        return project_contract_conformance_application(
            compilation,
            context,
            binders,
            checked_fact,
            expression,
            static_argument_position,
            argument,
            parameter_kind,
            depth,
        );
    }
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

pub(crate) fn require_exact_named_const_static_argument_selections(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    arguments: &[psi_typed_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    fn collect_argument_consts(
        compilation: &CheckedCompilation,
        arguments: &[psi_typed_trees::expression::StaticMachineArgument],
        selected: &mut Vec<SymbolHandle>,
    ) {
        for argument in arguments {
            if argument.symbol.is_valid()
                && compilation.typed.symbols.get(argument.symbol).kind
                    == psi_symbols::SymbolKind::Const
            {
                selected.push(argument.symbol);
            }
            if let Some(application) = &argument.application {
                collect_argument_consts(compilation, &application.arguments, selected);
            }
        }
    }

    let mut argument_consts = Vec::new();
    collect_argument_consts(compilation, arguments, &mut argument_consts);
    let mut selected_consts = Vec::new();
    for occurrence in compilation
        .expression_table
        .authored_selection_occurrences(expression)
    {
        let Some(selection) = compilation
            .authored_declaration_selections()
            .get(occurrence)
        else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` retains an unknown static-argument selection occurrence",
                context.subject_kind, context.subject_name
            ))]);
        };
        if selection.kind() != AuthoredDeclarationSelectionKind::StaticArgument {
            continue;
        }
        let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
            continue;
        };
        if compilation.typed.symbols.get(target.selected_symbol()).kind
            != psi_symbols::SymbolKind::Const
        {
            continue;
        }
        if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` named const is not retained as a public-interface static-argument selection",
                context.subject_kind, context.subject_name
            ))]);
        }
        selected_consts.push(target.selected_symbol());
    }
    if selected_consts != argument_consts {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named const arguments do not match their exact authored static-argument selections",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(())
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
        if argument.symbol.is_valid()
            && compilation.typed.symbols.get(argument.symbol).kind == psi_symbols::SymbolKind::Const
        {
            let declarations = compilation
                .const_declarations()
                .iter()
                .filter(|declaration| declaration.symbol == argument.symbol)
                .collect::<Vec<_>>();
            let [declaration] = declarations.as_slice() else {
                return Err(rejected(
                    "whose selected const does not rejoin exactly one checked declaration",
                ));
            };
            return project_named_const_static_argument(
                compilation,
                subject_kind,
                subject_name,
                declaration,
            );
        }
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

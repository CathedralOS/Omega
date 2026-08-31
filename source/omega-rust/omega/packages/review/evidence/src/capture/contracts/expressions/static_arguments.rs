use super::names::portable_parameter_position;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::lifetimes::lifetime_binder_ordinal;
use crate::capture::semantics::types::missing_exact_toolchain_type_owner;
use crate::record::{PackageReviewContractStaticArgument, PackageReviewTypeIdentity};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
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
            let Some(encoding) = declaration.canonical_value_encoding.as_deref() else {
                return Err(rejected(
                    "whose selected const has no admitted canonical public value",
                ));
            };
            if let Some(value) = canonical_integer_value(encoding) {
                return Ok(PackageReviewContractStaticArgument::ConstInteger(value));
            }
            return match decode_canonical_const_value(encoding) {
                Some(DecodedCanonicalConstValue::Boolean(value))
                    if exact_boolean_type(compilation, declaration.declared_type) =>
                {
                    Ok(PackageReviewContractStaticArgument::ConstBoolean(value))
                }
                _ => Err(rejected(
                    "whose selected const is not a supported canonical value for its exact declared carrier",
                )),
            };
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

/// Recover the canonical decimal payload from the closed public-const
/// encoding. The encoder is length-delimited, so this never interprets source
/// text or a diagnostic display as semantic identity.
fn canonical_integer_value(encoding: &str) -> Option<String> {
    let DecodedCanonicalConstValue::Integer { type_name, value } =
        decode_canonical_const_value(encoding)?
    else {
        return None;
    };
    canonical_integer_type_name(type_name.as_str()).then(|| value.to_string())
}

fn decode_canonical_const_value(encoding: &str) -> Option<DecodedCanonicalConstValue> {
    CanonicalConstValue::new("", encoding, "").decode_encoding()
}

fn canonical_integer_type_name(type_name: &str) -> bool {
    matches!(
        type_name,
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "addr"
    )
}

fn exact_boolean_type(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } = compilation
        .typed
        .type_reference_table
        .type_reference(type_reference)
    else {
        return false;
    };
    name.as_str() == "bool"
        && compilation.typed.symbols.builtin_type_atom(*symbol)
            == Some(psi_symbols::BuiltinTypeAtom::Bool)
}

#[cfg(test)]
mod tests {
    use super::canonical_integer_value;

    #[test]
    fn canonical_integer_value_decodes_only_closed_integer_encodings() {
        assert_eq!(
            canonical_integer_value("integer3:u642:42"),
            Some("42".to_owned())
        );
        assert_eq!(
            canonical_integer_value("integer3:i642:-1"),
            Some("-1".to_owned())
        );
        assert_eq!(canonical_integer_value("integer3:u642:07"), None);
        assert_eq!(canonical_integer_value("boolean4:true"), None);
        assert_eq!(canonical_integer_value("integer3:u641:7tail"), None);
    }
}

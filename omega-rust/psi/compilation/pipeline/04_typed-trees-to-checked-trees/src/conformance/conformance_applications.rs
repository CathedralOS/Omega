//! Closure and validation of name-owned generic conformance applications.
//!
//! The call syntax carries a recursively delimited static application. This
//! module classifies its declaration-owned telescope and publishes an
//! argument-sensitive semantic identity.

use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use sha2::{Digest, Sha256};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use typed_trees::references::{StaticArgumentCall, StaticArgumentRole};
use typed_trees::statement::StatementNode;
use typed_trees::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceConstArgument, ClosedConformanceRowIdentity,
};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_conformance_applications(
    program: &TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            validate_call_arguments(
                program,
                StaticArgumentCall::Expression(call),
                &mut diagnostics,
            );
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement {
                    validate_call_arguments(
                        program,
                        StaticArgumentCall::Statement(call),
                        &mut diagnostics,
                    );
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_call_arguments(
    program: &TypedTrees,
    call: StaticArgumentCall<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let declarations = match crate::authored_selections::static_argument_role(program, call) {
        StaticArgumentRole::DeclarationDescription => program.product_declarations(),
        StaticArgumentRole::ValueDependency => program,
    };
    validate_arguments(declarations, call.arguments(), diagnostics);
}

fn validate_arguments(
    program: &TypedTrees,
    arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for argument in arguments {
        if matches!(
            program.symbols.get(argument.symbol).kind,
            SymbolKind::Conformance
        ) && let Err(diagnostic) = close_conformance_application(program, argument)
        {
            diagnostics.push(diagnostic);
        }
        if let Some(application) = &argument.application {
            validate_arguments(program, &application.arguments, diagnostics);
        }
    }
}

pub fn close_conformance_application(
    program: &TypedTrees,
    selected: &StaticMachineArgument,
) -> Result<ClosedConformanceApplication, Diagnostic> {
    let Some(conformance) = program
        .conformances()
        .iter()
        .find(|candidate| candidate.symbol == selected.symbol)
    else {
        return Err(Diagnostic::error(format!(
            "static argument `{}` does not name a package-scoped conformance",
            selected.display_name()
        )));
    };
    let declaration_name = conformance
        .alias
        .as_ref()
        .map_or("<unnamed-conformance>", |name| name.as_str());
    let parameters = program.conformance_type_parameters(conformance);
    let supplied = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| application.arguments.as_ref());
    if parameters.len() != supplied.len() {
        return Err(Diagnostic::error(format!(
            "generic conformance `{declaration_name}` requires {} explicit non-lifetime argument(s), got {}; expected subject and trait shape never fill its own telescope",
            parameters.len(),
            supplied.len()
        )));
    }

    let lifetime_arguments = match &selected.application {
        Some(application)
            if application.lifetime_arguments.len() == conformance.lifetime_parameters.len() =>
        {
            application
                .lifetime_arguments
                .iter()
                .map(|lifetime| lifetime.as_str().to_owned())
                .collect::<Vec<_>>()
        }
        Some(application) if !application.lifetime_arguments.is_empty() => {
            return Err(Diagnostic::error(format!(
                "generic conformance `{declaration_name}` requires {} lifetime argument(s), got {}",
                conformance.lifetime_parameters.len(),
                application.lifetime_arguments.len()
            )));
        }
        _ if conformance.lifetime_parameters.is_empty() => Vec::new(),
        _ => {
            return Err(Diagnostic::error(format!(
                "generic conformance `{declaration_name}` has {} erased lifetime parameter(s), but this application supplies none and no unique ordinary borrow constraint is available; write the lifetime argument explicitly",
                conformance.lifetime_parameters.len()
            )));
        }
    };

    let mut substitutions = Vec::<(SymbolHandle, String)>::new();
    let mut type_arguments = Vec::new();
    let mut const_arguments = Vec::new();
    let mut machine_arguments = Vec::new();
    for (parameter, argument) in parameters.iter().zip(supplied) {
        match &parameter.kind {
            TypeParameterKind::Type => {
                validate_type_argument(
                    program,
                    declaration_name,
                    parameter.name.as_str(),
                    argument,
                )?;
                let identity = type_argument_identity(program, argument);
                substitutions.push((parameter.symbol, identity.clone()));
                type_arguments.push(identity);
            }
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                let (argument, substitution_identity) = close_const_argument(
                    program,
                    declaration_name,
                    parameter.name.as_str(),
                    *type_reference,
                    argument,
                )?;
                substitutions.push((parameter.symbol, substitution_identity));
                const_arguments.push(argument);
            }
            TypeParameterKind::Machine { .. } => {
                if !matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::State | SymbolKind::MachineParameter
                ) {
                    return Err(category_error(
                        declaration_name,
                        parameter.name.as_str(),
                        "static machine",
                        argument,
                    ));
                }
                substitutions.push((
                    parameter.symbol,
                    static_argument_identity(program, argument),
                ));
                machine_arguments.push(argument.symbol);
            }
            TypeParameterKind::Proposition { .. } => {
                return Err(Diagnostic::error(format!(
                    "conformance `{declaration_name}` retains an unsupported proposition parameter `{}` in its name-owned telescope",
                    parameter.name
                )));
            }
        }
    }

    // The conformance's bound trait symbol is exact; a name match would grab
    // a same-named trait from a different checked instance (toolchain mounts
    // are per-package instances, so a foreign conformance can carry a
    // different `OpaqueRepresentation` handle than the root's first-by-name).
    let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == conformance.trait_symbol)
        .or_else(|| {
            program
                .traits()
                .iter()
                .find(|definition| definition.name == conformance.trait_name)
        })
    else {
        return Err(Diagnostic::error(format!(
            "conformance `{declaration_name}` names unresolved trait `{}`",
            conformance.trait_name
        )));
    };
    let trait_arguments = program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .iter()
        .map(|argument| {
            substituted_type_identity_with_lifetimes(
                program,
                *argument,
                &substitutions,
                &conformance
                    .lifetime_parameters
                    .iter()
                    .zip(&lifetime_arguments)
                    .map(|(parameter, argument)| (parameter.as_str().to_owned(), argument.clone()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let trait_lifetime_arguments = conformance
        .trait_lifetime_arguments
        .iter()
        .map(|ordinal| {
            usize::try_from(*ordinal)
                .ok()
                .and_then(|ordinal| lifetime_arguments.get(ordinal))
                .cloned()
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "conformance `{declaration_name}` target-trait lifetime falls outside its closed application"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rows = conformance.rows.as_slice();
    let row_identities = rows
        .iter()
        .map(|row| ClosedConformanceRowIdentity {
            declaring_trait: row.declaring_trait,
            requirement: row.requirement,
            realization_machine: row.realization_machine,
            realization_state: row.realization_state,
        })
        .collect::<Vec<_>>();
    let identity = application_identity(
        program,
        declaration_name,
        &lifetime_arguments,
        &type_arguments,
        &const_arguments,
        &machine_arguments,
        trait_definition.name.as_str(),
        &trait_lifetime_arguments,
        &trait_arguments,
        rows,
    );
    Ok(ClosedConformanceApplication {
        declaration: conformance.symbol,
        arguments: supplied.into(),
        lifetime_arguments,
        type_arguments,
        const_arguments,
        machine_arguments,
        trait_definition: trait_definition.symbol,
        trait_lifetime_arguments,
        trait_arguments,
        rows: row_identities,
        report_fingerprint: identity.report_fingerprint,
        commitment: identity.commitment,
    })
}

fn validate_type_argument(
    program: &TypedTrees,
    conformance: &str,
    parameter: &str,
    argument: &StaticMachineArgument,
) -> Result<(), Diagnostic> {
    if argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
        || !matches!(
            program.symbols.get(argument.symbol).kind,
            SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
        )
    {
        return Err(category_error(conformance, parameter, "type", argument));
    }
    Ok(())
}

fn category_error(
    conformance: &str,
    parameter: &str,
    category: &str,
    argument: &StaticMachineArgument,
) -> Diagnostic {
    Diagnostic::error(format!(
        "conformance `{conformance}` parameter `{parameter}` requires a {category} argument, but `{}` has another static category",
        argument.display_name()
    ))
}

fn close_const_argument(
    program: &TypedTrees,
    conformance: &str,
    parameter: &str,
    parameter_carrier: TypeReferenceHandle,
    argument: &StaticMachineArgument,
) -> Result<(ClosedConformanceConstArgument, String), Diagnostic> {
    if let Some(literal) = argument.const_literal.as_ref() {
        let value = literal.text().parse::<i128>().map_err(|_| {
            Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` has a non-integer const literal `{}`",
                literal.text(),
            ))
        })?;
        let primitive = program
            .type_reference_table
            .primitive_type(parameter_carrier)
            .filter(|primitive| primitive.accepts_integer_literal())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "conformance `{conformance}` parameter `{parameter}` has an integer literal under a non-integer carrier",
                ))
            })?;
        let value = CanonicalConstIdentity::integer(primitive.name(), value);
        validation::validate_exact_const_value_encoding(
            program,
            parameter_carrier,
            value.encoding.as_str(),
        )
        .map_err(|reason| {
            Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` has an invalid literal value: {reason}",
            ))
        })?;
        return Ok((
            ClosedConformanceConstArgument::Evaluated {
                parameter_carrier,
                declared_carrier: parameter_carrier,
                value,
            },
            literal.text().to_owned(),
        ));
    }
    if program.symbols.get(argument.symbol).kind == SymbolKind::TypeParameter {
        let matching = program
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .filter(|parameter| parameter.symbol == argument.symbol)
            .collect::<Vec<_>>();
        let [binder] = matching.as_slice() else {
            return Err(Diagnostic::error(format!(
                "conformance `{conformance}` parameter `{parameter}` resolves caller const binder `{}` to {} declarations; expected exactly one",
                argument.display_name(),
                matching.len(),
            )));
        };
        let TypeParameterKind::Const {
            type_reference: binder_carrier,
        } = binder.kind
        else {
            return Err(category_error(conformance, parameter, "const", argument));
        };
        return Ok((
            ClosedConformanceConstArgument::CallerBinder {
                parameter_carrier,
                binder: binder.symbol,
                binder_carrier,
            },
            argument.display_name(),
        ));
    }
    if program.symbols.get(argument.symbol).kind != SymbolKind::Const {
        return Err(category_error(conformance, parameter, "const", argument));
    }
    let declarations = program
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.symbol == argument.symbol)
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "conformance `{conformance}` parameter `{parameter}` resolves named const `{}` to {} declarations; expected exactly one",
            argument.display_name(),
            declarations.len(),
        )));
    };
    let Some(encoding) = declaration
        .evaluated_value
        .as_ref()
        .and_then(|value| value.encoding())
    else {
        return Err(Diagnostic::error(format!(
            "conformance `{conformance}` parameter `{parameter}` names const `{}` without a canonical checked value",
            argument.display_name(),
        )));
    };
    for carrier in [declaration.declared_type, parameter_carrier] {
        validation::validate_exact_const_value_encoding(program, carrier, encoding).map_err(
            |reason| {
                Diagnostic::error(format!(
                    "conformance `{conformance}` parameter `{parameter}` names const `{}` whose canonical value does not replay against its exact carriers: {reason}",
                    argument.display_name(),
                ))
            },
        )?;
    }
    let value = CanonicalConstIdentity {
        type_name: program
            .normalized_type_identity(declaration.declared_type)
            .into_string(),
        encoding: encoding.to_owned(),
    };
    let substitution_identity =
        CanonicalConstValue::new(value.type_name.clone(), value.encoding.clone(), "").atom();
    Ok((
        ClosedConformanceConstArgument::Evaluated {
            parameter_carrier,
            declared_carrier: declaration.declared_type,
            value,
        },
        substitution_identity,
    ))
}

pub(crate) fn static_argument_identity(
    _program: &TypedTrees,
    argument: &StaticMachineArgument,
) -> String {
    argument.display_name()
}

/// The identity a type argument contributes to a closed application: the same
/// normalized identity an unsubstituted concrete trait argument carries, so an
/// evidence binder's instantiated trait arguments and an implementation's own
/// trait arguments compare in one vocabulary.
pub(crate) fn type_argument_identity(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
) -> String {
    if argument.type_reference.is_valid() {
        return program
            .normalized_type_identity(argument.type_reference)
            .into_string();
    }
    program
        .type_reference_table
        .named_references()
        .find(|(_, symbol, _)| *symbol == argument.symbol)
        .map_or_else(
            || argument.display_name(),
            |(handle, _, _)| program.normalized_type_identity(handle).into_string(),
        )
}

pub(crate) fn substituted_type_identity_with_lifetimes(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, String)],
    lifetime_substitutions: &[(String, String)],
) -> String {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { symbol, .. } => substitutions
            .iter()
            .find_map(|(parameter, identity)| (*parameter == *symbol).then(|| identity.clone()))
            .unwrap_or_else(|| program.normalized_type_identity(handle).into_string()),
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => format!(
            "&{}{}{}",
            lifetime.as_ref().map_or(String::new(), |name| {
                let lifetime = lifetime_substitutions
                    .iter()
                    .rev()
                    .find_map(|(parameter, argument)| {
                        (parameter == name.as_str()).then_some(argument.as_str())
                    })
                    .unwrap_or_else(|| name.as_str());
                format!("'{lifetime} ")
            }),
            match access {
                language_semantics::ReferenceAccess::Shared => "",
                language_semantics::ReferenceAccess::Mutable => "mut ",
                language_semantics::ReferenceAccess::WriteOnly => "write ",
            },
            substituted_type_identity_with_lifetimes(
                program,
                *referee,
                substitutions,
                lifetime_substitutions,
            )
        ),
        TypeReferenceNode::Constrained { base_type, .. } => {
            substituted_type_identity_with_lifetimes(
                program,
                *base_type,
                substitutions,
                lifetime_substitutions,
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let length = match length {
                FixedArrayLength::Literal(value) => value.to_string(),
                FixedArrayLength::ConstParameter { symbol, name } => substitutions
                    .iter()
                    .find_map(|(parameter, identity)| {
                        (*parameter == *symbol).then(|| identity.clone())
                    })
                    .unwrap_or_else(|| name.as_str().to_owned()),
                FixedArrayLength::ConstCall { name, .. } => format!("{}()", name.as_str()),
            };
            format!(
                "[{}; {length}]",
                substituted_type_identity_with_lifetimes(
                    program,
                    *element_type,
                    substitutions,
                    lifetime_substitutions,
                )
            )
        }
        TypeReferenceNode::Slice { element_type } => format!(
            "[{}]",
            substituted_type_identity_with_lifetimes(
                program,
                *element_type,
                substitutions,
                lifetime_substitutions,
            )
        ),
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
            ..
        } => {
            let mut rendered = lifetime_arguments
                .iter()
                .map(|lifetime| {
                    let lifetime = lifetime_substitutions
                        .iter()
                        .rev()
                        .find_map(|(parameter, argument)| {
                            (parameter == lifetime.as_str()).then_some(argument.as_str())
                        })
                        .unwrap_or_else(|| lifetime.as_str());
                    format!("'{lifetime}")
                })
                .collect::<Vec<_>>();
            rendered.extend(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| {
                        substituted_type_identity_with_lifetimes(
                            program,
                            *argument,
                            substitutions,
                            lifetime_substitutions,
                        )
                    }),
            );
            format!("{}<{}>", base_name.as_str(), rendered.join(","))
        }
        _ => program.normalized_type_identity(handle).into_string(),
    }
}

#[allow(clippy::too_many_arguments)]
struct ApplicationIdentity {
    report_fingerprint: u64,
    commitment: typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
}

#[allow(clippy::too_many_arguments)]
fn application_identity(
    program: &TypedTrees,
    declaration: &str,
    lifetime_arguments: &[String],
    type_arguments: &[String],
    const_arguments: &[ClosedConformanceConstArgument],
    machine_arguments: &[SymbolHandle],
    trait_name: &str,
    trait_lifetime_arguments: &[String],
    trait_arguments: &[String],
    rows: &[typed_trees::trait_definition::ConformanceRow],
) -> ApplicationIdentity {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut bytes = Vec::new();
    let singleton_lanes = [declaration, trait_name];
    for lane in singleton_lanes {
        bytes.extend(1u64.to_le_bytes());
        bytes.extend((lane.len() as u64).to_le_bytes());
        bytes.extend(lane.as_bytes());
        bytes.push(0xff);
    }
    for lane in [
        lifetime_arguments,
        type_arguments,
        trait_lifetime_arguments,
        trait_arguments,
    ] {
        bytes.extend((lane.len() as u64).to_le_bytes());
        for item in lane {
            bytes.extend((item.len() as u64).to_le_bytes());
            bytes.extend(item.as_bytes());
        }
        bytes.push(0xff);
    }
    bytes.extend((const_arguments.len() as u64).to_le_bytes());
    for argument in const_arguments {
        let append = |bytes: &mut Vec<u8>, value: &str| {
            bytes.extend((value.len() as u64).to_le_bytes());
            bytes.extend(value.as_bytes());
        };
        match argument {
            ClosedConformanceConstArgument::Evaluated {
                parameter_carrier,
                declared_carrier,
                value,
            } => {
                bytes.push(0);
                append(
                    &mut bytes,
                    program
                        .normalized_type_identity(*parameter_carrier)
                        .as_str(),
                );
                append(
                    &mut bytes,
                    program.normalized_type_identity(*declared_carrier).as_str(),
                );
                append(&mut bytes, value.type_name.as_str());
                append(&mut bytes, value.encoding.as_str());
            }
            ClosedConformanceConstArgument::CallerBinder {
                parameter_carrier,
                binder,
                binder_carrier,
            } => {
                bytes.push(1);
                append(
                    &mut bytes,
                    program
                        .normalized_type_identity(*parameter_carrier)
                        .as_str(),
                );
                append(
                    &mut bytes,
                    program.normalized_type_identity(*binder_carrier).as_str(),
                );
                append(
                    &mut bytes,
                    program.symbols.display_path(*binder, "::").as_str(),
                );
            }
        }
        bytes.push(0xfd);
    }
    for machine in machine_arguments {
        let identity = program
            .machines()
            .iter()
            .flat_map(|owner| {
                program
                    .machine_states(owner)
                    .iter()
                    .map(move |state| (owner, state))
            })
            .find(|(_, state)| state.symbol == *machine)
            .map(|(owner, state)| format!("{}::{}", owner.name, state.name))
            .unwrap_or_else(|| "<unresolved-machine>".to_owned());
        bytes.extend((identity.len() as u64).to_le_bytes());
        bytes.extend(identity.as_bytes());
    }
    for row in rows {
        for item in [
            row.declaring_trait_name.as_str(),
            row.requirement_name.as_str(),
            row.realization_name.as_str(),
        ] {
            bytes.extend((item.len() as u64).to_le_bytes());
            bytes.extend(item.as_bytes());
        }
        if let Some(signature) = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == row.declaring_trait)
            .and_then(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == row.requirement)
            })
        {
            let signature_bytes =
                crate::monomorphization::canonical_state_signature_bytes(program, signature);
            bytes.push(1);
            bytes.extend((signature_bytes.len() as u64).to_le_bytes());
            bytes.extend(signature_bytes);
        } else {
            bytes.push(0);
        }
        bytes.push(0xfe);
    }
    let mut hash = OFFSET;
    let mut strong = Sha256::new();
    strong.update(b"omega.psi.closed-conformance-application.v3\0");
    strong.update(&bytes);
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    ApplicationIdentity {
        report_fingerprint: hash,
        commitment: typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest(
            strong.finalize().into(),
        ),
    }
}

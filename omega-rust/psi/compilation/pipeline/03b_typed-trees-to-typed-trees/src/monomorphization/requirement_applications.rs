//! Close applied public requirements before immutable checking and planning.
//!
//! The public signature owns the roster, independently of selected providers
//! or observed calls. Each complete tuple goes through the ordinary retained
//! static application carrier and deep callable substitution. Calls can name
//! only their exact admitted tuple; this pass neither grants membership nor
//! chooses a provider. Invalid or open source tuples remain source obligations.

use diagnostics::Diagnostic;
mod calls;
mod demands;
use typed_trees::finite_family::FamilyProbe;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
use typed_trees::{TypedTrees, expression::StaticMachineArgument};

pub(crate) fn instantiate_requirement_applications(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    program
        .prepare_closed_conformance_scopes()
        .map_err(|message| vec![Diagnostic::error(message)])?;
    super::contextual_calls::prepare_conformance_machine_demands(program)?;
    // Closed named maps introduce source scopes after initial callable setup.
    // Their logical signatures must exist before publishing closed instances.
    program
        .prepare_applied_callable_signatures()
        .map_err(|message| {
            vec![Diagnostic::error(format!(
                "applied logical signatures: {message}"
            ))]
        })?;
    let demands = demands::collect(program);
    for demand in demands {
        let requirement = demand.requirement;
        let has_outer_operands = {
            let scope = demand.environment.scope(program).ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary family lost its complete source application",
                )]
            })?;
            if program.require_receiving_trait_operands(&scope).is_err()
                || scope.portable_application_identity(program).is_none()
            {
                // A local representation key alone cannot close a portable
                // requirement. Leave unresolved source applications to their
                // ordinary obligations, rather than issue an incomplete row.
                continue;
            }
            // This publishes a logical requirement under the retained source
            // telescope, not an executable provider. Regional correspondence
            // is qualified later and travels with the exact candidate.
            !program.trait_type_parameters(scope.declaration).is_empty()
                || !scope.declaration.lifetime_parameters.is_empty()
        };
        // Invalid source operands must reach their owning admission diagnostic.
        // Materialization is meaningful only after the scope has exact custody.
        let trait_arguments = program
            .materialize_trait_application_arguments(&demand.environment)
            .map_err(|message| {
                vec![Diagnostic::error(format!(
                    "public requirement operands: {message}"
                ))]
            })?;
        let (_, signature) = program
            .trait_machine_signature(requirement)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "boundary family lost its source requirement",
                )]
            })?;
        let name = signature.name.clone();
        let parameters = program.state_signature_type_parameters(signature).to_vec();
        let tuples = if parameters.is_empty() {
            if !has_outer_operands {
                continue;
            }
            vec![calls::InvocationTuple {
                owner: demand.argument_owner,
                arguments: Vec::new(),
                lifetimes: Vec::new(),
            }]
        } else {
            match program.finite_signature_family(signature) {
                FamilyProbe::Finite { tuples, .. } => tuples
                    .into_iter()
                    .map(|tuple| {
                        tuple
                            .identities
                            .iter()
                            .map(|identity| tuple_argument(identity))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(|arguments| calls::InvocationTuple {
                        owner: demand.argument_owner,
                        arguments,
                        lifetimes: Vec::new(),
                    })
                    .collect(),
                FamilyProbe::NotFinite(_) => {
                    let scope = demand.environment.scope(program).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "static invocation lost its declaring scope",
                        )]
                    })?;
                    calls::invocation_tuples(program, requirement, &scope)?
                }
            }
        };
        for invocation in tuples {
            let mut arguments = invocation.arguments;
            // Closed instances consume a derived tuple. Keep the occurrence's
            // authored recipes available to final formation checking.
            super::body_rewriting::normalize_const_arguments(
                program,
                invocation.owner,
                &mut arguments,
            )
            .map_err(|message| vec![Diagnostic::error(message)])?;
            if arguments.len() != parameters.len() {
                return Err(vec![Diagnostic::error(
                    "static method application requires its complete invocation telescope",
                )]);
            }
            // A symbolic call checks the authored universal schema. It does
            // not demand an executable instance until its own tuple closes.
            let mut open = false;
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                match parameter.kind {
                    typed_trees::data::TypeParameterKind::Type
                    | typed_trees::data::TypeParameterKind::Const { .. }
                    | typed_trees::data::TypeParameterKind::Value { .. } => {
                        let actual = program
                            .static_operand_type_reference(argument)
                            .map_err(|message| vec![Diagnostic::error(message)])?;
                        open |= super::contextual_calls::operand_is_open(program, actual);
                    }
                    typed_trees::data::TypeParameterKind::Machine { .. } => {
                        open |= program.symbols.get(argument.symbol).kind
                            == symbols::SymbolKind::MachineParameter;
                    }
                    _ => {}
                }
            }
            if open {
                continue;
            }
            let bindings = invocation_bindings(program, &parameters, &arguments)?;
            let application = program.expression_table.retain_static_machine_application(
                requirement,
                name.clone(),
                &arguments,
            );
            let source = program
                .expression_table
                .static_machine_application_mut(application);
            source.lexical_owner = if invocation.owner.is_valid() {
                invocation.owner
            } else {
                requirement
            };
            source.binding_declaration = requirement;
            program
                .expression_table
                .set_static_machine_application_lifetimes(application, &invocation.lifetimes);
            program
                .expression_table
                .set_static_machine_application_bindings(application, &bindings);
            let instance = program
                .instantiate_requirement_application(
                    &demand.environment,
                    requirement,
                    &trait_arguments,
                    application,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
            // Deep substitution produces new membership operands as well as
            // carriers. Publish their identities with this exact signature;
            // the earlier template normalization cannot describe this tuple.
            let signature = program
                .closed_requirement_application(instance)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "closed requirement lost its produced callable signature",
                    )]
                })?
                .signature;
            let references =
                typed_trees::references::collect_signature_references(program, signature);
            super::finalize_referenced_domain_instance_identities(program, references)
                .map_err(|diagnostic| vec![diagnostic])?;
        }
    }
    calls::retarget(program)?;
    super::contextual_calls::retain_closed_requirement_demands(program)?;
    Ok(())
}

/// Symbolic and executable requirement calls share one binding roster.
/// Openness prevents executable instantiation, not substitution in a contract.
fn invocation_bindings(
    program: &mut TypedTrees,
    parameters: &[typed_trees::data::TypeParameter],
    arguments: &[StaticMachineArgument],
) -> Result<Vec<StaticMachineBinding>, Vec<Diagnostic>> {
    parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            Ok(StaticMachineBinding {
                parameter: parameter.symbol,
                value: match &parameter.kind {
                    typed_trees::data::TypeParameterKind::Type => StaticMachineBindingValue::Type(
                        program
                            .static_operand_type_reference(argument)
                            .map_err(|message| vec![Diagnostic::error(message)])?,
                    ),
                    typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                        // A runtime subject is not a closed value. Retain the
                        // exact subject beside its declared carrier, the same
                        // representation candidate selection uses; its roster
                        // membership is discharged at the call's own proof.
                        // The subject shape alone also fits an unresolved
                        // spelling such as a boolean atom, so require that no
                        // canonical const identity closes this operand.
                        let forwarded_value = argument.symbol.is_valid()
                            && program
                                .data_type_parameter_by_symbol(argument.symbol)
                                .is_some_and(|source| {
                                    matches!(
                                        source.kind,
                                        typed_trees::data::TypeParameterKind::Value { .. }
                                    )
                                });
                        if matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Value { .. }
                        ) && (super::const_arguments::is_runtime_value_subject(
                            program, argument,
                        ) || forwarded_value)
                            && program.static_const_argument_identity(argument).is_none()
                        {
                            StaticMachineBindingValue::RuntimeValue {
                                type_reference: *type_reference,
                                subject: argument.symbol,
                                expression: super::const_arguments::runtime_subject_expression(
                                    program, argument,
                                )
                                .unwrap_or_else(typed_trees::expression::ExpressionHandle::invalid),
                            }
                        } else {
                            // Retained bindings use the ordinary static-value
                            // namespace, not a new expression recipe. Authored
                            // operands remain on the application for carrier and
                            // bounds admission. Other operands retain their exact
                            // reference; immutable checking still judges closure.
                            let actual = if let Some(atom) =
                                super::const_arguments::spelling(program, argument)
                            {
                                program.type_reference_table.insert(
                                    typed_trees::types::TypeReferenceNode::Named {
                                        symbol: symbols::SymbolHandle::invalid(),
                                        name: typed_trees::name::Identifier::generated(atom),
                                    },
                                )
                            } else {
                                program
                                    .static_operand_type_reference(argument)
                                    .map_err(|message| vec![Diagnostic::error(message)])?
                            };
                            StaticMachineBindingValue::Const(actual)
                        }
                    }
                    typed_trees::data::TypeParameterKind::Machine { .. } => {
                        StaticMachineBindingValue::Machine(argument.clone())
                    }
                    _ => {
                        return Err(vec![Diagnostic::error(
                            "static requirement invocation has an unsupported binder category",
                        )]);
                    }
                },
            })
        })
        .collect()
}

fn tuple_argument(identity: &str) -> Result<StaticMachineArgument, Vec<Diagnostic>> {
    let spelling = super::dynamic_families::const_identity_spelling(identity).ok_or_else(|| {
        vec![Diagnostic::error(
            "boundary family has no represented canonical static operand",
        )]
    })?;
    if let Ok(integer) = spelling.parse::<i128>() {
        let text = integer.to_string();
        let literal = numerics::literals::IntegerLiteral::from_parts(
            integer < 0,
            numerics::literals::IntegerRadix::Decimal,
            text.strip_prefix('-').unwrap_or(&text),
        )
        .map_err(|_| {
            vec![Diagnostic::error(
                "boundary family integer has no represented literal",
            )]
        })?;
        return Ok(StaticMachineArgument {
            const_literal: Some(literal),
            ..StaticMachineArgument::default()
        });
    }
    Ok(StaticMachineArgument {
        path: vec![typed_trees::name::Identifier::generated(spelling)].into_boxed_slice(),
        ..StaticMachineArgument::default()
    })
}

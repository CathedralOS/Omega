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
            let arguments = invocation.arguments;
            if arguments.len() != parameters.len() {
                return Err(vec![Diagnostic::error(
                    "static method application requires its complete explicit invocation telescope",
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
            let mut bindings = Vec::with_capacity(parameters.len());
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                bindings.push(StaticMachineBinding {
                    parameter: parameter.symbol,
                    value: match &parameter.kind {
                        typed_trees::data::TypeParameterKind::Type => {
                            StaticMachineBindingValue::Type(
                                program
                                    .static_operand_type_reference(argument)
                                    .map_err(|message| vec![Diagnostic::error(message)])?,
                            )
                        }
                        typed_trees::data::TypeParameterKind::Const { .. }
                        | typed_trees::data::TypeParameterKind::Value { .. } => {
                            StaticMachineBindingValue::Const(
                                program
                                    .static_operand_type_reference(argument)
                                    .map_err(|message| vec![Diagnostic::error(message)])?,
                            )
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
                });
            }
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
            program
                .instantiate_requirement_application(
                    &demand.environment,
                    requirement,
                    &trait_arguments,
                    application,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
    }
    calls::retarget(program)?;
    super::contextual_calls::retain_closed_requirement_demands(program)?;
    Ok(())
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

//! Close declared boundary families before immutable checking and planning.
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

pub(crate) fn instantiate_boundary_requirement_families(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let demands = demands::collect(program);
    for demand in demands {
        let owner = demand.declaring_trait;
        let requirement = demand.requirement;
        let trait_arguments = program
            .materialize_trait_application_arguments(
                owner,
                &demand.substitutions,
                &demand.scope_starts,
                demand.argument_owner,
            )
            .map_err(|message| vec![Diagnostic::error(message)])?;
        if program
            .require_closed_application_operands(&trait_arguments)
            .is_err()
        {
            continue;
        }
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
            if trait_arguments.is_empty() {
                continue;
            }
            vec![Vec::new()]
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
                    .collect::<Result<Vec<_>, _>>()?,
                FamilyProbe::NotFinite(_) => continue,
            }
        };
        for arguments in tuples {
            let mut bindings = Vec::with_capacity(parameters.len());
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                let actual = program
                    .static_operand_type_reference(argument)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                bindings.push(StaticMachineBinding {
                    parameter: parameter.symbol,
                    value: StaticMachineBindingValue::Const(actual),
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
            source.lexical_owner = if demand.argument_owner.is_valid() {
                demand.argument_owner
            } else {
                requirement
            };
            source.binding_declaration = requirement;
            program
                .expression_table
                .set_static_machine_application_bindings(application, &bindings);
            program
                .instantiate_requirement_application(
                    owner,
                    requirement,
                    &trait_arguments,
                    application,
                )
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
    }
    calls::retarget(program)?;
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

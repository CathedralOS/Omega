//! Close declared boundary families before immutable checking and planning.
//!
//! The public signature owns the roster, independently of selected providers
//! or observed calls. Each complete tuple goes through the ordinary retained
//! static application carrier and deep callable substitution. Calls can name
//! only their exact admitted tuple; this pass neither grants membership nor
//! chooses a provider. Invalid or open source tuples remain source obligations.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::finite_family::FamilyProbe;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
use typed_trees::{TypedTrees, expression::StaticMachineArgument};

pub(crate) fn instantiate_boundary_requirement_families(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let demands = program
        .traits()
        .iter()
        .filter(|owner| owner.is_boundary)
        .flat_map(|owner| {
            program
                .trait_machine_signatures(owner)
                .iter()
                .filter_map(
                    |signature| match program.finite_signature_family(signature) {
                        FamilyProbe::Finite { tuples, .. }
                            if !signature.type_parameters.is_empty() =>
                        {
                            Some((owner.symbol, signature.symbol, tuples))
                        }
                        _ => None,
                    },
                )
        })
        .collect::<Vec<_>>();
    for (owner, requirement, tuples) in demands {
        // A declaration's outer parameters stay rigid. Their exact application
        // must already be represented before a local tuple can close it; one
        // visible provider does not supply missing trait arguments.
        let declaring = program.trait_definition_by_symbol(owner).ok_or_else(|| {
            vec![Diagnostic::error(
                "boundary family lost its declaring trait",
            )]
        })?;
        if !program.trait_type_parameters(declaring).is_empty()
            || !declaring.lifetime_parameters.is_empty()
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
        for tuple in tuples {
            let arguments = tuple
                .identities
                .iter()
                .map(|identity| tuple_argument(identity))
                .collect::<Result<Vec<_>, _>>()?;
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
            source.lexical_owner = requirement;
            source.binding_declaration = requirement;
            program
                .expression_table
                .set_static_machine_application_bindings(application, &bindings);
            program
                .instantiate_requirement_application(owner, requirement, &[], application)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
    }
    retarget_family_calls(program);
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

fn exact_family_instance(
    program: &TypedTrees,
    target: SymbolHandle,
    arguments: &[StaticMachineArgument],
) -> SymbolHandle {
    let Some((owner, _)) = program.trait_machine_signature(target) else {
        return SymbolHandle::invalid();
    };
    // A bare declaration receiver retains its declaration's rigid scope. It
    // cannot borrow an applied ancestor scope from a selected implementation.
    if !program.trait_type_parameters(owner).is_empty() || !owner.lifetime_parameters.is_empty() {
        return SymbolHandle::invalid();
    }
    let Some(tuple) = arguments
        .iter()
        .map(|argument| program.static_const_argument_identity(argument))
        .collect::<Option<Vec<_>>>()
    else {
        return SymbolHandle::invalid();
    };
    let mut matches = program
        .closed_requirement_applications()
        .filter(|application| {
            application.template == target
                && application.trait_arguments.is_empty()
                && program
                    .expression_table
                    .static_machine_application_arguments(application.application)
                    .iter()
                    .map(|argument| program.static_const_argument_identity(argument))
                    .collect::<Option<Vec<_>>>()
                    .as_ref()
                    == Some(&tuple)
        });
    let Some(instance) = matches.next() else {
        return SymbolHandle::invalid();
    };
    if matches.next().is_some() {
        return SymbolHandle::invalid();
    }
    instance.signature.symbol
}

fn retarget_family_calls(program: &mut TypedTrees) {
    let expressions = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let typed_trees::expression::ExpressionNode::Call(call) = expression else {
                return None;
            };
            let instance =
                exact_family_instance(program, call.target_symbol, &call.machine_arguments);
            instance.is_valid().then_some((handle, instance))
        })
        .collect::<Vec<_>>();
    let statements = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .filter_map(|(position, statement)| {
                    let call = statement.as_call()?;
                    let instance =
                        exact_family_instance(program, call.target_symbol, &call.machine_arguments);
                    instance
                        .is_valid()
                        .then_some((state.statement_nodes.handle_at(position as u32), instance))
                })
        })
        .collect::<Vec<_>>();
    for (handle, instance) in expressions {
        if let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression_mut(handle)
        {
            call.target_symbol = instance;
        }
    }
    for (handle, instance) in statements {
        if let typed_trees::statement::StatementNode::Call(call) =
            program.statement_table.statement_mut(handle)
        {
            call.target_symbol = instance;
        }
    }
}

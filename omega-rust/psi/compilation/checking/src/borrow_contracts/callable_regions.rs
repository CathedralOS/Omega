//! Resolve one callable's captured application environments before either the
//! structural region walk or the physical loan walk. Each substitution frame
//! borrows its preceding environment. Fresh invocation binders overlay only
//! the original signature: a compound operand may also retain captured regions.

use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
use super::carried_lifetimes::RegionIdentity;
use typed_trees::type_identity::applied_lifetime_parameter_identity;

pub(super) struct CallableRegions {
    pub(super) names: Vec<(String, RegionIdentity)>,
    pub(super) arguments: Vec<(String, RegionIdentity)>,
    pub(super) applications: Vec<Vec<(String, RegionIdentity)>>,
    pub(super) fresh_count: usize,
}

pub(super) fn resolve(
    program: &dyn crate::CheckingInput,
    contract: &CallableBorrowContract<'_>,
) -> Result<CallableRegions, &'static str> {
    let arguments = captured_lifetimes(program, contract.argument_owner);
    let mut applications = Vec::new();
    for (ordinal, scope) in contract.application_scopes.iter().enumerate() {
        let names = captured_lifetimes(program, scope.declaration)
            .into_iter()
            .map(|(name, _)| {
                let identity = if scope
                    .lifetimes
                    .iter()
                    .any(|(parameter, argument)| parameter == &name && argument.is_none())
                {
                    let declaration =
                        typed_trees::static_applications::StaticApplicationDeclaration::for_symbol(
                            program.typed_program(),
                            scope.declaration,
                        )?;
                    let mut formals = declaration
                        .lifetimes()
                        .iter()
                        .filter(|formal| formal.name.as_str() == name && formal.invocation);
                    let formal = formals
                        .next()
                        .ok_or("anonymous region is not an invocation formal")?;
                    if formals.next().is_some() {
                        return Err("anonymous invocation formal is ambiguous");
                    }
                    RegionIdentity::AnonymousInvocation(formal.declaration, formal.ordinal)
                } else {
                    let identity = applied_lifetime_parameter_identity(
                        program.typed_program(),
                        contract.argument_owner,
                        &name,
                        &contract.application_scopes[..=ordinal],
                    )
                    .ok_or("callable application region has no lexical declaration custody")?;
                    RegionIdentity::Captured(identity.declaration, identity.ordinal)
                };
                Ok((name, identity))
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        applications.push(names);
    }
    let mut names = captured_lifetimes(program, contract.lexical_owner);
    if let Some(application) = applications.last() {
        names.extend_from_slice(application);
    }
    for binding in contract.lifetime_bindings {
        let identity = match &binding.argument {
            typed_trees::static_applications::StaticMachineLifetimeArgument::Named(argument) => {
                arguments
                    .iter()
                    .rev()
                    .find(|(name, _)| name == argument.as_str())
                    .map(|(_, identity)| identity.clone())
                    .ok_or("callable input region has no lexical declaration custody")?
            }
            typed_trees::static_applications::StaticMachineLifetimeArgument::Anonymous => {
                let declaration =
                    typed_trees::static_applications::StaticApplicationDeclaration::for_symbol(
                        program.typed_program(),
                        contract.lifetime_binding_declaration,
                    )?;
                let mut formals = declaration
                    .lifetimes()
                    .iter()
                    .filter(|formal| formal.name == &binding.parameter && formal.invocation);
                let formal = formals
                    .next()
                    .ok_or("anonymous region is not an invocation formal")?;
                if formals.next().is_some() {
                    return Err("anonymous invocation formal is ambiguous");
                }
                RegionIdentity::AnonymousInvocation(formal.declaration, formal.ordinal)
            }
        };
        names.push((binding.parameter.as_str().to_owned(), identity));
    }
    let mut fresh_count = 0;
    for parameter in contract.lifetime_parameters {
        if contract.supplied_telescope
            && contract
                .lifetime_bindings
                .iter()
                .any(|binding| binding.parameter == *parameter)
        {
            continue;
        }
        names.push((
            parameter.as_str().to_owned(),
            RegionIdentity::Fresh(fresh_count),
        ));
        fresh_count += 1;
    }
    Ok(CallableRegions {
        names,
        arguments,
        applications,
        fresh_count,
    })
}

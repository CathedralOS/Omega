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
                let identity = applied_lifetime_parameter_identity(
                    program.typed_program(),
                    contract.argument_owner,
                    &name,
                    &contract.application_scopes[..=ordinal],
                )
                .ok_or("callable application region has no lexical declaration custody")?;
                Ok((
                    name,
                    RegionIdentity::Captured(identity.declaration, identity.ordinal),
                ))
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        applications.push(names);
    }
    let mut names = captured_lifetimes(program, contract.lexical_owner);
    if let Some(application) = applications.last() {
        names.extend_from_slice(application);
    }
    for binding in contract.lifetime_bindings {
        let identity = arguments
            .iter()
            .rev()
            .find(|(name, _)| name == binding.argument.as_str())
            .map(|(_, identity)| identity.clone())
            .ok_or("callable input region has no lexical declaration custody")?;
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

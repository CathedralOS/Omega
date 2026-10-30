//! Resolve one callable's captured application environments before either the
//! structural region walk or the physical loan walk. Each substitution frame
//! borrows its preceding environment. Fresh invocation binders overlay only
//! the original signature: a compound operand may also retain captured regions.

use super::CallableLifetimeApplication;
use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
use super::carried_lifetimes::RegionIdentity;
use typed_trees::type_identity::{
    LifetimeParameterIdentity, applied_lifetime_parameter_identity, lifetime_parameter_identity,
};

pub(super) struct AppliedRegionBinding {
    pub(super) parameter: LifetimeParameterIdentity,
    pub(super) argument: RegionIdentity,
}

pub(super) fn applied_region<'scope>(
    program: &dyn crate::CheckingInput,
    bindings: &'scope [AppliedRegionBinding],
    parameter: LifetimeParameterIdentity,
) -> Option<&'scope RegionIdentity> {
    let owner = program.lifetime_declaration_owner(parameter.declaration);
    if !owner.is_valid() {
        return None;
    }
    bindings
        .iter()
        .rev()
        .find(|binding| {
            binding.parameter.ordinal == parameter.ordinal && binding.parameter.declaration == owner
        })
        .map(|binding| &binding.argument)
}

pub(super) struct CallableRegions {
    pub(super) names: Vec<(String, RegionIdentity)>,
    pub(super) arguments: Vec<(String, RegionIdentity)>,
    pub(super) applications: Vec<Vec<(String, RegionIdentity)>>,
    pub(super) fresh_count: usize,
    pub(super) bindings: Vec<AppliedRegionBinding>,
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
    let owner = program.lifetime_declaration_owner(contract.lexical_owner);
    let fresh_parameters = contract
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            lifetime_parameter_identity(
                program.typed_program(),
                contract.lexical_owner,
                parameter.as_str(),
            )
            .filter(|parameter| parameter.declaration == owner && owner.is_valid())
            .ok_or("callable invocation binder has no exact declaration custody")
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let mut bindings = Vec::new();
    for application in contract.lifetime_applications {
        let supplied = match application {
            CallableLifetimeApplication::Enclosing { bindings }
            | CallableLifetimeApplication::Invocation { bindings } => *bindings,
        };
        // All arguments of one frame see only preceding frames. Repeated
        // enclosing scopes compose without rebinding a simultaneous actual.
        let preceding = bindings.len();
        for (position, binding) in supplied.iter().enumerate() {
            if program.lifetime_declaration_owner(binding.parameter.declaration)
                != binding.parameter.declaration
                || program.lifetime_declaration_owner(binding.argument.declaration)
                    != binding.argument.declaration
            {
                return Err("callable lifetime frame lost its canonical declaration coordinates");
            }
            let parameter_name = typed_trees::type_identity::lifetime_parameter_name(
                program.typed_program(),
                binding.parameter,
            )
            .ok_or("callable capture lost its exact receiving lifetime ordinal")?;
            typed_trees::type_identity::lifetime_parameter_name(
                program.typed_program(),
                binding.argument,
            )
            .ok_or("callable input region lost its exact supplying lifetime ordinal")?;
            if supplied[..position]
                .iter()
                .any(|prior| prior.parameter == binding.parameter)
            {
                return Err("callable capture repeats a receiving lifetime parameter");
            }
            if matches!(application, CallableLifetimeApplication::Enclosing { .. })
                && fresh_parameters.contains(&binding.parameter)
            {
                continue;
            }
            let argument = applied_region(program, &bindings[..preceding], binding.argument)
                .cloned()
                .unwrap_or(RegionIdentity::Captured(
                    binding.argument.declaration,
                    binding.argument.ordinal,
                ));
            names.push((parameter_name.as_str().to_owned(), argument.clone()));
            bindings.push(AppliedRegionBinding {
                parameter: binding.parameter,
                argument,
            });
        }
    }
    let mut fresh_count = 0;
    for (parameter, declaration) in contract.lifetime_parameters.iter().zip(fresh_parameters) {
        if applied_region(program, &bindings, declaration).is_some() {
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
        bindings,
    })
}

use super::{Result, Scope};
use crate::record::PackagePolicyMachineParameterContract;
use crate::record::PackagePolicyTypeParameter;
use crate::record::PackagePolicyTypeParameterKind;
use crate::record::PackageReviewConformanceBound;
use crate::record::PackageReviewTypeIdentity;
use crate::record::package::callable_policy::validation::signature::depth;
use crate::record::package::callable_policy::validation::signature::lifetimes;
use crate::record::package::callable_policy::validation::signature::owned_pair;
use crate::record::package::callable_policy::validation::signature::text;
use crate::record::package::callable_policy::validation::signature::value_type;
use crate::record::validation::behavior;
use crate::record::validation::contracts;

pub(in crate::record) fn parameters(scope: &Scope<'_>, nesting: usize) -> Result {
    u32::try_from(scope.lifetimes)
        .map_err(|_| "public lifetime telescope exceeds portable width")?;
    u32::try_from(scope.static_count())
        .map_err(|_| "public static telescope exceeds portable width")?;
    for parameter in scope.policy_statics {
        match &parameter.kind {
            PackagePolicyTypeParameterKind::Type => {}
            PackagePolicyTypeParameterKind::Implementation => {}
            PackagePolicyTypeParameterKind::Const(value)
            | PackagePolicyTypeParameterKind::Value(value) => value_type(value)?,
            PackagePolicyTypeParameterKind::Proposition(signature) => {
                for parameter in &signature.parameters {
                    value_type(&parameter.type_identity)?;
                }
            }
            PackagePolicyTypeParameterKind::Machine(contract) => machine(contract, scope, nesting)?,
        }
    }
    Ok(())
}

fn machine(
    contract: &PackagePolicyMachineParameterContract,
    outer: &Scope<'_>,
    nesting: usize,
) -> Result {
    depth(nesting)?;
    let signature = match contract {
        PackagePolicyMachineParameterContract::RequirementIdentity => return Ok(()),
        PackagePolicyMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
            trait_arguments,
        } => {
            for argument in trait_arguments {
                value_type(argument)?;
            }
            return owned_pair(trait_identity, requirement_identity);
        }
        PackagePolicyMachineParameterContract::Structural(signature) => signature,
    };
    let mut scope = child(
        outer,
        &signature.type_parameters,
        signature.lifetime_parameter_count,
    )?;
    formals(&mut scope, &signature.parameters, |value| {
        (&value.name, &value.type_identity)
    })?;
    result(&mut scope, signature.return_type.as_ref())?;
    parameters(&scope, nesting + 1)?;
    for contract in &signature.contracts {
        contracts::contract(contract, &scope, nesting + 1)?;
    }
    behavior::validate(
        &signature.published_crash,
        &signature.service_reach,
        &signature.synchronous_invocations,
        &signature.termination,
        &scope,
        nesting + 1,
    )
}

pub(super) fn child<'a>(
    outer: &'a Scope<'a>,
    parameters: &'a [PackagePolicyTypeParameter],
    lifetimes: usize,
) -> std::result::Result<Scope<'a>, &'static str> {
    Ok(Scope {
        outer: Some(outer),
        static_offset: outer.static_count(),
        lifetimes: outer
            .lifetimes
            .checked_add(lifetimes)
            .ok_or("public nested lifetime telescope overflows")?,
        ..super::scope(parameters, 0)
    })
}

pub(super) fn formals<'a, T>(
    scope: &mut Scope<'_>,
    parameters: &'a [T],
    project: impl Fn(&'a T) -> (&'a str, &'a PackageReviewTypeIdentity),
) -> Result {
    for (index, parameter) in parameters.iter().enumerate() {
        let (name, identity) = project(parameter);
        text(name)?;
        value_type(identity)?;
        if parameters[..index].iter().any(|prior| {
            let (prior_name, _) = project(prior);
            prior_name == name
        }) {
            return Err("public signature repeats a formal name");
        }
    }
    scope.parameters = parameters.len();
    Ok(())
}

pub(super) fn result(scope: &mut Scope<'_>, value: Option<&PackageReviewTypeIdentity>) -> Result {
    scope.result = value.is_some();
    if let Some(value) = value {
        value_type(value)?;
    }
    Ok(())
}

pub(in crate::record) fn bounds(
    values: &[PackageReviewConformanceBound],
    scope: &Scope<'_>,
) -> Result {
    let mut next = 0u32;
    for value in values {
        if value.binder_ordinal != next {
            return Err("public conformance binders are not declaration ordered");
        }
        next = next
            .checked_add(1)
            .ok_or("public conformance binder count overflows")?;
        value.trait_identity.validate_resolved()?;
        lifetimes(&value.trait_lifetime_arguments, scope)?;
        for argument in &value.arguments {
            value_type(argument)?;
        }
    }
    Ok(())
}

use super::{signature::*, signature_contracts::contract, signature_expressions::expression};
use crate::record::*;

pub(super) fn parameters(scope: &Scope<'_>, nesting: usize) -> Result {
    depth(nesting)?;
    for parameter in scope.statics {
        match &parameter.kind {
            PackageReviewTypeParameterKind::Type => {}
            PackageReviewTypeParameterKind::Const(value) => value_type(value)?,
            PackageReviewTypeParameterKind::Proposition(signature) => {
                for parameter in &signature.parameters {
                    value_type(&parameter.type_identity)?;
                }
            }
            PackageReviewTypeParameterKind::Machine(signature) => {
                machine(signature, scope, nesting + 1)?
            }
        }
    }
    Ok(())
}

fn machine(
    value: &PackageReviewMachineParameterContract,
    outer: &Scope<'_>,
    nesting: usize,
) -> Result {
    depth(nesting)?;
    let signature = match value {
        PackageReviewMachineParameterContract::RequirementIdentity => return Ok(()),
        PackageReviewMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        } => return owned_pair(trait_identity, requirement_identity),
        PackageReviewMachineParameterContract::Structural(signature) => signature,
    };
    let scope = Scope {
        outer: Some(outer),
        statics: &signature.type_parameters,
        static_offset: outer.static_count(),
        lifetimes: outer
            .lifetimes
            .checked_add(signature.lifetime_parameter_count)
            .ok_or("nested lifetime telescope overflows")?,
        parameters: signature.parameters.len(),
        nonself_parameters: signature
            .parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count(),
        has_self: signature
            .parameters
            .iter()
            .any(|parameter| parameter.is_self),
        result: true,
    };
    u32::try_from(scope.lifetimes)
        .map_err(|_| "nested lifetime telescope exceeds portable width")?;
    parameters(&scope, nesting + 1)?;
    for (index, parameter) in signature.parameters.iter().enumerate() {
        text(&parameter.name)?;
        value_type(&parameter.type_identity)?;
        if signature.parameters[..index]
            .iter()
            .any(|prior| prior.name == parameter.name || (prior.is_self && parameter.is_self))
        {
            return Err("structural machine signature repeats a formal or receiver");
        }
    }
    value_type(&signature.return_type)?;
    for value in &signature.contracts {
        contract(value, &scope, nesting + 1)?;
    }
    let mut crash_scope = scope;
    crash_scope.result = false;
    if signature
        .published_crash
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
    {
        return Err("structural machine crash causes repeat or are out of order");
    }
    for route in &signature.published_crash {
        super::ordered(&route.alternative_guards)?;
        if route.alternative_guards.len() != 1
            && route
                .alternative_guards
                .contains(&PackageReviewCrashRouteGuard::Truth)
        {
            return Err("structural machine crash route repeats an unconditional alternative");
        }
        if route.alternative_guards.is_empty() {
            return Err("structural machine crash route has no guards");
        }
        for guard in &route.alternative_guards {
            match guard {
                PackageReviewCrashRouteGuard::Truth => {}
                PackageReviewCrashRouteGuard::Expression(value) => {
                    expression(value, &crash_scope, nesting + 1)?
                }
                PackageReviewCrashRouteGuard::Predicate(_) => {
                    return Err("callable static crash policy retains unqualified predicate bytes");
                }
            }
        }
    }
    super::ordered(&signature.service_reach)?;
    for service in &signature.service_reach {
        nominal(service)?;
    }
    super::ordered(&signature.synchronous_invocations)?;
    for invocation in &signature.synchronous_invocations {
        match invocation {
            PackageReviewSynchronousInvocation::Service(service) => nominal(service)?,
            PackageReviewSynchronousInvocation::Parameter(ordinal) => {
                if *ordinal as usize >= scope.nonself_parameters {
                    return Err("static invocation escapes its parameter telescope");
                }
            }
        }
    }
    if let PackageReviewTermination::Terminates { premises } = &signature.termination {
        super::ordered(premises)?;
        for premise in premises {
            nominal(&premise.profile)?;
            for projection in &premise.projections {
                nominal(projection)?;
            }
            match &premise.subject {
                PackageReviewProgressSubject::Declaration(value) => nominal(value)?,
                PackageReviewProgressSubject::Parameter(ordinal) => {
                    if *ordinal as usize >= scope.nonself_parameters {
                        return Err("static progress subject escapes its parameter telescope");
                    }
                }
                PackageReviewProgressSubject::Receiver => {
                    if !scope.has_self {
                        return Err("static progress subject has no receiver");
                    }
                }
            }
        }
    }
    Ok(())
}

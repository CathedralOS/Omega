//! Exact authority sets and entry-rooted operational declarations.

use super::*;

pub(super) fn validate(callable: &PackagePolicyCallable) -> Result<(), &'static str> {
    if let Some(ceiling) = &callable.declared_service_reach {
        nominal_set(ceiling)?;
    }
    if let PackageReviewCheckedServiceReach::CheckedBody { realized, concrete } =
        &callable.checked_service_reach
    {
        nominal_set(realized)?;
        nominal_set(concrete)?;
        if concrete
            .iter()
            .any(|service| realized.binary_search(service).is_err())
        {
            return Err("concrete callable reach lies outside its realized bound");
        }
    }
    ordered(&callable.unresolved_installation_reaches)?;
    for reach in &callable.unresolved_installation_reaches {
        nominal(&reach.requirement)?;
        nominal_set(&reach.upper_bound)?;
    }
    if let Some(ceiling) = &callable.declared_synchronous_invocations {
        invocations(ceiling, callable.parameters.len())?;
    }
    invocations(
        &callable.realized_synchronous_invocations,
        callable.parameters.len(),
    )?;
    for flows in [
        &callable.capability_flows,
        &callable.reachable_capability_flows,
    ] {
        if flows
            .windows(2)
            .any(|pair| !pair[0].compare_canonical(&pair[1]).is_lt())
        {
            return Err("callable capability facts are repeated or out of order");
        }
        for flow in flows {
            nominal(&flow.capability)?;
        }
    }
    if callable.capability_flows.iter().any(|flow| {
        callable
            .reachable_capability_flows
            .binary_search_by(|candidate| candidate.compare_canonical(flow))
            .is_err()
    }) {
        return Err("reachable callable flows omit a caller-local checked fact");
    }
    ordered(&callable.mutation.paths)?;
    if callable.mutation.paths.iter().any(String::is_empty) {
        return Err("callable mutation contains an empty path");
    }
    if let Some(declared) = &callable.declared_termination {
        progress(callable, declared)?;
    }
    progress(callable, &callable.checked_termination)?;
    let crash = &callable.checked_crash;
    if crash.interface == PackageReviewCrashInterface::InternalInferred
        && (!crash.published.is_empty()
            || callable.supply != PackageReviewCallableSupply::CheckedBody
            || callable.role != PackagePolicyCallableRole::Build)
    {
        return Err("internal crash interface has a published or bodyless callable owner");
    }
    if let PackagePolicyInferredCrash::Complete { causes } = &crash.inferred {
        ordered(causes)?;
        if callable.supply != PackageReviewCallableSupply::CheckedBody
            || !crash.published.is_empty()
        {
            return Err("inferred callable crash summary has no checked body-only owner");
        }
    }
    if crash
        .published
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
    {
        return Err("callable crash causes are repeated or out of order");
    }
    for route in &crash.published {
        ordered(&route.alternative_guards)?;
        if route.alternative_guards.is_empty()
            || (route.alternative_guards.len() != 1
                && route
                    .alternative_guards
                    .contains(&PackagePolicyCrashGuard::Truth))
        {
            return Err("callable crash route has empty or redundant alternatives");
        }
        for guard in &route.alternative_guards {
            if let PackagePolicyCrashGuard::Expression(expression) = guard {
                signature::expression(expression, callable)?;
            }
        }
    }
    if let Some(requirements) = &crash.structural_runtime_requirements {
        for requirement in requirements {
            structural::validate(requirement, callable.parameters.len(), 0)?;
        }
    }
    Ok(())
}

fn invocations(
    values: &[PackageReviewSynchronousInvocation],
    count: usize,
) -> Result<(), &'static str> {
    ordered(values)?;
    for value in values {
        match value {
            PackageReviewSynchronousInvocation::Service(service) => nominal(service)?,
            PackageReviewSynchronousInvocation::Parameter(position)
                if *position as usize >= count =>
            {
                return Err("callable invocation lies outside its parameter telescope");
            }
            PackageReviewSynchronousInvocation::Parameter(_) => {}
        }
    }
    Ok(())
}

fn progress(
    callable: &PackagePolicyCallable,
    guarantee: &PackagePolicyTermination,
) -> Result<(), &'static str> {
    let PackagePolicyTermination::Terminates { premises } = guarantee else {
        return Ok(());
    };
    ordered(premises)?;
    let count = callable
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    for premise in premises {
        nominal(&premise.profile)?;
        match &premise.subject {
            PackageReviewProgressSubject::Declaration(declaration) => nominal(declaration)?,
            PackageReviewProgressSubject::Receiver
                if !callable
                    .parameters
                    .iter()
                    .any(|parameter| parameter.is_self) =>
            {
                return Err("callable progress receiver is absent");
            }
            PackageReviewProgressSubject::Parameter(position) if *position as usize >= count => {
                return Err("callable progress subject lies outside its telescope");
            }
            PackageReviewProgressSubject::Receiver | PackageReviewProgressSubject::Parameter(_) => {
            }
        }
        for projection in &premise.projections {
            nominal(projection)?;
        }
        ordered(&premise.establishment_routes)?;
        for route in &premise.establishment_routes {
            nominal(&route.requirement_owner)?;
            nominal(&route.requirement)?;
            if route.requirement.owner != route.requirement_owner.owner {
                return Err("callable progress route changes its declaration owner");
            }
        }
    }
    Ok(())
}

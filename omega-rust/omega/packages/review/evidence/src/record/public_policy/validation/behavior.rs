use super::*;

pub(super) fn validate(
    crashes: &[PackagePolicyCrashRoute],
    reaches: &[PackageReviewNominalIdentity],
    invocations: &[PackageReviewSynchronousInvocation],
    termination: &PackagePolicyTermination,
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    crash(crashes, scope, nesting)?;
    ordered(reaches)?;
    for reach in reaches {
        nominal(reach)?;
    }
    ordered(invocations)?;
    for invocation in invocations {
        match invocation {
            PackageReviewSynchronousInvocation::Service(service) => nominal(service)?,
            PackageReviewSynchronousInvocation::Parameter(ordinal)
                if (*ordinal as usize) < scope.nonself_parameters => {}
            _ => return Err("public invocation escapes its explicit value telescope"),
        }
    }
    if let PackagePolicyTermination::Terminates { premises } = termination {
        ordered(premises)?;
        for premise in premises {
            nominal(&premise.profile)?;
            for projection in &premise.projections {
                nominal(projection)?;
            }
            match &premise.subject {
                PackageReviewProgressSubject::Declaration(value) => nominal(value)?,
                PackageReviewProgressSubject::Receiver if scope.has_self => {}
                PackageReviewProgressSubject::Parameter(ordinal)
                    if (*ordinal as usize) < scope.nonself_parameters => {}
                _ => return Err("public progress subject escapes its declaration scope"),
            }
            ordered(&premise.establishment_routes)?;
            for route in &premise.establishment_routes {
                owned_pair(&route.requirement_owner, &route.requirement)?;
            }
        }
    }
    Ok(())
}

pub(super) fn crash(
    routes: &[PackagePolicyCrashRoute],
    scope: &Scope<'_>,
    nesting: usize,
) -> Result {
    if routes.windows(2).any(|pair| pair[0].cause >= pair[1].cause) {
        return Err("public crash causes repeat or change order");
    }
    let scope = Scope {
        result: false,
        ..*scope
    };
    for route in routes {
        ordered(&route.alternative_guards)?;
        if route.alternative_guards.is_empty()
            || (route.alternative_guards.len() != 1
                && route
                    .alternative_guards
                    .contains(&PackagePolicyCrashGuard::Truth))
        {
            return Err(
                "public crash guard set is empty or has redundant unconditional alternatives",
            );
        }
        for guard in &route.alternative_guards {
            if let PackagePolicyCrashGuard::Expression(value) = guard {
                expressions::expression(value, &scope, nesting)?;
            }
        }
    }
    Ok(())
}

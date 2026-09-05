use super::*;

pub(in crate::record) fn parameters(scope: &Scope<'_>, nesting: usize) -> Result {
    u32::try_from(scope.lifetimes)
        .map_err(|_| "public lifetime telescope exceeds portable width")?;
    u32::try_from(scope.static_count())
        .map_err(|_| "public static telescope exceeds portable width")?;
    for parameter in scope.policy_statics {
        match &parameter.kind {
            PackagePolicyTypeParameterKind::Type => {}
            PackagePolicyTypeParameterKind::Const(value) => value_type(value)?,
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
        } => return owned_pair(trait_identity, requirement_identity),
        PackagePolicyMachineParameterContract::Structural(signature) => signature,
    };
    let mut scope = child(
        outer,
        &signature.type_parameters,
        signature.lifetime_parameter_count,
    )?;
    formals(&mut scope, &signature.parameters, |value| {
        (&value.name, &value.type_identity, value.is_self)
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
    project: impl Fn(&'a T) -> (&'a str, &'a PackageReviewTypeIdentity, bool),
) -> Result {
    for (index, parameter) in parameters.iter().enumerate() {
        let (name, identity, receiver) = project(parameter);
        text(name)?;
        value_type(identity)?;
        if parameters[..index].iter().any(|prior| {
            let (prior_name, _, prior_receiver) = project(prior);
            prior_name == name || (prior_receiver && receiver)
        }) {
            return Err("public signature repeats a formal name or receiver");
        }
    }
    scope.parameters = parameters.len();
    scope.has_self = parameters.iter().any(|value| project(value).2);
    scope.nonself_parameters = parameters.len() - usize::from(scope.has_self);
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
        if let Some(ordinal) = value.binder_ordinal {
            if ordinal != next {
                return Err("public conformance binders are not declaration ordered");
            }
            next = next
                .checked_add(1)
                .ok_or("public conformance binder count overflows")?;
        }
        if !matches!(
            scope.static_kind(value.subject_parameter),
            Some(BinderKind::Type)
        ) {
            return Err("public conformance bound has no type-parameter subject");
        }
        nominal(&value.trait_identity)?;
        lifetimes(&value.trait_lifetime_arguments, scope)?;
        for argument in &value.arguments {
            value_type(argument)?;
        }
        match (&value.selected_conformance, &value.selected_subject) {
            (None, None)
                if value.selected_lifetime_arguments.is_empty()
                    && value.selected_arguments.is_empty() => {}
            (Some(selected), Some(subject)) => {
                nominal(selected)?;
                lifetimes(&value.selected_lifetime_arguments, scope)?;
                expressions::static_argument(subject, scope, 0)?;
                for argument in &value.selected_arguments {
                    expressions::static_argument(argument, scope, 0)?;
                }
            }
            _ => return Err("public conformance selection loses its complete application"),
        }
    }
    Ok(())
}

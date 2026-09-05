use super::*;

pub(super) fn trait_shape(value: &PackagePolicyTraitShape, api: &PackagePolicyPublicApi) -> Result {
    let scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    signatures::parameters(&scope, 0)?;
    signatures::bounds(&value.conformance_bounds, &scope)?;
    for parent in &value.parents {
        application(
            &parent.identity,
            &parent.lifetime_arguments,
            &parent.arguments,
            &scope,
            api,
        )?;
    }
    for (index, requirement) in value.requirements.iter().enumerate() {
        owned_pair(&value.identity, &requirement.identity)?;
        if value.requirements[..index]
            .iter()
            .any(|prior| prior.identity == requirement.identity)
        {
            return Err("public trait repeats an exact requirement");
        }
        let mut scope = signatures::child(
            &scope,
            &requirement.type_parameters,
            requirement.lifetime_parameter_count,
        )?;
        signatures::formals(&mut scope, &requirement.parameters, |value| {
            (&value.name, &value.type_identity, value.is_self)
        })?;
        signatures::result(&mut scope, requirement.return_type.as_ref())?;
        signatures::parameters(&scope, 0)?;
        for contract in &requirement.contracts {
            contracts::contract(contract, &scope, 0)?;
        }
        behavior::validate(
            &requirement.published_crash,
            &requirement.service_reach,
            &requirement.synchronous_invocations,
            &requirement.termination,
            &scope,
            0,
        )?;
    }
    Ok(())
}

pub(super) fn operator(value: &PackagePolicyOperatorShape) -> Result {
    let mut scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    signatures::formals(&mut scope, &value.parameters, |value| {
        (&value.name, &value.type_identity, value.is_self)
    })?;
    signatures::result(&mut scope, value.return_type.as_ref())?;
    signatures::parameters(&scope, 0)?;
    for contract in &value.contracts {
        contracts::contract(contract, &scope, 0)?;
    }
    behavior::crash(&value.published_crash, &scope, 0)
}

pub(super) fn conformance(
    value: &PackagePolicyConformanceShape,
    api: &PackagePolicyPublicApi,
) -> Result {
    conformance_scope(value)?;
    let scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    evidence(&value.interface, &scope, api)
}

pub(super) fn conformance_scope(value: &PackagePolicyConformanceShape) -> Result {
    nominal(&value.identity)?;
    let scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    signatures::parameters(&scope, 0)?;
    match &value.subject {
        PackageReviewConformanceSubject::Subjectless => {}
        PackageReviewConformanceSubject::Nominal(value) => nominal(value)?,
        PackageReviewConformanceSubject::TypeParameter(ordinal) => {
            if !matches!(scope.static_kind(*ordinal), Some(BinderKind::Type)) {
                return Err("public conformance subject is not a type binder");
            }
        }
    }
    contracts::evidence(&value.interface, &scope)
}

pub(super) fn domain(value: &PackagePolicyDomainShape) -> Result {
    let scope = Scope {
        domain_subject: true,
        ..super::scope(&value.type_parameters, 0)
    };
    signatures::parameters(&scope, 0)?;
    value_type(&value.target_type)?;
    for value in &value.index_arguments {
        value_type(value)?;
    }
    ordered(&value.predicate_facts)?;
    for fact in &value.predicate_facts {
        contracts::fact(fact, &scope, 0)?;
    }
    if value.predicate_body == language_semantics::DomainPredicateBody::Bodyless
        && !value.predicate_facts.is_empty()
    {
        return Err("bodyless public domain retains predicate facts");
    }
    if let Some(atoms) = &value.alias_expansion {
        ordered(atoms)?;
        for atom in atoms {
            if let PackageReviewDomainAliasAtom::Declared(value) = atom {
                nominal(value)?;
            }
        }
    }
    ordered(&value.semantic_roles)?;
    ordered(&value.establishment_routes)?;
    for route in &value.establishment_routes {
        owned_pair(&route.trait_identity, &route.requirement_identity)?;
    }
    Ok(())
}

pub(super) fn proposition(
    value: &PackageReviewPropositionShape,
    api: &PackagePolicyPublicApi,
) -> Result {
    let scope = Scope {
        proposition_binders: &value.binders,
        parameters: value.parameter_types.len(),
        nonself_parameters: value.parameter_types.len(),
        ..super::scope(&[], 0)
    };
    signatures::parameters(&scope, 0)?;
    for binder in &value.binders {
        if let PackageReviewPropositionBinderKind::Const(value) = &binder.kind {
            value_type(value)?;
        }
    }
    for value in &value.parameter_types {
        value_type(value)?;
    }
    match &value.body {
        PackageReviewPublicPropositionBody::Primitive => Ok(()),
        PackageReviewPublicPropositionBody::Witness(value) => evidence(value, &scope, api),
        PackageReviewPublicPropositionBody::Transparent(fact) => contracts::fact(fact, &scope, 0),
    }
}

fn application(
    identity: &PackageReviewNominalIdentity,
    lifetime_arguments: &[u32],
    arguments: &[PackageReviewTypeIdentity],
    scope: &Scope<'_>,
    api: &PackagePolicyPublicApi,
) -> Result {
    nominal(identity)?;
    lifetimes(lifetime_arguments, scope)?;
    for argument in arguments {
        value_type(argument)?;
    }
    if let Some(declaration) = api.traits.iter().find(|value| value.identity == *identity)
        && (lifetime_arguments.len() != declaration.lifetime_parameter_count
            || arguments.len() != declaration.type_parameters.len())
    {
        return Err("public trait application changes its retained declaration telescope");
    }
    Ok(())
}

fn evidence(
    value: &PackageReviewEvidenceInterface,
    scope: &Scope<'_>,
    api: &PackagePolicyPublicApi,
) -> Result {
    contracts::evidence(value, scope)?;
    application(
        &value.trait_identity,
        &value.lifetime_arguments,
        &value.arguments,
        scope,
        api,
    )?;
    for (index, requirement) in value.requirements.iter().enumerate() {
        if value.requirements[..index]
            .iter()
            .any(|prior| prior == requirement)
        {
            return Err("public evidence repeats an exact inherited requirement");
        }
        application(
            &requirement.declaring_trait,
            &requirement.declaring_trait_lifetime_arguments,
            &requirement.declaring_trait_arguments,
            scope,
            api,
        )?;
        if let Some(declaration) = api
            .traits
            .iter()
            .find(|value| value.identity == requirement.declaring_trait)
            && !declaration
                .requirements
                .iter()
                .any(|value| value.identity == requirement.requirement)
        {
            return Err("public evidence requirement is absent from its retained declaring trait");
        }
    }
    Ok(())
}

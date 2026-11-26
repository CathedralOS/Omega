// Structural validation for public traits, operators, conformances, domains, propositions, and evidence interfaces.

use super::{Result, Scope};
use crate::record::PackagePolicyConformanceShape;
use crate::record::PackagePolicyDomainShape;
use crate::record::PackagePolicyOperatorShape;
use crate::record::PackagePolicyPublicApi;
use crate::record::PackagePolicyTraitShape;
use crate::record::PackageReviewDomainEstablishmentRoute;
use crate::record::PackageReviewEvidenceInterface;
use crate::record::PackageReviewPropositionBinderKind;
use crate::record::PackageReviewPropositionShape;
use crate::record::PackageReviewPublicPropositionBody;
use crate::record::PackageReviewTypeIdentity;
use crate::record::package::callable_policy::validation::signature::lifetimes;
use crate::record::package::callable_policy::validation::signature::owned_pair;
use crate::record::package::callable_policy::validation::signature::value_type;
use crate::record::validation::behavior;
use crate::record::validation::contracts;
use crate::record::validation::ordered;
use crate::record::validation::signatures;
use crate::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};

pub(super) fn trait_shape(value: &PackagePolicyTraitShape, api: &PackagePolicyPublicApi) -> Result {
    let scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    signatures::parameters(&scope, 0)?;
    signatures::evidence_parameters(&value.type_parameters, &value.conformance_bounds, &scope)?;
    for parent in &value.parents {
        application(
            &parent.identity,
            &parent.lifetime_arguments,
            &parent.arguments,
            &scope,
            api,
            &value.identity.owner,
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
        if requirement.admitted_same_stack_bytes != 0
            && (!value.is_boundary || requirement.has_default_realization)
        {
            return Err("only a bodyless boundary requirement can declare foreign stack demand");
        }
        let mut scope = signatures::child(
            &scope,
            &requirement.type_parameters,
            requirement.lifetime_parameter_count,
        )?;
        signatures::evidence_parameters(
            &requirement.type_parameters,
            &requirement.conformance_bounds,
            &scope,
        )?;
        signatures::formals(&mut scope, &requirement.parameters, |value| {
            (&value.name, &value.type_identity)
        })?;
        signatures::result(&mut scope, requirement.return_type.as_ref())?;
        signatures::parameters(&scope, 0)?;
        contracts::roster(&requirement.contracts, &scope, 0)?;
        behavior::validate(
            requirement.published_crash.as_deref().unwrap_or_default(),
            requirement.service_reach.as_deref().unwrap_or_default(),
            &requirement.synchronous_invocations,
            &requirement.termination,
            &scope,
            0,
        )?;
        if requirement.service_reach_is_installation_bound && requirement.service_reach.is_none() {
            return Err("an installation-bound public requirement has no declared service ceiling");
        }
        let positions = requirement
            .structural_parameter_positions()
            .unwrap_or_default();
        ordered(positions)?;
        if positions
            .last()
            .is_some_and(|&position| position as usize >= requirement.parameters.len())
        {
            return Err("structural parameter names an unknown source parameter");
        }
        ordered(&requirement.returned_borrow_origins)?;
        for origin in &requirement.returned_borrow_origins {
            if origin
                .parameter_index()
                .is_some_and(|index| index as usize >= positions.len())
            {
                return Err("returned borrow names an unknown structural parameter");
            }
            if let Some(carrier) = origin.invocation_carrier() {
                value_type(carrier)?;
            }
        }
        ordered(&requirement.parameter_qualification_preconditions)?;
        for qualification in &requirement.parameter_qualification_preconditions {
            if qualification.structural_index() as usize >= positions.len() {
                return Err("qualification names an unknown structural parameter");
            }
            qualification.domain().validate_resolved()?;
        }
    }
    Ok(())
}

pub(super) fn operator(value: &PackagePolicyOperatorShape) -> Result {
    let mut scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    signatures::formals(&mut scope, &value.parameters, |value| {
        (&value.name, &value.type_identity)
    })?;
    signatures::result(&mut scope, value.return_type.as_ref())?;
    signatures::parameters(&scope, 0)?;
    contracts::roster(&value.contracts, &scope, 0)?;
    behavior::crash(&value.published_crash, &scope, 0)
}

pub(super) fn conformance(
    value: &PackagePolicyConformanceShape,
    api: &PackagePolicyPublicApi,
) -> Result {
    conformance_scope(value)?;
    let scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    evidence(&value.interface, &scope, api, &value.identity.owner)
}

pub(super) fn conformance_scope(value: &PackagePolicyConformanceShape) -> Result {
    value.identity.validate_resolved()?;
    let scope = super::scope(&value.type_parameters, value.lifetime_parameter_count);
    signatures::parameters(&scope, 0)?;
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
    if value.predicate_presence == language_core::DomainPredicatePresence::Absent
        && !value.predicate_facts.is_empty()
    {
        return Err("bodyless public domain retains predicate facts");
    }
    if let Some(atoms) = &value.alias_expansion {
        ordered(atoms)?;
        for domain in atoms {
            domain.validate_resolved()?;
        }
    }
    ordered(&value.semantic_roles)?;
    let routes = value.issuance.as_deref().unwrap_or_default();
    ordered(routes)?;
    for route in routes {
        match route {
            PackageReviewDomainEstablishmentRoute::CheckedRequirement {
                trait_identity,
                requirement_identity,
            }
            | PackageReviewDomainEstablishmentRoute::BoundaryRequirement {
                trait_identity,
                requirement_identity,
            } => owned_pair(trait_identity, requirement_identity)?,
            PackageReviewDomainEstablishmentRoute::ExactMachine { machine_identity } => {
                machine_identity.validate_resolved()?
            }
        }
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
        PackageReviewPublicPropositionBody::Witness(interface) => {
            evidence(interface, &scope, api, &value.identity.owner)
        }
        PackageReviewPublicPropositionBody::Transparent(fact) => contracts::fact(fact, &scope, 0),
    }
}

fn application<'a>(
    identity: &PackageReviewNominalIdentity,
    lifetime_arguments: &[u32],
    arguments: &[PackageReviewTypeIdentity],
    scope: &Scope<'_>,
    api: &'a PackagePolicyPublicApi,
    local_owner: &PackageReviewNominalOwner,
) -> std::result::Result<Option<&'a PackagePolicyTraitShape>, &'static str> {
    identity.validate_resolved()?;
    lifetimes(lifetime_arguments, scope)?;
    for argument in arguments {
        value_type(argument)?;
    }
    if identity.owner != *local_owner {
        return Ok(None);
    }
    let declaration = api
        .traits
        .binary_search_by(|value| value.identity.cmp(identity))
        .ok()
        .map(|index| &api.traits[index])
        .ok_or("public trait application lacks its local declaration")?;
    if lifetime_arguments.len() != declaration.lifetime_parameter_count
        || arguments.len() != declaration.type_parameters.len()
    {
        return Err("public trait application changes its retained declaration telescope");
    }
    Ok(Some(declaration))
}

fn evidence(
    value: &PackageReviewEvidenceInterface,
    scope: &Scope<'_>,
    api: &PackagePolicyPublicApi,
    local_owner: &PackageReviewNominalOwner,
) -> Result {
    contracts::evidence(value, scope)?;
    application(
        &value.trait_identity,
        &value.lifetime_arguments,
        &value.arguments,
        scope,
        api,
        local_owner,
    )?;
    for (index, requirement) in value.requirements.iter().enumerate() {
        if value.requirements[..index]
            .iter()
            .any(|prior| prior == requirement)
        {
            return Err("public evidence repeats an exact inherited requirement");
        }
        let declaring_trait = application(
            &requirement.declaring_trait,
            &requirement.declaring_trait_lifetime_arguments,
            &requirement.declaring_trait_arguments,
            scope,
            api,
            local_owner,
        )?;
        if let Some(declaration) = declaring_trait
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

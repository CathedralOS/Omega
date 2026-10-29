//! Direct bindings select a callable application, not a whole named map.
//! The original declaration and complete static tuple remain in provenance;
//! the schema uses the already formed callable identity, including phantom
//! arguments. ABI equality supplies neither an application nor coverage.
use super::*;
use checked_trees::typed_trees::{
    signature::StateSignature,
    trait_definition::{RetainedTraitScope, TraitDefinition},
};
use symbols::SymbolHandle;

pub fn from_typed_requirement_application(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
    requirement: SymbolHandle,
    retained_scope: Option<RetainedTraitScope>,
    closed_requirement: Option<SymbolHandle>,
) -> Option<ServiceSchema> {
    let program = context.typed;
    let Some((owner, template)) = program.trait_machine_signature(requirement) else {
        return from_typed_boundary_requirement(context, program.machine(requirement)?);
    };
    if !owner.is_boundary || !context.contains(owner.symbol) || !context.contains(requirement) {
        return None;
    }
    let scope = match retained_scope {
        Some(scope) => scope.scope(program)?,
        None => AppliedTraitScope::abstract_scope(owner),
    };
    if scope.declaration.symbol != owner.symbol {
        return None;
    }
    let signature = match closed_requirement {
        Some(symbol) => {
            let application = program.closed_requirement_application(symbol)?;
            if application.template != requirement
                || !program.requirement_application_matches_scope(symbol, &scope)
            {
                return None;
            }
            application.signature
        }
        None => template,
    };
    let mut identities = ProviderIdentityTable::default();
    let identity =
        identities.intern(&program.trait_requirement_callable_identity_text(owner, signature));
    let method = super::project_service_method(
        program,
        calling_plans,
        &scope,
        owner.symbol,
        &[],
        signature,
        identity,
        &mut identities,
    );
    Some(ServiceSchema {
        trait_name: identity,
        trait_package_identity: program.symbols.symbol_package_identity(owner.symbol),
        identities,
        receiver_applications: Vec::new(),
        methods: vec![method],
    })
}

/// Join an exact callable coordinate to its source signature. Both authored
/// family signatures and ordinarily formed closed applications use this one
/// query; equal physical parameter lists do not merge distinct static tuples.
pub fn requirement_application_for_schema<'a>(
    program: &'a checked_trees::typed_trees::TypedTrees,
    method: &ServiceMethod,
    identities: &ProviderIdentityTable,
) -> Result<(&'a TraitDefinition, &'a StateSignature), String> {
    let identity = identities.text(method.requirement_identity);
    let context = ProviderPlanningContext::new(program, source::DependencyScope::Product);
    let mut matches = Vec::new();
    for owner in program.traits().iter().filter(|owner| {
        owner.is_boundary
            && context.contains(owner.symbol)
            && program.trait_declaration_path(owner) == method.requirement_owner
            && program.symbols.symbol_package_identity(owner.symbol)
                == method.requirement_owner_package_identity
    }) {
        for signature in program.trait_machine_signatures(owner).iter().chain(
            program
                .closed_requirement_applications()
                .filter(|application| application.declaring_trait == owner.symbol)
                .map(|application| application.signature),
        ) {
            if signature.name.as_str() == method.name
                && program.trait_requirement_callable_identity_text(owner, signature) == identity
            {
                matches.push((owner, signature));
            }
        }
    }
    match matches.as_slice() {
        [application] => Ok(*application),
        _ => Err(format!(
            "boundary callable `{}` has {} exact source applications for `{identity}`",
            method.name,
            matches.len()
        )),
    }
}

/// Whether the selected named map owns this exact receiving coordinate.
/// Declaration equality alone cannot cover another application of the same
/// generic trait. Inherited scopes come from the same schema traversal used
/// to project the complete map.
pub(crate) fn conformance_covers_requirement_application(
    program: &checked_trees::typed_trees::TypedTrees,
    conformance: SymbolHandle,
    template: SymbolHandle,
    coordinate: SymbolHandle,
) -> bool {
    let Some((owner, _)) = program.trait_machine_signature(template) else {
        return false;
    };
    let Some(map) = program
        .conformances()
        .iter()
        .find(|map| map.symbol == conformance)
    else {
        return false;
    };
    let Some(root) = AppliedTraitScope::from_conformance(program, map) else {
        return false;
    };
    let Some(scopes) = super::applied_receivers::collect_scopes(program, root) else {
        return false;
    };
    scopes.iter().any(|scope| {
        scope.declaration.symbol == owner.symbol
            && if program.closed_requirement_application(coordinate).is_some() {
                program.requirement_application_matches_scope(coordinate, scope)
            } else {
                coordinate == template
                    && program.trait_type_parameters(owner).is_empty()
                    && owner.lifetime_parameters.is_empty()
            }
    })
}

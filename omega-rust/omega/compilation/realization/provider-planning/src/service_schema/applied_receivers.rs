//! Applied ancestor views stay beneath one whole provider schema.
//!
//! A declaration-only visited set loses applied diamonds and phantom arguments.
//! Collect exact scopes first, then publish each boundary receiver's complete
//! original requirement roster. Neither operation selects or constructs another
//! provider plan. The source derivation and review replay establish these views;
//! the digest commits to them without turning an annotation into authority.

use checked_trees::typed_trees::trait_definition::AppliedTraitScope;
use checked_trees::typed_trees::type_identity::TypeIdentityQualification;
use effects::provider_plan::{ProviderIdentityTable, ServiceReceiverApplication};

pub(super) fn collect_scopes<'program>(
    program: &'program checked_trees::typed_trees::TypedTrees,
    root: AppliedTraitScope<'program>,
) -> Option<Vec<AppliedTraitScope<'program>>> {
    let mut scopes: Vec<AppliedTraitScope<'_>> = Vec::new();
    let mut ambiguous = false;
    root.visit(program, &mut |scope| {
        if let Some(existing) = scopes
            .iter()
            .find(|existing| existing.declaration.symbol == scope.declaration.symbol)
        {
            ambiguous |= !existing.same_application(program, scope);
        } else {
            scopes.push(scope.clone());
        }
    });
    // A whole map cannot pick the first path of an applied diamond: retained
    // conformance rows name one original declaration, not one path per row.
    (!ambiguous).then_some(scopes)
}

pub(super) fn receiver_applications(
    program: &checked_trees::typed_trees::TypedTrees,
    scopes: &[AppliedTraitScope<'_>],
    identities: &mut ProviderIdentityTable,
) -> Option<Vec<ServiceReceiverApplication>> {
    if scopes
        .iter()
        .any(|scope| !scope.declaration.lifetime_parameters.is_empty())
    {
        // Parent lifetime substitutions are not retained by provider maps yet.
        // Absence cannot license ancestor selection. Exact-root admission still
        // uses the existing whole schema and its independent obligations.
        return Some(Vec::new());
    }
    let mut applications = Vec::new();
    for scope in scopes.iter().filter(|scope| scope.declaration.is_boundary) {
        let mut argument_identities = Vec::new();
        for parameter in program.trait_type_parameters(scope.declaration) {
            let Some(identity) = scope.region_free_parameter_identity(
                program,
                parameter.symbol,
                TypeIdentityQualification::PackageQualified,
            ) else {
                // A lifetime-free trait can still receive a region-bearing
                // type argument. Keep the whole schema, but do not license
                // ancestor selection using its erased carrier identity.
                return Some(Vec::new());
            };
            argument_identities.push(identities.intern(&program.type_identity_text(identity)?));
        }
        let mut requirement_identities = Vec::new();
        scope.visit(program, &mut |declaring| {
            for signature in program.trait_machine_signatures(declaring.declaration) {
                let identity = identities.intern(
                    &program
                        .trait_requirement_callable_identity_text(declaring.declaration, signature),
                );
                if !requirement_identities.contains(&identity) {
                    requirement_identities.push(identity);
                }
            }
        });
        requirement_identities
            .sort_by(|left, right| identities.text(*left).cmp(identities.text(*right)));
        applications.push(ServiceReceiverApplication {
            trait_name: identities.intern(&program.trait_declaration_path(scope.declaration)),
            trait_package_identity: program
                .symbols
                .symbol_package_identity(scope.declaration.symbol),
            argument_identities,
            requirement_identities,
        });
    }
    // Keep the root prominent while making ancestor commitment independent of
    // inheritance traversal order and process-local identity mint order.
    if let Some(ancestors) = applications.get_mut(1..) {
        ancestors.sort_by(|left, right| {
            identities
                .text(left.trait_name)
                .cmp(identities.text(right.trait_name))
                .then_with(|| {
                    left.trait_package_identity
                        .cmp(&right.trait_package_identity)
                })
        });
    }
    Some(applications)
}

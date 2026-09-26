//! Exact authored crash guards in policy-only nested static signatures.

use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::capture::contracts::facts::ContractProjectionContext;
use crate::package_evidence::capture::semantics::facts::exactly_one;
use crate::package_evidence::record::{
    PackagePolicyCrashGuard, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
};
use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateSignature;
use symbols::SymbolHandle;

pub(crate) fn project(
    compilation: &PackageReviewInput<'_>,
    owner: SymbolHandle,
    signature: &StateSignature,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCrashRoute>, Vec<Diagnostic>> {
    let capsule = exactly_one(
        compilation
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .filter(|capsule| {
                capsule.target_machine() == owner && capsule.target_state() == signature.symbol
            }),
        context.subject_name,
        "static signature crash capsule",
    )?;
    let derived = typed_trees_to_checked_trees::derive_authored_signature_crash_buckets(
        &compilation.typed,
        signature,
    );
    if derived != capsule.published_buckets() {
        return Err(vec![Diagnostic::error(
            "callable policy static crash capsule differs from its exact authored signature",
        )]);
    }
    crate::package_evidence::capture::behavior::policy::crash_routes(
        compilation,
        compilation.state_signature_contracts(signature),
        context,
        binders,
        &derived,
    )
    .map(|routes| {
        routes
            .into_iter()
            .map(|route| PackageReviewCrashRoute {
                cause: route.cause,
                alternative_guards: route
                    .alternative_guards
                    .into_iter()
                    .map(|guard| match guard {
                        PackagePolicyCrashGuard::Truth => PackageReviewCrashRouteGuard::Truth,
                        PackagePolicyCrashGuard::Expression(expression) => {
                            PackageReviewCrashRouteGuard::Expression(expression)
                        }
                    })
                    .collect(),
            })
            .collect()
    })
}

//! Exact authored crash guards in policy-only nested static signatures.

use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::facts::exactly_one;
use crate::record::{
    PackagePolicyCrashGuard, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::signature::StateSignature;

pub(super) fn project(
    compilation: &CheckedCompilation,
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
    let derived = psi_typed_trees_to_checked_trees::derive_authored_signature_crash_buckets(
        &compilation.typed,
        signature,
    );
    if derived != capsule.published_buckets() {
        return Err(vec![Diagnostic::error(
            "callable policy static crash capsule differs from its exact authored signature",
        )]);
    }
    crate::capture::behavior::policy::crash_routes(
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

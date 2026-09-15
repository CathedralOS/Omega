//! Unit call contract places and contract propositions.

use crate::validation::{
    BTreeSet, CrashRouteGuard, ModuleError, OperationId, Proposition, TerminalMachine, propositions,
};

/// Validate the complete bounded representation for a nonempty run of
/// pairwise-disjoint field transfers, followed by disposal of every maximal
/// residual sibling subtree in recursive reverse declaration order. This
/// partition is checked independently of producer facts before the ownership
/// walk relies on the path-sensitive terminator.
pub(crate) fn validate_unit_call_contract_places(
    callee: &TerminalMachine,
    operation: OperationId,
) -> Result<(), ModuleError> {
    let parameters = callee
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    for proposition in unit_call_contract_propositions(callee) {
        if let Some(place) = propositions::proposition_content_roots(proposition)
            .into_iter()
            .find(|place| !parameters.contains(place))
        {
            return Err(ModuleError::UnitCallContractPlaceHasNoArgument {
                operation,
                callee: callee.id,
                place,
            });
        }
    }
    Ok(())
}

pub(crate) fn unit_call_contract_propositions(
    callee: &TerminalMachine,
) -> impl Iterator<Item = &Proposition> {
    callee
        .contract
        .requires
        .iter()
        .chain(
            callee
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition),
        )
        .chain(
            callee
                .contract
                .crash_routes
                .iter()
                .flat_map(|bucket| &bucket.alternatives)
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => None,
                    CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
                }),
        )
}

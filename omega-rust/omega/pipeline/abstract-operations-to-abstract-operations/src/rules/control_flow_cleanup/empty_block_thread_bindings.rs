//! What both empty-block threads share: composing the threaded edge's value
//! bindings, and deciding that the thread leaves ownership frontiers
//! identical.

use std::collections::BTreeMap;

use abstract_operations::ValueBinding;
use optimization_unit::{
    OwnershipFrontierSite, PsiOptimizationFunction, PsiOptimizationUnit, ValueDefinition,
};
use semantic_vocabulary::{BlockId, EdgeId};

use crate::OwnershipFrontierAnalysis;

pub(super) fn compose_linear_thread_bindings(
    parameters: &[ValueDefinition],
    incoming: &[ValueBinding],
    outgoing: &[ValueBinding],
) -> Option<Vec<ValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| ValueBinding {
                        parameter: binding.parameter,
                        argument: *argument,
                        scalar_type: *scalar_type,
                    })
            })
            .collect(),
    )
}

pub(super) fn linear_thread_ownership_is_identity(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    frontiers: &OwnershipFrontierAnalysis,
    incoming: EdgeId,
    empty: BlockId,
    outgoing: EdgeId,
    target: BlockId,
) -> bool {
    let sites = [
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(empty),
        OwnershipFrontierSite::EdgeEntry(outgoing),
        OwnershipFrontierSite::EdgeExit(outgoing),
        OwnershipFrontierSite::BlockEntry(target),
    ];
    let facts = sites.map(|site| frontiers.fact(function.machine, site));
    if facts.iter().all(Option::is_none) {
        return function.structural_parameters.is_empty()
            && function.entry_claim_declarations.is_empty()
            && function.declared_places.is_empty();
    }
    facts.iter().all(|fact| {
        fact.is_some_and(|fact| fact.revision == unit.identity && fact.machine == function.machine)
    }) && facts
        .windows(2)
        .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
}

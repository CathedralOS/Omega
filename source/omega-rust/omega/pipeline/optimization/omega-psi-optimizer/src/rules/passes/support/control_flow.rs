//! Control-flow predicates shared by independently named Psi passes.

use omega_optimization_unit::ValueDefinitionSite;
use psi_core::{BlockId, MachineId, ValueId};

pub(in crate::rules::passes) fn replacement_dominates_parameter_uses(
    machine: MachineId,
    replacement: ValueId,
    parameter: ValueId,
    dominators: &[(BlockId, Vec<BlockId>)],
    use_definitions: &crate::UseDefinitionAnalysis,
) -> bool {
    let Some((_, definition)) = use_definitions
        .definitions
        .iter()
        .find(|(owner, definition)| *owner == machine && definition.value == replacement)
    else {
        return false;
    };
    use_definitions
        .uses
        .iter()
        .filter(|(owner, use_site)| *owner == machine && use_site.value == parameter)
        .all(|(_, use_site)| match definition.site {
            ValueDefinitionSite::FunctionParameter(_) => true,
            ValueDefinitionSite::BlockParameter {
                block: defining, ..
            } => block_dominates(dominators, defining, use_site.block),
            ValueDefinitionSite::Node {
                block: defining,
                node,
            } if defining == use_site.block => node < use_site.node,
            ValueDefinitionSite::Node {
                block: defining, ..
            } => block_dominates(dominators, defining, use_site.block),
        })
}

pub(in crate::rules::passes) fn block_dominates(
    dominators: &[(BlockId, Vec<BlockId>)],
    dominator: BlockId,
    block: BlockId,
) -> bool {
    dominators
        .iter()
        .find(|(candidate, _)| *candidate == block)
        .is_some_and(|(_, rows)| rows.contains(&dominator))
}

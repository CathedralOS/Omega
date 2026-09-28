use optimization_unit::{PsiOptimizationUnit, ValueDefinition, ValueDefinitionSite, ValueUse};
use semantic_vocabulary::{BlockId, MachineId, ValueId};

use crate::analyses::block_dominates;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseDefinitionAnalysis {
    pub definitions: Vec<(MachineId, ValueDefinition)>,
    pub uses: Vec<(MachineId, ValueUse)>,
}

impl UseDefinitionAnalysis {
    pub(crate) fn replacement_dominates_parameter_uses(
        &self,
        machine: MachineId,
        replacement: ValueId,
        parameter: ValueId,
        dominators: &[(BlockId, Vec<BlockId>)],
    ) -> bool {
        let Some((_, definition)) = self
            .definitions
            .iter()
            .find(|(owner, definition)| *owner == machine && definition.value == replacement)
        else {
            return false;
        };
        self.uses
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
}

pub(in crate::analyses) fn use_definitions(unit: &PsiOptimizationUnit) -> UseDefinitionAnalysis {
    let mut definitions = Vec::new();
    let mut uses = Vec::new();
    for function in &unit.functions {
        definitions.extend(
            function
                .parameters
                .iter()
                .chain(function.blocks.iter().flat_map(|block| {
                    block
                        .parameters
                        .iter()
                        .chain(block.nodes.iter().flat_map(|node| &node.definitions))
                }))
                .copied()
                .map(|definition| (function.machine, definition)),
        );
        uses.extend(
            function
                .blocks
                .iter()
                .flat_map(|block| block.nodes.iter().flat_map(|node| &node.uses))
                .copied()
                .map(|use_site| (function.machine, use_site)),
        );
    }
    UseDefinitionAnalysis { definitions, uses }
}

use super::*;

pub(super) fn collect_value_definitions(
    function: &PsiOptimizationFunction,
) -> Result<BTreeMap<ValueId, ValueDefinition>, OptimizationUnitValidationError> {
    let mut definitions = BTreeMap::new();
    for definition in function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
    {
        if definitions.insert(definition.value, *definition).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateValue(
                definition.value,
            ));
        }
    }
    Ok(definitions)
}

pub(super) fn validate_node_uses(
    function: &PsiOptimizationFunction,
    block: &optimization_unit::OptimizationBlock,
    node_index: usize,
    node: &optimization_unit::OptimizationNode,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    for use_site in &node.uses {
        let Some(definition) = definitions.get(&use_site.value) else {
            return Err(OptimizationUnitValidationError::UndefinedValue {
                machine: function.machine,
                block: block.id,
                value: use_site.value,
            });
        };
        match definition.site {
            ValueDefinitionSite::FunctionParameter(_) => {}
            ValueDefinitionSite::BlockParameter {
                block: defining, ..
            } => {
                if !dominators
                    .get(&block.id)
                    .is_some_and(|set| set.contains(&defining))
                {
                    return Err(OptimizationUnitValidationError::NondominatingValue {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                }
            }
            ValueDefinitionSite::Node {
                block: defining,
                node,
            } if defining == block.id => {
                if usize::try_from(node).expect("u32 fits usize") >= node_index {
                    return Err(OptimizationUnitValidationError::UseBeforeDefinition {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                }
            }
            ValueDefinitionSite::Node {
                block: defining, ..
            } => {
                if !dominators
                    .get(&block.id)
                    .is_some_and(|set| set.contains(&defining))
                {
                    return Err(OptimizationUnitValidationError::NondominatingValue {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_successor_bindings(
    function: &PsiOptimizationFunction,
    node: &optimization_unit::OptimizationNode,
    definitions: &BTreeMap<ValueId, ValueDefinition>,
    blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
) -> Result<(), OptimizationUnitValidationError> {
    for edge in &node.successors {
        let target = blocks.get(&edge.target).expect("successor validated");
        if edge.bindings.len() != target.parameters.len() {
            return Err(OptimizationUnitValidationError::BindingArityMismatch {
                machine: function.machine,
                edge: edge.psi_edge,
            });
        }
        for (binding, parameter) in edge.bindings.iter().zip(&target.parameters) {
            let source_type = definitions
                .get(&binding.argument)
                .map(|row| row.scalar_type);
            if binding.parameter != parameter.value
                || binding.scalar_type != parameter.scalar_type
                || source_type != Some(parameter.scalar_type)
            {
                return Err(OptimizationUnitValidationError::BindingTypeMismatch {
                    machine: function.machine,
                    edge: edge.psi_edge,
                    value: binding.argument,
                });
            }
        }
    }
    Ok(())
}

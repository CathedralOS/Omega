use super::control_flow::operation_edges;
use super::facts::collect_fact;
use super::provenance::operation_node_provenance;
use super::scalar_dataflow::{operation_definition, operation_uses};
use super::structural_custody::{
    collect_operation_structural_places, collect_places, operation_ownership,
};
use super::*;

pub(super) fn build_function(
    function: &AbstractFunction,
) -> Result<PsiOptimizationFunction, OptimizationUnitBuildError> {
    if function.block_entries.is_empty() {
        return Err(OptimizationUnitBuildError::MissingBlocks(function.machine));
    }
    if function.block_entries[0].operation_offset != 0 {
        return Err(OptimizationUnitBuildError::FirstBlockDoesNotStartAtZero(
            function.machine,
        ));
    }
    let mut block_ids = BTreeSet::new();
    for entry in &function.block_entries {
        if entry.operation_offset > function.operations.len() {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: entry.operation_offset,
            });
        }
        if !block_ids.insert(entry.block) {
            return Err(OptimizationUnitBuildError::DuplicateBlock(
                function.machine,
                entry.block,
            ));
        }
    }

    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            Ok(ValueDefinition {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                site: ValueDefinitionSite::FunctionParameter(u32::try_from(position).map_err(
                    |_| OptimizationUnitBuildError::ParameterIndexOverflow(function.machine),
                )?),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut facts = Vec::new();
    let mut structural_places = function
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .chain(
            function
                .result
                .structural()
                .map(|result| StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::Result,
                }),
        )
        .collect::<Vec<_>>();
    let mut declared_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .chain(function.entry_claims.iter().map(|claim| claim.input))
        .chain(function.result.structural().map(|result| result.place))
        .collect::<BTreeSet<_>>();
    let mut effect_token = 0u64;
    let mut blocks = Vec::with_capacity(function.block_entries.len());
    for (block_index, entry) in function.block_entries.iter().enumerate() {
        let end = function
            .block_entries
            .get(block_index + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if end < entry.operation_offset {
            return Err(OptimizationUnitBuildError::InvalidBlockOffset {
                machine: function.machine,
                offset: end,
            });
        }
        let block_parameter_rows = entry
            .parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                Ok(ValueDefinition {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    site: ValueDefinitionSite::BlockParameter {
                        block: entry.block,
                        position: u32::try_from(position).map_err(|_| {
                            OptimizationUnitBuildError::ParameterIndexOverflow(function.machine)
                        })?,
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut nodes = Vec::with_capacity(end - entry.operation_offset);
        for (local_index, operation) in function.operations[entry.operation_offset..end]
            .iter()
            .enumerate()
        {
            let node = u32::try_from(local_index)
                .map_err(|_| OptimizationUnitBuildError::NodeIndexOverflow(function.machine))?;
            let provenance = operation_node_provenance(operation);
            let fuel = provenance
                .iter()
                .copied()
                .map(|site| FuelSettlement { site, units: 1 })
                .collect();
            let definitions = operation_definition(operation)
                .into_iter()
                .map(|(value, scalar_type)| ValueDefinition {
                    value,
                    scalar_type,
                    site: ValueDefinitionSite::Node {
                        block: entry.block,
                        node,
                    },
                })
                .collect();
            let uses = operation_uses(operation)
                .into_iter()
                .map(|value| ValueUse {
                    value,
                    block: entry.block,
                    node,
                })
                .collect();
            collect_places(operation, &mut declared_places);
            collect_operation_structural_places(operation, &mut structural_places);
            collect_fact(operation, &mut facts);
            let ownership = operation_ownership(operation);
            let successors = operation_edges(operation);
            nodes.push(OptimizationNode {
                operation: operation.clone(),
                provenance,
                fuel,
                effect: EffectLink {
                    input: effect_token,
                    output: effect_token + 1,
                },
                definitions,
                uses,
                successors,
                ownership,
            });
            effect_token += 1;
        }
        blocks.push(OptimizationBlock {
            id: entry.block,
            parameters: block_parameter_rows,
            nodes,
        });
    }

    Ok(PsiOptimizationFunction {
        machine: function.machine,
        attachment: function.attachment,
        entry: function.entry,
        parameters,
        structural_parameters: function.structural_parameters.clone(),
        structural_places,
        result: function.result.clone(),
        declared_places,
        entry_claim_declarations: function.entry_claims.clone(),
        content_entry_claims: Vec::new(),
        verified_contract: None,
        evidence_contract_lanes: Vec::new(),
        entry_claims: function
            .entry_claims
            .iter()
            .map(|claim| claim.claim)
            .collect(),
        published_service_ceiling: function.published_service_ceiling.clone(),
        facts,
        blocks,
    })
}

//! Independent SCCP lattice and machine-snapshot reconstruction.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidatorSccpValue {
    Unknown,
    Constant(ScalarConstantValue),
    Overdefined,
}

pub(crate) fn validator_scalar_constant_facts(
    input: optimization_core::OptimizationUnitIdentity,
    function: &PsiOptimizationFunction,
) -> Vec<(
    ValueId,
    ScalarConstantValue,
    optimization_core::ScalarConstantFactIdentity,
)> {
    fn merge(target: &mut ValidatorSccpValue, incoming: ValidatorSccpValue) -> bool {
        let next = match (*target, incoming) {
            (ValidatorSccpValue::Unknown, incoming) => incoming,
            (_, ValidatorSccpValue::Unknown) | (ValidatorSccpValue::Overdefined, _) => {
                return false;
            }
            (_, ValidatorSccpValue::Overdefined) => ValidatorSccpValue::Overdefined,
            (ValidatorSccpValue::Constant(current), ValidatorSccpValue::Constant(incoming))
                if current == incoming =>
            {
                return false;
            }
            (ValidatorSccpValue::Constant(_), ValidatorSccpValue::Constant(_)) => {
                ValidatorSccpValue::Overdefined
            }
        };
        if *target == next {
            false
        } else {
            *target = next;
            true
        }
    }

    let mut values = BTreeMap::<ValueId, ValidatorSccpValue>::new();
    for parameter in &function.parameters {
        values.insert(parameter.value, ValidatorSccpValue::Overdefined);
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            values.insert(parameter.value, ValidatorSccpValue::Unknown);
        }
        for definition in block.nodes.iter().flat_map(|node| &node.definitions) {
            values.insert(definition.value, ValidatorSccpValue::Overdefined);
        }
    }
    let support_blocks = function
        .blocks
        .iter()
        .flat_map(|block| {
            block.nodes.iter().flat_map(move |node| {
                node.provenance
                    .iter()
                    .filter_map(move |source| match source {
                        PsiProvenance::Operation(operation) => Some((*operation, block.id)),
                        PsiProvenance::Edge(_) => None,
                    })
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut literal_rows = Vec::new();
    let mut literal_support = BTreeMap::new();
    for fact in &function.facts {
        let (value, constant, support) = match fact {
            OptimizationFact::BooleanConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Boolean(*constant), *support),
            OptimizationFact::IntegerConstant {
                value,
                constant,
                support,
            } => (*value, ScalarConstantValue::Integer(*constant), *support),
            OptimizationFact::OperationObligationReference { .. } => continue,
        };
        let block = support_blocks.get(&support).copied();
        literal_rows.push((value, constant, block));
        literal_support.insert(value, support);
        values.insert(
            value,
            if block.is_some() {
                ValidatorSccpValue::Unknown
            } else {
                ValidatorSccpValue::Constant(constant)
            },
        );
    }

    let mut reachable = BTreeSet::from([function.entry]);
    let mut feasible_edges = BTreeSet::<EdgeId>::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            for (value, constant, site) in &literal_rows {
                if *site == Some(block.id)
                    && matches!(values.get(value), Some(ValidatorSccpValue::Unknown))
                {
                    values.insert(*value, ValidatorSccpValue::Constant(*constant));
                    changed = true;
                }
            }
            let Some(node) = block.nodes.last() else {
                continue;
            };
            let operation_successors = validator_scalar_operation_successors(&node.operation);
            let successors = match &node.operation {
                abstract_operations::AbstractOperation::Jump { .. } => {
                    operation_successors.iter().collect::<Vec<_>>()
                }
                abstract_operations::AbstractOperation::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => match values.get(condition) {
                    Some(ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value))) => {
                        let selected = if *value {
                            when_true.psi_edge
                        } else {
                            when_false.psi_edge
                        };
                        operation_successors
                            .iter()
                            .filter(|successor| successor.psi_edge == selected)
                            .collect()
                    }
                    Some(ValidatorSccpValue::Overdefined) => {
                        operation_successors.iter().collect::<Vec<_>>()
                    }
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            for successor in successors {
                changed |= feasible_edges.insert(successor.psi_edge);
                changed |= reachable.insert(successor.target);
                for binding in &successor.bindings {
                    let incoming = values
                        .get(&binding.argument)
                        .copied()
                        .unwrap_or(ValidatorSccpValue::Overdefined);
                    let target = values
                        .entry(binding.parameter)
                        .or_insert(ValidatorSccpValue::Unknown);
                    changed |= merge(target, incoming);
                }
            }
        }
        if !changed {
            break;
        }
    }

    let snapshot = validator_sccp_snapshot(function, &values, &reachable, &feasible_edges);
    values
        .into_iter()
        .filter_map(|(value, state)| {
            let ValidatorSccpValue::Constant(constant) = state else {
                return None;
            };
            let definition = scalar_value_definition(function, value)?;
            let identity = literal_support
                .get(&value)
                .and_then(|support| {
                    literal_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        *support,
                    )
                })
                .or_else(|| {
                    derived_sccp_scalar_constant_fact_identity(
                        input,
                        function.machine,
                        definition,
                        constant,
                        &snapshot,
                    )
                })?;
            Some((value, constant, identity))
        })
        .collect()
}

pub(crate) fn validator_scalar_operation_successors(
    operation: &abstract_operations::AbstractOperation,
) -> Vec<OptimizationEdge> {
    use abstract_operations::AbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|successor| OptimizationEdge {
                psi_edge: successor.psi_edge,
                target: successor.target,
                bindings: successor.bindings.clone(),
                trivial_affine_discards: successor.trivial_affine_discards.clone(),
                provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
                fuel: vec![optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn validator_sccp_snapshot(
    function: &PsiOptimizationFunction,
    values: &BTreeMap<ValueId, ValidatorSccpValue>,
    reachable: &BTreeSet<BlockId>,
    feasible_edges: &BTreeSet<EdgeId>,
) -> SccpMachineSnapshot {
    use abstract_operations::AbstractOperation as O;
    let mut blocks = function
        .blocks
        .iter()
        .map(|block| SccpBlockRow {
            block: block.id,
            executable: reachable.contains(&block.id),
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|row| row.block);
    let mut edges = function
        .blocks
        .iter()
        .flat_map(|block| {
            let reachable_source = reachable.contains(&block.id);
            block.nodes.last().into_iter().flat_map(move |node| {
                validator_scalar_operation_successors(&node.operation)
                    .into_iter()
                    .map(move |successor| {
                        let state = if feasible_edges.contains(&successor.psi_edge) {
                            SccpEdgeState::Executable
                        } else if !reachable_source {
                            SccpEdgeState::Inexecutable
                        } else if let O::Conditional { condition, .. } = &node.operation {
                            match values.get(condition) {
                                Some(ValidatorSccpValue::Constant(
                                    ScalarConstantValue::Boolean(_),
                                )) => SccpEdgeState::Inexecutable,
                                _ => SccpEdgeState::Unknown,
                            }
                        } else {
                            SccpEdgeState::Inexecutable
                        };
                        SccpEdgeRow {
                            source: block.id,
                            edge: successor.psi_edge,
                            target: successor.target,
                            state,
                        }
                    })
            })
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|row| (row.source, row.edge));
    let mut snapshot_values = values
        .iter()
        .filter_map(|(value, state)| {
            let definition = scalar_value_definition(function, *value)?;
            Some(SccpValueRow {
                definition,
                state: match state {
                    ValidatorSccpValue::Unknown => SccpValueState::Unknown,
                    ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value)) => {
                        SccpValueState::Boolean(*value)
                    }
                    ValidatorSccpValue::Constant(ScalarConstantValue::Integer(value)) => {
                        SccpValueState::Integer(*value)
                    }
                    ValidatorSccpValue::Overdefined => SccpValueState::Overdefined,
                },
            })
        })
        .collect::<Vec<_>>();
    snapshot_values.sort_by_key(|row| row.definition.value);
    SccpMachineSnapshot {
        blocks,
        edges,
        values: snapshot_values,
    }
}

pub(crate) fn scalar_value_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ValueDefinition> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .copied()
        .find(|definition| definition.value == value)
}

pub(crate) fn validator_integer_value_type(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<semantic_vocabulary::IntegerType> {
    scalar_value_definition(function, value).and_then(|definition| match definition.scalar_type {
        ScalarType::Integer(integer) => Some(integer),
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => None,
    })
}

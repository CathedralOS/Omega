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
                abstract_operations::AbstractOperation::Jump { .. }
                | abstract_operations::AbstractOperation::StructuralCase { .. } => {
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
                if let abstract_operations::AbstractOperation::StructuralCase { cases, .. } =
                    &node.operation
                {
                    for payload in cases
                        .iter()
                        .filter(|case| case.psi_edge == successor.psi_edge)
                        .flat_map(|case| &case.payloads)
                    {
                        let target = values
                            .entry(payload.parameter)
                            .or_insert(ValidatorSccpValue::Unknown);
                        changed |= merge(target, ValidatorSccpValue::Overdefined);
                    }
                }
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
            structural_bindings,
            trivial_affine_discards,
            residual_affine_discards,
        } => vec![OptimizationEdge {
            structural_bindings: structural_bindings.clone(),
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            residual_affine_discards: residual_affine_discards.clone(),
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
                structural_bindings: successor.structural_bindings.clone(),
                psi_edge: successor.psi_edge,
                target: successor.target,
                bindings: successor.bindings.clone(),
                trivial_affine_discards: successor.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
                provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
                fuel: vec![optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        O::StructuralCase { cases, .. } => cases
            .iter()
            .map(|case| OptimizationEdge {
                psi_edge: case.psi_edge,
                target: case.target,
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                trivial_affine_discards: case.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
                provenance: vec![PsiProvenance::Edge(case.psi_edge)],
                fuel: vec![optimization_unit::FuelSettlement {
                    site: PsiProvenance::Edge(case.psi_edge),
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

#[cfg(test)]
mod structural_case_tests {
    use super::*;
    use abstract_operations::{AbstractOperation as O, AbstractSuccessor, ValueBinding};
    use optimization_unit::{ValueDefinition, ValueDefinitionSite};
    use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId};

    #[test]
    fn independent_sccp_case_payload_overdefines_an_ordinary_constant_arrival() {
        // Exercise lattice reconstruction directly, not structural source admission.
        let mut input = crate::tests::unit();
        let function = &mut input.functions[0];
        let original = function.blocks[0].clone();
        let literal = original.nodes[0].definitions[0];
        let condition = ValueId::new(90).unwrap();
        let parameter = ValueId::new(91).unwrap();
        let case_block = BlockId::new(92).unwrap();
        let join_block = BlockId::new(93).unwrap();
        let case_edge = EdgeId::new(94).unwrap();
        function.parameters.push(ValueDefinition {
            value: condition,
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::FunctionParameter(0),
        });
        let successor = |edge, target, bindings| AbstractSuccessor {
            psi_edge: EdgeId::new(edge).unwrap(),
            target,
            bindings,
            structural_bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let mut branch = original.nodes[1].clone();
        branch.operation = O::Conditional {
            condition,
            when_true: successor(
                95,
                join_block,
                vec![ValueBinding {
                    parameter,
                    argument: literal.value,
                    scalar_type: literal.scalar_type,
                }],
            ),
            when_false: successor(96, case_block, Vec::new()),
        };
        branch.provenance.clear();
        function.blocks[0].nodes[1] = branch;
        let mut dispatch = original.clone();
        dispatch.id = case_block;
        dispatch.nodes = vec![original.nodes[1].clone()];
        dispatch.nodes[0].provenance.clear();
        dispatch.nodes[0].operation = O::StructuralCase {
            source: PlaceId::new(97).unwrap(),
            cases: vec![abstract_operations::AbstractStructuralCaseSuccessor {
                psi_edge: case_edge,
                target: join_block,
                case: StructuralCaseId::new(98).unwrap(),
                payloads: vec![abstract_operations::AbstractStructuralCasePayloadBinding {
                    parameter,
                    field: StructuralFieldId::new(99).unwrap(),
                    scalar_type: literal.scalar_type,
                }],
                trivial_affine_discards: Vec::new(),
            }],
        };
        let mut join = original;
        join.id = join_block;
        join.nodes.remove(0);
        join.parameters = vec![ValueDefinition {
            value: parameter,
            scalar_type: literal.scalar_type,
            site: ValueDefinitionSite::BlockParameter {
                block: join_block,
                position: 0,
            },
        }];
        function.blocks.extend([dispatch, join]);
        let facts = validator_scalar_constant_facts(input.identity, function);
        assert!(facts.iter().any(|(value, _, _)| *value == literal.value));
        assert!(
            facts.iter().all(|(value, _, _)| *value != parameter),
            "unknown case payload must overdefine the constant ordinary arrival"
        );
        let case_edges =
            validator_scalar_operation_successors(&function.blocks[1].nodes[0].operation);
        assert_eq!(case_edges.len(), 1);
        assert_eq!(case_edges[0].psi_edge, case_edge);
        assert!(case_edges[0].bindings.is_empty());
    }
}

#[cfg(test)]
mod residual_edge_tests {
    #[test]
    fn successor_projection_retains_exact_ordered_residuals() {
        use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralTypeId};
        use terminal_psi::{StructuralAffineDiscard, StructuralPathSegment};
        let residuals = [2, 0]
            .map(|element| StructuralAffineDiscard {
                place: PlaceId::new(81).unwrap(),
                path: vec![StructuralPathSegment::FixedIndex(element)],
                structural_type: StructuralTypeId::new(82).unwrap(),
            })
            .to_vec();
        let operation = abstract_operations::AbstractOperation::Jump {
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(83).unwrap(),
            target: BlockId::new(84).unwrap(),
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: residuals.clone(),
        };
        let successors = super::validator_scalar_operation_successors(&operation);
        assert_eq!(successors.len(), 1);
        assert_eq!(successors[0].residual_affine_discards, residuals);
        assert!(successors[0].trivial_affine_discards.is_empty());
        assert_eq!(successors[0].target, BlockId::new(84).unwrap());
        assert_eq!(successors[0].psi_edge, EdgeId::new(83).unwrap());
    }
}

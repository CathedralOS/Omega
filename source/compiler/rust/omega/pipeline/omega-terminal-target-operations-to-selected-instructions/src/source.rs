use omega_optimization_unit::{
    AcceptedObligationFact, FuelSettlement, OptimizationFact, PsiOptimizationUnit, PsiProvenance,
    ValueDefinitionSite,
};
use omega_terminal_abstract_operations::{
    TerminalAbstractOperation, TerminalAbstractOperationPlan, TerminalValueBinding,
};
use omega_terminal_target_operations::{
    MachineRegister, TerminalPsiProvenance, TerminalScalarParameterLocation,
    TerminalTargetIntegerControl, TerminalTargetIntegerExpression, TerminalTargetOperation,
    TerminalTargetOperationPlan,
};
use psi_core::{
    BlockId, EdgeId, IntegerSign, IntegerValue, ObligationId, OperationId, ScalarType, ValueId,
};

use crate::{SelectedInstructionError, SelectedInstructionError as Error};

#[derive(Debug, Clone)]
pub(crate) struct SourceFunction {
    pub condition_source: ValueId,
    pub condition_parameter_index: usize,
    pub condition_register: MachineRegister,
    pub condition_definition_site: ValueDefinitionSite,
    pub entry_block: BlockId,
    pub true_block: BlockId,
    pub false_block: BlockId,
    pub branch_true_edge: EdgeId,
    pub branch_false_edge: EdgeId,
    pub branch_true_fuel: Vec<FuelSettlement>,
    pub branch_false_fuel: Vec<FuelSettlement>,
    pub branch_true_bindings: Vec<TerminalValueBinding>,
    pub branch_false_bindings: Vec<TerminalValueBinding>,
    pub when_true: SourceLeaf,
    pub when_false: SourceLeaf,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceLeaf {
    pub return_edge: EdgeId,
    pub source_value: ValueId,
    pub return_fuel: Vec<FuelSettlement>,
    pub value: SourceLeafValue,
}

#[derive(Debug, Clone)]
pub(crate) enum SourceLeafValue {
    Immediate {
        value: IntegerValue,
        constant_operation: OperationId,
        definition_site: ValueDefinitionSite,
        constant_fuel: Vec<FuelSettlement>,
    },
    EntryParameter {
        parameter_index: usize,
        register: MachineRegister,
        definition_site: ValueDefinitionSite,
    },
    ExactAdd {
        obligation: ObligationId,
        accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity,
        add_operation: OperationId,
        definition_site: ValueDefinitionSite,
        add_fuel: Vec<FuelSettlement>,
        left: SourceImmediate,
        right: SourceImmediate,
    },
    ExactSubtract {
        obligation: ObligationId,
        accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity,
        subtract_operation: OperationId,
        definition_site: ValueDefinitionSite,
        subtract_fuel: Vec<FuelSettlement>,
        left: SourceImmediate,
        right: SourceImmediate,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SourceImmediate {
    pub source_value: ValueId,
    pub value: IntegerValue,
    pub constant_operation: OperationId,
    pub definition_site: ValueDefinitionSite,
    pub fuel: Vec<FuelSettlement>,
}

pub(crate) fn derive_source_functions(
    target: &TerminalTargetOperationPlan,
    abstract_plan: &TerminalAbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceFunction>, SelectedInstructionError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.terminal_psi != abstract_plan.terminal_psi
        || target.terminal_psi != unit.terminal_psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }

    target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .enumerate()
        .map(|(index, ((target, abstracted), optimized))| {
            derive_source_function(
                index,
                target,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
            )
        })
        .collect()
}

fn derive_source_function(
    function: usize,
    target: &omega_terminal_target_operations::TerminalTargetFunction,
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
) -> Result<SourceFunction, SelectedInstructionError> {
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || abstracted.block_entries.len() != 3
        || optimized.blocks.len() != 3
        || optimized.entry != abstracted.entry
        || optimized.blocks[0].id != abstracted.block_entries[0].block
        || optimized.blocks[1].id != abstracted.block_entries[1].block
        || optimized.blocks[2].id != abstracted.block_entries[2].block
        || optimized.blocks[0].nodes.len() != 1
        || abstracted
            .block_entries
            .iter()
            .any(|entry| !entry.parameters.is_empty())
        || optimized
            .blocks
            .iter()
            .any(|block| !block.parameters.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let TerminalTargetOperation::ReturnIntegerConditionalControl {
        condition_source,
        condition_parameter_index,
        condition_location: TerminalScalarParameterLocation::Register(condition_register),
        scalar_type,
        when_true,
        when_false,
    } = &target.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if scalar_type.is_address()
        || scalar_type.sign() != IntegerSign::Unsigned
        || scalar_type.bits() != 64
    {
        return Err(Error::UnsupportedIntegerShape { function });
    }
    let constant_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Immediate { .. },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Immediate { .. },
                ..
            }
        )
    );
    let parameter_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Parameter { .. },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::Parameter { .. },
                ..
            }
        )
    );
    let exact_add_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactAdd {
                    left,
                    right,
                    ..
                },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactAdd {
                    left: false_left,
                    right: false_right,
                    ..
                },
                ..
            }
        ) if matches!(
            (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
            (
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
            )
        )
    );
    let exact_subtract_leaves = matches!(
        (when_true.control.as_ref(), when_false.control.as_ref()),
        (
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactSubtract {
                    left,
                    right,
                    ..
                },
                ..
            },
            TerminalTargetIntegerControl::Return {
                expression: TerminalTargetIntegerExpression::ExactSubtract {
                    left: false_left,
                    right: false_right,
                    ..
                },
                ..
            }
        ) if matches!(
            (left.as_ref(), right.as_ref(), false_left.as_ref(), false_right.as_ref()),
            (
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
                TerminalTargetIntegerExpression::Immediate { .. },
            )
        )
    );
    let expected_offsets = if constant_leaves {
        [0, 1, 3]
    } else if parameter_leaves {
        [0, 1, 2]
    } else if exact_add_leaves || exact_subtract_leaves {
        [0, 1, 5]
    } else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let (expected_operation_count, expected_leaf_node_count) = if constant_leaves {
        (5, 2)
    } else if parameter_leaves {
        (3, 1)
    } else {
        (9, 4)
    };
    if abstracted.operations.len() != expected_operation_count
        || abstracted
            .block_entries
            .iter()
            .zip(expected_offsets)
            .any(|(entry, offset)| entry.operation_offset != offset)
        || optimized.blocks[1].nodes.len() != expected_leaf_node_count
        || optimized.blocks[2].nodes.len() != expected_leaf_node_count
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let Some(parameter) = optimized.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    let Some(abstract_parameter) = abstracted.parameters.get(*condition_parameter_index) else {
        return Err(Error::UnsupportedCondition { function });
    };
    if parameter.value != *condition_source
        || parameter.scalar_type != ScalarType::Boolean
        || abstract_parameter.value != *condition_source
        || abstract_parameter.scalar_type != ScalarType::Boolean
    {
        return Err(Error::UnsupportedCondition { function });
    }

    let entry_node = &optimized.blocks[0].nodes[0];
    if entry_node.operation != abstracted.operations[0] {
        return Err(Error::SourceCustodyMismatch);
    }
    let TerminalAbstractOperation::Conditional {
        condition,
        when_true: abstract_true,
        when_false: abstract_false,
    } = &entry_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *condition != *condition_source
        || abstract_true.psi_edge != when_true.psi_edge
        || abstract_false.psi_edge != when_false.psi_edge
        || abstract_true.target != optimized.blocks[1].id
        || abstract_false.target != optimized.blocks[2].id
        || !abstract_true.bindings.is_empty()
        || !abstract_false.bindings.is_empty()
        || entry_node.successors.len() != 2
        || entry_node.successors[0].psi_edge != abstract_true.psi_edge
        || entry_node.successors[0].target != abstract_true.target
        || !entry_node.successors[0].bindings.is_empty()
        || entry_node.successors[1].psi_edge != abstract_false.psi_edge
        || entry_node.successors[1].target != abstract_false.target
        || !entry_node.successors[1].bindings.is_empty()
        || entry_node.provenance
            != vec![
                PsiProvenance::Edge(abstract_true.psi_edge),
                PsiProvenance::Edge(abstract_false.psi_edge),
            ]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let branch_true_fuel = exact_edge_fuel(entry_node, abstract_true.psi_edge, function)?;
    let branch_false_fuel = exact_edge_fuel(entry_node, abstract_false.psi_edge, function)?;
    if entry_node.fuel.len() != branch_true_fuel.len() + branch_false_fuel.len() {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let when_true = derive_leaf(
        function,
        when_true.psi_edge,
        when_true.control.as_ref(),
        &abstracted.operations[expected_offsets[1]..expected_offsets[2]],
        &optimized.blocks[1].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
    )?;
    let when_false = derive_leaf(
        function,
        when_false.psi_edge,
        when_false.control.as_ref(),
        &abstracted.operations[expected_offsets[2]..],
        &optimized.blocks[2].nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
    )?;
    if let (
        SourceLeafValue::EntryParameter {
            parameter_index: true_index,
            register: true_register,
            ..
        },
        SourceLeafValue::EntryParameter {
            parameter_index: false_index,
            register: false_register,
            ..
        },
    ) = (&when_true.value, &when_false.value)
        && (when_true.source_value != when_false.source_value
            || true_index != false_index
            || true_register != false_register
            || *true_index == *condition_parameter_index)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let expected_provenance = TerminalPsiProvenance {
        operations: when_true
            .value
            .operations()
            .into_iter()
            .chain(when_false.value.operations())
            .collect(),
        edges: vec![
            abstract_true.psi_edge,
            abstract_false.psi_edge,
            when_true.return_edge,
            when_false.return_edge,
        ],
    };
    if target.provenance != expected_provenance {
        return Err(Error::SourceCustodyMismatch);
    }

    Ok(SourceFunction {
        condition_source: *condition_source,
        condition_parameter_index: *condition_parameter_index,
        condition_register: *condition_register,
        condition_definition_site: parameter.site,
        entry_block: optimized.blocks[0].id,
        true_block: optimized.blocks[1].id,
        false_block: optimized.blocks[2].id,
        branch_true_edge: abstract_true.psi_edge,
        branch_false_edge: abstract_false.psi_edge,
        branch_true_fuel,
        branch_false_fuel,
        branch_true_bindings: abstract_true.bindings.clone(),
        branch_false_bindings: abstract_false.bindings.clone(),
        when_true,
        when_false,
    })
}

#[allow(clippy::too_many_arguments)]
fn derive_leaf(
    function: usize,
    arm_edge: EdgeId,
    target: &TerminalTargetIntegerControl,
    abstract_operations: &[TerminalAbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_terminal_abstract_operations::TerminalAbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
) -> Result<SourceLeaf, SelectedInstructionError> {
    if nodes.len() != abstract_operations.len()
        || nodes
            .iter()
            .zip(abstract_operations)
            .any(|(node, operation)| node.operation != *operation)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TerminalTargetIntegerControl::Return {
        psi_return_edge,
        source_value,
        expression,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let u64_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer_type);
    let (return_node, value) = match expression {
        TerminalTargetIntegerExpression::Immediate {
            source_value: expression_source,
            value: target_value,
        } => {
            if nodes.len() != 2 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } = &nodes[0].operation
            else {
                return Err(Error::MissingConstantDefinition { function, arm_edge });
            };
            if *result != *source_value
                || *value != *target_value
                || *scalar_type != u64_type
                || nodes[0].definitions.len() != 1
                || nodes[0].definitions[0].value != *source_value
                || nodes[0].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::MissingConstantDefinition { function, arm_edge });
            }
            let constant_fuel = exact_operation_fuel(&nodes[0], *psi_operation, function)?;
            (
                &nodes[1],
                SourceLeafValue::Immediate {
                    value: *value,
                    constant_operation: *psi_operation,
                    definition_site: nodes[0].definitions[0].site,
                    constant_fuel,
                },
            )
        }
        TerminalTargetIntegerExpression::Parameter {
            source_value: expression_source,
            parameter_index,
            location: TerminalScalarParameterLocation::Register(register),
        } => {
            if nodes.len() != 1 || source_value != expression_source {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let Some(parameter) = optimized.parameters.get(*parameter_index) else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            let Some(abstract_parameter) = abstracted.parameters.get(*parameter_index) else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if parameter.value != *source_value
                || parameter.scalar_type != u64_type
                || abstract_parameter.value != *source_value
                || abstract_parameter.scalar_type != u64_type
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            (
                &nodes[0],
                SourceLeafValue::EntryParameter {
                    parameter_index: *parameter_index,
                    register: *register,
                    definition_site: parameter.site,
                },
            )
        }
        TerminalTargetIntegerExpression::ExactAdd {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            if nodes.len() != 4 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u64_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u64_type)?;
            let TerminalAbstractOperation::ExactIntegerAdd {
                psi_operation: abstract_operation,
                obligation: abstract_obligation,
                result,
                scalar_type,
                left: abstract_left,
                right: abstract_right,
            } = &nodes[2].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_operation != psi_operation
                || abstract_obligation != obligation
                || *result != *source_value
                || *scalar_type != u64_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *source_value
                || nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let add_fuel = exact_operation_fuel(&nodes[2], *psi_operation, function)?;
            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation && *support == *psi_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            (
                &nodes[3],
                SourceLeafValue::ExactAdd {
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    add_operation: *psi_operation,
                    definition_site: nodes[2].definitions[0].site,
                    add_fuel,
                    left,
                    right,
                },
            )
        }
        TerminalTargetIntegerExpression::ExactSubtract {
            psi_operation,
            obligation,
            left,
            right,
        } => {
            if nodes.len() != 4 {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let left = derive_immediate(function, arm_edge, left, &nodes[0], u64_type)?;
            let right = derive_immediate(function, arm_edge, right, &nodes[1], u64_type)?;
            let TerminalAbstractOperation::ExactIntegerSubtract {
                psi_operation: abstract_operation,
                obligation: abstract_obligation,
                result,
                scalar_type,
                left: abstract_left,
                right: abstract_right,
            } = &nodes[2].operation
            else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            if abstract_operation != psi_operation
                || abstract_obligation != obligation
                || *result != *source_value
                || *scalar_type != u64_integer_type
                || *abstract_left != left.source_value
                || *abstract_right != right.source_value
                || nodes[2].definitions.len() != 1
                || nodes[2].definitions[0].value != *source_value
                || nodes[2].provenance != vec![PsiProvenance::Operation(*psi_operation)]
            {
                return Err(Error::UnsupportedSourceShape { function });
            }
            let subtract_fuel = exact_operation_fuel(&nodes[2], *psi_operation, function)?;
            let Some(accepted_fact) = accepted_obligation_facts.iter().find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            }) else {
                return Err(Error::SourceCustodyMismatch);
            };
            if !optimized.facts.iter().any(|fact| {
                matches!(
                    fact,
                    OptimizationFact::OperationObligationReference {
                        obligation: referenced_obligation,
                        support,
                    } if *referenced_obligation == *obligation && *support == *psi_operation
                )
            }) {
                return Err(Error::SourceCustodyMismatch);
            }
            (
                &nodes[3],
                SourceLeafValue::ExactSubtract {
                    obligation: *obligation,
                    accepted_fact: accepted_fact.identity,
                    subtract_operation: *psi_operation,
                    definition_site: nodes[2].definitions[0].site,
                    subtract_fuel,
                    left,
                    right,
                },
            )
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    let TerminalAbstractOperation::Return {
        psi_edge,
        value: returned_value,
        scalar_type: returned_type,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_edge != *psi_return_edge
        || *returned_value != *source_value
        || *returned_type != u64_type
        || !cleanup_actions.is_empty()
        || return_node.provenance != vec![PsiProvenance::Edge(*psi_return_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let return_fuel = exact_edge_fuel(return_node, *psi_return_edge, function)?;
    if return_node.fuel.len() != return_fuel.len() {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(SourceLeaf {
        return_edge: *psi_return_edge,
        source_value: *source_value,
        return_fuel,
        value,
    })
}

impl SourceLeafValue {
    fn operations(&self) -> Vec<OperationId> {
        match self {
            Self::Immediate {
                constant_operation, ..
            } => vec![*constant_operation],
            Self::EntryParameter { .. } => Vec::new(),
            Self::ExactAdd {
                add_operation,
                left,
                right,
                ..
            } => vec![
                left.constant_operation,
                right.constant_operation,
                *add_operation,
            ],
            Self::ExactSubtract {
                subtract_operation,
                left,
                right,
                ..
            } => vec![
                left.constant_operation,
                right.constant_operation,
                *subtract_operation,
            ],
        }
    }
}

fn derive_immediate(
    function: usize,
    arm_edge: EdgeId,
    target: &TerminalTargetIntegerExpression,
    node: &omega_optimization_unit::OptimizationNode,
    expected_type: ScalarType,
) -> Result<SourceImmediate, SelectedInstructionError> {
    let TerminalTargetIntegerExpression::Immediate {
        source_value,
        value: target_value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TerminalAbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &node.operation
    else {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    };
    if result != source_value
        || value != target_value
        || *scalar_type != expected_type
        || node.definitions.len() != 1
        || node.definitions[0].value != *source_value
        || node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    }
    Ok(SourceImmediate {
        source_value: *source_value,
        value: *value,
        constant_operation: *psi_operation,
        definition_site: node.definitions[0].site,
        fuel: exact_operation_fuel(node, *psi_operation, function)?,
    })
}

fn exact_edge_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    edge: EdgeId,
    function: usize,
) -> Result<Vec<FuelSettlement>, SelectedInstructionError> {
    let fuel = node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Edge(edge))
        .collect::<Vec<_>>();
    if fuel.is_empty() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}

fn exact_operation_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    function: usize,
) -> Result<Vec<FuelSettlement>, SelectedInstructionError> {
    let fuel = node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Operation(operation))
        .collect::<Vec<_>>();
    if fuel.is_empty() || fuel.len() != node.fuel.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}

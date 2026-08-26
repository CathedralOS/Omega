use std::collections::BTreeMap;

use omega_optimization_core::OptimizationUnitIdentity;
use omega_optimization_unit::{OptimizationFact, PsiOptimizationUnit, ValueDefinition, ValueUse};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{BlockId, EdgeId, IntegerValue, MachineId, OperationId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseDefinitionAnalysis {
    pub definitions: Vec<(MachineId, ValueDefinition)>,
    pub uses: Vec<(MachineId, ValueUse)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarConstant {
    Boolean(bool),
    Integer(IntegerValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueFactRegion {
    pub revision: OptimizationUnitIdentity,
    pub machine: MachineId,
    pub value: ValueId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarConstantFact {
    pub value: ValueId,
    pub constant: ScalarConstant,
    pub support: OperationId,
    pub valid_in: ValueFactRegion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarConstantAnalysis {
    pub facts: Vec<ScalarConstantFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExecutableEdgeKnowledge {
    KnownExecutable,
    KnownInexecutable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableEdgeFact {
    pub machine: MachineId,
    pub source: BlockId,
    pub edge: EdgeId,
    pub knowledge: ExecutableEdgeKnowledge,
    /// Exact literal-operation facts supporting a known conditional result.
    /// Empty support on an unconditional jump is structural, not guessed.
    pub support: Vec<OperationId>,
    pub revision: OptimizationUnitIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableEdgeAnalysis {
    pub edges: Vec<ExecutableEdgeFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeFact {
    pub value: ValueId,
    pub minimum: IntegerValue,
    pub maximum: IntegerValue,
    pub support: OperationId,
    pub valid_in: ValueFactRegion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRangeAnalysis {
    pub facts: Vec<ValueRangeFact>,
}

pub(super) fn use_definitions(unit: &PsiOptimizationUnit) -> UseDefinitionAnalysis {
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

pub(super) fn scalar_constants(unit: &PsiOptimizationUnit) -> ScalarConstantAnalysis {
    let mut facts = Vec::new();
    for function in &unit.functions {
        for fact in &function.facts {
            let (value, constant, support) = match fact {
                OptimizationFact::BooleanConstant {
                    value,
                    constant,
                    support,
                } => (*value, ScalarConstant::Boolean(*constant), *support),
                OptimizationFact::IntegerConstant {
                    value,
                    constant,
                    support,
                } => (*value, ScalarConstant::Integer(*constant), *support),
                OptimizationFact::OperationObligationReference { .. } => continue,
            };
            facts.push(ScalarConstantFact {
                value,
                constant,
                support,
                valid_in: ValueFactRegion {
                    revision: unit.identity,
                    machine: function.machine,
                    value,
                },
            });
        }
    }
    ScalarConstantAnalysis { facts }
}

pub(super) fn executable_edges(unit: &PsiOptimizationUnit) -> ExecutableEdgeAnalysis {
    let constants = scalar_constants(unit)
        .facts
        .into_iter()
        .filter_map(|fact| match fact.constant {
            ScalarConstant::Boolean(value) => {
                Some(((fact.valid_in.machine, fact.value), (value, fact.support)))
            }
            ScalarConstant::Integer(_) => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut edges = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            let Some(operation) = block.nodes.last().map(|node| &node.operation) else {
                continue;
            };
            match operation {
                O::Jump { psi_edge, .. } => edges.push(ExecutableEdgeFact {
                    machine: function.machine,
                    source: block.id,
                    edge: *psi_edge,
                    knowledge: ExecutableEdgeKnowledge::KnownExecutable,
                    support: Vec::new(),
                    revision: unit.identity,
                }),
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    let known = constants.get(&(function.machine, *condition)).copied();
                    for (selected_value, edge) in [(true, when_true), (false, when_false)] {
                        edges.push(ExecutableEdgeFact {
                            machine: function.machine,
                            source: block.id,
                            edge: edge.psi_edge,
                            knowledge: match known {
                                Some((value, _)) if value == selected_value => {
                                    ExecutableEdgeKnowledge::KnownExecutable
                                }
                                Some(_) => ExecutableEdgeKnowledge::KnownInexecutable,
                                None => ExecutableEdgeKnowledge::Unknown,
                            },
                            support: known.map_or_else(Vec::new, |(_, support)| vec![support]),
                            revision: unit.identity,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    ExecutableEdgeAnalysis { edges }
}

pub(super) fn value_ranges(unit: &PsiOptimizationUnit) -> ValueRangeAnalysis {
    ValueRangeAnalysis {
        facts: scalar_constants(unit)
            .facts
            .into_iter()
            .filter_map(|fact| match fact.constant {
                ScalarConstant::Integer(value) => Some(ValueRangeFact {
                    value: fact.value,
                    minimum: value,
                    maximum: value,
                    support: fact.support,
                    valid_in: fact.valid_in,
                }),
                ScalarConstant::Boolean(_) => None,
            })
            .collect(),
    }
}

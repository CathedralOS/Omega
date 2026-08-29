use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{OptimizationUnitIdentity, ScalarConstantFactIdentity};
use omega_optimization_unit::{
    OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ScalarConstantValue, SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot,
    SccpValueRow, SccpValueState, derived_sccp_scalar_constant_fact_identity,
    literal_scalar_constant_fact_identity,
};
use psi_core::{BlockId, EdgeId, IntegerValue, MachineId, OperationId, ValueId};

use super::shared::{scalar_operation_successors, scalar_value_definition};

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarConstantFact {
    pub value: ValueId,
    pub constant: ScalarConstant,
    /// Present only when this fact has a canonical derivation that the
    /// independent validator can reconstruct. Propagated facts stay
    /// unavailable to rewrite witnesses until that derivation vocabulary lands.
    pub identity: Option<ScalarConstantFactIdentity>,
    pub support: ScalarConstantSupport,
    pub valid_in: ValueFactRegion,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarConstantSupport {
    pub operations: Vec<OperationId>,
    pub edges: Vec<EdgeId>,
}

impl ScalarConstantSupport {
    fn literal(operation: OperationId) -> Self {
        Self {
            operations: vec![operation],
            edges: Vec::new(),
        }
    }

    pub fn literal_operation(&self) -> Option<OperationId> {
        let [operation] = self.operations.as_slice() else {
            return None;
        };
        self.edges.is_empty().then_some(*operation)
    }

    fn through_edge(mut self, edge: EdgeId) -> Self {
        if let Err(position) = self.edges.binary_search(&edge) {
            self.edges.insert(position, edge);
        }
        self
    }

    fn union_with(&mut self, other: &Self) {
        self.operations.extend_from_slice(&other.operations);
        self.operations.sort_unstable();
        self.operations.dedup();
        self.edges.extend_from_slice(&other.edges);
        self.edges.sort_unstable();
        self.edges.dedup();
    }
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
    /// Exact operations and edges supporting this feasibility verdict.
    pub support: ScalarConstantSupport,
    pub revision: OptimizationUnitIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableEdgeAnalysis {
    pub edges: Vec<ExecutableEdgeFact>,
}

pub(in crate::analyses) fn scalar_constants(unit: &PsiOptimizationUnit) -> ScalarConstantAnalysis {
    sparse_conditional_constants(unit).0
}

pub(in crate::analyses) fn executable_edges(unit: &PsiOptimizationUnit) -> ExecutableEdgeAnalysis {
    sparse_conditional_constants(unit).1
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LatticeValue {
    Unknown,
    Constant(ScalarConstant, ScalarConstantSupport),
    Overdefined,
}

fn sparse_conditional_constants(
    unit: &PsiOptimizationUnit,
) -> (ScalarConstantAnalysis, ExecutableEdgeAnalysis) {
    fn merge(
        target: &mut LatticeValue,
        incoming: &LatticeValue,
        path_support: &ScalarConstantSupport,
    ) -> bool {
        let incoming = match incoming {
            LatticeValue::Unknown => return false,
            LatticeValue::Overdefined => LatticeValue::Overdefined,
            LatticeValue::Constant(constant, support) => {
                let mut support = support.clone();
                support.union_with(path_support);
                LatticeValue::Constant(*constant, support)
            }
        };
        let next = match (&*target, incoming) {
            (LatticeValue::Unknown, incoming) => incoming,
            (_, LatticeValue::Unknown) => return false,
            (LatticeValue::Overdefined, _) => return false,
            (_, LatticeValue::Overdefined) => LatticeValue::Overdefined,
            (
                LatticeValue::Constant(current, current_support),
                LatticeValue::Constant(incoming, incoming_support),
            ) if *current == incoming => {
                let mut support = current_support.clone();
                support.union_with(&incoming_support);
                LatticeValue::Constant(*current, support)
            }
            (LatticeValue::Constant(..), LatticeValue::Constant(..)) => LatticeValue::Overdefined,
        };
        if *target == next {
            false
        } else {
            *target = next;
            true
        }
    }

    let mut facts = Vec::new();
    let mut edge_facts = Vec::new();
    for function in &unit.functions {
        let mut values = BTreeMap::<ValueId, LatticeValue>::new();
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
        for parameter in &function.parameters {
            values.insert(parameter.value, LatticeValue::Overdefined);
        }
        for block in &function.blocks {
            for parameter in &block.parameters {
                values.insert(parameter.value, LatticeValue::Unknown);
            }
            for definition in block.nodes.iter().flat_map(|node| &node.definitions) {
                values.insert(definition.value, LatticeValue::Overdefined);
            }
        }
        let mut literal_rows = Vec::new();
        for fact in &function.facts {
            let (value, constant, support) = match fact {
                OptimizationFact::BooleanConstant {
                    value,
                    constant,
                    support,
                } => (
                    *value,
                    ScalarConstant::Boolean(*constant),
                    ScalarConstantSupport::literal(*support),
                ),
                OptimizationFact::IntegerConstant {
                    value,
                    constant,
                    support,
                } => (
                    *value,
                    ScalarConstant::Integer(*constant),
                    ScalarConstantSupport::literal(*support),
                ),
                OptimizationFact::OperationObligationReference { .. } => continue,
            };
            let block = support_blocks.get(&support.operations[0]).copied();
            literal_rows.push((value, constant, support.clone(), block));
            values.insert(
                value,
                if block.is_some() {
                    LatticeValue::Unknown
                } else {
                    LatticeValue::Constant(constant, support)
                },
            );
        }

        let mut reachable = BTreeSet::from([function.entry]);
        let mut feasible_edges = BTreeMap::<EdgeId, ScalarConstantSupport>::new();
        let mut reach_support = BTreeMap::from([(
            function.entry,
            ScalarConstantSupport {
                operations: Vec::new(),
                edges: Vec::new(),
            },
        )]);
        loop {
            let mut changed = false;
            for block in &function.blocks {
                if !reachable.contains(&block.id) {
                    continue;
                }
                for (value, constant, support, site) in &literal_rows {
                    if *site == Some(block.id)
                        && matches!(values.get(value), Some(LatticeValue::Unknown))
                    {
                        values.insert(*value, LatticeValue::Constant(*constant, support.clone()));
                        changed = true;
                    }
                }
                let Some(node) = block.nodes.last() else {
                    continue;
                };
                let operation_successors = scalar_operation_successors(&node.operation);
                let successors = match &node.operation {
                    O::Jump { .. } => operation_successors
                        .iter()
                        .map(|successor| (successor, None))
                        .collect::<Vec<_>>(),
                    O::Conditional { condition, .. } => match values.get(condition) {
                        Some(LatticeValue::Constant(
                            ScalarConstant::Boolean(value),
                            condition_support,
                        )) => operation_successors
                            .iter()
                            .filter(|successor| {
                                matches!(
                                    &node.operation,
                                    O::Conditional {
                                        when_true,
                                        when_false,
                                        ..
                                    } if successor.psi_edge
                                        == if *value {
                                            when_true.psi_edge
                                        } else {
                                            when_false.psi_edge
                                        }
                                )
                            })
                            .map(|successor| (successor, Some(condition_support.clone())))
                            .collect(),
                        Some(LatticeValue::Overdefined) => operation_successors
                            .iter()
                            .map(|successor| (successor, None))
                            .collect::<Vec<_>>(),
                        Some(LatticeValue::Constant(ScalarConstant::Integer(_), _))
                        | Some(LatticeValue::Unknown)
                        | None => Vec::new(),
                    },
                    _ => Vec::new(),
                };
                for (successor, condition_support) in successors {
                    let mut path_support = reach_support
                        .get(&block.id)
                        .cloned()
                        .expect("reachable block has support");
                    if let Some(condition_support) = condition_support {
                        path_support.union_with(&condition_support);
                    }
                    path_support = path_support.through_edge(successor.psi_edge);
                    match feasible_edges.entry(successor.psi_edge) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(path_support.clone());
                            changed = true;
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            let mut joined = entry.get().clone();
                            joined.union_with(&path_support);
                            if joined != *entry.get() {
                                entry.insert(joined);
                                changed = true;
                            }
                        }
                    }
                    changed |= reachable.insert(successor.target);
                    match reach_support.entry(successor.target) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(path_support.clone());
                            changed = true;
                        }
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            let mut joined = entry.get().clone();
                            joined.union_with(&path_support);
                            if joined != *entry.get() {
                                entry.insert(joined);
                                changed = true;
                            }
                        }
                    }
                    for binding in &successor.bindings {
                        let incoming = values
                            .get(&binding.argument)
                            .cloned()
                            .unwrap_or(LatticeValue::Overdefined);
                        let target = values
                            .entry(binding.parameter)
                            .or_insert(LatticeValue::Unknown);
                        changed |= merge(target, &incoming, &path_support);
                    }
                }
            }
            if !changed {
                break;
            }
        }

        for block in &function.blocks {
            let source_reachable = reachable.contains(&block.id);
            let source_support = reach_support.get(&block.id);
            let Some(node) = block.nodes.last() else {
                continue;
            };
            for successor in scalar_operation_successors(&node.operation) {
                let feasible_support = feasible_edges.get(&successor.psi_edge);
                let (knowledge, support) = if let Some(support) = feasible_support {
                    (ExecutableEdgeKnowledge::KnownExecutable, support.clone())
                } else if !source_reachable {
                    (
                        ExecutableEdgeKnowledge::KnownInexecutable,
                        ScalarConstantSupport {
                            operations: Vec::new(),
                            edges: Vec::new(),
                        },
                    )
                } else if let O::Conditional { condition, .. } = &node.operation {
                    match values.get(condition) {
                        Some(LatticeValue::Constant(ScalarConstant::Boolean(_), condition)) => {
                            let mut support =
                                source_support
                                    .cloned()
                                    .unwrap_or_else(|| ScalarConstantSupport {
                                        operations: Vec::new(),
                                        edges: Vec::new(),
                                    });
                            support.union_with(condition);
                            (ExecutableEdgeKnowledge::KnownInexecutable, support)
                        }
                        _ => (
                            ExecutableEdgeKnowledge::Unknown,
                            ScalarConstantSupport {
                                operations: Vec::new(),
                                edges: Vec::new(),
                            },
                        ),
                    }
                } else {
                    (
                        ExecutableEdgeKnowledge::KnownInexecutable,
                        ScalarConstantSupport {
                            operations: Vec::new(),
                            edges: Vec::new(),
                        },
                    )
                };
                edge_facts.push(ExecutableEdgeFact {
                    machine: function.machine,
                    source: block.id,
                    edge: successor.psi_edge,
                    knowledge,
                    support,
                    revision: unit.identity,
                });
            }
        }

        let snapshot = sccp_machine_snapshot(function, &values, &reachable, &feasible_edges);

        facts.extend(values.into_iter().filter_map(|(value, state)| {
            let LatticeValue::Constant(constant, support) = state else {
                return None;
            };
            let definition = scalar_value_definition(function, value);
            let constant_value = match constant {
                ScalarConstant::Boolean(value) => ScalarConstantValue::Boolean(value),
                ScalarConstant::Integer(value) => ScalarConstantValue::Integer(value),
            };
            let identity = definition.and_then(|definition| {
                support
                    .literal_operation()
                    .and_then(|operation| {
                        literal_scalar_constant_fact_identity(
                            unit.identity,
                            function.machine,
                            definition,
                            constant_value,
                            operation,
                        )
                    })
                    .or_else(|| {
                        derived_sccp_scalar_constant_fact_identity(
                            unit.identity,
                            function.machine,
                            definition,
                            constant_value,
                            &snapshot,
                        )
                    })
            });
            Some(ScalarConstantFact {
                value,
                constant,
                identity,
                support,
                valid_in: ValueFactRegion {
                    revision: unit.identity,
                    machine: function.machine,
                    value,
                },
            })
        }));
    }
    facts.sort_by_key(|fact| (fact.valid_in.machine, fact.value));
    (
        ScalarConstantAnalysis { facts },
        ExecutableEdgeAnalysis { edges: edge_facts },
    )
}

fn sccp_machine_snapshot(
    function: &PsiOptimizationFunction,
    values: &BTreeMap<ValueId, LatticeValue>,
    reachable: &BTreeSet<BlockId>,
    feasible_edges: &BTreeMap<EdgeId, ScalarConstantSupport>,
) -> SccpMachineSnapshot {
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
            let operation = block.nodes.last().map(|node| &node.operation);
            operation.into_iter().flat_map(move |operation| {
                scalar_operation_successors(operation)
                    .into_iter()
                    .map(move |successor| {
                        let state = if feasible_edges.contains_key(&successor.psi_edge) {
                            SccpEdgeState::Executable
                        } else if !reachable_source {
                            SccpEdgeState::Inexecutable
                        } else if let O::Conditional { condition, .. } = operation {
                            match values.get(condition) {
                                Some(LatticeValue::Constant(ScalarConstant::Boolean(_), _)) => {
                                    SccpEdgeState::Inexecutable
                                }
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
                    LatticeValue::Unknown => SccpValueState::Unknown,
                    LatticeValue::Constant(ScalarConstant::Boolean(value), _) => {
                        SccpValueState::Boolean(*value)
                    }
                    LatticeValue::Constant(ScalarConstant::Integer(value), _) => {
                        SccpValueState::Integer(*value)
                    }
                    LatticeValue::Overdefined => SccpValueState::Overdefined,
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

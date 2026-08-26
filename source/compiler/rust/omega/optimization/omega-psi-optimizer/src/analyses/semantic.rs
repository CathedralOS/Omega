use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::{OptimizationUnitIdentity, ScalarConstantFactIdentity};
use omega_optimization_unit::{
    OptimizationEdge, OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, ScalarConstantValue, SccpBlockRow, SccpEdgeRow, SccpEdgeState,
    SccpMachineSnapshot, SccpValueRow, SccpValueState, ValueDefinition, ValueUse,
    derived_sccp_scalar_constant_fact_identity, literal_scalar_constant_fact_identity,
};
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRangeFact {
    pub value: ValueId,
    pub minimum: IntegerValue,
    pub maximum: IntegerValue,
    pub support: ScalarConstantSupport,
    pub valid_in: ValueFactRegion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRangeAnalysis {
    pub facts: Vec<ValueRangeFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectKnowledge {
    No,
    May,
    Yes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectClass {
    PureScalar,
    StructuralState,
    InternalCall,
    BoundaryCall,
    Service,
    Control,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEffectSummary {
    pub machine: MachineId,
    pub block: BlockId,
    pub node: u32,
    pub class: EffectClass,
    pub observable: EffectKnowledge,
    pub structural_state: EffectKnowledge,
    pub crash: EffectKnowledge,
    pub suspension: EffectKnowledge,
    pub support: Vec<PsiProvenance>,
    pub revision: OptimizationUnitIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSummaryAnalysis {
    pub nodes: Vec<NodeEffectSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeLiveness {
    pub node: u32,
    pub entry: Vec<ValueId>,
    pub exit: Vec<ValueId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueLivenessBlock {
    pub machine: MachineId,
    pub block: BlockId,
    pub entry: Vec<ValueId>,
    pub exit: Vec<ValueId>,
    pub nodes: Vec<NodeLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueLivenessAnalysis {
    pub blocks: Vec<ValueLivenessBlock>,
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
    sparse_conditional_constants(unit).0
}

pub(super) fn executable_edges(unit: &PsiOptimizationUnit) -> ExecutableEdgeAnalysis {
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

fn scalar_value_definition(
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

fn scalar_operation_successors(operation: &O) -> Vec<OptimizationEdge> {
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
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
            })
            .collect(),
        _ => Vec::new(),
    }
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

pub(super) fn effect_summaries(unit: &PsiOptimizationUnit) -> EffectSummaryAnalysis {
    let mut nodes = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let node_index = u32::try_from(node_index).expect("optimization node index is u32");
                let (class, observable, structural_state, crash, suspension) =
                    operation_effect(&node.operation);
                nodes.push(NodeEffectSummary {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                    class,
                    observable,
                    structural_state,
                    crash,
                    suspension,
                    support: node.provenance.clone(),
                    revision: unit.identity,
                });
            }
        }
    }
    EffectSummaryAnalysis { nodes }
}

fn operation_effect(
    operation: &O,
) -> (
    EffectClass,
    EffectKnowledge,
    EffectKnowledge,
    EffectKnowledge,
    EffectKnowledge,
) {
    use EffectKnowledge::{May, No, Yes};
    match operation {
        O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::ExactIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::ExactIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::ExactIntegerMultiply { .. }
        | O::ExactIntegerDivide { .. }
        | O::ExactIntegerRemainder { .. }
        | O::WrappingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::SaturatingIntegerRemainder { .. }
        | O::SaturatingIntegerMultiply { .. } => (EffectClass::PureScalar, No, No, No, No),
        O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnStructural { .. } => (EffectClass::StructuralState, No, Yes, No, No),
        O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::Call { .. } => (EffectClass::InternalCall, May, May, May, May),
        O::BoundaryCall { .. } => (EffectClass::BoundaryCall, Yes, May, May, May),
        O::PortWrite { .. } => (EffectClass::Service, Yes, No, No, No),
        O::Crash { .. } => (EffectClass::Control, Yes, No, Yes, No),
        O::Jump { .. } | O::Conditional { .. } | O::Return { .. } | O::ReturnUnit { .. } => {
            (EffectClass::Control, No, May, No, No)
        }
    }
}

pub(super) fn value_liveness(unit: &PsiOptimizationUnit) -> ValueLivenessAnalysis {
    let mut output = Vec::new();
    for function in &unit.functions {
        let successors = function
            .blocks
            .iter()
            .map(|block| {
                let targets = block
                    .nodes
                    .last()
                    .map(|node| match &node.operation {
                        O::Jump { target, .. } => vec![*target],
                        O::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => vec![when_true.target, when_false.target],
                        _ => Vec::new(),
                    })
                    .unwrap_or_default();
                (block.id, targets)
            })
            .collect::<BTreeMap<_, _>>();
        let mut live_entry = function
            .blocks
            .iter()
            .map(|block| (block.id, std::collections::BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut live_exit = live_entry.clone();
        loop {
            let mut changed = false;
            for block in function.blocks.iter().rev() {
                let next_exit = successors[&block.id]
                    .iter()
                    .filter_map(|successor| live_entry.get(successor))
                    .flat_map(|set| set.iter().copied())
                    .collect::<std::collections::BTreeSet<_>>();
                let mut next_entry = next_exit.clone();
                for node in block.nodes.iter().rev() {
                    for definition in &node.definitions {
                        next_entry.remove(&definition.value);
                    }
                    next_entry.extend(node.uses.iter().map(|use_site| use_site.value));
                }
                for parameter in &block.parameters {
                    next_entry.remove(&parameter.value);
                }
                if live_exit[&block.id] != next_exit {
                    live_exit.insert(block.id, next_exit);
                    changed = true;
                }
                if live_entry[&block.id] != next_entry {
                    live_entry.insert(block.id, next_entry);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for block in &function.blocks {
            let mut live = live_exit[&block.id].clone();
            let mut rows = Vec::with_capacity(block.nodes.len());
            for (node_index, node) in block.nodes.iter().enumerate().rev() {
                let exit = live.iter().copied().collect();
                for definition in &node.definitions {
                    live.remove(&definition.value);
                }
                live.extend(node.uses.iter().map(|use_site| use_site.value));
                rows.push(NodeLiveness {
                    node: u32::try_from(node_index).expect("optimization node index is u32"),
                    entry: live.iter().copied().collect(),
                    exit,
                });
            }
            rows.reverse();
            output.push(ValueLivenessBlock {
                machine: function.machine,
                block: block.id,
                entry: live_entry[&block.id].iter().copied().collect(),
                exit: live_exit[&block.id].iter().copied().collect(),
                nodes: rows,
            });
        }
    }
    ValueLivenessAnalysis { blocks: output }
}

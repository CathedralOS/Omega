use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    OptimizationUnitIdentity, OwnershipFrontierFactIdentity, ScalarConstantFactIdentity,
};
use omega_optimization_unit::{
    FuelSettlement, OptimizationEdge, OptimizationFact, OwnershipFrontierSite,
    OwnershipFrontierSnapshot, ProofQuestionOwner, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, ScalarConstantValue, SccpBlockRow, SccpEdgeRow, SccpEdgeState,
    SccpMachineSnapshot, SccpValueRow, SccpValueState, ValueDefinition, ValueRangeFact,
    ValueRangeRegion, ValueRangeScope, ValueRangeSupport, ValueUse,
    derived_sccp_scalar_constant_fact_identity, literal_scalar_constant_fact_identity,
    value_range_fact_identity,
};
use omega_optimization_validation::validate_current_value_range_fact_at;
use psi_core::{
    BlockId, BoundaryMachineId, EdgeId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue,
    MachineId, OperationId, Proposition, ScalarTerm, ScalarType, ServiceId, ValueId,
};
use psi_terminal_semantics::CanonicalScalarGoal;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRangeAnalysis {
    pub facts: Vec<ValueRangeFact>,
}

impl ValueRangeAnalysis {
    pub fn fact_applies_at(
        &self,
        fact: &ValueRangeFact,
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        block: BlockId,
        node: u32,
    ) -> bool {
        self.facts.iter().any(|candidate| candidate == fact)
            && validate_current_value_range_fact_at(unit, fact, machine, block, node).is_ok()
    }
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
    pub functions: Vec<FunctionEffectSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionEffectSummary {
    pub machine: MachineId,
    pub observable: EffectKnowledge,
    pub structural_state: EffectKnowledge,
    pub crash: EffectKnowledge,
    pub suspension: EffectKnowledge,
    pub services: Vec<ServiceId>,
    pub boundaries: Vec<BoundaryMachineId>,
    pub support: Vec<PsiProvenance>,
    pub revision: OptimizationUnitIdentity,
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

/// Exact immutable verifier fact made available in one optimization revision.
/// The site remains a source Terminal-Psi site; consumers must match that exact
/// site rather than treating the snapshot as a timeless function-wide fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipFrontierAnalysisFact {
    pub identity: OwnershipFrontierFactIdentity,
    pub revision: OptimizationUnitIdentity,
    pub machine: MachineId,
    pub site: OwnershipFrontierSite,
    pub snapshot: OwnershipFrontierSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipFrontierAnalysis {
    pub facts: Vec<OwnershipFrontierAnalysisFact>,
}

impl OwnershipFrontierAnalysis {
    pub fn fact(
        &self,
        machine: MachineId,
        site: OwnershipFrontierSite,
    ) -> Option<&OwnershipFrontierAnalysisFact> {
        self.facts
            .binary_search_by_key(&(machine, site), |fact| (fact.machine, fact.site))
            .ok()
            .map(|index| &self.facts[index])
    }
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

pub(super) fn ownership_frontiers(unit: &PsiOptimizationUnit) -> OwnershipFrontierAnalysis {
    OwnershipFrontierAnalysis {
        facts: unit
            .ownership_frontier_facts
            .iter()
            .map(|fact| OwnershipFrontierAnalysisFact {
                identity: fact.identity,
                revision: unit.identity,
                machine: fact.machine,
                site: fact.site,
                snapshot: fact.snapshot.clone(),
            })
            .collect(),
    }
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
            trivial_affine_discards,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![FuelSettlement {
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
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct PartialIntegerBounds {
    lower: Option<IntegerValue>,
    upper: Option<IntegerValue>,
}

enum IntervalExtraction {
    Unsupported,
    Contradiction,
    Bounds(BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>),
}

pub(super) fn value_ranges(unit: &PsiOptimizationUnit) -> ValueRangeAnalysis {
    let mut facts = Vec::new();
    let dominators = super::control_flow::dominators(unit, false);
    for constant in scalar_constants(unit).facts {
        let ScalarConstant::Integer(value) = constant.constant else {
            continue;
        };
        let Some(identity) = constant.identity else {
            continue;
        };
        let Some(function) = unit
            .functions
            .iter()
            .find(|function| function.machine == constant.valid_in.machine)
        else {
            continue;
        };
        let Some(ValueDefinition {
            scalar_type: ScalarType::Integer(scalar_type),
            ..
        }) = scalar_value_definition(function, constant.value)
        else {
            continue;
        };
        let support = ValueRangeSupport::ScalarConstant(identity);
        let valid_in = ValueRangeRegion {
            revision: unit.identity,
            machine: function.machine,
            value: constant.value,
            scope: ValueRangeScope::EntireValue,
            dominated_blocks: Vec::new(),
        };
        facts.push(new_value_range_fact(
            constant.value,
            scalar_type,
            value,
            value,
            support,
            valid_in,
        ));
    }

    for function in &unit.functions {
        let reachable = reachable_blocks(function);
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some((operation, obligation, goal)) = proof_range_goal(&node.operation) else {
                    continue;
                };
                if function
                    .blocks
                    .iter()
                    .flat_map(|candidate| &candidate.nodes)
                    .filter(|candidate| {
                        proof_range_goal(&candidate.operation)
                            .is_some_and(|(candidate, _, _)| candidate == operation)
                    })
                    .count()
                    != 1
                    || function
                        .facts
                        .iter()
                        .filter(|fact| {
                            matches!(
                                fact,
                                OptimizationFact::OperationObligationReference {
                                    obligation: candidate_obligation,
                                    support,
                                } if *candidate_obligation == obligation && *support == operation
                            )
                        })
                        .count()
                        != 1
                {
                    continue;
                }
                let Ok(Some(proposition)) = goal.kernel_proposition() else {
                    continue;
                };
                let Ok(canonical) =
                    psi_terminal_codec::canonical_proposition_order_key(&proposition)
                else {
                    continue;
                };
                let mut accepted_matches = unit.accepted_obligation_facts.iter().filter(|fact| {
                    fact.machine == function.machine
                        && fact.operation == operation
                        && fact.obligation == obligation
                        && fact.proposition == canonical
                        && fact.psi == unit.psi
                        && fact.has_canonical_identity()
                });
                let Some(accepted) = accepted_matches.next() else {
                    continue;
                };
                if accepted_matches.next().is_some() {
                    continue;
                }
                let mut question_matches = unit.proof_questions.iter().filter(|question| {
                    question.owner
                        == (ProofQuestionOwner::Operation {
                            machine: function.machine,
                            operation,
                        })
                        && question.obligation == obligation
                        && question.proposition == canonical
                        && question.canonical_certificate
                        && question.terminal_psi == unit.psi
                        && question.proof_bundle_fingerprint == accepted.proof_bundle_fingerprint
                        && question.has_canonical_identity()
                });
                let Some(question) = question_matches.next() else {
                    continue;
                };
                if question_matches.next().is_some() {
                    continue;
                }
                let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition)
                else {
                    continue;
                };
                let node_index =
                    u32::try_from(node_index).expect("optimization node position fits u32");
                for ((value, scalar_type), bounds) in bounds {
                    if scalar_value_definition(function, value).is_none_or(|definition| {
                        definition.scalar_type != ScalarType::Integer(scalar_type)
                    }) {
                        continue;
                    }
                    let minimum = bounds.lower.unwrap_or_else(|| scalar_type.minimum_value());
                    let maximum = bounds.upper.unwrap_or_else(|| scalar_type.maximum_value());
                    if minimum == scalar_type.minimum_value()
                        && maximum == scalar_type.maximum_value()
                    {
                        continue;
                    }
                    let support = ValueRangeSupport::AcceptedOperationProof {
                        accepted: accepted.identity,
                        question: question.identity,
                        operation,
                    };
                    let valid_in = ValueRangeRegion {
                        revision: unit.identity,
                        machine: function.machine,
                        value,
                        scope: ValueRangeScope::DominatedOperationEntry {
                            block: block.id,
                            node: node_index,
                            operation,
                        },
                        dominated_blocks: dominated_blocks(&dominators, function.machine, block.id),
                    };
                    facts.push(new_value_range_fact(
                        value,
                        scalar_type,
                        minimum,
                        maximum,
                        support,
                        valid_in,
                    ));
                }
            }
        }
    }
    facts.sort_by_key(|fact| {
        (
            fact.valid_in.machine,
            fact.value,
            fact.valid_in.scope,
            fact.identity,
        )
    });
    ValueRangeAnalysis { facts }
}

fn dominated_blocks(
    dominators: &super::DominatorAnalysis,
    machine: MachineId,
    anchor: BlockId,
) -> Vec<BlockId> {
    let mut blocks = dominators
        .functions
        .iter()
        .find(|(candidate, _)| *candidate == machine)
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter_map(|(block, values)| values.contains(&anchor).then_some(*block))
        .collect::<Vec<_>>();
    blocks.sort_unstable();
    blocks.dedup();
    blocks
}

fn reachable_blocks(function: &PsiOptimizationFunction) -> BTreeSet<BlockId> {
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if !reachable.insert(block) {
            continue;
        }
        let Some(block) = blocks.get(&block) else {
            continue;
        };
        let Some(terminal) = block.nodes.last() else {
            continue;
        };
        pending.extend(
            scalar_operation_successors(&terminal.operation)
                .into_iter()
                .map(|edge| edge.target),
        );
    }
    reachable
}

fn proof_range_goal(
    operation: &O,
) -> Option<(OperationId, psi_core::ObligationId, CanonicalScalarGoal)> {
    let value_term = |id, scalar_type| ScalarTerm::value(id, ScalarType::Integer(scalar_type));
    match operation {
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            value_type,
            count_type,
            count,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactShiftCount {
                value_type: *value_type,
                count_type: *count_type,
                count: value_term(*count, *count_type),
            },
        )),
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            left,
            right,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: *scalar_type,
                left: value_term(*left, *scalar_type),
                right: value_term(*right, *scalar_type),
            },
        )),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: *scalar_type,
                divisor: value_term(*right, *scalar_type),
            },
        )),
        _ => None,
    }
}

fn extract_integer_intervals(proposition: &Proposition) -> IntervalExtraction {
    if proposition.validate().is_err() {
        return IntervalExtraction::Unsupported;
    }
    let mut bounds = BTreeMap::new();
    if !extract_integer_interval_conjunct(proposition, &mut bounds) {
        return IntervalExtraction::Contradiction;
    }
    for ((_, scalar_type), interval) in &bounds {
        let minimum = interval
            .lower
            .unwrap_or_else(|| scalar_type.minimum_value());
        let maximum = interval
            .upper
            .unwrap_or_else(|| scalar_type.maximum_value());
        if integer_value_cmp(*scalar_type, minimum, maximum).is_none_or(|order| order.is_gt()) {
            return IntervalExtraction::Contradiction;
        }
    }
    if bounds.is_empty() {
        IntervalExtraction::Unsupported
    } else {
        IntervalExtraction::Bounds(bounds)
    }
}

fn extract_integer_interval_conjunct(
    proposition: &Proposition,
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
) -> bool {
    match proposition {
        Proposition::Truth => true,
        Proposition::Falsehood => false,
        Proposition::Conjunction(conjuncts) => conjuncts
            .iter()
            .all(|conjunct| extract_integer_interval_conjunct(conjunct, bounds)),
        Proposition::Equal(left, right) => {
            let Some((value, scalar_type, literal)) =
                value_and_literal(left, right).or_else(|| value_and_literal(right, left))
            else {
                return true;
            };
            merge_lower(bounds, value, scalar_type, literal)
                && merge_upper(bounds, value, scalar_type, literal)
        }
        Proposition::LessOrEqual(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                merge_upper(bounds, value, scalar_type, literal)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                merge_lower(bounds, value, scalar_type, literal)
            } else {
                true
            }
        }
        Proposition::LessThan(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                let Some(upper) = predecessor(scalar_type, literal) else {
                    return false;
                };
                merge_upper(bounds, value, scalar_type, upper)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                let Some(lower) = successor(scalar_type, literal) else {
                    return false;
                };
                merge_lower(bounds, value, scalar_type, lower)
            } else {
                true
            }
        }
        Proposition::Atom(_)
        | Proposition::IeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralCaseMembership { .. }
        | Proposition::Disjunction(_)
        | Proposition::Implication { .. }
        | Proposition::ContentConservation(_) => true,
    }
}

fn value_and_literal(
    value: &ScalarTerm,
    literal: &ScalarTerm,
) -> Option<(ValueId, IntegerType, IntegerValue)> {
    let ScalarTerm::Value {
        id,
        scalar_type: ScalarType::Integer(value_type),
    } = value
    else {
        return None;
    };
    let ScalarTerm::Integer {
        scalar_type: literal_type,
        value,
    } = literal
    else {
        return None;
    };
    (*value_type == *literal_type
        && value_type.carrier() == IntegerCarrier::Fixed
        && value_type.admits(*value))
    .then_some((*id, *value_type, *value))
}

fn merge_lower(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    lower: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.lower = match interval.lower {
        None => Some(lower),
        Some(current) => match integer_value_cmp(scalar_type, current, lower) {
            Some(order) if order.is_lt() => Some(lower),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

fn merge_upper(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    upper: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.upper = match interval.upper {
        None => Some(upper),
        Some(current) => match integer_value_cmp(scalar_type, current, upper) {
            Some(order) if order.is_gt() => Some(upper),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

fn integer_value_cmp(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<std::cmp::Ordering> {
    if !scalar_type.admits(left) || !scalar_type.admits(right) {
        return None;
    }
    match (scalar_type.sign(), left, right) {
        (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            Some(left.cmp(&right))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            Some(left.cmp(&right))
        }
        _ => None,
    }
}

fn predecessor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    scalar_type.exact_sub(value, one)
}

fn successor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    scalar_type.exact_add(value, one)
}

fn new_value_range_fact(
    value: ValueId,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    support: ValueRangeSupport,
    valid_in: ValueRangeRegion,
) -> ValueRangeFact {
    let identity =
        value_range_fact_identity(value, scalar_type, minimum, maximum, &support, &valid_in)
            .expect("internally derived value range has a canonical identity");
    ValueRangeFact {
        identity,
        value,
        scalar_type,
        minimum,
        maximum,
        support,
        valid_in,
    }
}

#[cfg(test)]
mod value_range_tests {
    use super::*;
    use psi_core::PropositionId;

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
        IntegerType::new(sign, bits).expect("fixed test integer")
    }

    fn value(raw: u64, scalar_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(id(raw, ValueId::new), ScalarType::Integer(scalar_type))
    }

    fn literal(scalar_type: IntegerType, value: IntegerValue) -> ScalarTerm {
        ScalarTerm::integer(scalar_type, value).expect("admitted test literal")
    }

    #[test]
    fn interval_extraction_is_closed_conjunctive_and_nonconvex_safe() {
        let signed = integer(IntegerSign::Signed, 8);
        let count = id(1, ValueId::new);
        let proposition = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(signed, IntegerValue::Signed(0)), value(1, signed)),
            Proposition::LessOrEqual(value(1, signed), literal(signed, IntegerValue::Signed(63))),
        ]);
        let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition) else {
            panic!("signed shift bounds produce one interval")
        };
        assert_eq!(
            bounds[&(count, signed)].lower,
            Some(IntegerValue::Signed(0))
        );
        assert_eq!(
            bounds[&(count, signed)].upper,
            Some(IntegerValue::Signed(63))
        );

        let unsigned = integer(IntegerSign::Unsigned, 8);
        let divisor = id(2, ValueId::new);
        let IntervalExtraction::Bounds(bounds) =
            extract_integer_intervals(&Proposition::LessOrEqual(
                literal(unsigned, IntegerValue::Unsigned(1)),
                value(2, unsigned),
            ))
        else {
            panic!("unsigned nonzero proof produces a lower bound")
        };
        assert_eq!(
            bounds[&(divisor, unsigned)].lower,
            Some(IntegerValue::Unsigned(1))
        );
        assert_eq!(bounds[&(divisor, unsigned)].upper, None);

        let signed_nonzero = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(value(1, signed), literal(signed, IntegerValue::Signed(-1))),
            Proposition::LessOrEqual(literal(signed, IntegerValue::Signed(1)), value(1, signed)),
        ]);
        assert!(matches!(
            extract_integer_intervals(&signed_nonzero),
            IntervalExtraction::Unsupported
        ));
    }

    #[test]
    fn interval_extraction_rejects_empty_strict_and_conflicting_domains() {
        let unsigned = integer(IntegerSign::Unsigned, 8);
        let minimum = literal(unsigned, unsigned.minimum_value());
        let maximum = literal(unsigned, unsigned.maximum_value());
        assert!(matches!(
            extract_integer_intervals(&Proposition::LessThan(value(1, unsigned), minimum)),
            IntervalExtraction::Contradiction
        ));
        assert!(matches!(
            extract_integer_intervals(&Proposition::LessThan(maximum, value(1, unsigned))),
            IntervalExtraction::Contradiction
        ));
        assert!(matches!(
            extract_integer_intervals(&Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    literal(unsigned, IntegerValue::Unsigned(1)),
                    value(1, unsigned),
                ),
                Proposition::LessOrEqual(
                    value(1, unsigned),
                    literal(unsigned, IntegerValue::Unsigned(0)),
                ),
            ])),
            IntervalExtraction::Contradiction
        ));
    }

    #[test]
    fn unsupported_conjunct_does_not_hide_an_independent_supported_bound() {
        let unsigned = integer(IntegerSign::Unsigned, 8);
        let value_id = id(1, ValueId::new);
        let proposition = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(
                literal(unsigned, IntegerValue::Unsigned(1)),
                value(1, unsigned),
            ),
            Proposition::Atom(id(9, PropositionId::new)),
        ]);
        let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition) else {
            panic!("supported conjunct remains a valid consequence")
        };
        assert_eq!(
            bounds[&(value_id, unsigned)].lower,
            Some(IntegerValue::Unsigned(1))
        );
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
                    support: node
                        .provenance
                        .iter()
                        .chain(node.successors.iter().flat_map(|edge| &edge.provenance))
                        .copied()
                        .collect(),
                    revision: unit.identity,
                });
            }
        }
    }
    EffectSummaryAnalysis {
        nodes,
        functions: transitive_function_effects(unit),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingFunctionEffect {
    observable: EffectKnowledge,
    structural_state: EffectKnowledge,
    crash: EffectKnowledge,
    suspension: EffectKnowledge,
    services: BTreeSet<ServiceId>,
    boundaries: BTreeSet<BoundaryMachineId>,
    support: BTreeSet<PsiProvenance>,
    callees: BTreeSet<MachineId>,
}

fn transitive_function_effects(unit: &PsiOptimizationUnit) -> Vec<FunctionEffectSummary> {
    fn join(left: EffectKnowledge, right: EffectKnowledge) -> EffectKnowledge {
        use EffectKnowledge::{May, No, Yes};
        match (left, right) {
            (May, _) | (_, May) => May,
            (Yes, _) | (_, Yes) => Yes,
            (No, No) => No,
        }
    }

    let machines = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut summaries = BTreeMap::<MachineId, PendingFunctionEffect>::new();
    for function in &unit.functions {
        let mut summary = PendingFunctionEffect {
            observable: EffectKnowledge::No,
            structural_state: EffectKnowledge::No,
            crash: EffectKnowledge::No,
            suspension: EffectKnowledge::No,
            services: BTreeSet::new(),
            boundaries: BTreeSet::new(),
            support: BTreeSet::new(),
            callees: BTreeSet::new(),
        };
        let reachable = semantically_reachable_blocks(function);
        for node in function
            .blocks
            .iter()
            .filter(|block| reachable.contains(&block.id))
            .flat_map(|block| &block.nodes)
        {
            summary.support.extend(node.provenance.iter().copied());
            summary.support.extend(
                node.successors
                    .iter()
                    .flat_map(|edge| edge.provenance.iter().copied()),
            );
            match &node.operation {
                O::CallUnit { callee, .. }
                | O::CallStructuralScalar { callee, .. }
                | O::CallStructural { callee, .. }
                | O::Call { callee, .. } => {
                    summary.callees.insert(*callee);
                    if !machines.contains(callee) {
                        summary.observable = EffectKnowledge::May;
                        summary.structural_state = EffectKnowledge::May;
                        summary.crash = EffectKnowledge::May;
                        summary.suspension = EffectKnowledge::May;
                    }
                }
                O::PortWrite { service, .. } => {
                    summary.services.insert(*service);
                    let (_, observable, structural, crash, suspension) =
                        operation_effect(&node.operation);
                    summary.observable = join(summary.observable, observable);
                    summary.structural_state = join(summary.structural_state, structural);
                    summary.crash = join(summary.crash, crash);
                    summary.suspension = join(summary.suspension, suspension);
                }
                O::BoundaryCall { boundary, .. } => {
                    summary.boundaries.insert(*boundary);
                    let (_, observable, structural, crash, suspension) =
                        operation_effect(&node.operation);
                    summary.observable = join(summary.observable, observable);
                    summary.structural_state = join(summary.structural_state, structural);
                    summary.crash = join(summary.crash, crash);
                    summary.suspension = join(summary.suspension, suspension);
                }
                _ => {
                    let (_, observable, structural, crash, suspension) =
                        operation_effect(&node.operation);
                    summary.observable = join(summary.observable, observable);
                    summary.structural_state = join(summary.structural_state, structural);
                    summary.crash = join(summary.crash, crash);
                    summary.suspension = join(summary.suspension, suspension);
                }
            }
        }
        summaries.insert(function.machine, summary);
    }

    loop {
        let prior = summaries.clone();
        let mut changed = false;
        for summary in summaries.values_mut() {
            for callee in summary.callees.clone() {
                let Some(callee) = prior.get(&callee) else {
                    continue;
                };
                let before = summary.clone();
                summary.observable = join(summary.observable, callee.observable);
                summary.structural_state = join(summary.structural_state, callee.structural_state);
                summary.crash = join(summary.crash, callee.crash);
                summary.suspension = join(summary.suspension, callee.suspension);
                summary.services.extend(callee.services.iter().copied());
                summary.boundaries.extend(callee.boundaries.iter().copied());
                summary.support.extend(callee.support.iter().copied());
                changed |= *summary != before;
            }
        }
        if !changed {
            break;
        }
    }

    summaries
        .into_iter()
        .map(|(machine, summary)| FunctionEffectSummary {
            machine,
            observable: summary.observable,
            structural_state: summary.structural_state,
            crash: summary.crash,
            suspension: summary.suspension,
            services: summary.services.into_iter().collect(),
            boundaries: summary.boundaries.into_iter().collect(),
            support: summary.support.into_iter().collect(),
            revision: unit.identity,
        })
        .collect()
}

fn semantically_reachable_blocks(function: &PsiOptimizationFunction) -> BTreeSet<BlockId> {
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .last()
                    .map(|node| scalar_operation_successors(&node.operation))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|edge| edge.target)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    reachable
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
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
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

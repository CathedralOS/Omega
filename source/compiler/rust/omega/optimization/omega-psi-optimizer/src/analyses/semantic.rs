use std::collections::BTreeMap;

use omega_optimization_core::OptimizationUnitIdentity;
use omega_optimization_unit::{
    OptimizationFact, PsiOptimizationUnit, PsiProvenance, ValueDefinition, ValueUse,
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

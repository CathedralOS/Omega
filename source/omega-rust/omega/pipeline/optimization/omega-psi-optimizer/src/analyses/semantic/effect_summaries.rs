use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::OptimizationUnitIdentity;
use omega_optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance};
use psi_core::{BlockId, BoundaryMachineId, MachineId, ServiceId};

use super::shared::scalar_operation_successors;

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

pub(in crate::analyses) fn effect_summaries(unit: &PsiOptimizationUnit) -> EffectSummaryAnalysis {
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
                O::CallStructuralScalarWithDynamicArguments {
                    callee,
                    dynamic_arguments,
                    ..
                }
                | O::CallUnitWithDynamicArguments {
                    callee,
                    dynamic_arguments,
                    ..
                } => {
                    summary.callees.insert(*callee);
                    for argument in dynamic_arguments {
                        if let omega_abstract_operations::AbstractDynamicDescriptorSource::Selection {
                            application,
                            ..
                        }
                        | omega_abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                            application,
                            ..
                        } = &argument.source {
                            summary.callees.extend(
                                application
                                    .realization_callables
                                    .iter()
                                    .map(|callable| callable.machine),
                            );
                        }
                    }
                    if summary
                        .callees
                        .iter()
                        .any(|callee| !machines.contains(callee))
                    {
                        summary.observable = EffectKnowledge::May;
                        summary.structural_state = EffectKnowledge::May;
                        summary.crash = EffectKnowledge::May;
                        summary.suspension = EffectKnowledge::May;
                    }
                }
                O::CallDynamicScalar {
                    dynamic_dispatch, ..
                }
                | O::CallDynamicUnit {
                    dynamic_dispatch, ..
                } => {
                    let callee = dynamic_dispatch.dispatch.realization;
                    summary.callees.insert(callee);
                    if !machines.contains(&callee) {
                        summary.observable = EffectKnowledge::May;
                        summary.structural_state = EffectKnowledge::May;
                        summary.crash = EffectKnowledge::May;
                        summary.suspension = EffectKnowledge::May;
                    }
                }
                O::CallDynamicParameterScalar { .. } | O::CallDynamicParameterUnit { .. } => {
                    // The concrete table row is an incoming runtime value.
                    // Until target realization rejoins every caller-supplied
                    // descriptor, retain the conservative internal-call
                    // summary without inventing one static callee.
                    summary.observable = EffectKnowledge::May;
                    summary.structural_state = EffectKnowledge::May;
                    summary.crash = EffectKnowledge::May;
                    summary.suspension = EffectKnowledge::May;
                }
                O::PortWrite { service, .. } => {
                    summary.services.insert(*service);
                    join_operation_effect(&mut summary, &node.operation, join);
                }
                O::BoundaryCall { boundary, .. } => {
                    summary.boundaries.insert(*boundary);
                    join_operation_effect(&mut summary, &node.operation, join);
                }
                _ => join_operation_effect(&mut summary, &node.operation, join),
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

fn join_operation_effect(
    summary: &mut PendingFunctionEffect,
    operation: &O,
    join: fn(EffectKnowledge, EffectKnowledge) -> EffectKnowledge,
) {
    let (_, observable, structural, crash, suspension) = operation_effect(operation);
    summary.observable = join(summary.observable, observable);
    summary.structural_state = join(summary.structural_state, structural);
    summary.crash = join(summary.crash, crash);
    summary.suspension = join(summary.suspension, suspension);
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
        O::DynamicDescriptorParameter { .. } => (EffectClass::StructuralState, No, No, No, No),
        O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
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
        O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishAffineScalarRecord { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. }
        | O::ReturnStructural { .. } => (EffectClass::StructuralState, No, Yes, No, No),
        O::CallUnit { .. }
        | O::CallUnitWithDynamicArguments { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructuralScalarWithDynamicArguments { .. }
        | O::CallDynamicScalar { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallDynamicUnit { .. }
        | O::CallDynamicParameterUnit { .. }
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

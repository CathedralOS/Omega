use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{PsiOptimizationUnit, PsiProvenance};
use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, ServiceId};

use super::semantically_reachable_blocks;

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
                // A call that forwards descriptors also reaches every
                // realization its descriptors select.
                O::CallStructural {
                    callee,
                    dynamic_arguments,
                    ..
                } if !dynamic_arguments.is_empty() => {
                    summary.callees.insert(*callee);
                    for argument in dynamic_arguments {
                        if let abstract_operations::AbstractDynamicDescriptorSource::Selection {
                            application,
                            ..
                        }
                        | abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                            application,
                            ..
                        } = &argument.source
                        {
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
                O::CallStructural { callee, .. } | O::Call { callee, .. } => {
                    summary.callees.insert(*callee);
                    if !machines.contains(callee) {
                        summary.observable = EffectKnowledge::May;
                        summary.structural_state = EffectKnowledge::May;
                        summary.crash = EffectKnowledge::May;
                        summary.suspension = EffectKnowledge::May;
                    }
                }
                O::CallDynamic { dispatch, .. } => match dispatch.realization() {
                    // Rebound and stored custody name their selected row.
                    Some(callee) => {
                        summary.callees.insert(callee);
                        if !machines.contains(&callee) {
                            summary.observable = EffectKnowledge::May;
                            summary.structural_state = EffectKnowledge::May;
                            summary.crash = EffectKnowledge::May;
                            summary.suspension = EffectKnowledge::May;
                        }
                    }
                    // A parameter descriptor's table row is an incoming
                    // runtime value. Until target realization rejoins every
                    // caller-supplied descriptor, retain the conservative
                    // internal-call summary without inventing one static
                    // callee.
                    None => {
                        summary.observable = EffectKnowledge::May;
                        summary.structural_state = EffectKnowledge::May;
                        summary.crash = EffectKnowledge::May;
                        summary.suspension = EffectKnowledge::May;
                    }
                },
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
        | O::IeeeFloatCompare { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::ScalarRepresentationView { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::IntegerShift { .. }
        | O::IntegerArithmetic { .. } => (EffectClass::PureScalar, No, No, No, No),
        O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::WriteOnlyPrimitiveStore { .. }
        | O::ByteSequenceWrite { .. }
        | O::ByteSequenceStore { .. } | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::StructuralScalarFieldStore { .. }
        // A restoration window extracts and reseats storage beneath a
        // mutable-borrowed root: structural state, unobservable from scalar
        // consumers but never erasable, duplicable, or reorderable.
        | O::MoveStructuralField { .. }
        | O::StoreStructuralField { .. }
        // A leaf copy observes the readable root's subtree and establishes
        // a fresh owned value: structural state, unobservable from scalar
        // consumers but never erasable, duplicable, or reorderable.
        | O::StructuralLeafCopy { .. }
        | O::StoreDynamicDescriptor { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishRecord { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequencePredicate { .. }
        | O::ByteSequenceEqual { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceLength { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | O::ElementViewRead { .. }
        | O::ElementViewWrite { .. }
        | O::ElementViewSubslice { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::StructuralByteSequenceFieldRead { .. }
        | O::StructuralScalarFieldRead { .. }
        | O::ReturnStructural { .. } => (EffectClass::StructuralState, No, Yes, No, No),
        O::AtomicEvent { event, .. } => {
            // Atomic memory is shared across activations: every atomic
            // event is observable, so none may be erased, reordered into
            // pure scalar computation, or folded by a scalar-only pass.
            // A place-carrying event touches structural state; a fence
            // accesses no place but remains an ordering event.
            (
                EffectClass::StructuralState,
                Yes,
                if event.place().is_some() { Yes } else { No },
                No,
                No,
            )
        }
        O::CallStructural { .. }
        | O::CallDynamic { .. }
        | O::Call { .. } => (EffectClass::InternalCall, May, May, May, May),
        O::BoundaryCall { .. } => (EffectClass::BoundaryCall, Yes, May, May, May),
        O::PortWrite { .. } => (EffectClass::Service, Yes, No, No, No),
        O::Crash { .. } => (EffectClass::Control, Yes, No, Yes, No),
        // A Trapping primitive defines a scalar but may leave through its own
        // `Trap` crash: that exit is observable, so it is not pure scalar
        // work, and it happens only when its operands meet the predicate.
        O::TrappingInteger { .. } => (EffectClass::Control, May, No, May, No),
        O::FloatResultPolicy { policy, .. } => {
            if policy.can_trap() {
                (EffectClass::Control, May, No, May, No)
            } else {
                (EffectClass::PureScalar, No, No, No, No)
            }
        }
        O::StructuralCase { .. } => (EffectClass::Control, No, Yes, No, No),
        O::Jump { .. } | O::Conditional { .. } | O::Return { .. } | O::ReturnUnit { .. } => {
            (EffectClass::Control, No, May, No, No)
        }
    }
}

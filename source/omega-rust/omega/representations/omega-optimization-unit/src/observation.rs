use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{OptimizationUnitIdentity, OwnershipFrontierFactIdentity};
use psi_core::{BlockId, MachineId};

use crate::{
    EffectLink, FuelSettlement, NodeLocation, OptimizationEdge, OwnershipEvent,
    OwnershipFrontierSite, PsiOptimizationUnit, PsiProvenance, ValueDefinition, ValueUse,
};

/// Whether one exit class is absent, possible through opaque work, or certain
/// at this exact node. `May` is deliberately not narrowed from call syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationKnowledge {
    No,
    May,
    Yes,
}

/// Externally relevant event axis represented by a Terminal-Psi-derived node.
/// The complete operation is retained beside the class so comparisons cannot
/// accidentally erase a callee, boundary, service, edge, cleanup, or crash
/// payload while considering only its broad category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationEventClass {
    StructuralState,
    InternalCall,
    BoundaryCall,
    Service,
    ControlTransfer,
    NormalExit,
    CrashExit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiObservableEvent {
    pub class: ObservationEventClass,
    pub operation: O,
}

/// Compiler-owned observation row for one optimization node.
///
/// Empty `events` means the operation is a pure scalar computation under its
/// already-verified exact Terminal semantics. It does not mean provenance,
/// logical fuel, definitions, or live uses may be dropped: those axes remain
/// explicit in every row and a closed-region validator must join liveness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiNodeObservation {
    pub machine: psi_core::MachineId,
    pub block: psi_core::BlockId,
    pub node: u32,
    /// Full operation semantics are retained even when `events` is empty.
    /// Structural-identity validators may normalize exact scalar-use slots,
    /// but cannot thereby hide an arithmetic-policy, call, edge, or exit
    /// change.
    pub operation: O,
    pub definitions: Vec<ValueDefinition>,
    pub uses: Vec<ValueUse>,
    pub successors: Vec<OptimizationEdge>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
    pub provenance: Vec<PsiProvenance>,
    pub fuel: Vec<FuelSettlement>,
    pub crash: ObservationKnowledge,
    pub suspension: ObservationKnowledge,
    pub events: Vec<PsiObservableEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiObservationModel {
    pub revision: OptimizationUnitIdentity,
    pub nodes: Vec<PsiNodeObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiRegionBoundaryEdgeObservation {
    pub source: NodeLocation,
    pub edge: OptimizationEdge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiRegionFrontierObservation {
    pub site: OwnershipFrontierSite,
    /// Absence is explicit because synthetic/bare optimization units do not
    /// carry the verifier-owned source frontier catalog. A retained identity
    /// is never broadened into a newly inferred current-region fact.
    pub identity: Option<OwnershipFrontierFactIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiClosedRegionBlockObservation {
    pub block: BlockId,
    pub parameters: Vec<ValueDefinition>,
    pub nodes: Vec<PsiNodeObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiClosedRegionSemantics {
    pub machine: MachineId,
    pub blocks: Vec<PsiClosedRegionBlockObservation>,
    pub scalar_live_ins: Vec<ValueDefinition>,
    pub scalar_live_outs: Vec<ValueDefinition>,
    pub incoming_edges: Vec<PsiRegionBoundaryEdgeObservation>,
    pub outgoing_edges: Vec<PsiRegionBoundaryEdgeObservation>,
    pub retained_source_frontiers: Vec<PsiRegionFrontierObservation>,
}

/// Canonical observation of one closed set of whole blocks. The source
/// revision is retained for custody but is intentionally outside `semantics`:
/// an accepted rewrite must change revision while preserving the normalized
/// closed-region question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiClosedRegionObservation {
    pub source_revision: OptimizationUnitIdentity,
    pub semantics: PsiClosedRegionSemantics,
}

/// Reconstruct the observation boundary from the immutable unit. The exhaustive
/// operation match is intentional: adding a abstract operation cannot
/// compile until its observation class is chosen here.
pub fn reconstruct_psi_observation_model(unit: &PsiOptimizationUnit) -> PsiObservationModel {
    let mut nodes = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node, operation) in block.nodes.iter().enumerate() {
                let (events, crash, suspension) = operation_observations(&operation.operation);
                nodes.push(PsiNodeObservation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node).expect("optimization node index is u32"),
                    operation: operation.operation.clone(),
                    definitions: operation.definitions.clone(),
                    uses: operation.uses.clone(),
                    successors: operation.successors.clone(),
                    effect: operation.effect,
                    ownership: operation.ownership.clone(),
                    provenance: operation.provenance.clone(),
                    fuel: operation.fuel.clone(),
                    crash,
                    suspension,
                    events,
                });
            }
        }
    }
    PsiObservationModel {
        revision: unit.identity,
        nodes,
    }
}

/// Reconstruct a canonical whole-block region directly from unit content.
/// Caller order and duplicates cannot affect the result. This function applies
/// no rewrite-specific normalization; independent validators must construct
/// any permitted normalization before asking this model to observe it.
pub fn reconstruct_psi_closed_region_observation(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    blocks: &[BlockId],
) -> Option<PsiClosedRegionObservation> {
    let requested = blocks.iter().copied().collect::<BTreeSet<_>>();
    if requested.is_empty() {
        return None;
    }
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    if requested.iter().any(|block| {
        !function
            .blocks
            .iter()
            .any(|candidate| candidate.id == *block)
    }) {
        return None;
    }

    let all_definitions = function
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
        .map(|definition| (definition.value, *definition))
        .collect::<BTreeMap<_, _>>();
    let internal_definitions = function
        .blocks
        .iter()
        .filter(|block| requested.contains(&block.id))
        .flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        })
        .map(|definition| (definition.value, *definition))
        .collect::<BTreeMap<_, _>>();

    let model = reconstruct_psi_observation_model(unit);
    let rows = model
        .nodes
        .into_iter()
        .filter(|row| row.machine == machine)
        .map(|row| ((row.block, row.node), row))
        .collect::<BTreeMap<_, _>>();
    let mut region_blocks = Vec::with_capacity(requested.len());
    let mut live_in_values = BTreeSet::new();
    let mut live_out_values = BTreeSet::new();
    let mut frontier_sites = BTreeSet::new();
    let mut incoming_edges = Vec::new();
    let mut outgoing_edges = Vec::new();

    for block in function
        .blocks
        .iter()
        .filter(|block| requested.contains(&block.id))
    {
        frontier_sites.insert(OwnershipFrontierSite::BlockEntry(block.id));
        let mut nodes = Vec::with_capacity(block.nodes.len());
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).ok()?;
            for use_site in &node.uses {
                if !internal_definitions.contains_key(&use_site.value) {
                    live_in_values.insert(use_site.value);
                }
            }
            for provenance in &node.provenance {
                match provenance {
                    PsiProvenance::Operation(operation) => {
                        frontier_sites.insert(OwnershipFrontierSite::OperationEntry(*operation));
                        frontier_sites.insert(OwnershipFrontierSite::OperationExit(*operation));
                    }
                    PsiProvenance::Edge(edge) => {
                        frontier_sites.insert(OwnershipFrontierSite::EdgeEntry(*edge));
                        frontier_sites.insert(OwnershipFrontierSite::EdgeExit(*edge));
                    }
                }
            }
            for edge in &node.successors {
                for provenance in &edge.provenance {
                    match provenance {
                        PsiProvenance::Operation(operation) => {
                            frontier_sites
                                .insert(OwnershipFrontierSite::OperationEntry(*operation));
                            frontier_sites.insert(OwnershipFrontierSite::OperationExit(*operation));
                        }
                        PsiProvenance::Edge(edge) => {
                            frontier_sites.insert(OwnershipFrontierSite::EdgeEntry(*edge));
                            frontier_sites.insert(OwnershipFrontierSite::EdgeExit(*edge));
                        }
                    }
                }
                if !requested.contains(&edge.target) {
                    for binding in &edge.bindings {
                        if internal_definitions.contains_key(&binding.argument) {
                            live_out_values.insert(binding.argument);
                        }
                    }
                    outgoing_edges.push(PsiRegionBoundaryEdgeObservation {
                        source: NodeLocation {
                            machine,
                            block: block.id,
                            node: node_index,
                        },
                        edge: edge.clone(),
                    });
                }
            }
            nodes.push(rows.get(&(block.id, node_index))?.clone());
        }
        region_blocks.push(PsiClosedRegionBlockObservation {
            block: block.id,
            parameters: block.parameters.clone(),
            nodes,
        });
    }
    region_blocks.sort_by_key(|block| block.block);

    for block in function
        .blocks
        .iter()
        .filter(|block| !requested.contains(&block.id))
    {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for use_site in &node.uses {
                if internal_definitions.contains_key(&use_site.value) {
                    live_out_values.insert(use_site.value);
                }
            }
            for edge in &node.successors {
                if requested.contains(&edge.target) {
                    for binding in &edge.bindings {
                        live_in_values.insert(binding.argument);
                    }
                    incoming_edges.push(PsiRegionBoundaryEdgeObservation {
                        source: NodeLocation {
                            machine,
                            block: block.id,
                            node: u32::try_from(node_index).ok()?,
                        },
                        edge: edge.clone(),
                    });
                }
            }
        }
    }
    incoming_edges.sort_by_key(|row| {
        (
            row.source.block,
            row.source.node,
            row.edge.psi_edge,
            row.edge.target,
        )
    });
    outgoing_edges.sort_by_key(|row| {
        (
            row.source.block,
            row.source.node,
            row.edge.psi_edge,
            row.edge.target,
        )
    });

    let scalar_live_ins = live_in_values
        .into_iter()
        .map(|value| all_definitions.get(&value).copied())
        .collect::<Option<Vec<_>>>()?;
    let scalar_live_outs = live_out_values
        .into_iter()
        .map(|value| internal_definitions.get(&value).copied())
        .collect::<Option<Vec<_>>>()?;
    let retained_source_frontiers = frontier_sites
        .into_iter()
        .map(|site| PsiRegionFrontierObservation {
            site,
            identity: unit
                .ownership_frontier_facts
                .iter()
                .find(|fact| fact.machine == machine && fact.site == site)
                .map(|fact| fact.identity),
        })
        .collect();

    Some(PsiClosedRegionObservation {
        source_revision: unit.identity,
        semantics: PsiClosedRegionSemantics {
            machine,
            blocks: region_blocks,
            scalar_live_ins,
            scalar_live_outs,
            incoming_edges,
            outgoing_edges,
            retained_source_frontiers,
        },
    })
}

fn operation_observations(
    operation: &O,
) -> (
    Vec<PsiObservableEvent>,
    ObservationKnowledge,
    ObservationKnowledge,
) {
    use ObservationEventClass as C;
    use ObservationKnowledge::{May, No, Yes};

    let event = |class| PsiObservableEvent {
        class,
        operation: operation.clone(),
    };
    match operation {
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
        | O::SaturatingIntegerMultiply { .. } => (Vec::new(), No, No),
        O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. } => (vec![event(C::StructuralState)], No, No),
        O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallDynamicScalar { .. }
        | O::CallStructural { .. }
        | O::Call { .. } => (vec![event(C::InternalCall)], May, May),
        O::BoundaryCall { .. } => (vec![event(C::BoundaryCall)], May, May),
        O::PortWrite { .. } => (vec![event(C::Service)], No, No),
        O::Jump { .. } | O::Conditional { .. } => (vec![event(C::ControlTransfer)], No, No),
        O::Return { .. } | O::ReturnUnit { .. } => (vec![event(C::NormalExit)], No, No),
        O::ReturnStructural { .. } => (
            vec![event(C::StructuralState), event(C::NormalExit)],
            No,
            No,
        ),
        O::Crash { .. } => (vec![event(C::CrashExit)], Yes, No),
    }
}

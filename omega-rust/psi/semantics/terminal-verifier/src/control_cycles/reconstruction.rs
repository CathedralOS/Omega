//! One natural well-foundedness citation and exact per-edge proof questions.

use std::collections::BTreeMap;

use proof_admission::{
    CertificateObligation, Obligation, ObligationClass, RecursiveComponentObligation,
    RecursiveEdgeObligation,
};
use semantic_vocabulary::{
    BlockId, CycleComponentId, EdgeId, IntegerSign, ObligationId, Proposition, PsiSemanticId,
    RankingRelationId, ScalarTerm, ScalarType,
};
use sha2::{Digest, Sha256};
use terminal_psi::{
    TerminalMachine, TerminalModule, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalRankedScc,
};

use super::ReconstructedControlCycleObligation;
use crate::{ModuleError, validate_module_representation};

/// Canonical topology for a producer retaining a selected source witness.
/// No proof or progress authority is returned by this query.
pub fn control_cycle_members(machine: &TerminalMachine) -> Result<Vec<Vec<BlockId>>, ModuleError> {
    check_control_graph(machine)?;
    Ok(crate::control_graph::cyclic_components(machine))
}

/// Canonical iteration entries dominating a member: the feedback-edge targets
/// above it in the dominance tree. A member inside nested loops has one entry
/// per enclosing natural cycle, not only the component's outermost arrival.
/// No proof or progress authority is returned by this query.
pub fn dominating_control_cycle_entries(
    machine: &TerminalMachine,
    member: BlockId,
) -> Result<Vec<BlockId>, ModuleError> {
    check_control_graph(machine)?;
    let dominators = crate::control_graph::dominators(machine);
    let mut entries = crate::control_graph::feedback_edges(machine)
        .values()
        .copied()
        .filter(|entry| dominators.dominates(*entry, member))
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

fn check_control_graph(machine: &TerminalMachine) -> Result<(), ModuleError> {
    let blocks = machine
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<std::collections::BTreeSet<_>>();
    if blocks.len() != machine.blocks.len() || !blocks.contains(&machine.entry) {
        return Err(ModuleError::InvalidRankedScc(machine.id));
    }
    let outgoing = crate::control_graph::successors(machine);
    for (_, target) in outgoing.values().flatten() {
        if !blocks.contains(target) {
            return Err(ModuleError::UnknownTargetBlock(*target));
        }
    }
    let mut reached = std::collections::BTreeSet::new();
    let mut pending = vec![machine.entry];
    while let Some(block) = pending.pop() {
        if reached.insert(block) {
            pending.extend(outgoing[&block].iter().map(|(_, target)| *target));
        }
    }
    if let Some(block) = blocks.difference(&reached).next() {
        return Err(ModuleError::UnreachableBlock(*block));
    }
    Ok(())
}

pub fn reconstruct_control_cycle_obligations(
    module: &TerminalModule,
) -> Result<Vec<ReconstructedControlCycleObligation>, ModuleError> {
    validate_module_representation(module)?;
    reconstruct_validated_control_cycle_obligations(module)
}

/// The canonical natural-cycle rows a machine's retained ranking claims.
pub fn control_cycle_components(
    machine: &TerminalMachine,
) -> Result<Vec<TerminalNaturalCycle>, ModuleError> {
    match &machine.ranked_scc {
        None => Ok(Vec::new()),
        Some(TerminalRankedScc::Natural(components)) => Ok(components.clone()),
    }
}

pub(crate) fn reconstruct_validated_control_cycle_obligations(
    module: &TerminalModule,
) -> Result<Vec<ReconstructedControlCycleObligation>, ModuleError> {
    let mut questions = Vec::new();
    for machine in &module.machines {
        let components = control_cycle_components(machine)?;
        if components.is_empty() {
            continue;
        }
        let edge_axioms =
            crate::verification::reconstruct_validated_control_edge_axioms(module, machine)?;
        for component in &components {
            questions.push(natural_cycle_question(machine, component, &edge_axioms)?);
        }
    }
    questions.sort_by_key(|question| question.component);
    Ok(questions)
}

fn natural_cycle_question(
    machine: &TerminalMachine,
    component: &TerminalNaturalCycle,
    edge_axioms: &BTreeMap<EdgeId, Vec<Proposition>>,
) -> Result<ReconstructedControlCycleObligation, ModuleError> {
    let commitment = question_commitment(machine, component);
    let relation = natural_relation(component);
    let well_foundedness = natural_well_foundedness(component);
    let ranks = component
        .ranks
        .iter()
        .map(|rank| (rank.block, rank.value))
        .collect::<BTreeMap<_, _>>();
    let mut edges = Vec::new();
    for edge in &component.edges {
        let mut identity = Sha256::new();
        identity.update(b"psi.control-cycle.edge-obligation.v1\0");
        identity.update(commitment);
        identity.update(edge.edge.get().to_le_bytes());
        let before = ScalarTerm::value(
            ranks[&edge.source],
            ScalarType::Integer(component.rank_type),
        );
        let after = ScalarTerm::value(
            edge.successor_rank,
            ScalarType::Integer(component.rank_type),
        );
        let proposition = match edge.comparison {
            TerminalNaturalRankComparison::Preserving => Proposition::LessOrEqual(after, before),
            TerminalNaturalRankComparison::Strict => Proposition::LessThan(after, before),
        };
        edges.push(RecursiveEdgeObligation {
            caller: edge.source,
            callee: edge.target,
            decrease: CertificateObligation {
                obligation: Obligation {
                    id: semantic_id(&identity.finalize().into()),
                    proposition,
                    class: ObligationClass::Derivable,
                },
                assumptions: machine.contract.requires.clone(),
                semantic_axioms: edge_axioms
                    .get(&edge.edge)
                    .cloned()
                    .ok_or(ModuleError::InvalidRankedScc(machine.id))?,
            },
        });
    }
    edges.sort_by_key(|edge| edge.decrease.obligation.id);
    Ok(ReconstructedControlCycleObligation {
        machine: machine.id,
        component: control_cycle_identity(machine, component),
        obligation: RecursiveComponentObligation {
            members: component.ranks.iter().map(|rank| rank.block).collect(),
            ranking_relation: Some(relation),
            well_foundedness,
            edges,
        },
    })
}

/// Topology identity has no producer-supplied component key. Rank bindings
/// separately enter the question commitment, so changing a witness invalidates
/// its comparison evidence even when the component topology is unchanged.
pub fn control_cycle_identity(
    machine: &TerminalMachine,
    component: &TerminalNaturalCycle,
) -> CycleComponentId {
    let edges = component
        .edges
        .iter()
        .map(|edge| (edge.edge, edge.source, edge.target));
    topology_identity(machine, edges)
}

/// Topology-derived component identity for a cyclic component that carries no
/// producer ranking row — the name an absence-of-bound report uses when an
/// unranked component is the directed cause. Validation pins a producer's
/// component edges to ascending edge-id order covering exactly the members'
/// internal edges, so for any member set a producer later ranks this identity
/// equals `control_cycle_identity` on that row.
pub fn cyclic_component_identity(
    machine: &TerminalMachine,
    members: &[BlockId],
) -> CycleComponentId {
    let member_set: std::collections::BTreeSet<BlockId> = members.iter().copied().collect();
    let outgoing = crate::control_graph::successors(machine);
    let mut internal: Vec<(EdgeId, BlockId, BlockId)> = Vec::new();
    for member in members {
        for (edge, target) in outgoing.get(member).into_iter().flatten() {
            if member_set.contains(target) {
                internal.push((*edge, *member, *target));
            }
        }
    }
    internal.sort();
    topology_identity(machine, internal)
}

fn topology_identity(
    machine: &TerminalMachine,
    edges: impl IntoIterator<Item = (EdgeId, BlockId, BlockId)>,
) -> CycleComponentId {
    let mut digest = Sha256::new();
    digest.update(b"psi.control-cycle.topology.v1\0");
    digest.update(machine.id.get().to_le_bytes());
    for (edge, source, target) in edges {
        digest.update(edge.get().to_le_bytes());
        digest.update(source.get().to_le_bytes());
        digest.update(target.get().to_le_bytes());
    }
    semantic_id(&digest.finalize().into())
}

fn question_commitment(machine: &TerminalMachine, component: &TerminalNaturalCycle) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"psi.control-cycle.question.v1\0");
    digest.update(
        control_cycle_identity(machine, component)
            .get()
            .to_le_bytes(),
    );
    digest.update(natural_relation(component).get().to_le_bytes());
    for rank in &component.ranks {
        digest.update(rank.block.get().to_le_bytes());
        digest.update(rank.value.get().to_le_bytes());
    }
    for edge in &component.edges {
        digest.update(edge.edge.get().to_le_bytes());
        digest.update(edge.successor_rank.get().to_le_bytes());
        digest.update([match edge.comparison {
            TerminalNaturalRankComparison::Preserving => 0,
            TerminalNaturalRankComparison::Strict => 1,
        }]);
    }
    digest.finalize().into()
}

fn natural_relation(component: &TerminalNaturalCycle) -> RankingRelationId {
    let mut digest = Sha256::new();
    digest.update(match component.rank_type.sign() {
        IntegerSign::Unsigned => b"psi.control-cycle.fixed-unsigned-natural-order.v1\0".as_slice(),
        IntegerSign::Signed => b"psi.control-cycle.fixed-signed-integer-order.v1\0".as_slice(),
    });
    digest.update(component.rank_type.bits().to_le_bytes());
    semantic_id(&digest.finalize().into())
}

fn natural_well_foundedness(component: &TerminalNaturalCycle) -> CertificateObligation {
    // Validation admits only fixed carriers. An unsigned carrier embeds in the
    // naturals; a signed one, shifted by its minimum, embeds in a finite
    // initial segment of them. Either way strict decrease is well founded.
    // The sign enters the relation identity, so the two orders never share a
    // citation. This says nothing about callee progress or a fuel bound.
    let mut digest = Sha256::new();
    digest.update(b"psi.control-cycle.natural-well-foundedness.v1\0");
    digest.update(natural_relation(component).get().to_le_bytes());
    let commitment: [u8; 32] = digest.finalize().into();
    let proposition = Proposition::Atom(semantic_id(&commitment));
    let mut obligation = Sha256::new();
    obligation.update(b"psi.control-cycle.well-founded-obligation.v1\0");
    obligation.update(commitment);
    CertificateObligation {
        obligation: Obligation {
            id: semantic_id::<ObligationId>(&obligation.finalize().into()),
            proposition: proposition.clone(),
            class: ObligationClass::Derivable,
        },
        assumptions: Vec::new(),
        semantic_axioms: vec![proposition],
    }
}

fn semantic_id<Identity: PsiSemanticId>(digest: &[u8; 32]) -> Identity {
    let raw = u64::from_le_bytes(digest[..8].try_into().expect("SHA-256 prefix")) | 1;
    Identity::new(raw).expect("the low bit establishes a nonzero semantic identity")
}

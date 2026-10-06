//! One natural well-foundedness citation and exact per-edge proof questions.

use std::collections::BTreeMap;

use proof_admission::{
    CertificateObligation, Obligation, ObligationClass, RecursiveComponentObligation,
    RecursiveEdgeObligation,
};
use semantic_vocabulary::{
    EdgeId, IntegerSign, ObligationId, Proposition, PsiSemanticId, RankingRelationId, ScalarTerm,
    ScalarType,
};
use sha2::{Digest, Sha256};
use terminal_psi::{
    TerminalMachine, TerminalModule, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalRankedScc,
};

use super::ReconstructedControlCycleObligation;
use crate::{ModuleError, validate_module_representation};
use terminal_semantics::control_graph::control_cycle_identity;

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

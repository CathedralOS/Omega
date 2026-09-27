//! Verifier-owned reconstruction of proof-only recursive-component questions.

use proof_admission::{
    CertificateObligation, Obligation, ObligationClass, RecursiveComponentObligation,
    RecursiveEdgeObligation,
};
use semantic_vocabulary::{
    ObligationId, Proposition, PsiSemanticId, RankingRelationId, RecursiveComponentId,
};
use sha2::{Digest, Sha256};
use terminal_psi::{
    TerminalModule, TerminalProofRankingRelation, TerminalProofRecursiveCallSite,
    TerminalProofRecursiveComponent, TerminalProofRecursiveEdge,
    TerminalProofRecursiveTransitionLane,
};

use crate::{ModuleError, validate_module_representation};

const COMPONENT_DOMAIN: &[u8] = b"psi.proof-recursion.component.v1\0";
const RELATION_DOMAIN: &[u8] = b"psi.proof-recursion.relation.v1\0";
const WELL_FOUNDED_DOMAIN: &[u8] = b"psi.proof-recursion.well-founded.v1\0";
const EDGE_DOMAIN: &[u8] = b"psi.proof-recursion.edge.v1\0";
const OBLIGATION_DOMAIN: &[u8] = b"psi.proof-recursion.obligation.v1\0";

/// Reconstruct the complete recursive proof question from a canonical module.
/// A proof bundle cannot add components, choose members, remove call sites, or
/// select a different ranking relation.
pub fn reconstruct_proof_recursive_component_obligations(
    module: &TerminalModule,
) -> Result<Vec<RecursiveComponentObligation>, ModuleError> {
    validate_module_representation(module)?;
    Ok(module
        .proof_recursive_components
        .iter()
        .map(reconstruct_component)
        .collect())
}

pub(crate) fn reconstruct_validated_proof_recursive_component_obligations(
    module: &TerminalModule,
) -> Vec<RecursiveComponentObligation> {
    module
        .proof_recursive_components
        .iter()
        .map(reconstruct_component)
        .collect()
}

pub(crate) fn proof_recursive_obligation_ids(
    component: &TerminalProofRecursiveComponent,
) -> Vec<ObligationId> {
    let reconstructed = reconstruct_component(component);
    std::iter::once(reconstructed.well_foundedness.obligation.id)
        .chain(
            reconstructed
                .edges
                .into_iter()
                .map(|edge| edge.decrease.obligation.id),
        )
        .collect()
}

fn reconstruct_component(
    component: &TerminalProofRecursiveComponent,
) -> RecursiveComponentObligation {
    let component_commitment = component_commitment(component);
    let well_foundedness = committed_obligation(WELL_FOUNDED_DOMAIN, &component_commitment);
    let mut edges = component
        .edges
        .iter()
        .map(|edge| {
            let decrease = committed_obligation(EDGE_DOMAIN, &edge_commitment(component, edge));
            RecursiveEdgeObligation {
                caller: edge.caller,
                callee: edge.callee,
                decrease,
            }
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|edge| edge.decrease.obligation.id);
    RecursiveComponentObligation {
        members: component
            .members
            .iter()
            .map(|member| member.contract)
            .collect(),
        ranking_relation: Some(relation_identity(component)),
        well_foundedness,
        edges,
    }
}

fn committed_obligation(domain: &[u8], semantic_commitment: &[u8; 32]) -> CertificateObligation {
    let proposition = committed_proposition(domain, semantic_commitment);
    let mut digest = Sha256::new();
    digest.update(OBLIGATION_DOMAIN);
    digest.update(domain);
    digest.update(semantic_commitment);
    let digest: [u8; 32] = digest.finalize().into();
    CertificateObligation {
        obligation: Obligation {
            id: semantic_id_from_chunk::<ObligationId>(&digest, 0),
            proposition: proposition.clone(),
            class: ObligationClass::Derivable,
        },
        assumptions: Vec::new(),
        // The trusted verifier has already replayed the exact structural row.
        // The certificate must cite that row; it cannot manufacture or weaken
        // the proposition because every bit is committed below.
        semantic_axioms: vec![proposition],
    }
}

fn committed_proposition(domain: &[u8], semantic_commitment: &[u8; 32]) -> Proposition {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(semantic_commitment);
    let digest: [u8; 32] = digest.finalize().into();
    Proposition::Conjunction(
        (0..4)
            .map(|index| Proposition::Atom(semantic_id_from_chunk(&digest, index)))
            .collect(),
    )
}

fn relation_identity(component: &TerminalProofRecursiveComponent) -> RankingRelationId {
    let mut digest = Sha256::new();
    digest.update(RELATION_DOMAIN);
    digest.update([ranking_relation_tag(component.ranking_relation)]);
    update_string(&mut digest, &component.rank_type_identity);
    update_types(&mut digest, component);
    let digest: [u8; 32] = digest.finalize().into();
    semantic_id_from_chunk(&digest, 0)
}

/// Canonical semantic key used to join a proof-bundle certificate to the
/// exact verifier-reconstructed component it discharges.
pub fn proof_recursive_component_identity(
    component: &TerminalProofRecursiveComponent,
) -> RecursiveComponentId {
    semantic_id_from_chunk(&component_commitment(component), 0)
}

pub fn proof_recursive_well_foundedness_obligation_id(
    component: &TerminalProofRecursiveComponent,
) -> ObligationId {
    reconstruct_component(component)
        .well_foundedness
        .obligation
        .id
}

pub fn proof_recursive_edge_obligation_id(
    component: &TerminalProofRecursiveComponent,
    edge: &TerminalProofRecursiveEdge,
) -> ObligationId {
    committed_obligation(EDGE_DOMAIN, &edge_commitment(component, edge))
        .obligation
        .id
}

fn component_commitment(component: &TerminalProofRecursiveComponent) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(COMPONENT_DOMAIN);
    digest.update([ranking_relation_tag(component.ranking_relation)]);
    update_string(&mut digest, &component.rank_type_identity);
    update_types(&mut digest, component);
    update_len(&mut digest, component.members.len());
    for member in &component.members {
        digest.update(member.contract.get().to_le_bytes());
        update_string(&mut digest, &member.machine_identity);
        update_string(&mut digest, &member.rank_parameter_identity);
    }
    update_len(&mut digest, component.edges.len());
    for edge in &component.edges {
        update_edge(&mut digest, edge);
    }
    digest.finalize().into()
}

fn update_types(digest: &mut Sha256, component: &TerminalProofRecursiveComponent) {
    update_len(digest, component.types.len());
    for proof_type in &component.types {
        update_string(digest, &proof_type.identity);
        update_len(digest, proof_type.fields.len());
        for field in &proof_type.fields {
            update_string(digest, &field.identity);
            update_string(digest, &field.type_identity);
        }
    }
}

fn edge_commitment(
    component: &TerminalProofRecursiveComponent,
    edge: &TerminalProofRecursiveEdge,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(EDGE_DOMAIN);
    digest.update(component_commitment(component));
    update_edge(&mut digest, edge);
    digest.finalize().into()
}

fn update_edge(digest: &mut Sha256, edge: &TerminalProofRecursiveEdge) {
    digest.update(edge.caller.get().to_le_bytes());
    digest.update(edge.callee.get().to_le_bytes());
    match &edge.site {
        TerminalProofRecursiveCallSite::Statement {
            state_identity,
            statement_index,
        } => {
            digest.update([1]);
            update_string(digest, state_identity);
            digest.update(statement_index.to_le_bytes());
        }
        TerminalProofRecursiveCallSite::Expression {
            state_identity,
            statement_index,
            expression_ordinal,
        } => {
            digest.update([2]);
            update_string(digest, state_identity);
            digest.update(statement_index.to_le_bytes());
            digest.update(expression_ordinal.to_le_bytes());
        }
        TerminalProofRecursiveCallSite::Transition {
            state_identity,
            statement_index,
            lane,
        } => {
            digest.update([3]);
            update_string(digest, state_identity);
            digest.update(statement_index.to_le_bytes());
            digest.update([match lane {
                TerminalProofRecursiveTransitionLane::Target => 1,
                TerminalProofRecursiveTransitionLane::Continuation => 2,
            }]);
        }
    }
    update_len(digest, edge.strict_member_path.len());
    for member in &edge.strict_member_path {
        update_string(digest, member);
    }
}

const fn ranking_relation_tag(relation: TerminalProofRankingRelation) -> u8 {
    match relation {
        TerminalProofRankingRelation::StructuralSubterm => 1,
    }
}

fn update_len(digest: &mut Sha256, len: usize) {
    digest.update(
        u64::try_from(len)
            .expect("Terminal collection length fits u64")
            .to_le_bytes(),
    );
}

fn update_string(digest: &mut Sha256, value: &str) {
    update_len(digest, value.len());
    digest.update(value.as_bytes());
}

fn semantic_id_from_chunk<T: PsiSemanticId>(digest: &[u8; 32], chunk: usize) -> T {
    let start = chunk * 8;
    let raw = u64::from_le_bytes(
        digest[start..start + 8]
            .try_into()
            .expect("one SHA-256 u64 chunk"),
    ) | 1;
    T::new(raw).expect("forcing the low bit makes the semantic identity nonzero")
}

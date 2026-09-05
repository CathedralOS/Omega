use super::*;
use semantic_vocabulary::RankingRelationId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveEdgeCertificate {
    pub obligation: ObligationId,
    pub evidence: EvidenceRoute,
}

/// One certificate owns exactly one reconstructed strongly connected
/// component. The relation citation and well-foundedness evidence occur once;
/// edge-local evidence proves only the corresponding decrease proposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveComponentCertificate {
    pub identity: EvidenceIdentity,
    pub ranking_relation: RankingRelationId,
    pub well_foundedness: EvidenceRoute,
    pub edges: Vec<RecursiveEdgeCertificate>,
}

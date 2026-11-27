/*
We define the immutable rewrite candidate taxonomy (`PsiRewriteCandidate`,
`PsiRewritePatch`, `PsiCandidateDeclaration`), scalar and control-flow rewrite
plans, SCCP machine snapshots, and loop-invariant scalar motion descriptors.
Every candidate constructor derives a canonical `PsiRewriteDecisionPoint`,
validates common and patch-specific custody invariants, and seals its
`OptimizationCandidateIdentity` before returning an admitted candidate.
*/

use semantic_vocabulary::{BlockId, EdgeId, MachineId};

use crate::{PsiProvenance, WorkSettlement};

pub(crate) use foundations::encode_provenance_rows;
pub use foundations::{NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PsiRealizationSite};

mod foundations {
    use super::{BlockId, EdgeId, MachineId, PsiProvenance, WorkSettlement};
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct NodeLocation {
        pub machine: MachineId,
        pub block: BlockId,
        pub node: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum PsiRealizationSite {
        Node(NodeLocation),
        Edge { machine: MachineId, edge: EdgeId },
    }

    impl PsiRealizationSite {
        pub const fn machine(self) -> MachineId {
            match self {
                Self::Node(location) => location.machine,
                Self::Edge { machine, .. } => machine,
            }
        }

        pub const fn node(self) -> Option<NodeLocation> {
            match self {
                Self::Node(location) => Some(location),
                Self::Edge { .. } => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum ProvenanceDisposition {
        RealizedAt(PsiRealizationSite),
        ProvenUnreachableAt(PsiRealizationSite),
    }

    impl ProvenanceDisposition {
        pub const fn canonical_tag(self) -> u8 {
            match self {
                Self::RealizedAt(_) => 1,
                Self::ProvenUnreachableAt(_) => 2,
            }
        }

        pub const fn site(self) -> PsiRealizationSite {
            match self {
                Self::RealizedAt(site) | Self::ProvenUnreachableAt(site) => site,
            }
        }

        pub const fn is_realized(self) -> bool {
            matches!(self, Self::RealizedAt(_))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ProvenanceRewrite {
        pub input: PsiRealizationSite,
        pub disposition: ProvenanceDisposition,
        pub sources: Vec<PsiProvenance>,
        pub work: Vec<WorkSettlement>,
    }

    impl ProvenanceRewrite {
        pub fn canonical_sort_key(&self) -> (PsiRealizationSite, u8, PsiRealizationSite) {
            (
                self.input,
                self.disposition.canonical_tag(),
                self.disposition.site(),
            )
        }
    }

    pub fn encode_location(canonical: &mut Vec<u8>, location: NodeLocation) {
        crate::codec_bytes::push_encoded(canonical, location.machine.get().to_le_bytes());
        crate::codec_bytes::push_encoded(canonical, location.block.get().to_le_bytes());
        crate::codec_bytes::push_encoded(canonical, location.node.to_le_bytes());
    }

    pub(crate) fn encode_realization_site(bytes: &mut Vec<u8>, site: PsiRealizationSite) {
        match site {
            PsiRealizationSite::Node(location) => {
                bytes.push(1);
                encode_location(bytes, location);
            }
            PsiRealizationSite::Edge { machine, edge } => {
                bytes.push(2);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            }
        }
    }

    fn encode_provenance(bytes: &mut Vec<u8>, provenance: PsiProvenance) {
        let (tag, id) = match provenance {
            PsiProvenance::Operation(operation) => (1, operation.get()),
            PsiProvenance::Edge(edge) => (2, edge.get()),
        };
        bytes.push(tag);
        crate::codec_bytes::push_encoded(bytes, id.to_le_bytes());
    }

    /// Candidates and the transformation ledger encode provenance rows with
    /// this one layout, so a ledger record names exactly what its candidate did.
    pub(crate) fn encode_provenance_rows(bytes: &mut Vec<u8>, rows: &[ProvenanceRewrite]) {
        let len = |bytes: &mut Vec<u8>, len: usize| {
            bytes.extend_from_slice(
                &u64::try_from(len)
                    .expect("provenance row count fits u64")
                    .to_le_bytes(),
            );
        };
        len(bytes, rows.len());
        for row in rows {
            encode_realization_site(bytes, row.input);
            bytes.push(row.disposition.canonical_tag());
            encode_realization_site(bytes, row.disposition.site());
            len(bytes, row.sources.len());
            for source in &row.sources {
                encode_provenance(bytes, *source);
            }
            len(bytes, row.work.len());
            for settlement in &row.work {
                encode_provenance(bytes, settlement.site);
                crate::codec_bytes::push_encoded(bytes, settlement.units.to_le_bytes());
            }
        }
    }
}

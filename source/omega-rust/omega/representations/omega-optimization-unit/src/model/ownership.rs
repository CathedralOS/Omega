//! Path-sensitive ownership-frontier carriers and canonical identities.

use super::*;

/// Exact verifier-owned source site whose path-sensitive ownership state is
/// retained by the optimization unit. Entry and exit are deliberately distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwnershipFrontierSite {
    BlockEntry(BlockId),
    OperationEntry(OperationId),
    OperationExit(OperationId),
    EdgeEntry(EdgeId),
    /// Present for control-successor edges in the current verifier vocabulary.
    /// Terminal return/crash edges currently retain entry state only.
    EdgeExit(EdgeId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierLiveClaim {
    pub claim: ClaimId,
    pub input: Option<PlaceId>,
    pub path: Vec<StructuralPathSegment>,
    pub multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierOwnedPlace {
    pub place: PlaceId,
    pub multiplicity: StructuralMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierPartialCustody {
    pub place: PlaceId,
    pub moved_paths: Vec<Vec<StructuralPathSegment>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnershipFrontierSnapshot {
    pub claims: Vec<OwnershipFrontierLiveClaim>,
    pub owned_places: Vec<OwnershipFrontierOwnedPlace>,
    pub partial_custody: Vec<OwnershipFrontierPartialCustody>,
}

/// One immutable source ownership fact projected from the retained verifier
/// context. Rewrites preserve this catalog; analyses bind usable rows to the
/// current unit revision rather than manufacturing new ownership authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipFrontierFact {
    pub identity: OwnershipFrontierFactIdentity,
    pub psi: TerminalPsiIdentity,
    pub machine: MachineId,
    pub site: OwnershipFrontierSite,
    pub snapshot: OwnershipFrontierSnapshot,
}

impl OwnershipFrontierFact {
    pub fn new(
        psi: TerminalPsiIdentity,
        machine: MachineId,
        site: OwnershipFrontierSite,
        snapshot: OwnershipFrontierSnapshot,
    ) -> Self {
        let identity = ownership_frontier_fact_identity(psi, machine, site, &snapshot);
        Self {
            identity,
            psi,
            machine,
            site,
            snapshot,
        }
    }

    pub fn has_canonical_identity(&self) -> bool {
        self.identity
            == ownership_frontier_fact_identity(self.psi, self.machine, self.site, &self.snapshot)
    }
}

pub fn ownership_frontier_fact_identity(
    psi: TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &OwnershipFrontierSnapshot,
) -> OwnershipFrontierFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-ownership-frontier-fact.v1\0");
    canonical.extend_from_slice(psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    encode_frontier_site_identity(&mut canonical, site);
    encode_frontier_snapshot_identity(&mut canonical, snapshot);
    OwnershipFrontierFactIdentity::from_canonical_bytes(&canonical)
}

fn encode_frontier_site_identity(bytes: &mut Vec<u8>, site: OwnershipFrontierSite) {
    let (tag, identity) = match site {
        OwnershipFrontierSite::BlockEntry(id) => (1, id.get()),
        OwnershipFrontierSite::OperationEntry(id) => (2, id.get()),
        OwnershipFrontierSite::OperationExit(id) => (3, id.get()),
        OwnershipFrontierSite::EdgeEntry(id) => (4, id.get()),
        OwnershipFrontierSite::EdgeExit(id) => (5, id.get()),
    };
    bytes.push(tag);
    bytes.extend_from_slice(&identity.to_le_bytes());
}

fn encode_frontier_snapshot_identity(bytes: &mut Vec<u8>, snapshot: &OwnershipFrontierSnapshot) {
    encode_frontier_len(bytes, snapshot.claims.len());
    for claim in &snapshot.claims {
        bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
        encode_frontier_optional_id(bytes, claim.input.map(PlaceId::get));
        encode_frontier_path(bytes, &claim.path);
        encode_frontier_multiplicity(bytes, claim.multiplicity);
    }
    encode_frontier_len(bytes, snapshot.owned_places.len());
    for place in &snapshot.owned_places {
        bytes.extend_from_slice(&place.place.get().to_le_bytes());
        encode_frontier_multiplicity(bytes, Some(place.multiplicity));
    }
    encode_frontier_len(bytes, snapshot.partial_custody.len());
    for partial in &snapshot.partial_custody {
        bytes.extend_from_slice(&partial.place.get().to_le_bytes());
        encode_frontier_len(bytes, partial.moved_paths.len());
        for path in &partial.moved_paths {
            encode_frontier_path(bytes, path);
        }
    }
}

fn encode_frontier_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("canonical ownership-frontier length fits u64")
            .to_le_bytes(),
    );
}

fn encode_frontier_optional_id(bytes: &mut Vec<u8>, id: Option<u64>) {
    bytes.push(u8::from(id.is_some()));
    if let Some(id) = id {
        bytes.extend_from_slice(&id.to_le_bytes());
    }
}

fn encode_frontier_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_frontier_len(bytes, path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                bytes.push(1);
                encode_frontier_len(bytes, identity.len());
                bytes.extend_from_slice(identity.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.push(2);
                bytes.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
}

fn encode_frontier_multiplicity(bytes: &mut Vec<u8>, multiplicity: Option<StructuralMultiplicity>) {
    bytes.push(match multiplicity {
        None => 0,
        Some(StructuralMultiplicity::Unrestricted) => 1,
        Some(StructuralMultiplicity::Affine) => 2,
        Some(StructuralMultiplicity::Linear) => 3,
    });
}

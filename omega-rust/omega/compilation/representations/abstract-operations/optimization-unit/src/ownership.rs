/*
We represent path-sensitive ownership-frontier snapshots (`OwnershipFrontierFact`)
and their one-time verifier attachment (`attach_ownership_frontier_facts`). Each
site records live claims, owned places, and partially moved structural paths in
strictly increasing canonical order, and binds that snapshot into both its own
fact identity and the enclosing `OptimizationUnitIdentity`.
*/

use super::{
    BlockId, ClaimId, EdgeId, OperationId, OwnershipFrontierFactIdentity, PlaceId,
    StructuralMultiplicity, StructuralPathSegment,
};
use super::{MachineId, TerminalPsiIdentity};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwnershipFrontierSite {
    BlockEntry(BlockId),
    OperationEntry(OperationId),
    OperationExit(OperationId),
    EdgeEntry(EdgeId),
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

pub(crate) fn ownership_frontier_fact_identity(
    psi: TerminalPsiIdentity,
    machine: MachineId,
    site: OwnershipFrontierSite,
    snapshot: &OwnershipFrontierSnapshot,
) -> OwnershipFrontierFactIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-ownership-frontier-fact.v2\0");
    canonical.extend_from_slice(psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, psi.vocabulary_marker.get().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut canonical, machine.get().to_le_bytes());
    encode_ownership_frontier_site(&mut canonical, site);
    encode_frontier_snapshot_identity(&mut canonical, snapshot);
    OwnershipFrontierFactIdentity::from_canonical_bytes(&canonical)
}

pub(crate) fn encode_ownership_frontier_site(bytes: &mut Vec<u8>, site: OwnershipFrontierSite) {
    let (tag, identity) = match site {
        OwnershipFrontierSite::BlockEntry(id) => (1, id.get()),
        OwnershipFrontierSite::OperationEntry(id) => (2, id.get()),
        OwnershipFrontierSite::OperationExit(id) => (3, id.get()),
        OwnershipFrontierSite::EdgeEntry(id) => (4, id.get()),
        OwnershipFrontierSite::EdgeExit(id) => (5, id.get()),
    };
    bytes.push(tag);
    crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
}

fn encode_frontier_snapshot_identity(bytes: &mut Vec<u8>, snapshot: &OwnershipFrontierSnapshot) {
    encode_frontier_len(bytes, snapshot.claims.len());
    for claim in &snapshot.claims {
        crate::codec_bytes::push_encoded(bytes, claim.claim.get().to_le_bytes());
        encode_frontier_optional_id(bytes, claim.input.map(PlaceId::get));
        encode_frontier_path(bytes, &claim.path);
        encode_frontier_multiplicity(bytes, claim.multiplicity);
    }
    encode_frontier_len(bytes, snapshot.owned_places.len());
    for place in &snapshot.owned_places {
        crate::codec_bytes::push_encoded(bytes, place.place.get().to_le_bytes());
        encode_frontier_multiplicity(bytes, Some(place.multiplicity));
    }
    encode_frontier_len(bytes, snapshot.partial_custody.len());
    for partial in &snapshot.partial_custody {
        crate::codec_bytes::push_encoded(bytes, partial.place.get().to_le_bytes());
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
        crate::codec_bytes::push_encoded(bytes, id.to_le_bytes());
    }
}

fn encode_frontier_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    encode_frontier_len(bytes, path.len());
    for segment in path {
        terminal_psi::structural_path_encoding::encode_structural_path_segment(
            bytes,
            segment,
            Vec::extend_from_slice,
            |bytes, scalar| {
                super::encode_scalar_type(
                    bytes,
                    scalar,
                    |bytes, tag| bytes.push(tag),
                    super::range::encode_integer_type,
                )
            },
        );
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipFrontierFactIndexError {
    AlreadyAttached,
    TerminalIdentityMismatch,
    InvalidFactIdentity,
    NonCanonicalOrder,
    NonCanonicalSnapshot,
}

optimization_core::invalid_error_display!(
    OwnershipFrontierFactIndexError,
    "ownership frontier fact index"
);

pub fn attach_ownership_frontier_facts(
    mut unit: super::PsiOptimizationUnit,
    facts: Vec<OwnershipFrontierFact>,
) -> Result<super::PsiOptimizationUnit, OwnershipFrontierFactIndexError> {
    if !unit.ownership_frontier_facts.is_empty() {
        return Err(OwnershipFrontierFactIndexError::AlreadyAttached);
    }
    if facts.iter().any(|fact| fact.psi != unit.psi) {
        return Err(OwnershipFrontierFactIndexError::TerminalIdentityMismatch);
    }
    if facts.iter().any(|fact| !fact.has_canonical_identity()) {
        return Err(OwnershipFrontierFactIndexError::InvalidFactIdentity);
    }
    if facts
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].site) >= (pair[1].machine, pair[1].site))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalOrder);
    }
    if facts
        .iter()
        .any(|fact| !canonical_ownership_frontier_snapshot(&fact.snapshot))
    {
        return Err(OwnershipFrontierFactIndexError::NonCanonicalSnapshot);
    }
    unit.ownership_frontier_facts = facts;
    unit.identity = super::recompute_psi_optimization_unit_identity(&unit);
    Ok(unit)
}

pub fn canonical_ownership_frontier_snapshot(snapshot: &OwnershipFrontierSnapshot) -> bool {
    strictly_ordered_by(&snapshot.claims, |claim| claim.claim)
        && strictly_ordered_by(&snapshot.owned_places, |place| place.place)
        && strictly_ordered_by(&snapshot.partial_custody, |partial| partial.place)
        && snapshot
            .partial_custody
            .iter()
            .all(|partial| partial.moved_paths.windows(2).all(|pair| pair[0] < pair[1]))
}

fn strictly_ordered_by<T, K: Ord>(values: &[T], key: impl Fn(&T) -> K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

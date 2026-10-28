//! Structural backing supplied at a Terminal entry, independent of its executor.

use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, TerminalPlacedViewInput};

/// Opaque target-neutral runtime carrier for one structural argument.
///
/// `opaque_identity` is chosen by the embedding host and is only preserved for
/// argument forwarding and deterministic effect observation. Psi never treats
/// it as an address or layout. Qualification IDs are semantic runtime facts
/// supplied by the root installation and must be strictly increasing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct TerminalStructuralValue {
    pub opaque_identity: u64,
    pub structural_type: StructuralTypeId,
    pub qualifications: Vec<StructuralDomainId>,
    /// Exact occurrence-relative child memberships, retained independently
    /// of storage identity and selected again whenever the value is projected.
    pub projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    pub path: Vec<StructuralPathSegment>,
}

impl TerminalStructuralValue {
    /// A fresh root value minted for `structural_type`: new opaque identity,
    /// authored qualifications, no projected state, and no path.
    pub fn minted(
        opaque_identity: u64,
        structural_type: StructuralTypeId,
        qualifications: Vec<StructuralDomainId>,
    ) -> Self {
        Self {
            opaque_identity,
            structural_type,
            qualifications,
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        }
    }

    /// Adds every domain in `source` this value does not already carry,
    /// keeping the roster sorted — the union every binding site performs.
    pub fn absorb_qualifications(&mut self, source: &[StructuralDomainId]) {
        for domain in source {
            if !self.qualifications.contains(domain) {
                self.qualifications.push(*domain);
            }
        }
        self.qualifications.sort();
    }
}

/// One established placed-view input supplied beside the ordinary structural
/// arguments.
///
/// `input` must equal its entry roster row exactly — machine, position, every
/// declaration identity, access and binding flags, and both sealed placement
/// identities — so a stale or substituted row cannot answer a different
/// declared input. `referent` is the qualified backing carrier the provider
/// lends for the invocation's duration: the borrowed establishment's parent
/// claim. Qualification IDs are semantic runtime facts supplied by the root
/// installation, the same as on structural arguments; Psi treats the carrier
/// as an opaque occurrence root, never an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedViewEstablishment {
    pub input: TerminalPlacedViewInput,
    pub referent: TerminalStructuralValue,
}

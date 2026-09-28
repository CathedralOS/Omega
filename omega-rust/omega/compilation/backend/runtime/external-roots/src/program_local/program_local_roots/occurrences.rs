//! Installed program-local root occurrences, subjects and scalar bindings.

use crate::program_local::program_local_roots::{
    ProgramLocalEntryActivation, ProgramLocalRootInstalledPrebinding, ProgramLocalRootPrebindingId,
};
use crate::{ExternalRootDiagnostic, InstalledExternalRoot};
use effects::{ComponentEraLedgerId, ProgramLocalRootEpochLease, ProgramLocalRootEpochLeaseId};
use numerics::bignum::BigInt;
use std::collections::BTreeMap;

/// Exact lifecycle-qualified identity of one installed occurrence. A later
/// epoch is intentionally a distinct origin even when it reuses the same code
/// and slot prebinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstalledProgramLocalRootOccurrenceId {
    pub(crate) prebinding: ProgramLocalRootPrebindingId,
    pub(crate) lifecycle_ledger: ComponentEraLedgerId,
    pub(crate) lifecycle_epoch: u64,
}

impl InstalledProgramLocalRootOccurrenceId {
    pub const fn prebinding(self) -> ProgramLocalRootPrebindingId {
        self.prebinding
    }

    pub const fn lifecycle_ledger(self) -> ComponentEraLedgerId {
        self.lifecycle_ledger
    }

    pub const fn lifecycle_epoch(self) -> u64 {
        self.lifecycle_epoch
    }
}

/// Report identity for one concrete activation of an installed entry bridge.
///
/// This number is not authority. Generated-entry subjects stamp it from the
/// live [`ProgramLocalEntryActivation`] they were observed under, and the
/// installation ledger re-derives it from the presented activation at
/// establishment, so a caller-asserted value cannot stand in for a real
/// entered activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootEntryInvocationId(u64);

impl ProgramLocalRootEntryInvocationId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExternalRootDiagnostic> {
        if identity == 0 {
            return Err(ExternalRootDiagnostic(
                "normalized program-local entry invocation identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Report identity of the exact runtime place occupying one installed entry
/// parameter. It distinguishes activations and places but carries no authority
/// by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootSubjectPlaceId(u64);

impl ProgramLocalRootSubjectPlaceId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExternalRootDiagnostic> {
        if identity == 0 {
            return Err(ExternalRootDiagnostic(
                "normalized program-local subject-place identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Which compiler-checked scalar projection supplies one symbolic capacity
/// leaf. The distinction is semantic even when two leaves use the same field
/// path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgramLocalRootScalarSource {
    SubjectField,
    RuntimeScalarEmbedding,
}

/// One bridge-observed proof-natural scalar used to instantiate the verified
/// per-occurrence capacity expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootScalarBinding {
    source: ProgramLocalRootScalarSource,
    path: Vec<String>,
    value: BigInt,
}

impl ProgramLocalRootScalarBinding {
    pub fn subject_field(
        path: impl IntoIterator<Item = impl Into<String>>,
        value: BigInt,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Self::new(ProgramLocalRootScalarSource::SubjectField, path, value)
    }

    pub fn runtime_scalar_embedding(
        path: impl IntoIterator<Item = impl Into<String>>,
        value: BigInt,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Self::new(
            ProgramLocalRootScalarSource::RuntimeScalarEmbedding,
            path,
            value,
        )
    }

    fn new(
        source: ProgramLocalRootScalarSource,
        path: impl IntoIterator<Item = impl Into<String>>,
        value: BigInt,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let path = path.into_iter().map(Into::into).collect::<Vec<_>>();
        if path.is_empty() || path.iter().any(String::is_empty) {
            return Err(ExternalRootDiagnostic(
                "program-local capacity scalar path must contain only nonempty segments".into(),
            ));
        }
        if value.is_negative() {
            return Err(ExternalRootDiagnostic(
                "program-local capacity scalar observation must be a proof-natural".into(),
            ));
        }
        Ok(Self {
            source,
            path,
            value,
        })
    }

    pub const fn source(&self) -> ProgramLocalRootScalarSource {
        self.source
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }

    pub const fn value(&self) -> &BigInt {
        &self.value
    }
}

pub(crate) type ProgramLocalRootScalarKey = (ProgramLocalRootScalarSource, Vec<String>);

/// Single-use subject observation emitted by a generated installed-entry
/// bridge. It borrows the exact installed root, stamps the live activation's
/// invocation identity, and records the semantic and ABI parameter positions;
/// an ordinary call has no such installed-root binding.
#[derive(Debug)]
pub struct InstalledProgramLocalRootSubject<'root, 'code> {
    pub(crate) root: &'root InstalledExternalRoot<'code>,
    pub(crate) invocation: ProgramLocalRootEntryInvocationId,
    pub(crate) argument_index: u32,
    pub(crate) source_parameter_position: u32,
    pub(crate) qualification_identity: String,
    pub(crate) carrier_identity: String,
    pub(crate) subject_place: ProgramLocalRootSubjectPlaceId,
    pub(crate) scalars: BTreeMap<ProgramLocalRootScalarKey, BigInt>,
}

impl<'root, 'code> InstalledProgramLocalRootSubject<'root, 'code> {
    #[allow(clippy::too_many_arguments)]
    pub fn from_generated_entry(
        root: &'root InstalledExternalRoot<'code>,
        activation: &ProgramLocalEntryActivation,
        argument_index: u32,
        source_parameter_position: u32,
        qualification_identity: impl Into<String>,
        carrier_identity: impl Into<String>,
        subject_place: ProgramLocalRootSubjectPlaceId,
        scalars: impl IntoIterator<Item = ProgramLocalRootScalarBinding>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let qualification_identity = qualification_identity.into();
        let carrier_identity = carrier_identity.into();
        if qualification_identity.is_empty() || carrier_identity.is_empty() {
            return Err(ExternalRootDiagnostic(
                "program-local installed subject requires nonempty qualification and carrier identities"
                    .into(),
            ));
        }
        if root
            .evidence
            .root
            .boundary
            .plan()
            .call
            .parameters
            .get(argument_index as usize)
            .is_none()
            || !root
                .evidence
                .root
                .candidate
                .entry_claims
                .iter()
                .any(|claim| {
                    claim.parameter_index == argument_index as usize
                        && claim.domain == qualification_identity
                })
        {
            return Err(ExternalRootDiagnostic(
                "program-local installed subject does not name an exact qualified entry ABI parameter"
                    .into(),
            ));
        }
        let mut scalar_map = BTreeMap::new();
        for scalar in scalars {
            let key = (scalar.source, scalar.path);
            if scalar_map.insert(key, scalar.value).is_some() {
                return Err(ExternalRootDiagnostic(
                    "program-local installed subject repeats one capacity scalar observation"
                        .into(),
                ));
            }
        }
        Ok(Self {
            root,
            invocation: activation.invocation(),
            argument_index,
            source_parameter_position,
            qualification_identity,
            carrier_identity,
            subject_place,
            scalars: scalar_map,
        })
    }

    pub const fn invocation(&self) -> ProgramLocalRootEntryInvocationId {
        self.invocation
    }

    pub const fn argument_index(&self) -> u32 {
        self.argument_index
    }

    pub const fn source_parameter_position(&self) -> u32 {
        self.source_parameter_position
    }

    pub fn qualification_identity(&self) -> &str {
        &self.qualification_identity
    }

    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub const fn subject_place(&self) -> ProgramLocalRootSubjectPlaceId {
        self.subject_place
    }
}

/// Exact installed slot plus its non-duplicable lifecycle hold.
///
/// This is the complete per-occurrence join, but it is deliberately not yet a
/// lineage source: installation still needs a sealed finite eligible cohort.
/// Borrowing the installed root pins both that slot and its InstalledCode;
/// owning the epoch lease prevents lifecycle quiescence and retirement.
#[derive(Debug)]
pub struct InstalledProgramLocalRootOccurrence<'root, 'code> {
    pub(crate) identity: InstalledProgramLocalRootOccurrenceId,
    pub(crate) prebinding: ProgramLocalRootInstalledPrebinding,
    pub(crate) root: &'root InstalledExternalRoot<'code>,
    pub(crate) epoch_lease: ProgramLocalRootEpochLease,
}

impl InstalledProgramLocalRootOccurrence<'_, '_> {
    pub const fn identity(&self) -> InstalledProgramLocalRootOccurrenceId {
        self.identity
    }

    pub const fn prebinding(&self) -> &ProgramLocalRootInstalledPrebinding {
        &self.prebinding
    }

    pub const fn epoch_lease_identity(&self) -> ProgramLocalRootEpochLeaseId {
        self.epoch_lease.identity()
    }

    pub const fn installed_root(&self) -> &InstalledExternalRoot<'_> {
        self.root
    }
}

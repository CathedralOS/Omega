//! Public identity is a projection of checked declaration structure, not a
//! second source encoder. These values are transient normalization scratch;
//! the durable machine plan retains only the resulting commitment. Type and
//! expression leaves come from the typed semantic normalizers, never diagnostic
//! rendering. Body witnesses and scalar crash evaluation plans are excluded.
//!
//! Hash framing is Rust's derived structural framing under the pinned toolchain.
//! Integer writes are explicitly little-endian and pointer-width independent.
//! Changing the projection requires a new commitment domain, not an old-format
//! fallback: foreign packages are reviewed again at the trust boundary.

use super::{CrashInterface, MachineContractCommitment, MachineContractIdentity};
use language_semantics::{
    BlockingInterface, MachineSupplyMode, SuspensionInterface, SynchronousInvocationInterface,
};
use sha2::{Digest, Sha256};
use std::hash::{Hash, Hasher};
use typed_trees::type_identity::NormalizedTypeIdentity;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclaredContractFact {
    Expression(String),
    Membership {
        value: String,
        domain: String,
        arguments: Vec<NormalizedTypeIdentity>,
    },
    Proposition(String),
    ContentConservation(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclaredContractKind {
    Requires,
    Ensures,
    EnsuresForResultCase { data: String, case: String },
    Crashes(super::CrashCause),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredContractClause {
    pub kind: DeclaredContractKind,
    /// None is an unconditional crash route. Other clauses carry one fact.
    pub fact: Option<DeclaredContractFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredContractParameter {
    pub mutable: bool,
    pub constant: bool,
    pub relevance: language_core::BindingRelevance,
    pub type_identity: NormalizedTypeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredCallableContract {
    pub parameters: Vec<DeclaredContractParameter>,
    pub result: NormalizedTypeIdentity,
    pub clauses: Vec<DeclaredContractClause>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeclaredContractStructure {
    pub callables: Vec<DeclaredCallableContract>,
    pub clauses: Vec<DeclaredContractClause>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclaredProgressPremise {
    pub profile: String,
    pub subject: String,
    pub projections: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeclaredTermination {
    InternalDerived,
    NoGuarantee,
    Terminates(Vec<DeclaredProgressPremise>),
}

/// One commitment over normalized declarations, also used by universal
/// templates. There is no separately maintained template contract grammar.
pub fn contract_declarations_commitment(
    clauses: &[DeclaredContractClause],
) -> MachineContractCommitment {
    let mut identity = ContractHasher::new(b"omega.declared-contract-structure.v1\0");
    clauses.hash(&mut identity);
    identity.complete().commitment
}

pub fn declared_termination_commitment(
    termination: &DeclaredTermination,
) -> MachineContractCommitment {
    let mut identity = ContractHasher::new(b"omega.declared-termination.v1\0");
    termination.hash(&mut identity);
    identity.complete().commitment
}

pub fn contract_identity(
    supply_mode: MachineSupplyMode,
    published_service_names: &[String],
    invocation_interface: SynchronousInvocationInterface,
    published_invocations: &[String],
    suspension_interface: SuspensionInterface,
    blocking_interface: BlockingInterface,
    crash_interface: CrashInterface,
    termination: &DeclaredTermination,
    declaration: &DeclaredContractStructure,
) -> MachineContractIdentity {
    let mut identity = ContractHasher::new(b"omega.checked-machine-public-contract.v2\0");
    let supply = match supply_mode {
        MachineSupplyMode::CheckedBody => 1_u8,
        MachineSupplyMode::Requirement => 2,
        MachineSupplyMode::Boundary => 3,
        MachineSupplyMode::AdmissionClaim => 4,
        MachineSupplyMode::ExternalRealization { .. } => 5,
        MachineSupplyMode::TopLevelRequirement => 6,
    };
    supply.hash(&mut identity);
    if let MachineSupplyMode::ExternalRealization { binding, mechanism } = supply_mode {
        (
            binding.map(|binding| binding.0),
            mechanism.map(|mechanism| mechanism.identity_tag()),
        )
            .hash(&mut identity);
    }
    let mut services = published_service_names.iter().collect::<Vec<_>>();
    services.sort_unstable();
    services.dedup();
    services.hash(&mut identity);
    matches!(
        invocation_interface,
        SynchronousInvocationInterface::PublishedCeiling
    )
    .hash(&mut identity);
    let mut invocations = published_invocations.iter().collect::<Vec<_>>();
    invocations.sort_unstable();
    invocations.dedup();
    invocations.hash(&mut identity);
    match suspension_interface {
        SuspensionInterface::InternalInferred => None,
        SuspensionInterface::PublishedMaySuspend(value) => Some(value),
    }
    .hash(&mut identity);
    match blocking_interface {
        BlockingInterface::InternalInferred => None,
        BlockingInterface::PublishedMayBlock(value) => Some(value),
    }
    .hash(&mut identity);
    // Declared crash routes are already normalized in the declaration. The
    // runtime predicate carrier and its private scalar proof are not identity
    // inputs; passing only the interface makes that boundary explicit.
    matches!(crash_interface, CrashInterface::PublishedCeiling).hash(&mut identity);
    termination.hash(&mut identity);
    declaration.hash(&mut identity);
    identity.complete()
}

pub(super) struct ContractHasher {
    report: semantic_vocabulary::Fnv1a,
    commitment: Sha256,
}

impl ContractHasher {
    pub(super) fn new(domain: &[u8]) -> Self {
        let mut identity = Self {
            report: semantic_vocabulary::Fnv1a::new(),
            commitment: Sha256::new(),
        };
        identity.write(domain);
        identity
    }

    pub(super) fn complete(self) -> MachineContractIdentity {
        MachineContractIdentity {
            report_fingerprint: self.report.finish(),
            commitment: MachineContractCommitment::from_digest(self.commitment.finalize().into()),
        }
    }
}

macro_rules! integer_writes {
    ($($method:ident: $integer:ty),* $(,)?) => {
        $(fn $method(&mut self, value: $integer) { self.write(&value.to_le_bytes()); })*
    };
}

impl Hasher for ContractHasher {
    fn finish(&self) -> u64 {
        self.report.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.commitment.update(bytes);
        self.report.bytes(bytes);
    }

    integer_writes!(write_u8: u8, write_u16: u16, write_u32: u32, write_u64: u64,
        write_u128: u128, write_i8: i8, write_i16: i16, write_i32: i32,
        write_i64: i64, write_i128: i128);

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    fn write_isize(&mut self, value: isize) {
        self.write_i64(value as i64);
    }
}

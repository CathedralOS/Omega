//! Public identity is a projection of checked declaration structure, not a
//! second source encoder. These values are transient normalization scratch;
//! the durable machine plan retains only the resulting commitment. Type and
//! expression leaves come from the typed semantic normalizers, never diagnostic
//! rendering. Type handles retain their owning table through publication; only
//! borrowed canonical keys enter the portable hash projection. Local handle
//! ordinals never enter a portable commitment. Body witnesses and scalar crash evaluation plans are excluded.
//!
//! Hash framing is Rust's derived structural framing under the pinned toolchain.
//! Integer writes are explicitly little-endian and pointer-width independent.
//! Changing the projection requires a new commitment domain, not an old-format
//! fallback: foreign packages are reviewed again at the trust boundary.

use super::{CrashInterface, MachineContractCommitment, MachineContractIdentity};
use language_semantics::{
    BlockingInterface, MachineSupplyMode, SuspensionInterface, SynchronousInvocationInterface,
};
use std::hash::{Hash, Hasher};
use typed_trees::type_identity::{NormalizedTypeIdentity, TypeIdentityTable};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclaredContractClause {
    pub kind: DeclaredContractKind,
    /// None is an unconditional crash route. Other clauses carry one fact.
    pub fact: Option<DeclaredContractFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclaredContractParameter {
    pub mutable: bool,
    pub constant: bool,
    pub relevance: language_core::BindingRelevance,
    pub type_identity: NormalizedTypeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclaredCallableContract {
    pub parameters: Vec<DeclaredContractParameter>,
    pub result: NormalizedTypeIdentity,
    pub clauses: Vec<DeclaredContractClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    type_identities: &TypeIdentityTable,
) -> MachineContractCommitment {
    let mut identity = ContractHasher::new(b"omega.declared-contract-structure.v2\0");
    canonical_clauses(clauses, type_identities).hash(&mut identity);
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
    type_identities: &TypeIdentityTable,
) -> MachineContractIdentity {
    let mut identity = ContractHasher::new(b"omega.checked-machine-public-contract.v4\0");
    let supply = match supply_mode {
        MachineSupplyMode::CheckedBody => 1_u8,
        MachineSupplyMode::Requirement => 2,
        MachineSupplyMode::Boundary => 3,
        MachineSupplyMode::AdmissionClaim => 4,
        MachineSupplyMode::TopLevelRequirement => 6,
    };
    supply.hash(&mut identity);
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
    CanonicalDeclaration {
        callables: declaration
            .callables
            .iter()
            .map(|callable| CanonicalCallable {
                parameters: callable
                    .parameters
                    .iter()
                    .map(|parameter| CanonicalParameter {
                        mutable: parameter.mutable,
                        constant: parameter.constant,
                        relevance: parameter.relevance,
                        type_key: type_key(type_identities, parameter.type_identity),
                    })
                    .collect(),
                result: type_key(type_identities, callable.result),
                clauses: canonical_clauses(&callable.clauses, type_identities),
            })
            .collect(),
        clauses: canonical_clauses(&declaration.clauses, type_identities),
    }
    .hash(&mut identity);
    identity.complete()
}

// These private values exist only while borrowing the declaration's owning
// interner. Sorting happens after projection: local mint order cannot choose
// the order of two membership clauses in a portable contract.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CanonicalFact<'a> {
    Expression(&'a str),
    Membership {
        value: &'a str,
        domain: &'a str,
        arguments: Vec<&'a str>,
    },
    Proposition(&'a str),
    ContentConservation(&'a [u8]),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CanonicalClause<'a> {
    kind: &'a DeclaredContractKind,
    fact: Option<CanonicalFact<'a>>,
}

#[derive(Hash)]
struct CanonicalParameter<'a> {
    mutable: bool,
    constant: bool,
    relevance: language_core::BindingRelevance,
    type_key: &'a str,
}

#[derive(Hash)]
struct CanonicalCallable<'a> {
    parameters: Vec<CanonicalParameter<'a>>,
    result: &'a str,
    clauses: Vec<CanonicalClause<'a>>,
}

#[derive(Hash)]
struct CanonicalDeclaration<'a> {
    callables: Vec<CanonicalCallable<'a>>,
    clauses: Vec<CanonicalClause<'a>>,
}

fn type_key(table: &TypeIdentityTable, identity: NormalizedTypeIdentity) -> &str {
    table
        .text(identity)
        .expect("checked contract type must retain its owning identity")
}

fn canonical_clauses<'a>(
    clauses: &'a [DeclaredContractClause],
    table: &'a TypeIdentityTable,
) -> Vec<CanonicalClause<'a>> {
    let mut projected = clauses
        .iter()
        .map(|clause| CanonicalClause {
            kind: &clause.kind,
            fact: clause.fact.as_ref().map(|fact| canonical_fact(fact, table)),
        })
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

fn canonical_fact<'a>(
    fact: &'a DeclaredContractFact,
    table: &'a TypeIdentityTable,
) -> CanonicalFact<'a> {
    match fact {
        DeclaredContractFact::Expression(value) => CanonicalFact::Expression(value),
        DeclaredContractFact::Membership {
            value,
            domain,
            arguments,
        } => CanonicalFact::Membership {
            value,
            domain,
            arguments: arguments
                .iter()
                .map(|argument| type_key(table, *argument))
                .collect(),
        },
        DeclaredContractFact::Proposition(value) => CanonicalFact::Proposition(value),
        DeclaredContractFact::ContentConservation(value) => {
            CanonicalFact::ContentConservation(value)
        }
    }
}

pub(super) fn hash_declared_fact(
    fact: &DeclaredContractFact,
    table: &TypeIdentityTable,
    identity: &mut ContractHasher,
) {
    canonical_fact(fact, table).hash(identity);
}

pub(super) struct ContractHasher {
    report: semantic_vocabulary::Fnv1a,
    commitment: crate::digest_encoder::DomainDigestEncoder,
}

impl ContractHasher {
    pub(super) fn new(domain: &[u8]) -> Self {
        let mut identity = Self {
            report: semantic_vocabulary::Fnv1a::new(),
            commitment: crate::digest_encoder::DomainDigestEncoder::new(b""),
        };
        identity.write(domain);
        identity
    }

    pub(super) fn complete(self) -> MachineContractIdentity {
        MachineContractIdentity {
            report_fingerprint: self.report.finish(),
            commitment: MachineContractCommitment::from_digest(self.commitment.finish().into()),
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
        self.commitment.raw(bytes);
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

#[cfg(test)]
mod binding_projection_tests {
    use super::*;

    #[test]
    fn membership_clause_hash_uses_content_order_after_minting() {
        let clauses = |table: &mut TypeIdentityTable, keys: &[&str]| {
            keys.iter()
                .map(|key| DeclaredContractClause {
                    kind: DeclaredContractKind::Requires,
                    fact: Some(DeclaredContractFact::Membership {
                        value: "$V0".into(),
                        domain: "AcrossSuspend".into(),
                        arguments: vec![table.intern(key)],
                    }),
                })
                .collect::<Vec<_>>()
        };
        let mut first = TypeIdentityTable::default();
        let mut left = clauses(&mut first, &["Alpha", "Beta"]);
        left.sort();
        let mut reordered = TypeIdentityTable::default();
        let mut right = clauses(&mut reordered, &["Beta", "Alpha"]);
        right.sort();
        assert_ne!(
            first.text(
                left[0]
                    .fact
                    .as_ref()
                    .and_then(|fact| match fact {
                        DeclaredContractFact::Membership { arguments, .. } => Some(arguments[0]),
                        _ => None,
                    })
                    .unwrap()
            ),
            reordered.text(
                right[0]
                    .fact
                    .as_ref()
                    .and_then(|fact| match fact {
                        DeclaredContractFact::Membership { arguments, .. } => Some(arguments[0]),
                        _ => None,
                    })
                    .unwrap()
            )
        );
        assert_eq!(
            contract_declarations_commitment(&left, &first),
            contract_declarations_commitment(&right, &reordered)
        );
        right.pop();
        assert_ne!(
            contract_declarations_commitment(&left, &first),
            contract_declarations_commitment(&right, &reordered)
        );
    }

    #[test]
    fn declared_crash_fact_hash_uses_owned_type_content() {
        let mut first = TypeIdentityTable::default();
        let alpha = first.intern("Alpha");
        let mut reordered = TypeIdentityTable::default();
        reordered.intern("Beta");
        let same = reordered.intern("Alpha");
        let mut different = TypeIdentityTable::default();
        let beta = different.intern("Beta");
        assert_ne!(alpha, same);
        assert_eq!(alpha, beta);
        let fact = |argument| DeclaredContractFact::Membership {
            value: "$V0".into(),
            domain: "AcrossSuspend".into(),
            arguments: vec![argument],
        };
        assert_eq!(
            crate::CrashPredicateIdentity::from_declared_fact(&fact(alpha), &first),
            crate::CrashPredicateIdentity::from_declared_fact(&fact(same), &reordered)
        );
        assert_ne!(
            crate::CrashPredicateIdentity::from_declared_fact(&fact(alpha), &first),
            crate::CrashPredicateIdentity::from_declared_fact(&fact(beta), &different)
        );
    }
}

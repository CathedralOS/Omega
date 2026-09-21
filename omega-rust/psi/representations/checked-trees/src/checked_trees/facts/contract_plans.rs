//! STR4 checked plans (wiki/spec/language/machines.md): the normalized MACHINE
//! SEMANTIC CONTRACT, independent of syntax and lowering -- component
//! manifests, proof artifacts, provider admission, and hot-swap checks
//! reference this identity, never re-derived booleans. The checked public
//! axes publish independently; this carrier retains the remaining contract
//! plans plus their domain-separated commitment. Its compact FNV projection
//! is report/cache data only and still incorporates the published supply and
//! canonical service reach.
//! Prover-independence (acceptance 8: a stronger prover cannot change an
//! exported contract ID) holds BY CONSTRUCTION: only declared/published
//! halves enter either projection, never inferred rows or witnesses.
//!
//! This file owns the machine contract plans, commitments and identities.
//! `crash_predicates.rs` carries crash causes, predicates and route guards,
//! `crash_sites.rs` crash sites and contract capsules, `crash_plans.rs`
//! crash route buckets and plans, `resource_envelopes.rs` resource axes and
//! envelopes, `scalar_contracts.rs` closed scalar value contracts and
//! `tests.rs` the plan tests.

mod crash_plans;
mod crash_predicates;
mod crash_sites;
mod resource_envelopes;
mod scalar_contracts;
#[cfg(test)]
mod tests;

pub use crash_plans::{CrashPlan, CrashRouteBucket};
pub use crash_predicates::{
    CrashCause, CrashInterface, CrashPredicateExpression, CrashPredicateIdentity,
    CrashRouteBucketId, CrashRouteGuard,
};
pub use crash_sites::{
    CheckedCrashCallSite, CheckedCrashSite, CrashCallSiteLocation, CrashContractCapsule,
    CrashSiteLocation,
};
pub use resource_envelopes::{
    CheckedEntryResourceEnvelope, CheckedMachineResourceEnvelopes, CheckedResourceAxisAnchor,
    CheckedResourceDerivationObligation, RealizedMachineContractEnvelope,
};
pub use scalar_contracts::{
    ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement, ClosedScalarContractValue,
    ClosedScalarValueContractPlan,
};

use language_semantics::{
    BlockingInterface, MachineSupplyMode, SuspensionInterface, SynchronousInvocationInterface,
    TerminationGuarantee, TerminationInterface,
};
use sha2::{Digest, Sha256};
use std::hash::Hash;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineContractPlans {
    /// One entry per machine, in machine order.
    pub machines: Vec<MachineContractPlan>,
    /// Trait requirements and compile-time machine-parameter contracts do not
    /// own local machine plans. Their normalized callable identity and crash
    /// projection live here for modular call-site selection.
    pub crash_capsules: Vec<CrashContractCapsule>,
    /// Checked implementation axes assembled under the same exact machine
    /// identity as `machines`. Published requirement capsules remain separate;
    /// this row is the narrower realized endpoint used by callback admission.
    pub realized_envelopes: Vec<RealizedMachineContractEnvelope>,
}

impl MachineContractPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineContractPlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn crash_capsule(
        &self,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
    ) -> Option<&CrashContractCapsule> {
        self.crash_capsules.iter().find(|capsule| {
            capsule.target_machine == target_machine && capsule.target_state == target_state
        })
    }

    pub fn realized_envelope(
        &self,
        machine: SymbolHandle,
    ) -> Option<&RealizedMachineContractEnvelope> {
        self.realized_envelopes
            .iter()
            .find(|envelope| envelope.machine == machine)
    }

    pub fn resource_envelope(
        &self,
        machine: SymbolHandle,
        entry: SymbolHandle,
    ) -> Option<&CheckedEntryResourceEnvelope> {
        self.realized_envelope(machine)
            .and_then(|envelope| envelope.resources.get(entry))
    }

    /// Independently replay the checked resource anchor for every exact
    /// machine contract and compare it with the retained realized row.
    ///
    /// This deliberately rederives only the structural obligations available
    /// at checked-Psi time. It does not accept a stack number, a fuel number,
    /// a target footprint, or an installation receipt as an input.
    pub fn validate_resource_envelopes(&self) -> Result<(), &'static str> {
        for contract in &self.machines {
            if contract.report_fingerprint == 0 {
                return Err(
                    "checked resource envelope requires a nonzero contract report coordinate",
                );
            }
            if contract.commitment.is_zero() {
                return Err(
                    "checked resource envelope requires a nonzero machine contract commitment",
                );
            }
            let mut matching = self
                .realized_envelopes
                .iter()
                .filter(|envelope| envelope.machine == contract.machine);
            let Some(realized) = matching.next() else {
                return Err("checked resource envelope is missing its exact realized machine row");
            };
            if matching.next().is_some() {
                return Err("checked resource envelope has duplicate realized machine rows");
            }
            if realized.contract_report_fingerprint != contract.report_fingerprint
                || realized.contract_commitment != contract.commitment
            {
                return Err(
                    "checked resource envelope contract identity disagrees with its realized machine row",
                );
            }
            realized.resources.validate()?;
            if realized.resources.machine != contract.machine
                || realized.resources.contract_report_fingerprint != contract.report_fingerprint
                || realized.resources.contract_commitment != contract.commitment
            {
                return Err(
                    "checked resource roster does not bind its exact realized machine contract",
                );
            }
        }
        if self.realized_envelopes.iter().any(|realized| {
            !self
                .machines
                .iter()
                .any(|contract| contract.machine == realized.machine)
        }) {
            return Err("checked resource envelope retained an unknown realized machine row");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContractPlan {
    pub machine: SymbolHandle,
    /// Source-handle-free projection of authored value clauses into the
    /// closed reflexive scalar equality subset. An unrecognized clause is
    /// retained as `None`, so consumers fail closed without reopening typed
    /// proof expressions.
    pub closed_scalar_values: ClosedScalarValueContractPlan,
    /// Canonical published crash ceiling plus independent checked body sites.
    /// Clause grouping, ordering, duplicate predicates, and `true` spelling do
    /// not survive into the published carrier; sites do not enter identity.
    pub crash: CrashPlan,
    /// Historical compact compatibility coordinate. This is report/cache data
    /// and never sufficient contract authority without `commitment`.
    pub report_fingerprint: u64,
    /// Domain-separated SHA-256 commitment to the complete canonical public
    /// contract structure.
    pub commitment: MachineContractCommitment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachineContractCommitment([u8; 32]);

impl MachineContractCommitment {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 32]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineContractIdentity {
    pub report_fingerprint: u64,
    pub commitment: MachineContractCommitment,
}

/// Canonical public contract identity. The historical FNV-1a coordinate is
/// retained for reports and caches; authority uses the domain-separated
/// SHA-256 commitment over the same complete normalized byte stream.
pub fn contract_identity(
    supply_mode: MachineSupplyMode,
    published_service_names: &[String],
    invocation_interface: SynchronousInvocationInterface,
    published_invocations: &[String],
    suspension_interface: SuspensionInterface,
    blocking_interface: BlockingInterface,
    crash: &CrashPlan,
    termination: &TerminationInterface,
    canonical_facts: &[Vec<u8>],
) -> MachineContractIdentity {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    let mut strong = Sha256::new();
    strong.update(b"omega.checked-machine-public-contract.v1\0");
    let mut fold = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
        strong.update([byte]);
    };
    fold(match supply_mode {
        MachineSupplyMode::CheckedBody => 1,
        MachineSupplyMode::Requirement => 2,
        MachineSupplyMode::Boundary => 3,
        MachineSupplyMode::AdmissionClaim => 4,
        // PRV4: the leaf's supply tag; the binding identity folds separately
        // below so two leaves with different bindings differ.
        MachineSupplyMode::ExternalRealization { .. } => 5,
        MachineSupplyMode::TopLevelRequirement => 6,
    });
    if let MachineSupplyMode::ExternalRealization { binding, mechanism } = supply_mode {
        match mechanism {
            Some(mechanism) => {
                fold(1);
                fold(mechanism.identity_tag());
            }
            None => fold(0),
        }
        match binding {
            Some(binding) => {
                fold(1);
                for byte in binding.0.to_le_bytes() {
                    fold(byte);
                }
            }
            None => fold(0),
        }
    }
    // Boundary-service declaration identity, rendered canonically rather than
    // folding per-program row or service-table indices.
    fold(0xfb);
    let mut canonical_service_names = published_service_names.iter().collect::<Vec<_>>();
    canonical_service_names.sort_unstable();
    canonical_service_names.dedup();
    for name in canonical_service_names {
        for byte in name.as_bytes() {
            fold(*byte);
        }
        fold(0xfa);
    }
    fold(match invocation_interface {
        SynchronousInvocationInterface::InternalInferred => 1,
        SynchronousInvocationInterface::PublishedCeiling => 2,
    });
    let mut canonical_invocations = published_invocations.iter().collect::<Vec<_>>();
    canonical_invocations.sort_unstable();
    canonical_invocations.dedup();
    for invocation in canonical_invocations {
        for byte in invocation.as_bytes() {
            fold(*byte);
        }
        fold(0xf9);
    }
    fold(match suspension_interface {
        SuspensionInterface::InternalInferred => 1,
        SuspensionInterface::PublishedMaySuspend(false) => 2,
        SuspensionInterface::PublishedMaySuspend(true) => 3,
    });
    fold(match blocking_interface {
        BlockingInterface::InternalInferred => 1,
        BlockingInterface::PublishedMayBlock(false) => 2,
        BlockingInterface::PublishedMayBlock(true) => 3,
    });
    fold(0xf8);
    fold(match crash.interface {
        CrashInterface::InternalInferred => 1,
        CrashInterface::PublishedCeiling => 2,
    });
    let mut crash_buckets = crash.published.clone();
    crash_buckets.sort();
    crash_buckets.dedup();
    for bucket in crash_buckets {
        fold(match bucket.cause {
            CrashCause::Trap => 1,
            CrashCause::Abort => 2,
        });
        for guard in bucket.alternative_guards {
            match guard {
                CrashRouteGuard::Truth => fold(0),
                CrashRouteGuard::Predicate(predicate) => {
                    fold(1);
                    for byte in predicate.canonical_bytes() {
                        fold(*byte);
                    }
                }
            }
            fold(0xf7);
        }
        fold(0xf6);
    }
    fold(0xff);
    match termination {
        TerminationInterface::InternalDerived => fold(0),
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => fold(1),
        TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
            fold(2);
            for premise in premises {
                for byte in premise.profile.0.to_le_bytes() {
                    fold(byte);
                }
                for byte in premise.subject.root.arena_index().to_le_bytes() {
                    fold(byte);
                }
                fold(0xfb);
                for projection in &premise.subject.projections {
                    for byte in projection.arena_index().to_le_bytes() {
                        fold(byte);
                    }
                }
                fold(0xfa);
            }
        }
    }
    // Slice 2: the declared requires/ensures facts, pre-sorted by the
    // caller (clause order never enters the identity).
    fold(0xfd);
    for fact in canonical_facts {
        for byte in fact {
            fold(*byte);
        }
        fold(0xfc);
    }
    MachineContractIdentity {
        report_fingerprint: hash,
        commitment: MachineContractCommitment::from_digest(strong.finalize().into()),
    }
}

pub fn contract_report_fingerprint(
    supply_mode: MachineSupplyMode,
    published_service_names: &[String],
    invocation_interface: SynchronousInvocationInterface,
    published_invocations: &[String],
    suspension_interface: SuspensionInterface,
    blocking_interface: BlockingInterface,
    crash: &CrashPlan,
    termination: &TerminationInterface,
    canonical_facts: &[Vec<u8>],
) -> u64 {
    contract_identity(
        supply_mode,
        published_service_names,
        invocation_interface,
        published_invocations,
        suspension_interface,
        blocking_interface,
        crash,
        termination,
        canonical_facts,
    )
    .report_fingerprint
}

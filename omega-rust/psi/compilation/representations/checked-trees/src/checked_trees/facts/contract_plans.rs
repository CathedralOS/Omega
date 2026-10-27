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
//! This file owns the machine contract plans and commitment carriers.
//! `contract_identity.rs` projects normalized declarations into public identity.
//! `crash_predicates.rs` carries crash causes, predicates and route guards,
//! `crash_sites.rs` crash sites and contract capsules, `crash_plans.rs`
//! crash route buckets and plans, `resource_envelopes.rs` resource axes and
//! envelopes and `scalar_contracts.rs` closed scalar value contracts.

/// The four reads that carry one exact contract identity — the machine, its
/// state or entry, the report fingerprint and the commitment. The fingerprint
/// and its commitment always travel together, so the crash capsule, the crash
/// call site and both resource carriers declare these reads once, here.
macro_rules! contract_identity_accessors {
    ($machine:ident, $entry:ident, $fingerprint:ident, $commitment:ident) => {
        pub const fn $machine(&self) -> SymbolHandle {
            self.$machine
        }

        pub const fn $entry(&self) -> SymbolHandle {
            self.$entry
        }

        pub const fn $fingerprint(&self) -> u64 {
            self.$fingerprint
        }

        pub const fn $commitment(&self) -> MachineContractCommitment {
            self.$commitment
        }
    };
}
pub(crate) use contract_identity_accessors;

mod contract_identity;
mod crash_plans;
mod crash_predicates;
mod crash_sites;
mod resource_envelopes;
mod scalar_contracts;

pub use contract_identity::{
    DeclaredCallableContract, DeclaredContractClause, DeclaredContractFact, DeclaredContractKind,
    DeclaredContractParameter, DeclaredContractStructure, DeclaredProgressPremise,
    DeclaredTermination, contract_declarations_commitment, contract_identity,
    declared_termination_commitment,
};
pub use crash_plans::{CheckedRuntimeMembershipSubject, CrashPlan, CrashRouteBucket};
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
        contract_plan_by_machine(&self.machines, machine)
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
        let aligned = self.machines.len() == self.realized_envelopes.len()
            && self
                .machines
                .iter()
                .zip(&self.realized_envelopes)
                .all(|(contract, realized)| contract.machine == realized.machine)
            && {
                let mut seen = symbols::SymbolKeyMap::<SymbolHandle, ()>::default();
                self.machines
                    .iter()
                    .all(|contract| seen.insert(contract.machine, ()).is_none())
            };
        if aligned {
            for (contract, realized) in self.machines.iter().zip(&self.realized_envelopes) {
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
            return Ok(());
        }
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

/// The contract plan for `machine` in a plan roster, if present.
pub fn contract_plan_by_machine<'plans>(
    plans: &'plans [MachineContractPlan],
    machine: SymbolHandle,
) -> Option<&'plans MachineContractPlan> {
    plans.iter().find(|plan| plan.machine == machine)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContractPlan {
    pub machine: SymbolHandle,
    /// Projection of unbound requires/ensures clauses into the closed
    /// scalar vocabulary: reflexive `==` literal
    /// values (`Boolean`/`Integer`), `Predicate`s over the entry scalar
    /// parameters (an ensures clause may also name the result),
    /// requires-tail `FloatRange` windows, and ensures-only
    /// `SourceCheckedGuarantee` clauses without runtime scalar consequences.
    /// Those source facts remain available to checker consequence derivation
    /// and erase before Terminal publication. A derived
    /// parameter-range tail follows the authored requires prefix.
    /// An unsupported requires clause keeps its row as `None`; guarantees
    /// without scalar consequences keep their exact source facts. Runtime
    /// consumers never reopen those facts or treat them as requirements.
    /// Named-witness lanes produce no row at all — the evidence contract
    /// plan checks them — and crash or outcome-specific clauses only set
    /// the plan's `has_*` flags.
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

/// Domain-separated commitment to one machine's complete canonical public
/// contract, declared through `typed_trees`'s shared digest carrier.
typed_trees::digest_commitment! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    MachineContractCommitment
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineContractIdentity {
    pub report_fingerprint: u64,
    pub commitment: MachineContractCommitment,
}

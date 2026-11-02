//! Checked crash sites, call sites and crash contract capsules.

use crate::checked_trees::facts::contract_plans::{
    CrashCause, CrashPredicateIdentity, CrashRouteBucket, CrashRouteBucketId,
    MachineContractCommitment,
};
use language_semantics::TerminationGuarantee;
use symbols::SymbolHandle;

/// Source-handle-free location of one crash transition within a checked
/// machine body. State identity plus the statement's state-local ordinal is
/// stable against unrelated statement-arena insertions and is sufficient for
/// checked-tree consumers to join the derived site back to its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct CrashSiteLocation {
    pub(crate) state: SymbolHandle,
    pub(crate) statement_ordinal: u32,
}

/// Source-handle-free identity of one invocation within a checked machine
/// body. This deliberately reuses the flow layer's state/statement/call
/// coordinates so later crash propagation never has to rediscover a source
/// expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct CrashCallSiteLocation {
    pub(crate) state: SymbolHandle,
    pub(crate) statement_ordinal: u32,
    pub(crate) call_ordinal: u32,
}

/// Sorted-unique `Vec<CrashPredicateIdentity>` roster setter, the shape both
/// crash-plan guards share.
macro_rules! sorted_roster_setter {
    ($name:ident, $field:ident) => {
        pub fn $name(mut self, mut $field: Vec<CrashPredicateIdentity>) -> Self {
            $field = sorted_dedup($field);
            self.$field = $field;
            self
        }
    };
}

impl CrashCallSiteLocation {
    pub const fn new(state: SymbolHandle, statement_ordinal: u32, call_ordinal: u32) -> Self {
        Self {
            state,
            statement_ordinal,
            call_ordinal,
        }
    }

    pub const fn state(self) -> SymbolHandle {
        self.state
    }

    pub const fn statement_ordinal(self) -> u32 {
        self.statement_ordinal
    }

    pub const fn call_ordinal(self) -> u32 {
        self.call_ordinal
    }
}

impl CrashSiteLocation {
    pub const fn new(state: SymbolHandle, statement_ordinal: u32) -> Self {
        Self {
            state,
            statement_ordinal,
        }
    }

    pub const fn state(self) -> SymbolHandle {
        self.state
    }

    pub const fn statement_ordinal(self) -> u32 {
        self.statement_ordinal
    }
}

/// Body-derived seed for a crash-terminator plan. Structurally unconditional
/// guard coverage is attached immediately; path-conditioned entailment and
/// frontier reconstruction remain independent later passes.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCrashSite {
    pub(crate) location: CrashSiteLocation,
    pub(crate) cause: CrashCause,
    /// Exact canonical predicates known to hold on every path into this site.
    /// Their conjunction is the retained derived path guard; implication
    /// consequences remain separate coverage evidence.
    path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    /// Sound canonical consequences of the exact incoming conjunction. These
    /// witness guarded-route coverage without rewriting the exact guard.
    path_guard_consequences: Vec<CrashPredicateIdentity>,
    /// Published buckets whose guard implication is already established for
    /// this site.
    pub(crate) guard_covering_buckets: Vec<CrashRouteBucketId>,
    /// Stable identities of claims proved live at this exact machine-local
    /// crash site. This is deliberately a lower bound: a conditionally live
    /// sum payload joins only when incoming-edge case evidence proves every
    /// case segment on its claim path; payloads without that proof and
    /// obligations outside this activation remain absent until a later
    /// analysis can prove their membership.
    pub(crate) frontier_lower_bound: Vec<language_semantics::PermissionClaimIdentity>,
}

impl CheckedCrashSite {
    pub fn new(
        location: CrashSiteLocation,
        cause: CrashCause,
        mut guard_covering_buckets: Vec<CrashRouteBucketId>,
        mut frontier_lower_bound: Vec<language_semantics::PermissionClaimIdentity>,
    ) -> Self {
        guard_covering_buckets.sort_unstable();
        guard_covering_buckets.dedup();
        frontier_lower_bound.sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
        frontier_lower_bound.dedup();
        Self {
            location,
            cause,
            path_guard_conjuncts: Vec::new(),
            path_guard_consequences: Vec::new(),
            guard_covering_buckets,
            frontier_lower_bound,
        }
    }

    pub const fn location(&self) -> CrashSiteLocation {
        self.location
    }

    pub const fn cause(&self) -> CrashCause {
        self.cause
    }

    pub fn with_guard_covering_buckets(
        mut self,
        mut guard_covering_buckets: Vec<CrashRouteBucketId>,
    ) -> Self {
        guard_covering_buckets.sort_unstable();
        guard_covering_buckets.dedup();
        self.guard_covering_buckets = guard_covering_buckets;
        self
    }

    sorted_roster_setter!(with_path_guard_conjuncts, path_guard_conjuncts);

    sorted_roster_setter!(with_path_guard_consequences, path_guard_consequences);

    pub fn with_frontier_lower_bound(
        mut self,
        mut frontier_lower_bound: Vec<language_semantics::PermissionClaimIdentity>,
    ) -> Self {
        frontier_lower_bound.sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
        frontier_lower_bound.dedup();
        self.frontier_lower_bound = frontier_lower_bound;
        self
    }

    pub fn guard_covering_buckets(&self) -> &[CrashRouteBucketId] {
        &self.guard_covering_buckets
    }

    pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_consequences
    }

    pub fn frontier_lower_bound(&self) -> &[language_semantics::PermissionClaimIdentity] {
        &self.frontier_lower_bound
    }
}

/// Invocation-specific refinement of a selected callee crash summary. The
/// summary may be a published ceiling or conservative same-unit checked-body
/// evidence. `surviving_buckets` are already expressed in the caller's
/// canonical scalar value namespace, including direct caller-local arguments.
/// `target_machine` plus `target_contract_commitment` pins the exact
/// parameter-relative route origin when terminal control must bind a staged
/// argument value directly rather than reverse-matching caller expressions.
/// The compact fingerprint remains a compatibility/report coordinate only.
/// Exact incoming conjuncts remain distinct
/// from the sound structural consequences used by ceiling coverage. An empty
/// surviving set is meaningful evidence that the selected summary is
/// crash-free at this invocation, so such records are retained rather than
/// elided.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCrashCallSite {
    pub(crate) location: CrashCallSiteLocation,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    target_contract_report_fingerprint: u64,
    target_contract_commitment: MachineContractCommitment,
    path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    path_guard_consequences: Vec<CrashPredicateIdentity>,
    surviving_buckets: Vec<CrashRouteBucket>,
}

/// Source-independent published envelope for a callable requirement that has
/// no local `MachineContractPlan`. The commitment pins the complete normalized
/// callable contract; the fingerprint is a compatibility/report coordinate.
/// Independent operational axes and the crash ceiling stay directly queryable
/// without reopening the authored signature.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CrashContractCapsule {
    pub(crate) target_machine: SymbolHandle,
    pub(crate) target_state: SymbolHandle,
    target_contract_report_fingerprint: u64,
    target_contract_commitment: MachineContractCommitment,
    published_service_reach: Vec<String>,
    published_synchronous_invocations: Vec<String>,
    published_may_suspend: bool,
    published_may_block: bool,
    published_termination: TerminationGuarantee,
    published_buckets: Vec<CrashRouteBucket>,
}

impl CrashContractCapsule {
    pub fn new(
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        published_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        Self::new_with_commitment(
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            MachineContractCommitment::from_digest([0; 32]),
            published_buckets,
        )
    }

    pub fn new_with_commitment(
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        target_contract_commitment: MachineContractCommitment,
        mut published_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        published_buckets = sorted_dedup(published_buckets);
        Self {
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            target_contract_commitment,
            published_service_reach: Vec::new(),
            published_synchronous_invocations: Vec::new(),
            published_may_suspend: false,
            published_may_block: false,
            published_termination: TerminationGuarantee::NoGuarantee,
            published_buckets,
        }
    }

    pub fn with_operational_envelope(
        mut self,
        mut published_service_reach: Vec<String>,
        mut published_synchronous_invocations: Vec<String>,
        published_may_suspend: bool,
        published_may_block: bool,
        published_termination: TerminationGuarantee,
    ) -> Self {
        published_service_reach = sorted_dedup(published_service_reach);
        published_synchronous_invocations = sorted_dedup(published_synchronous_invocations);
        self.published_service_reach = published_service_reach;
        self.published_synchronous_invocations = published_synchronous_invocations;
        self.published_may_suspend = published_may_suspend;
        self.published_may_block = published_may_block;
        self.published_termination = published_termination;
        self
    }

    super::contract_identity_accessors!(
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        target_contract_commitment
    );

    pub fn published_buckets(&self) -> &[CrashRouteBucket] {
        &self.published_buckets
    }

    pub fn published_service_reach(&self) -> &[String] {
        &self.published_service_reach
    }

    pub fn published_synchronous_invocations(&self) -> &[String] {
        &self.published_synchronous_invocations
    }

    pub const fn published_may_suspend(&self) -> bool {
        self.published_may_suspend
    }

    pub const fn published_may_block(&self) -> bool {
        self.published_may_block
    }

    pub const fn published_termination(&self) -> &TerminationGuarantee {
        &self.published_termination
    }
}

impl CheckedCrashCallSite {
    /// Bind the checked source condition to the function execution will enter.
    /// A selected requirement and its implementation have different identities;
    /// the retained call record connects them without changing either contract.
    pub fn executable_contract_identity(
        &self,
        plans: &crate::MachineContractPlans,
        proof: &crate::ProofFacts,
        caller_machine: SymbolHandle,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
    ) -> Result<crate::MachineContractIdentity, &'static str> {
        let target = plans
            .for_machine(target_machine)
            .ok_or("executable call target has no checked contract plan")?;
        let identity = crate::MachineContractIdentity {
            report_fingerprint: target.report_fingerprint,
            commitment: target.commitment,
        };
        if self.target_machine == target_machine && self.target_state == target_state {
            if self.target_contract_report_fingerprint != identity.report_fingerprint
                || self.target_contract_commitment != identity.commitment
            {
                return Err("checked call target contract identity disagrees");
            }
            return Ok(identity);
        }

        let binding = proof
            .unique_contract_call_at(&(
                caller_machine,
                self.location.state,
                self.location.statement_ordinal as usize,
                self.location.call_ordinal as usize,
            ))
            .ok_or("requirement call has no unique checked executable binding")?;
        if binding.target_machine_symbol != target_machine
            || binding.target_state_symbol != target_state
            || !proof.call_contract_signatures.is_valid(binding.signature)
        {
            return Err("requirement call has a mismatched checked executable binding");
        }
        let public = proof.call_contract_signatures.get(binding.signature);
        if public.caller_machine_symbol != caller_machine
            || public.caller_state_symbol != self.location.state
            || public.statement_index != self.location.statement_ordinal as usize
            || public.call_ordinal != self.location.call_ordinal as usize
            || self.target_machine != public.declaring_trait
            || self.target_state != public.signature.symbol
        {
            return Err("requirement crash check belongs to another public call");
        }
        Ok(identity)
    }

    pub fn new(
        location: CrashCallSiteLocation,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        surviving_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        Self::new_with_commitment(
            location,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            MachineContractCommitment::from_digest([0; 32]),
            surviving_buckets,
        )
    }

    pub fn new_with_commitment(
        location: CrashCallSiteLocation,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_report_fingerprint: u64,
        target_contract_commitment: MachineContractCommitment,
        mut surviving_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        surviving_buckets = sorted_dedup(surviving_buckets);
        Self {
            location,
            target_machine,
            target_state,
            target_contract_report_fingerprint,
            target_contract_commitment,
            path_guard_conjuncts: Vec::new(),
            path_guard_consequences: Vec::new(),
            surviving_buckets,
        }
    }

    pub const fn location(&self) -> CrashCallSiteLocation {
        self.location
    }

    super::contract_identity_accessors!(
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        target_contract_commitment
    );

    pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_consequences
    }

    pub fn surviving_buckets(&self) -> &[CrashRouteBucket] {
        &self.surviving_buckets
    }

    sorted_roster_setter!(with_path_guard_conjuncts, path_guard_conjuncts);

    sorted_roster_setter!(with_path_guard_consequences, path_guard_consequences);
}

/// Sorted unique rows, the shape every roster field on the crash plans
/// keeps.
fn sorted_dedup<Row: Ord>(mut rows: Vec<Row>) -> Vec<Row> {
    rows.sort();
    rows.dedup();
    rows
}

fn crash_frontier_claim_sort_key(
    identity: language_semantics::PermissionClaimIdentity,
) -> [u64; 11] {
    use language_semantics::{PermissionClaimIdentity, PermissionEventSource};

    let PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = identity
    else {
        return [0; 11];
    };
    let mut key = [0; 11];
    key[0] = 1;
    key[1] = u64::from(machine_symbol.arena_index());
    key[2] = u64::from(machine_symbol.generation());
    key[3] = u64::from(state_symbol.arena_index());
    key[4] = u64::from(state_symbol.generation());
    match source {
        PermissionEventSource::StateEntry => key[5] = 0,
        PermissionEventSource::Statement { statement_index } => {
            key[5] = 1;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
        }
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            key[5] = 2;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
            key[7] = u64::try_from(call_ordinal).unwrap_or(u64::MAX);
            key[8] = u64::from(target_symbol.arena_index());
            key[9] = u64::from(target_symbol.generation());
        }
        PermissionEventSource::StateExit => key[5] = 3,
        PermissionEventSource::InstructionExitDisposition { statement_index } => {
            key[5] = 5;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
        }
        PermissionEventSource::InstructionDisposition { statement_index } => {
            key[5] = 4;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
        }
    }
    key[10] = u64::from(ordinal);
    key
}

#[cfg(test)]
mod executable_binding_tests {
    use super::*;
    use crate::{
        CallContractSignature, ContractCallFact, MachineContractPlan, MachineContractPlans,
        ProofFacts,
    };

    struct Binding {
        plans: MachineContractPlans,
        proof: ProofFacts,
        checked: CheckedCrashCallSite,
        caller: SymbolHandle,
        target: SymbolHandle,
        entry: SymbolHandle,
    }

    impl Binding {
        fn new() -> Self {
            let symbol = |index| SymbolHandle::from_parts(index, 1);
            let caller = symbol(1);
            let caller_state = symbol(2);
            let target = symbol(3);
            let entry = symbol(4);
            let public_trait = symbol(5);
            let public_member = symbol(6);
            let plans = MachineContractPlans {
                machines: vec![MachineContractPlan {
                    machine: target,
                    closed_scalar_values: Default::default(),
                    crash: Default::default(),
                    report_fingerprint: 37,
                    commitment: MachineContractCommitment::from_digest([7; 32]),
                }],
                ..Default::default()
            };
            let mut proof = ProofFacts::default();
            let signature = proof
                .call_contract_signatures
                .append(CallContractSignature {
                    caller_machine_symbol: caller,
                    caller_state_symbol: caller_state,
                    statement_index: 2,
                    call_ordinal: 1,
                    declaring_trait: public_trait,
                    signature: typed_trees::signature::StateSignature {
                        symbol: public_member,
                        ..Default::default()
                    },
                    ..Default::default()
                });
            proof.contract_calls.append(ContractCallFact {
                caller_machine_symbol: caller,
                caller_state_symbol: caller_state,
                statement_index: 2,
                call_ordinal: 1,
                target_machine_symbol: target,
                target_state_symbol: entry,
                signature,
                ..Default::default()
            });
            let checked = CheckedCrashCallSite::new_with_commitment(
                CrashCallSiteLocation::new(caller_state, 2, 1),
                public_trait,
                public_member,
                41,
                MachineContractCommitment::from_digest([8; 32]),
                Vec::new(),
            );
            Self {
                plans,
                proof,
                checked,
                caller,
                target,
                entry,
            }
        }

        fn identity(&self) -> Result<crate::MachineContractIdentity, &'static str> {
            self.checked.executable_contract_identity(
                &self.plans,
                &self.proof,
                self.caller,
                self.target,
                self.entry,
            )
        }
    }

    #[test]
    fn public_call_binds_its_distinct_executable_identity() {
        let binding = Binding::new();
        let identity = binding
            .identity()
            .expect("the exact public/executable link is valid");
        assert_eq!(identity.report_fingerprint, 37);
        assert_eq!(
            identity.commitment,
            MachineContractCommitment::from_digest([7; 32])
        );
    }

    #[test]
    fn missing_duplicate_or_other_executable_binding_rejects() {
        let mut missing = Binding::new();
        missing.proof.contract_calls = Default::default();
        assert!(missing.identity().is_err());

        let mut duplicate = Binding::new();
        let row = duplicate
            .proof
            .contract_calls
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        duplicate.proof.contract_calls.append(row);
        assert!(duplicate.identity().is_err());

        let mut other = Binding::new();
        let target = SymbolHandle::from_parts(7, 1);
        let mut plan = other.plans.machines[0].clone();
        plan.machine = target;
        other.plans.machines.push(plan);
        other.target = target;
        assert!(other.identity().is_err());

        let mut other_entry = Binding::new();
        other_entry.entry = SymbolHandle::from_parts(8, 1);
        assert!(other_entry.identity().is_err());
    }

    #[test]
    fn missing_or_another_public_signature_rejects() {
        let mut missing = Binding::new();
        missing.proof.call_contract_signatures = Default::default();
        assert!(missing.identity().is_err());

        let mut other = Binding::new();
        let signature = other
            .proof
            .contract_calls
            .iter()
            .next()
            .unwrap()
            .1
            .signature;
        other
            .proof
            .call_contract_signatures
            .get_mut(signature)
            .call_ordinal = 3;
        assert!(other.identity().is_err());

        let mut other_member = Binding::new();
        let signature = other_member
            .proof
            .contract_calls
            .iter()
            .next()
            .unwrap()
            .1
            .signature;
        other_member
            .proof
            .call_contract_signatures
            .get_mut(signature)
            .signature
            .symbol = SymbolHandle::from_parts(8, 1);
        assert!(other_member.identity().is_err());
    }

    #[test]
    fn ordinary_call_still_requires_its_exact_commitment() {
        let mut binding = Binding::new();
        binding.checked = CheckedCrashCallSite::new_with_commitment(
            binding.checked.location(),
            binding.target,
            binding.entry,
            37,
            MachineContractCommitment::from_digest([9; 32]),
            Vec::new(),
        );
        assert!(binding.identity().is_err());

        binding.checked = CheckedCrashCallSite::new_with_commitment(
            binding.checked.location(),
            binding.target,
            binding.entry,
            42,
            MachineContractCommitment::from_digest([7; 32]),
            Vec::new(),
        );
        assert!(binding.identity().is_err());
    }
}

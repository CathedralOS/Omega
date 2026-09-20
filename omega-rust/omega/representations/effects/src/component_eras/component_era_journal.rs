//! Ordered, restart-replayable journal facts for one component-era ledger.
//!
//! Every transition the [`super::component_era_entry_ledger`] accepts has a
//! fact shape carrying that transition receipt's exact fields. Recording is
//! append-only plain data: a journal can be persisted and handed to a restart
//! without carrying any lifecycle authority. The checked leg is [`replay`]:
//! it folds the fact sequence through the ledger's own accept/reject rules
//! and rebuilds the live-era roster, the current era, and the program-local
//! epoch-lease holds. A complete journal replays to the same roster the
//! source ledger reports; a dropped, reordered, substituted, or
//! replayed-identity fact rejects with a positioned diagnostic instead of
//! silently producing a different roster.
//!
//! [`replay`]: ComponentEraJournal::replay

use std::collections::{BTreeMap, BTreeSet};

use super::component_era_entry_ledger::{
    ComponentEraCandidate, ComponentEraEntryReceipt, ComponentEraEntryState,
    ComponentEraLeaveReceipt, ComponentEraPublicationReceipt, ComponentEraQuiescenceReceipt,
    ComponentEraRetirementReceipt, ProgramLocalRootEpochLease, ProgramLocalRootEpochLeaseId,
};

/// One journal-recorded ledger transition. Variants carry the accepted
/// receipt's fields verbatim; nothing about a fact is trusted until replay
/// accepts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentEraJournalFact {
    Published {
        publication_identity: u64,
        binding_contract_identity: String,
        entry_contract_identity: String,
        previous_era_identity: Option<u64>,
        candidate: ComponentEraCandidate,
        new_era_visible: bool,
        previous_era_closed: bool,
    },
    Entered {
        invocation_identity: u64,
        binding_contract_identity: String,
        entry_contract_identity: String,
        resolved_era_identity: u64,
        entry_plan_identity: String,
        entry_linearized: bool,
    },
    Left {
        invocation_identity: u64,
        binding_contract_identity: String,
        era_identity: u64,
        entry_plan_identity: String,
        leave_completed: bool,
    },
    EpochLeaseAcquired {
        lease_identity: ProgramLocalRootEpochLeaseId,
        era_identity: u64,
        entry_contract_identity: String,
    },
    EpochLeaseReleased {
        lease_identity: ProgramLocalRootEpochLeaseId,
        era_identity: u64,
    },
    Quiesced {
        era_identity: u64,
        binding_contract_identity: String,
        residual_lifetime_cohort_holds: usize,
        all_dispositions_complete: bool,
    },
    Retired {
        retirement_identity: u64,
        era_identity: u64,
        binding_contract_identity: String,
        lifetime_cohort_released: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplayedEraRecord {
    candidate: ComponentEraCandidate,
    state: ComponentEraEntryState,
    active_entries: usize,
    epoch_leases: BTreeSet<ProgramLocalRootEpochLeaseId>,
}

/// The roster a journal's facts reconstruct, in the same shape the ledger
/// reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraJournalRoster {
    current_era: Option<u64>,
    eras: BTreeMap<u64, ReplayedEraRecord>,
}

impl ComponentEraJournalRoster {
    /// Live eras in ascending era-identity order, each with its entry state
    /// and active entry count — the journal twin of
    /// `ComponentEraEntryLedger::live_eras`.
    pub fn live_eras(&self) -> impl Iterator<Item = (u64, ComponentEraEntryState, usize)> + '_ {
        self.eras
            .iter()
            .map(|(identity, record)| (*identity, record.state, record.active_entries))
    }

    pub const fn current_era(&self) -> Option<u64> {
        self.current_era
    }

    /// Live program-local epoch-lease hold count for one era — the journal
    /// twin of `ComponentEraEntryLedger::program_local_root_authority_holds`.
    pub fn epoch_lease_holds(&self, era_identity: u64) -> Option<usize> {
        self.eras
            .get(&era_identity)
            .map(|record| record.epoch_leases.len())
    }

    /// The exact candidate a replayed era published under.
    pub fn era_candidate(&self, era_identity: u64) -> Option<&ComponentEraCandidate> {
        self.eras.get(&era_identity).map(|record| &record.candidate)
    }
}

/// A replayed journal binds one ledger's contract identities: facts recorded
/// under a different contract reject at replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraJournal {
    binding_contract_identity: String,
    entry_contract_identity: String,
    facts: Vec<ComponentEraJournalFact>,
}

impl ComponentEraJournal {
    pub fn new(
        binding_contract_identity: String,
        entry_contract_identity: String,
    ) -> Result<Self, String> {
        if binding_contract_identity.trim().is_empty() {
            return Err("component era journal has no binding-contract identity".into());
        }
        if entry_contract_identity.trim().is_empty() {
            return Err("component era journal has no entry-contract identity".into());
        }
        Ok(Self {
            binding_contract_identity,
            entry_contract_identity,
            facts: Vec::new(),
        })
    }

    /// Rebuild a journal from persisted facts. No trust is attached to the
    /// input: `replay` re-checks every fact before any roster is consumed.
    pub fn from_facts(
        binding_contract_identity: String,
        entry_contract_identity: String,
        facts: Vec<ComponentEraJournalFact>,
    ) -> Result<Self, String> {
        let mut journal = Self::new(binding_contract_identity, entry_contract_identity)?;
        journal.facts = facts;
        Ok(journal)
    }

    pub fn binding_contract_identity(&self) -> &str {
        &self.binding_contract_identity
    }

    pub fn entry_contract_identity(&self) -> &str {
        &self.entry_contract_identity
    }

    pub fn facts(&self) -> &[ComponentEraJournalFact] {
        &self.facts
    }

    pub const fn len(&self) -> usize {
        self.facts.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    pub fn record_publication(&mut self, receipt: &ComponentEraPublicationReceipt) {
        self.facts.push(ComponentEraJournalFact::Published {
            publication_identity: receipt.publication_identity(),
            binding_contract_identity: self.binding_contract_identity.clone(),
            entry_contract_identity: self.entry_contract_identity.clone(),
            previous_era_identity: receipt.previous_era_identity(),
            candidate: receipt.candidate().clone(),
            new_era_visible: receipt.new_era_visible(),
            previous_era_closed: receipt.previous_era_closed(),
        });
    }

    pub fn record_entry(&mut self, receipt: &ComponentEraEntryReceipt) {
        self.facts.push(ComponentEraJournalFact::Entered {
            invocation_identity: receipt.invocation_identity(),
            binding_contract_identity: self.binding_contract_identity.clone(),
            entry_contract_identity: self.entry_contract_identity.clone(),
            resolved_era_identity: receipt.resolved_era_identity(),
            entry_plan_identity: receipt.entry_plan_identity().to_owned(),
            entry_linearized: receipt.entry_linearized(),
        });
    }

    pub fn record_leave(&mut self, receipt: &ComponentEraLeaveReceipt) {
        self.facts.push(ComponentEraJournalFact::Left {
            invocation_identity: receipt.invocation_identity(),
            binding_contract_identity: self.binding_contract_identity.clone(),
            era_identity: receipt.era_identity(),
            entry_plan_identity: receipt.entry_plan_identity().to_owned(),
            leave_completed: receipt.leave_completed(),
        });
    }

    /// Record an accepted epoch-lease acquisition. The lease object is the
    /// authority; the fact carries only its identity coordinates.
    pub fn record_epoch_lease_acquisition(&mut self, lease: &ProgramLocalRootEpochLease) {
        self.facts
            .push(ComponentEraJournalFact::EpochLeaseAcquired {
                lease_identity: lease.identity(),
                era_identity: lease.era_identity(),
                entry_contract_identity: lease.entry_contract_identity().to_owned(),
            });
    }

    /// Record a release before the ledger consumes the lease; the fact keeps
    /// the identity and era, not the consumed hold.
    pub fn record_epoch_lease_release(&mut self, lease: &ProgramLocalRootEpochLease) {
        self.facts
            .push(ComponentEraJournalFact::EpochLeaseReleased {
                lease_identity: lease.identity(),
                era_identity: lease.era_identity(),
            });
    }

    pub fn record_quiescence(&mut self, receipt: &ComponentEraQuiescenceReceipt) {
        self.facts.push(ComponentEraJournalFact::Quiesced {
            era_identity: receipt.era_identity(),
            binding_contract_identity: self.binding_contract_identity.clone(),
            residual_lifetime_cohort_holds: receipt.residual_lifetime_cohort_holds(),
            all_dispositions_complete: receipt.all_dispositions_complete(),
        });
    }

    pub fn record_retirement(&mut self, receipt: &ComponentEraRetirementReceipt) {
        self.facts.push(ComponentEraJournalFact::Retired {
            retirement_identity: receipt.retirement_identity(),
            era_identity: receipt.era_identity(),
            binding_contract_identity: self.binding_contract_identity.clone(),
            lifetime_cohort_released: receipt.lifetime_cohort_released(),
        });
    }

    /// Fold the fact sequence through the ledger's accept rules and rebuild
    /// the roster they describe. Rejects at the first fact that could not
    /// have been produced by the ledger the journal is bound to.
    pub fn replay(&self) -> Result<ComponentEraJournalRoster, ComponentEraJournalReplayError> {
        let mut current_era: Option<u64> = None;
        let mut eras: BTreeMap<u64, ReplayedEraRecord> = BTreeMap::new();
        let mut consumed_publications = BTreeSet::new();
        let mut consumed_invocations = BTreeSet::new();
        let mut active_invocations: BTreeMap<u64, (u64, String)> = BTreeMap::new();
        let mut consumed_retirements = BTreeSet::new();
        let mut issued_leases = BTreeSet::new();

        for (position, fact) in self.facts.iter().enumerate() {
            let reject = |diagnostic: &str| ComponentEraJournalReplayError {
                position,
                diagnostic: diagnostic.into(),
            };
            match fact {
                ComponentEraJournalFact::Published {
                    publication_identity,
                    binding_contract_identity,
                    entry_contract_identity,
                    previous_era_identity,
                    candidate,
                    new_era_visible,
                    previous_era_closed,
                } => {
                    if candidate.era_identity == 0
                        || candidate.artifact_instance_compatibility_report_identity == 0
                        || candidate.binding_contract_identity.trim().is_empty()
                        || candidate.entry_contract_identity.trim().is_empty()
                        || candidate.entry_plan_identity.trim().is_empty()
                        || candidate
                            .entry_plan_admission_receipt_identity
                            .trim()
                            .is_empty()
                    {
                        return Err(reject(
                            "journaled publication carries an incomplete era candidate",
                        ));
                    }
                    if candidate.binding_contract_identity != self.binding_contract_identity
                        || candidate.entry_contract_identity != self.entry_contract_identity
                        || *binding_contract_identity != self.binding_contract_identity
                        || *entry_contract_identity != self.entry_contract_identity
                    {
                        return Err(reject("journaled publication binds a foreign contract"));
                    }
                    if *previous_era_identity != current_era {
                        return Err(reject(
                            "journaled publication names a previous era that is not current",
                        ));
                    }
                    if *publication_identity == 0
                        || !consumed_publications.insert(*publication_identity)
                    {
                        return Err(reject("journaled publication identity is zero or replayed"));
                    }
                    if eras.contains_key(&candidate.era_identity) {
                        return Err(reject("journaled era identity is already live"));
                    }
                    if !*new_era_visible {
                        return Err(reject(
                            "journaled publication does not expose the candidate era",
                        ));
                    }
                    if current_era.is_some() && !*previous_era_closed {
                        return Err(reject(
                            "journaled publication does not close the previous era to future entry",
                        ));
                    }
                    if let Some(previous) = current_era {
                        eras.get_mut(&previous)
                            .expect("replayed current era is live")
                            .state = ComponentEraEntryState::Closing;
                    }
                    current_era = Some(candidate.era_identity);
                    eras.insert(
                        candidate.era_identity,
                        ReplayedEraRecord {
                            candidate: candidate.clone(),
                            state: ComponentEraEntryState::Open,
                            active_entries: 0,
                            epoch_leases: BTreeSet::new(),
                        },
                    );
                }
                ComponentEraJournalFact::Entered {
                    invocation_identity,
                    binding_contract_identity,
                    entry_contract_identity,
                    resolved_era_identity,
                    entry_plan_identity,
                    entry_linearized,
                } => {
                    if *binding_contract_identity != self.binding_contract_identity
                        || *entry_contract_identity != self.entry_contract_identity
                    {
                        return Err(reject("journaled entry binds a foreign contract"));
                    }
                    let Some(record) = eras.get_mut(resolved_era_identity) else {
                        return Err(reject("journaled entry names an era that is not live"));
                    };
                    if Some(*resolved_era_identity) != current_era
                        || record.state != ComponentEraEntryState::Open
                    {
                        return Err(reject(
                            "journaled entry resolves an era that is not the current open era",
                        ));
                    }
                    if *entry_plan_identity != record.candidate.entry_plan_identity {
                        return Err(reject(
                            "journaled entry carries a plan the era did not publish",
                        ));
                    }
                    if *invocation_identity == 0
                        || !*entry_linearized
                        || !consumed_invocations.insert(*invocation_identity)
                    {
                        return Err(reject(
                            "journaled entry does not linearize a fresh invocation",
                        ));
                    }
                    record.active_entries += 1;
                    active_invocations.insert(
                        *invocation_identity,
                        (*resolved_era_identity, entry_plan_identity.clone()),
                    );
                }
                ComponentEraJournalFact::Left {
                    invocation_identity,
                    binding_contract_identity,
                    era_identity,
                    entry_plan_identity,
                    leave_completed,
                } => {
                    if *binding_contract_identity != self.binding_contract_identity {
                        return Err(reject("journaled leave binds a foreign contract"));
                    }
                    let Some((active_era, active_plan)) =
                        active_invocations.get(invocation_identity)
                    else {
                        return Err(reject("journaled leave names an entry that never entered"));
                    };
                    if *active_era != *era_identity || active_plan != entry_plan_identity {
                        return Err(reject(
                            "journaled leave does not match the entered era and plan",
                        ));
                    }
                    if !*leave_completed {
                        return Err(reject("journaled leave did not complete"));
                    }
                    let record = eras
                        .get_mut(era_identity)
                        .expect("entered era remains live");
                    debug_assert!(record.active_entries > 0);
                    record.active_entries -= 1;
                    active_invocations.remove(invocation_identity);
                }
                ComponentEraJournalFact::EpochLeaseAcquired {
                    lease_identity,
                    era_identity,
                    entry_contract_identity,
                } => {
                    if *entry_contract_identity != self.entry_contract_identity {
                        return Err(reject("journaled epoch lease binds a foreign contract"));
                    }
                    let Some(record) = eras.get_mut(era_identity) else {
                        return Err(reject(
                            "journaled epoch lease names an era that is not live",
                        ));
                    };
                    if Some(*era_identity) != current_era
                        || record.state != ComponentEraEntryState::Open
                        || *entry_contract_identity != record.candidate.entry_contract_identity
                    {
                        return Err(reject(
                            "journaled epoch lease requires the exact current open era",
                        ));
                    }
                    if !issued_leases.insert(*lease_identity)
                        || !record.epoch_leases.insert(*lease_identity)
                    {
                        return Err(reject("journaled epoch lease identity is replayed"));
                    }
                }
                ComponentEraJournalFact::EpochLeaseReleased {
                    lease_identity,
                    era_identity,
                } => {
                    let Some(record) = eras.get_mut(era_identity) else {
                        return Err(reject(
                            "journaled epoch release names an era that is not live",
                        ));
                    };
                    if !issued_leases.contains(lease_identity)
                        || !record.epoch_leases.remove(lease_identity)
                    {
                        return Err(reject(
                            "journaled epoch release names no live hold of that era",
                        ));
                    }
                }
                ComponentEraJournalFact::Quiesced {
                    era_identity,
                    binding_contract_identity,
                    residual_lifetime_cohort_holds,
                    all_dispositions_complete,
                } => {
                    if *binding_contract_identity != self.binding_contract_identity {
                        return Err(reject("journaled quiescence binds a foreign contract"));
                    }
                    let Some(record) = eras.get_mut(era_identity) else {
                        return Err(reject("journaled quiescence names an era that is not live"));
                    };
                    if record.state != ComponentEraEntryState::Closing
                        || record.active_entries != 0
                        || !record.epoch_leases.is_empty()
                        || *residual_lifetime_cohort_holds != 0
                        || !*all_dispositions_complete
                    {
                        return Err(reject(
                            "journaled quiescence requires closing, no entries, no epoch holds, and complete disposition",
                        ));
                    }
                    record.state = ComponentEraEntryState::Quiescent;
                }
                ComponentEraJournalFact::Retired {
                    retirement_identity,
                    era_identity,
                    binding_contract_identity,
                    lifetime_cohort_released,
                } => {
                    if *binding_contract_identity != self.binding_contract_identity {
                        return Err(reject("journaled retirement binds a foreign contract"));
                    }
                    let Some(record) = eras.get(era_identity) else {
                        return Err(reject("journaled retirement names an era that is not live"));
                    };
                    if record.state != ComponentEraEntryState::Quiescent
                        || !record.epoch_leases.is_empty()
                        || Some(*era_identity) == current_era
                    {
                        return Err(reject(
                            "journaled retirement requires a noncurrent quiescent era with no holds",
                        ));
                    }
                    if *retirement_identity == 0
                        || !consumed_retirements.insert(*retirement_identity)
                        || !*lifetime_cohort_released
                    {
                        return Err(reject(
                            "journaled retirement does not linearize a fresh released cohort",
                        ));
                    }
                    eras.remove(era_identity);
                }
            }
        }

        Ok(ComponentEraJournalRoster { current_era, eras })
    }
}

/// Why replay refused a fact, with its position in the recorded sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraJournalReplayError {
    position: usize,
    diagnostic: String,
}

impl ComponentEraJournalReplayError {
    pub const fn position(&self) -> usize {
        self.position
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

#[cfg(test)]
mod tests;

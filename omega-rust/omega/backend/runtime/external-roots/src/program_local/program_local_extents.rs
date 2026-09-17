use std::collections::{BTreeMap, BTreeSet};

use effects::ComponentEraEntryLedger;
use extents::{
    Extent, ExtentLineageId, ExtentProgramLocalOrigin, ExtentRootGrant, OwnedExtentPartition,
    ValidatedExtentGeometry,
};
use language_semantics::content::{CanonicalIntervalSet, NaturalInterval};
use numerics::bignum::BigInt;
use semantic_vocabulary::ContentAlgebraKind;

use crate::{
    EstablishedProgramLocalRoot, EstablishedProgramLocalRootCapacity, ExternalRootDiagnostic,
    ProgramLocalRootEpochAggregateCapacity, ProgramLocalRootInstallationLedger,
    RetiredProgramLocalRootOccurrence,
};

mod activation_loans;
mod retained_foreign_arguments;

pub use retained_foreign_arguments::*;

#[derive(Debug)]
struct LiveRetention {
    base: u64,
    length: u64,
    access: RetainedForeignAccess,
}

/// The conserved siblings of one receiver partition, retained inert inside
/// the held account for the account's lifetime.
///
/// `Extent::partition_owned` splits a receiver extent into up to three
/// conserved pieces — the lower residual, the selected partition the account
/// backs onto, and the upper residual. When materialization consumes the
/// whole partition the residuals stay under the same ledger custody as the
/// backing instead of remaining ambient authority outside the ledger: while
/// the account is live no residual can be spent, dropped, or repartitioned,
/// and retirement rejoins them around the returned partition so the exact
/// receiver extent is restored rather than a detached range. The merge order
/// is fixed by the conserved split tree — the selected partition rejoins its
/// upper sibling first, then the lower residual rejoins the result — which
/// `OwnedExtentPartition::rejoin` documents and this replays.
#[derive(Debug, Default)]
struct ReceiverPartitionResiduals {
    before: Option<Extent>,
    after: Option<Extent>,
}

impl ReceiverPartitionResiduals {
    fn rejoin(self, selected: Extent) -> Extent {
        let mut restored = selected;
        if let Some(after) = self.after {
            restored = restored
                .merge(after)
                .expect("held receiver partition residual remains the exact upper sibling");
        }
        if let Some(before) = self.before {
            restored = before
                .merge(restored)
                .expect("held receiver partition residual remains the exact lower sibling");
        }
        restored
    }
}

/// One fully validated materialization input: the exact established
/// occurrence, the installed backing its interval capacity equals, and the
/// conserved receiver-partition residuals retained alongside when the
/// backing was carved out of a receiver extent. Plain backing inputs carry
/// empty residuals.
#[derive(Debug)]
struct MaterializationMember<'root, 'code> {
    root: EstablishedProgramLocalRoot<'root, 'code>,
    backing: Extent,
    residuals: ReceiverPartitionResiduals,
}

impl<'root, 'code> MaterializationMember<'root, 'code> {
    /// Restore the consumed input authority: for a receiver partition the
    /// residuals merge back around the selected backing into the exact
    /// receiver extent; for a plain backing this is the identity.
    fn into_consumed(self) -> (EstablishedProgramLocalRoot<'root, 'code>, Extent) {
        (self.root, self.residuals.rejoin(self.backing))
    }
}

#[derive(Debug)]
struct HeldProgramLocalExtent<'root, 'code> {
    root: EstablishedProgramLocalRoot<'root, 'code>,
    lineage: ExtentLineageId,
    /// The actual installed backing consumed into this account for the
    /// account's lifetime.
    ///
    /// The backing Extent is the real authority over the range the introduced
    /// root occupies — for example the receiver partition carved out of the
    /// installed writable image by `Extent::partition_owned`, or a
    /// provider-issued extent. Every runtime fact on the minted program-local
    /// Extent (geometry, address space, rights, provenance, and mapping era)
    /// is derived from this backing rather than caller-asserted, so there is
    /// no ambient provision. While the account is held the backing is inert
    /// inside the registry and the program-local Extent is the only live
    /// authority over its range; [`ProgramLocalExtentRegistry::retire`]
    /// returns the exact backing so the caller can rejoin it into the
    /// installed storage it was partitioned from.
    backing: Extent,
    /// The receiver-partition residuals held beside `backing` when the
    /// account materialized over an `OwnedExtentPartition`; empty for plain
    /// backing inputs.
    residuals: ReceiverPartitionResiduals,
    retained: BTreeMap<RetainedForeignArgumentId, LiveRetention>,
}

/// Epoch/installation owner for exact program-local Extent accounts.
///
/// Extents carry only passive origin identity. This registry retains the
/// actual installed occurrence, its actual installed backing, and lifecycle
/// lease while any split descendant may remain live. Dropping a registry does
/// not retire its accounts; it drops the Rust carrier while the underlying
/// lifecycle ledger remains held, which fails closed by preventing quiescence.
#[derive(Debug)]
pub struct ProgramLocalExtentRegistry<'root, 'code> {
    held: BTreeMap<ExtentProgramLocalOrigin, HeldProgramLocalExtent<'root, 'code>>,
    next_lineage: u64,
    next_retention: u64,
}

impl<'root, 'code> Default for ProgramLocalExtentRegistry<'root, 'code> {
    fn default() -> Self {
        Self {
            held: BTreeMap::new(),
            next_lineage: 1,
            next_retention: 1,
        }
    }
}

impl<'root, 'code> ProgramLocalExtentRegistry<'root, 'code> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn held_accounts(&self) -> usize {
        self.held.len()
    }

    pub fn live_retained_foreign_arguments(&self) -> usize {
        self.held.values().map(|held| held.retained.len()).sum()
    }

    /// Atomically materialize a batch over actual installed backing. Every
    /// account and backing Extent is validated before any passive Extent grant
    /// is minted or any account is retained. Each backing Extent is consumed
    /// into its held account for the account's lifetime: the minted
    /// program-local Extent is the only live authority over its range, and
    /// every runtime fact on it derives from that exact backing rather than a
    /// caller roster of asserted facts.
    pub fn materialize_batch(
        &mut self,
        inputs: Vec<(EstablishedProgramLocalRoot<'root, 'code>, Extent)>,
    ) -> Result<Vec<Extent>, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let mut origins = BTreeSet::new();
        for (root, backing) in &inputs {
            if let Err(diagnostic) =
                self.validate_materialization_member(&mut origins, root, backing)
            {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs,
                    diagnostic,
                }));
            }
        }
        self.mint_held(
            inputs
                .into_iter()
                .map(|(root, backing)| MaterializationMember {
                    root,
                    backing,
                    residuals: ReceiverPartitionResiduals::default(),
                })
                .collect(),
        )
    }

    /// Materialize one established root over the receiver partition carved
    /// out of installed writable backing by `Extent::partition_owned`.
    ///
    /// Where [`ProgramLocalExtentRegistry::materialize`] consumes a bare
    /// backing Extent and leaves the partition residuals as ambient authority
    /// outside the ledger, this route consumes the whole
    /// [`OwnedExtentPartition`]: the selected member is validated as the
    /// account's backing exactly as there, and the conserved lower/upper
    /// residuals are retained inert inside the held account for the same
    /// occurrence and lifecycle epoch — they cannot be spent, dropped, or
    /// repartitioned while the program-local account is live. Completion
    /// rejoins them around the returned partition, so
    /// [`RetiredProgramLocalExtent::backing`] restores the exact receiver
    /// extent rather than a detached range.
    ///
    /// The selected member must equal the root's evaluated interval capacity
    /// and cannot carry a program-local origin: partitioning a held
    /// program-local account's Extent cannot reticket the same installed
    /// range under a second occurrence. Rejection returns the input with its
    /// partition rejoined into the restored receiver extent.
    pub fn materialize_over_receiver(
        &mut self,
        root: EstablishedProgramLocalRoot<'root, 'code>,
        receiver_partition: OwnedExtentPartition,
    ) -> Result<Extent, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let [extent]: [Extent; 1] = self
            .materialize_batch_over_receiver(vec![(root, receiver_partition)])?
            .try_into()
            .expect("one receiver partition materializes one Extent");
        Ok(extent)
    }

    /// Batch counterpart of
    /// [`ProgramLocalExtentRegistry::materialize_over_receiver`]: every
    /// member's partition is validated before any Extent mints or any
    /// residual is retained. Rejection returns each input with its partition
    /// rejoined into the restored receiver extent.
    pub fn materialize_batch_over_receiver(
        &mut self,
        inputs: Vec<(
            EstablishedProgramLocalRoot<'root, 'code>,
            OwnedExtentPartition,
        )>,
    ) -> Result<Vec<Extent>, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let mut origins = BTreeSet::new();
        for (root, partition) in &inputs {
            if let Err(diagnostic) =
                self.validate_materialization_member(&mut origins, root, partition.selected())
            {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs: inputs
                        .into_iter()
                        .map(|(root, partition)| (root, partition.rejoin()))
                        .collect(),
                    diagnostic,
                }));
            }
        }
        self.mint_held(
            inputs
                .into_iter()
                .map(|(root, partition)| {
                    let (before, backing, after) = partition.into_parts();
                    MaterializationMember {
                        root,
                        backing,
                        residuals: ReceiverPartitionResiduals { before, after },
                    }
                })
                .collect(),
        )
    }

    /// Atomically materialize the complete live membership of one
    /// reconstructed aggregate capacity over its actual installed backing
    /// partitions.
    ///
    /// The aggregate is the requirement this batch discharges, not a roster
    /// hint. The installation ledger re-derives the group's complete live
    /// membership from the presented members, so the presented aggregate must
    /// still equal the live reconstruction for the same installed occurrence
    /// and lifecycle epoch: a stale or substituted row — reconstructed
    /// before a pending member established, under a different epoch, or for
    /// a foreign lifecycle — understates live demand and rejects, as does an
    /// omitted, repeated, or substituted member. Each member's backing must
    /// equal its own evaluated interval in one shared address space, and the
    /// presented receiver partitions must compose — disjoint or exactly
    /// adjacent, with overlap rejected — to the exact reconstructed interval
    /// set, so the installed backing covers the group's whole live demand
    /// without a gap or remainder.
    ///
    /// Success mints one program-local Extent per member — the authority the
    /// establishing activation borrows through
    /// [`ProgramLocalExtentRegistry::loan_under_activation`]/
    /// [`ProgramLocalExtentRegistry::loan_mut_under_activation`], the
    /// checked `Extent::loan`/`loan_mut` route — and retains each account
    /// until its recombined root returns through
    /// [`ProgramLocalExtentRegistry::retire`], which releases the exact
    /// occurrence and returns its partition for rejoin into installed
    /// storage. When the whole discharged membership ends inside the
    /// cohort's epoch, [`ProgramLocalExtentRegistry::retire_aggregate`]
    /// completes the same occurrence set and epoch in one transaction.
    pub fn materialize_aggregate(
        &mut self,
        installation: &ProgramLocalRootInstallationLedger,
        lifecycle: &ComponentEraEntryLedger,
        aggregate: &ProgramLocalRootEpochAggregateCapacity,
        inputs: Vec<(EstablishedProgramLocalRoot<'root, 'code>, Extent)>,
    ) -> Result<Vec<Extent>, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let Some(required) = aggregate.capacity().interval_set() else {
            return Err(Box::new(ProgramLocalExtentMaterializationError {
                inputs,
                diagnostic: ExternalRootDiagnostic(
                    "counted program-local aggregate capacity cannot materialize Extent partitions"
                        .into(),
                ),
            }));
        };
        let fresh = match installation
            .reconstruct_aggregate_capacity(lifecycle, inputs.iter().map(|(root, _)| root))
        {
            Ok(fresh) => fresh,
            Err(diagnostic) => {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs,
                    diagnostic,
                }));
            }
        };
        if fresh != *aggregate {
            return Err(Box::new(ProgramLocalExtentMaterializationError {
                inputs,
                diagnostic: ExternalRootDiagnostic(
                    "presented program-local aggregate capacity is stale or substituted for the live reconstructed membership".into(),
                ),
            }));
        }

        let mut origins = BTreeSet::new();
        let mut backings = Vec::with_capacity(inputs.len());
        for (root, backing) in &inputs {
            if let Err(diagnostic) =
                self.validate_materialization_member(&mut origins, root, backing)
            {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs,
                    diagnostic,
                }));
            }
            backings.push(backing);
        }
        if backings
            .windows(2)
            .any(|pair| pair[0].address_space() != pair[1].address_space())
        {
            return Err(Box::new(ProgramLocalExtentMaterializationError {
                inputs,
                diagnostic: ExternalRootDiagnostic(
                    "program-local aggregate backing partitions span distinct address spaces"
                        .into(),
                ),
            }));
        }
        let covered = match CanonicalIntervalSet::new(backings.iter().map(|extent| {
            NaturalInterval::new(
                BigInt::from_u64(extent.base()),
                BigInt::from_u64(extent.end()),
            )
            .expect("validated installed backing is a nonempty proof-natural interval")
        })) {
            Ok(covered) => covered,
            Err(_overlap) => {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs,
                    diagnostic: ExternalRootDiagnostic(
                        "program-local aggregate backing partitions overlap".into(),
                    ),
                }));
            }
        };
        if covered != *required {
            return Err(Box::new(ProgramLocalExtentMaterializationError {
                inputs,
                diagnostic: ExternalRootDiagnostic(
                    "installed backing partitions do not compose the exact reconstructed aggregate capacity".into(),
                ),
            }));
        }
        self.mint_held(
            inputs
                .into_iter()
                .map(|(root, backing)| MaterializationMember {
                    root,
                    backing,
                    residuals: ReceiverPartitionResiduals::default(),
                })
                .collect(),
        )
    }

    /// The receiver-partition counterpart of
    /// [`ProgramLocalExtentRegistry::materialize_aggregate`]: each presented
    /// member is a whole [`OwnedExtentPartition`] carved out of installed
    /// writable backing, the selected member of each is validated and
    /// composed exactly as there, and every partition's conserved residuals
    /// are retained inert inside its held account for the same occurrence
    /// and lifecycle epoch. Completion of the discharged membership rejoins
    /// each member's residuals around its returned partition, restoring the
    /// exact receiver extents the partitions were carved from.
    ///
    /// Rejection — a stale or substituted aggregate, a repeated occurrence,
    /// a mismatched selected member, mixed address spaces, overlapping
    /// partitions, or an under-covering composition — returns each input
    /// with its partition rejoined into the restored receiver extent.
    pub fn materialize_aggregate_over_receiver(
        &mut self,
        installation: &ProgramLocalRootInstallationLedger,
        lifecycle: &ComponentEraEntryLedger,
        aggregate: &ProgramLocalRootEpochAggregateCapacity,
        inputs: Vec<(
            EstablishedProgramLocalRoot<'root, 'code>,
            OwnedExtentPartition,
        )>,
    ) -> Result<Vec<Extent>, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let rejoin_inputs = |inputs: Vec<(
            EstablishedProgramLocalRoot<'root, 'code>,
            OwnedExtentPartition,
        )>,
                             diagnostic| {
            Box::new(ProgramLocalExtentMaterializationError {
                inputs: inputs
                    .into_iter()
                    .map(|(root, partition)| (root, partition.rejoin()))
                    .collect(),
                diagnostic,
            })
        };
        let Some(required) = aggregate.capacity().interval_set() else {
            return Err(rejoin_inputs(
                inputs,
                ExternalRootDiagnostic(
                    "counted program-local aggregate capacity cannot materialize Extent partitions"
                        .into(),
                ),
            ));
        };
        let fresh = match installation
            .reconstruct_aggregate_capacity(lifecycle, inputs.iter().map(|(root, _)| root))
        {
            Ok(fresh) => fresh,
            Err(diagnostic) => {
                return Err(rejoin_inputs(inputs, diagnostic));
            }
        };
        if fresh != *aggregate {
            return Err(rejoin_inputs(
                inputs,
                ExternalRootDiagnostic(
                    "presented program-local aggregate capacity is stale or substituted for the live reconstructed membership".into(),
                ),
            ));
        }

        let mut origins = BTreeSet::new();
        let mut backings = Vec::with_capacity(inputs.len());
        for (root, partition) in &inputs {
            if let Err(diagnostic) =
                self.validate_materialization_member(&mut origins, root, partition.selected())
            {
                return Err(rejoin_inputs(inputs, diagnostic));
            }
            backings.push(partition.selected());
        }
        if backings
            .windows(2)
            .any(|pair| pair[0].address_space() != pair[1].address_space())
        {
            return Err(rejoin_inputs(
                inputs,
                ExternalRootDiagnostic(
                    "program-local aggregate backing partitions span distinct address spaces"
                        .into(),
                ),
            ));
        }
        let covered = match CanonicalIntervalSet::new(backings.iter().map(|extent| {
            NaturalInterval::new(
                BigInt::from_u64(extent.base()),
                BigInt::from_u64(extent.end()),
            )
            .expect("validated installed backing is a nonempty proof-natural interval")
        })) {
            Ok(covered) => covered,
            Err(_overlap) => {
                return Err(rejoin_inputs(
                    inputs,
                    ExternalRootDiagnostic(
                        "program-local aggregate backing partitions overlap".into(),
                    ),
                ));
            }
        };
        if covered != *required {
            return Err(rejoin_inputs(
                inputs,
                ExternalRootDiagnostic(
                    "installed backing partitions do not compose the exact reconstructed aggregate capacity".into(),
                ),
            ));
        }
        self.mint_held(
            inputs
                .into_iter()
                .map(|(root, partition)| {
                    let (before, backing, after) = partition.into_parts();
                    MaterializationMember {
                        root,
                        backing,
                        residuals: ReceiverPartitionResiduals { before, after },
                    }
                })
                .collect(),
        )
    }

    /// Reject one member/backing pair before commitment: the exact
    /// established occurrence must not already be held by this registry or
    /// repeated inside the batch, and the backing must equal the member's
    /// evaluated interval capacity.
    fn validate_materialization_member(
        &self,
        origins: &mut BTreeSet<ExtentProgramLocalOrigin>,
        root: &EstablishedProgramLocalRoot<'root, 'code>,
        backing: &Extent,
    ) -> Result<(), ExternalRootDiagnostic> {
        let origin = exact_origin(root)?;
        if self.held.contains_key(&origin) || !origins.insert(origin) {
            return Err(ExternalRootDiagnostic(
                "program-local Extent batch repeats an exact established occurrence".into(),
            ));
        }
        validate_materialization(root, backing)
    }

    /// Commit a fully validated member/backing batch: reserve one lineage
    /// identity per member, mint each program-local Extent over its backing's
    /// own runtime facts, and retain the accounts — including each member's
    /// receiver-partition residuals. Rejection returns every input's full
    /// consumed authority, rejoining residuals around their selected backing.
    fn mint_held(
        &mut self,
        members: Vec<MaterializationMember<'root, 'code>>,
    ) -> Result<Vec<Extent>, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let count = match u64::try_from(members.len()) {
            Ok(count) => count,
            Err(_) => {
                return Err(Box::new(ProgramLocalExtentMaterializationError {
                    inputs: members
                        .into_iter()
                        .map(MaterializationMember::into_consumed)
                        .collect(),
                    diagnostic: ExternalRootDiagnostic(
                        "program-local Extent batch cardinality does not fit its lineage space"
                            .into(),
                    ),
                }));
            }
        };
        let Some(next_lineage) = self.next_lineage.checked_add(count) else {
            return Err(Box::new(ProgramLocalExtentMaterializationError {
                inputs: members
                    .into_iter()
                    .map(MaterializationMember::into_consumed)
                    .collect(),
                diagnostic: ExternalRootDiagnostic(
                    "program-local Extent lineage space is exhausted".into(),
                ),
            }));
        };

        let first_lineage = self.next_lineage;
        self.next_lineage = next_lineage;
        let mut extents = Vec::with_capacity(members.len());
        for (offset, member) in members.into_iter().enumerate() {
            let MaterializationMember {
                root,
                backing,
                residuals,
            } = member;
            let origin = exact_origin(&root)
                .expect("validated established program-local origin remains exact");
            let lineage = ExtentLineageId::from_normalized_identity(
                first_lineage + u64::try_from(offset).expect("batch offset fits u64"),
            )
            .expect("reserved program-local lineage identities are nonzero");
            let geometry = ValidatedExtentGeometry::check(backing.base(), backing.length())
                .expect("validated installed backing geometry remains valid");
            let extent = ExtentRootGrant::from_established_program_local(
                origin,
                lineage,
                backing.address_space(),
                backing.rights().clone(),
                backing.provenance(),
                backing.era(),
            )
            .mint_validated(geometry);
            let previous = self.held.insert(
                origin,
                HeldProgramLocalExtent {
                    root,
                    lineage,
                    backing,
                    residuals,
                    retained: BTreeMap::new(),
                },
            );
            debug_assert!(previous.is_none());
            extents.push(extent);
        }
        Ok(extents)
    }

    /// Materialize one established root over its actual installed backing. The
    /// backing Extent must cover exactly the root's evaluated interval
    /// capacity in this installed occurrence's lifecycle epoch; its authority
    /// is consumed into the held account and returned by
    /// [`ProgramLocalExtentRegistry::retire`].
    pub fn materialize(
        &mut self,
        root: EstablishedProgramLocalRoot<'root, 'code>,
        backing: Extent,
    ) -> Result<Extent, Box<ProgramLocalExtentMaterializationError<'root, 'code>>> {
        let [extent]: [Extent; 1] = self
            .materialize_batch(vec![(root, backing)])?
            .try_into()
            .expect("one program-local input materializes one Extent");
        Ok(extent)
    }

    /// Resolve one Extent's claimed program-local origin to its held account
    /// in this registry: the origin must name a live account, and the
    /// Extent's lineage root and runtime facts (address space, provenance,
    /// mapping era) must match the actual installed backing consumed at
    /// materialization. Shared by activation loans and retained foreign
    /// arguments; a provider-issued or ambient Extent has no such account.
    fn validate_backing(
        &self,
        extent: &Extent,
    ) -> Result<
        (
            &HeldProgramLocalExtent<'root, 'code>,
            ExtentProgramLocalOrigin,
        ),
        ExternalRootDiagnostic,
    > {
        let Some(origin) = extent.program_local_origin() else {
            return Err(ExternalRootDiagnostic(
                "extent has unknown ambient backing: the Extent is not rooted in an established program-local account"
                    .into(),
            ));
        };
        let Some(held) = self.held.get(&origin) else {
            return Err(ExternalRootDiagnostic(
                "extent has unknown ambient backing: no held program-local account exists for the Extent"
                    .into(),
            ));
        };
        if extent.lineage_root() != held.lineage {
            return Err(ExternalRootDiagnostic(
                "extent has substituted lineage for its held program-local account".into(),
            ));
        }
        if extent.address_space() != held.backing.address_space()
            || extent.provenance() != held.backing.provenance()
            || extent.era() != held.backing.era()
        {
            return Err(ExternalRootDiagnostic(
                "extent revision provenance does not match the installed occurrence (mapping era / provenance / address space)"
                    .into(),
            ));
        }
        Ok((held, origin))
    }

    /// Consume the exact recombined root Extent and release its retained
    /// installed occurrence. Split descendants and substituted runtime facts
    /// reject without removing the held account. Success returns the exact
    /// installed authority consumed at materialization, completing the
    /// account's custody of that range — for an account materialized over a
    /// receiver partition the retained residuals rejoin around the selected
    /// backing, so the returned extent is the restored receiver itself. The
    /// aggregate counterpart
    /// [`ProgramLocalExtentRegistry::retire_aggregate`] completes one
    /// aggregate schema group's complete live membership in the same epoch.
    pub fn retire(
        &mut self,
        extent: Extent,
        installation: &mut ProgramLocalRootInstallationLedger,
        lifecycle: &mut ComponentEraEntryLedger,
    ) -> Result<RetiredProgramLocalExtent, Box<ProgramLocalExtentRetirementError>> {
        let origin = match self.validate_retirement_member(&extent) {
            Ok(origin) => origin,
            Err(diagnostic) => {
                return Err(Box::new(ProgramLocalExtentRetirementError::new(
                    extent,
                    diagnostic.0,
                )));
            }
        };

        let HeldProgramLocalExtent {
            root,
            lineage,
            backing,
            residuals,
            retained,
        } = self
            .held
            .remove(&origin)
            .expect("validated held program-local account remains present");
        match installation.retire_established(root, lifecycle) {
            Ok(occurrence) => Ok(RetiredProgramLocalExtent {
                occurrence,
                backing: residuals.rejoin(backing),
            }),
            Err(error) => {
                let root = (*error).into_root();
                let replaced = self.held.insert(
                    origin,
                    HeldProgramLocalExtent {
                        root,
                        lineage,
                        backing,
                        residuals,
                        retained,
                    },
                );
                debug_assert!(replaced.is_none());
                Err(Box::new(ProgramLocalExtentRetirementError::new(
                    extent,
                    "program-local Extent retirement could not release its exact lifecycle lease",
                )))
            }
        }
    }

    /// Atomically complete the complete live membership of one reconstructed
    /// aggregate — the completion counterpart of
    /// [`ProgramLocalExtentRegistry::materialize_aggregate`].
    ///
    /// Every presented Extent must resolve to a held account as its exact
    /// recombined lineage root, carrying the held installed backing's runtime
    /// facts and blocked by no live retained foreign argument — the same
    /// per-member checks [`ProgramLocalExtentRegistry::retire`] applies —
    /// and no two members may name one occurrence. The installation ledger
    /// then re-derives the group's complete live established membership from
    /// the presented members' retained roots, so an omitted, substituted,
    /// cross-group, or cross-cohort member rejects transactionally with the
    /// presented extents, as does a presented aggregate that no longer equals
    /// the live reconstruction for the same lifecycle cohort.
    ///
    /// Because reconstruction replays each member's live epoch lease,
    /// aggregate completion runs inside the cohort's epoch; after an epoch
    /// roll the single-account [`ProgramLocalExtentRegistry::retire`] route
    /// still releases each held occurrence's stale-era lease. Success
    /// releases every member's lifecycle lease through the exact ledger
    /// retirement and returns each member's installed authority — the
    /// receiver partitions consumed at materialization, each rejoined with
    /// its retained residuals into the restored receiver extent — in
    /// presented order for rejoin into installed storage. A counted
    /// aggregate names no
    /// Extent-partitioned membership and rejects, as does an empty member
    /// set.
    pub fn retire_aggregate(
        &mut self,
        installation: &mut ProgramLocalRootInstallationLedger,
        lifecycle: &mut ComponentEraEntryLedger,
        aggregate: &ProgramLocalRootEpochAggregateCapacity,
        extents: Vec<Extent>,
    ) -> Result<Vec<RetiredProgramLocalExtent>, Box<ProgramLocalExtentAggregateRetirementError>>
    {
        if aggregate.capacity().interval_set().is_none() {
            return Err(Box::new(
                ProgramLocalExtentAggregateRetirementError::validation(
                    extents,
                    "counted program-local aggregate capacity names no Extent-partitioned membership to complete",
                ),
            ));
        }
        if extents.is_empty() {
            return Err(Box::new(
                ProgramLocalExtentAggregateRetirementError::validation(
                    extents,
                    "program-local Extent aggregate retirement requires at least one recombined member",
                ),
            ));
        }

        let mut origins = BTreeSet::new();
        let mut ordered = Vec::with_capacity(extents.len());
        for extent in &extents {
            let origin = match self.validate_retirement_member(extent) {
                Ok(origin) => origin,
                Err(diagnostic) => {
                    return Err(Box::new(
                        ProgramLocalExtentAggregateRetirementError::validation(
                            extents,
                            diagnostic.0,
                        ),
                    ));
                }
            };
            if !origins.insert(origin) {
                return Err(Box::new(
                    ProgramLocalExtentAggregateRetirementError::validation(
                        extents,
                        "program-local Extent aggregate retirement repeats one exact occurrence",
                    ),
                ));
            }
            ordered.push(origin);
        }

        let fresh = match installation.reconstruct_aggregate_capacity(
            lifecycle,
            ordered.iter().map(|origin| &self.held[origin].root),
        ) {
            Ok(fresh) => fresh,
            Err(diagnostic) => {
                return Err(Box::new(
                    ProgramLocalExtentAggregateRetirementError::validation(extents, diagnostic.0),
                ));
            }
        };
        if fresh != *aggregate {
            return Err(Box::new(
                ProgramLocalExtentAggregateRetirementError::validation(
                    extents,
                    "presented program-local aggregate capacity is stale or substituted for the live reconstructed membership",
                ),
            ));
        }

        let mut extents_iter = extents.into_iter();
        let mut retired = Vec::with_capacity(ordered.len());
        for origin in ordered {
            let extent = extents_iter
                .next()
                .expect("each validated member retains its Extent");
            let HeldProgramLocalExtent {
                root,
                lineage,
                backing,
                residuals,
                retained,
            } = self
                .held
                .remove(&origin)
                .expect("validated held program-local account remains present");
            match installation.retire_established(root, lifecycle) {
                Ok(occurrence) => retired.push(RetiredProgramLocalExtent {
                    occurrence,
                    backing: residuals.rejoin(backing),
                }),
                Err(error) => {
                    let root = (*error).into_root();
                    let replaced = self.held.insert(
                        origin,
                        HeldProgramLocalExtent {
                            root,
                            lineage,
                            backing,
                            residuals,
                            retained,
                        },
                    );
                    debug_assert!(replaced.is_none());
                    let mut uncommitted = Vec::with_capacity(1 + extents_iter.len());
                    uncommitted.push(extent);
                    uncommitted.extend(extents_iter);
                    return Err(Box::new(ProgramLocalExtentAggregateRetirementError {
                        extents: uncommitted,
                        retired,
                        diagnostic: ExternalRootDiagnostic(
                            "program-local Extent aggregate retirement could not release its exact lifecycle lease".into(),
                        ),
                    }));
                }
            }
        }
        debug_assert!(extents_iter.next().is_none());
        Ok(retired)
    }

    /// Reject one Extent that cannot complete a held program-local account:
    /// it must name a live account's exact origin, be that account's
    /// recombined lineage root carrying the held installed backing's runtime
    /// facts, and the account must have no live retained foreign arguments.
    /// Shared by single retirement and aggregate completion; success returns
    /// the held account's origin.
    fn validate_retirement_member(
        &self,
        extent: &Extent,
    ) -> Result<ExtentProgramLocalOrigin, ExternalRootDiagnostic> {
        let Some(origin) = extent.program_local_origin() else {
            return Err(ExternalRootDiagnostic(
                "program-local Extent retirement received a provider-issued root".into(),
            ));
        };
        let Some(held) = self.held.get(&origin) else {
            return Err(ExternalRootDiagnostic(
                "program-local Extent retirement names no held exact occurrence".into(),
            ));
        };
        if !extent.is_lineage_root()
            || extent.lineage_root() != held.lineage
            || extent.base() != held.backing.base()
            || extent.length() != held.backing.length()
            || extent.address_space() != held.backing.address_space()
            || extent.provenance() != held.backing.provenance()
            || extent.era() != held.backing.era()
            || !held.backing.rights().contains(extent.rights())
        {
            return Err(ExternalRootDiagnostic(
                "program-local Extent retirement requires the exact recombined root and the held installed backing's runtime facts".into(),
            ));
        }
        if !held.retained.is_empty() {
            return Err(ExternalRootDiagnostic(format!(
                "program-local Extent account retirement is blocked by {} live retained foreign argument(s)",
                held.retained.len()
            )));
        }
        Ok(origin)
    }
}

/// Result of one program-local Extent account retirement: the released exact
/// installed occurrence plus the actual installed authority consumed at
/// materialization, returned to the caller's custody. For an account
/// materialized over an [`OwnedExtentPartition`] the consumed authority is
/// the whole partition, so `backing` is the receiver extent restored by
/// rejoining the retained residuals around the returned partition — not a
/// detached range of it.
#[derive(Debug)]
pub struct RetiredProgramLocalExtent {
    occurrence: RetiredProgramLocalRootOccurrence,
    backing: Extent,
}

impl RetiredProgramLocalExtent {
    pub const fn occurrence(&self) -> &RetiredProgramLocalRootOccurrence {
        &self.occurrence
    }

    pub const fn backing(&self) -> &Extent {
        &self.backing
    }

    pub fn into_backing(self) -> Extent {
        self.backing
    }
}

/// Rejection of a materialization route. Every input's full consumed
/// authority returns in presented order — for a receiver-partition input the
/// partition is rejoined into the restored receiver extent first, so no
/// residual is dropped into ambient custody on failure.
#[derive(Debug)]
pub struct ProgramLocalExtentMaterializationError<'root, 'code> {
    inputs: Vec<(EstablishedProgramLocalRoot<'root, 'code>, Extent)>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalExtentMaterializationError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    /// Consume into the inputs with each receiver partition rejoined into
    /// its restored receiver extent.
    pub fn into_inputs(self) -> Vec<(EstablishedProgramLocalRoot<'root, 'code>, Extent)> {
        self.inputs
    }
}

#[derive(Debug)]
pub struct ProgramLocalExtentRetirementError {
    extent: Extent,
    diagnostic: ExternalRootDiagnostic,
}

impl ProgramLocalExtentRetirementError {
    fn new(extent: Extent, diagnostic: impl Into<String>) -> Self {
        Self {
            extent,
            diagnostic: ExternalRootDiagnostic(diagnostic.into()),
        }
    }

    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_extent(self) -> Extent {
        self.extent
    }
}

/// Rejection of [`ProgramLocalExtentRegistry::retire_aggregate`]. The
/// uncommitted member Extents return in presented order — the rejected member
/// followed by every member never attempted — alongside the members that
/// completed before a mid-commit lease failure, so no installed backing is
/// dropped. Validation rejections always carry an empty retired prefix.
#[derive(Debug)]
pub struct ProgramLocalExtentAggregateRetirementError {
    extents: Vec<Extent>,
    retired: Vec<RetiredProgramLocalExtent>,
    diagnostic: ExternalRootDiagnostic,
}

impl ProgramLocalExtentAggregateRetirementError {
    fn validation(extents: Vec<Extent>, diagnostic: impl Into<String>) -> Self {
        Self {
            extents,
            retired: Vec::new(),
            diagnostic: ExternalRootDiagnostic(diagnostic.into()),
        }
    }

    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    /// Consume into the uncommitted member Extents and the retired prefix,
    /// both in presented order.
    pub fn into_parts(self) -> (Vec<Extent>, Vec<RetiredProgramLocalExtent>) {
        (self.extents, self.retired)
    }
}

fn exact_origin(
    root: &EstablishedProgramLocalRoot<'_, '_>,
) -> Result<ExtentProgramLocalOrigin, ExternalRootDiagnostic> {
    let occurrence = root.occurrence_identity();
    let prebinding = occurrence.prebinding();
    ExtentProgramLocalOrigin::from_normalized_identities([
        prebinding.installed_code().normalized_identity(),
        prebinding.root().normalized_identity(),
        prebinding.slot().normalized_identity(),
        root.prebinding().schema_compatibility_report_identity(),
        occurrence.lifecycle_ledger().normalized_identity(),
        occurrence.lifecycle_epoch(),
        root.invocation().normalized_identity(),
        root.subject_place().normalized_identity(),
    ])
    .map_err(|diagnostic| {
        ExternalRootDiagnostic(format!(
            "established program-local root has no exact Extent origin: {diagnostic}"
        ))
    })
}

/// Check that one actual installed backing Extent is the exact range the
/// established root's interval capacity evaluates to. The backing supplies
/// every runtime fact; a program-local Extent already held by an account is
/// not installed backing and cannot be reticketed under a second occurrence.
fn validate_materialization(
    root: &EstablishedProgramLocalRoot<'_, '_>,
    backing: &Extent,
) -> Result<(), ExternalRootDiagnostic> {
    let prebinding = root.prebinding();
    let EstablishedProgramLocalRootCapacity::IntervalSet(capacity) = root.capacity() else {
        return Err(ExternalRootDiagnostic(
            "counted program-local capacity cannot materialize one Extent".into(),
        ));
    };
    if prebinding.algebra().kind != ContentAlgebraKind::IntervalSet {
        return Err(ExternalRootDiagnostic(
            "program-local Extent requires the exact installed interval-set algebra".into(),
        ));
    }
    if backing.program_local_origin().is_some() {
        return Err(ExternalRootDiagnostic(
            "program-local Extent materialization requires actual installed backing, not another held program-local account's authority".into(),
        ));
    }
    let [member] = capacity.members() else {
        return Err(ExternalRootDiagnostic(
            "program-local Extent requires exactly one nonempty interval".into(),
        ));
    };
    let Some(start) = member.start().to_u64() else {
        return Err(ExternalRootDiagnostic(
            "program-local Extent interval start does not fit the target address model".into(),
        ));
    };
    let Some(end) = member.end().to_u64() else {
        return Err(ExternalRootDiagnostic(
            "program-local Extent interval end does not fit the target address model".into(),
        ));
    };
    if start != backing.base() || end != backing.end() || start == end {
        return Err(ExternalRootDiagnostic(
            "program-local Extent backing does not equal its established interval capacity".into(),
        ));
    }
    Ok(())
}

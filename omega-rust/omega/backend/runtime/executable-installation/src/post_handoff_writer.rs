//! Validate and execute post-handoff writers against one exact installed realization.
//!
//! Destination custody, once-resolved context, write results, and consumer replay
//! belong to this protocol. Successful output remains unpublished; it does not
//! grant hardware-table validity or executable installation authority.

use crate::authority_digests::{
    ArtifactId, DestinationPreparationReceiptId, InstalledCodeId,
    NonAuthoritativeWriterContextFingerprint64,
};
use crate::installation::InstalledCodeEvidence;
use crate::installation::{InstallationDiagnostic, InstalledCode};
use extents::{ExtentRights, MappedExtent, MappingReceiptContext};
use layout_plans::{
    MaterializationDiagnostic, POST_HANDOFF_WRITER_CONTEXT_ABI_V1, PlacementSite,
    PostHandoffWriterInvocationPlan, PostHandoffWriterPlan, PostHandoffWriterSource,
    PostHandoffWriterSourceSlot, RelocationTarget,
};

impl InstalledCode {
    /// Check the exact post-handoff entry writer without resolving an entry
    /// address into a public value or changing destination bytes. This is the
    /// provider-side preparation gate used before compiler-generated writer
    /// lowering. Pre-resolved entry fragments must equal the address from this
    /// exact installed realization; unresolved fragments must be members of
    /// this artifact's admitted entry set.
    pub fn validate_post_handoff_entry_writer(
        &self,
        plan: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        let invocation = plan.lower_reusable_fragment()?;
        self.validate_post_handoff_entry_writer_invocation(
            plan,
            &invocation,
            destination_len,
            destination_site,
        )
        .map(|_| ())
    }

    fn validate_post_handoff_entry_writer_invocation(
        &self,
        plan: &PostHandoffWriterPlan,
        invocation: &PostHandoffWriterInvocationPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<Vec<u64>, MaterializationDiagnostic> {
        plan.validate(destination_len, destination_site)?;
        let mut source_values = Vec::with_capacity(invocation.sources().len());
        for slot in invocation.sources() {
            let value = match slot.source {
                PostHandoffWriterSource::Resolve(target) => {
                    if target != slot.target || !self.contains_entry_target(target) {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff writer target {target:?} is not an admitted entry in the exact installed artifact"
                        )));
                    }
                    self.resolve_entry_target(target).ok_or_else(|| {
                        MaterializationDiagnostic(format!(
                            "post-handoff writer could not resolve admitted target {target:?}"
                        ))
                    })?
                }
                PostHandoffWriterSource::Resolved(value) => match slot.target {
                    RelocationTarget::Entry(_)
                        if self.resolve_entry_target(slot.target) == Some(value) =>
                    {
                        value
                    }
                    RelocationTarget::Entry(_) => {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff writer pre-resolved entry {:?} does not match the exact installed realization",
                            slot.target
                        )));
                    }
                    RelocationTarget::Data(_) => {
                        return Err(MaterializationDiagnostic(format!(
                            "post-handoff entry writer target {:?} is not an admitted entry in the exact installed artifact",
                            slot.target
                        )));
                    }
                },
            };
            source_values.push(value);
        }
        invocation.validate_source_values(&source_values)?;
        Ok(source_values)
    }

    /// Resolve every distinct source exactly once into an opaque packed
    /// provider context. No numeric code or destination address is returned to
    /// the caller; only the sealed carrier may be passed to checked execution.
    pub fn populate_post_handoff_entry_writer_context(
        &self,
        plan: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<ResolvedPostHandoffEntryWriterContext, MaterializationDiagnostic> {
        let invocation = plan.lower_reusable_fragment()?;
        let source_values = self.validate_post_handoff_entry_writer_invocation(
            plan,
            &invocation,
            destination_len,
            destination_site,
        )?;

        let mut packed_words = Vec::with_capacity(invocation.sources().len() + 1);
        packed_words.push(destination_site.base_address);
        packed_words.extend(source_values);
        let non_authoritative_fingerprint =
            non_authoritative_post_handoff_entry_writer_context_fingerprint(
                self.identity,
                self.artifact(),
                destination_site,
                destination_len,
                &invocation,
                &packed_words,
            );
        Ok(ResolvedPostHandoffEntryWriterContext {
            installed_evidence: InstalledCodeEvidence::from_installed(self),
            installed_code: self.identity,
            artifact: self.artifact(),
            destination_site,
            destination_len,
            invocation,
            packed_words,
            non_authoritative_fingerprint,
        })
    }

    /// Execute with the exact once-resolved values sealed into `context`.
    /// Context/plan/site drift rejects before destination mutation.
    pub fn execute_populated_post_handoff_entry_writer(
        &self,
        context: &ResolvedPostHandoffEntryWriterContext,
        plan: &PostHandoffWriterPlan,
        destination: &mut [u8],
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        let invocation = plan.lower_reusable_fragment()?;
        if context.installed_evidence != InstalledCodeEvidence::from_installed(self)
            || context.installed_code != self.identity
            || context.artifact != self.artifact()
            || context.destination_site != destination_site
            || context.destination_len != destination.len()
            || context.invocation != invocation
            || context.context_abi() != POST_HANDOFF_WRITER_CONTEXT_ABI_V1
            || context.packed_words.first().copied() != Some(destination_site.base_address)
            || context.packed_words.len() != context.invocation.sources().len() + 1
        {
            return Err(MaterializationDiagnostic(
                "populated post-handoff writer context does not bind the exact installed code, plan, destination, and packed geometry"
                    .into(),
            ));
        }
        self.validate_post_handoff_entry_writer(plan, destination.len(), destination_site)?;
        plan.execute(destination, destination_site, |target| {
            context
                .invocation
                .sources()
                .iter()
                .zip(&context.packed_words[1..])
                .find_map(|(slot, value)| {
                    (slot.target == target
                        && slot.source == PostHandoffWriterSource::Resolve(target))
                    .then_some(*value)
                })
        })
    }

    fn validate_written_post_handoff_context(
        &self,
        context: &ResolvedPostHandoffEntryWriterContext,
        destination_site: PlacementSite,
        destination_len: usize,
    ) -> Result<(), InstallationDiagnostic> {
        context
            .invocation
            .validate_structure()
            .map_err(|diagnostic| InstallationDiagnostic(diagnostic.0))?;
        if context.installed_evidence != InstalledCodeEvidence::from_installed(self)
            || context.installed_code != self.identity
            || context.artifact != self.artifact()
            || context.destination_site != destination_site
            || context.destination_len != destination_len
            || context.context_abi() != POST_HANDOFF_WRITER_CONTEXT_ABI_V1
            || context.packed_words.first().copied() != Some(destination_site.base_address)
            || context.packed_words.len() != context.invocation.sources().len() + 1
        {
            return Err(InstallationDiagnostic(
                "written post-handoff destination does not retain its exact installed context, invocation, and destination geometry"
                    .into(),
            ));
        }
        let source_values = &context.packed_words[1..];
        context
            .invocation
            .validate_source_values(source_values)
            .map_err(|diagnostic| InstallationDiagnostic(diagnostic.0))?;
        for (slot, value) in context.invocation.sources().iter().zip(source_values) {
            let (exact, mismatch) = match slot.source {
                PostHandoffWriterSource::Resolve(target) => (
                    target == slot.target && self.resolve_entry_target(target) == Some(*value),
                    "is not an admitted entry in the exact installed artifact",
                ),
                PostHandoffWriterSource::Resolved(expected) => (
                    expected == *value && self.resolve_entry_target(slot.target) == Some(expected),
                    "does not match the exact installed realization",
                ),
            };
            if !exact {
                return Err(InstallationDiagnostic(format!(
                    "written post-handoff destination source slot for {:?} {mismatch}",
                    slot.target
                )));
            }
        }
        let replayed_fingerprint = non_authoritative_post_handoff_entry_writer_context_fingerprint(
            context.installed_code,
            context.artifact,
            context.destination_site,
            context.destination_len,
            &context.invocation,
            &context.packed_words,
        );
        if replayed_fingerprint != context.non_authoritative_fingerprint {
            return Err(InstallationDiagnostic(
                "written post-handoff destination context non-authoritative fingerprint fails exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Consume one exact activated, pinned, writable, unpublished destination
    /// and its non-clonable once-resolved context, then execute the writer into
    /// it. Failure returns both linear inputs; success retains the exact
    /// context with the still-unpublished destination for independent consumer
    /// replay before semantic validation and publication.
    pub fn write_prepared_post_handoff_destination<'mapping, 'bytes>(
        &self,
        context: ResolvedPostHandoffEntryWriterContext,
        plan: &PostHandoffWriterPlan,
        destination: ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) -> Result<
        WrittenPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<DestinationWriteError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.execute_populated_post_handoff_entry_writer(
            &context,
            plan,
            destination.destination.bytes,
            destination.destination.site,
        ) {
            return Err(Box::new(DestinationWriteError {
                context,
                destination,
                diagnostic,
            }));
        }
        let mut exact_produced_bytes = Vec::new();
        if exact_produced_bytes
            .try_reserve_exact(destination.destination.bytes.len())
            .is_err()
        {
            return Err(Box::new(DestinationWriteError {
                context,
                destination,
                diagnostic: MaterializationDiagnostic(
                    "cannot retain the exact successful writer output within resource limits"
                        .into(),
                ),
            }));
        }
        exact_produced_bytes.extend_from_slice(destination.destination.bytes);
        let PreparedPostHandoffWriterDestination {
            mapping,
            receipt,
            site,
            bytes,
        } = destination.destination;
        Ok(WrittenPostHandoffWriterDestination {
            mapping,
            receipt,
            site,
            bytes,
            exact_produced_bytes,
            context,
        })
    }

    /// Executes an atomic post-handoff writer using this installed code as the
    /// resolver authority for entry targets. Data symbols and entries from any
    /// other artifact fail before the destination is published.
    pub fn execute_post_handoff_entry_writer(
        &self,
        plan: &PostHandoffWriterPlan,
        destination: &mut [u8],
        destination_site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        self.validate_post_handoff_entry_writer(plan, destination.len(), destination_site)?;
        plan.execute(destination, destination_site, |target| {
            self.resolve_entry_target(target)
        })
    }

    fn resolve_entry_target(&self, target: RelocationTarget) -> Option<u64> {
        match target {
            RelocationTarget::Entry(identity) => self
                .validated
                .frozen
                .artifact
                .artifact
                .entry(identity)
                .and_then(|entry| {
                    self.validated
                        .frozen
                        .placement
                        .extent
                        .base()
                        .checked_add(entry.code_offset)
                }),
            RelocationTarget::Data(_) => None,
        }
    }

    fn contains_entry_target(&self, target: RelocationTarget) -> bool {
        match target {
            RelocationTarget::Entry(identity) => self
                .validated
                .frozen
                .artifact
                .artifact
                .entry(identity)
                .is_some(),
            RelocationTarget::Data(_) => false,
        }
    }
}

/// Opaque provider-private words for one checked post-handoff entry writer.
/// Word zero is the exact destination base and the remaining words are the
/// dense, first-occurrence-ordered source slots. The numeric words have no
/// public accessor and this carrier is deliberately non-clonable.
#[derive(PartialEq, Eq)]
pub struct ResolvedPostHandoffEntryWriterContext {
    installed_evidence: InstalledCodeEvidence,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination_site: PlacementSite,
    destination_len: usize,
    invocation: PostHandoffWriterInvocationPlan,
    packed_words: Vec<u64>,
    non_authoritative_fingerprint: NonAuthoritativeWriterContextFingerprint64,
}

/// Provider receipt establishing the runtime properties needed before a
/// generated writer may reach one activated mapping. The exact mapping
/// evidence is sealed in `mapping`; compact receipt identity is report-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationPreparationReceipt {
    identity: DestinationPreparationReceiptId,
    mapping: MappingReceiptContext,
    required_write_rights: ExtentRights,
    pinned: bool,
    unpublished: bool,
}

impl DestinationPreparationReceipt {
    pub fn from_admitted_provider(
        identity: DestinationPreparationReceiptId,
        mapping: &MappingReceiptContext,
        required_write_rights: ExtentRights,
        pinned: bool,
        unpublished: bool,
    ) -> Self {
        Self {
            identity,
            mapping: mapping.clone(),
            required_write_rights,
            pinned,
            unpublished,
        }
    }

    pub const fn identity(&self) -> DestinationPreparationReceiptId {
        self.identity
    }
}

/// Linear provider-side destination ready for one post-handoff writer.
///
/// `MappedExtent` establishes an activated translation and exact custody;
/// `receipt` establishes pinning and non-publication for that same mapping;
/// `required_write_rights` names the target/provider-defined write authority.
/// The byte slice is the provider's concrete mutable view and cannot outlive
/// this carrier. No source-language write-only view is implied.
#[derive(Debug)]
pub struct PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    mapping: MappedExtent<'mapping>,
    receipt: DestinationPreparationReceipt,
    site: PlacementSite,
    bytes: &'bytes mut [u8],
}

/// A prepared destination whose activated mapping, provider receipt, write
/// rights, pinning, unpublished state, placement, and byte geometry have been
/// replayed before symbolic-source resolution.
#[derive(Debug)]
#[must_use = "validated prepared destination retains mapping and byte custody"]
pub struct ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    destination: PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
}

#[derive(Debug)]
pub struct PreparedPostHandoffWriterDestinationValidationError<'mapping, 'bytes> {
    destination: PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: InstallationDiagnostic,
}

impl<'mapping, 'bytes> PreparedPostHandoffWriterDestinationValidationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_destination(self) -> PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
        self.destination
    }
}

impl<'mapping, 'bytes> PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    pub fn claim(
        mapping: MappedExtent<'mapping>,
        receipt: DestinationPreparationReceipt,
        site: PlacementSite,
        bytes: &'bytes mut [u8],
    ) -> Result<Self, Box<DestinationClaimError<'mapping, 'bytes>>> {
        if let Err(diagnostic) =
            validate_post_handoff_destination_binding(&mapping, &receipt, site, bytes.len())
        {
            return Err(Box::new(DestinationClaimError {
                mapping,
                receipt,
                site,
                bytes,
                diagnostic,
            }));
        }
        Ok(Self {
            mapping,
            receipt,
            site,
            bytes,
        })
    }

    pub const fn site(&self) -> PlacementSite {
        self.site
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Independently replay the exact activated mapping, provider receipt,
    /// placement, and byte-view geometry before a resolver observes symbolic
    /// source values. This borrows the carrier and grants no write or
    /// publication authority.
    pub fn validate_for_writer_preparation(&self) -> Result<(), InstallationDiagnostic> {
        validate_post_handoff_destination_binding(
            &self.mapping,
            &self.receipt,
            self.site,
            self.bytes.len(),
        )
    }

    /// Consume this destination into replayed custody before a resolver may
    /// observe symbolic sources. Rejection returns the exact raw destination.
    pub fn into_validated_for_writer_preparation(
        self,
    ) -> Result<
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<PreparedPostHandoffWriterDestinationValidationError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_for_writer_preparation() {
            return Err(Box::new(
                PreparedPostHandoffWriterDestinationValidationError {
                    destination: self,
                    diagnostic,
                },
            ));
        }
        Ok(ValidatedPreparedPostHandoffWriterDestination { destination: self })
    }
}

impl<'mapping, 'bytes> ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn site(&self) -> PlacementSite {
        self.destination.site
    }

    pub fn len(&self) -> usize {
        self.destination.bytes.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.destination.bytes.is_empty()
    }

    pub fn into_destination(self) -> PreparedPostHandoffWriterDestination<'mapping, 'bytes> {
        self.destination
    }
}

fn validate_post_handoff_destination_binding(
    mapping: &MappedExtent<'_>,
    receipt: &DestinationPreparationReceipt,
    site: PlacementSite,
    byte_len: usize,
) -> Result<(), InstallationDiagnostic> {
    let mismatch = if receipt.mapping != mapping.receipt_context() {
        Some("destination preparation receipt does not bind the exact activated mapping")
    } else if !receipt.pinned {
        Some("destination preparation receipt does not establish pinning")
    } else if !receipt.unpublished {
        Some("destination preparation receipt does not establish an unpublished destination")
    } else if receipt.required_write_rights.identities().next().is_none() {
        Some("destination preparation receipt names no writer right")
    } else if !mapping.rights().contains(&receipt.required_write_rights) {
        Some("activated destination mapping lacks required writer rights")
    } else if site.phase != layout_plans::PlacementPhase::PostHandoff {
        Some("prepared writer destination is not in the post-handoff placement phase")
    } else if site.base_address != mapping.base() {
        Some("prepared writer destination base does not match the activated mapping")
    } else if usize::try_from(mapping.length()).ok() != Some(byte_len) {
        Some("prepared writer byte view does not cover the exact activated mapping")
    } else {
        None
    };
    mismatch.map_or(Ok(()), |message| {
        Err(InstallationDiagnostic(message.into()))
    })
}

#[derive(Debug)]
pub struct DestinationClaimError<'mapping, 'bytes> {
    mapping: MappedExtent<'mapping>,
    receipt: DestinationPreparationReceipt,
    site: PlacementSite,
    bytes: &'bytes mut [u8],
    diagnostic: InstallationDiagnostic,
}

impl<'mapping, 'bytes> DestinationClaimError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        MappedExtent<'mapping>,
        DestinationPreparationReceipt,
        PlacementSite,
        &'bytes mut [u8],
    ) {
        (self.mapping, self.receipt, self.site, self.bytes)
    }
}

/// Exact destination after successful generated writing. It remains
/// unpublished and retains the activated mapping plus a hash-free exact copy
/// of the complete produced destination image for replay before
/// consumer-specific validation and eventual publication or recovery.
#[derive(Debug)]
pub struct WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    mapping: MappedExtent<'mapping>,
    receipt: DestinationPreparationReceipt,
    site: PlacementSite,
    bytes: &'bytes mut [u8],
    exact_produced_bytes: Vec<u8>,
    context: ResolvedPostHandoffEntryWriterContext,
}

/// A still-unpublished written destination whose exact installed context,
/// activated mapping, provider receipt, placement, and byte geometry have
/// been replayed before observation.
#[derive(Debug)]
#[must_use = "validated written destination retains mapping and byte custody"]
pub struct ValidatedWrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    written: WrittenPostHandoffWriterDestination<'mapping, 'bytes>,
}

/// Validation rejection preserves the complete written destination so its
/// exact retained evidence can be repaired and retried without reconstruction.
#[derive(Debug)]
pub struct WrittenPostHandoffWriterConsumerValidationError<'mapping, 'bytes> {
    written: WrittenPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: InstallationDiagnostic,
}

impl<'mapping, 'bytes> WrittenPostHandoffWriterConsumerValidationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &InstallationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(self) -> WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }

    pub fn into_prepared_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        let (context, destination) = self.written.into_prepared_parts();
        (
            context,
            ValidatedPreparedPostHandoffWriterDestination { destination },
        )
    }
}

impl<'mapping, 'bytes> WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.context.installed_code()
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.context.artifact()
    }

    pub const fn site(&self) -> PlacementSite {
        self.site
    }

    pub const fn non_authoritative_writer_context_fingerprint(
        &self,
    ) -> NonAuthoritativeWriterContextFingerprint64 {
        self.context.non_authoritative_fingerprint()
    }

    pub fn binds_invocation(&self, invocation: &PostHandoffWriterInvocationPlan) -> bool {
        self.context.binds_invocation(invocation)
    }

    pub const fn normalized_fragment_report_fingerprint(&self) -> u64 {
        self.context.normalized_fragment_report_fingerprint()
    }

    /// Independently replay the exact non-clonable writer context, destination
    /// preparation, and complete producer-captured byte image before an owning
    /// consumer validates semantic contents or publishes the mapping. This
    /// establishes no consumer value and performs no publication.
    pub fn validate_for_consumer(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), InstallationDiagnostic> {
        installed_code.validate_written_post_handoff_context(
            &self.context,
            self.site,
            self.bytes.len(),
        )?;
        validate_post_handoff_destination_binding(
            &self.mapping,
            &self.receipt,
            self.site,
            self.bytes.len(),
        )?;
        if self.bytes != self.exact_produced_bytes.as_slice() {
            return Err(InstallationDiagnostic(
                "written post-handoff destination bytes differ from the exact successful writer output"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Consume this still-unpublished destination only after exact replay.
    /// Rejection exposes no context or bytes and returns complete custody.
    pub fn into_validated_for_consumer(
        self,
        installed_code: &InstalledCode,
    ) -> Result<
        ValidatedWrittenPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<WrittenPostHandoffWriterConsumerValidationError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_for_consumer(installed_code) {
            return Err(Box::new(WrittenPostHandoffWriterConsumerValidationError {
                written: self,
                diagnostic,
            }));
        }
        Ok(ValidatedWrittenPostHandoffWriterDestination { written: self })
    }

    /// Recover the exact non-clonable context and still-unpublished prepared
    /// destination when a later consumer rejects validation. No byte or
    /// authority is reconstructed by this transition.
    fn into_prepared_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        PreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        let Self {
            mapping,
            receipt,
            site,
            bytes,
            exact_produced_bytes: _,
            context,
        } = self;
        (
            context,
            PreparedPostHandoffWriterDestination {
                mapping,
                receipt,
                site,
                bytes,
            },
        )
    }

    fn into_parts(
        self,
    ) -> (
        MappedExtent<'mapping>,
        DestinationPreparationReceipt,
        PlacementSite,
        &'bytes mut [u8],
    ) {
        (self.mapping, self.receipt, self.site, self.bytes)
    }
}

impl<'mapping, 'bytes> ValidatedWrittenPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.written.installed_code()
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.written.artifact()
    }

    pub const fn site(&self) -> PlacementSite {
        self.written.site()
    }

    pub const fn non_authoritative_writer_context_fingerprint(
        &self,
    ) -> NonAuthoritativeWriterContextFingerprint64 {
        self.written.non_authoritative_writer_context_fingerprint()
    }

    pub fn binds_invocation(&self, invocation: &PostHandoffWriterInvocationPlan) -> bool {
        self.written.binds_invocation(invocation)
    }

    pub const fn normalized_fragment_report_fingerprint(&self) -> u64 {
        self.written.normalized_fragment_report_fingerprint()
    }

    /// Replay the installed realization and destination preparation without
    /// downgrading this already-validated custody carrier.
    pub fn validate_for_consumer(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), InstallationDiagnostic> {
        self.written.validate_for_consumer(installed_code)
    }

    pub const fn context(&self) -> &ResolvedPostHandoffEntryWriterContext {
        &self.written.context
    }

    /// Bytes remain unpublished; this is observation after exact replay.
    pub fn bytes(&self) -> &[u8] {
        self.written.bytes
    }

    pub fn into_written(self) -> WrittenPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }

    pub fn into_prepared_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        let (context, destination) = self.written.into_prepared_parts();
        (
            context,
            ValidatedPreparedPostHandoffWriterDestination { destination },
        )
    }

    pub fn into_parts(
        self,
    ) -> (
        MappedExtent<'mapping>,
        DestinationPreparationReceipt,
        PlacementSite,
        &'bytes mut [u8],
    ) {
        self.written.into_parts()
    }
}

#[derive(Debug)]
pub struct DestinationWriteError<'mapping, 'bytes> {
    context: ResolvedPostHandoffEntryWriterContext,
    destination: ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: MaterializationDiagnostic,
}

impl<'mapping, 'bytes> DestinationWriteError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ResolvedPostHandoffEntryWriterContext,
        ValidatedPreparedPostHandoffWriterDestination<'mapping, 'bytes>,
    ) {
        (self.context, self.destination)
    }
}

impl std::fmt::Debug for ResolvedPostHandoffEntryWriterContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedPostHandoffEntryWriterContext")
            .field("installed_code", &self.installed_code)
            .field("artifact", &self.artifact)
            .field("destination_len", &self.destination_len)
            .field("source_slot_count", &self.invocation.sources().len())
            .field(
                "normalized_fragment_report_fingerprint",
                &format_args!("{:016x}", self.invocation.fragment().report_fingerprint()),
            )
            .field(
                "non_authoritative_fingerprint",
                &format_args!(
                    "{:016x}",
                    self.non_authoritative_fingerprint.compatibility_value()
                ),
            )
            .finish()
    }
}

impl ResolvedPostHandoffEntryWriterContext {
    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn source_slot_count(&self) -> usize {
        self.invocation.source_slot_count()
    }

    pub const fn packed_byte_len(&self) -> usize {
        self.packed_words.len() * std::mem::size_of::<u64>()
    }

    pub const fn non_authoritative_fingerprint(
        &self,
    ) -> NonAuthoritativeWriterContextFingerprint64 {
        self.non_authoritative_fingerprint
    }

    pub const fn context_abi(&self) -> u64 {
        self.invocation.fragment().context_abi()
    }

    pub const fn normalized_fragment_report_fingerprint(&self) -> u64 {
        self.invocation.fragment().report_fingerprint()
    }

    /// Report whether this opaque, once-resolved context is the invocation
    /// sibling of one exact reusable fragment plan. Numeric packed words remain
    /// inaccessible.
    pub fn binds_invocation(&self, invocation: &PostHandoffWriterInvocationPlan) -> bool {
        self.invocation == *invocation
    }

    /// Replay this sealed context against the exact installed realization and
    /// destination geometry without resolving or exposing its numeric words.
    pub fn validate_for_destination(
        &self,
        installed_code: &InstalledCode,
        destination_site: PlacementSite,
        destination_len: usize,
    ) -> Result<(), InstallationDiagnostic> {
        installed_code.validate_written_post_handoff_context(
            self,
            destination_site,
            destination_len,
        )
    }
}

fn non_authoritative_post_handoff_entry_writer_context_fingerprint(
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination_site: PlacementSite,
    destination_len: usize,
    invocation: &PostHandoffWriterInvocationPlan,
    packed_words: &[u64],
) -> NonAuthoritativeWriterContextFingerprint64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(installed_code.normalized_identity());
    mix(artifact.normalized_identity());
    mix(destination_site.base_address);
    mix(destination_len as u64);
    mix(invocation.fragment().report_fingerprint());
    mix(invocation.sources().len() as u64);
    for PostHandoffWriterSourceSlot { target, source } in invocation.sources() {
        match target {
            RelocationTarget::Entry(entry) => {
                mix(1);
                mix(entry.normalized_identity());
            }
            RelocationTarget::Data(data) => {
                mix(2);
                mix(data.normalized_identity());
            }
        }
        match source {
            PostHandoffWriterSource::Resolved(_) => mix(3),
            PostHandoffWriterSource::Resolve(RelocationTarget::Entry(entry)) => {
                mix(4);
                mix(entry.normalized_identity());
            }
            PostHandoffWriterSource::Resolve(RelocationTarget::Data(data)) => {
                mix(5);
                mix(data.normalized_identity());
            }
        }
    }
    mix(invocation.fit_constraints().len() as u64);
    for constraint in invocation.fit_constraints() {
        mix(constraint.source_slot as u64);
        mix(constraint.fit.source_width_bits.into());
        mix(constraint.fit.stored_width_bits.into());
        mix(match constraint.fit.interpretation {
            layout_plans::IntegerInterpretation::Signed => 1,
            layout_plans::IntegerInterpretation::Unsigned => 2,
        });
        for byte in constraint.field.as_bytes() {
            mix(u64::from(*byte));
        }
        mix(0xff);
    }
    mix(packed_words.len() as u64);
    for word in packed_words {
        mix(*word);
    }
    NonAuthoritativeWriterContextFingerprint64::from_compatibility_value(if hash == 0 {
        0xcbf2_9ce4_8422_2325
    } else {
        hash
    })
    .expect("fixed FNV normalization replaces zero")
}

#[cfg(test)]
mod tests;

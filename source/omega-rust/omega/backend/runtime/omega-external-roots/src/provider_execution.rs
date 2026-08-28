//! Admitted provider execution and post-handoff writer custody.
//!
//! This module owns opaque-exit assurance, exact provider execution identity,
//! and the prepared, written, consumer-validated, and retryable writer
//! transitions. It does not validate candidate root policy, publish roots, or
//! execute interrupt lifecycle operations.

use std::collections::BTreeSet;

use omega_calling_conventions::{ProviderExitRealization, validate_provider_exit_realization};
use omega_executable_installation::{InstalledCode, ResolvedPostHandoffEntryWriterContext};
use psi_layout_plans::{
    EntryStubId, PlacementSite, PostHandoffWriterInvocationPlan, PostHandoffWriterPlan,
    RelocationTarget,
};

use super::{
    ExternalRootDiagnostic, ExternalRootEntryClaim, ExternalRootId, FixedFuelLocalEvidence, Fnv1a,
    ProviderExecutionId, ProviderPlanId, RootEffectId, RootProviderId, StackLocalEvidence,
    StateValidationReceiptId, TrustReceiptId, ValidatedExternalRoot, validate_external_root,
    validate_installed_entry_fuel, validate_installed_entry_stack,
};

/// Evidence that an opaque provider cannot escape the boundary's admitted
/// exit contract. An accepted claim is checked against the exact normalized
/// `CallPlan + StatePlan`; adequate hardware isolation is the explicit
/// alternative when the provider's exit is not inspectable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueProviderExitAssurance {
    AcceptedClaim {
        realization: ProviderExitRealization,
        validation_receipt: TrustReceiptId,
    },
    HardwareIsolation {
        validation_receipt: TrustReceiptId,
    },
}

impl OpaqueProviderExitAssurance {
    fn validate(self, root: &ValidatedExternalRoot) -> Result<Self, ExternalRootDiagnostic> {
        let validation_receipt = match self {
            Self::AcceptedClaim {
                validation_receipt, ..
            }
            | Self::HardwareIsolation { validation_receipt } => validation_receipt,
        };
        if !root.candidate.trust_receipts.contains(&validation_receipt) {
            return Err(ExternalRootDiagnostic(
                "opaque provider exit assurance is absent from the root's admitted trust receipts"
                    .into(),
            ));
        }
        if let Self::AcceptedClaim { realization, .. } = self {
            validate_provider_exit_realization(root.boundary.plan(), &realization).map_err(
                |error| {
                    ExternalRootDiagnostic(format!(
                        "opaque provider exit claim violates the admitted boundary: {error}"
                    ))
                },
            )?;
        }
        Ok(self)
    }

    fn fingerprint(self) -> u64 {
        let mut hash = Fnv1a::new();
        match self {
            Self::AcceptedClaim {
                validation_receipt, ..
            } => {
                hash.u64(0);
                hash.u64(validation_receipt.normalized_identity());
            }
            Self::HardwareIsolation { validation_receipt } => {
                hash.u64(1);
                hash.u64(validation_receipt.normalized_identity());
            }
        }
        hash.finish()
    }
}

/// Admitted execution binding for one exact external-root realization.
///
/// This does not fuse the stack, logical-fuel, and machine-state algebras.
/// It binds their independently validated results, the selected normalized
/// provider plan, and the executable entry into one provider execution that a
/// root admission may publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderExecution {
    pub(super) identity: ProviderExecutionId,
    pub(super) root_evidence: ValidatedExternalRoot,
    pub(super) provider_plan: ProviderPlanId,
    pub(super) root: ExternalRootId,
    pub(super) normalized_root_identity: u64,
    pub(super) provider: RootProviderId,
    pub(super) entry: EntryStubId,
    pub(super) boundary_contract_fingerprint: u64,
    pub(super) stack_artifact_composition_fingerprint: u64,
    pub(super) stack_demand_fingerprint: u64,
    pub(super) logical_fuel_fingerprint: u64,
    pub(super) machine_state_validation_receipt: StateValidationReceiptId,
    pub(super) exit_assurance: OpaqueProviderExitAssurance,
    pub(super) exit_assurance_fingerprint: u64,
    pub(super) effects: BTreeSet<RootEffectId>,
    pub(super) normalized_identity: u64,
}

/// Non-constructible evidence that the external-root ledger admitted one exact
/// provider execution. Terminal lowering may borrow or retain this value; wire
/// formats record its fields but cannot recreate executable authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdmittedProviderExecution {
    provider_plan: u64,
    provider_execution_identity: u64,
    provider_execution_fingerprint: u64,
    normalized_root_identity: u64,
    boundary_contract_fingerprint: u64,
}

/// One exact provider execution joined to the installed artifact resolver and
/// the provider-private input for a post-handoff entry writer.
///
/// Construction matches the selected provider-plan identity, retains the
/// execution that already sealed stack/fuel/state root evidence, and rechecks
/// the installed entry, writer's exact symbolic target set, and concrete
/// destination placement. The packed numeric context remains opaque and
/// non-clonable.
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedExternalRootPostHandoffWriterInvocation {
    pub(super) provider_execution: AdmittedProviderExecution,
    pub(super) provider_execution_evidence: ProviderExecution,
    pub(super) root_evidence: ValidatedExternalRoot,
    pub(super) selected_entry: EntryStubId,
    pub(super) selected_entry_source_slot: usize,
    pub(super) architecture: omega_target::Architecture,
    pub(super) invocation: PostHandoffWriterInvocationPlan,
    pub(super) writer: PostHandoffWriterPlan,
    pub(super) context: ResolvedPostHandoffEntryWriterContext,
}

/// Still-unpublished destination retaining the exact selected external-root
/// execution and writer preparation that produced its bytes. The provider
/// evidence is not reduced to copied report identities, and the installation-
/// owned destination remains in its consuming validated typestate rather than
/// being downgraded after replay. This outer carrier still exposes no bytes and
/// does not establish consumer semantics or publication authority.
#[derive(Debug)]
#[must_use = "written external-root destination retains provider and mapping custody"]
pub struct WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    provider_execution: AdmittedProviderExecution,
    provider_execution_evidence: ProviderExecution,
    root_evidence: ValidatedExternalRoot,
    selected_entry: EntryStubId,
    selected_entry_source_slot: usize,
    architecture: omega_target::Architecture,
    invocation: PostHandoffWriterInvocationPlan,
    writer: PostHandoffWriterPlan,
    written: omega_executable_installation::ValidatedWrittenPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
}

/// A written external-root destination whose provider, root, invocation,
/// installation, mapping, and destination evidence has been replayed before
/// its still-unpublished bytes become observable.
#[derive(Debug)]
#[must_use = "validated written external-root destination retains provider and mapping custody"]
pub struct ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    provider_execution: AdmittedProviderExecution,
    provider_execution_evidence: ProviderExecution,
    root_evidence: ValidatedExternalRoot,
    selected_entry: EntryStubId,
    selected_entry_source_slot: usize,
    architecture: omega_target::Architecture,
    invocation: PostHandoffWriterInvocationPlan,
    writer: PostHandoffWriterPlan,
    written: omega_executable_installation::ValidatedWrittenPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
}

/// Consumer-validation rejection returns the complete written carrier for a
/// corrected installed-realization retry without exposing destination bytes.
#[derive(Debug)]
pub struct WrittenExternalRootConsumerValidationError<'mapping, 'bytes> {
    written: WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'mapping, 'bytes> WrittenExternalRootConsumerValidationError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(self) -> WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }
}

/// Failed recovery of a still-unpublished external-root writer destination.
/// The complete written carrier is returned unchanged so the owning consumer
/// can correct the installed-code input or choose another recovery path.
#[derive(Debug)]
pub struct WrittenExternalRootWriterRecoveryError<'mapping, 'bytes> {
    written: WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'mapping, 'bytes> WrittenExternalRootWriterRecoveryError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_written(self) -> WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
        self.written
    }
}

#[derive(Debug)]
pub struct PreparedExternalRootWriterExecutionError<'mapping, 'bytes> {
    prepared: PreparedExternalRootPostHandoffWriterInvocation,
    destination: omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
        'mapping,
        'bytes,
    >,
    diagnostic: psi_layout_plans::MaterializationDiagnostic,
}

impl<'mapping, 'bytes> PreparedExternalRootWriterExecutionError<'mapping, 'bytes> {
    pub const fn diagnostic(&self) -> &psi_layout_plans::MaterializationDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PreparedExternalRootPostHandoffWriterInvocation,
        omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) {
        (self.prepared, self.destination)
    }
}

impl PreparedExternalRootPostHandoffWriterInvocation {
    pub(super) fn validate_execution(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
        self.invocation.validate_structure()?;
        let replayed_invocation = self.writer.lower_reusable_fragment()?;
        if replayed_invocation != self.invocation {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer no longer matches its retained invocation".into(),
            ));
        }
        validate_external_root_writer_source(
            &self.provider_execution_evidence,
            &self.root_evidence,
            self.provider_execution,
            &self.invocation,
            self.selected_entry,
            self.selected_entry_source_slot,
        )?;
        if !self.context.binds_invocation(&self.invocation) {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer context no longer binds its retained invocation"
                    .into(),
            ));
        }
        if installed_code.identity() != self.context.installed_code()
            || installed_code.artifact() != self.context.artifact()
        {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer does not bind the exact installed artifact".into(),
            ));
        }
        if installed_code.architecture() != self.architecture {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "prepared external-root writer architecture does not match the exact installed artifact"
                    .into(),
            ));
        }
        Ok(())
    }

    pub const fn provider_execution(&self) -> AdmittedProviderExecution {
        self.provider_execution
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.selected_entry
    }

    pub const fn selected_entry_source_slot(&self) -> usize {
        self.selected_entry_source_slot
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn selected_boundary_parameter_count(&self) -> usize {
        self.root_evidence.boundary.plan().call.parameters.len()
    }

    pub const fn selected_boundary_contract_fingerprint(&self) -> u64 {
        self.root_evidence.boundary_contract_fingerprint
    }

    pub fn selected_entry_claims(&self) -> &[ExternalRootEntryClaim] {
        &self.root_evidence.candidate.entry_claims
    }

    pub const fn architecture(&self) -> omega_target::Architecture {
        self.architecture
    }

    pub const fn invocation(&self) -> &PostHandoffWriterInvocationPlan {
        &self.invocation
    }

    pub const fn context(&self) -> &ResolvedPostHandoffEntryWriterContext {
        &self.context
    }

    /// Consume one exact provider-prepared destination through the installed
    /// artifact resolver used during preparation. The successful result is
    /// still unpublished; consumer-specific validation and publication remain
    /// separate transitions.
    pub fn execute<'mapping, 'bytes>(
        self,
        installed_code: &InstalledCode,
        destination: omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) -> Result<
        WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<PreparedExternalRootWriterExecutionError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_execution(installed_code) {
            return Err(Box::new(PreparedExternalRootWriterExecutionError {
                prepared: self,
                destination,
                diagnostic,
            }));
        }
        if let Err(diagnostic) = self.context.validate_for_destination(
            installed_code,
            destination.site(),
            destination.len(),
        ) {
            return Err(Box::new(PreparedExternalRootWriterExecutionError {
                prepared: self,
                destination,
                diagnostic: psi_layout_plans::MaterializationDiagnostic(diagnostic.0),
            }));
        }
        let Self {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            context,
        } = self;
        match installed_code.write_prepared_post_handoff_destination(context, &writer, destination)
        {
            Ok(written) => {
                let written = match written.into_validated_for_consumer(installed_code) {
                    Ok(written) => written,
                    Err(error) => {
                        let diagnostic = psi_layout_plans::MaterializationDiagnostic(
                            error.diagnostic().0.clone(),
                        );
                        let (context, destination) = (*error).into_prepared_parts();
                        return Err(Box::new(PreparedExternalRootWriterExecutionError {
                            prepared: Self {
                                provider_execution,
                                provider_execution_evidence,
                                root_evidence,
                                selected_entry,
                                selected_entry_source_slot,
                                architecture,
                                invocation,
                                writer,
                                context,
                            },
                            destination,
                            diagnostic,
                        }));
                    }
                };
                Ok(WrittenExternalRootPostHandoffWriterDestination {
                    provider_execution,
                    provider_execution_evidence,
                    root_evidence,
                    selected_entry,
                    selected_entry_source_slot,
                    architecture,
                    invocation,
                    writer,
                    written,
                })
            }
            Err(destination_error) => {
                let diagnostic = destination_error.diagnostic().clone();
                let (context, destination) = (*destination_error).into_parts();
                Err(Box::new(PreparedExternalRootWriterExecutionError {
                    prepared: Self {
                        provider_execution,
                        provider_execution_evidence,
                        root_evidence,
                        selected_entry,
                        selected_entry_source_slot,
                        architecture,
                        invocation,
                        writer,
                        context,
                    },
                    destination,
                    diagnostic,
                }))
            }
        }
    }
}

impl<'mapping, 'bytes> WrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    pub const fn provider_execution(&self) -> AdmittedProviderExecution {
        self.provider_execution
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.selected_entry
    }

    pub const fn selected_entry_source_slot(&self) -> usize {
        self.selected_entry_source_slot
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn selected_boundary_parameter_count(&self) -> usize {
        self.root_evidence.boundary.plan().call.parameters.len()
    }

    pub const fn selected_boundary_contract_fingerprint(&self) -> u64 {
        self.root_evidence.boundary_contract_fingerprint
    }

    pub fn selected_entry_claims(&self) -> &[ExternalRootEntryClaim] {
        &self.root_evidence.candidate.entry_claims
    }

    pub const fn architecture(&self) -> omega_target::Architecture {
        self.architecture
    }

    pub const fn invocation(&self) -> &PostHandoffWriterInvocationPlan {
        &self.invocation
    }

    /// Independently replay provider preparation, invocation structure, and
    /// the installation-owned context. Rejection only borrows this carrier so
    /// the exact provider and destination inputs remain available for retry.
    pub fn validate_for_consumer(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
        self.invocation.validate_structure()?;
        let replayed_invocation = self.writer.lower_reusable_fragment()?;
        if replayed_invocation != self.invocation {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "written external-root destination does not retain its exact provider preparation and invocation"
                    .into(),
            ));
        }
        validate_external_root_writer_source(
            &self.provider_execution_evidence,
            &self.root_evidence,
            self.provider_execution,
            &self.invocation,
            self.selected_entry,
            self.selected_entry_source_slot,
        )?;
        if self.architecture != installed_code.architecture()
            || !self.written.binds_invocation(&self.invocation)
            || self.written.normalized_fragment_fingerprint()
                != self.invocation.fragment().fingerprint()
        {
            return Err(psi_layout_plans::MaterializationDiagnostic(
                "written external-root destination does not retain its exact provider preparation and invocation"
                    .into(),
            ));
        }
        self.written
            .validate_for_consumer(installed_code)
            .map_err(|diagnostic| psi_layout_plans::MaterializationDiagnostic(diagnostic.0))
    }

    /// Consume this carrier only after replaying its complete retained context
    /// against the consumer's installed realization. Rejection returns the
    /// original carrier and exposes no destination bytes.
    pub fn into_validated_for_consumer(
        self,
        installed_code: &InstalledCode,
    ) -> Result<
        ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes>,
        Box<WrittenExternalRootConsumerValidationError<'mapping, 'bytes>>,
    > {
        if let Err(diagnostic) = self.validate_for_consumer(installed_code) {
            return Err(Box::new(WrittenExternalRootConsumerValidationError {
                written: self,
                diagnostic,
            }));
        }
        let Self {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            written,
        } = self;
        Ok(ValidatedWrittenExternalRootPostHandoffWriterDestination {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            written,
        })
    }

    /// Return this still-unpublished destination to the exact provider-writer
    /// preparation state from which it can be executed again. Recovery first
    /// replays the complete provider, invocation, installation, mapping, and
    /// destination binding; rejection preserves this written carrier intact.
    /// Success does not restore old bytes, validate consumer semantics, or
    /// publish the destination.
    pub fn recover_for_retry(
        self,
        installed_code: &InstalledCode,
    ) -> Result<
        (
            PreparedExternalRootPostHandoffWriterInvocation,
            omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
                'mapping,
                'bytes,
            >,
        ),
        Box<WrittenExternalRootWriterRecoveryError<'mapping, 'bytes>>,
    > {
        match self.into_validated_for_consumer(installed_code) {
            Ok(written) => written.recover_for_retry(),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                Err(Box::new(WrittenExternalRootWriterRecoveryError {
                    written: (*error).into_written(),
                    diagnostic,
                }))
            }
        }
    }
}

impl<'mapping, 'bytes> ValidatedWrittenExternalRootPostHandoffWriterDestination<'mapping, 'bytes> {
    /// Bytes remain unpublished; this is observation after exact replay, not
    /// consumer semantic validation or publication.
    pub fn bytes(&self) -> &[u8] {
        self.written.bytes()
    }

    pub const fn provider_execution(&self) -> AdmittedProviderExecution {
        self.provider_execution
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.selected_entry
    }

    pub const fn selected_entry_source_slot(&self) -> usize {
        self.selected_entry_source_slot
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn recover_for_retry(
        self,
    ) -> Result<
        (
            PreparedExternalRootPostHandoffWriterInvocation,
            omega_executable_installation::ValidatedPreparedPostHandoffWriterDestination<
                'mapping,
                'bytes,
            >,
        ),
        Box<WrittenExternalRootWriterRecoveryError<'mapping, 'bytes>>,
    > {
        let Self {
            provider_execution,
            provider_execution_evidence,
            root_evidence,
            selected_entry,
            selected_entry_source_slot,
            architecture,
            invocation,
            writer,
            written,
        } = self;
        let (context, destination) = written.into_prepared_parts();
        Ok((
            PreparedExternalRootPostHandoffWriterInvocation {
                provider_execution,
                provider_execution_evidence,
                root_evidence,
                selected_entry,
                selected_entry_source_slot,
                architecture,
                invocation,
                writer,
                context,
            },
            destination,
        ))
    }

    pub fn into_parts(
        self,
    ) -> (
        AdmittedProviderExecution,
        ProviderExecution,
        ValidatedExternalRoot,
        EntryStubId,
        usize,
        omega_target::Architecture,
        PostHandoffWriterInvocationPlan,
        PostHandoffWriterPlan,
        omega_executable_installation::ValidatedWrittenPostHandoffWriterDestination<
            'mapping,
            'bytes,
        >,
    ) {
        (
            self.provider_execution,
            self.provider_execution_evidence,
            self.root_evidence,
            self.selected_entry,
            self.selected_entry_source_slot,
            self.architecture,
            self.invocation,
            self.writer,
            self.written,
        )
    }
}

fn selected_entry_source_slot(
    invocation: &PostHandoffWriterInvocationPlan,
    selected_entry: EntryStubId,
) -> Result<usize, psi_layout_plans::MaterializationDiagnostic> {
    invocation.validate_structure()?;
    let target = RelocationTarget::Entry(selected_entry);
    let mut matches = invocation
        .sources()
        .iter()
        .enumerate()
        .filter(|(_, source)| source.target == target);
    let Some((source_slot, source)) = matches.next() else {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer does not contain the admitted external-root entry".into(),
        ));
    };
    if matches.next().is_some() {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer repeats the admitted external-root entry source".into(),
        ));
    }
    if source.source != psi_layout_plans::PostHandoffWriterSource::Resolve(target) {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer must resolve the admitted external-root entry through its sealed provider context"
                .into(),
        ));
    }
    Ok(source_slot)
}

fn validate_selected_entry_source(
    invocation: &PostHandoffWriterInvocationPlan,
    selected_entry: EntryStubId,
    retained_source_slot: usize,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    let replayed_source_slot = selected_entry_source_slot(invocation, selected_entry)?;
    if replayed_source_slot != retained_source_slot {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer selected-entry source-slot correspondence does not match its retained preparation"
                .into(),
        ));
    }
    Ok(())
}

fn validate_external_root_writer_source(
    provider_execution_evidence: &ProviderExecution,
    root_evidence: &ValidatedExternalRoot,
    provider_execution: AdmittedProviderExecution,
    invocation: &PostHandoffWriterInvocationPlan,
    selected_entry: EntryStubId,
    retained_source_slot: usize,
) -> Result<(), psi_layout_plans::MaterializationDiagnostic> {
    if !provider_execution_evidence.matches_root(root_evidence)
        || provider_execution_evidence.binding() != provider_execution
        || root_evidence.candidate.entry != selected_entry
        || root_evidence.candidate.provider_plan.normalized_identity()
            != provider_execution.provider_plan
        || root_evidence.normalized_identity != provider_execution.normalized_root_identity
        || root_evidence.boundary_contract_fingerprint
            != provider_execution.boundary_contract_fingerprint
    {
        return Err(psi_layout_plans::MaterializationDiagnostic(
            "post-handoff writer source does not retain its exact validated external-root requirement and provider execution"
                .into(),
        ));
    }
    validate_selected_entry_source(invocation, selected_entry, retained_source_slot)
}

impl AdmittedProviderExecution {
    pub const fn provider_plan(&self) -> u64 {
        self.provider_plan
    }

    pub const fn provider_execution_identity(&self) -> u64 {
        self.provider_execution_identity
    }

    pub const fn provider_execution_fingerprint(&self) -> u64 {
        self.provider_execution_fingerprint
    }

    pub const fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_identity
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

impl omega_installation_evidence::ProviderExecutionEvidence for ProviderExecution {
    fn requirement_identity(&self) -> &str {
        self.selected_requirement_identity()
    }

    fn provider_plan(&self) -> u64 {
        self.provider_plan.normalized_identity()
    }

    fn provider_execution_identity(&self) -> u64 {
        self.identity.normalized_identity()
    }

    fn provider_execution_fingerprint(&self) -> u64 {
        self.normalized_identity
    }

    fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_identity
    }

    fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

fn fingerprint_provider_execution(
    identity: ProviderExecutionId,
    root: &ValidatedExternalRoot,
    exit_assurance_fingerprint: u64,
) -> u64 {
    let candidate = root.candidate();
    let mut hash = Fnv1a::new();
    hash.u64(identity.normalized_identity());
    hash.u64(candidate.provider_plan.normalized_identity());
    hash.u64(candidate.identity.normalized_identity());
    hash.u64(root.normalized_identity());
    hash.u64(candidate.provider.normalized_identity());
    hash.u64(candidate.entry.normalized_identity());
    hash.u64(root.boundary_contract_fingerprint());
    hash.u64(candidate.stack.realization.composition().fingerprint());
    hash.u64(candidate.stack.realization.fingerprint());
    hash.u64(candidate.logical_fuel.realization.composition_fingerprint());
    hash.u64(
        candidate
            .machine_state
            .validation_receipt
            .normalized_identity(),
    );
    hash.u64(exit_assurance_fingerprint);
    for effect in &candidate.effects {
        hash.u64(effect.normalized_identity());
    }
    hash.finish()
}

impl ProviderExecution {
    /// Provider/trust admission creates this binding only after selecting the
    /// exact provider plan that will execute the validated root.
    pub fn from_admitted_provider(
        identity: ProviderExecutionId,
        root: &ValidatedExternalRoot,
        exit_assurance: Option<OpaqueProviderExitAssurance>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let exit_assurance = exit_assurance
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "opaque provider requires an accepted exit claim or adequate hardware isolation"
                        .into(),
                )
            })?
            .validate(root)?;
        let exit_assurance_fingerprint = exit_assurance.fingerprint();
        let candidate = root.candidate();
        let normalized_identity =
            fingerprint_provider_execution(identity, root, exit_assurance_fingerprint);
        Ok(Self {
            identity,
            root_evidence: root.clone(),
            provider_plan: candidate.provider_plan,
            root: candidate.identity,
            normalized_root_identity: root.normalized_identity(),
            provider: candidate.provider,
            entry: candidate.entry,
            boundary_contract_fingerprint: root.boundary_contract_fingerprint(),
            stack_artifact_composition_fingerprint: candidate
                .stack
                .realization
                .composition()
                .fingerprint(),
            stack_demand_fingerprint: candidate.stack.realization.fingerprint(),
            logical_fuel_fingerprint: candidate.logical_fuel.realization.composition_fingerprint(),
            machine_state_validation_receipt: candidate.machine_state.validation_receipt,
            exit_assurance,
            exit_assurance_fingerprint,
            effects: candidate.effects.clone(),
            normalized_identity,
        })
    }

    pub const fn identity(&self) -> ProviderExecutionId {
        self.identity
    }

    pub const fn provider_plan(&self) -> ProviderPlanId {
        self.provider_plan
    }

    pub const fn selected_entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
    }

    pub fn selected_requirement_identity(&self) -> &str {
        &self.root_evidence.candidate.requirement_identity
    }

    pub fn selected_boundary_parameter_count(&self) -> usize {
        self.root_evidence.boundary.plan().call.parameters.len()
    }

    pub const fn selected_boundary_contract_fingerprint(&self) -> u64 {
        self.root_evidence.boundary_contract_fingerprint
    }

    pub fn selected_entry_claims(&self) -> &[ExternalRootEntryClaim] {
        &self.root_evidence.candidate.entry_claims
    }

    /// Export the exact admitted execution evidence consumed by the clean
    /// terminal-Psi native lane. Lowering does not accept a second provider
    /// plan choice: this binding inherits the plan selected by root admission.
    pub const fn binding(&self) -> AdmittedProviderExecution {
        AdmittedProviderExecution {
            provider_plan: self.provider_plan.normalized_identity(),
            provider_execution_identity: self.identity.normalized_identity(),
            provider_execution_fingerprint: self.normalized_identity,
            normalized_root_identity: self.normalized_root_identity,
            boundary_contract_fingerprint: self.boundary_contract_fingerprint,
        }
    }

    /// Independently replay the complete admitted root, execution-to-root
    /// binding, exit assurance, and normalized execution identity before an
    /// installed resolver observes symbolic writer sources.
    pub fn validate_for_writer_preparation(&self) -> Result<(), ExternalRootDiagnostic> {
        let replayed_root = validate_external_root(
            self.root_evidence.candidate.clone(),
            &self.root_evidence.boundary,
        )?;
        if replayed_root != self.root_evidence || !self.matches_root(&replayed_root) {
            return Err(ExternalRootDiagnostic(
                "post-handoff writer provider execution does not retain its exact validated root evidence"
                    .into(),
            ));
        }
        self.exit_assurance.validate(&replayed_root)?;
        let exit_assurance_fingerprint = self.exit_assurance.fingerprint();
        if exit_assurance_fingerprint != self.exit_assurance_fingerprint
            || fingerprint_provider_execution(
                self.identity,
                &replayed_root,
                exit_assurance_fingerprint,
            ) != self.normalized_identity
        {
            return Err(ExternalRootDiagnostic(
                "post-handoff writer provider execution identity fails exact structural replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Prepare one post-handoff writer invocation only when it contains this
    /// execution's exact selected entry and is resolved by the same installed
    /// artifact used by the root's terminal fixed-fuel evidence.
    ///
    /// The plan ID is an identity comparison, not authority. The compiler
    /// orchestration wrapper supplies its retained selection so a later stage
    /// cannot accidentally substitute a different closure after root
    /// admission.
    pub fn prepare_post_handoff_entry_writer(
        &self,
        selected_provider_plan: ProviderPlanId,
        installed_code: &InstalledCode,
        writer: &PostHandoffWriterPlan,
        destination_len: usize,
        destination_site: PlacementSite,
    ) -> Result<PreparedExternalRootPostHandoffWriterInvocation, ExternalRootDiagnostic> {
        if selected_provider_plan != self.provider_plan {
            return Err(ExternalRootDiagnostic(
                "post-handoff writer selected provider plan does not match the admitted provider execution"
                    .into(),
            ));
        }
        self.validate_for_writer_preparation()?;
        self.validate_installed_entry_binding(installed_code)?;

        let invocation = writer
            .lower_reusable_fragment()
            .map_err(|error| ExternalRootDiagnostic(error.0))?;
        let selected_entry_source_slot = selected_entry_source_slot(&invocation, self.entry)
            .map_err(|diagnostic| ExternalRootDiagnostic(diagnostic.0))?;

        let context = installed_code
            .populate_post_handoff_entry_writer_context(writer, destination_len, destination_site)
            .map_err(|error| ExternalRootDiagnostic(error.0))?;
        if !context.binds_invocation(&invocation) {
            return Err(ExternalRootDiagnostic(
                "installed artifact resolver context does not bind the exact post-handoff writer invocation"
                    .into(),
            ));
        }
        Ok(PreparedExternalRootPostHandoffWriterInvocation {
            provider_execution: self.binding(),
            provider_execution_evidence: self.clone(),
            root_evidence: self.root_evidence.clone(),
            selected_entry: self.entry,
            selected_entry_source_slot,
            architecture: installed_code.architecture(),
            invocation,
            writer: writer.clone(),
            context,
        })
    }

    pub const fn exit_assurance(&self) -> OpaqueProviderExitAssurance {
        self.exit_assurance
    }

    pub const fn exit_assurance_fingerprint(&self) -> u64 {
        self.exit_assurance_fingerprint
    }

    pub(super) fn matches_root(&self, root: &ValidatedExternalRoot) -> bool {
        let candidate = root.candidate();
        self.root_evidence == *root
            && self.root == candidate.identity
            && self.normalized_root_identity == root.normalized_identity()
            && self.provider_plan == candidate.provider_plan
            && self.provider == candidate.provider
            && self.entry == candidate.entry
            && self.boundary_contract_fingerprint == root.boundary_contract_fingerprint()
            && self.stack_artifact_composition_fingerprint
                == candidate.stack.realization.composition().fingerprint()
            && self.stack_demand_fingerprint == candidate.stack.realization.fingerprint()
            && self.logical_fuel_fingerprint
                == candidate.logical_fuel.realization.composition_fingerprint()
            && self.machine_state_validation_receipt == candidate.machine_state.validation_receipt
            && self.effects == candidate.effects
    }

    pub(super) fn validate_installed_entry_binding(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), ExternalRootDiagnostic> {
        if installed_code.selected_entry_target(self.entry).is_err() {
            return Err(ExternalRootDiagnostic(
                "external-root entry is not in the admitted installed artifact".into(),
            ));
        }
        let root_stack_summary = self
            .root_evidence
            .candidate
            .stack
            .realization
            .input(self.root_evidence.candidate.identity)
            .expect("stack composition retains its root input");
        if !root_stack_summary
            .realization_evidence()
            .matches_installed_code_entry(installed_code, self.entry)
        {
            return Err(ExternalRootDiagnostic(
                "entry stack realization is not bound to the exact installed code and selected entry"
                    .into(),
            ));
        }
        if let StackLocalEvidence::TerminalEntry(binding) = root_stack_summary.body_evidence() {
            validate_installed_entry_stack(binding, installed_code, self.entry).map_err(
                |_| {
                    ExternalRootDiagnostic(
                        "terminal stack root evidence is not bound to the exact installed code and selected entry"
                            .into(),
                    )
                },
            )?;
        }
        let root_fuel_summary = self
            .root_evidence
            .candidate
            .logical_fuel
            .realization
            .composition_evidence
            .summaries
            .get(&self.root_evidence.candidate.logical_fuel.realization.root())
            .expect("fixed-fuel composition retains its root summary");
        let fuel_binding_matches = match &root_fuel_summary.local_evidence {
            FixedFuelLocalEvidence::TerminalEntry(binding) => {
                validate_installed_entry_fuel(binding, installed_code, self.entry).is_ok()
            }
            FixedFuelLocalEvidence::TerminalSegment(_) => false,
            FixedFuelLocalEvidence::AdmittedProvider { .. } => true,
        };
        if !fuel_binding_matches {
            return Err(ExternalRootDiagnostic(
                "terminal fixed-fuel root evidence is not a whole-entry certificate bound to the exact installed code and selected entry"
                    .into(),
            ));
        }
        Ok(())
    }
}

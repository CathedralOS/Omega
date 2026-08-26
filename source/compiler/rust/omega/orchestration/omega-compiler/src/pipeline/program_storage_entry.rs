//! Installation handoff for the core-owned program-storage entry roots.
//!
//! Target entry traits inherit `ProgramStorageEntry::enter`; they do not own a
//! second image/storage convention.  This bridge therefore selects the exact
//! inherited requirement and its semantic parameter ordinals from the checked
//! provider schema, joins them to one validated calling plan and generated
//! prologue, and only then consumes admitted extent grants.

use omega_calling_conventions::{ValidatedBoundaryEntryPlan, ValuePlacement};
use omega_effects::ComponentEraEntryLedger;
use omega_effects::provider_plan::ServiceEntryAuthorityFlow;
use omega_external_roots::{
    EstablishedProgramLocalRoot, InstalledProgramLocalRootSubject,
    ProgramLocalExtentMaterializationPlan, ProgramLocalExtentRegistry,
    ProgramLocalRootEpochRuntime, ProgramLocalRootInstallationLedger,
};
use omega_instruction_selection::DerivedBoundaryEntryStorage;
use psi_diagnostics::Diagnostic;
use psi_extents::{
    Extent, ExtentLoan, ExtentRootGrant, OwnedExtentPartition, ValidatedExtentGeometry,
};
use std::path::Path;

use super::{ProgramLocalStorageCustody, ProgramLocalStorageCustodyError};

const PROGRAM_STORAGE_ENTRY_OWNER: &str = "ProgramStorageEntry";
const PROGRAM_STORAGE_ENTRY_METHOD: &str = "enter";
const EXTENT_CARRIER: &str = "named(name(Extent))";
const GRANTED_DOMAIN: &str = "Extent::Granted";
const IMAGE_PARAMETER_INDEX: usize = 0;
const INITIAL_STORAGE_PARAMETER_INDEX: usize = 1;

/// One exact qualified semantic parameter joined to its generated ABI capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryParameter {
    parameter_index: usize,
    carrier_identity: String,
    parameter_type_identity: String,
    domain: String,
    effective_carry: psi_language_semantics::CarryPolicy,
    placement: ValuePlacement,
    destination_byte_offset: usize,
    write_range: std::ops::Range<usize>,
}

impl ProgramStorageEntryParameter {
    pub const fn parameter_index(&self) -> usize {
        self.parameter_index
    }

    /// Bare semantic carrier independently retained from the qualified
    /// interface type. Program-local producer schemas name this identity.
    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub fn parameter_type_identity(&self) -> &str {
        &self.parameter_type_identity
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub const fn effective_carry(&self) -> psi_language_semantics::CarryPolicy {
        self.effective_carry
    }

    pub const fn placement(&self) -> &ValuePlacement {
        &self.placement
    }

    pub const fn destination_byte_offset(&self) -> usize {
        self.destination_byte_offset
    }

    pub const fn write_range(&self) -> &std::ops::Range<usize> {
        &self.write_range
    }
}

/// Exact selected target-entry contract that may introduce program storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryPlanBinding {
    root_slot: omega_external_roots::RootSlotId,
    requirement_identity: String,
    boundary_contract_fingerprint: u64,
    image: ProgramStorageEntryParameter,
    initial_storage: ProgramStorageEntryParameter,
    receiver: Option<ProgramEntryReceiverStoragePlan>,
    source_signature: Option<super::SelectedProgramEntrySourceSignature>,
    physical_contract: Option<super::ProgramEntryPhysicalContractPlan>,
}

impl ProgramStorageEntryPlanBinding {
    pub const fn root_slot(&self) -> omega_external_roots::RootSlotId {
        self.root_slot
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }

    pub const fn image(&self) -> &ProgramStorageEntryParameter {
        &self.image
    }

    pub const fn initial_storage(&self) -> &ProgramStorageEntryParameter {
        &self.initial_storage
    }

    pub const fn receiver(&self) -> Option<&ProgramEntryReceiverStoragePlan> {
        self.receiver.as_ref()
    }

    /// Exact declaration signature captured from the selected typed source
    /// entry before backend lowering. This is not a value or authority carrier.
    pub const fn source_signature(&self) -> Option<&super::SelectedProgramEntrySourceSignature> {
        self.source_signature.as_ref()
    }

    /// Target-fixed physical environment contract retained independently from
    /// this semantic root-installation plan. Lower-level semantic tests may
    /// omit it; production UEFI selection must retain it.
    pub const fn physical_contract(&self) -> Option<&super::ProgramEntryPhysicalContractPlan> {
        self.physical_contract.as_ref()
    }

    /// Attach the concrete layout of a receiver whose exclusive source shape
    /// and ZII validity were already checked for this selected entry.
    pub fn with_checked_receiver_layout(
        mut self,
        type_identity: String,
        layout: omega_layout::TypeLayout,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        if type_identity.is_empty() {
            return Err(ProgramStorageEntryDiagnostic(
                "selected entry receiver type identity cannot be empty".into(),
            ));
        }
        if layout.alignment == 0 || !layout.alignment.is_power_of_two() {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "selected entry receiver has invalid {}-byte alignment",
                layout.alignment
            )));
        }
        if self.receiver.is_some() {
            return Err(ProgramStorageEntryDiagnostic(
                "selected entry already has a checked receiver layout".into(),
            ));
        }
        self.receiver = Some(ProgramEntryReceiverStoragePlan {
            type_identity,
            byte_size: layout.size,
            byte_alignment: layout.alignment,
        });
        Ok(self)
    }
}

/// Compiler-checked storage demand for one occurrence-local entry receiver.
///
/// This is a reservation plan, not evidence that bytes have already been
/// zeroed. The generated physical bridge must materialize the ZII value and
/// lend the resulting occurrence exactly once before source execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryReceiverStoragePlan {
    type_identity: String,
    byte_size: usize,
    byte_alignment: usize,
}

impl ProgramEntryReceiverStoragePlan {
    pub fn type_identity(&self) -> &str {
        &self.type_identity
    }

    pub const fn byte_size(&self) -> usize {
        self.byte_size
    }

    pub const fn byte_alignment(&self) -> usize {
        self.byte_alignment
    }

    #[cfg(test)]
    pub(super) fn for_test(type_identity: &str, byte_size: usize, byte_alignment: usize) -> Self {
        Self {
            type_identity: type_identity.into(),
            byte_size,
            byte_alignment,
        }
    }
}

/// Exact target-owned environment-to-program slot and its normalized source
/// schema. This is deliberately not a provider plan: `ProgramEntry` accepts an
/// environment root and does not model an outbound service conformance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramStorageEntryPlan {
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    root_slot: omega_external_roots::RootSlotId,
    requirement_identity: String,
    schema: omega_effects::provider_plan::ServiceSchema,
    physical_contract: Option<super::ProgramEntryPhysicalContractPlan>,
}

impl SelectedProgramStorageEntryPlan {
    pub fn from_target_slot(
        slot: omega_target::ProgramEntrySlotDeclaration,
        schema: omega_effects::provider_plan::ServiceSchema,
        requirement_identity: String,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        if slot != slot.owner.program_entry_slot()
            || slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
            || slot.visible_parameters
                != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
            || slot.semantic_arrival_requirement
                != format!("{PROGRAM_STORAGE_ENTRY_OWNER}::{PROGRAM_STORAGE_ENTRY_METHOD}")
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` does not declare the exact program-storage entry contract",
                slot.owner.root_slot_owner_name(),
                slot.slot_name
            )));
        }
        let Some(boundary_schema) = slot.boundary_schema else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` has no source boundary schema",
                slot.owner.root_slot_owner_name(),
                slot.slot_name
            )));
        };
        if schema.trait_name != boundary_schema {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target root slot `{}::{}` requires boundary schema `{boundary_schema}`, not `{}`",
                slot.owner.root_slot_owner_name(),
                slot.slot_name,
                schema.trait_name
            )));
        }
        if requirement_identity.is_empty() {
            return Err(ProgramStorageEntryDiagnostic(
                "target program-storage entry has no exact arrival requirement identity".into(),
            ));
        }
        let matching_methods = schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement_identity)
            .collect::<Vec<_>>();
        let [method] = matching_methods.as_slice() else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target program-storage entry schema retains {} copies of exact arrival requirement `{requirement_identity}`",
                matching_methods.len(),
            )));
        };
        if method.requirement_owner != PROGRAM_STORAGE_ENTRY_OWNER
            || method.name != PROGRAM_STORAGE_ENTRY_METHOD
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "target program-storage arrival requirement `{requirement_identity}` drifted from `{PROGRAM_STORAGE_ENTRY_OWNER}::{PROGRAM_STORAGE_ENTRY_METHOD}`",
            )));
        }

        let root_slot = omega_external_roots::RootSlotId::for_target_program_entry(slot)
            .map_err(|diagnostic| ProgramStorageEntryDiagnostic(diagnostic.to_string()))?;
        Ok(Self {
            target_slot: slot,
            root_slot,
            requirement_identity,
            schema,
            physical_contract: None,
        })
    }

    pub(crate) fn with_physical_contract(
        mut self,
        physical_contract: super::ProgramEntryPhysicalContractPlan,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        if physical_contract.target_slot() != self.target_slot {
            return Err(ProgramStorageEntryDiagnostic(
                "physical entry contract belongs to a different target slot".into(),
            ));
        }
        if self.physical_contract.is_some() {
            return Err(ProgramStorageEntryDiagnostic(
                "selected program-storage entry already has a physical contract".into(),
            ));
        }
        self.physical_contract = Some(physical_contract);
        Ok(self)
    }

    pub const fn root_slot(&self) -> omega_external_roots::RootSlotId {
        self.root_slot
    }

    pub const fn target_slot(&self) -> omega_target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn schema(&self) -> &omega_effects::provider_plan::ServiceSchema {
        &self.schema
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn physical_contract(&self) -> Option<&super::ProgramEntryPhysicalContractPlan> {
        self.physical_contract.as_ref()
    }
}

/// Provider-admitted authority and the runtime geometry presented at one
/// program-entry parameter. Construction imports no fact; installation first
/// validates both inputs and returns both values intact on failure.
#[derive(Debug, PartialEq, Eq)]
pub struct ProgramStorageRootInput {
    grant: ExtentRootGrant,
    base: u64,
    length: u64,
}

/// Exact selected root-provider occurrence authorized to supply both roots
/// for one generated program-entry bridge invocation.
///
/// Image and initial-storage roots may carry different route, capacity, and
/// qualification evidence, but they must originate from this same selected
/// provider plan and concrete invocation. Construction retains those typed
/// identities from provider evidence rather than accepting untyped integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramStorageEntryProviderInvocation {
    provider: psi_extents::ExtentProviderId,
    provider_plan: psi_extents::ExtentProviderPlanId,
    invocation: psi_extents::ExtentProviderInvocationId,
}

/// Pending native handoff emitted for one exact program-storage entry.
///
/// Compilation can bind the selected source continuation to the final object
/// entry symbol and provider selection, but it cannot manufacture runtime
/// geometry, admitted issuances, or mapped receiver bytes. A platform
/// installer consumes this plan with those environment-supplied values through
/// [`install_and_activate_program_storage_entry_receiver`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryNativeBridgePlan {
    binding: ProgramStorageEntryPlanBinding,
    wrapper_transfer: super::program_storage_wrapper::ProgramStorageEntryWrapperTransferPlan,
    continuation_abi:
        Option<super::program_storage_source_call::ProgramStorageEntryContinuationAbiPlan>,
    continuation_inbound: Option<
        super::program_storage_continuation_inbound::ProgramStorageEntryContinuationInboundPlan,
    >,
    wrapper_body_template:
        Option<super::program_storage_wrapper_body::ProgramStorageEntryWrapperBodyTemplatePlan>,
    emitted_wrapper_evidence:
        Option<super::program_storage_wrapper_evidence::ProgramStorageEntryEmittedWrapperEvidence>,
    selected_provider: Option<super::provider_plans::SelectedExternalRootProviderPlan>,
    target_profile: String,
    entry_symbol: String,
    entry_text_offset: usize,
    entry_text_size: usize,
    entry_function_identity: omega_control_flow::MachineFunctionIdentity,
    continuation_key: omega_control_flow::StateKey,
    continuation_symbol: String,
    continuation_link_symbol: String,
    continuation_text_offset: usize,
    continuation_text_size: usize,
    continuation_machine: String,
    continuation_state: String,
}

impl ProgramStorageEntryNativeBridgePlan {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    /// Present for compiler-generated bindings. Lower-level physical-plan
    /// tests may bind only the arrival contract and therefore retain `None`.
    pub const fn source_signature(&self) -> Option<&super::SelectedProgramEntrySourceSignature> {
        self.binding.source_signature()
    }

    /// Retained target-fixed launch contract. This is plan identity only: the
    /// current bridge emits the inner semantic wrapper and does not claim a
    /// physical shell, bootstrap invocation, or firmware-supplied values.
    pub const fn physical_contract(&self) -> Option<&super::ProgramEntryPhysicalContractPlan> {
        self.binding.physical_contract()
    }

    /// Address-free semantic root/receiver handoff retained for the future
    /// generated wrapper. This does not claim an outbound call ABI or body.
    pub const fn wrapper_transfer(
        &self,
    ) -> &super::program_storage_wrapper::ProgramStorageEntryWrapperTransferPlan {
        &self.wrapper_transfer
    }

    /// Complete address-free internal ABI for compiler-generated bindings.
    /// Lower-level physical-plan tests without a sealed source declaration
    /// retain `None`. Neither form carries runtime root values or authority.
    pub const fn continuation_abi(
        &self,
    ) -> Option<&super::program_storage_source_call::ProgramStorageEntryContinuationAbiPlan> {
        self.continuation_abi.as_ref()
    }

    /// Exact encoded source-function capture evidence for the receiver-free
    /// continuation ABI. Attached continuations retain `None` until their
    /// hidden receiver and residual-root inbound realization exists.
    pub const fn continuation_inbound(
        &self,
    ) -> Option<
        &super::program_storage_continuation_inbound::ProgramStorageEntryContinuationInboundPlan,
    > {
        self.continuation_inbound.as_ref()
    }

    /// Address-free, post-encoding phase-alignment template for the exact
    /// receiver-free generated body. This is not evidence that the template
    /// has been inserted, lowered, emitted, or selected as the object entry.
    pub const fn wrapper_body_template(
        &self,
    ) -> Option<&super::program_storage_wrapper_body::ProgramStorageEntryWrapperBodyTemplatePlan>
    {
        self.wrapper_body_template.as_ref()
    }

    /// The only admitted native bridge without the receiver-free wrapper
    /// template: an exact checked mutable receiver is provisioned and passed
    /// through the continuation ABI as its activation loan.
    pub fn is_receiver_bound_without_wrapper_template(&self) -> bool {
        let (Some(storage), Some(source), Some(abi)) = (
            self.binding.receiver(),
            self.source_signature(),
            self.continuation_abi(),
        ) else {
            return false;
        };
        self.wrapper_body_template.is_none()
            && self.continuation_inbound.is_none()
            && matches!(
                source.receiver(),
                super::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    normalized_type_identity,
                } if normalized_type_identity == storage.type_identity()
            )
            && matches!(
                abi.receiver(),
                super::ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
                    storage: abi_storage,
                    ..
                } if abi_storage == storage
            )
    }

    /// Exact placed-region and relocated-call evidence retained only after the
    /// checked executable image has been emitted. This proves image content,
    /// not platform invocation or runtime root installation.
    pub const fn emitted_wrapper_evidence(
        &self,
    ) -> Option<&super::program_storage_wrapper_evidence::ProgramStorageEntryEmittedWrapperEvidence>
    {
        self.emitted_wrapper_evidence.as_ref()
    }

    pub(super) fn retain_emitted_wrapper_evidence(
        &mut self,
        evidence: super::program_storage_wrapper_evidence::ProgramStorageEntryEmittedWrapperEvidence,
    ) -> Result<(), ProgramStorageEntryDiagnostic> {
        if self.emitted_wrapper_evidence.is_some() {
            return Err(ProgramStorageEntryDiagnostic(
                "program-storage bridge already retained final wrapper evidence".into(),
            ));
        }
        self.emitted_wrapper_evidence = Some(evidence);
        Ok(())
    }

    pub const fn selected_provider(
        &self,
    ) -> Option<&super::provider_plans::SelectedExternalRootProviderPlan> {
        self.selected_provider.as_ref()
    }

    pub fn target_profile(&self) -> &str {
        &self.target_profile
    }

    pub fn entry_symbol(&self) -> &str {
        &self.entry_symbol
    }

    pub const fn entry_text_offset(&self) -> usize {
        self.entry_text_offset
    }

    pub const fn entry_text_size(&self) -> usize {
        self.entry_text_size
    }

    pub const fn entry_function_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.entry_function_identity
    }

    /// Exact control-flow identity of the selected source continuation. A
    /// generated entry wrapper retains this key without acquiring it as its
    /// own function identity.
    pub const fn continuation_key(&self) -> omega_control_flow::StateKey {
        self.continuation_key
    }

    /// Diagnostic symbol retained by the encoded source function.
    pub fn continuation_symbol(&self) -> &str {
        &self.continuation_symbol
    }

    /// Canonical object-local symbol that a direct-call relocation must name.
    pub fn continuation_link_symbol(&self) -> &str {
        &self.continuation_link_symbol
    }

    pub const fn continuation_text_offset(&self) -> usize {
        self.continuation_text_offset
    }

    pub const fn continuation_text_size(&self) -> usize {
        self.continuation_text_size
    }

    pub fn continuation_machine(&self) -> &str {
        &self.continuation_machine
    }

    pub fn continuation_state(&self) -> &str {
        &self.continuation_state
    }

    /// Run one platform executor only while this bridge's exact receiver
    /// activation remains live.
    ///
    /// This joins the retained emitted-entry identity to the production
    /// provider installer and mapped receiver carrier. The executor receives a
    /// sealed borrowed handoff only after installation and ZII construction
    /// succeed; it cannot detach either the handoff or receiver loan from this
    /// call. Successful return records only that this supplied executor ran
    /// through the checked bridge gate. It is not evidence that native bytes
    /// were invoked.
    pub fn dispatch_source_continuation_executor<Output>(
        &self,
        artifact_directory: &Path,
        provider_issuance: psi_extents::ExtentProviderIssuance,
        image: ProgramStorageRootInput,
        initial_storage: ProgramStorageRootInput,
        mapped_base: u64,
        mapped_storage: &mut [u8],
        execute: impl for<'handoff> FnOnce(
            ProgramStorageEntrySourceContinuationHandoff<'handoff>,
        ) -> Output,
    ) -> Result<ProgramStorageEntryExecutorDispatch<Output>, ProgramStorageEntryBridgeError> {
        let Some(selected_provider) = self.selected_provider.as_ref() else {
            return Err(ProgramStorageEntryBridgeError::Installation(
                ProgramStorageInstallationHandoffError::Rejected(Box::new(
                    ProgramStorageRootInstallationError {
                        binding: self.binding.clone(),
                        image,
                        initial_storage,
                        diagnostic: ProgramStorageEntryDiagnostic(
                            "program-storage native bridge has no retained selected physical provider"
                                .into(),
                        ),
                    },
                )),
            ));
        };
        let mut activation = install_and_activate_program_storage_entry_receiver(
            artifact_directory,
            self.binding.clone(),
            selected_provider,
            provider_issuance,
            image,
            initial_storage,
            mapped_base,
            mapped_storage,
        )?;
        let provider_invocation = activation
            .provider_invocation()
            .expect("physical bridge activation retains its selected provider invocation");
        let receiver_placement = activation.placement().clone();
        let continuation_receiver = if let Some(continuation_abi) = &self.continuation_abi {
            match continuation_abi
                .bind_activation_loan(&receiver_placement, activation.receiver().len())
            {
                Ok(receiver) => Some(receiver),
                Err(diagnostic) => {
                    return Err(ProgramStorageEntryBridgeError::ContinuationReceiverBinding(
                        Box::new(ProgramStorageEntryContinuationReceiverBindingError {
                            roots: activation.finish(),
                            diagnostic,
                        }),
                    ));
                }
            }
        } else {
            None
        };
        let output = execute(ProgramStorageEntrySourceContinuationHandoff {
            bridge: self,
            provider_invocation,
            receiver_placement,
            continuation_receiver,
            receiver: activation.receiver(),
        });
        let roots = activation.finish();
        Ok(ProgramStorageEntryExecutorDispatch { roots, output })
    }
}

/// Non-constructible borrowed input to one platform-owned source-continuation
/// executor.
///
/// The exact emitted identity and provider occurrence are observations. The
/// mutable bytes are the one live activation loan and can only be reborrowed
/// while this handoff is executing.
pub struct ProgramStorageEntrySourceContinuationHandoff<'a> {
    bridge: &'a ProgramStorageEntryNativeBridgePlan,
    provider_invocation: ProgramStorageEntryProviderInvocation,
    receiver_placement: ProgramEntryReceiverPlacementRecord,
    continuation_receiver:
        Option<super::program_storage_source_call::ProgramStorageEntryContinuationReceiverBinding>,
    receiver: &'a mut [u8],
}

impl ProgramStorageEntrySourceContinuationHandoff<'_> {
    pub fn entry_symbol(&self) -> &str {
        self.bridge.entry_symbol()
    }

    pub const fn entry_text_offset(&self) -> usize {
        self.bridge.entry_text_offset()
    }

    pub const fn entry_text_size(&self) -> usize {
        self.bridge.entry_text_size()
    }

    pub const fn entry_function_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.bridge.entry_function_identity()
    }

    pub const fn wrapper_transfer(
        &self,
    ) -> &super::program_storage_wrapper::ProgramStorageEntryWrapperTransferPlan {
        self.bridge.wrapper_transfer()
    }

    /// Exact checked typed declaration selected before backend lowering. It
    /// carries no runtime roots, authority, ABI placement, or call readiness.
    pub const fn source_signature(&self) -> Option<&super::SelectedProgramEntrySourceSignature> {
        self.bridge.source_signature()
    }

    pub const fn continuation_abi(
        &self,
    ) -> Option<&super::program_storage_source_call::ProgramStorageEntryContinuationAbiPlan> {
        self.bridge.continuation_abi()
    }

    /// Exact mapped receiver loan bound to the retained outbound ABI. A
    /// lower-level physical-plan-only handoff has no ABI binding. Neither form
    /// carries a root value or detachable authority.
    pub const fn continuation_receiver(
        &self,
    ) -> Option<&super::program_storage_source_call::ProgramStorageEntryContinuationReceiverBinding>
    {
        self.continuation_receiver.as_ref()
    }

    pub const fn continuation_key(&self) -> omega_control_flow::StateKey {
        self.bridge.continuation_key()
    }

    pub fn continuation_symbol(&self) -> &str {
        self.bridge.continuation_symbol()
    }

    pub fn continuation_link_symbol(&self) -> &str {
        self.bridge.continuation_link_symbol()
    }

    pub const fn continuation_text_offset(&self) -> usize {
        self.bridge.continuation_text_offset()
    }

    pub const fn continuation_text_size(&self) -> usize {
        self.bridge.continuation_text_size()
    }

    pub const fn provider_invocation(&self) -> ProgramStorageEntryProviderInvocation {
        self.provider_invocation
    }

    pub const fn receiver_placement(&self) -> &ProgramEntryReceiverPlacementRecord {
        &self.receiver_placement
    }

    pub fn receiver(&mut self) -> &mut [u8] {
        self.receiver
    }
}

/// Conserved authority and ordinary output returned after a checked platform
/// executor finishes. This is not a native-execution receipt.
#[derive(Debug)]
pub struct ProgramStorageEntryExecutorDispatch<Output> {
    roots: InstalledProgramStorageRoots,
    output: Output,
}

impl<Output> ProgramStorageEntryExecutorDispatch<Output> {
    pub const fn roots(&self) -> &InstalledProgramStorageRoots {
        &self.roots
    }

    pub const fn output(&self) -> &Output {
        &self.output
    }

    pub fn into_parts(self) -> (InstalledProgramStorageRoots, Output) {
        (self.roots, self.output)
    }
}

/// Bind the pending physical bridge to the actual emitted entry function.
/// This records installable coordinates only; successful compilation remains
/// distinct from runtime provider installation.
pub fn bind_emitted_program_storage_entry_native_bridge(
    binding: ProgramStorageEntryPlanBinding,
    selected_provider: Option<super::provider_plans::SelectedExternalRootProviderPlan>,
    target_profile: String,
    object: &omega_object_file::ObjectPlan,
    encoded_machine: &omega_machine_bytes::EncodedMachinePlan,
    continuation_key: omega_control_flow::StateKey,
    boundary_contract_fingerprint: Option<u64>,
    continuation_machine: String,
    continuation_state: String,
) -> Result<ProgramStorageEntryNativeBridgePlan, ProgramStorageEntryDiagnostic> {
    if target_profile.is_empty() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge has no selected target profile".into(),
        ));
    }
    let entry_handle = object.layout.entry_symbol;
    if !object.layout.symbols.is_valid(entry_handle) {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge has no emitted object entry symbol".into(),
        ));
    }
    let entry = object.layout.symbols.get(entry_handle);
    if entry.kind != omega_object_file::SymbolKind::Function
        || entry.section
            != omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text)
        || entry.size == 0
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge entry symbol is not a nonempty text function".into(),
        ));
    }
    let encoded_entry =
        validate_encoded_program_storage_entry(entry, encoded_machine, continuation_key)?;
    let continuation_identity =
        omega_control_flow::MachineFunctionIdentity::source(continuation_key);
    let (_, continuation_link) = omega_object_file::object_function_symbol(
        object,
        continuation_identity,
    )
    .ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "program-storage native bridge source continuation has no exact object linkage".into(),
        )
    })?;
    if continuation_link.offset != encoded_entry.continuation.byte_offset
        || continuation_link.size != encoded_entry.continuation.byte_count
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge source-continuation object linkage does not cover its exact encoded function"
                .into(),
        ));
    }
    if boundary_contract_fingerprint != Some(binding.boundary_contract_fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "emitted entry footprint does not retain the program-storage boundary fingerprint"
                .into(),
        ));
    }
    if let Some(selected_provider) = &selected_provider {
        validate_selected_provider_binding(&binding, selected_provider)?;
    }
    if continuation_machine.is_empty() || continuation_state.is_empty() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge lost its selected source continuation".into(),
        ));
    }
    if let Some(source_signature) = binding.source_signature() {
        source_signature
            .validate_program_storage_binding(
                source_signature.target_slot(),
                continuation_key,
                binding
                    .receiver()
                    .map(ProgramEntryReceiverStoragePlan::type_identity),
                binding.image().parameter_type_identity(),
                binding.initial_storage().parameter_type_identity(),
            )
            .map_err(ProgramStorageEntryDiagnostic)?;
        if source_signature.machine_name() != continuation_machine
            || source_signature.state_name() != continuation_state
        {
            return Err(ProgramStorageEntryDiagnostic(
                "program-storage native bridge source declaration names drifted from the exact lowered continuation"
                    .into(),
            ));
        }
    }
    let wrapper_transfer =
        super::program_storage_wrapper::plan_program_storage_entry_wrapper_transfer(
            &binding,
            continuation_key,
        )?;
    let continuation_abi = binding
        .source_signature()
        .map(|source_signature| {
            super::program_storage_source_call::plan_program_storage_entry_continuation_abi(
                encoded_machine.target,
                &wrapper_transfer,
                source_signature,
            )
        })
        .transpose()?;
    let continuation_inbound = continuation_abi
        .as_ref()
        .map(|abi| {
            super::program_storage_continuation_inbound::plan_program_storage_entry_continuation_inbound(
                &binding,
                abi,
                encoded_entry.continuation,
                encoded_machine,
            )
        })
        .transpose()?
        .flatten();
    let wrapper_body_template = match (&continuation_abi, &continuation_inbound) {
        (Some(abi), Some(inbound)) => Some(
            super::program_storage_wrapper_body::plan_program_storage_entry_wrapper_body_template(
                &wrapper_transfer,
                abi,
                inbound,
            )?,
        ),
        _ => None,
    };
    Ok(ProgramStorageEntryNativeBridgePlan {
        binding,
        wrapper_transfer,
        continuation_abi,
        continuation_inbound,
        wrapper_body_template,
        emitted_wrapper_evidence: None,
        selected_provider,
        target_profile,
        entry_symbol: entry.name.clone(),
        entry_text_offset: entry.offset,
        entry_text_size: entry.size,
        entry_function_identity: encoded_entry.entry.identity,
        continuation_key,
        continuation_symbol: encoded_entry.continuation.symbol.to_string(),
        continuation_link_symbol: continuation_link.name.clone(),
        continuation_text_offset: encoded_entry.continuation.byte_offset,
        continuation_text_size: encoded_entry.continuation.byte_count,
        continuation_machine,
        continuation_state,
    })
}

/// Bind the compiler-generated program-storage bridge around the exact
/// backend plan that will enter emission. Receiver-free source entry requires
/// a preview so its wrapper can be inserted; the returned bridge is always
/// replayed against the final, possibly mutated plan.
pub(super) fn bind_compiler_generated_program_storage_entry_native_bridge(
    binding: Option<ProgramStorageEntryPlanBinding>,
    selected_provider: Option<super::provider_plans::SelectedExternalRootProviderPlan>,
    selected_target: Option<&str>,
    backend: &mut omega_backend_plan::BackendPlan,
) -> Result<Option<ProgramStorageEntryNativeBridgePlan>, Vec<Diagnostic>> {
    let Some(binding) = binding else {
        return Ok(None);
    };
    if binding.source_signature().is_none() {
        return Err(vec![Diagnostic::error(
            "compiler-generated program-storage binding lost its checked source signature",
        )]);
    }
    if binding.physical_contract().is_none() {
        return Err(vec![Diagnostic::error(
            "compiler-generated UEFI program-storage binding lost its distinct physical entry contract",
        )]);
    }

    let target_profile = compiler_generated_program_storage_target_profile(selected_target);
    let bind = |binding, selected_provider, backend: &omega_backend_plan::BackendPlan| {
        bind_emitted_program_storage_entry_native_bridge(
            binding,
            selected_provider,
            target_profile.clone(),
            &backend.object,
            &backend.encoded_machine,
            backend.entry_key,
            backend
                .encoded_machine
                .semantics
                .boundaries
                .footprints
                .boundary_contract_fingerprint,
            backend.entry_machine_name().to_owned(),
            backend.entry_state_name().to_owned(),
        )
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
    };

    let preview = bind(binding.clone(), selected_provider.clone(), backend)?;
    if let Some(template) = preview.wrapper_body_template() {
        super::program_storage_wrapper_body::insert_and_validate_program_storage_entry_wrapper(
            template, backend,
        )
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    }
    bind(binding, selected_provider, backend).map(Some)
}

/// Settle the optional compiler-generated bridge against the checked final
/// image immediately before publication. Receiver-bound continuations are the
/// sole admitted template-free form; receiver-free bridges retain one exact
/// final-image evidence row before any output becomes visible.
pub(super) fn retain_compiler_generated_program_storage_entry_publication_evidence(
    bridge: Option<&mut ProgramStorageEntryNativeBridgePlan>,
    backend: &omega_backend_plan::BackendPlan,
    checked_image: Option<&omega_image::EmittedImageOutput>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(bridge) = bridge else {
        return Ok(());
    };
    if bridge.wrapper_body_template().is_none() {
        if bridge.is_receiver_bound_without_wrapper_template() {
            return Ok(());
        }
        return Err(vec![Diagnostic::error(
            "native program-storage publication lost its receiver-free wrapper template without an exact receiver-bound continuation",
        )]);
    }
    let checked_image = checked_image.ok_or_else(|| {
        vec![Diagnostic::error(
            "program-storage entry target emitted no checked executable image",
        )]
    })?;
    let evidence =
        super::program_storage_wrapper_evidence::bind_final_program_storage_entry_wrapper_evidence(
            bridge,
            backend,
            checked_image,
        )
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    bridge
        .retain_emitted_wrapper_evidence(evidence)
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
}

fn compiler_generated_program_storage_target_profile(selected_target: Option<&str>) -> String {
    selected_target.unwrap_or("host").to_owned()
}

#[derive(Debug)]
struct ValidatedEncodedProgramStorageEntry<'a> {
    entry: &'a omega_machine_bytes::EncodedMachineFunction,
    continuation: &'a omega_machine_bytes::EncodedMachineFunction,
}

fn validate_encoded_program_storage_entry<'a>(
    entry: &omega_object_file::SymbolPlan,
    encoded_machine: &'a omega_machine_bytes::EncodedMachinePlan,
    continuation_key: omega_control_flow::StateKey,
) -> Result<ValidatedEncodedProgramStorageEntry<'a>, ProgramStorageEntryDiagnostic> {
    if !continuation_key.is_valid() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage native bridge has no exact source-continuation key".into(),
        ));
    }
    let mut encoded_entries = encoded_machine
        .code
        .functions
        .iter()
        .filter_map(|(_, function)| (function.symbol.as_ref() == entry.name).then_some(function));
    let Some(encoded_entry) = encoded_entries.next() else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-storage native bridge object entry `{}` has no encoded function",
            entry.name
        )));
    };
    if encoded_entries.next().is_some() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-storage native bridge object entry `{}` names more than one encoded function",
            entry.name
        )));
    }
    if !encoded_entry.identity.is_valid() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-storage native bridge object entry `{}` has an invalid function identity",
            entry.name
        )));
    }
    let continuation = if encoded_entry.identity.source_key() == Some(continuation_key) {
        encoded_entry
    } else if encoded_entry.identity.program_storage_entry_continuation() == Some(continuation_key)
    {
        let source_identity = omega_control_flow::MachineFunctionIdentity::source(continuation_key);
        let mut continuations = encoded_machine
            .code
            .functions
            .iter()
            .filter_map(|(_, function)| (function.identity == source_identity).then_some(function));
        let Some(continuation) = continuations.next() else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage native bridge object entry `{}` has no encoded source continuation {:?}",
                entry.name, continuation_key
            )));
        };
        if continuations.next().is_some() {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage native bridge source continuation {:?} names more than one encoded function",
                continuation_key
            )));
        }
        if continuation.byte_count == 0 {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage native bridge source continuation {:?} is not a nonempty encoded function",
                continuation_key
            )));
        }
        let entry_end = encoded_entry
            .byte_offset
            .checked_add(encoded_entry.byte_count)
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "program-storage native bridge entry interval overflows encoded text".into(),
                )
            })?;
        let continuation_end = continuation
            .byte_offset
            .checked_add(continuation.byte_count)
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "program-storage native bridge source-continuation interval overflows encoded text"
                        .into(),
                )
            })?;
        if encoded_entry.byte_offset < continuation_end && continuation.byte_offset < entry_end {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage native bridge object entry `{}` overlaps its separately identified source continuation",
                entry.name
            )));
        }
        continuation
    } else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-storage native bridge object entry `{}` redirects source continuation {:?} through identity {:?}",
            entry.name, continuation_key, encoded_entry.identity
        )));
    };
    if encoded_entry.byte_offset != entry.offset || encoded_entry.byte_count != entry.size {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-storage native bridge object entry `{}` does not cover its exact encoded entry function",
            entry.name
        )));
    }
    Ok(ValidatedEncodedProgramStorageEntry {
        entry: encoded_entry,
        continuation,
    })
}

impl ProgramStorageEntryProviderInvocation {
    fn bind_selected_provider(
        binding: &ProgramStorageEntryPlanBinding,
        selected: &super::provider_plans::SelectedExternalRootProviderPlan,
        issuance: psi_extents::ExtentProviderIssuance,
    ) -> Result<Self, ProgramStorageEntryDiagnostic> {
        let invocation = issuance.invocation();
        if selected.identity.normalized_identity()
            != invocation.provider_plan().normalized_identity()
        {
            return Err(ProgramStorageEntryDiagnostic(
                "root issuance does not belong to the compiler-selected provider plan".into(),
            ));
        }
        validate_selected_provider_binding(binding, selected)?;
        Ok(Self {
            provider: issuance.provider(),
            provider_plan: invocation.provider_plan(),
            invocation: invocation.invocation(),
        })
    }

    pub const fn provider(&self) -> psi_extents::ExtentProviderId {
        self.provider
    }

    pub const fn provider_plan(&self) -> psi_extents::ExtentProviderPlanId {
        self.provider_plan
    }

    pub const fn invocation(&self) -> psi_extents::ExtentProviderInvocationId {
        self.invocation
    }

    fn matches(&self, issuance: psi_extents::ExtentProviderIssuance) -> bool {
        let invocation = issuance.invocation();
        self.provider == issuance.provider()
            && self.provider_plan == invocation.provider_plan()
            && self.invocation == invocation.invocation()
    }
}

fn validate_selected_provider_binding(
    binding: &ProgramStorageEntryPlanBinding,
    selected: &super::provider_plans::SelectedExternalRootProviderPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let matching_methods = selected
        .schema
        .methods
        .iter()
        .filter(|method| method.requirement_identity == binding.requirement_identity)
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(
            "selected root provider does not implement the bound semantic arrival requirement exactly once"
                .into(),
        ));
    };
    if method.calling_plan_fingerprint != Some(binding.boundary_contract_fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "selected root provider calling plan does not match the semantic bridge binding".into(),
        ));
    }
    Ok(())
}

impl ProgramStorageRootInput {
    pub const fn new(grant: ExtentRootGrant, base: u64, length: u64) -> Self {
        Self {
            grant,
            base,
            length,
        }
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub fn with_geometry(self, base: u64, length: u64) -> Self {
        Self {
            grant: self.grant,
            base,
            length,
        }
    }

    pub fn into_grant(self) -> ExtentRootGrant {
        self.grant
    }
}

/// The two non-duplicable `Extent::Granted` roots introduced by one installed
/// target entry. Their roles remain attached to the exact inherited semantic
/// positions used to capture them.
#[derive(Debug)]
pub struct InstalledProgramStorageRoots {
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: Extent,
    initial_storage: Option<Extent>,
    receiver_storage: Option<ReservedProgramEntryReceiverStorage>,
    initial_storage_record: ProgramStorageInstalledExtentRecord,
}

/// Conserved storage reserved for one future ZII entry-receiver occurrence.
/// The selected extent and every nonempty remainder retain the installed
/// initial-storage root's exact lineage.
#[derive(Debug)]
pub struct ReservedProgramEntryReceiverStorage {
    plan: ProgramEntryReceiverStoragePlan,
    placement: ProgramEntryReceiverPlacementRecord,
    partition: Option<OwnedExtentPartition>,
}

impl ReservedProgramEntryReceiverStorage {
    pub const fn plan(&self) -> &ProgramEntryReceiverStoragePlan {
        &self.plan
    }

    pub const fn placement(&self) -> &ProgramEntryReceiverPlacementRecord {
        &self.placement
    }

    pub fn storage(&self) -> Option<&Extent> {
        self.partition.as_ref().map(OwnedExtentPartition::selected)
    }

    pub fn before(&self) -> Option<&Extent> {
        self.partition
            .as_ref()
            .and_then(OwnedExtentPartition::before)
    }

    pub fn after(&self) -> Option<&Extent> {
        self.partition
            .as_ref()
            .and_then(OwnedExtentPartition::after)
    }
}

/// Program-storage roots whose successful installation has also been recorded
/// in the installing bridge's artifact directory.
///
/// Keeping this wrapper distinct from [`InstalledProgramStorageRoots`] prevents
/// an ordinary successful handoff from silently skipping the required
/// non-authoritative completion record.
#[derive(Debug)]
pub struct RecordedProgramStorageInstallation {
    roots: InstalledProgramStorageRoots,
}

/// Recorded installation of roots introduced from exact installed
/// program-local occurrences.
///
/// The registry is deliberately owned beside the Extents rather than hidden
/// behind their copyable origin rows. It retains both lifecycle leases through
/// record retry, receiver partitioning, and every borrowed observation of the
/// installation. No API releases the raw roots without their account owner.
pub type RecordedProgramLocalStorageInstallation<'root, 'code> =
    ProgramLocalStorageCustody<'root, 'code, RecordedProgramStorageInstallation>;

impl<'root, 'code> ProgramLocalStorageCustody<'root, 'code, RecordedProgramStorageInstallation> {
    pub const fn roots(&self) -> &InstalledProgramStorageRoots {
        self.stage().roots()
    }

    pub fn installation_record(&self) -> ProgramStorageInstallationRecord {
        self.stage().installation_record()
    }

    /// Release a receiver-free installation while retaining its program-local
    /// account owner beside the raw installed-root carrier.
    pub fn into_roots(
        self,
    ) -> Result<
        ProgramLocalStorageCustody<'root, 'code, InstalledProgramStorageRoots>,
        ProgramLocalStorageCustodyError<'root, 'code, RecordedProgramStorageInstallation>,
    > {
        let (installation, registry) = self.into_parts();
        match installation.into_roots() {
            Ok(roots) => Ok(ProgramLocalStorageCustody::new(roots, registry)),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                Err(ProgramLocalStorageCustodyError::new(
                    ProgramLocalStorageCustody::new(error.into_installation(), registry),
                    diagnostic,
                ))
            }
        }
    }

    /// Activate the exact receiver mapping without separating passive roots
    /// from the registry that owns their program-local accounts.
    pub fn activate_receiver<'mapping>(
        self,
        mapped_base: u64,
        mapped_storage: &'mapping mut [u8],
    ) -> Result<
        ProgramLocalEntryReceiverActivation<'mapping, 'root, 'code>,
        ProgramLocalEntryReceiverActivationError<'root, 'code>,
    > {
        let (installation, registry) = self.into_parts();
        match installation.activate_receiver(mapped_base, mapped_storage) {
            Ok(activation) => Ok(ProgramLocalStorageCustody::new(activation, registry)),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                Err(ProgramLocalStorageCustodyError::new(
                    ProgramLocalStorageCustody::new(error.into_installation(), registry),
                    diagnostic,
                ))
            }
        }
    }
}

impl RecordedProgramStorageInstallation {
    pub const fn roots(&self) -> &InstalledProgramStorageRoots {
        &self.roots
    }

    pub fn installation_record(&self) -> ProgramStorageInstallationRecord {
        self.roots.installation_record()
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.roots.provider_invocation
    }

    /// Release an installation that does not reserve an attached entry
    /// receiver. Receiver-bound installations must instead pass through
    /// [`RecordedProgramStorageInstallation::activate_receiver`], so callers
    /// cannot bypass the bridge's required ZII construction and exclusive
    /// activation loan.
    pub fn into_roots(
        self,
    ) -> Result<InstalledProgramStorageRoots, ProgramEntryReceiverActivationError> {
        if self.roots.receiver_storage.is_some() {
            return Err(ProgramEntryReceiverActivationError {
                installation: self,
                diagnostic: ProgramStorageEntryDiagnostic(
                    "receiver-bound program storage must be zeroed and activated before its roots are released"
                        .into(),
                ),
            });
        }
        Ok(self.roots)
    }

    /// Bind the exact mapped reservation, construct its checked ZII value, and
    /// lend it for this one source-entry activation.
    ///
    /// The backing slice is deliberately supplied by the installed physical
    /// bridge: an admitted numeric address alone is not a Rust memory mapping
    /// and cannot be dereferenced by the compiler. Exact base and length checks
    /// bind that physical mapping to the conserved reservation before any byte
    /// is written. Consuming `self` makes this the installation's only route to
    /// an activation; the returned token keeps the exclusive borrow live until
    /// the source activation finishes.
    pub fn activate_receiver(
        self,
        mapped_base: u64,
        mapped_storage: &mut [u8],
    ) -> Result<ProgramEntryReceiverActivation<'_>, ProgramEntryReceiverActivationError> {
        let Some(placement) = self
            .roots
            .receiver_storage
            .as_ref()
            .map(|receiver| receiver.placement.clone())
        else {
            return Err(ProgramEntryReceiverActivationError {
                installation: self,
                diagnostic: ProgramStorageEntryDiagnostic(
                    "program-storage installation has no entry receiver to activate".into(),
                ),
            });
        };
        let expected_length = match usize::try_from(placement.length) {
            Ok(length) => length,
            Err(_) => {
                return Err(ProgramEntryReceiverActivationError {
                    installation: self,
                    diagnostic: ProgramStorageEntryDiagnostic(
                        "entry receiver length does not fit the bridge address model".into(),
                    ),
                });
            }
        };
        if mapped_base != placement.base || mapped_storage.len() != expected_length {
            return Err(ProgramEntryReceiverActivationError {
                installation: self,
                diagnostic: ProgramStorageEntryDiagnostic(format!(
                    "entry receiver mapping must exactly cover 0x{:016x}..0x{:016x}, got base 0x{mapped_base:016x} and {} bytes",
                    placement.base,
                    placement.end(),
                    mapped_storage.len()
                )),
            });
        }

        mapped_storage.fill(0);
        Ok(ProgramEntryReceiverActivation {
            roots: self.roots,
            receiver: mapped_storage,
        })
    }
}

/// The one exclusive source-entry activation of a provisioned receiver.
///
/// This token is intentionally non-cloneable. Its lifetime holds the physical
/// bridge's exact mapped bytes exclusively, while its owned roots keep the
/// reservation and every conserved remainder alive.
#[derive(Debug)]
pub struct ProgramEntryReceiverActivation<'a> {
    roots: InstalledProgramStorageRoots,
    receiver: &'a mut [u8],
}

impl ProgramEntryReceiverActivation<'_> {
    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.roots.provider_invocation
    }

    pub const fn placement(&self) -> &ProgramEntryReceiverPlacementRecord {
        &self
            .roots
            .receiver_storage
            .as_ref()
            .expect("receiver activation retains its reserved storage")
            .placement
    }

    /// The ZII-initialized occurrence lent as the selected machine's one
    /// `&mut self` activation.
    pub fn receiver(&mut self) -> &mut [u8] {
        self.receiver
    }

    /// End the source activation and return the installed authority with its
    /// receiver reservation and all remainders still conserved.
    pub fn finish(self) -> InstalledProgramStorageRoots {
        self.roots
    }
}

pub type ProgramLocalEntryReceiverActivation<'mapping, 'root, 'code> =
    ProgramLocalStorageCustody<'root, 'code, ProgramEntryReceiverActivation<'mapping>>;

pub type ProgramLocalEntryReceiverActivationError<'root, 'code> =
    ProgramLocalStorageCustodyError<'root, 'code, RecordedProgramStorageInstallation>;

impl<'mapping, 'root, 'code>
    ProgramLocalStorageCustody<'root, 'code, ProgramEntryReceiverActivation<'mapping>>
{
    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.stage().provider_invocation()
    }

    pub const fn placement(&self) -> &ProgramEntryReceiverPlacementRecord {
        self.stage().placement()
    }

    pub fn receiver(&mut self) -> &mut [u8] {
        self.stage_mut().receiver()
    }

    pub fn finish(self) -> ProgramLocalStorageCustody<'root, 'code, InstalledProgramStorageRoots> {
        let (activation, registry) = self.into_parts();
        ProgramLocalStorageCustody::new(activation.finish(), registry)
    }
}

/// Failed receiver activation retains the recorded installation, allowing the
/// bridge to correct its mapping without replaying root admission.
#[derive(Debug)]
pub struct ProgramEntryReceiverActivationError {
    installation: RecordedProgramStorageInstallation,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramEntryReceiverActivationError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_installation(self) -> RecordedProgramStorageInstallation {
        self.installation
    }
}

impl std::fmt::Display for ProgramEntryReceiverActivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramEntryReceiverActivationError {}

/// A generated physical bridge failed while installing its selected provider,
/// activating its mapped receiver, or binding the post-activation receiver to
/// its retained outbound ABI. Only the installation and activation variants
/// retain the complete carrier needed to retry their respective transition.
#[derive(Debug)]
pub enum ProgramStorageEntryBridgeError {
    Installation(ProgramStorageInstallationHandoffError),
    Activation(ProgramEntryReceiverActivationError),
    ContinuationReceiverBinding(Box<ProgramStorageEntryContinuationReceiverBindingError>),
}

impl std::fmt::Display for ProgramStorageEntryBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Installation(error) => std::fmt::Display::fmt(error, formatter),
            Self::Activation(error) => std::fmt::Display::fmt(error, formatter),
            Self::ContinuationReceiverBinding(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for ProgramStorageEntryBridgeError {}

/// A post-activation receiver-ABI rejection returns the already-partitioned
/// installation: image, receiver reservation, and every conserved remainder.
/// It does not reconstruct a whole `InitialStorage` argument and therefore is
/// not, by itself, a retry carrier for a source call. The mutable mapping loan
/// is released when dispatch returns this error.
#[derive(Debug)]
pub struct ProgramStorageEntryContinuationReceiverBindingError {
    roots: InstalledProgramStorageRoots,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryContinuationReceiverBindingError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_roots(self) -> InstalledProgramStorageRoots {
        self.roots
    }
}

impl std::fmt::Display for ProgramStorageEntryContinuationReceiverBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryContinuationReceiverBindingError {}

/// Report-only identity and geometry of one installed program-storage root.
/// This value carries no grant and cannot recreate an [`Extent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageInstalledExtentRecord {
    base: u64,
    length: u64,
    address_space: psi_extents::AddressSpaceId,
    rights: Vec<psi_extents::ExtentRightId>,
    provenance: psi_extents::ExtentProvenanceId,
    mapping_era: psi_extents::MappingEraId,
    origin: psi_extents::ExtentRootOrigin,
    lineage_root: psi_extents::ExtentLineageId,
}

/// Non-authoritative placement of the storage reserved beneath the installed
/// initial-storage root for one entry-receiver occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryReceiverPlacementRecord {
    type_identity: String,
    base: u64,
    length: u64,
    alignment: u64,
    initial_storage_offset: u64,
    lineage_root: psi_extents::ExtentLineageId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProgramEntryReceiverPlacementGeometry {
    type_identity: String,
    base: u64,
    length: u64,
    alignment: u64,
    initial_storage_offset: u64,
}

impl ProgramEntryReceiverPlacementGeometry {
    fn bind_lineage(
        self,
        lineage_root: psi_extents::ExtentLineageId,
    ) -> ProgramEntryReceiverPlacementRecord {
        ProgramEntryReceiverPlacementRecord {
            type_identity: self.type_identity,
            base: self.base,
            length: self.length,
            alignment: self.alignment,
            initial_storage_offset: self.initial_storage_offset,
            lineage_root,
        }
    }
}

impl ProgramEntryReceiverPlacementRecord {
    pub fn type_identity(&self) -> &str {
        &self.type_identity
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn end(&self) -> u64 {
        self.base + self.length
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn initial_storage_offset(&self) -> u64 {
        self.initial_storage_offset
    }

    pub const fn lineage_root(&self) -> psi_extents::ExtentLineageId {
        self.lineage_root
    }

    #[cfg(test)]
    pub(super) fn for_test(type_identity: &str, base: u64, length: u64, alignment: u64) -> Self {
        Self {
            type_identity: type_identity.into(),
            base,
            length,
            alignment,
            initial_storage_offset: 0,
            lineage_root: psi_extents::ExtentLineageId::from_normalized_identity(1)
                .expect("test lineage"),
        }
    }
}

impl ProgramStorageInstalledExtentRecord {
    fn from_extent(extent: &Extent) -> Self {
        Self {
            base: extent.base(),
            length: extent.length(),
            address_space: extent.address_space(),
            rights: extent.rights().identities().collect(),
            provenance: extent.provenance(),
            mapping_era: extent.era(),
            origin: extent.origin(),
            lineage_root: extent.lineage_root(),
        }
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn end(&self) -> u64 {
        self.base + self.length
    }

    pub const fn address_space(&self) -> psi_extents::AddressSpaceId {
        self.address_space
    }

    pub fn rights(&self) -> &[psi_extents::ExtentRightId] {
        &self.rights
    }

    pub const fn provenance(&self) -> psi_extents::ExtentProvenanceId {
        self.provenance
    }

    pub const fn mapping_era(&self) -> psi_extents::MappingEraId {
        self.mapping_era
    }

    pub const fn origin(&self) -> psi_extents::ExtentRootOrigin {
        self.origin
    }

    pub const fn provider_issuance(&self) -> Option<psi_extents::ExtentProviderIssuance> {
        self.origin.provider_issuance()
    }

    pub const fn lineage_root(&self) -> psi_extents::ExtentLineageId {
        self.lineage_root
    }
}

/// Stable audit record produced only after both program-storage geometries
/// validate and both admitted grants are consumed. Cloning this record clones
/// observations, never authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageInstallationRecord {
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: ProgramStorageInstalledExtentRecord,
    initial_storage: ProgramStorageInstalledExtentRecord,
    receiver: Option<ProgramEntryReceiverPlacementRecord>,
}

impl ProgramStorageInstallationRecord {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &ProgramStorageInstalledExtentRecord {
        &self.image
    }

    pub const fn initial_storage(&self) -> &ProgramStorageInstalledExtentRecord {
        &self.initial_storage
    }

    pub const fn receiver(&self) -> Option<&ProgramEntryReceiverPlacementRecord> {
        self.receiver.as_ref()
    }
}

impl InstalledProgramStorageRoots {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &Extent {
        &self.image
    }

    /// Whole initial-storage authority is available only when the selected
    /// entry has no reserved receiver occurrence.
    pub const fn initial_storage(&self) -> Option<&Extent> {
        self.initial_storage.as_ref()
    }

    pub const fn receiver_storage(&self) -> Option<&ReservedProgramEntryReceiverStorage> {
        self.receiver_storage.as_ref()
    }

    pub fn installation_record(&self) -> ProgramStorageInstallationRecord {
        ProgramStorageInstallationRecord {
            binding: self.binding.clone(),
            provider_invocation: self.provider_invocation,
            image: ProgramStorageInstalledExtentRecord::from_extent(&self.image),
            initial_storage: self.initial_storage_record.clone(),
            receiver: self
                .receiver_storage
                .as_ref()
                .map(|receiver| receiver.placement.clone()),
        }
    }

    pub(super) fn into_root_authority_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        Option<ProgramStorageEntryProviderInvocation>,
        Extent,
        Option<Extent>,
        Option<ReservedProgramEntryReceiverStorage>,
    ) {
        let Self {
            binding,
            provider_invocation,
            image,
            initial_storage,
            receiver_storage,
            initial_storage_record: _,
        } = self;
        (
            binding,
            provider_invocation,
            image,
            initial_storage,
            receiver_storage,
        )
    }

    /// Derive a section/static view without splitting the installed image's
    /// ownership. Several disjoint or overlapping compiler views may coexist
    /// under the same one admitted image root.
    pub fn image_subextent(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<InstalledImageSubextent<'_>, ProgramStorageEntryDiagnostic> {
        let loan = self.image.loan(offset, length).map_err(|diagnostic| {
            ProgramStorageEntryDiagnostic(format!(
                "installed image subextent is outside the admitted image root: {diagnostic}"
            ))
        })?;
        Ok(InstalledImageSubextent {
            binding: &self.binding,
            loan,
        })
    }

    /// Split one independently owned allocation from initial storage while
    /// retaining the installed image and every storage remainder.
    pub fn partition_initial_storage(
        self,
        offset: u64,
        length: u64,
    ) -> Result<PartitionedProgramStorageRoots, Box<ProgramStoragePartitionError>> {
        let Self {
            binding,
            provider_invocation,
            image,
            initial_storage,
            receiver_storage,
            initial_storage_record,
        } = self;
        let Some(initial_storage) = initial_storage else {
            return Err(Box::new(ProgramStoragePartitionError {
                roots: Self {
                    binding,
                    provider_invocation,
                    image,
                    initial_storage,
                    receiver_storage,
                    initial_storage_record,
                },
                diagnostic: ProgramStorageEntryDiagnostic(
                    "initial storage already reserves the selected entry receiver".into(),
                ),
            }));
        };
        match initial_storage.partition_owned(offset, length) {
            Ok(initial_storage) => Ok(PartitionedProgramStorageRoots {
                binding,
                provider_invocation,
                image,
                initial_storage,
                initial_storage_record,
            }),
            Err(error) => {
                let diagnostic = ProgramStorageEntryDiagnostic(format!(
                    "initial-storage allocation cannot be derived: {}",
                    error.diagnostic()
                ));
                Err(Box::new(ProgramStoragePartitionError {
                    roots: Self {
                        binding,
                        provider_invocation,
                        image,
                        initial_storage: Some(error.into_extent()),
                        receiver_storage,
                        initial_storage_record,
                    },
                    diagnostic,
                }))
            }
        }
    }
}

/// Borrowed compiler-derived range within the one admitted program image.
pub struct InstalledImageSubextent<'a> {
    binding: &'a ProgramStorageEntryPlanBinding,
    loan: ExtentLoan<'a>,
}

impl<'a> InstalledImageSubextent<'a> {
    pub const fn binding(&self) -> &'a ProgramStorageEntryPlanBinding {
        self.binding
    }

    pub const fn loan(&self) -> &ExtentLoan<'a> {
        &self.loan
    }
}

impl std::fmt::Debug for InstalledImageSubextent<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledImageSubextent")
            .field("root_slot", &self.binding.root_slot)
            .field("requirement_identity", &self.binding.requirement_identity)
            .field("loan", &self.loan)
            .finish()
    }
}

/// One owned allocation plus all authority needed to conserve its installed
/// initial-storage parent.
#[derive(Debug)]
pub struct PartitionedProgramStorageRoots {
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: Extent,
    initial_storage: OwnedExtentPartition,
    initial_storage_record: ProgramStorageInstalledExtentRecord,
}

impl PartitionedProgramStorageRoots {
    pub const fn binding(&self) -> &ProgramStorageEntryPlanBinding {
        &self.binding
    }

    pub const fn provider_invocation(&self) -> Option<ProgramStorageEntryProviderInvocation> {
        self.provider_invocation
    }

    pub const fn image(&self) -> &Extent {
        &self.image
    }

    pub const fn allocation(&self) -> &Extent {
        self.initial_storage.selected()
    }

    pub const fn before(&self) -> Option<&Extent> {
        self.initial_storage.before()
    }

    pub const fn after(&self) -> Option<&Extent> {
        self.initial_storage.after()
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        Option<ProgramStorageEntryProviderInvocation>,
        Extent,
        OwnedExtentPartition,
    ) {
        (
            self.binding,
            self.provider_invocation,
            self.image,
            self.initial_storage,
        )
    }

    pub fn rejoin(self) -> InstalledProgramStorageRoots {
        InstalledProgramStorageRoots {
            binding: self.binding,
            provider_invocation: self.provider_invocation,
            image: self.image,
            initial_storage: Some(self.initial_storage.rejoin()),
            receiver_storage: None,
            initial_storage_record: self.initial_storage_record,
        }
    }
}

/// Bind a selected target entry to the stable core storage requirement.
///
/// Only the core requirement owner and semantic domain are recognized. The
/// selected target trait's name, source parameter names, registers, and stack
/// offsets play no role in identifying image versus initial storage.
pub fn bind_program_storage_entry_plan(
    selected: &SelectedProgramStorageEntryPlan,
    boundary: &ValidatedBoundaryEntryPlan,
    storage: &DerivedBoundaryEntryStorage,
) -> Result<ProgramStorageEntryPlanBinding, ProgramStorageEntryDiagnostic> {
    let matches = selected
        .schema
        .methods
        .iter()
        .filter(|method| method.requirement_identity == selected.requirement_identity)
        .collect::<Vec<_>>();
    let [method] = matches.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(match matches.len() {
            0 => "selected target entry lost its exact program-storage arrival requirement".into(),
            count => format!(
                "selected target entry carries {count} copies of its exact program-storage arrival requirement"
            ),
        }));
    };
    if method.requirement_owner != PROGRAM_STORAGE_ENTRY_OWNER
        || method.name != PROGRAM_STORAGE_ENTRY_METHOD
    {
        return Err(ProgramStorageEntryDiagnostic(
            "selected target entry arrival requirement drifted from ProgramStorageEntry::enter"
                .into(),
        ));
    }
    if method.parameter_count != 2 || method.parameter_type_identities.len() != 2 {
        return Err(ProgramStorageEntryDiagnostic(
            "ProgramStorageEntry::enter must retain exactly two semantic parameters".into(),
        ));
    }
    let boundary_fingerprint = boundary.contract_fingerprint();
    if method.calling_plan_fingerprint != Some(boundary_fingerprint) {
        return Err(ProgramStorageEntryDiagnostic(
            "selected target-entry schema is not bound to this validated boundary plan".into(),
        ));
    }
    if boundary.plan().call.parameters.len() != 2 {
        return Err(ProgramStorageEntryDiagnostic(
            "validated program-storage boundary plan must place exactly two parameters".into(),
        ));
    }

    let image = bind_parameter(method, boundary, storage, IMAGE_PARAMETER_INDEX)?;
    let initial_storage =
        bind_parameter(method, boundary, storage, INITIAL_STORAGE_PARAMETER_INDEX)?;
    Ok(ProgramStorageEntryPlanBinding {
        root_slot: selected.root_slot,
        requirement_identity: method.requirement_identity.clone(),
        boundary_contract_fingerprint: boundary_fingerprint,
        image,
        initial_storage,
        receiver: None,
        source_signature: None,
        physical_contract: selected.physical_contract.clone(),
    })
}

/// Join an optional compiler selection to its exact checked source and
/// backend-owned entry plan. Absence does not inspect or require backend
/// storage; presence projects every generated input through the retained plan.
pub(super) fn bind_compiler_generated_program_storage_entry_plan(
    selected: Option<&SelectedProgramStorageEntryPlan>,
    source_signature: Option<&super::SelectedProgramEntrySourceSignature>,
    backend: &omega_backend_plan::BackendPlan,
) -> Result<Option<ProgramStorageEntryPlanBinding>, Vec<Diagnostic>> {
    let Some(selected) = selected else {
        return Ok(None);
    };
    let source_signature = source_signature.ok_or_else(|| {
        vec![Diagnostic::error(
            "selected program-storage entry lost its checked source signature before backend binding",
        )]
    })?;
    let plan = backend.entry_boundary_plan.as_ref().ok_or_else(|| {
        vec![Diagnostic::error(
            "selected program-storage entry lost its retained calling plan before backend binding",
        )]
    })?;

    bind_generated_program_storage_entry_plan(
        selected,
        plan,
        &backend.runtime_storage,
        &backend.layouts,
        backend.entry_key,
        source_signature,
    )
    .map(Some)
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])
}

/// Join the selected root slot to the concrete entry-frame captures generated
/// for its source continuation. Parameter order comes from the checked entry
/// state; ABI shapes and placements come only from the retained evaluated plan.
pub fn bind_generated_program_storage_entry_plan(
    selected: &SelectedProgramStorageEntryPlan,
    plan: &omega_calling_conventions::BoundaryEntryPlan,
    runtime_storage: &omega_runtime_storage::RuntimeStoragePlan,
    layouts: &omega_layout::LayoutPlan,
    entry_key: omega_control_flow::StateKey,
    source_signature: &super::SelectedProgramEntrySourceSignature,
) -> Result<ProgramStorageEntryPlanBinding, ProgramStorageEntryDiagnostic> {
    let signature = omega_calling_conventions::CallSignature {
        parameters: plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: plan.call.result.as_ref().map(|placement| placement.shape),
    };
    let boundary =
        omega_calling_conventions::validate_boundary_entry_plan(plan.clone(), &signature).map_err(
            |diagnostic| {
                ProgramStorageEntryDiagnostic(format!(
                    "retained program-entry calling plan is invalid: {diagnostic}"
                ))
            },
        )?;
    let mut slots = runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            (slot.source_key == entry_key
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::Parameter
                ))
            .then_some(slot)
        })
        .collect::<Vec<_>>();
    slots.sort_unstable_by_key(|slot| slot.byte_offset);
    if slots.len() != boundary.plan().call.parameters.len() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "generated program entry retains {} parameter slots for {} calling-plan parameters",
            slots.len(),
            boundary.plan().call.parameters.len()
        )));
    }
    let mut destinations = Vec::with_capacity(slots.len());
    for (index, (slot, placement)) in slots
        .into_iter()
        .zip(boundary.plan().call.parameters.iter())
        .enumerate()
    {
        if slot.byte_size != usize::from(placement.shape.byte_size) {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "generated program-entry parameter {index} reserves {} bytes, but its retained calling plan places {}",
                slot.byte_size, placement.shape.byte_size
            )));
        }
        destinations.push((slot.byte_offset, placement.shape));
    }
    let storage = omega_instruction_selection::derive_boundary_entry_storage(
        boundary.plan(),
        &destinations,
        None,
        None,
    )
    .map_err(|diagnostic| {
        ProgramStorageEntryDiagnostic(format!(
            "cannot derive generated program-entry captures: {diagnostic}"
        ))
    })?;
    let mut binding = bind_program_storage_entry_plan(selected, &boundary, &storage)?;
    let receiver_type_identity = source_signature.receiver().normalized_type_identity();
    source_signature
        .validate_program_storage_binding(
            selected.target_slot(),
            entry_key,
            receiver_type_identity,
            binding.image().parameter_type_identity(),
            binding.initial_storage().parameter_type_identity(),
        )
        .map_err(ProgramStorageEntryDiagnostic)?;
    for parameter in source_signature.visible_parameters() {
        parameter
            .extent_value_layout()
            .validate_backend_layout(layouts)
            .map_err(|diagnostic| {
                ProgramStorageEntryDiagnostic(format!(
                    "selected program-storage {:?} value layout failed backend replay: {diagnostic}",
                    parameter.role()
                ))
            })?;
    }
    if let Some(type_identity) = receiver_type_identity {
        let layout = layouts
            .machine_layouts
            .iter()
            .find_map(|(_, layout)| (layout.symbol == entry_key.machine).then_some(layout.layout))
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "selected entry receiver has no concrete machine layout".into(),
                )
            })?;
        binding = binding.with_checked_receiver_layout(type_identity.to_owned(), layout)?;
    }
    binding.source_signature = Some(source_signature.clone());
    Ok(binding)
}

fn bind_parameter(
    method: &omega_effects::provider_plan::ServiceMethod,
    boundary: &ValidatedBoundaryEntryPlan,
    storage: &DerivedBoundaryEntryStorage,
    parameter_index: usize,
) -> Result<ProgramStorageEntryParameter, ProgramStorageEntryDiagnostic> {
    let claims = method
        .entry_claims
        .iter()
        .filter(|claim| claim.parameter_index == parameter_index)
        .collect::<Vec<_>>();
    let [claim] = claims.as_slice() else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "ProgramStorageEntry::enter parameter {parameter_index} carries {} routed entry claims instead of exactly one",
            claims.len()
        )));
    };
    if claim.carrier_identity != EXTENT_CARRIER
        || claim.domain != GRANTED_DOMAIN
        || !claim.predicate_body.is_present()
        || claim.effective_carry != psi_language_semantics::CarryPolicy::STRICT
        || claim.authority_flow != ServiceEntryAuthorityFlow::Accepts
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "ProgramStorageEntry::enter parameter {parameter_index} does not carry the exact strict accepted Extent::Granted claim"
        )));
    }
    let placement = boundary
        .plan()
        .call
        .parameters
        .get(parameter_index)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(format!(
                "validated boundary plan has no placement for program-storage parameter {parameter_index}"
            ))
        })?;
    let capture = storage.parameter(parameter_index).ok_or_else(|| {
        ProgramStorageEntryDiagnostic(format!(
            "generated entry prologue has no capture for program-storage parameter {parameter_index}"
        ))
    })?;
    if capture.placement != *placement {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "generated entry capture for program-storage parameter {parameter_index} drifted from the validated boundary plan"
        )));
    }
    Ok(ProgramStorageEntryParameter {
        parameter_index,
        carrier_identity: claim.carrier_identity.clone(),
        parameter_type_identity: method.parameter_type_identities[parameter_index].clone(),
        domain: claim.domain.clone(),
        effective_carry: claim.effective_carry,
        placement: placement.clone(),
        destination_byte_offset: capture.destination_byte_offset,
        write_range: capture.write_range.clone(),
    })
}

/// Validate both `no_wrap` obligations before importing either complete root,
/// then consume the two provider-admitted grants in semantic position order.
fn install_program_storage_entry_provider_roots_unrecorded(
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: ProgramStorageEntryProviderInvocation,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
) -> Result<InstalledProgramStorageRoots, Box<ProgramStorageRootInstallationError>> {
    let validation = validate_physical_provider_root("image", &image, provider_invocation)
        .and_then(|()| {
            validate_physical_provider_root(
                "initial-storage",
                &initial_storage,
                provider_invocation,
            )
        });
    if let Err(diagnostic) = validation {
        return Err(Box::new(ProgramStorageRootInstallationError {
            binding,
            image,
            initial_storage,
            diagnostic,
        }));
    }

    let (image_geometry, storage_geometry, receiver_placement) =
        match validate_program_storage_entry_geometry(
            &binding,
            image.base,
            image.length,
            initial_storage.base,
            initial_storage.length,
        ) {
            Ok(validated) => validated,
            Err(diagnostic) => {
                return Err(Box::new(ProgramStorageRootInstallationError {
                    binding,
                    image,
                    initial_storage,
                    diagnostic,
                }));
            }
        };

    let image = image.grant.mint_validated(image_geometry);
    let initial_storage = initial_storage.grant.mint_validated(storage_geometry);
    Ok(assemble_program_storage_entry_extents(
        binding,
        Some(provider_invocation),
        image,
        initial_storage,
        receiver_placement,
    ))
}

fn validate_program_storage_entry_geometry(
    binding: &ProgramStorageEntryPlanBinding,
    image_base: u64,
    image_length: u64,
    storage_base: u64,
    storage_length: u64,
) -> Result<
    (
        ValidatedExtentGeometry,
        ValidatedExtentGeometry,
        Option<ProgramEntryReceiverPlacementGeometry>,
    ),
    ProgramStorageEntryDiagnostic,
> {
    let image_geometry =
        ValidatedExtentGeometry::check(image_base, image_length).map_err(|diagnostic| {
            ProgramStorageEntryDiagnostic(format!(
                "image root does not satisfy Extent::Granted no_wrap: {diagnostic}"
            ))
        })?;
    let storage_geometry =
        ValidatedExtentGeometry::check(storage_base, storage_length).map_err(|diagnostic| {
            ProgramStorageEntryDiagnostic(format!(
                "initial-storage root does not satisfy Extent::Granted no_wrap: {diagnostic}"
            ))
        })?;
    let receiver_placement = binding
        .receiver
        .as_ref()
        .map(|receiver| receiver_placement(receiver, storage_base, storage_length))
        .transpose()?;
    Ok((image_geometry, storage_geometry, receiver_placement))
}

fn assemble_program_storage_entry_extents(
    binding: ProgramStorageEntryPlanBinding,
    provider_invocation: Option<ProgramStorageEntryProviderInvocation>,
    image: Extent,
    initial_storage: Extent,
    receiver_placement: Option<ProgramEntryReceiverPlacementGeometry>,
) -> InstalledProgramStorageRoots {
    let receiver_placement =
        receiver_placement.map(|placement| placement.bind_lineage(initial_storage.lineage_root()));
    let initial_storage_record = ProgramStorageInstalledExtentRecord::from_extent(&initial_storage);
    let (initial_storage, receiver_storage) = match receiver_placement {
        Some(placement) if placement.length != 0 => {
            let partition = initial_storage
                .partition_owned(placement.initial_storage_offset, placement.length)
                .expect("receiver placement was validated before consuming either grant");
            let plan = binding
                .receiver
                .clone()
                .expect("receiver placement requires a checked receiver plan");
            (
                None,
                Some(ReservedProgramEntryReceiverStorage {
                    plan,
                    placement,
                    partition: Some(partition),
                }),
            )
        }
        Some(placement) => {
            let plan = binding
                .receiver
                .clone()
                .expect("receiver placement requires a checked receiver plan");
            (
                Some(initial_storage),
                Some(ReservedProgramEntryReceiverStorage {
                    plan,
                    placement,
                    partition: None,
                }),
            )
        }
        None => (Some(initial_storage), None),
    };

    InstalledProgramStorageRoots {
        binding,
        provider_invocation,
        image,
        initial_storage,
        receiver_storage,
        initial_storage_record,
    }
}

fn validate_physical_provider_root(
    role: &str,
    input: &ProgramStorageRootInput,
    selected: ProgramStorageEntryProviderInvocation,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let Some(issuance) = input.grant.origin().provider_issuance() else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "{role} root is not issued by the selected root-provider invocation"
        )));
    };
    if !selected.matches(issuance) {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "{role} root does not belong to the selected root provider, plan, and invocation"
        )));
    }
    Ok(())
}

fn receiver_placement(
    plan: &ProgramEntryReceiverStoragePlan,
    storage_base: u64,
    storage_length: u64,
) -> Result<ProgramEntryReceiverPlacementGeometry, ProgramStorageEntryDiagnostic> {
    let alignment = u64::try_from(plan.byte_alignment).map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "entry receiver alignment does not fit the target address model".into(),
        )
    })?;
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "entry receiver requires invalid {alignment}-byte alignment"
        )));
    }
    let length = u64::try_from(plan.byte_size).map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "entry receiver size does not fit the target address model".into(),
        )
    })?;
    let aligned_base = storage_base
        .checked_add(alignment - 1)
        .map(|address| address & !(alignment - 1))
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "initial-storage base cannot be aligned for the selected entry receiver".into(),
            )
        })?;
    let offset = aligned_base - storage_base;
    let end = offset.checked_add(length).ok_or_else(|| {
        ProgramStorageEntryDiagnostic("entry receiver storage range overflows".into())
    })?;
    if end > storage_length {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "initial storage cannot reserve the selected entry receiver: aligned range {offset}..{end} exceeds {storage_length} bytes"
        )));
    }
    Ok(ProgramEntryReceiverPlacementGeometry {
        type_identity: plan.type_identity.clone(),
        base: aligned_base,
        length,
        alignment,
        initial_storage_offset: offset,
    })
}

/// Establish, materialize, install, and record the exact two program-local
/// roots supplied by one generated program-entry activation.
///
/// Preflight precedes the atomic cohort transition, so geometry or receiver
/// rejection leaves both occurrences dormant. Each later failure carrier
/// returns custody in the representation valid for that phase.
#[allow(clippy::too_many_arguments)]
pub fn establish_program_storage_entry_program_local_roots<'root, 'code>(
    artifact_directory: &Path,
    binding: ProgramStorageEntryPlanBinding,
    installation: &mut ProgramLocalRootInstallationLedger,
    runtime: &mut ProgramLocalRootEpochRuntime<'root, 'code>,
    lifecycle: &ComponentEraEntryLedger,
    image_subject: InstalledProgramLocalRootSubject<'root, 'code>,
    image_plan: ProgramLocalExtentMaterializationPlan,
    initial_storage_subject: InstalledProgramLocalRootSubject<'root, 'code>,
    initial_storage_plan: ProgramLocalExtentMaterializationPlan,
) -> Result<
    RecordedProgramLocalStorageInstallation<'root, 'code>,
    ProgramLocalStorageInstallationHandoffError<'root, 'code>,
> {
    let subjects = vec![image_subject, initial_storage_subject];
    let plans = [image_plan, initial_storage_plan];
    let preflight = validate_program_local_plan_role(binding.image(), &plans[0])
        .and_then(|()| validate_program_local_plan_role(binding.initial_storage(), &plans[1]))
        .and_then(|()| {
            validate_program_storage_entry_geometry(
                &binding,
                plans[0].base(),
                plans[0].length(),
                plans[1].base(),
                plans[1].length(),
            )
            .map(|_| ())
        });
    if let Err(diagnostic) = preflight {
        return Err(ProgramLocalStorageInstallationHandoffError::Subject(
            Box::new(ProgramLocalStorageSubjectHandoffError {
                binding,
                subjects,
                plans,
                diagnostic,
            }),
        ));
    }

    let established = match installation.establish_batch(runtime, lifecycle, subjects) {
        Ok(established) => established,
        Err(error) => {
            let diagnostic = error.diagnostic().clone();
            return Err(ProgramLocalStorageInstallationHandoffError::Subject(
                Box::new(ProgramLocalStorageSubjectHandoffError {
                    binding,
                    subjects: (*error).into_subjects(),
                    plans,
                    diagnostic: ProgramStorageEntryDiagnostic(diagnostic.0),
                }),
            ));
        }
    };
    let [image, initial_storage]: [EstablishedProgramLocalRoot<'root, 'code>; 2] = established
        .try_into()
        .expect("two generated program-storage subjects establish two exact accounts");
    install_established_program_storage_entry_program_local_roots(
        artifact_directory,
        binding,
        [
            (image, plans[0].clone()),
            (initial_storage, plans[1].clone()),
        ],
    )
}

/// Install two already-established accounts. This is also the retry boundary
/// after materialization rejection; it never replays cohort establishment.
pub fn install_established_program_storage_entry_program_local_roots<'root, 'code>(
    artifact_directory: &Path,
    binding: ProgramStorageEntryPlanBinding,
    inputs: [(
        EstablishedProgramLocalRoot<'root, 'code>,
        ProgramLocalExtentMaterializationPlan,
    ); 2],
) -> Result<
    RecordedProgramLocalStorageInstallation<'root, 'code>,
    ProgramLocalStorageInstallationHandoffError<'root, 'code>,
> {
    let [image, initial_storage] = &inputs;
    let validation = validate_program_local_plan_role(binding.image(), &image.1)
        .and_then(|()| {
            validate_program_local_plan_role(binding.initial_storage(), &initial_storage.1)
        })
        .and_then(|()| validate_program_local_account_role(&binding, binding.image(), &image.0))
        .and_then(|()| {
            validate_program_local_account_role(
                &binding,
                binding.initial_storage(),
                &initial_storage.0,
            )
        })
        .and_then(|()| validate_program_local_account_pair(&image.0, &initial_storage.0));
    let (_, _, receiver_placement) = match validation.and_then(|()| {
        validate_program_storage_entry_geometry(
            &binding,
            image.1.base(),
            image.1.length(),
            initial_storage.1.base(),
            initial_storage.1.length(),
        )
    }) {
        Ok(validated) => validated,
        Err(diagnostic) => {
            return Err(ProgramLocalStorageInstallationHandoffError::Account(
                Box::new(ProgramLocalStorageAccountHandoffError {
                    binding,
                    inputs: Vec::from(inputs),
                    diagnostic,
                }),
            ));
        }
    };

    let mut registry = ProgramLocalExtentRegistry::new();
    let extents = match registry.materialize_batch(Vec::from(inputs)) {
        Ok(extents) => extents,
        Err(error) => {
            let diagnostic = error.diagnostic().clone();
            return Err(ProgramLocalStorageInstallationHandoffError::Account(
                Box::new(ProgramLocalStorageAccountHandoffError {
                    binding,
                    inputs: (*error).into_inputs(),
                    diagnostic: ProgramStorageEntryDiagnostic(diagnostic.0),
                }),
            ));
        }
    };
    let [image, initial_storage]: [Extent; 2] = extents
        .try_into()
        .expect("two exact program-local accounts materialize two Extents");
    let roots = assemble_program_storage_entry_extents(
        binding,
        None,
        image,
        initial_storage,
        receiver_placement,
    );
    record_program_local_storage_installation(artifact_directory, roots, registry)
}

fn validate_program_local_plan_role(
    parameter: &ProgramStorageEntryParameter,
    plan: &ProgramLocalExtentMaterializationPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    if plan.carrier_identity() != parameter.carrier_identity()
        || plan.qualification_identity() != parameter.domain()
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-local Extent plan for semantic parameter {} substituted its exact carrier or qualification",
            parameter.parameter_index()
        )));
    }
    Ok(())
}

fn validate_program_local_account_role(
    binding: &ProgramStorageEntryPlanBinding,
    parameter: &ProgramStorageEntryParameter,
    root: &EstablishedProgramLocalRoot<'_, '_>,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let prebinding = root.prebinding();
    if prebinding.requirement_identity() != binding.requirement_identity()
        || prebinding.identity().slot() != binding.root_slot()
        || usize::try_from(prebinding.argument_index()).ok() != Some(parameter.parameter_index())
        || usize::try_from(prebinding.source_parameter_position()).ok()
            != Some(parameter.parameter_index())
        || prebinding.carrier_identity() != parameter.carrier_identity()
        || prebinding.qualification_identity() != parameter.domain()
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "established program-local account does not belong to exact program-storage parameter {}",
            parameter.parameter_index()
        )));
    }
    Ok(())
}

fn validate_program_local_account_pair(
    image: &EstablishedProgramLocalRoot<'_, '_>,
    initial_storage: &EstablishedProgramLocalRoot<'_, '_>,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let image_occurrence = image.occurrence_identity();
    let storage_occurrence = initial_storage.occurrence_identity();
    if image_occurrence == storage_occurrence
        || image_occurrence.prebinding().installed_code()
            != storage_occurrence.prebinding().installed_code()
        || image_occurrence.prebinding().root() != storage_occurrence.prebinding().root()
        || image_occurrence.prebinding().slot() != storage_occurrence.prebinding().slot()
        || image_occurrence.lifecycle_ledger() != storage_occurrence.lifecycle_ledger()
        || image_occurrence.lifecycle_epoch() != storage_occurrence.lifecycle_epoch()
        || image.invocation() != initial_storage.invocation()
        || image.subject_place() == initial_storage.subject_place()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage roots must be distinct positions from one exact installed entry activation and lifecycle epoch"
                .into(),
        ));
    }
    Ok(())
}

/// Install the two roots supplied by one exact selected root-provider
/// invocation and retain that occurrence identity through audit and entry
/// activation.
///
/// This is the production-facing installation carrier consumed by a generated
/// bridge. It does not itself emit or invoke native code. Unlike the local
/// handoff seam, it rejects program-local grants and roots from
/// another provider plan or invocation before consuming either grant.
pub fn install_program_storage_entry_provider_invocation(
    artifact_directory: &Path,
    binding: ProgramStorageEntryPlanBinding,
    selected_provider: &super::provider_plans::SelectedExternalRootProviderPlan,
    provider_issuance: psi_extents::ExtentProviderIssuance,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
) -> Result<RecordedProgramStorageInstallation, ProgramStorageInstallationHandoffError> {
    let provider_invocation = match ProgramStorageEntryProviderInvocation::bind_selected_provider(
        &binding,
        selected_provider,
        provider_issuance,
    ) {
        Ok(invocation) => invocation,
        Err(diagnostic) => {
            return Err(ProgramStorageInstallationHandoffError::Rejected(Box::new(
                ProgramStorageRootInstallationError {
                    binding,
                    image,
                    initial_storage,
                    diagnostic,
                },
            )));
        }
    };
    let roots = install_program_storage_entry_provider_roots_unrecorded(
        binding,
        provider_invocation,
        image,
        initial_storage,
    )
    .map_err(ProgramStorageInstallationHandoffError::Rejected)?;
    record_program_storage_installation(artifact_directory, roots)
}

/// Prepare a receiver-bearing generated bridge in one linear handoff: install
/// exact invocation roots, emit the completion audit, bind the exact mapped
/// reservation, construct its ZII value, and return the exclusive activation
/// loan that a native bridge caller must pass to the source continuation.
pub fn install_and_activate_program_storage_entry_receiver<'mapping>(
    artifact_directory: &Path,
    binding: ProgramStorageEntryPlanBinding,
    selected_provider: &super::provider_plans::SelectedExternalRootProviderPlan,
    provider_issuance: psi_extents::ExtentProviderIssuance,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
    mapped_base: u64,
    mapped_storage: &'mapping mut [u8],
) -> Result<ProgramEntryReceiverActivation<'mapping>, ProgramStorageEntryBridgeError> {
    let installation = install_program_storage_entry_provider_invocation(
        artifact_directory,
        binding,
        selected_provider,
        provider_issuance,
        image,
        initial_storage,
    )
    .map_err(ProgramStorageEntryBridgeError::Installation)?;
    installation
        .activate_receiver(mapped_base, mapped_storage)
        .map_err(ProgramStorageEntryBridgeError::Activation)
}

fn record_program_storage_installation(
    artifact_directory: &Path,
    roots: InstalledProgramStorageRoots,
) -> Result<RecordedProgramStorageInstallation, ProgramStorageInstallationHandoffError> {
    let record = roots.installation_record();
    match super::artifacts::write_program_storage_installation_record(artifact_directory, &record) {
        Ok(()) => Ok(RecordedProgramStorageInstallation { roots }),
        Err(diagnostic) => Err(ProgramStorageInstallationHandoffError::Record(Box::new(
            ProgramStorageRecordEmissionError { roots, diagnostic },
        ))),
    }
}

fn record_program_local_storage_installation<'root, 'code>(
    artifact_directory: &Path,
    roots: InstalledProgramStorageRoots,
    registry: ProgramLocalExtentRegistry<'root, 'code>,
) -> Result<
    RecordedProgramLocalStorageInstallation<'root, 'code>,
    ProgramLocalStorageInstallationHandoffError<'root, 'code>,
> {
    let record = roots.installation_record();
    match super::artifacts::write_program_storage_installation_record(artifact_directory, &record) {
        Ok(()) => Ok(ProgramLocalStorageCustody::new(
            RecordedProgramStorageInstallation { roots },
            registry,
        )),
        Err(diagnostic) => Err(ProgramLocalStorageInstallationHandoffError::Record(
            Box::new(ProgramLocalStorageRecordEmissionError {
                roots,
                registry,
                diagnostic,
            }),
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryDiagnostic(pub String);

impl std::fmt::Display for ProgramStorageEntryDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProgramStorageEntryDiagnostic {}

/// A generated program-local handoff failed before establishment, after
/// establishment, or after installation while persisting its audit record.
/// Each variant retains the exact linear inputs owned at that phase.
#[derive(Debug)]
pub enum ProgramLocalStorageInstallationHandoffError<'root, 'code> {
    Subject(Box<ProgramLocalStorageSubjectHandoffError<'root, 'code>>),
    Account(Box<ProgramLocalStorageAccountHandoffError<'root, 'code>>),
    Record(Box<ProgramLocalStorageRecordEmissionError<'root, 'code>>),
}

impl std::fmt::Display for ProgramLocalStorageInstallationHandoffError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Subject(error) => std::fmt::Display::fmt(error, formatter),
            Self::Account(error) => std::fmt::Display::fmt(error, formatter),
            Self::Record(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for ProgramLocalStorageInstallationHandoffError<'_, '_> {}

/// Rejection before the installed entry subjects have been established.
/// The two subjects and their materialization plans remain available to the
/// generated bridge; no program-local root occurrence was introduced.
#[derive(Debug)]
pub struct ProgramLocalStorageSubjectHandoffError<'root, 'code> {
    binding: ProgramStorageEntryPlanBinding,
    subjects: Vec<InstalledProgramLocalRootSubject<'root, 'code>>,
    plans: [ProgramLocalExtentMaterializationPlan; 2],
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl<'root, 'code> ProgramLocalStorageSubjectHandoffError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        Vec<InstalledProgramLocalRootSubject<'root, 'code>>,
        [ProgramLocalExtentMaterializationPlan; 2],
    ) {
        (self.binding, self.subjects, self.plans)
    }
}

impl std::fmt::Display for ProgramLocalStorageSubjectHandoffError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramLocalStorageSubjectHandoffError<'_, '_> {}

/// Rejection after the entry cohort has established its exact root accounts
/// but before those accounts have become installed Extent values. Retrying
/// this carrier never replays cohort establishment.
#[derive(Debug)]
pub struct ProgramLocalStorageAccountHandoffError<'root, 'code> {
    binding: ProgramStorageEntryPlanBinding,
    inputs: Vec<(
        EstablishedProgramLocalRoot<'root, 'code>,
        ProgramLocalExtentMaterializationPlan,
    )>,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl<'root, 'code> ProgramLocalStorageAccountHandoffError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        Vec<(
            EstablishedProgramLocalRoot<'root, 'code>,
            ProgramLocalExtentMaterializationPlan,
        )>,
    ) {
        (self.binding, self.inputs)
    }

    pub fn retry(
        self,
        artifact_directory: &Path,
    ) -> Result<
        RecordedProgramLocalStorageInstallation<'root, 'code>,
        ProgramLocalStorageInstallationHandoffError<'root, 'code>,
    > {
        let inputs = self
            .inputs
            .try_into()
            .expect("program-storage account rejection always retains two exact inputs");
        install_established_program_storage_entry_program_local_roots(
            artifact_directory,
            self.binding,
            inputs,
        )
    }
}

impl std::fmt::Display for ProgramLocalStorageAccountHandoffError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramLocalStorageAccountHandoffError<'_, '_> {}

/// Installed program-local roots and their account registry retained across
/// an audit-artifact write failure. A successful retry is the only route back
/// to a usable installation aggregate.
#[derive(Debug)]
pub struct ProgramLocalStorageRecordEmissionError<'root, 'code> {
    roots: InstalledProgramStorageRoots,
    registry: ProgramLocalExtentRegistry<'root, 'code>,
    diagnostic: psi_diagnostics::Diagnostic,
}

impl<'root, 'code> ProgramLocalStorageRecordEmissionError<'root, 'code> {
    pub const fn diagnostic(&self) -> &psi_diagnostics::Diagnostic {
        &self.diagnostic
    }

    pub const fn roots(&self) -> &InstalledProgramStorageRoots {
        &self.roots
    }

    pub const fn registry(&self) -> &ProgramLocalExtentRegistry<'root, 'code> {
        &self.registry
    }

    pub fn retry(
        self,
        artifact_directory: &Path,
    ) -> Result<
        RecordedProgramLocalStorageInstallation<'root, 'code>,
        ProgramLocalStorageInstallationHandoffError<'root, 'code>,
    > {
        record_program_local_storage_installation(artifact_directory, self.roots, self.registry)
    }
}

impl std::fmt::Display for ProgramLocalStorageRecordEmissionError<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramLocalStorageRecordEmissionError<'_, '_> {}

#[derive(Debug)]
pub struct ProgramStorageRootInstallationError {
    binding: ProgramStorageEntryPlanBinding,
    image: ProgramStorageRootInput,
    initial_storage: ProgramStorageRootInput,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageRootInstallationError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramStorageEntryPlanBinding,
        ProgramStorageRootInput,
        ProgramStorageRootInput,
    ) {
        (self.binding, self.image, self.initial_storage)
    }
}

impl std::fmt::Display for ProgramStorageRootInstallationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageRootInstallationError {}

/// A program-storage handoff either failed before consuming its grants or
/// installed them but could not yet persist its completion record.
#[derive(Debug)]
pub enum ProgramStorageInstallationHandoffError {
    Rejected(Box<ProgramStorageRootInstallationError>),
    Record(Box<ProgramStorageRecordEmissionError>),
}

impl std::fmt::Display for ProgramStorageInstallationHandoffError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(error) => std::fmt::Display::fmt(error, formatter),
            Self::Record(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for ProgramStorageInstallationHandoffError {}

/// Installed authority retained across an audit-artifact write failure.
/// Retrying the write is the only route back to usable installed roots.
#[derive(Debug)]
pub struct ProgramStorageRecordEmissionError {
    roots: InstalledProgramStorageRoots,
    diagnostic: psi_diagnostics::Diagnostic,
}

impl ProgramStorageRecordEmissionError {
    pub const fn diagnostic(&self) -> &psi_diagnostics::Diagnostic {
        &self.diagnostic
    }

    pub fn retry(
        self,
        artifact_directory: &Path,
    ) -> Result<RecordedProgramStorageInstallation, ProgramStorageInstallationHandoffError> {
        record_program_storage_installation(artifact_directory, self.roots)
    }
}

impl std::fmt::Display for ProgramStorageRecordEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageRecordEmissionError {}

#[derive(Debug)]
pub struct ProgramStoragePartitionError {
    roots: InstalledProgramStorageRoots,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStoragePartitionError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_roots(self) -> InstalledProgramStorageRoots {
        self.roots
    }
}

impl std::fmt::Display for ProgramStoragePartitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStoragePartitionError {}

#[cfg(test)]
mod tests {
    use super::{
        ProgramStorageEntryParameter, ProgramStorageEntryPlanBinding,
        SelectedProgramStorageEntryPlan,
        bind_compiler_generated_program_storage_entry_native_bridge,
        bind_compiler_generated_program_storage_entry_plan,
        compiler_generated_program_storage_target_profile,
        retain_compiler_generated_program_storage_entry_publication_evidence,
        validate_encoded_program_storage_entry,
    };
    use omega_backend_plan::{BackendArtifactRoots, BackendPlan, BackendPlanPhaseTiming};
    use omega_calling_conventions::{ValuePlacement, ValueShape};
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_machine_bytes::{EncodedMachineFunction, EncodedMachinePlan};
    use omega_object_file::{SectionKind, SymbolKind, SymbolPlan, SymbolSection};
    use psi_arena::{Arena, HandleSpan};
    use psi_symbols::SymbolHandle;
    use std::{fs, path::PathBuf, sync::Arc};

    fn continuation_key(state: u32) -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        }
    }

    fn object_entry() -> SymbolPlan {
        SymbolPlan {
            name: "_main".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 32,
            size: 8,
            kind: SymbolKind::Function,
            import_library: String::new(),
        }
    }

    fn encoded_entry(key: StateKey) -> EncodedMachineFunction {
        EncodedMachineFunction {
            symbol: Arc::from("_main"),
            identity: MachineFunctionIdentity::source(key),
            byte_offset: 32,
            byte_count: 8,
            instructions: HandleSpan::empty(),
        }
    }

    fn backend_plan() -> BackendPlan {
        let target = omega_target::NativeTarget::uefi_x64();
        BackendPlan {
            target_profile: omega_target::TargetProfile::UefiX64,
            target,
            artifacts: BackendArtifactRoots::empty_for_target(target),
            host_abi: Arc::new(omega_calling_conventions::build_host_abi_plan(target)),
            host_calls: Arc::new(Default::default()),
            state_calls: Arc::new(Default::default()),
            alias_flow: Default::default(),
            state_storage: Arc::new(Default::default()),
            state_values: Default::default(),
            abstract_data: Default::default(),
            data: Default::default(),
            abstract_operations: Default::default(),
            target_operations: Default::default(),
            assigned_target_operations: Default::default(),
            control_flow: Arc::new(Default::default()),
            runtime_flow: Arc::new(Default::default()),
            state_dispatch: Arc::new(Default::default()),
            state_guards: Arc::new(Default::default()),
            runtime_bodies: Arc::new(Default::default()),
            runtime_branching_calls: Default::default(),
            runtime_dispatch_loop: Default::default(),
            runtime_storage: Default::default(),
            runtime_text: Default::default(),
            layouts: Arc::new(omega_layout::LayoutPlan {
                data_layouts: Arena::new(),
                fields: Arena::new(),
                bit_fields: Vec::new(),
                stored_integers: Vec::new(),
                repeated_fields: Vec::new(),
                machine_layouts: Arena::new(),
                variants: Arena::new(),
                private_callback_demands: Vec::new(),
            }),
            entry_key: continuation_key(2),
            entry_boundary_plan: None,
            callback_placements: Arc::from([]),
            callback_thunks: Arc::from([]),
            callback_private_relocations: Arc::from([]),
            callback_registrar_arguments: Arc::from([]),
            callback_registrar_destinations: Arc::from([]),
            receiver_bases: Vec::new(),
            state_contexts: Vec::new(),
            phase_timings: Arena::<BackendPlanPhaseTiming>::new(),
        }
    }

    fn parameter(parameter_index: usize) -> ProgramStorageEntryParameter {
        ProgramStorageEntryParameter {
            parameter_index,
            carrier_identity: "named(name(Extent))".into(),
            parameter_type_identity: format!("Extent::Granted::{parameter_index}"),
            domain: "Extent::Granted".into(),
            effective_carry: psi_language_semantics::CarryPolicy::STRICT,
            placement: ValuePlacement {
                shape: ValueShape::integer(16, 8),
                locations: Vec::new(),
            },
            destination_byte_offset: parameter_index * 16,
            write_range: parameter_index * 16..(parameter_index + 1) * 16,
        }
    }

    fn checked_source_signature() -> super::super::SelectedProgramEntrySourceSignature {
        let extent_layout = |base| {
            super::super::ProgramEntrySourceExtentValueLayout::from_checked_record(
                SymbolHandle::from_arena_index(base),
                SymbolHandle::from_arena_index(base + 1),
                0,
                ValueShape::integer(8, 8),
                SymbolHandle::from_arena_index(base + 2),
                8,
                ValueShape::integer(8, 8),
                ValueShape::integer(16, 8),
            )
            .expect("exact Extent layout")
        };
        super::super::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            SymbolHandle::from_arena_index(1),
            SymbolHandle::from_arena_index(2),
            "Boot::launch".into(),
            "launch".into(),
            "Boot::launch#exact".into(),
            super::super::ProgramEntrySourceReceiverSignature::Free,
            vec![
                super::super::SelectedProgramEntrySourceSignature::visible_parameter(
                    super::super::ProgramStorageEntryRootRole::Image,
                    0,
                    "Extent::Granted::0".into(),
                    ValueShape::integer(16, 8),
                    extent_layout(10),
                    false,
                    false,
                ),
                super::super::SelectedProgramEntrySourceSignature::visible_parameter(
                    super::super::ProgramStorageEntryRootRole::InitialStorage,
                    1,
                    "Extent::Granted::1".into(),
                    ValueShape::integer(16, 8),
                    extent_layout(20),
                    false,
                    false,
                ),
            ],
        )
        .expect("exact checked source signature")
    }

    fn compiler_binding(with_source: bool) -> ProgramStorageEntryPlanBinding {
        ProgramStorageEntryPlanBinding {
            root_slot: omega_external_roots::RootSlotId::from_normalized_identity(1)
                .expect("root slot"),
            requirement_identity: "ProgramStorageEntry::enter#exact".into(),
            boundary_contract_fingerprint: 1,
            image: parameter(0),
            initial_storage: parameter(1),
            receiver: None,
            source_signature: with_source.then(checked_source_signature),
            physical_contract: None,
        }
    }

    fn selected_storage_entry() -> SelectedProgramStorageEntryPlan {
        let slot = omega_target::TargetProfile::UefiX64.program_entry_slot();
        let requirement_identity = "ProgramStorageEntry::enter#exact";
        SelectedProgramStorageEntryPlan::from_target_slot(
            slot,
            omega_effects::provider_plan::ServiceSchema {
                trait_name: slot
                    .boundary_schema
                    .expect("UEFI program entry has a source boundary schema")
                    .into(),
                methods: vec![omega_effects::provider_plan::ServiceMethod {
                    name: "enter".into(),
                    requirement_owner: "ProgramStorageEntry".into(),
                    requirement_identity: requirement_identity.into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            requirement_identity.into(),
        )
        .expect("selected UEFI program-storage entry")
    }

    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../..")
    }

    fn compile_pending_receiver_bound_bridge(
        label: &str,
    ) -> super::ProgramStorageEntryNativeBridgePlan {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-program-storage-publication-bound-{}-{label}",
            std::process::id(),
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let report = super::super::compiler::compile(
            super::super::compiler::CompileRequest::new(
                super::super::compile_options::CompileOptions {
                    root_path: repository_root().join(
                        "tests/canaries/pass/build/uefi_program_entry_storage_roots/main.omg",
                    ),
                    build_dir: Some(build_dir.clone()),
                    target_name: Some("uefi_x64".into()),
                    write_output: false,
                },
            )
            .with_artifact_policy(
                super::super::compile_options::ArtifactEmissionPolicy::OutputOnly,
            ),
        )
        .expect("receiver-bound UEFI program-storage bridge");
        let bridge = report
            .program_storage_entry_bridge()
            .cloned()
            .expect("pending receiver-bound bridge");
        let _ = fs::remove_dir_all(build_dir);
        bridge
    }

    fn compile_pending_receiver_free_bridge() -> super::ProgramStorageEntryNativeBridgePlan {
        let directory = std::env::temp_dir().join(format!(
            "omega-program-storage-publication-free-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create receiver-free publication fixture");
        let source = include_str!(
            "../../../../../../../../tests/canaries/pass/build/uefi_program_entry_storage_roots/main.omg"
        );
        let prefix = source
            .split_once("data Boot {")
            .expect("UEFI fixture retains its Boot declaration")
            .0;
        fs::write(
            directory.join("main.omg"),
            format!(
                r#"{prefix}data Boot {{ }}

machine Boot::launch(
    image: Extent in Granted,
    initial_storage: Extent in Granted
) {{
    transition {{
        _ -> retain(image as Extent, initial_storage as Extent)
    }}

    state retain(image: Extent, initial_storage: Extent) {{
        transition {{
            _ -> retain(image, initial_storage)
        }}
    }}
}}
"#,
            ),
        )
        .expect("write receiver-free publication source");
        fs::write(
            directory.join("build.omg"),
            r#"target uefi_x64 {
}

machine build(builder: &mut Build) {
    builder.application("receiver-free-publication-evidence");
    builder.subsystem = Subsystem::EfiApplication;
    builder.freestanding = true;
    builder.roots.bind(uefi_x86_64::ProgramEntry, Boot::launch);
}
"#,
        )
        .expect("write receiver-free publication build root");
        let report = super::super::compiler::compile(
            super::super::compiler::CompileRequest::new(
                super::super::compile_options::CompileOptions {
                    root_path: directory.join("main.omg"),
                    build_dir: Some(directory.join("build")),
                    target_name: Some("uefi_x64".into()),
                    write_output: false,
                },
            )
            .with_artifact_policy(
                super::super::compile_options::ArtifactEmissionPolicy::OutputOnly,
            ),
        )
        .expect("receiver-free UEFI program-storage bridge");
        let bridge = report
            .program_storage_entry_bridge()
            .cloned()
            .expect("pending receiver-free bridge");
        let _ = fs::remove_dir_all(directory);
        bridge
    }

    #[test]
    fn absent_compiler_plan_join_has_no_backend_dependency() {
        let backend = backend_plan();

        let binding = bind_compiler_generated_program_storage_entry_plan(None, None, &backend)
            .expect("absent program-storage selection");

        assert!(binding.is_none());
    }

    #[test]
    fn compiler_plan_join_requires_checked_source_before_backend_plan() {
        let selected = selected_storage_entry();
        let backend = backend_plan();

        let diagnostics =
            bind_compiler_generated_program_storage_entry_plan(Some(&selected), None, &backend)
                .expect_err("selected program-storage entry must retain its source signature");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "selected program-storage entry lost its checked source signature before backend binding"
        );
    }

    #[test]
    fn compiler_plan_join_requires_retained_backend_entry_plan() {
        let selected = selected_storage_entry();
        let source_signature = checked_source_signature();
        let backend = backend_plan();

        let diagnostics = bind_compiler_generated_program_storage_entry_plan(
            Some(&selected),
            Some(&source_signature),
            &backend,
        )
        .expect_err("selected program-storage entry must retain its backend calling plan");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "selected program-storage entry lost its retained calling plan before backend binding"
        );
    }

    #[test]
    fn compiler_publication_settlement_preserves_absence_and_receiver_bound_exemption() {
        let backend = backend_plan();
        retain_compiler_generated_program_storage_entry_publication_evidence(None, &backend, None)
            .expect("absent bridge needs no publication evidence");

        let mut bridge = compile_pending_receiver_bound_bridge("exemption");
        assert!(bridge.is_receiver_bound_without_wrapper_template());
        let original = bridge.clone();
        retain_compiler_generated_program_storage_entry_publication_evidence(
            Some(&mut bridge),
            &backend,
            None,
        )
        .expect("exact receiver-bound bridge is the sole template-free exemption");
        assert_eq!(bridge, original);
        assert!(bridge.emitted_wrapper_evidence().is_none());
    }

    #[test]
    fn compiler_publication_settlement_checks_template_before_image() {
        let backend = backend_plan();
        let mut receiver_bound = compile_pending_receiver_bound_bridge("template-order");
        receiver_bound.binding.receiver = None;
        assert!(!receiver_bound.is_receiver_bound_without_wrapper_template());
        let template = retain_compiler_generated_program_storage_entry_publication_evidence(
            Some(&mut receiver_bound),
            &backend,
            None,
        )
        .expect_err("non-exempt template loss must reject before image selection");
        assert_eq!(template.len(), 1);
        assert_eq!(
            template[0].message,
            "native program-storage publication lost its receiver-free wrapper template without an exact receiver-bound continuation"
        );

        let mut receiver_free = compile_pending_receiver_free_bridge();
        assert!(receiver_free.wrapper_body_template().is_some());
        let image = retain_compiler_generated_program_storage_entry_publication_evidence(
            Some(&mut receiver_free),
            &backend,
            None,
        )
        .expect_err("receiver-free bridge requires a checked executable image");
        assert_eq!(image.len(), 1);
        assert_eq!(
            image[0].message,
            "program-storage entry target emitted no checked executable image"
        );
    }

    #[test]
    fn absent_compiler_bridge_leaves_backend_plan_unchanged() {
        let mut backend = backend_plan();
        let original = backend.clone();

        let bridge = bind_compiler_generated_program_storage_entry_native_bridge(
            None,
            None,
            Some("uefi_x64"),
            &mut backend,
        )
        .expect("absent program-storage binding");

        assert!(bridge.is_none());
        assert_eq!(backend, original);
    }

    #[test]
    fn compiler_bridge_requires_source_then_physical_custody_exactly() {
        let mut backend = backend_plan();
        let source = bind_compiler_generated_program_storage_entry_native_bridge(
            Some(compiler_binding(false)),
            None,
            Some("uefi_x64"),
            &mut backend,
        )
        .expect_err("missing source signature must reject first");
        assert_eq!(
            source[0].message,
            "compiler-generated program-storage binding lost its checked source signature"
        );

        let physical = bind_compiler_generated_program_storage_entry_native_bridge(
            Some(compiler_binding(true)),
            None,
            Some("uefi_x64"),
            &mut backend,
        )
        .expect_err("missing physical contract must reject second");
        assert_eq!(
            physical[0].message,
            "compiler-generated UEFI program-storage binding lost its distinct physical entry contract"
        );
    }

    #[test]
    fn compiler_bridge_target_profile_preserves_selection_and_host_fallback() {
        assert_eq!(
            compiler_generated_program_storage_target_profile(Some("uefi_x64")),
            "uefi_x64"
        );
        assert_eq!(
            compiler_generated_program_storage_target_profile(None),
            "host"
        );
    }

    #[test]
    fn emitted_bridge_requires_the_exact_encoded_continuation() {
        let key = continuation_key(2);
        let mut encoded = EncodedMachinePlan::default();
        encoded.code.functions.insert(encoded_entry(key));

        validate_encoded_program_storage_entry(&object_entry(), &encoded, key)
            .expect("exact entry symbol, interval, and StateKey should bind");

        let error =
            validate_encoded_program_storage_entry(&object_entry(), &encoded, continuation_key(3))
                .expect_err("a display-compatible but differently keyed continuation must reject");
        assert!(error.0.contains("redirects source continuation"), "{error}");
    }

    #[test]
    fn emitted_bridge_requires_one_global_encoded_symbol_identity() {
        let key = continuation_key(2);
        let empty = EncodedMachinePlan::default();
        let error = validate_encoded_program_storage_entry(&object_entry(), &empty, key)
            .expect_err("the object entry symbol must name an encoded function");
        assert!(error.0.contains("has no encoded function"), "{error}");

        let mut duplicate = EncodedMachinePlan::default();
        duplicate.code.functions.insert(encoded_entry(key));
        duplicate.code.functions.insert(encoded_entry(key));
        let error = validate_encoded_program_storage_entry(&object_entry(), &duplicate, key)
            .expect_err("duplicate exact entry identities must reject");
        assert!(
            error.0.contains("more than one encoded function"),
            "{error}"
        );

        let mut redirected_duplicate = EncodedMachinePlan::default();
        redirected_duplicate
            .code
            .functions
            .insert(encoded_entry(key));
        redirected_duplicate
            .code
            .functions
            .insert(encoded_entry(continuation_key(3)));
        let error =
            validate_encoded_program_storage_entry(&object_entry(), &redirected_duplicate, key)
                .expect_err("a same-symbol function redirected to another key is still ambiguous");
        assert!(
            error.0.contains("more than one encoded function"),
            "{error}"
        );

        let mut unrelated = EncodedMachinePlan::default();
        unrelated.code.functions.insert(encoded_entry(key));
        let mut helper = encoded_entry(continuation_key(3));
        helper.symbol = Arc::from("helper");
        unrelated.code.functions.insert(helper);
        validate_encoded_program_storage_entry(&object_entry(), &unrelated, key)
            .expect("a differently named encoded function must not create entry ambiguity");
    }

    #[test]
    fn emitted_bridge_rejects_interval_drift() {
        let key = continuation_key(2);
        let mut drifted = EncodedMachinePlan::default();
        let mut function = encoded_entry(key);
        function.byte_count = 7;
        drifted.code.functions.insert(function);
        let error = validate_encoded_program_storage_entry(&object_entry(), &drifted, key)
            .expect_err("object and encoded intervals must match exactly");
        assert!(error.0.contains("does not cover"), "{error}");
    }

    #[test]
    fn emitted_bridge_retains_generated_entry_and_source_continuation_separately() {
        let key = continuation_key(2);
        let wrapper_identity = MachineFunctionIdentity::program_storage_entry_wrapper(key)
            .expect("valid continuation should admit wrapper identity");
        let mut wrapper = encoded_entry(key);
        wrapper.identity = wrapper_identity;
        let mut source = encoded_entry(key);
        source.symbol = Arc::from("__omega_source_continuation");
        source.byte_offset = 8;
        source.byte_count = 12;
        let mut encoded = EncodedMachinePlan::default();
        encoded.code.functions.insert(source);
        encoded.code.functions.insert(wrapper);

        let validated = validate_encoded_program_storage_entry(&object_entry(), &encoded, key)
            .expect("generated entry and exact source continuation should bind separately");

        assert_eq!(validated.entry.identity, wrapper_identity);
        assert_eq!(validated.entry.symbol.as_ref(), "_main");
        assert_eq!(
            (validated.entry.byte_offset, validated.entry.byte_count),
            (32, 8)
        );
        assert_eq!(
            validated.continuation.identity,
            MachineFunctionIdentity::source(key)
        );
        assert_eq!(
            validated.continuation.symbol.as_ref(),
            "__omega_source_continuation"
        );
        assert_eq!(
            (
                validated.continuation.byte_offset,
                validated.continuation.byte_count
            ),
            (8, 12)
        );
    }

    #[test]
    fn emitted_bridge_rejects_generated_entry_without_exact_source_continuation() {
        let key = continuation_key(2);
        let mut wrapper = encoded_entry(key);
        wrapper.identity = MachineFunctionIdentity::program_storage_entry_wrapper(key)
            .expect("valid continuation should admit wrapper identity");
        let mut encoded = EncodedMachinePlan::default();
        encoded.code.functions.insert(wrapper);

        let error = validate_encoded_program_storage_entry(&object_entry(), &encoded, key)
            .expect_err("wrapper identity alone must not erase its source continuation");
        assert!(
            error.0.contains("has no encoded source continuation"),
            "{error}"
        );

        let mut duplicate = EncodedMachinePlan::default();
        let mut wrapper = encoded_entry(key);
        wrapper.identity = MachineFunctionIdentity::program_storage_entry_wrapper(key)
            .expect("valid continuation should admit wrapper identity");
        duplicate.code.functions.insert(wrapper);
        for symbol in ["source_a", "source_b"] {
            let mut source = encoded_entry(key);
            source.symbol = Arc::from(symbol);
            source.byte_offset = 8;
            duplicate.code.functions.insert(source);
        }
        let error = validate_encoded_program_storage_entry(&object_entry(), &duplicate, key)
            .expect_err("duplicate source-continuation identities must reject");
        assert!(
            error.0.contains("source continuation")
                && error.0.contains("more than one encoded function"),
            "{error}"
        );

        let mut aliased = EncodedMachinePlan::default();
        let mut wrapper = encoded_entry(key);
        wrapper.identity = MachineFunctionIdentity::program_storage_entry_wrapper(key)
            .expect("valid continuation should admit wrapper identity");
        aliased.code.functions.insert(wrapper);
        let mut source = encoded_entry(key);
        source.symbol = Arc::from("source_alias");
        aliased.code.functions.insert(source);
        let error = validate_encoded_program_storage_entry(&object_entry(), &aliased, key)
            .expect_err("distinct identities must not relabel the same encoded interval");
        assert!(
            error
                .0
                .contains("overlaps its separately identified source continuation"),
            "{error}"
        );
    }
}

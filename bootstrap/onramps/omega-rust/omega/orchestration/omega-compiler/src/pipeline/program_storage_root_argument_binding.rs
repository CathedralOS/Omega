//! Sealed receiver-free join between installed root authority and the exact
//! outbound source-continuation ABI.
//!
//! The join retains existing authority, declaration identity, shape, and
//! address-free placement facts. It does not materialize operand bytes,
//! populate registers or stack, emit a call edge, or claim native execution.

use super::{
    InstalledProgramStorageRoots, ProgramEntrySourceReceiverSignature,
    ProgramStorageEntryContinuationReceiverAbiPlan, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryNativeBridgePlan, ProgramStorageEntryRootAuthorityDisposition,
    ProgramStorageEntryRootRole, ProgramStorageEntryWholeRootAuthority,
    ProgramStorageEntryWrapperReceiverTransfer, RecordedProgramStorageInstallation,
};
use omega_calling_conventions::{ValuePlacement, ValueShape};
use psi_extents::Extent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryWholeRootArgumentBinding {
    role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    call_parameter_index: usize,
    normalized_type_identity: String,
    shape: ValueShape,
    placement: ValuePlacement,
    extent_value_layout: super::ProgramEntrySourceExtentValueLayout,
}

impl ProgramStorageEntryWholeRootArgumentBinding {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }
    pub const fn visible_parameter_index(&self) -> usize {
        self.visible_parameter_index
    }
    pub const fn call_parameter_index(&self) -> usize {
        self.call_parameter_index
    }
    pub fn normalized_type_identity(&self) -> &str {
        &self.normalized_type_identity
    }
    pub const fn shape(&self) -> ValueShape {
        self.shape
    }
    pub const fn placement(&self) -> &ValuePlacement {
        &self.placement
    }
    pub const fn extent_value_layout(&self) -> &super::ProgramEntrySourceExtentValueLayout {
        &self.extent_value_layout
    }
}

/// Two installed whole root authorities bound to one exact free continuation
/// ABI. The owned `Extent`s remain the authority; argument rows are immutable
/// observations of where a later emitter must realize their ordinary values.
#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootArgumentCarrier {
    authority: ProgramStorageEntryWholeRootAuthority,
    target: omega_target::NativeTarget,
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    normalized_callable_identity: String,
    arguments: [ProgramStorageEntryWholeRootArgumentBinding; 2],
}

impl ProgramStorageEntryWholeRootArgumentCarrier {
    pub const fn authority(&self) -> &ProgramStorageEntryWholeRootAuthority {
        &self.authority
    }
    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }
    pub const fn target_slot(&self) -> omega_target::ProgramEntrySlotDeclaration {
        self.target_slot
    }
    pub const fn continuation_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.continuation_identity
    }
    pub fn normalized_callable_identity(&self) -> &str {
        &self.normalized_callable_identity
    }
    pub const fn arguments(&self) -> &[ProgramStorageEntryWholeRootArgumentBinding; 2] {
        &self.arguments
    }

    pub const fn root_authority(&self, role: ProgramStorageEntryRootRole) -> &Extent {
        match role {
            ProgramStorageEntryRootRole::Image => self.authority.image(),
            ProgramStorageEntryRootRole::InitialStorage => self.authority.initial_storage(),
        }
    }

    pub fn into_authority(self) -> ProgramStorageEntryWholeRootAuthority {
        self.authority
    }
}

/// Bind receiver-free installed authorities to the exact ABI retained by the
/// bridge that produced their installation binding.
pub fn bind_program_storage_entry_whole_root_arguments(
    authority: ProgramStorageEntryWholeRootAuthority,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<ProgramStorageEntryWholeRootArgumentCarrier, ProgramStorageEntryWholeRootArgumentError>
{
    match validate_whole_root_arguments(&authority, bridge) {
        Ok(arguments) => {
            let abi = bridge
                .continuation_abi()
                .expect("validated whole-root arguments retain an exact continuation ABI");
            Ok(ProgramStorageEntryWholeRootArgumentCarrier {
                authority,
                target: abi.target(),
                target_slot: abi.target_slot(),
                continuation_identity: abi.continuation_identity(),
                normalized_callable_identity: abi.normalized_callable_identity().to_owned(),
                arguments,
            })
        }
        Err(diagnostic) => Err(ProgramStorageEntryWholeRootArgumentError {
            authority,
            diagnostic,
        }),
    }
}

/// Join one completed installation directly to its exact receiver-free
/// continuation ABI without allowing callers to bypass any ownership stage.
///
/// Every check that can reject an ordinary input runs against a borrow first,
/// so that rejection returns the intact recorded installation. The staged
/// recovery carrier exists for fail-closed invariant failures after ownership
/// begins moving; it always retains the highest successfully constructed
/// authority carrier.
pub fn bind_recorded_program_storage_entry_whole_root_arguments(
    installation: RecordedProgramStorageInstallation,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<
    ProgramStorageEntryWholeRootArgumentCarrier,
    ProgramStorageEntryRecordedWholeRootArgumentError,
> {
    if let Err(diagnostic) = validate_recorded_whole_root_arguments(&installation, bridge) {
        return Err(ProgramStorageEntryRecordedWholeRootArgumentError {
            diagnostic,
            recovery: ProgramStorageEntryRecordedWholeRootArgumentRecovery::RecordedInstallation(
                installation,
            ),
        });
    }

    let roots = installation.into_roots().map_err(|error| {
        let diagnostic = error.diagnostic().clone();
        ProgramStorageEntryRecordedWholeRootArgumentError {
            diagnostic,
            recovery: ProgramStorageEntryRecordedWholeRootArgumentRecovery::RecordedInstallation(
                error.into_installation(),
            ),
        }
    })?;
    let disposition = roots.into_root_authority_disposition().map_err(|error| {
        let diagnostic = error.diagnostic().clone();
        ProgramStorageEntryRecordedWholeRootArgumentError {
            diagnostic,
            recovery: ProgramStorageEntryRecordedWholeRootArgumentRecovery::InstalledRoots(
                error.into_roots(),
            ),
        }
    })?;
    let authority = disposition
        .try_into_receiver_free_whole_roots()
        .map_err(|error| {
            let diagnostic = error.diagnostic().clone();
            ProgramStorageEntryRecordedWholeRootArgumentError {
                diagnostic,
                recovery:
                    ProgramStorageEntryRecordedWholeRootArgumentRecovery::RootAuthorityDisposition(
                        error.into_disposition(),
                    ),
            }
        })?;
    bind_program_storage_entry_whole_root_arguments(authority, bridge).map_err(|error| {
        let diagnostic = error.diagnostic().clone();
        ProgramStorageEntryRecordedWholeRootArgumentError {
            diagnostic,
            recovery: ProgramStorageEntryRecordedWholeRootArgumentRecovery::WholeRootAuthority(
                error.into_authority(),
            ),
        }
    })
}

fn validate_recorded_whole_root_arguments(
    installation: &RecordedProgramStorageInstallation,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let roots = installation.roots();
    super::program_storage_root_authority::validate_root_authority_disposition(roots)?;
    if roots.receiver_storage().is_some() || roots.initial_storage().is_none() {
        return Err(ProgramStorageEntryDiagnostic(
            "attached program storage cannot bind the receiver-free whole-root argument carrier"
                .into(),
        ));
    }
    validate_whole_root_binding(roots.binding(), bridge).map(|_| ())
}

fn validate_whole_root_arguments(
    authority: &ProgramStorageEntryWholeRootAuthority,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<[ProgramStorageEntryWholeRootArgumentBinding; 2], ProgramStorageEntryDiagnostic> {
    validate_whole_root_binding(authority.binding(), bridge)
}

fn validate_whole_root_binding(
    binding: &super::ProgramStorageEntryPlanBinding,
    bridge: &ProgramStorageEntryNativeBridgePlan,
) -> Result<[ProgramStorageEntryWholeRootArgumentBinding; 2], ProgramStorageEntryDiagnostic> {
    if binding != bridge.binding() {
        return Err(ProgramStorageEntryDiagnostic(
            "whole root authority does not belong to this exact program-storage bridge binding"
                .into(),
        ));
    }
    if binding.receiver().is_some() {
        return Err(ProgramStorageEntryDiagnostic(
            "attached program storage cannot bind the receiver-free whole-root argument carrier"
                .into(),
        ));
    }
    let source = bridge.source_signature().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "whole root authority has no sealed selected source declaration".into(),
        )
    })?;
    let abi = bridge.continuation_abi().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "whole root authority has no retained source-continuation ABI".into(),
        )
    })?;
    if !matches!(source.receiver(), ProgramEntrySourceReceiverSignature::Free)
        || !matches!(
            abi.receiver(),
            ProgramStorageEntryContinuationReceiverAbiPlan::Free
        )
        || !matches!(
            bridge.wrapper_transfer().receiver(),
            ProgramStorageEntryWrapperReceiverTransfer::Free
        )
    {
        return Err(ProgramStorageEntryDiagnostic(
            "whole root argument carrier requires one exact receiver-free continuation".into(),
        ));
    }
    let exact_continuation =
        omega_control_flow::MachineFunctionIdentity::source(omega_control_flow::StateKey {
            machine: source.machine_symbol(),
            state: source.state_symbol(),
            segment_index: 0,
        });
    if source.target_slot() != abi.target_slot()
        || source.normalized_callable_identity() != abi.normalized_callable_identity()
        || abi.continuation_identity() != exact_continuation
        || abi.continuation_identity()
            != omega_control_flow::MachineFunctionIdentity::source(bridge.continuation_key())
        || bridge.wrapper_transfer().continuation_identity() != abi.continuation_identity()
        || abi.call().result.is_some()
        || abi.call().parameters.len() != 2
    {
        return Err(ProgramStorageEntryDiagnostic(
            "whole root authority drifted from its exact free Unit continuation ABI".into(),
        ));
    }
    let [image_source, storage_source] = source.visible_parameters() else {
        return Err(ProgramStorageEntryDiagnostic(
            "whole root argument carrier requires exact Image then InitialStorage declarations"
                .into(),
        ));
    };
    let [image_abi, storage_abi] = abi.visible_arguments() else {
        return Err(ProgramStorageEntryDiagnostic(
            "whole root argument carrier requires exact Image then InitialStorage ABI rows".into(),
        ));
    };
    let [image_transfer, storage_transfer] = bridge.wrapper_transfer().roots();
    let rows = [
        (
            ProgramStorageEntryRootRole::Image,
            binding.image().parameter_type_identity(),
            image_source,
            image_abi,
            image_transfer,
        ),
        (
            ProgramStorageEntryRootRole::InitialStorage,
            binding.initial_storage().parameter_type_identity(),
            storage_source,
            storage_abi,
            storage_transfer,
        ),
    ];
    let mut arguments = Vec::with_capacity(2);
    for (expected_index, (role, binding_type, source, abi_argument, transfer)) in
        rows.into_iter().enumerate()
    {
        if source.role() != role
            || abi_argument.role() != role
            || transfer.role() != role
            || source.visible_parameter_index() != expected_index
            || abi_argument.visible_parameter_index() != expected_index
            || transfer.source_parameter_index() != expected_index
            || abi_argument.call_parameter_index() != expected_index
            || source.normalized_type_identity() != binding_type
            || abi_argument.normalized_type_identity() != binding_type
            || abi_argument.shape() != source.value_shape()
            || abi_argument.placement().shape != abi_argument.shape()
            || abi.call().parameters.get(expected_index) != Some(abi_argument.placement())
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "whole root {role:?} authority drifted from continuation argument {expected_index}"
            )));
        }
        arguments.push(ProgramStorageEntryWholeRootArgumentBinding {
            role,
            visible_parameter_index: expected_index,
            call_parameter_index: expected_index,
            normalized_type_identity: binding_type.to_owned(),
            shape: abi_argument.shape(),
            placement: abi_argument.placement().clone(),
            extent_value_layout: source.extent_value_layout().clone(),
        });
    }
    arguments.try_into().map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "whole root argument carrier lost its exact two argument rows".into(),
        )
    })
}

#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootArgumentError {
    authority: ProgramStorageEntryWholeRootAuthority,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryWholeRootArgumentError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }
    pub fn into_authority(self) -> ProgramStorageEntryWholeRootAuthority {
        self.authority
    }
}

impl std::fmt::Display for ProgramStorageEntryWholeRootArgumentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryWholeRootArgumentError {}

/// Highest valid owned carrier retained by a failed recorded-installation
/// transition.
#[derive(Debug)]
pub enum ProgramStorageEntryRecordedWholeRootArgumentRecovery {
    RecordedInstallation(RecordedProgramStorageInstallation),
    InstalledRoots(InstalledProgramStorageRoots),
    RootAuthorityDisposition(ProgramStorageEntryRootAuthorityDisposition),
    WholeRootAuthority(ProgramStorageEntryWholeRootAuthority),
}

#[derive(Debug)]
pub struct ProgramStorageEntryRecordedWholeRootArgumentError {
    diagnostic: ProgramStorageEntryDiagnostic,
    recovery: ProgramStorageEntryRecordedWholeRootArgumentRecovery,
}

impl ProgramStorageEntryRecordedWholeRootArgumentError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_recovery(self) -> ProgramStorageEntryRecordedWholeRootArgumentRecovery {
        self.recovery
    }
}

impl std::fmt::Display for ProgramStorageEntryRecordedWholeRootArgumentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryRecordedWholeRootArgumentError {}

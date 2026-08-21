//! Checked declaration signature for the exact source machine selected by a
//! target-owned `ProgramEntry` slot.
//!
//! This carrier is captured from typed trees before they are consumed by
//! checked/backend lowering. It records declarations only: it owns no runtime
//! value, `Extent`, root authority, ABI shape, placement, or call plan.

use super::program_storage_wrapper::ProgramStorageEntryRootRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramEntrySourceReceiverSignature {
    Free,
    ProvisionedMutable { normalized_type_identity: String },
}

impl ProgramEntrySourceReceiverSignature {
    pub fn normalized_type_identity(&self) -> Option<&str> {
        match self {
            Self::Free => None,
            Self::ProvisionedMutable {
                normalized_type_identity,
            } => Some(normalized_type_identity),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntrySourceResultSignature {
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntrySourceVisibleParameterSignature {
    role: ProgramStorageEntryRootRole,
    /// Receiver-excluded ordinal in the target schema's visible list. This is
    /// not a source formal index or an outbound ABI parameter index.
    visible_parameter_index: usize,
    normalized_type_identity: String,
    value_shape: omega_calling_conventions::ValueShape,
    is_const: bool,
    is_mutable: bool,
}

impl ProgramEntrySourceVisibleParameterSignature {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn visible_parameter_index(&self) -> usize {
        self.visible_parameter_index
    }

    pub fn normalized_type_identity(&self) -> &str {
        &self.normalized_type_identity
    }

    /// Exact address-free shape derived from this typed source declaration,
    /// independently of the physical arrival calling plan.
    pub const fn value_shape(&self) -> omega_calling_conventions::ValueShape {
        self.value_shape
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }
}

/// Sealed typed-tree evidence for one selected source continuation signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntrySourceSignature {
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    machine_symbol: psi_symbols::SymbolHandle,
    state_symbol: psi_symbols::SymbolHandle,
    machine_name: String,
    state_name: String,
    normalized_callable_identity: String,
    receiver: ProgramEntrySourceReceiverSignature,
    visible_parameters: Vec<ProgramEntrySourceVisibleParameterSignature>,
    result: ProgramEntrySourceResultSignature,
}

impl SelectedProgramEntrySourceSignature {
    pub(super) fn from_checked_typed_entry(
        target_slot: omega_target::ProgramEntrySlotDeclaration,
        machine_symbol: psi_symbols::SymbolHandle,
        state_symbol: psi_symbols::SymbolHandle,
        machine_name: String,
        state_name: String,
        normalized_callable_identity: String,
        receiver: ProgramEntrySourceReceiverSignature,
        visible_parameters: Vec<ProgramEntrySourceVisibleParameterSignature>,
    ) -> Result<Self, String> {
        let signature = Self {
            target_slot,
            machine_symbol,
            state_symbol,
            machine_name,
            state_name,
            normalized_callable_identity,
            receiver,
            visible_parameters,
            result: ProgramEntrySourceResultSignature::Unit,
        };
        signature.validate_capture()?;
        Ok(signature)
    }

    pub const fn target_slot(&self) -> omega_target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn machine_symbol(&self) -> psi_symbols::SymbolHandle {
        self.machine_symbol
    }

    pub const fn state_symbol(&self) -> psi_symbols::SymbolHandle {
        self.state_symbol
    }

    pub fn machine_name(&self) -> &str {
        &self.machine_name
    }

    pub fn state_name(&self) -> &str {
        &self.state_name
    }

    pub fn normalized_callable_identity(&self) -> &str {
        &self.normalized_callable_identity
    }

    pub const fn receiver(&self) -> &ProgramEntrySourceReceiverSignature {
        &self.receiver
    }

    pub fn visible_parameters(&self) -> &[ProgramEntrySourceVisibleParameterSignature] {
        &self.visible_parameters
    }

    pub const fn result(&self) -> ProgramEntrySourceResultSignature {
        self.result
    }

    pub(super) fn validate_program_storage_binding(
        &self,
        selected_slot: omega_target::ProgramEntrySlotDeclaration,
        continuation_key: omega_control_flow::StateKey,
        receiver_type_identity: Option<&str>,
        image_type_identity: &str,
        initial_storage_type_identity: &str,
    ) -> Result<(), String> {
        self.validate_capture()?;
        if self.target_slot != selected_slot
            || selected_slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
            || selected_slot.visible_parameters
                != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        {
            return Err(
                "selected source signature drifted from its exact program-storage target slot"
                    .into(),
            );
        }
        if continuation_key.segment_index != 0
            || continuation_key.machine != self.machine_symbol
            || continuation_key.state != self.state_symbol
        {
            return Err(
                "selected source signature does not identify the exact lowered continuation".into(),
            );
        }
        let receiver_matches = match (&self.receiver, receiver_type_identity) {
            (ProgramEntrySourceReceiverSignature::Free, None) => true,
            (
                ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                    normalized_type_identity,
                },
                Some(actual),
            ) => normalized_type_identity == actual,
            _ => false,
        };
        if !receiver_matches {
            return Err(
                "selected source signature receiver drifted from its checked storage layout".into(),
            );
        }
        if !matches!(
            self.visible_parameters.as_slice(),
            [
                ProgramEntrySourceVisibleParameterSignature {
                    role: ProgramStorageEntryRootRole::Image,
                    visible_parameter_index: 0,
                    normalized_type_identity: image,
                    value_shape: _,
                    is_const: false,
                    is_mutable: false,
                },
                ProgramEntrySourceVisibleParameterSignature {
                    role: ProgramStorageEntryRootRole::InitialStorage,
                    visible_parameter_index: 1,
                    normalized_type_identity: initial_storage,
                    value_shape: _,
                    is_const: false,
                    is_mutable: false,
                }
            ] if image == image_type_identity && initial_storage == initial_storage_type_identity
        ) {
            return Err(
                "selected source signature visible parameters drifted from exact Image then InitialStorage declarations"
                    .into(),
            );
        }
        Ok(())
    }

    fn validate_capture(&self) -> Result<(), String> {
        if self.target_slot != self.target_slot.owner.program_entry_slot()
            || !self.machine_symbol.is_valid()
            || !self.state_symbol.is_valid()
            || self.machine_name.is_empty()
            || self.state_name.is_empty()
            || self.normalized_callable_identity.is_empty()
            || self.result != ProgramEntrySourceResultSignature::Unit
        {
            return Err("selected source signature is not one exact typed Unit entry".into());
        }
        if self
            .receiver
            .normalized_type_identity()
            .is_some_and(str::is_empty)
            || self.visible_parameters.iter().any(|parameter| {
                parameter.normalized_type_identity.is_empty()
                    || parameter.value_shape.byte_size == 0
                    || parameter.value_shape.alignment == 0
                    || !parameter.value_shape.alignment.is_power_of_two()
                    || parameter.is_const
                    || parameter.is_mutable
            })
        {
            return Err(
                "selected source signature contains an invalid parameter declaration".into(),
            );
        }
        Ok(())
    }

    pub(super) fn visible_parameter(
        role: ProgramStorageEntryRootRole,
        visible_parameter_index: usize,
        normalized_type_identity: String,
        value_shape: omega_calling_conventions::ValueShape,
        is_const: bool,
        is_mutable: bool,
    ) -> ProgramEntrySourceVisibleParameterSignature {
        ProgramEntrySourceVisibleParameterSignature {
            role,
            visible_parameter_index,
            normalized_type_identity,
            value_shape,
            is_const,
            is_mutable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_symbols::SymbolHandle;

    fn key() -> omega_control_flow::StateKey {
        omega_control_flow::StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        }
    }

    fn signature(
        receiver: ProgramEntrySourceReceiverSignature,
    ) -> SelectedProgramEntrySourceSignature {
        SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            key().machine,
            key().state,
            "Boot::launch".into(),
            "launch".into(),
            "normalized-callable".into(),
            receiver,
            vec![
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::Image,
                    0,
                    "Extent in Granted".into(),
                    omega_calling_conventions::ValueShape::integer(16, 8),
                    false,
                    false,
                ),
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::InitialStorage,
                    1,
                    "Extent in Granted".into(),
                    omega_calling_conventions::ValueShape::integer(16, 8),
                    false,
                    false,
                ),
            ],
        )
        .expect("exact typed source signature")
    }

    #[test]
    fn exact_program_storage_signature_binds_declarations_without_values() {
        let signature = signature(ProgramEntrySourceReceiverSignature::ProvisionedMutable {
            normalized_type_identity: "Boot".into(),
        });
        signature
            .validate_program_storage_binding(
                omega_target::TargetProfile::UefiX64.program_entry_slot(),
                key(),
                Some("Boot"),
                "Extent in Granted",
                "Extent in Granted",
            )
            .expect("exact declaration binding");
        assert_eq!(signature.result(), ProgramEntrySourceResultSignature::Unit);
    }

    #[test]
    fn continuation_receiver_and_parameter_drift_fail_closed() {
        let signature = signature(ProgramEntrySourceReceiverSignature::ProvisionedMutable {
            normalized_type_identity: "Boot".into(),
        });
        let mut wrong_key = key();
        wrong_key.state = SymbolHandle::from_arena_index(3);
        for (key, receiver, image, storage) in [
            (
                wrong_key,
                Some("Boot"),
                "Extent in Granted",
                "Extent in Granted",
            ),
            (
                key(),
                Some("Other"),
                "Extent in Granted",
                "Extent in Granted",
            ),
            (key(), Some("Boot"), "Other", "Extent in Granted"),
            (key(), Some("Boot"), "Extent in Granted", "Other"),
        ] {
            signature
                .validate_program_storage_binding(
                    omega_target::TargetProfile::UefiX64.program_entry_slot(),
                    key,
                    receiver,
                    image,
                    storage,
                )
                .expect_err("source signature drift must reject");
        }
    }

    #[test]
    fn role_mode_slot_and_free_receiver_drift_fail_closed() {
        let mut role_drift = signature(ProgramEntrySourceReceiverSignature::Free);
        role_drift.visible_parameters.swap(0, 1);
        role_drift
            .validate_program_storage_binding(
                omega_target::TargetProfile::UefiX64.program_entry_slot(),
                key(),
                None,
                "Extent in Granted",
                "Extent in Granted",
            )
            .expect_err("role order drift must reject");

        let mut mode_drift = signature(ProgramEntrySourceReceiverSignature::Free);
        mode_drift.visible_parameters[0].is_mutable = true;
        mode_drift
            .validate_program_storage_binding(
                omega_target::TargetProfile::UefiX64.program_entry_slot(),
                key(),
                None,
                "Extent in Granted",
                "Extent in Granted",
            )
            .expect_err("parameter mode drift must reject");

        let signature = signature(ProgramEntrySourceReceiverSignature::Free);
        signature
            .validate_program_storage_binding(
                omega_target::TargetProfile::WindowsX64.program_entry_slot(),
                key(),
                None,
                "Extent in Granted",
                "Extent in Granted",
            )
            .expect_err("target slot drift must reject");
        signature
            .validate_program_storage_binding(
                omega_target::TargetProfile::UefiX64.program_entry_slot(),
                key(),
                Some("Boot"),
                "Extent in Granted",
                "Extent in Granted",
            )
            .expect_err("free source signature cannot acquire a receiver");
    }
}

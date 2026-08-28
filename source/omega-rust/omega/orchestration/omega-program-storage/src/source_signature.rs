//! Checked declaration signature for the exact source machine selected by a
//! target-owned `ProgramEntry` slot.
//!
//! This carrier is captured from typed trees before they are consumed by
//! checked/backend lowering. It owns no runtime value, `Extent`, root
//! authority, placement, or call plan. For the currently admitted
//! program-storage schema it also retains the exact address-free `Extent`
//! record graph needed to distinguish its fields from a size-only lookalike.

use crate::ProgramStorageEntryRootRole;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntrySourceExtentFieldRole {
    Base,
    Length,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntrySourceExtentFieldLayout {
    role: ProgramEntrySourceExtentFieldRole,
    symbol: psi_symbols::SymbolHandle,
    byte_offset: u16,
    shape: omega_calling_conventions::ValueShape,
}

impl ProgramEntrySourceExtentFieldLayout {
    pub const fn role(&self) -> ProgramEntrySourceExtentFieldRole {
        self.role
    }

    pub const fn symbol(&self) -> psi_symbols::SymbolHandle {
        self.symbol
    }

    pub const fn byte_offset(&self) -> u16 {
        self.byte_offset
    }

    pub const fn shape(&self) -> omega_calling_conventions::ValueShape {
        self.shape
    }
}

/// Exact checked record graph for the ordinary value portion of one
/// `Extent in Granted`. Domain authority is deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntrySourceExtentValueLayout {
    data_symbol: psi_symbols::SymbolHandle,
    shape: omega_calling_conventions::ValueShape,
    fields: [ProgramEntrySourceExtentFieldLayout; 2],
}

impl ProgramEntrySourceExtentValueLayout {
    pub const fn data_symbol(&self) -> psi_symbols::SymbolHandle {
        self.data_symbol
    }

    pub const fn shape(&self) -> omega_calling_conventions::ValueShape {
        self.shape
    }

    pub const fn fields(&self) -> &[ProgramEntrySourceExtentFieldLayout; 2] {
        &self.fields
    }

    pub fn from_checked_record(
        data_symbol: psi_symbols::SymbolHandle,
        base_symbol: psi_symbols::SymbolHandle,
        base_offset: u16,
        base_shape: omega_calling_conventions::ValueShape,
        length_symbol: psi_symbols::SymbolHandle,
        length_offset: u16,
        length_shape: omega_calling_conventions::ValueShape,
        shape: omega_calling_conventions::ValueShape,
    ) -> Result<Self, String> {
        let layout = Self {
            data_symbol,
            shape,
            fields: [
                ProgramEntrySourceExtentFieldLayout {
                    role: ProgramEntrySourceExtentFieldRole::Base,
                    symbol: base_symbol,
                    byte_offset: base_offset,
                    shape: base_shape,
                },
                ProgramEntrySourceExtentFieldLayout {
                    role: ProgramEntrySourceExtentFieldRole::Length,
                    symbol: length_symbol,
                    byte_offset: length_offset,
                    shape: length_shape,
                },
            ],
        };
        layout.validate_capture()?;
        Ok(layout)
    }

    fn validate_capture(&self) -> Result<(), String> {
        let scalar = omega_calling_conventions::ValueShape::integer(8, 8);
        if !self.data_symbol.is_valid()
            || self.shape != omega_calling_conventions::ValueShape::integer(16, 8)
            || self.fields[0].role != ProgramEntrySourceExtentFieldRole::Base
            || self.fields[0].byte_offset != 0
            || self.fields[0].shape != scalar
            || self.fields[1].role != ProgramEntrySourceExtentFieldRole::Length
            || self.fields[1].byte_offset != 8
            || self.fields[1].shape != scalar
            || !self.fields[0].symbol.is_valid()
            || !self.fields[1].symbol.is_valid()
            || self.fields[0].symbol == self.fields[1].symbol
        {
            return Err(
                "selected source Extent must be the exact {base: addr@0, length: u64@8} x86-64 record"
                    .into(),
            );
        }
        Ok(())
    }

    pub fn validate_backend_layout(
        &self,
        layouts: &omega_layout::LayoutPlan,
    ) -> Result<(), String> {
        self.validate_capture()?;
        let matches = layouts
            .data_layouts
            .iter()
            .filter_map(|(_, layout)| (layout.symbol == self.data_symbol).then_some(layout))
            .collect::<Vec<_>>();
        let [layout] = matches.as_slice() else {
            return Err(format!(
                "selected source Extent has {} backend data-layout rows instead of exactly one",
                matches.len()
            ));
        };
        let omega_layout::DataShape::Record { fields } = layout.shape else {
            return Err("selected source Extent backend layout is not a record".into());
        };
        if layout.layout.size != usize::from(self.shape.byte_size)
            || layout.layout.alignment != usize::from(self.shape.alignment)
        {
            return Err(
                "selected source Extent aggregate layout drifted from its checked graph".into(),
            );
        }
        let fields = layouts
            .fields
            .span(fields)
            .ok_or_else(|| "selected source Extent backend field span is invalid".to_owned())?;
        if fields.len() != 2 {
            return Err(
                "selected source Extent backend layout must retain exactly two fields".into(),
            );
        }
        for (expected, actual) in self.fields.iter().zip(fields) {
            let expected_name = match expected.role {
                ProgramEntrySourceExtentFieldRole::Base => "base",
                ProgramEntrySourceExtentFieldRole::Length => "length",
            };
            let expected_type = match expected.role {
                ProgramEntrySourceExtentFieldRole::Base => "addr",
                ProgramEntrySourceExtentFieldRole::Length => "u64",
            };
            if actual.symbol != expected.symbol
                || actual.name.as_str() != expected_name
                || actual.offset != usize::from(expected.byte_offset)
                || actual.type_name.as_ref() != expected_type
                || actual.layout.size != usize::from(expected.shape.byte_size)
                || actual.layout.alignment != usize::from(expected.shape.alignment)
                || layouts.bit_field(actual.symbol).is_some()
                || layouts.stored_integer(actual.symbol).is_some()
                || layouts.repeated_field(actual.symbol).is_some()
            {
                return Err(format!(
                    "selected source Extent backend field `{expected_name}` drifted from its exact checked layout"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntrySourceVisibleParameterSignature {
    role: ProgramStorageEntryRootRole,
    /// Receiver-excluded ordinal in the target schema's visible list. This is
    /// not a source formal index or an outbound ABI parameter index.
    visible_parameter_index: usize,
    normalized_type_identity: String,
    value_shape: omega_calling_conventions::ValueShape,
    extent_value_layout: ProgramEntrySourceExtentValueLayout,
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

    pub const fn extent_value_layout(&self) -> &ProgramEntrySourceExtentValueLayout {
        &self.extent_value_layout
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
    pub fn from_checked_typed_entry(
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

    pub fn validate_program_storage_binding(
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
                    extent_value_layout: _,
                    is_const: false,
                    is_mutable: false,
                },
                ProgramEntrySourceVisibleParameterSignature {
                    role: ProgramStorageEntryRootRole::InitialStorage,
                    visible_parameter_index: 1,
                    normalized_type_identity: initial_storage,
                    value_shape: _,
                    extent_value_layout: _,
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
                    || parameter.extent_value_layout.validate_capture().is_err()
                    || parameter.extent_value_layout.shape() != parameter.value_shape
            })
        {
            return Err(
                "selected source signature contains an invalid parameter declaration".into(),
            );
        }
        Ok(())
    }

    pub fn visible_parameter(
        role: ProgramStorageEntryRootRole,
        visible_parameter_index: usize,
        normalized_type_identity: String,
        value_shape: omega_calling_conventions::ValueShape,
        extent_value_layout: ProgramEntrySourceExtentValueLayout,
        is_const: bool,
        is_mutable: bool,
    ) -> ProgramEntrySourceVisibleParameterSignature {
        ProgramEntrySourceVisibleParameterSignature {
            role,
            visible_parameter_index,
            normalized_type_identity,
            value_shape,
            extent_value_layout,
            is_const,
            is_mutable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_arena::Arena;
    use psi_checked_trees::name::Identifier;
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    fn key() -> omega_control_flow::StateKey {
        omega_control_flow::StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        }
    }

    fn extent_layout() -> ProgramEntrySourceExtentValueLayout {
        ProgramEntrySourceExtentValueLayout::from_checked_record(
            SymbolHandle::from_arena_index(3),
            SymbolHandle::from_arena_index(4),
            0,
            omega_calling_conventions::ValueShape::integer(8, 8),
            SymbolHandle::from_arena_index(5),
            8,
            omega_calling_conventions::ValueShape::integer(8, 8),
            omega_calling_conventions::ValueShape::integer(16, 8),
        )
        .expect("exact Extent record graph")
    }

    fn backend_extent_layout(
        base_name: &str,
        base_offset: usize,
        length_type: &str,
        aggregate_size: usize,
    ) -> omega_layout::LayoutPlan {
        let layout = extent_layout();
        let mut fields = Arena::with_capacity(2);
        let field_span = fields.insert_many([
            omega_layout::FieldLayout {
                symbol: layout.fields()[0].symbol(),
                name: Identifier::from(base_name),
                offset: base_offset,
                type_symbol: SymbolHandle::invalid(),
                type_name: Arc::from("addr"),
                type_descriptor: omega_layout::TypeLayoutDescriptor::default(),
                layout: omega_layout::TypeLayout {
                    size: 8,
                    alignment: 8,
                },
            },
            omega_layout::FieldLayout {
                symbol: layout.fields()[1].symbol(),
                name: Identifier::from("length"),
                offset: 8,
                type_symbol: SymbolHandle::invalid(),
                type_name: Arc::from(length_type),
                type_descriptor: omega_layout::TypeLayoutDescriptor::default(),
                layout: omega_layout::TypeLayout {
                    size: 8,
                    alignment: 8,
                },
            },
        ]);
        let mut data_layouts = Arena::with_capacity(1);
        data_layouts.insert(omega_layout::DataLayout {
            symbol: layout.data_symbol(),
            name: Identifier::from("Extent"),
            shape: omega_layout::DataShape::Record { fields: field_span },
            layout: omega_layout::TypeLayout {
                size: aggregate_size,
                alignment: 8,
            },
        });
        omega_layout::LayoutPlan {
            data_layouts,
            fields,
            bit_fields: Vec::new(),
            stored_integers: Vec::new(),
            repeated_fields: Vec::new(),
            machine_layouts: Arena::with_capacity(0),
            variants: Arena::with_capacity(0),
            private_callback_demands: Vec::new(),
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
                    extent_layout(),
                    false,
                    false,
                ),
                SelectedProgramEntrySourceSignature::visible_parameter(
                    ProgramStorageEntryRootRole::InitialStorage,
                    1,
                    "Extent in Granted".into(),
                    omega_calling_conventions::ValueShape::integer(16, 8),
                    extent_layout(),
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

    #[test]
    fn extent_field_graph_drift_fails_closed() {
        for (base_offset, length_offset, aggregate) in [
            (1, 8, omega_calling_conventions::ValueShape::integer(16, 8)),
            (0, 0, omega_calling_conventions::ValueShape::integer(16, 8)),
            (0, 8, omega_calling_conventions::ValueShape::integer(8, 8)),
        ] {
            ProgramEntrySourceExtentValueLayout::from_checked_record(
                SymbolHandle::from_arena_index(3),
                SymbolHandle::from_arena_index(4),
                base_offset,
                omega_calling_conventions::ValueShape::integer(8, 8),
                SymbolHandle::from_arena_index(5),
                length_offset,
                omega_calling_conventions::ValueShape::integer(8, 8),
                aggregate,
            )
            .expect_err("Extent structural drift must reject");
        }
    }

    #[test]
    fn extent_backend_field_name_type_offset_and_size_drift_fail_closed() {
        extent_layout()
            .validate_backend_layout(&backend_extent_layout("base", 0, "u64", 16))
            .expect("exact backend Extent layout should replay");
        for backend in [
            backend_extent_layout("address", 0, "u64", 16),
            backend_extent_layout("base", 1, "u64", 16),
            backend_extent_layout("base", 0, "i64", 16),
            backend_extent_layout("base", 0, "u64", 24),
        ] {
            extent_layout()
                .validate_backend_layout(&backend)
                .expect_err("backend Extent structural drift must reject");
        }
    }
}

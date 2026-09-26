//! Checked declaration signature for the exact source machine selected by a
//! target-owned `ProgramEntry` slot.
//!
//! This carrier is captured from typed trees before they are consumed by
//! checked/backend lowering. It owns no runtime value, `Extent`, root
//! authority, placement, or call plan. For the currently admitted
//! program-storage schema it also retains the exact address-free `Extent`
//! record graph needed to distinguish its fields from a size-only lookalike.

use crate::ProgramStorageEntryRootRole;
use sha2::{Digest, Sha256};

const SOURCE_SIGNATURE_SCHEMA: &[u8] = b"omega.program-entry.source-signature.v1\0";

/// Source-path-free identity of one exact checked `ProgramEntry` signature.
/// Arena handles remain private replay coordinates; normalized callable/type
/// identities and the complete target-owned slot and value layout define the
/// durable join key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgramEntrySourceSignatureIdentity([u8; 32]);

impl ProgramEntrySourceSignatureIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

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
    symbol: symbols::SymbolHandle,
    byte_offset: u16,
    shape: calling_conventions::ValueShape,
}

impl ProgramEntrySourceExtentFieldLayout {
    pub const fn role(&self) -> ProgramEntrySourceExtentFieldRole {
        self.role
    }

    pub const fn symbol(&self) -> symbols::SymbolHandle {
        self.symbol
    }

    pub const fn byte_offset(&self) -> u16 {
        self.byte_offset
    }

    pub const fn shape(&self) -> calling_conventions::ValueShape {
        self.shape
    }
}

/// Exact checked record graph for the ordinary value portion of one
/// `Extent in Granted`. Domain authority is deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntrySourceExtentValueLayout {
    data_symbol: symbols::SymbolHandle,
    shape: calling_conventions::ValueShape,
    fields: [ProgramEntrySourceExtentFieldLayout; 2],
}

impl ProgramEntrySourceExtentValueLayout {
    pub const fn data_symbol(&self) -> symbols::SymbolHandle {
        self.data_symbol
    }

    pub const fn shape(&self) -> calling_conventions::ValueShape {
        self.shape
    }

    pub const fn fields(&self) -> &[ProgramEntrySourceExtentFieldLayout; 2] {
        &self.fields
    }

    pub fn from_checked_record(
        data_symbol: symbols::SymbolHandle,
        base_symbol: symbols::SymbolHandle,
        base_offset: u16,
        base_shape: calling_conventions::ValueShape,
        length_symbol: symbols::SymbolHandle,
        length_offset: u16,
        length_shape: calling_conventions::ValueShape,
        shape: calling_conventions::ValueShape,
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
        let scalar = calling_conventions::ValueShape::integer(8, 8);
        if !self.data_symbol.is_valid()
            || self.shape != calling_conventions::ValueShape::integer(16, 8)
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

    pub fn validate_backend_layout(&self, layouts: &layout::LayoutPlan) -> Result<(), String> {
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
        let layout::DataShape::Record { fields } = layout.shape else {
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
    value_shape: calling_conventions::ValueShape,
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
    pub const fn value_shape(&self) -> calling_conventions::ValueShape {
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
    target_slot: target::ProgramEntrySlotDeclaration,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    machine_name: String,
    state_name: String,
    normalized_callable_identity: String,
    receiver: ProgramEntrySourceReceiverSignature,
    visible_parameters: Vec<ProgramEntrySourceVisibleParameterSignature>,
    result: ProgramEntrySourceResultSignature,
}

impl SelectedProgramEntrySourceSignature {
    pub fn from_checked_typed_entry(
        target_slot: target::ProgramEntrySlotDeclaration,
        machine_symbol: symbols::SymbolHandle,
        state_symbol: symbols::SymbolHandle,
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

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub const fn machine_symbol(&self) -> symbols::SymbolHandle {
        self.machine_symbol
    }

    pub const fn state_symbol(&self) -> symbols::SymbolHandle {
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

    /// Recompute the canonical source-signature join identity. Symbol handles
    /// are intentionally excluded because the normalized callable and type
    /// identities already bind semantic ownership while arena numbering is a
    /// property of one frontend run.
    pub fn identity(&self) -> ProgramEntrySourceSignatureIdentity {
        let mut canonical = SOURCE_SIGNATURE_SCHEMA.to_vec();
        push_string(&mut canonical, self.target_slot.owner.target_name());
        push_string(&mut canonical, self.target_slot.slot_name);
        canonical.push(match self.target_slot.schema {
            target::ProgramEntrySchema::HostedApplication => 1,
            target::ProgramEntrySchema::ProgramStorageApplication => 2,
        });
        push_string(
            &mut canonical,
            self.target_slot.semantic_arrival_requirement,
        );
        push_optional_string(
            &mut canonical,
            self.target_slot.physical_arrival_requirement,
        );
        push_optional_string(
            &mut canonical,
            self.target_slot
                .physical_contract_package
                .map(target::ProgramEntryPhysicalContractPackage::manifest_identity),
        );
        push_optional_string(&mut canonical, self.target_slot.boundary_schema);
        push_optional_calling_convention(
            &mut canonical,
            self.target_slot.physical_calling_convention,
        );
        push_optional_calling_convention(
            &mut canonical,
            self.target_slot.semantic_calling_convention,
        );
        canonical.push(match self.target_slot.visible_parameters {
            target::ProgramEntryVisibleParameters::None => 1,
            target::ProgramEntryVisibleParameters::ImageAndInitialStorage => 2,
        });
        canonical.push(match self.target_slot.receiver {
            target::ProgramEntryReceiverProvisioning::NoneOrProvisionedZii => 1,
        });
        push_string(&mut canonical, &self.normalized_callable_identity);
        match &self.receiver {
            ProgramEntrySourceReceiverSignature::Free => canonical.push(1),
            ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity,
            } => {
                canonical.push(2);
                push_string(&mut canonical, normalized_type_identity);
            }
        }
        canonical.extend_from_slice(
            &u64::try_from(self.visible_parameters.len())
                .expect("ProgramEntry parameter count fits u64")
                .to_le_bytes(),
        );
        for parameter in &self.visible_parameters {
            canonical.push(match parameter.role {
                ProgramStorageEntryRootRole::Image => 1,
                ProgramStorageEntryRootRole::InitialStorage => 2,
            });
            canonical.extend_from_slice(
                &u64::try_from(parameter.visible_parameter_index)
                    .expect("ProgramEntry parameter index fits u64")
                    .to_le_bytes(),
            );
            push_string(&mut canonical, &parameter.normalized_type_identity);
            push_shape(&mut canonical, parameter.value_shape);
            canonical.push(u8::from(parameter.is_const));
            canonical.push(u8::from(parameter.is_mutable));
            push_shape(&mut canonical, parameter.extent_value_layout.shape);
            for field in &parameter.extent_value_layout.fields {
                canonical.push(match field.role {
                    ProgramEntrySourceExtentFieldRole::Base => 1,
                    ProgramEntrySourceExtentFieldRole::Length => 2,
                });
                canonical.extend_from_slice(&field.byte_offset.to_le_bytes());
                push_shape(&mut canonical, field.shape);
            }
        }
        canonical.push(match self.result {
            ProgramEntrySourceResultSignature::Unit => 1,
        });
        ProgramEntrySourceSignatureIdentity(Sha256::digest(canonical).into())
    }

    pub fn validate_program_storage_binding(
        &self,
        selected_slot: target::ProgramEntrySlotDeclaration,
        continuation_key: function_identity::StateKey,
        receiver_type_identity: Option<&str>,
        image_type_identity: &str,
        initial_storage_type_identity: &str,
    ) -> Result<(), String> {
        self.validate_capture()?;
        if self.target_slot != selected_slot
            || selected_slot.schema != target::ProgramEntrySchema::ProgramStorageApplication
            || selected_slot.visible_parameters
                != target::ProgramEntryVisibleParameters::ImageAndInitialStorage
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
        value_shape: calling_conventions::ValueShape,
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

fn push_string(canonical: &mut Vec<u8>, value: &str) {
    canonical.extend_from_slice(
        &u64::try_from(value.len())
            .expect("ProgramEntry identity string length fits u64")
            .to_le_bytes(),
    );
    canonical.extend_from_slice(value.as_bytes());
}

fn push_optional_string(canonical: &mut Vec<u8>, value: Option<&str>) {
    match value {
        None => canonical.push(0),
        Some(value) => {
            canonical.push(1);
            push_string(canonical, value);
        }
    }
}

fn push_optional_calling_convention(
    canonical: &mut Vec<u8>,
    value: Option<target::ProgramEntryCallingConvention>,
) {
    canonical.push(match value {
        None => 0,
        Some(target::ProgramEntryCallingConvention::MicrosoftX64) => 1,
        Some(target::ProgramEntryCallingConvention::Aapcs64) => 2,
        Some(target::ProgramEntryCallingConvention::SystemVAMD64) => 3,
    });
}

fn push_shape(canonical: &mut Vec<u8>, shape: calling_conventions::ValueShape) {
    // Exact captured Extent signatures admit only integer shapes; the checked
    // constructor enforces that closed graph before this identity can exist.
    debug_assert!(matches!(
        shape.class,
        calling_conventions::ValueClass::Integer
    ));
    canonical.extend_from_slice(&shape.byte_size.to_le_bytes());
    canonical.extend_from_slice(&shape.alignment.to_le_bytes());
}

#[cfg(test)]
mod tests;

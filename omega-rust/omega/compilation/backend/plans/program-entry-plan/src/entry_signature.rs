//! Source-independent signature for one target-owned `ProgramEntry` slot.
//!
//! The source owner captures declaration identity and the completed value
//! layout. Arena coordinates never cross this boundary. It owns no runtime
//! value, `Extent`, root authority, placement, or call plan. For the currently admitted
//! program-storage schema it also retains the exact address-free `Extent`
//! record graph needed to distinguish its fields from a size-only lookalike.

use crate::ProgramStorageEntryRootRole;

const SOURCE_SIGNATURE_SCHEMA: &[u8] = b"omega.program-entry.source-signature.v4\0";

/// Source-path-free identity of one exact checked `ProgramEntry` signature.
/// Exact declaration ownership, normalized callable/type identities and the
/// complete target-owned slot and value layout define the durable join key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgramEntrySignatureIdentity([u8; 32]);

impl ProgramEntrySignatureIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// ProgramEntry never allocates an implicit receiver. Installed schema
/// arguments are represented individually in `visible_parameters`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramEntryReceiverSignature {
    Free,
}

impl ProgramEntryReceiverSignature {
    pub const fn source_position(&self) -> Option<u32> {
        None
    }
    pub const fn normalized_type_identity(&self) -> Option<terminal_psi::TerminalIdentityIndex> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryResultSignature {
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEntryExtentFieldRole {
    Base,
    Length,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryExtentFieldLayout {
    role: ProgramEntryExtentFieldRole,
    byte_offset: u16,
    shape: calling_conventions::ValueShape,
}

impl ProgramEntryExtentFieldLayout {
    pub const fn role(&self) -> ProgramEntryExtentFieldRole {
        self.role
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
pub struct ProgramEntryExtentValueLayout {
    shape: calling_conventions::ValueShape,
    fields: [ProgramEntryExtentFieldLayout; 2],
}

impl ProgramEntryExtentValueLayout {
    pub const fn shape(&self) -> calling_conventions::ValueShape {
        self.shape
    }

    pub const fn fields(&self) -> &[ProgramEntryExtentFieldLayout; 2] {
        &self.fields
    }

    pub fn from_checked_record(
        base_offset: u16,
        base_shape: calling_conventions::ValueShape,
        length_offset: u16,
        length_shape: calling_conventions::ValueShape,
        shape: calling_conventions::ValueShape,
    ) -> Result<Self, String> {
        let layout = Self {
            shape,
            fields: [
                ProgramEntryExtentFieldLayout {
                    role: ProgramEntryExtentFieldRole::Base,
                    byte_offset: base_offset,
                    shape: base_shape,
                },
                ProgramEntryExtentFieldLayout {
                    role: ProgramEntryExtentFieldRole::Length,
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
        if self.shape != calling_conventions::ValueShape::integer(16, 8)
            || self.fields[0].role != ProgramEntryExtentFieldRole::Base
            || self.fields[0].byte_offset != 0
            || self.fields[0].shape != scalar
            || self.fields[1].role != ProgramEntryExtentFieldRole::Length
            || self.fields[1].byte_offset != 8
            || self.fields[1].shape != scalar
        {
            return Err(
                "selected source Extent must be the exact {base: addr@0, length: u64@8} x86-64 record"
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEntryVisibleParameterSignature {
    role: ProgramStorageEntryRootRole,
    /// Receiver-excluded ordinal in the target schema's visible list. This is
    /// not a source formal index or an outbound ABI parameter index.
    visible_parameter_index: usize,
    normalized_type_identity: terminal_psi::TerminalIdentityIndex,
    value_shape: calling_conventions::ValueShape,
    extent_value_layout: ProgramEntryExtentValueLayout,
    is_const: bool,
    is_mutable: bool,
}

impl ProgramEntryVisibleParameterSignature {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn visible_parameter_index(&self) -> usize {
        self.visible_parameter_index
    }

    pub const fn normalized_type_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.normalized_type_identity
    }

    /// Exact address-free shape derived from this typed source declaration,
    /// independently of the physical arrival calling plan.
    pub const fn value_shape(&self) -> calling_conventions::ValueShape {
        self.value_shape
    }

    pub const fn extent_value_layout(&self) -> &ProgramEntryExtentValueLayout {
        &self.extent_value_layout
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }
}

/// Completed declaration and value-layout evidence for one selected entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntrySignature {
    target_slot: target::ProgramEntrySlotDeclaration,
    declaration: String,
    machine_name: String,
    state_name: String,
    normalized_callable_identity: terminal_psi::TerminalIdentityIndex,
    receiver: ProgramEntryReceiverSignature,
    visible_parameters: Vec<ProgramEntryVisibleParameterSignature>,
    result: ProgramEntryResultSignature,
    /// Identity text every `TerminalIdentityIndex` on this signature and its
    /// bound establishment rows resolves against. The signature is minted
    /// beside the checked program, before a Terminal module exists; native
    /// consumers rejoin these indices through the text, never the slot.
    identities: terminal_psi::TerminalIdentityTable,
}

impl SelectedProgramEntrySignature {
    pub fn from_checked_declaration(
        target_slot: target::ProgramEntrySlotDeclaration,
        declaration: String,
        machine_name: String,
        state_name: String,
        normalized_callable_identity: terminal_psi::TerminalIdentityIndex,
        receiver: ProgramEntryReceiverSignature,
        visible_parameters: Vec<ProgramEntryVisibleParameterSignature>,
        identities: terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, String> {
        let signature = Self {
            target_slot,
            declaration,
            machine_name,
            state_name,
            normalized_callable_identity,
            receiver,
            visible_parameters,
            result: ProgramEntryResultSignature::Unit,
            identities,
        };
        signature.validate_capture()?;
        Ok(signature)
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.target_slot
    }

    pub fn declaration(&self) -> &str {
        &self.declaration
    }

    pub fn machine_name(&self) -> &str {
        &self.machine_name
    }

    pub fn state_name(&self) -> &str {
        &self.state_name
    }

    pub const fn normalized_callable_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.normalized_callable_identity
    }

    /// The plan-local table this signature's identity indices resolve
    /// against — minted beside the checked program before the module's table
    /// exists.
    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    /// Replaces the table after late minting — bound establishments intern
    /// their rows into the signature's table so all ProgramEntry identities
    /// share one ordering.
    pub fn set_identities(&mut self, identities: terminal_psi::TerminalIdentityTable) {
        self.identities = identities;
    }

    pub const fn receiver(&self) -> &ProgramEntryReceiverSignature {
        &self.receiver
    }

    pub fn visible_parameters(&self) -> &[ProgramEntryVisibleParameterSignature] {
        &self.visible_parameters
    }

    pub const fn result(&self) -> ProgramEntryResultSignature {
        self.result
    }

    /// Recompute the canonical signature join identity, including exact package
    /// or toolchain ownership rather than frontend arena numbering.
    pub fn identity(&self) -> ProgramEntrySignatureIdentity {
        let mut canonical = SOURCE_SIGNATURE_SCHEMA.to_vec();
        push_string(&mut canonical, &self.declaration);
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
        push_string(
            &mut canonical,
            self.identities
                .text(self.normalized_callable_identity)
                .unwrap_or_default(),
        );

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
            push_string(
                &mut canonical,
                self.identities
                    .text(parameter.normalized_type_identity)
                    .unwrap_or_default(),
            );
            push_shape(&mut canonical, parameter.value_shape);
            canonical.push(u8::from(parameter.is_const));
            canonical.push(u8::from(parameter.is_mutable));
            push_shape(&mut canonical, parameter.extent_value_layout.shape);
            for field in &parameter.extent_value_layout.fields {
                canonical.push(match field.role {
                    ProgramEntryExtentFieldRole::Base => 1,
                    ProgramEntryExtentFieldRole::Length => 2,
                });
                crate::codec_bytes::push_encoded(&mut canonical, field.byte_offset.to_le_bytes());
                push_shape(&mut canonical, field.shape);
            }
        }
        canonical.push(match self.result {
            ProgramEntryResultSignature::Unit => 1,
        });
        {
            let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"");
            digest.raw(&canonical);
            ProgramEntrySignatureIdentity(digest.finish().into())
        }
    }

    fn validate_capture(&self) -> Result<(), String> {
        if self.target_slot != self.target_slot.owner.program_entry_slot()
            || self.machine_name.is_empty()
            || self.state_name.is_empty()
            || self.normalized_callable_identity.is_absent()
            || self.result != ProgramEntryResultSignature::Unit
        {
            return Err("selected source signature is not one exact typed Unit entry".into());
        }
        if self
            .receiver
            .normalized_type_identity()
            .is_some_and(terminal_psi::TerminalIdentityIndex::is_absent)
            || self.visible_parameters.iter().any(|parameter| {
                parameter.normalized_type_identity.is_absent()
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
        normalized_type_identity: terminal_psi::TerminalIdentityIndex,
        value_shape: calling_conventions::ValueShape,
        extent_value_layout: ProgramEntryExtentValueLayout,
        is_const: bool,
        is_mutable: bool,
    ) -> ProgramEntryVisibleParameterSignature {
        ProgramEntryVisibleParameterSignature {
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
    crate::codec_bytes::push_encoded(canonical, shape.byte_size.to_le_bytes());
    crate::codec_bytes::push_encoded(canonical, shape.alignment.to_le_bytes());
}

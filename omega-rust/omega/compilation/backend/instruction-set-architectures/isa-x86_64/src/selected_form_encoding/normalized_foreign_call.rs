//! Canonical unresolved AMD64 normalized-foreign-call templates.
//!
//! The emitted bytes are the same `E8 + zero rel32` placeholder an internal
//! call uses, but the fixup names the selected `{boundary, ordinal}` roster
//! row rather than an internal machine. The field stays unresolved until
//! object construction binds it to the declared import symbol; raw locator
//! bytes never enter this encoding contract.

use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

use crate::machine_effects::x86_64_selected_abi;

/// Callback-bearing rows place one address-materialization group per private
/// callback destination before the call itself: a `lea <destination>,
/// [rip+disp32]` for a private parameter slot, or a `lea r11,
/// [rip+disp32]` + `mov [root+offset], r11` pair for a record-field
/// destination — then the canonical 5-byte call.
pub(crate) const X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;
pub(crate) const X86_64_CALLBACK_MATERIALIZATION_BYTE_COUNT: u16 = 7;
pub(crate) const X86_64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT: u16 = 14;
pub(crate) const X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET: u16 = 3;

/// Where one callback materialization's resolved private-function address
/// lands: an ABI parameter register, an outgoing stack slot at pre-call RSP,
/// or a private field inside a register-resident record. Memory destinations
/// use scratch `r11`; the retained call row excludes its overlap with inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64CallbackMaterializationDestination {
    Register(calling_conventions::MachineRegister),
    OutgoingStack {
        byte_offset: u32,
    },
    RecordField {
        root_register: calling_conventions::MachineRegister,
        byte_offset: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupKind {
    Relative32FromNextInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupState {
    UnresolvedImportFieldV1,
}

/// Target-owned private-callback materialization record: a `lea
/// <destination>, [rip+disp32]` prefix (optionally followed by the
/// record-field slot store) whose displacement field binds the selected
/// private function symbol at object construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64CallbackAddressMaterialization {
    pub destination: X86_64CallbackMaterializationDestination,
    pub materialization_byte_offset: u16,
    pub materialization_byte_count: u16,
    pub relocation_byte_offset: u16,
}

/// Target-owned unresolved import-field fixup for one normalized foreign
/// call. `{boundary, ordinal}` names the selected roster row owning the
/// evaluated locator; no raw foreign coordinate appears here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64NormalizedForeignCallFixup {
    pub kind: X86_64NormalizedForeignCallFixupKind,
    pub state: X86_64NormalizedForeignCallFixupState,
    pub boundary: BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
    /// One entry per callback destination the call's private schedule names,
    /// in evaluated roster order.
    pub callbacks: Vec<X86_64CallbackAddressMaterialization>,
}

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedNormalizedForeignCallTemplate {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: Vec<RegisterViewId>,
    effects: MachineEncodedEffects,
    bytes: Vec<u8>,
    fixup: X86_64NormalizedForeignCallFixup,
}

impl ValidatedX86_64SelectedNormalizedForeignCallTemplate {
    pub const fn kind(&self) -> SelectedInstructionKind {
        self.kind
    }

    pub const fn alternative(&self) -> MachineAlternativeKey {
        self.alternative
    }

    pub fn operand_views(&self) -> &[RegisterViewId] {
        &self.operand_views
    }

    pub const fn effects(&self) -> &MachineEncodedEffects {
        &self.effects
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn fixup(&self) -> &X86_64NormalizedForeignCallFixup {
        &self.fixup
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallTemplateError {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    InstructionKindMismatch,
    AlternativeMismatch,
    OperandViewMismatch,
    EffectMismatch,
    MalformedTemplate,
    FixupMismatch,
}

impl std::fmt::Display for X86_64NormalizedForeignCallTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 normalized-foreign-call template: {self:?}"
        )
    }
}

impl std::error::Error for X86_64NormalizedForeignCallTemplateError {}

/// Produce `E8 + zero rel32` with an explicit unresolved import-field fixup.
/// `callback_destinations` names each private callback destination in the
/// call's evaluated roster order; every entry emits its materialization
/// group ahead of the call bytes and lands a custody record in the fixup.
pub fn encode_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    callback_destinations: &[X86_64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let mut bytes = Vec::new();
    for destination in callback_destinations {
        bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    let fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
    validate_x86_64_selected_normalized_foreign_call_template(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        operand_accesses,
        effects,
        &bytes,
        fixup,
        callback_destinations,
        record,
    )
}

/// Independently validate every selected input, byte, and unresolved fixup.
/// `callback_destinations` is the same evaluated roster the encoder received:
/// it must equal the exact destinations retained by the independently checked call.
pub fn validate_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: X86_64NormalizedForeignCallFixup,
    callback_destinations: &[X86_64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    x86_64_selected_abi(target)
        .map_err(|_| X86_64NormalizedForeignCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64NormalizedForeignCallTemplateError::NonCanonicalPhysicalModel);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    if record.call.boundary != boundary
        || record.call.binding.native_target() != Some(target)
        || record.call.binding.import().is_none()
    {
        return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let row = crate::machine_effects::normalized_foreign_constraint(record)
        .ok_or(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    if row.operands.len() != operand_views.len()
        || row.operands.len() != operand_accesses.len()
        || row
            .operands
            .iter()
            .zip(operand_views.iter().zip(operand_accesses))
            .any(|(operand, (view, access))| {
                operand.fixed_view != Some(*view) || operand.access != *access
            })
    {
        return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let declaration = crate::x86_64_normalized_foreign_call_declaration(record, &row)
        .ok_or(X86_64NormalizedForeignCallTemplateError::EffectMismatch)?;
    let [expected] = declaration.alternatives.as_slice() else {
        return Err(X86_64NormalizedForeignCallTemplateError::AlternativeMismatch);
    };
    if alternative != expected.key {
        return Err(X86_64NormalizedForeignCallTemplateError::AlternativeMismatch);
    }
    if effects != &expected.encoded {
        return Err(X86_64NormalizedForeignCallTemplateError::EffectMismatch);
    }
    let destinations = record
        .call
        .callbacks
        .iter()
        .map(|callback| {
            match (
                &callback.destination,
                callback.application.placement.locations.as_slice(),
            ) {
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => Some(X86_64CallbackMaterializationDestination::Register(
                    *register,
                )),
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [
                        calling_conventions::ValueLocation::Stack {
                            stack_byte_offset, ..
                        },
                    ],
                ) => Some(X86_64CallbackMaterializationDestination::OutgoingStack {
                    byte_offset: *stack_byte_offset,
                }),
                (
                    calling_conventions::NativePlace::Field { .. },
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => Some(X86_64CallbackMaterializationDestination::RecordField {
                    root_register: *register,
                    byte_offset: u32::try_from(callback.field_byte_offset?).ok()?,
                }),
                _ => None,
            }
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    if destinations != callback_destinations {
        return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let mut expected_bytes = Vec::new();
    for destination in callback_destinations {
        expected_bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    expected_bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    if bytes != expected_bytes.as_slice() {
        return Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
    if fixup != expected_fixup {
        return Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch);
    }
    Ok(ValidatedX86_64SelectedNormalizedForeignCallTemplate {
        kind,
        alternative,
        operand_views: operand_views.to_vec(),
        effects: effects.clone(),
        bytes: bytes.to_vec(),
        fixup,
    })
}

/// One materialization group: `REX.W 8D /r` — `lea <destination>,
/// [rip+disp32]` — for a private parameter slot, or `lea r11,
/// [rip+disp32]` followed by `mov [root+disp32], r11` for a record-field
/// destination. The lea's zero disp32 is the relocation field; the field
/// store's displacement is the literal private-slot offset.
fn callback_group_bytes(destination: X86_64CallbackMaterializationDestination) -> Option<Vec<u8>> {
    match destination {
        X86_64CallbackMaterializationDestination::Register(register) => {
            let index = x86_64_machine_register_encoding(register)?;
            let rex = 0x48 | (u8::from(index >= 8) << 2);
            let modrm = 0x05 | ((index & 7) << 3);
            Some(vec![rex, 0x8d, modrm, 0, 0, 0, 0])
        }
        X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } => {
            if byte_offset > i32::MAX as u32 {
                return None;
            }
            // The offset names the caller's outgoing area at pre-call RSP,
            // not the callee's post-CALL stack or its private frame.
            let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0, 0x4c, 0x89];
            if byte_offset <= i8::MAX as u32 {
                group.extend_from_slice(&[0x5c, 0x24, byte_offset as u8]);
            } else {
                group.extend_from_slice(&[0x9c, 0x24]);
                crate::codec_bytes::push_encoded(&mut group, byte_offset.to_le_bytes());
            }
            Some(group)
        }
        X86_64CallbackMaterializationDestination::RecordField {
            root_register,
            byte_offset,
        } => {
            let root = x86_64_machine_register_encoding(root_register)?;
            // The scratch is overwritten by LEA before the root is observed.
            if root == 11 || byte_offset > i32::MAX as u32 {
                return None;
            }
            // `4C 8D 1D` — lea r11, [rip+disp32] — then `4C|B 89 /r` storing
            // r11 at [root + byte_offset] with a mod=10 disp32 form, which
            // R12 additionally needs the no-index SIB byte.
            let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0];
            let rex = 0x4c | u8::from(root >= 8);
            let modrm = 0x98 | (root & 7);
            group.extend_from_slice(&[rex, 0x89, modrm]);
            if root & 7 == 4 {
                group.push(0x24);
            }
            crate::codec_bytes::push_encoded(&mut group, byte_offset.to_le_bytes());
            Some(group)
        }
    }
}

pub(super) fn x86_64_machine_register_encoding(
    register: calling_conventions::MachineRegister,
) -> Option<u8> {
    Some(match register {
        calling_conventions::MachineRegister::X86Rax => 0,
        calling_conventions::MachineRegister::X86Rcx => 1,
        calling_conventions::MachineRegister::X86Rdx => 2,
        calling_conventions::MachineRegister::X86Rbx => 3,
        calling_conventions::MachineRegister::X86Rsi => 6,
        calling_conventions::MachineRegister::X86Rdi => 7,
        calling_conventions::MachineRegister::X86R8 => 8,
        calling_conventions::MachineRegister::X86R9 => 9,
        calling_conventions::MachineRegister::X86R10 => 10,
        calling_conventions::MachineRegister::X86R11 => 11,
        calling_conventions::MachineRegister::X86R12 => 12,
        calling_conventions::MachineRegister::X86R13 => 13,
        calling_conventions::MachineRegister::X86R14 => 14,
        calling_conventions::MachineRegister::X86R15 => 15,
        _ => return None,
    })
}

fn callback_fixup(
    boundary: BoundaryMachineId,
    ordinal: u32,
    destinations: &[X86_64CallbackMaterializationDestination],
) -> Result<X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallTemplateError> {
    let mut callbacks = Vec::with_capacity(destinations.len());
    let mut offset = 0_u16;
    for destination in destinations {
        let byte_count = match destination {
            X86_64CallbackMaterializationDestination::Register(_) => {
                X86_64_CALLBACK_MATERIALIZATION_BYTE_COUNT
            }
            X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } => {
                if *byte_offset <= i8::MAX as u32 {
                    12
                } else {
                    15
                }
            }
            X86_64CallbackMaterializationDestination::RecordField { root_register, .. } => {
                X86_64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT
                    + u16::from(*root_register == calling_conventions::MachineRegister::X86R12)
            }
        };
        callbacks.push(X86_64CallbackAddressMaterialization {
            destination: *destination,
            materialization_byte_offset: offset,
            materialization_byte_count: byte_count,
            relocation_byte_offset: offset
                .checked_add(X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        });
        offset = offset
            .checked_add(byte_count)
            .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    }
    let patch_byte_offset = offset
        .checked_add(1)
        .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    let reference_byte_offset = offset
        .checked_add(5)
        .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    Ok(X86_64NormalizedForeignCallFixup {
        kind: X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1,
        state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset,
        reference_byte_offset,
        patch_byte_width: X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64NormalizedForeignCallTemplateError, callback_fixup,
        encode_x86_64_selected_normalized_foreign_call_template,
        validate_x86_64_selected_normalized_foreign_call_template,
    };
    use register_model::{
        RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel,
        validate_physical_register_model,
    };
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        SelectedInstructionKind, SelectedNormalizedForeignCall,
    };
    use semantic_vocabulary::BoundaryMachineId;
    use target::NativeTarget;

    use crate::x86_64_physical_register_model;

    fn inputs(
        windows: bool,
        input_layout: usize,
        result: usize,
    ) -> (
        ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        Vec<RegisterViewId>,
        Vec<RegisterOperandAccess>,
        MachineEncodedEffects,
        SelectedNormalizedForeignCall,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 2,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        let record = record(windows, input_layout, result);
        let row = crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
        let operands = row
            .operands
            .iter()
            .map(|operand| operand.fixed_view.unwrap())
            .collect::<Vec<_>>();
        let accesses = row
            .operands
            .iter()
            .map(|operand| operand.access)
            .collect::<Vec<_>>();
        let stack_pointer = physical.model().view_named("rsp").unwrap().id;
        let effects = crate::machine_effects::user_direct_call_effects(&row, stack_pointer);
        (
            physical,
            kind,
            alternative,
            operands,
            accesses,
            effects,
            record,
        )
    }

    /// These records carry physical call inputs only; source ownership and
    /// declarations remain the selection tests' independent responsibility.
    fn record(windows: bool, input_layout: usize, result: usize) -> SelectedNormalizedForeignCall {
        use calling_conventions::{CallSignature, CallingPolicy, ValueShape};
        use semantic_vocabulary::{
            IeeeFloatFormat, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
            StructuralTypeId, ValueId,
        };
        use target_operations::{
            TargetBoundaryResult, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
            TargetStructuralHomeRequirement, TargetUnitScalarHomeRequirement,
        };
        let target = if windows {
            NativeTarget::windows_x64()
        } else {
            NativeTarget::linux_x64()
        };
        let (general, floating) = if input_layout < 7 {
            (input_layout, 0)
        } else {
            ((input_layout - 7) / 8, (input_layout - 7) % 8 + 1)
        };
        let parameters = std::iter::repeat_n(ValueShape::integer(8, 8), general)
            .chain(std::iter::repeat_n(ValueShape::float(8), floating))
            .collect::<Vec<_>>();
        let result_shape = match result {
            0 => None,
            1 => Some(ValueShape::integer(8, 8)),
            2 => Some(ValueShape::float(8)),
            3 => Some(ValueShape::integer(16, 8)),
            4 => Some(if windows {
                ValueShape::integer(16, 8)
            } else {
                ValueShape::system_v_aggregate(
                    16,
                    8,
                    calling_conventions::SystemVEightbyteClass::Integer,
                    calling_conventions::SystemVEightbyteClass::Sse,
                )
            }),
            5 => Some(ValueShape::integer(24, 8)),
            _ => panic!("test result"),
        };
        let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters,
                result: result_shape,
            },
        )
        .unwrap()
        .plan()
        .clone();
        let locator = if windows {
            target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::PeByName {
                    library: b"KERNEL32.dll".to_vec(),
                    export: b"Leaf".to_vec(),
                },
                target::TargetProfile::WindowsX64,
            )
        } else {
            target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"leaf".to_vec(),
                    version: b"GLIBC_2.0".to_vec(),
                },
                target::TargetProfile::LinuxX64,
            )
        }
        .unwrap();
        let commitment = task_plans::SameStackProviderPlanCommitment::from_digest([0x42; 32]);
        let identities = terminal_psi::TerminalIdentityTable::from(vec![
            String::new(),
            "Foreign::leaf".to_owned(),
        ]);
        let requirement_index = identities
            .index_of("Foreign::leaf")
            .expect("test requirement identity must intern");
        let same_stack_contribution = task_plans::admit_same_stack_contribution(
            task_plans::SameStackContributionAdmissionCandidate {
                provider_plan_report_identity: 0xA1,
                provider_plan_commitment: commitment,
                requirement_identity: requirement_index,
                receipt:
                    task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                        0xA2,
                    )
                    .unwrap(),
                bytes: 64,
                alignment: 16,
            },
            0xA1,
            commitment,
            requirement_index,
        )
        .unwrap();
        let operation = OperationId::new(7).unwrap();
        let scalar_type = |floating| {
            if floating {
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
            } else {
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
            }
        };
        let scalar_arguments = boundary_entry_plan
            .call
            .parameters
            .iter()
            .enumerate()
            .map(
                |(position, placement)| target_operations::TargetUnitScalarCallArgument {
                    parameter_index: position as u32,
                    source: target_operations::TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: position as u32,
                        source_value: ValueId::new(100 + position as u64).unwrap(),
                        scalar_type: scalar_type(position >= general),
                    },
                    placement: placement.clone(),
                },
            )
            .collect();
        let completion = match result_shape {
            None => TargetBoundaryResult::Unit,
            Some(shape) if result <= 2 => {
                TargetBoundaryResult::Scalar(TargetUnitScalarHomeRequirement {
                    defining_operation: operation,
                    source_value: ValueId::new(8).unwrap(),
                    scalar_type: scalar_type(result == 2),
                    shape,
                })
            }
            Some(shape) => TargetBoundaryResult::Structural(TargetStructuralHomeRequirement {
                origin: TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    result: terminal_psi::StructuralOperationResult {
                        place: PlaceId::new(9).unwrap(),
                        structural_type: StructuralTypeId::new(2).unwrap(),
                        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
                layout: TargetStructuralHomeLayout::Aggregate(ValueShape::integer(
                    shape.byte_size,
                    shape.alignment,
                )),
            }),
        };
        SelectedNormalizedForeignCall {
            instruction: selected_instructions::SelectedInstructionId(4),
            invocation_site: operation.into(),
            call: legalized_operations::LegalizedNormalizedForeignCall {
                boundary: BoundaryMachineId::new(3).unwrap(),
                provider_execution:
                    target_operations::ProviderExecutionBinding::from_execution_record(
                        target_operations::ProviderPlanReportIdentity::new(0xA1).unwrap(),
                        1,
                        2,
                        3,
                        4,
                    )
                    .unwrap(),
                binding: target_operations::NormalizedForeignCallBinding {
                    boundary_entry_plan,
                    mechanism: target_operations::NormalizedForeignCallMechanism::Import {
                        locator,
                        same_stack_contribution,
                    },
                },
                scalar_arguments,
                structural_arguments: Vec::new(),
                structural_indices: Vec::new(),
                result: completion,
                callbacks: Vec::new(),
            },
            effect: optimization_unit::EffectLink {
                input: 1,
                output: 2,
            },
            ownership: Vec::new(),
        }
    }

    #[test]
    fn table_calls_encode_the_retained_field_and_reject_scratch_aliases() {
        use super::super::table_call::{
            encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
        };
        for windows in [false, true] {
            for byte_offset in [0, 8, 65_536] {
                let (_, _, alternative, operands, accesses, _, mut record) = inputs(windows, 1, 1);
                let same_stack_contribution = record
                    .call
                    .binding
                    .same_stack_contribution()
                    .unwrap()
                    .clone();
                record.call.binding.mechanism =
                    target_operations::NormalizedForeignCallMechanism::Table {
                        target: if windows {
                            target::TargetProfile::WindowsX64
                        } else {
                            target::TargetProfile::LinuxX64
                        },
                        byte_offset,
                        dispatch_only: false,
                        same_stack_contribution,
                    };
                let encoded = encode_x86_64_selected_table_call_form(
                    &record,
                    alternative,
                    &operands,
                    &accesses,
                )
                .unwrap();
                assert_eq!(encoded.bytes.len(), 10);
                assert_eq!(&encoded.bytes[3..7], &byte_offset.to_le_bytes());
                assert_eq!(&encoded.bytes[7..], &[0x41, 0xff, 0xd3]);
                for position in 0..encoded.bytes.len() {
                    let mut corrupted = encoded.bytes.clone();
                    corrupted[position] ^= 1;
                    assert!(
                        validate_x86_64_selected_table_call_form(
                            &record,
                            alternative,
                            &operands,
                            &accesses,
                            &corrupted,
                        )
                        .is_err(),
                        "changed byte {position}"
                    );
                }
                let calling_conventions::ValueLocation::Register { register, .. } =
                    &mut record.call.binding.boundary_entry_plan.call.parameters[0].locations[0]
                else {
                    panic!("one pointer argument must be in a register");
                };
                *register = calling_conventions::MachineRegister::X86R11;
                assert!(
                    encode_x86_64_selected_table_call_form(
                        &record,
                        alternative,
                        &operands,
                        &accesses,
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn retained_multiple_callbacks_bind_operands_effects_bytes_and_fixups() {
        use calling_conventions::{
            CallbackBinderRequirement, CallbackMaterialization, CallbackMaterializationContext,
            CallbackRequirementId, LayoutPlanId, LayoutSlotId, NativeCallbackDemand,
            NativeParameterApplication, NativeParameterId, NativePlace, StaticMachineBinderId,
            ValueLocation,
        };
        use function_identity::MachineFunctionIdentity;

        for alternate_abi in [false, true] {
            let (physical, kind, _, _, _, _, mut record) = inputs(alternate_abi, 3, 1);
            let target = record.call.binding.native_target().unwrap();
            let original = crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
            let root = NativeParameterId::new(81).unwrap();
            let destinations = [
                NativePlace::Parameter(NativeParameterId::new(82).unwrap()),
                NativePlace::Field {
                    parameter: root,
                    layout: LayoutPlanId::new(83).unwrap(),
                    field_path: vec![LayoutSlotId::new(1).unwrap()],
                },
                NativePlace::Parameter(NativeParameterId::new(84).unwrap()),
                NativePlace::Field {
                    parameter: root,
                    layout: LayoutPlanId::new(83).unwrap(),
                    field_path: vec![LayoutSlotId::new(2).unwrap()],
                },
            ];
            let requirement = CallbackRequirementId::new(85).unwrap();
            let context = CallbackMaterializationContext {
                binders: (1..=4)
                    .map(|identity| CallbackBinderRequirement {
                        binder: StaticMachineBinderId::new(identity).unwrap(),
                        requirement,
                    })
                    .collect(),
                demands: destinations
                    .iter()
                    .cloned()
                    .map(|destination| NativeCallbackDemand {
                        destination,
                        requirement,
                    })
                    .collect(),
            };
            let plan = &mut record.call.binding.boundary_entry_plan;
            plan.call.callback_materializations = destinations
                .iter()
                .cloned()
                .enumerate()
                .map(|(position, destination)| CallbackMaterialization {
                    binder: context.binders[position].binder,
                    destination,
                })
                .collect();
            let continuation = terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
            };
            let mut encoded_destinations = Vec::new();
            for (placement_index, destination) in destinations.into_iter().enumerate() {
                let (parameter, native_ordinal, field_byte_offset) = match destination {
                    NativePlace::Parameter(parameter) => (parameter, placement_index / 2 + 1, None),
                    NativePlace::Field { parameter, .. } => {
                        (parameter, 0, Some((placement_index / 2) * 8))
                    }
                };
                let placement = plan.call.parameters[native_ordinal].clone();
                let [ValueLocation::Register { register, .. }] = placement.locations.as_slice()
                else {
                    panic!("fixture callback root must be register resident")
                };
                encoded_destinations.push(match field_byte_offset {
                    None => super::X86_64CallbackMaterializationDestination::Register(*register),
                    Some(byte_offset) => {
                        super::X86_64CallbackMaterializationDestination::RecordField {
                            root_register: *register,
                            byte_offset: byte_offset as u32,
                        }
                    }
                });
                record
                    .call
                    .callbacks
                    .push(target_operations::TargetNativeCallbackArgument {
                        registrar_invokes_callback: true,
                        terminal_operation: record
                            .invocation_site
                            .operation()
                            .expect("callback is owned by a returning operation"),
                        placement_index,
                        callback_function: MachineFunctionIdentity::callback_thunk(
                            continuation,
                            placement_index,
                        )
                        .unwrap(),
                        destination,
                        field_byte_offset,
                        application: NativeParameterApplication {
                            parameter,
                            native_ordinal: native_ordinal as u32,
                            shape: placement.shape,
                            placement,
                        },
                        registrar_boundary_entry_plan: plan.clone(),
                        registrar_context: context.clone(),
                        registrar_application_commitment: [0x66; 32],
                    });
            }
            // The two private parameter slots are materialized inside the call.
            // The field root remains an explicit input; source custody is checked upstream.
            record.call.scalar_arguments.truncate(1);
            let scratch_register = plan.call.policy.callback_field_scratch();
            let row = crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
            assert_eq!(row.key, original.key);
            assert_eq!(row.operands.len(), 2);
            assert_eq!(row.operands[0], original.operands[0]);
            assert_eq!(row.operands[1].access, RegisterOperandAccess::Def);
            assert_eq!(row.operands[1].fixed_view, original.operands[3].fixed_view);
            for register in [
                match encoded_destinations[0] {
                    super::X86_64CallbackMaterializationDestination::Register(register) => register,
                    _ => unreachable!(),
                },
                match encoded_destinations[2] {
                    super::X86_64CallbackMaterializationDestination::Register(register) => register,
                    _ => unreachable!(),
                },
                scratch_register,
            ] {
                let view = crate::x86_64_fixed_register_view(&physical, register).unwrap();
                let view = physical.model().view(view).unwrap();
                assert!(
                    view.units
                        .iter()
                        .all(|unit| row.implicit_uses.contains(unit))
                );
            }
            let declaration =
                crate::x86_64_normalized_foreign_call_declaration(&record, &row).unwrap();
            let alternative = &declaration.alternatives[0];
            assert_eq!(alternative.key.variant, 3);
            assert_eq!(
                alternative.size,
                selected_instructions::MachineSizeKnowledge::ExactBytes(47)
            );
            assert_eq!(alternative.encoded.external_operand_reads, vec![0]);
            assert_eq!(alternative.encoded.external_operand_writes, vec![1]);
            let operands = row
                .operands
                .iter()
                .map(|operand| operand.fixed_view.unwrap())
                .collect::<Vec<_>>();
            let accesses = row
                .operands
                .iter()
                .map(|operand| operand.access)
                .collect::<Vec<_>>();
            let template = encode_x86_64_selected_normalized_foreign_call_template(
                target,
                &physical,
                kind,
                alternative.key,
                &operands,
                &accesses,
                &alternative.encoded,
                &encoded_destinations,
                &record,
            )
            .unwrap();
            assert_eq!(template.bytes().len(), 47);
            assert_eq!(template.fixup().opcode_byte_offset, 42);
            assert_eq!(template.fixup().callbacks.len(), 4);
            let mut byte_offset = 0;
            for (position, fixup) in template.fixup().callbacks.iter().enumerate() {
                assert_eq!(fixup.destination, encoded_destinations[position]);
                assert_eq!(fixup.materialization_byte_offset, byte_offset);
                assert_eq!(
                    fixup.materialization_byte_count,
                    if position % 2 == 0 { 7 } else { 14 }
                );
                byte_offset += fixup.materialization_byte_count;
            }
            let replay =
                |bytes: &[u8], fixup, effects: &MachineEncodedEffects, destinations: &[_]| {
                    validate_x86_64_selected_normalized_foreign_call_template(
                        target,
                        &physical,
                        kind,
                        alternative.key,
                        &operands,
                        &accesses,
                        effects,
                        bytes,
                        fixup,
                        destinations,
                        &record,
                    )
                };
            assert!(
                replay(
                    template.bytes(),
                    template.fixup().clone(),
                    &alternative.encoded,
                    &encoded_destinations
                )
                .is_ok()
            );
            let mut changed_bytes = template.bytes().to_vec();
            changed_bytes[0] ^= 1;
            assert!(
                replay(
                    &changed_bytes,
                    template.fixup().clone(),
                    &alternative.encoded,
                    &encoded_destinations
                )
                .is_err()
            );
            let mut changed_fixup = template.fixup().clone();
            changed_fixup.callbacks[1].materialization_byte_offset += 1;
            assert!(
                replay(
                    template.bytes(),
                    changed_fixup,
                    &alternative.encoded,
                    &encoded_destinations
                )
                .is_err()
            );
            let mut changed_destinations = encoded_destinations.clone();
            changed_destinations.swap(0, 2);
            assert!(
                replay(
                    template.bytes(),
                    template.fixup().clone(),
                    &alternative.encoded,
                    &changed_destinations
                )
                .is_err()
            );
            let mut changed_effects = alternative.encoded.clone();
            changed_effects.implicit_unit_uses.clear();
            assert!(
                replay(
                    template.bytes(),
                    template.fixup().clone(),
                    &changed_effects,
                    &encoded_destinations
                )
                .is_err()
            );
            let mut changed_row = row.clone();
            changed_row.implicit_uses.clear();
            assert!(
                crate::x86_64_normalized_foreign_call_declaration(&record, &changed_row).is_none()
            );
            let mut changed_record = record.clone();
            changed_record.call.callbacks[0].application.native_ordinal = 0;
            assert!(
                crate::machine_effects::normalized_foreign_constraint(&changed_record).is_none()
            );
            let mut changed_record = record.clone();
            changed_record.call.callbacks[1].field_byte_offset = None;
            assert!(
                crate::machine_effects::normalized_foreign_constraint(&changed_record).is_none()
            );
            let mut changed_record = record.clone();
            changed_record.call.callbacks[0].terminal_operation =
                semantic_vocabulary::OperationId::new(99).unwrap();
            assert!(
                crate::x86_64_normalized_foreign_call_declaration(&changed_record, &row).is_none()
            );
        }
    }

    #[test]
    fn direct_and_hidden_aggregate_results_share_the_import_template() {
        for windows in [false, true] {
            let target = if windows {
                NativeTarget::windows_x64()
            } else {
                NativeTarget::linux_x64()
            };
            let mut shared_key = None;
            for result in 0..=5 {
                let (physical, kind, alternative, operands, accesses, effects, record) =
                    inputs(windows, 11, result);
                let row = crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
                if let Some(key) = shared_key {
                    assert_eq!(row.key, key);
                } else {
                    shared_key = Some(row.key);
                }
                let declaration =
                    crate::x86_64_normalized_foreign_call_declaration(&record, &row).unwrap();
                assert_eq!(declaration.alternatives[0].encoded, effects);
                let template = encode_x86_64_selected_normalized_foreign_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                    &[],
                    &record,
                )
                .unwrap();
                assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
                let mut corrupted = effects.clone();
                corrupted.implicit_unit_clobbers.clear();
                assert_eq!(
                    encode_x86_64_selected_normalized_foreign_call_template(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        &accesses,
                        &corrupted,
                        &[],
                        &record
                    ),
                    Err(X86_64NormalizedForeignCallTemplateError::EffectMismatch)
                );
                let mut changed_row = row.clone();
                changed_row.operands.last_mut().unwrap().access = RegisterOperandAccess::UseDef;
                assert!(
                    crate::x86_64_normalized_foreign_call_declaration(&record, &changed_row)
                        .is_none()
                );
                let mut changed_record = record.clone();
                changed_record
                    .call
                    .binding
                    .boundary_entry_plan
                    .call
                    .parameters[0]
                    .locations
                    .clear();
                assert!(
                    crate::machine_effects::normalized_foreign_constraint(&changed_record)
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn callback_stack_bytes_preserve_outgoing_offsets_and_fixup_extent() {
        use super::{X86_64CallbackMaterializationDestination, callback_group_bytes};
        for (offset, store) in [
            (0, vec![0x4c, 0x89, 0x5c, 0x24, 0]),
            (32, vec![0x4c, 0x89, 0x5c, 0x24, 32]),
            (127, vec![0x4c, 0x89, 0x5c, 0x24, 127]),
            (128, vec![0x4c, 0x89, 0x9c, 0x24, 128, 0, 0, 0]),
            (256, vec![0x4c, 0x89, 0x9c, 0x24, 0, 1, 0, 0]),
        ] {
            let destination = X86_64CallbackMaterializationDestination::OutgoingStack {
                byte_offset: offset,
            };
            let bytes = callback_group_bytes(destination).unwrap();
            assert_eq!(&bytes[..7], &[0x4c, 0x8d, 0x1d, 0, 0, 0, 0]);
            assert_eq!(&bytes[7..], store);
            let fixup =
                callback_fixup(BoundaryMachineId::new(3).unwrap(), 0, &[destination]).unwrap();
            assert_eq!(usize::from(fixup.opcode_byte_offset), bytes.len());
            assert_eq!(
                usize::from(fixup.callbacks[0].materialization_byte_count),
                bytes.len()
            );
            assert_eq!(fixup.callbacks[0].relocation_byte_offset, 3);
        }
        assert!(
            callback_group_bytes(X86_64CallbackMaterializationDestination::OutgoingStack {
                byte_offset: i32::MAX as u32 + 1,
            })
            .is_none()
        );
    }

    #[test]
    fn callback_field_bytes_preserve_root_addressing_and_fixup_extent() {
        use super::{X86_64CallbackMaterializationDestination, callback_group_bytes};
        use calling_conventions::MachineRegister;
        for (root, length) in [(MachineRegister::X86Rdi, 14), (MachineRegister::X86R12, 15)] {
            let destination = X86_64CallbackMaterializationDestination::RecordField {
                root_register: root,
                byte_offset: 24,
            };
            let bytes = callback_group_bytes(destination).unwrap();
            assert_eq!(bytes.len(), length);
            assert_eq!(&bytes[..7], &[0x4c, 0x8d, 0x1d, 0, 0, 0, 0]);
            assert_eq!(&bytes[length - 4..], &24_u32.to_le_bytes());
            if root == MachineRegister::X86R12 {
                assert_eq!(&bytes[7..11], &[0x4d, 0x89, 0x9c, 0x24]);
            }
            let fixup =
                callback_fixup(BoundaryMachineId::new(3).unwrap(), 0, &[destination]).unwrap();
            assert_eq!(usize::from(fixup.opcode_byte_offset), length);
            assert_eq!(
                usize::from(fixup.callbacks[0].materialization_byte_count),
                length
            );
        }
        for (root, byte_offset) in [
            (MachineRegister::X86R11, 0),
            (MachineRegister::X86Rdi, u32::MAX),
        ] {
            assert!(
                callback_group_bytes(X86_64CallbackMaterializationDestination::RecordField {
                    root_register: root,
                    byte_offset
                })
                .is_none()
            );
        }
    }

    #[test]
    fn template_has_exact_bytes_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects, record) =
            inputs(false, 1, 1);
        let template = encode_x86_64_selected_normalized_foreign_call_template(
            NativeTarget::linux_x64(),
            &physical,
            kind,
            alternative,
            &operands,
            &accesses,
            &effects,
            &[],
            &record,
        )
        .unwrap();
        assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
        assert_eq!(template.kind(), kind);
        assert_eq!(template.alternative(), alternative);
        assert_eq!(template.operand_views(), &operands);
        assert_eq!(template.effects(), &effects);
        let fixup = template.fixup().clone();
        assert_eq!(
            fixup,
            callback_fixup(BoundaryMachineId::new(3).unwrap(), 2, &[]).unwrap()
        );
        assert_eq!(fixup.opcode_byte_offset, 0);
        assert_eq!(fixup.patch_byte_offset, 1);
        assert_eq!(fixup.reference_byte_offset, 5);
        assert_eq!(fixup.patch_byte_width, 4);
    }

    #[test]
    fn evaluated_plans_select_exact_operands_and_effects_under_one_template() {
        let (physical, kind, alternative, _, _, _, _) = inputs(false, 0, 0);
        for input_layout in 0..63 {
            for result in 0..3 {
                let (_, _, _, operands, accesses, effects, record) =
                    inputs(false, input_layout, result);
                let template = encode_x86_64_selected_normalized_foreign_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                    &[],
                    &record,
                )
                .unwrap();
                assert_eq!(template.operand_views(), operands.as_slice());
                // A different result choice is a valid row, but cannot retain
                // this row's external writes and remaining caller-save effects.
                let (_, _, _, wrong, wrong_accesses, _, _) =
                    inputs(false, input_layout, (result + 1) % 3);
                assert_eq!(
                    encode_x86_64_selected_normalized_foreign_call_template(
                        NativeTarget::linux_x64(),
                        &physical,
                        kind,
                        alternative,
                        &wrong,
                        &wrong_accesses,
                        &effects,
                        &[],
                        &record,
                    ),
                    Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
                );
                if !operands.is_empty() {
                    // This view belongs to neither the input nor result bank.
                    let mut substituted = operands.clone();
                    substituted[0] = physical.model().view_named("r11").unwrap().id;
                    assert_eq!(
                        encode_x86_64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_x64(),
                            &physical,
                            kind,
                            alternative,
                            &substituted,
                            &accesses,
                            &effects,
                            &[],
                            &record,
                        ),
                        Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
                    );
                    // Flip the actual role: result-only rows start with Def,
                    // whereas nonempty input layouts start with Use. If the
                    // new roles describe another valid row, stale effects reject.
                    let mut swapped_accesses = accesses.clone();
                    swapped_accesses[0] = match accesses[0] {
                        RegisterOperandAccess::Use => RegisterOperandAccess::Def,
                        RegisterOperandAccess::Def => RegisterOperandAccess::Use,
                        RegisterOperandAccess::UseDef => unreachable!(),
                    };
                    assert!(
                        encode_x86_64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_x64(),
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &swapped_accesses,
                            &effects,
                            &[],
                            &record,
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn microsoft_calls_select_their_exact_operands() {
        let (physical, kind, alternative, operands, accesses, effects, record) = inputs(true, 2, 1);
        assert!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::windows_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                &[],
                &record
            )
            .is_ok()
        );
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::windows_x64(),
                &physical,
                kind,
                alternative,
                &[physical.model().view_named("rdi").unwrap().id],
                &[RegisterOperandAccess::Use],
                &effects,
                &[],
                &record
            ),
            Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
        );
    }

    #[test]
    fn rejects_wrong_kind_target_alternative_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects, record) =
            inputs(false, 1, 1);
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                SelectedInstructionKind::CallScalar {
                    target: selected_instructions::SelectedCallTarget::Machine(
                        semantic_vocabulary::MachineId::new(4).unwrap()
                    ),
                },
                alternative,
                &operands,
                &accesses,
                &effects,
                &[],
                &record,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch)
        );
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_arm64()] {
            assert_eq!(
                encode_x86_64_selected_normalized_foreign_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                    &[],
                    &record,
                ),
                Err(X86_64NormalizedForeignCallTemplateError::UnsupportedTarget)
            );
        }
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::CallScalar,
                    variant: 0,
                },
                &operands,
                &accesses,
                &effects,
                &[],
                &record,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::AlternativeMismatch)
        );
        let template = encode_x86_64_selected_normalized_foreign_call_template(
            NativeTarget::linux_x64(),
            &physical,
            kind,
            alternative,
            &operands,
            &accesses,
            &effects,
            &[],
            &record,
        )
        .unwrap();
        // A substituted boundary or ordinal cannot validate.
        let mut foreign = template.fixup().clone();
        foreign.boundary = BoundaryMachineId::new(9).unwrap();
        assert_eq!(
            validate_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                template.bytes(),
                foreign,
                &[],
                &record,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        foreign = template.fixup().clone();
        foreign.ordinal = 7;
        assert_eq!(
            validate_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                template.bytes(),
                foreign,
                &[],
                &record,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        // The canonical placeholder is the only admitted byte sequence.
        assert_eq!(
            validate_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                &[0xe8, 0, 0, 0, 1],
                template.fixup().clone(),
                &[],
                &record,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)
        );
        // A complete effect mismatch rejects.
        let mut wrong_effects = effects.clone();
        wrong_effects.implicit_unit_clobbers.clear();
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &wrong_effects,
                &[],
                &record,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::EffectMismatch)
        );
    }
}

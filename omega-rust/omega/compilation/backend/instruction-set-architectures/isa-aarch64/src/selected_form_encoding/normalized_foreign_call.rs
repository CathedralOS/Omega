//! Canonical unresolved AArch64 normalized-foreign-call templates.
//!
//! The emitted word is the same `BL #0` placeholder an internal call uses,
//! but the fixup names the selected `{boundary, ordinal}` roster row rather
//! than an internal machine. Private callbacks materialize their function
//! address before the call word: one `adrp`/`add` pair per register
//! destination, and that pair plus a `str` through the architecture-fixed
//! scratch register for a record-field destination. The field stays
//! unresolved until object construction binds it to the declared import
//! symbol; raw locator bytes never enter this encoding contract.

use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

use crate::machine_effects::aarch64_selected_abi;

pub const AARCH64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT: usize = 4;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_OPCODE_OFFSET: u16 = 0;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_OFFSET: u16 = 0;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_REFERENCE_OFFSET: u16 = 0;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;

/// One register-destination materialization group: an `adrp`/`add` pair.
/// A record-field destination adds the scratch store word after its pair.
pub const AARCH64_CALLBACK_MATERIALIZATION_BYTE_COUNT: u16 = 8;
pub const AARCH64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT: u16 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallFixupKind {
    BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallFixupState {
    UnresolvedImportFieldV1,
}

/// Where one private callback address lands when the call executes: the
/// parameter register the provider plan assigned, or a field inside the
/// record addressed by `root_register` at `byte_offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CallbackMaterializationDestination {
    Register(calling_conventions::MachineRegister),
    RecordField {
        root_register: calling_conventions::MachineRegister,
        byte_offset: u32,
    },
}

/// Target-owned private-callback materialization record: an `adrp`/`add`
/// pair whose page and page-offset fields bind the selected private function
/// symbol at object construction, followed by the scratch store when the
/// destination is a record field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64CallbackAddressMaterialization {
    pub destination: Aarch64CallbackMaterializationDestination,
    pub materialization_byte_offset: u16,
    pub materialization_byte_count: u16,
    pub page_relocation_byte_offset: u16,
    pub page_offset_relocation_byte_offset: u16,
}

/// Target-owned unresolved import-field fixup for one normalized foreign
/// call. `{boundary, ordinal}` names the selected roster row owning the
/// evaluated locator; `callbacks` carries every private-callback
/// materialization in the call's roster order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64NormalizedForeignCallFixup {
    pub kind: Aarch64NormalizedForeignCallFixupKind,
    pub state: Aarch64NormalizedForeignCallFixupState,
    pub boundary: BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
    pub callbacks: Vec<Aarch64CallbackAddressMaterialization>,
}

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64SelectedNormalizedForeignCallTemplate {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: Vec<RegisterViewId>,
    effects: MachineEncodedEffects,
    bytes: Vec<u8>,
    fixup: Aarch64NormalizedForeignCallFixup,
}

impl ValidatedAarch64SelectedNormalizedForeignCallTemplate {
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

    pub const fn fixup(&self) -> &Aarch64NormalizedForeignCallFixup {
        &self.fixup
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallTemplateError {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    InstructionKindMismatch,
    AlternativeMismatch,
    OperandViewMismatch,
    EffectMismatch,
    MalformedTemplate,
    FixupMismatch,
}

impl std::fmt::Display for Aarch64NormalizedForeignCallTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 normalized-foreign-call template: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64NormalizedForeignCallTemplateError {}

/// Produce the canonical `BL #0` word, preceded by one materialization
/// group per callback destination in roster order, with an unresolved
/// import-field fixup naming every group.
pub fn encode_aarch64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    callback_destinations: &[Aarch64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedAarch64SelectedNormalizedForeignCallTemplate,
    Aarch64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let mut bytes = Vec::new();
    for destination in callback_destinations {
        bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?,
        );
    }
    bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
    let fixup = callback_fixup(boundary, ordinal, callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
    validate_aarch64_selected_normalized_foreign_call_template(
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
pub fn validate_aarch64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: Aarch64NormalizedForeignCallFixup,
    callback_destinations: &[Aarch64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedAarch64SelectedNormalizedForeignCallTemplate,
    Aarch64NormalizedForeignCallTemplateError,
> {
    aarch64_selected_abi(target)
        .map_err(|_| Aarch64NormalizedForeignCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64NormalizedForeignCallTemplateError::NonCanonicalPhysicalModel);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    if record.call.boundary != boundary
        || record.call.binding.native_target() != Some(target)
        || record.call.binding.import().is_none()
    {
        return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let row = crate::machine_effects::normalized_foreign_constraint(record)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
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
        return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let declaration = crate::aarch64_normalized_foreign_call_declaration(record, &row)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::EffectMismatch)?;
    let [expected] = declaration.alternatives.as_slice() else {
        return Err(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch);
    };
    if alternative != expected.key {
        return Err(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch);
    }
    if effects != &expected.encoded {
        return Err(Aarch64NormalizedForeignCallTemplateError::EffectMismatch);
    }
    let destinations = record
        .call
        .callbacks
        .iter()
        .map(|callback| {
            let [calling_conventions::ValueLocation::Register { register, .. }] =
                callback.application.placement.locations.as_slice()
            else {
                return None;
            };
            match callback.destination {
                calling_conventions::NativePlace::Parameter(_) => Some(
                    Aarch64CallbackMaterializationDestination::Register(*register),
                ),
                calling_conventions::NativePlace::Field { .. } => {
                    Some(Aarch64CallbackMaterializationDestination::RecordField {
                        root_register: *register,
                        byte_offset: u32::try_from(callback.field_byte_offset?).ok()?,
                    })
                }
            }
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    if destinations != callback_destinations {
        return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    let mut expected_bytes = Vec::new();
    for destination in callback_destinations {
        expected_bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    expected_bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
    if bytes != expected_bytes.as_slice() {
        return Err(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)?;
    if fixup != expected_fixup {
        return Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch);
    }
    Ok(ValidatedAarch64SelectedNormalizedForeignCallTemplate {
        kind,
        alternative,
        operand_views: operand_views.to_vec(),
        effects: effects.clone(),
        bytes: bytes.to_vec(),
        fixup,
    })
}

/// One callback's materialization group: `adrp`/`add` pair into the
/// destination register — or the scratch `x16` for a field destination —
/// followed by `str x16, [xRoot, #off]` for field destinations.
fn callback_group_bytes(destination: Aarch64CallbackMaterializationDestination) -> Option<Vec<u8>> {
    match destination {
        Aarch64CallbackMaterializationDestination::Register(register) => {
            let calling_conventions::MachineRegister::Aarch64X(index) = register else {
                return None;
            };
            if index > 30 {
                return None;
            }
            let adrp = (0x9000_0000_u32 | u32::from(index)).to_le_bytes();
            let add = (0x9100_0000_u32 | (u32::from(index) << 5) | u32::from(index)).to_le_bytes();
            Some(vec![
                adrp[0], adrp[1], adrp[2], adrp[3], add[0], add[1], add[2], add[3],
            ])
        }
        Aarch64CallbackMaterializationDestination::RecordField {
            root_register,
            byte_offset,
        } => {
            let calling_conventions::MachineRegister::Aarch64X(root_index) = root_register else {
                return None;
            };
            if root_index > 30 || byte_offset % 8 != 0 || byte_offset / 8 >= 4096 {
                return None;
            }
            let scratch = 16_u32;
            let adrp = (0x9000_0000_u32 | scratch).to_le_bytes();
            let add = (0x9100_0000_u32 | (scratch << 5) | scratch).to_le_bytes();
            let store = (0xF900_0000_u32
                | ((byte_offset / 8) << 10)
                | (u32::from(root_index) << 5)
                | scratch)
                .to_le_bytes();
            Some(vec![
                adrp[0], adrp[1], adrp[2], adrp[3], add[0], add[1], add[2], add[3], store[0],
                store[1], store[2], store[3],
            ])
        }
    }
}

fn callback_fixup(
    boundary: BoundaryMachineId,
    ordinal: u32,
    destinations: &[Aarch64CallbackMaterializationDestination],
) -> Option<Aarch64NormalizedForeignCallFixup> {
    let mut materializations = Vec::with_capacity(destinations.len());
    let mut offset = 0_u16;
    for destination in destinations {
        let byte_count = match destination {
            Aarch64CallbackMaterializationDestination::Register(_) => {
                AARCH64_CALLBACK_MATERIALIZATION_BYTE_COUNT
            }
            Aarch64CallbackMaterializationDestination::RecordField { .. } => {
                AARCH64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT
            }
        };
        materializations.push(Aarch64CallbackAddressMaterialization {
            destination: *destination,
            materialization_byte_offset: offset,
            materialization_byte_count: byte_count,
            page_relocation_byte_offset: offset,
            page_offset_relocation_byte_offset: offset + 4,
        });
        offset = offset.checked_add(byte_count)?;
    }
    Some(Aarch64NormalizedForeignCallFixup {
        kind: Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset: offset,
        reference_byte_offset: offset,
        patch_byte_width: AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks: materializations,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Aarch64NormalizedForeignCallTemplateError, callback_fixup,
        encode_aarch64_selected_normalized_foreign_call_template,
        validate_aarch64_selected_normalized_foreign_call_template,
    };
    use register_model::{
        RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel,
        validate_physical_register_model,
    };
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, SelectedInstructionKind, SelectedNormalizedForeignCall,
    };
    use semantic_vocabulary::BoundaryMachineId;
    use target::NativeTarget;

    use crate::aarch64_physical_register_model;

    fn inputs(
        darwin: bool,
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
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 2,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        let record = record(darwin, input_layout, result);
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
        let effects = MachineEncodedEffects {
            external_operand_reads: row
                .operands
                .iter()
                .filter(|operand| operand.access == register_model::RegisterOperandAccess::Use)
                .map(|operand| operand.operand)
                .collect(),
            external_operand_writes: row
                .operands
                .iter()
                .filter(|operand| operand.access == register_model::RegisterOperandAccess::Def)
                .map(|operand| operand.operand)
                .collect(),
            implicit_unit_uses: row.implicit_uses.clone(),
            implicit_unit_defs: row.implicit_defs.clone(),
            implicit_unit_clobbers: row.clobbers.clone(),
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::DirectRelativeCallV1,
        };
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
    fn record(darwin: bool, input_layout: usize, result: usize) -> SelectedNormalizedForeignCall {
        use calling_conventions::{CallSignature, CallingPolicy, ValueShape};
        use semantic_vocabulary::{
            IeeeFloatFormat, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
            StructuralTypeId, ValueId,
        };
        use target_operations::{
            TargetBoundaryResult, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
            TargetStructuralHomeRequirement, TargetUnitScalarHomeRequirement,
        };
        let target = if darwin {
            NativeTarget::macos_arm64()
        } else {
            NativeTarget::linux_arm64()
        };
        let (general, floating) = if input_layout < 9 {
            (input_layout, 0)
        } else {
            ((input_layout - 9) / 8, (input_layout - 9) % 8 + 1)
        };
        let parameters = std::iter::repeat_n(ValueShape::integer(8, 8), general)
            .chain(std::iter::repeat_n(ValueShape::float(8), floating))
            .collect::<Vec<_>>();
        let result_shape = match result {
            0 => None,
            1 => Some(ValueShape::integer(8, 8)),
            2 => Some(ValueShape::float(8)),
            3 => Some(ValueShape::integer(16, 8)),
            4 => Some(ValueShape::homogeneous_float_aggregate(8, 2)),
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
        let locator = if darwin {
            target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: b"_leaf".to_vec(),
                },
                target::TargetProfile::MacosArm64,
            )
        } else {
            target::normalize_foreign_locator(
                target::ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: b"leaf".to_vec(),
                    version: b"GLIBC_2.0".to_vec(),
                },
                target::TargetProfile::LinuxArm64,
            )
        }
        .unwrap();
        let commitment = task_plans::SameStackProviderPlanCommitment::from_digest([0x42; 32]);
        let same_stack_contribution = task_plans::admit_same_stack_contribution(
            task_plans::SameStackContributionAdmissionCandidate {
                provider_plan_report_identity: 0xA1,
                provider_plan_commitment: commitment,
                requirement_identity: "Foreign::leaf".into(),
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
            "Foreign::leaf",
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
            operation,
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
    fn retained_multiple_callbacks_bind_operands_effects_bytes_and_fixups() {
        use calling_conventions::{
            CallbackBinderRequirement, CallbackMaterialization, CallbackMaterializationContext,
            CallbackRequirementId, LayoutPlanId, LayoutSlotId, NativeCallbackDemand,
            NativeParameterApplication, NativeParameterId, NativePlace, StaticMachineBinderId,
            ValueLocation,
        };
        use function_identity::{MachineFunctionIdentity, StateKey};
        use symbols::SymbolHandle;

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
            let continuation = StateKey {
                machine: SymbolHandle::from_parts(1, 1),
                state: SymbolHandle::from_parts(2, 1),
                segment_index: 0,
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
                    None => super::Aarch64CallbackMaterializationDestination::Register(*register),
                    Some(byte_offset) => {
                        super::Aarch64CallbackMaterializationDestination::RecordField {
                            root_register: *register,
                            byte_offset: byte_offset as u32,
                        }
                    }
                });
                record
                    .call
                    .callbacks
                    .push(target_operations::TargetNativeCallbackArgument {
                        terminal_operation: record.operation,
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
                    super::Aarch64CallbackMaterializationDestination::Register(register) => {
                        register
                    }
                    _ => unreachable!(),
                },
                match encoded_destinations[2] {
                    super::Aarch64CallbackMaterializationDestination::Register(register) => {
                        register
                    }
                    _ => unreachable!(),
                },
                scratch_register,
            ] {
                let view = crate::aarch64_fixed_register_view(&physical, register).unwrap();
                let view = physical
                    .model()
                    .views
                    .iter()
                    .find(|candidate| candidate.id == view)
                    .unwrap();
                assert!(
                    view.units
                        .iter()
                        .all(|unit| row.implicit_uses.contains(unit))
                );
            }
            let declaration =
                crate::aarch64_normalized_foreign_call_declaration(&record, &row).unwrap();
            let alternative = &declaration.alternatives[0];
            assert_eq!(alternative.key.variant, 3);
            assert_eq!(
                alternative.size,
                selected_instructions::MachineSizeKnowledge::ExactBytes(44)
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
            let template = encode_aarch64_selected_normalized_foreign_call_template(
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
            assert_eq!(template.bytes().len(), 44);
            assert_eq!(template.fixup().opcode_byte_offset, 40);
            assert_eq!(template.fixup().callbacks.len(), 4);
            let mut byte_offset = 0;
            for (position, fixup) in template.fixup().callbacks.iter().enumerate() {
                assert_eq!(fixup.destination, encoded_destinations[position]);
                assert_eq!(fixup.materialization_byte_offset, byte_offset);
                assert_eq!(
                    fixup.materialization_byte_count,
                    if position % 2 == 0 { 8 } else { 12 }
                );
                byte_offset += fixup.materialization_byte_count;
            }
            let replay =
                |bytes: &[u8], fixup, effects: &MachineEncodedEffects, destinations: &[_]| {
                    validate_aarch64_selected_normalized_foreign_call_template(
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
                crate::aarch64_normalized_foreign_call_declaration(&record, &changed_row).is_none()
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
                crate::aarch64_normalized_foreign_call_declaration(&changed_record, &row).is_none()
            );
        }
    }

    #[test]
    fn direct_and_hidden_aggregate_results_share_the_import_template() {
        for darwin in [false, true] {
            let target = if darwin {
                NativeTarget::macos_arm64()
            } else {
                NativeTarget::linux_arm64()
            };
            let mut shared_key = None;
            for result in 0..=5 {
                let (physical, kind, alternative, operands, accesses, effects, record) =
                    inputs(darwin, 11, result);
                let row = crate::machine_effects::normalized_foreign_constraint(&record).unwrap();
                if let Some(key) = shared_key {
                    assert_eq!(row.key, key);
                } else {
                    shared_key = Some(row.key);
                }
                let declaration =
                    crate::aarch64_normalized_foreign_call_declaration(&record, &row).unwrap();
                assert_eq!(declaration.alternatives[0].encoded, effects);
                let template = encode_aarch64_selected_normalized_foreign_call_template(
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
                assert_eq!(template.bytes(), &0x9400_0000_u32.to_le_bytes());
                let mut corrupted = effects.clone();
                corrupted.implicit_unit_clobbers.clear();
                assert_eq!(
                    encode_aarch64_selected_normalized_foreign_call_template(
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
                    Err(Aarch64NormalizedForeignCallTemplateError::EffectMismatch)
                );
                let mut changed_row = row.clone();
                changed_row.operands.last_mut().unwrap().access = RegisterOperandAccess::UseDef;
                assert!(
                    crate::aarch64_normalized_foreign_call_declaration(&record, &changed_row)
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
    fn template_has_exact_bytes_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects, record) = inputs(true, 1, 1);
        let template = encode_aarch64_selected_normalized_foreign_call_template(
            NativeTarget::macos_arm64(),
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
        assert_eq!(template.bytes(), &0x9400_0000_u32.to_le_bytes());
        assert_eq!(template.kind(), kind);
        assert_eq!(template.alternative(), alternative);
        assert_eq!(template.operand_views(), &operands);
        assert_eq!(template.effects(), &effects);
        let fixup = template.fixup().clone();
        assert_eq!(
            fixup,
            callback_fixup(BoundaryMachineId::new(3).unwrap(), 2, &[]).unwrap()
        );
        assert_eq!(fixup.patch_byte_offset, 0);
        assert_eq!(fixup.patch_byte_width, 4);
    }

    #[test]
    fn evaluated_plans_select_exact_operands_and_effects_under_one_template() {
        let (physical, kind, alternative, _, _, _, _) = inputs(false, 0, 0);
        for input_layout in 0..81 {
            for result in 0..3 {
                let (_, _, _, operands, accesses, effects, record) =
                    inputs(false, input_layout, result);
                let template = encode_aarch64_selected_normalized_foreign_call_template(
                    NativeTarget::linux_arm64(),
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
                    encode_aarch64_selected_normalized_foreign_call_template(
                        NativeTarget::linux_arm64(),
                        &physical,
                        kind,
                        alternative,
                        &wrong,
                        &wrong_accesses,
                        &effects,
                        &[],
                        &record,
                    ),
                    Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)
                );
                if !operands.is_empty() {
                    // This view belongs to neither the input nor result bank.
                    let mut substituted = operands.clone();
                    substituted[0] = physical.model().view_named("x8").unwrap().id;
                    assert_eq!(
                        encode_aarch64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_arm64(),
                            &physical,
                            kind,
                            alternative,
                            &substituted,
                            &accesses,
                            &effects,
                            &[],
                            &record,
                        ),
                        Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)
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
                        encode_aarch64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_arm64(),
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
    fn rejects_wrong_kind_target_alternative_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects, record) = inputs(true, 1, 1);
        assert_eq!(
            encode_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
                &physical,
                SelectedInstructionKind::CallScalar {
                    callee: semantic_vocabulary::MachineId::new(4).unwrap(),
                },
                alternative,
                &operands,
                &accesses,
                &effects,
                &[],
                &record,
            ),
            Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch)
        );
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget {
                object_format: target::ObjectFormat::Coff,
                ..NativeTarget::macos_arm64()
            },
        ] {
            assert_eq!(
                encode_aarch64_selected_normalized_foreign_call_template(
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
                Err(Aarch64NormalizedForeignCallTemplateError::UnsupportedTarget)
            );
        }
        assert_eq!(
            encode_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
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
            Err(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)
        );
        let template = encode_aarch64_selected_normalized_foreign_call_template(
            NativeTarget::macos_arm64(),
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
            validate_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
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
            Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        foreign = template.fixup().clone();
        foreign.ordinal = 7;
        assert_eq!(
            validate_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
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
            Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        // The canonical placeholder is the only admitted word.
        assert_eq!(
            validate_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                &0x1400_0000_u32.to_le_bytes(),
                template.fixup().clone(),
                &[],
                &record,
            ),
            Err(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate)
        );
        // A complete effect mismatch rejects.
        let mut wrong_effects = effects.clone();
        wrong_effects.implicit_unit_clobbers.clear();
        assert_eq!(
            encode_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &wrong_effects,
                &[],
                &record,
            ),
            Err(Aarch64NormalizedForeignCallTemplateError::EffectMismatch)
        );
    }
}

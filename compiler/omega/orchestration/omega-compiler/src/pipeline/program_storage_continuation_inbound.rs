//! Encoded inbound realization of the receiver-free program-storage source
//! continuation ABI.
//!
//! This is compile-time evidence only. It joins the independently derived
//! compiler-private `CallPlan` to the exact argument-capture rows already
//! emitted at the retained `Source(StateKey)` function. It does not construct
//! a wrapper body, consume installed authority, emit a call edge, select a new
//! object entry, or claim that native code executed.

use super::{
    ProgramEntrySourceReceiverSignature, ProgramStorageEntryContinuationAbiPlan,
    ProgramStorageEntryContinuationReceiverAbiPlan, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryPlanBinding, ProgramStorageEntryRootRole,
};
use omega_calling_conventions::{
    CallPlan, IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement, ValueShape,
};
use omega_machine_bytes::{CompilerInstructionValidationKind, EncodedMachineFunction};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryContinuationInboundArgument {
    role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    call_parameter_index: usize,
    normalized_type_identity: String,
    shape: ValueShape,
    placement: ValuePlacement,
    destination_byte_offset: usize,
    source_capture_write_range: Range<usize>,
    pointer: IndirectPointerLocation,
    encoded_instruction_index: u32,
    encoded_byte_range: Range<usize>,
}

impl ProgramStorageEntryContinuationInboundArgument {
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

    pub const fn destination_byte_offset(&self) -> usize {
        self.destination_byte_offset
    }

    pub const fn source_capture_write_range(&self) -> &Range<usize> {
        &self.source_capture_write_range
    }

    pub const fn pointer(&self) -> IndirectPointerLocation {
        self.pointer
    }

    pub const fn encoded_instruction_index(&self) -> u32 {
        self.encoded_instruction_index
    }

    pub const fn encoded_byte_range(&self) -> &Range<usize> {
        &self.encoded_byte_range
    }
}

/// Exact encoded inbound capture evidence for the receiver-free source
/// continuation. Final-image validation independently replays the bytes and
/// static-storage relocations named by these retained rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryContinuationInboundPlan {
    target: omega_target::NativeTarget,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    continuation_symbol: String,
    continuation_text_range: Range<usize>,
    normalized_callable_identity: String,
    call: CallPlan,
    arguments: [ProgramStorageEntryContinuationInboundArgument; 2],
}

impl ProgramStorageEntryContinuationInboundPlan {
    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn continuation_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.continuation_identity
    }

    pub fn continuation_symbol(&self) -> &str {
        &self.continuation_symbol
    }

    pub const fn continuation_text_range(&self) -> &Range<usize> {
        &self.continuation_text_range
    }

    pub fn normalized_callable_identity(&self) -> &str {
        &self.normalized_callable_identity
    }

    pub const fn call(&self) -> &CallPlan {
        &self.call
    }

    pub const fn arguments(&self) -> &[ProgramStorageEntryContinuationInboundArgument; 2] {
        &self.arguments
    }
}

struct ArgumentFacts<'a> {
    role: ProgramStorageEntryRootRole,
    source_role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    source_visible_parameter_index: usize,
    call_parameter_index: usize,
    normalized_type_identity: &'a str,
    source_normalized_type_identity: &'a str,
    physical_type_identity: &'a str,
    shape: ValueShape,
    placement: &'a ValuePlacement,
    physical_placement: &'a ValuePlacement,
    destination_byte_offset: usize,
    source_capture_write_range: &'a Range<usize>,
}

pub(super) fn plan_program_storage_entry_continuation_inbound(
    binding: &ProgramStorageEntryPlanBinding,
    abi: &ProgramStorageEntryContinuationAbiPlan,
    continuation: &EncodedMachineFunction,
    encoded_machine: &omega_machine_bytes::EncodedMachinePlan,
) -> Result<Option<ProgramStorageEntryContinuationInboundPlan>, ProgramStorageEntryDiagnostic> {
    let source = binding.source_signature().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "continuation inbound realization has no sealed source declaration".into(),
        )
    })?;
    if !receiver_free_inbound_is_realizable(binding.receiver(), source.receiver(), abi.receiver())?
    {
        return Ok(None);
    }

    let [image_source, storage_source] = source.visible_parameters() else {
        return Err(ProgramStorageEntryDiagnostic(
            "receiver-free continuation inbound realization requires Image then InitialStorage declarations"
                .into(),
        ));
    };
    let [image_abi, storage_abi] = abi.visible_arguments() else {
        return Err(ProgramStorageEntryDiagnostic(
            "receiver-free continuation inbound realization requires two exact ABI rows".into(),
        ));
    };
    plan_from_facts(
        encoded_machine.target,
        abi.target_slot(),
        abi.continuation_identity(),
        abi.normalized_callable_identity(),
        abi.call(),
        [
            ArgumentFacts {
                role: image_abi.role(),
                source_role: image_source.role(),
                visible_parameter_index: image_abi.visible_parameter_index(),
                source_visible_parameter_index: image_source.visible_parameter_index(),
                call_parameter_index: image_abi.call_parameter_index(),
                normalized_type_identity: image_abi.normalized_type_identity(),
                source_normalized_type_identity: image_source.normalized_type_identity(),
                physical_type_identity: binding.image().parameter_type_identity(),
                shape: image_source.value_shape(),
                placement: image_abi.placement(),
                physical_placement: binding.image().placement(),
                destination_byte_offset: binding.image().destination_byte_offset(),
                source_capture_write_range: binding.image().write_range(),
            },
            ArgumentFacts {
                role: storage_abi.role(),
                source_role: storage_source.role(),
                visible_parameter_index: storage_abi.visible_parameter_index(),
                source_visible_parameter_index: storage_source.visible_parameter_index(),
                call_parameter_index: storage_abi.call_parameter_index(),
                normalized_type_identity: storage_abi.normalized_type_identity(),
                source_normalized_type_identity: storage_source.normalized_type_identity(),
                physical_type_identity: binding.initial_storage().parameter_type_identity(),
                shape: storage_source.value_shape(),
                placement: storage_abi.placement(),
                physical_placement: binding.initial_storage().placement(),
                destination_byte_offset: binding.initial_storage().destination_byte_offset(),
                source_capture_write_range: binding.initial_storage().write_range(),
            },
        ],
        continuation,
        encoded_machine,
    )
    .map(Some)
}

fn receiver_free_inbound_is_realizable(
    binding: Option<&super::ProgramEntryReceiverStoragePlan>,
    source: &ProgramEntrySourceReceiverSignature,
    abi: &ProgramStorageEntryContinuationReceiverAbiPlan,
) -> Result<bool, ProgramStorageEntryDiagnostic> {
    match (binding, source, abi) {
        (
            Some(storage),
            ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity,
            },
            ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
                storage: abi_storage,
                ..
            },
        ) if storage.type_identity() == normalized_type_identity && storage == abi_storage => {
            // Attached continuations need a distinct hidden receiver inbound
            // realization and cannot cite this receiver-free evidence.
            Ok(false)
        }
        (
            None,
            ProgramEntrySourceReceiverSignature::Free,
            ProgramStorageEntryContinuationReceiverAbiPlan::Free,
        ) => Ok(true),
        _ => Err(ProgramStorageEntryDiagnostic(
            "continuation inbound receiver facts drifted across binding, declaration, and ABI"
                .into(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_from_facts(
    target: omega_target::NativeTarget,
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    normalized_callable_identity: &str,
    call: &CallPlan,
    arguments: [ArgumentFacts<'_>; 2],
    continuation: &EncodedMachineFunction,
    encoded_machine: &omega_machine_bytes::EncodedMachinePlan,
) -> Result<ProgramStorageEntryContinuationInboundPlan, ProgramStorageEntryDiagnostic> {
    if target != omega_target::NativeTarget::uefi_x64()
        || target_slot.owner != omega_target::TargetProfile::UefiX64
        || target_slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
        || target_slot.visible_parameters
            != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        || target_slot.calling_convention
            != Some(omega_target::ProgramEntryCallingConvention::MicrosoftX64)
        || call.policy != omega_calling_conventions::CallingPolicy::MicrosoftX64
        || call.result.is_some()
        || call.parameters.len() != 2
    {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation inbound realization is restricted to the exact receiver-free UEFI/Microsoft storage ABI"
                .into(),
        ));
    }
    if continuation_identity.source_key().is_none()
        || continuation.identity != continuation_identity
        || continuation.symbol.is_empty()
        || continuation.byte_count == 0
        || normalized_callable_identity.is_empty()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation inbound realization lost its exact nonempty Source function identity"
                .into(),
        ));
    }
    let continuation_end = continuation
        .byte_offset
        .checked_add(continuation.byte_count)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "continuation inbound realization text interval overflows".into(),
            )
        })?;
    let instructions = encoded_machine
        .code
        .instructions
        .span(continuation.instructions)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "continuation inbound realization has an invalid source instruction span".into(),
            )
        })?;
    if !matches!(
        instructions
            .first()
            .and_then(|row| row.compiler_validation_kind.as_ref()),
        Some(&CompilerInstructionValidationKind::FunctionEnter)
    ) {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation inbound captures do not immediately follow an exact FunctionEnter row"
                .into(),
        ));
    }
    let capture_count = instructions
        .iter()
        .filter(|row| {
            matches!(
                row.compiler_validation_kind.as_ref(),
                Some(
                    CompilerInstructionValidationKind::EntryArgumentRegisterWrite { .. }
                        | CompilerInstructionValidationKind::EntryStackArgumentWrite { .. }
                        | CompilerInstructionValidationKind::EntryIndirectArgumentWrite { .. }
                )
            )
        })
        .count();
    if capture_count != 2 {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "receiver-free continuation inbound Source function retains {capture_count} argument captures instead of exactly two"
        )));
    }

    let expected_shape = ValueShape::integer(16, 8);
    let mut realized = Vec::with_capacity(2);
    for (index, facts) in arguments.into_iter().enumerate() {
        let expected_role = if index == 0 {
            ProgramStorageEntryRootRole::Image
        } else {
            ProgramStorageEntryRootRole::InitialStorage
        };
        let expected_pointer = if index == 0 {
            IndirectPointerLocation::Register(MachineRegister::X86Rcx)
        } else {
            IndirectPointerLocation::Register(MachineRegister::X86Rdx)
        };
        if facts.role != expected_role
            || facts.source_role != expected_role
            || facts.visible_parameter_index != index
            || facts.source_visible_parameter_index != index
            || facts.call_parameter_index != index
            || facts.normalized_type_identity.is_empty()
            || facts.normalized_type_identity != facts.source_normalized_type_identity
            || facts.normalized_type_identity != facts.physical_type_identity
            || facts.shape != expected_shape
            || facts.placement != facts.physical_placement
            || facts.placement.shape != facts.shape
            || call.parameters.get(index) != Some(facts.placement)
            || facts.source_capture_write_range != &(index..index + 1)
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "continuation inbound {expected_role:?} declaration, placement, or capture order drifted"
            )));
        }
        let [
            ValueLocation::Indirect {
                pointer, byte_size, ..
            },
        ] = facts.placement.locations.as_slice()
        else {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "continuation inbound {expected_role:?} is not one exact indirect argument"
            )));
        };
        if *pointer != expected_pointer || *byte_size != 16 {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "continuation inbound {expected_role:?} pointer placement drifted"
            )));
        }
        let instruction = instructions.get(index + 1).ok_or_else(|| {
            ProgramStorageEntryDiagnostic(format!(
                "continuation inbound {expected_role:?} capture row is missing"
            ))
        })?;
        let expected_capture = CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
            pointer: expected_pointer,
            byte_offset: facts.destination_byte_offset,
            byte_size: 16,
        };
        if instruction.compiler_validation_kind.as_ref() != Some(&expected_capture) {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "continuation inbound {expected_role:?} encoded capture row drifted"
            )));
        }
        if instruction.bytes.is_empty() || !instruction.bytes.start().is_valid() {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "continuation inbound {expected_role:?} capture has no encoded bytes"
            )));
        }
        let byte_start = instruction.bytes.start().arena_index() as usize - 1;
        let byte_end = byte_start
            .checked_add(instruction.bytes.len())
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(format!(
                    "continuation inbound {expected_role:?} capture interval overflows"
                ))
            })?;
        if byte_start < continuation.byte_offset || byte_end > continuation_end {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "continuation inbound {expected_role:?} capture escapes the exact Source interval"
            )));
        }
        realized.push(ProgramStorageEntryContinuationInboundArgument {
            role: expected_role,
            visible_parameter_index: index,
            call_parameter_index: index,
            normalized_type_identity: facts.normalized_type_identity.to_owned(),
            shape: facts.shape,
            placement: facts.placement.clone(),
            destination_byte_offset: facts.destination_byte_offset,
            source_capture_write_range: facts.source_capture_write_range.clone(),
            pointer: expected_pointer,
            encoded_instruction_index: instruction.selected_instruction_index,
            encoded_byte_range: byte_start..byte_end,
        });
    }

    Ok(ProgramStorageEntryContinuationInboundPlan {
        target,
        continuation_identity,
        continuation_symbol: continuation.symbol.to_string(),
        continuation_text_range: continuation.byte_offset..continuation_end,
        normalized_callable_identity: normalized_callable_identity.to_owned(),
        call: call.clone(),
        arguments: realized.try_into().map_err(|_| {
            ProgramStorageEntryDiagnostic(
                "continuation inbound realization lost its exact two capture rows".into(),
            )
        })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy};
    use omega_control_flow::StateKey;
    use omega_machine_bytes::{EncodedMachineInstruction, EncodedMachinePlan};
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    struct Fixture {
        encoded: EncodedMachinePlan,
        identity: omega_control_flow::MachineFunctionIdentity,
        call: CallPlan,
        physical: [ValuePlacement; 2],
        destinations: [usize; 2],
        ranges: [Range<usize>; 2],
    }

    fn fixture() -> Fixture {
        let shape = ValueShape::integer(16, 8);
        let call = omega_calling_conventions::evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![shape, shape],
                result: None,
            },
        )
        .expect("Microsoft x64 Extent call plan");
        let identity = omega_control_flow::MachineFunctionIdentity::source(StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        });
        let mut encoded =
            EncodedMachinePlan::with_capacity(omega_target::NativeTarget::uefi_x64(), 1, 4, 4);
        let byte_spans: [psi_arena::HandleSpan<u8>; 4] = std::array::from_fn(|index| {
            encoded
                .code
                .bytes
                .insert_many([u8::try_from(index + 1).expect("test byte")])
        });
        let instructions = encoded.code.instructions.insert_many([
            EncodedMachineInstruction {
                selected_instruction_index: 0,
                bytes: byte_spans[0],
                compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionEnter),
                ..Default::default()
            },
            EncodedMachineInstruction {
                selected_instruction_index: 1,
                bytes: byte_spans[1],
                compiler_validation_kind: Some(
                    CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
                        pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                        byte_offset: 16,
                        byte_size: 16,
                    },
                ),
                ..Default::default()
            },
            EncodedMachineInstruction {
                selected_instruction_index: 2,
                bytes: byte_spans[2],
                compiler_validation_kind: Some(
                    CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
                        pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                        byte_offset: 32,
                        byte_size: 16,
                    },
                ),
                ..Default::default()
            },
            EncodedMachineInstruction {
                selected_instruction_index: 3,
                bytes: byte_spans[3],
                compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionReturn),
                ..Default::default()
            },
        ]);
        encoded.code.functions.insert(EncodedMachineFunction {
            symbol: Arc::from("__omega_source_continuation"),
            identity,
            byte_offset: 0,
            byte_count: 4,
            instructions,
        });
        encoded.code.byte_count = 4;
        Fixture {
            encoded,
            identity,
            physical: [call.parameters[0].clone(), call.parameters[1].clone()],
            call,
            destinations: [16, 32],
            ranges: [0..1, 1..2],
        }
    }

    fn realize(
        fixture: &Fixture,
    ) -> Result<ProgramStorageEntryContinuationInboundPlan, ProgramStorageEntryDiagnostic> {
        let continuation = fixture
            .encoded
            .code
            .functions
            .iter()
            .find(|(_, function)| function.identity == fixture.identity)
            .map(|(_, function)| function)
            .or_else(|| {
                fixture
                    .encoded
                    .code
                    .functions
                    .iter()
                    .next()
                    .map(|(_, function)| function)
            })
            .expect("encoded function");
        plan_from_facts(
            fixture.encoded.target,
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            fixture.identity,
            "normalized-callable",
            &fixture.call,
            [
                ArgumentFacts {
                    role: ProgramStorageEntryRootRole::Image,
                    source_role: ProgramStorageEntryRootRole::Image,
                    visible_parameter_index: 0,
                    source_visible_parameter_index: 0,
                    call_parameter_index: 0,
                    normalized_type_identity: "Extent in Granted",
                    source_normalized_type_identity: "Extent in Granted",
                    physical_type_identity: "Extent in Granted",
                    shape: ValueShape::integer(16, 8),
                    placement: &fixture.call.parameters[0],
                    physical_placement: &fixture.physical[0],
                    destination_byte_offset: fixture.destinations[0],
                    source_capture_write_range: &fixture.ranges[0],
                },
                ArgumentFacts {
                    role: ProgramStorageEntryRootRole::InitialStorage,
                    source_role: ProgramStorageEntryRootRole::InitialStorage,
                    visible_parameter_index: 1,
                    source_visible_parameter_index: 1,
                    call_parameter_index: 1,
                    normalized_type_identity: "Extent in Granted",
                    source_normalized_type_identity: "Extent in Granted",
                    physical_type_identity: "Extent in Granted",
                    shape: ValueShape::integer(16, 8),
                    placement: &fixture.call.parameters[1],
                    physical_placement: &fixture.physical[1],
                    destination_byte_offset: fixture.destinations[1],
                    source_capture_write_range: &fixture.ranges[1],
                },
            ],
            continuation,
            &fixture.encoded,
        )
    }

    #[test]
    fn exact_receiver_free_source_captures_realize_the_internal_abi() {
        let plan = realize(&fixture()).expect("exact source inbound realization");
        assert_eq!(plan.continuation_symbol(), "__omega_source_continuation");
        assert_eq!(plan.continuation_text_range(), &(0..4));
        assert_eq!(plan.call().result, None);
        let [image, storage] = plan.arguments();
        assert_eq!(image.role(), ProgramStorageEntryRootRole::Image);
        assert_eq!(
            image.pointer(),
            IndirectPointerLocation::Register(MachineRegister::X86Rcx)
        );
        assert_eq!(image.destination_byte_offset(), 16);
        assert_eq!(image.encoded_instruction_index(), 1);
        assert_eq!(image.encoded_byte_range(), &(1..2));
        assert_eq!(storage.role(), ProgramStorageEntryRootRole::InitialStorage);
        assert_eq!(
            storage.pointer(),
            IndirectPointerLocation::Register(MachineRegister::X86Rdx)
        );
        assert_eq!(storage.destination_byte_offset(), 32);
    }

    #[test]
    fn attached_receiver_is_consistent_but_claims_no_receiver_free_realization() {
        let storage = super::super::ProgramEntryReceiverStoragePlan::for_test("Boot", 8, 8);
        let pointer_shape = ValueShape::integer(8, 8);
        let receiver_call = omega_calling_conventions::evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![pointer_shape],
                result: None,
            },
        )
        .expect("receiver pointer call plan");
        let source = ProgramEntrySourceReceiverSignature::ProvisionedMutable {
            normalized_type_identity: "Boot".into(),
        };
        let abi = ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
            parameter_index: 0,
            storage: storage.clone(),
            pointer_shape,
            placement: receiver_call.parameters[0].clone(),
        };
        assert!(
            !receiver_free_inbound_is_realizable(Some(&storage), &source, &abi)
                .expect("consistent attached facts are not receiver-free")
        );
        assert!(
            receiver_free_inbound_is_realizable(
                None,
                &ProgramEntrySourceReceiverSignature::Free,
                &ProgramStorageEntryContinuationReceiverAbiPlan::Free,
            )
            .expect("exact free facts")
        );
        assert!(receiver_free_inbound_is_realizable(None, &source, &abi).is_err());
    }

    #[test]
    fn placement_destination_order_and_capture_tampering_fail_closed() {
        let mut drifted = fixture();
        drifted.physical.swap(0, 1);
        assert!(realize(&drifted).unwrap_err().0.contains("placement"));

        let mut drifted = fixture();
        drifted.ranges.swap(0, 1);
        assert!(realize(&drifted).unwrap_err().0.contains("capture order"));

        let mut drifted = fixture();
        drifted.destinations[0] = 24;
        assert!(realize(&drifted).unwrap_err().0.contains("encoded capture"));

        let mut drifted = fixture();
        let function_instructions = drifted
            .encoded
            .code
            .functions
            .iter()
            .next()
            .unwrap()
            .1
            .instructions;
        let rows = drifted
            .encoded
            .code
            .instructions
            .span(function_instructions)
            .unwrap();
        let mut changed = rows[1].clone();
        changed.compiler_validation_kind = Some(
            CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                byte_offset: 16,
                byte_size: 16,
            },
        );
        let handle = drifted.encoded.code.instructions.iter().nth(1).unwrap().0;
        *drifted.encoded.code.instructions.get_mut(handle) = changed;
        assert!(realize(&drifted).unwrap_err().0.contains("encoded capture"));

        let mut missing = fixture();
        let second_capture = missing.encoded.code.instructions.iter().nth(2).unwrap().0;
        missing
            .encoded
            .code
            .instructions
            .get_mut(second_capture)
            .compiler_validation_kind = Some(CompilerInstructionValidationKind::FunctionReturn);
        assert!(realize(&missing).unwrap_err().0.contains("exactly two"));

        let mut duplicate = fixture();
        let return_row = duplicate.encoded.code.instructions.iter().nth(3).unwrap().0;
        duplicate
            .encoded
            .code
            .instructions
            .get_mut(return_row)
            .compiler_validation_kind = Some(
            CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                byte_offset: 32,
                byte_size: 16,
            },
        );
        assert!(realize(&duplicate).unwrap_err().0.contains("exactly two"));
    }

    #[test]
    fn wrapper_identity_and_source_interval_drift_cannot_supply_inbound_evidence() {
        let mut separated = fixture();
        let wrapper_bytes = separated.encoded.code.bytes.insert_many([0x90]);
        let wrapper_instructions =
            separated
                .encoded
                .code
                .instructions
                .insert_many([EncodedMachineInstruction {
                    selected_instruction_index: 4,
                    bytes: wrapper_bytes,
                    compiler_validation_kind: Some(
                        CompilerInstructionValidationKind::FunctionEnter,
                    ),
                    ..Default::default()
                }]);
        separated.encoded.code.byte_count = 5;
        separated
            .encoded
            .code
            .functions
            .insert(EncodedMachineFunction {
                symbol: Arc::from("_start"),
                identity:
                    omega_control_flow::MachineFunctionIdentity::program_storage_entry_wrapper(
                        separated.identity.source_key().unwrap(),
                    )
                    .unwrap(),
                byte_offset: 4,
                byte_count: 1,
                instructions: wrapper_instructions,
            });
        let exact_source = realize(&separated).expect("wrapper must not relabel source inbound");
        assert_eq!(exact_source.continuation_text_range(), &(0..4));

        let mut drifted = fixture();
        let function_handle = drifted.encoded.code.functions.iter().next().unwrap().0;
        drifted
            .encoded
            .code
            .functions
            .get_mut(function_handle)
            .identity = omega_control_flow::MachineFunctionIdentity::program_storage_entry_wrapper(
            fixture().identity.source_key().unwrap(),
        )
        .unwrap();
        assert!(
            realize(&drifted)
                .unwrap_err()
                .0
                .contains("Source function identity")
        );

        let mut drifted = fixture();
        let function_handle = drifted.encoded.code.functions.iter().next().unwrap().0;
        drifted
            .encoded
            .code
            .functions
            .get_mut(function_handle)
            .byte_offset = 2;
        drifted
            .encoded
            .code
            .functions
            .get_mut(function_handle)
            .byte_count = 2;
        assert!(
            realize(&drifted)
                .unwrap_err()
                .0
                .contains("escapes the exact Source interval")
        );
    }
}

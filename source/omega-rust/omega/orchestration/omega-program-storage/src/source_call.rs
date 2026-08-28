//! Address-free compiler-private ABI layout for a future generated
//! program-storage wrapper's call to its selected source continuation.
//!
//! The physical arrival `BoundaryEntryPlan` is intentionally absent. Parameter
//! shapes come from the sealed typed source declaration, and placements come
//! from Omega's compiler-private internal calling policy. This module owns no
//! runtime root value, `Extent`, authority, wrapper body, emitted call, or
//! callee inbound realization.
//!
//! The current flat shape carrier is deliberately fenced to the only admitted
//! program-storage source schema: UEFI x86-64 under MicrosoftX64. A future
//! SysV/AAPCS schema must retain structural classification before this planner
//! may admit it.

use super::program_storage_entry::{
    ProgramEntryReceiverPlacementRecord, ProgramEntryReceiverStoragePlan,
    ProgramStorageEntryDiagnostic,
};
use super::program_storage_wrapper::{
    ProgramStorageEntryWrapperReceiverTransfer, ProgramStorageEntryWrapperTransferPlan,
};
use crate::ProgramStorageEntryRootRole;
use crate::{ProgramEntrySourceReceiverSignature, SelectedProgramEntrySourceSignature};
use omega_calling_conventions::{CallPlan, CallSignature, ValuePlacement, ValueShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramStorageEntryContinuationReceiverAbiPlan {
    /// Complete no-receiver ABI form. The current production executor still
    /// cannot traverse it because that gate requires receiver activation.
    Free,
    BorrowedActivationLoan {
        parameter_index: usize,
        storage: ProgramEntryReceiverStoragePlan,
        pointer_shape: ValueShape,
        placement: ValuePlacement,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryContinuationVisibleArgumentPlan {
    role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    call_parameter_index: usize,
    normalized_type_identity: String,
    shape: ValueShape,
    placement: ValuePlacement,
}

impl ProgramStorageEntryContinuationVisibleArgumentPlan {
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
}

/// Complete address-free outbound layout for the exact selected declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryContinuationAbiPlan {
    target: omega_target::NativeTarget,
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    normalized_callable_identity: String,
    call: CallPlan,
    receiver: ProgramStorageEntryContinuationReceiverAbiPlan,
    visible_arguments: Vec<ProgramStorageEntryContinuationVisibleArgumentPlan>,
}

impl ProgramStorageEntryContinuationAbiPlan {
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

    pub const fn call(&self) -> &CallPlan {
        &self.call
    }

    pub const fn receiver(&self) -> &ProgramStorageEntryContinuationReceiverAbiPlan {
        &self.receiver
    }

    pub fn visible_arguments(&self) -> &[ProgramStorageEntryContinuationVisibleArgumentPlan] {
        &self.visible_arguments
    }

    pub(super) fn bind_activation_loan(
        &self,
        placement: &ProgramEntryReceiverPlacementRecord,
        loan_byte_count: usize,
    ) -> Result<ProgramStorageEntryContinuationReceiverBinding, ProgramStorageEntryDiagnostic> {
        let ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
            parameter_index,
            storage,
            pointer_shape,
            placement: abi_placement,
        } = &self.receiver
        else {
            return Err(ProgramStorageEntryDiagnostic(
                "free program-storage entries have no production receiver-activation executor traversal"
                    .into(),
            ));
        };
        let length = usize::try_from(placement.length()).map_err(|_| {
            ProgramStorageEntryDiagnostic(
                "mapped receiver length does not fit the continuation ABI address model".into(),
            )
        })?;
        let alignment = usize::try_from(placement.alignment()).map_err(|_| {
            ProgramStorageEntryDiagnostic(
                "mapped receiver alignment does not fit the continuation ABI address model".into(),
            )
        })?;
        if placement.alignment() == 0
            || placement.type_identity() != storage.type_identity()
            || length != storage.byte_size()
            || alignment != storage.byte_alignment()
            || loan_byte_count != length
            || placement.base() % placement.alignment() != 0
            || *parameter_index != 0
            || self.call.parameters.get(*parameter_index) != Some(abi_placement)
            || abi_placement.shape != *pointer_shape
        {
            return Err(ProgramStorageEntryDiagnostic(
                "mapped receiver activation loan does not match its exact outbound continuation ABI placement"
                    .into(),
            ));
        }
        Ok(ProgramStorageEntryContinuationReceiverBinding {
            parameter_index: *parameter_index,
            mapped_address: placement.base(),
            placement: abi_placement.clone(),
        })
    }
}

/// Runtime receiver address bound to the live loan held by the enclosing
/// continuation handoff. It carries no root argument or detached authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryContinuationReceiverBinding {
    parameter_index: usize,
    mapped_address: u64,
    placement: ValuePlacement,
}

impl ProgramStorageEntryContinuationReceiverBinding {
    pub const fn parameter_index(&self) -> usize {
        self.parameter_index
    }

    pub const fn mapped_address(&self) -> u64 {
        self.mapped_address
    }

    pub const fn placement(&self) -> &ValuePlacement {
        &self.placement
    }
}

pub(super) fn plan_program_storage_entry_continuation_abi(
    target: omega_target::NativeTarget,
    transfer: &ProgramStorageEntryWrapperTransferPlan,
    source_signature: &SelectedProgramEntrySourceSignature,
) -> Result<ProgramStorageEntryContinuationAbiPlan, ProgramStorageEntryDiagnostic> {
    let continuation_identity = transfer.continuation_identity();
    let Some(continuation_key) = continuation_identity.source_key() else {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation ABI plan has no exact source continuation identity".into(),
        ));
    };
    if continuation_key.machine != source_signature.machine_symbol()
        || continuation_key.state != source_signature.state_symbol()
        || continuation_key.segment_index != 0
    {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation ABI identity drifted from the sealed source declaration".into(),
        ));
    }
    let receiver_storage = match (transfer.receiver(), source_signature.receiver()) {
        (
            ProgramStorageEntryWrapperReceiverTransfer::Free,
            ProgramEntrySourceReceiverSignature::Free,
        ) => None,
        (
            ProgramStorageEntryWrapperReceiverTransfer::BorrowedActivationLoan(storage),
            ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity,
            },
        ) if storage.type_identity() == normalized_type_identity => Some(storage.clone()),
        _ => {
            return Err(ProgramStorageEntryDiagnostic(
                "continuation ABI receiver drifted from the sealed source declaration".into(),
            ));
        }
    };
    let visible = source_signature
        .visible_parameters()
        .iter()
        .map(|parameter| VisibleFacts {
            role: parameter.role(),
            visible_parameter_index: parameter.visible_parameter_index(),
            normalized_type_identity: parameter.normalized_type_identity().to_owned(),
            shape: parameter.value_shape(),
        })
        .collect();
    plan_from_facts(
        target,
        source_signature.target_slot(),
        continuation_identity,
        source_signature.normalized_callable_identity().to_owned(),
        receiver_storage,
        visible,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleFacts {
    role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    normalized_type_identity: String,
    shape: ValueShape,
}

fn plan_from_facts(
    target: omega_target::NativeTarget,
    target_slot: omega_target::ProgramEntrySlotDeclaration,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    normalized_callable_identity: String,
    receiver_storage: Option<ProgramEntryReceiverStoragePlan>,
    visible: Vec<VisibleFacts>,
) -> Result<ProgramStorageEntryContinuationAbiPlan, ProgramStorageEntryDiagnostic> {
    if target_slot.owner != omega_target::TargetProfile::UefiX64
        || target_slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
        || target_slot.visible_parameters
            != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        || target_slot.semantic_calling_convention
            != Some(omega_target::ProgramEntryCallingConvention::MicrosoftX64)
        || omega_calling_conventions::CallingPolicy::native_for_target(target)
            != omega_calling_conventions::CallingPolicy::MicrosoftX64
    {
        return Err(ProgramStorageEntryDiagnostic(
            "outbound continuation ABI is restricted to the exact UEFI/Microsoft program-storage schema"
                .into(),
        ));
    }
    if !continuation_identity.is_valid()
        || continuation_identity.source_key().is_none()
        || normalized_callable_identity.is_empty()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation ABI plan lost its exact source declaration identity".into(),
        ));
    }
    if !matches!(
        visible.as_slice(),
        [
            VisibleFacts {
                role: ProgramStorageEntryRootRole::Image,
                visible_parameter_index: 0,
                ..
            },
            VisibleFacts {
                role: ProgramStorageEntryRootRole::InitialStorage,
                visible_parameter_index: 1,
                ..
            }
        ]
    ) {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation ABI requires exact receiver-excluded Image then InitialStorage declarations"
                .into(),
        ));
    }
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "continuation ABI target pointer size exceeds the normalized model".into(),
        )
    })?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "continuation ABI target pointer alignment exceeds the normalized model".into(),
        )
    })?;
    if pointer_size == 0 || pointer_alignment == 0 || !pointer_alignment.is_power_of_two() {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation ABI target has an invalid pointer shape".into(),
        ));
    }
    let receiver_shape = receiver_storage
        .as_ref()
        .map(|_| ValueShape::integer(pointer_size, pointer_alignment));
    let signature = CallSignature {
        parameters: receiver_shape
            .into_iter()
            .chain(visible.iter().map(|parameter| parameter.shape))
            .collect(),
        result: None,
    };
    let call = omega_calling_conventions::evaluate_call_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &signature,
    )
    .map_err(|diagnostic| {
        ProgramStorageEntryDiagnostic(format!(
            "cannot derive compiler-private outbound continuation ABI: {diagnostic}"
        ))
    })?;
    let receiver_count = usize::from(receiver_storage.is_some());
    let receiver = match (receiver_storage, receiver_shape) {
        (Some(storage), Some(pointer_shape)) => {
            ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
                parameter_index: 0,
                storage,
                pointer_shape,
                placement: call.parameters[0].clone(),
            }
        }
        (None, None) => ProgramStorageEntryContinuationReceiverAbiPlan::Free,
        _ => unreachable!("receiver storage and pointer shape are constructed together"),
    };
    let visible_arguments = visible
        .into_iter()
        .map(|parameter| {
            let call_parameter_index = receiver_count + parameter.visible_parameter_index;
            ProgramStorageEntryContinuationVisibleArgumentPlan {
                role: parameter.role,
                visible_parameter_index: parameter.visible_parameter_index,
                call_parameter_index,
                normalized_type_identity: parameter.normalized_type_identity,
                shape: parameter.shape,
                placement: call.parameters[call_parameter_index].clone(),
            }
        })
        .collect();
    let plan = ProgramStorageEntryContinuationAbiPlan {
        target,
        target_slot,
        continuation_identity,
        normalized_callable_identity,
        call,
        receiver,
        visible_arguments,
    };
    validate_continuation_abi(&plan)?;
    Ok(plan)
}

fn validate_continuation_abi(
    plan: &ProgramStorageEntryContinuationAbiPlan,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let receiver_count = usize::from(matches!(
        &plan.receiver,
        ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan { .. }
    ));
    if plan.call.policy != omega_calling_conventions::CallingPolicy::MicrosoftX64
        || plan.call.result.is_some()
        || plan.call.parameters.len() != receiver_count + plan.visible_arguments.len()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "outbound continuation ABI drifted from its exact Unit declaration layout".into(),
        ));
    }
    if let ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
        parameter_index,
        pointer_shape,
        placement,
        ..
    } = &plan.receiver
        && (*parameter_index != 0
            || placement.shape != *pointer_shape
            || plan.call.parameters.first() != Some(placement))
    {
        return Err(ProgramStorageEntryDiagnostic(
            "outbound continuation receiver placement drifted from parameter zero".into(),
        ));
    }
    if plan
        .visible_arguments
        .iter()
        .enumerate()
        .any(|(index, argument)| {
            argument.visible_parameter_index != index
                || argument.call_parameter_index != receiver_count + index
                || argument.normalized_type_identity.is_empty()
                || argument.placement.shape != argument.shape
                || plan.call.parameters.get(argument.call_parameter_index)
                    != Some(&argument.placement)
        })
        || !matches!(
            plan.visible_arguments.as_slice(),
            [
                ProgramStorageEntryContinuationVisibleArgumentPlan {
                    role: ProgramStorageEntryRootRole::Image,
                    ..
                },
                ProgramStorageEntryContinuationVisibleArgumentPlan {
                    role: ProgramStorageEntryRootRole::InitialStorage,
                    ..
                }
            ]
        )
    {
        return Err(ProgramStorageEntryDiagnostic(
            "outbound continuation visible placements drifted from their sealed declarations"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{MachineRegister, ValueLocation};
    use psi_symbols::SymbolHandle;

    fn continuation() -> omega_control_flow::MachineFunctionIdentity {
        omega_control_flow::MachineFunctionIdentity::source(omega_control_flow::StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        })
    }

    fn receiver() -> ProgramEntryReceiverStoragePlan {
        ProgramEntryReceiverStoragePlan::for_test("Boot", 8, 8)
    }

    fn visible() -> Vec<VisibleFacts> {
        vec![
            VisibleFacts {
                role: ProgramStorageEntryRootRole::Image,
                visible_parameter_index: 0,
                normalized_type_identity: "Extent in Granted".into(),
                shape: ValueShape::integer(16, 8),
            },
            VisibleFacts {
                role: ProgramStorageEntryRootRole::InitialStorage,
                visible_parameter_index: 1,
                normalized_type_identity: "Extent in Granted".into(),
                shape: ValueShape::integer(16, 8),
            },
        ]
    }

    fn abi_plan(
        receiver: Option<ProgramEntryReceiverStoragePlan>,
    ) -> ProgramStorageEntryContinuationAbiPlan {
        plan_from_facts(
            omega_target::NativeTarget::uefi_x64(),
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            continuation(),
            "normalized-callable".into(),
            receiver,
            visible(),
        )
        .expect("exact UEFI continuation ABI")
    }

    #[test]
    fn free_and_attached_forms_place_complete_declaration_shapes() {
        let free = abi_plan(None);
        assert!(matches!(
            free.receiver(),
            ProgramStorageEntryContinuationReceiverAbiPlan::Free
        ));
        assert_eq!(free.call().parameters.len(), 2);
        assert_eq!(free.visible_arguments()[0].call_parameter_index(), 0);

        let attached = abi_plan(Some(receiver()));
        let ProgramStorageEntryContinuationReceiverAbiPlan::BorrowedActivationLoan {
            parameter_index,
            placement,
            ..
        } = attached.receiver()
        else {
            panic!("attached ABI must retain its receiver")
        };
        assert_eq!(*parameter_index, 0);
        assert!(placement.locations.iter().any(|location| matches!(
            location,
            ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                ..
            }
        )));
        assert_eq!(attached.call().parameters.len(), 3);
        assert_eq!(attached.visible_arguments()[0].call_parameter_index(), 1);
        assert_eq!(attached.visible_arguments()[1].call_parameter_index(), 2);
        assert!(
            attached
                .visible_arguments()
                .iter()
                .all(|argument| argument.shape() == ValueShape::integer(16, 8))
        );
    }

    #[test]
    fn identity_schema_role_shape_and_placement_drift_fail_closed() {
        let error = plan_from_facts(
            omega_target::NativeTarget::linux_x64(),
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            continuation(),
            "normalized-callable".into(),
            None,
            visible(),
        )
        .expect_err("non-Microsoft target policy must reject");
        assert!(error.0.contains("UEFI/Microsoft"), "{error}");

        let mut roles = visible();
        roles.swap(0, 1);
        roles[0].visible_parameter_index = 0;
        roles[1].visible_parameter_index = 1;
        let error = plan_from_facts(
            omega_target::NativeTarget::uefi_x64(),
            omega_target::TargetProfile::UefiX64.program_entry_slot(),
            continuation(),
            "normalized-callable".into(),
            None,
            roles,
        )
        .expect_err("role redirection must reject");
        assert!(error.0.contains("Image then InitialStorage"), "{error}");

        let mut plan = abi_plan(Some(receiver()));
        plan.visible_arguments[0].placement = plan.call.parameters[2].clone();
        let error =
            validate_continuation_abi(&plan).expect_err("placement redirection must reject");
        assert!(error.0.contains("visible placements"), "{error}");

        let mut plan = abi_plan(Some(receiver()));
        plan.call.parameters[1].shape = ValueShape::integer(8, 8);
        let error = validate_continuation_abi(&plan).expect_err("shape drift must reject");
        assert!(error.0.contains("visible placements"), "{error}");
    }

    #[test]
    fn mapped_receiver_binding_checks_exact_abi_placement_and_live_loan() {
        let plan = abi_plan(Some(receiver()));
        let exact = ProgramEntryReceiverPlacementRecord::for_test("Boot", 0x8008, 8, 8);
        let binding = plan
            .bind_activation_loan(&exact, 8)
            .expect("exact mapped receiver loan");
        assert_eq!(binding.parameter_index(), 0);
        assert_eq!(binding.mapped_address(), 0x8008);
        assert_eq!(binding.placement(), &plan.call().parameters[0]);

        for placement in [
            ProgramEntryReceiverPlacementRecord::for_test("Other", 0x8008, 8, 8),
            ProgramEntryReceiverPlacementRecord::for_test("Boot", 0x8008, 8, 0),
            ProgramEntryReceiverPlacementRecord::for_test("Boot", 0x8003, 8, 8),
        ] {
            let error = plan
                .bind_activation_loan(&placement, 8)
                .expect_err("receiver drift must reject");
            assert!(error.0.contains("exact outbound"), "{error}");
        }
        plan.bind_activation_loan(&exact, 7)
            .expect_err("short live loan must reject");
    }
}

//! Non-authoritative staging facts for a future generated program-storage
//! wrapper's source-continuation call.
//!
//! The physical arrival `BoundaryEntryPlan` is intentionally absent. Current
//! production carriers do not independently retain the selected source
//! signature or an authority-bearing value for each visible root. In
//! particular, an attached receiver partitions `InitialStorage`, and the
//! conserved before/after pieces need not form one contiguous `Extent`.
//! Therefore this module does not construct a source `CallPlan`, assign root
//! arguments, or claim that the source continuation can be called.
//!
//! For an attached entry only, it stages the first compiler-private internal
//! ABI position as a receiver-pointer candidate. The production executor gate
//! can bind that candidate to its exact mapped, live activation loan. A free
//! entry is represented for completeness, but the current production executor
//! cannot reach it because its installation path requires receiver activation.

use super::program_storage_entry::{
    ProgramEntryReceiverPlacementRecord, ProgramEntryReceiverStoragePlan,
    ProgramStorageEntryDiagnostic,
};
use super::program_storage_wrapper::{
    ProgramStorageEntryWrapperReceiverTransfer, ProgramStorageEntryWrapperTransferPlan,
};
use omega_calling_conventions::{CallSignature, ValuePlacement, ValueShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramStorageEntryContinuationReceiverStagingPlan {
    /// The source entry has no receiver, but no production executor traversal
    /// currently exists for this form.
    FreeWithoutProductionExecutor,
    /// Candidate first internal-ABI position for the attached receiver. This
    /// is not a published boundary placement or a complete source-call plan.
    BorrowedActivationLoan {
        candidate_parameter_index: usize,
        storage: ProgramEntryReceiverStoragePlan,
        pointer_shape: ValueShape,
        candidate_placement: ValuePlacement,
    },
}

/// Shape/placement staging only; it carries no source signature, root values,
/// root authority, callee realization, or emitted call evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryContinuationStagingPlan {
    target: omega_target::NativeTarget,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    receiver: ProgramStorageEntryContinuationReceiverStagingPlan,
}

impl ProgramStorageEntryContinuationStagingPlan {
    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn continuation_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.continuation_identity
    }

    pub const fn receiver(&self) -> &ProgramStorageEntryContinuationReceiverStagingPlan {
        &self.receiver
    }

    pub(super) fn bind_activation_loan(
        &self,
        placement: &ProgramEntryReceiverPlacementRecord,
        loan_byte_count: usize,
    ) -> Result<ProgramStorageEntryContinuationReceiverBinding, ProgramStorageEntryDiagnostic> {
        let ProgramStorageEntryContinuationReceiverStagingPlan::BorrowedActivationLoan {
            candidate_parameter_index,
            storage,
            pointer_shape,
            candidate_placement,
        } = &self.receiver
        else {
            return Err(ProgramStorageEntryDiagnostic(
                "free program-storage entries have no production receiver-activation executor traversal"
                    .into(),
            ));
        };
        let length = usize::try_from(placement.length()).map_err(|_| {
            ProgramStorageEntryDiagnostic(
                "mapped receiver length does not fit the continuation staging address model".into(),
            )
        })?;
        let alignment = usize::try_from(placement.alignment()).map_err(|_| {
            ProgramStorageEntryDiagnostic(
                "mapped receiver alignment does not fit the continuation staging address model"
                    .into(),
            )
        })?;
        if placement.alignment() == 0
            || placement.type_identity() != storage.type_identity()
            || length != storage.byte_size()
            || alignment != storage.byte_alignment()
            || loan_byte_count != length
            || placement.base() % placement.alignment() != 0
            || *candidate_parameter_index != 0
            || candidate_placement.shape != *pointer_shape
        {
            return Err(ProgramStorageEntryDiagnostic(
                "mapped receiver activation loan does not match its exact continuation staging candidate"
                    .into(),
            ));
        }
        Ok(ProgramStorageEntryContinuationReceiverBinding {
            candidate_parameter_index: *candidate_parameter_index,
            mapped_address: placement.base(),
            candidate_placement: candidate_placement.clone(),
        })
    }
}

/// Runtime receiver address bound to the live loan held by the enclosing
/// continuation handoff. The placement remains a non-authoritative candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryContinuationReceiverBinding {
    candidate_parameter_index: usize,
    mapped_address: u64,
    candidate_placement: ValuePlacement,
}

impl ProgramStorageEntryContinuationReceiverBinding {
    pub const fn candidate_parameter_index(&self) -> usize {
        self.candidate_parameter_index
    }

    pub const fn mapped_address(&self) -> u64 {
        self.mapped_address
    }

    pub const fn candidate_placement(&self) -> &ValuePlacement {
        &self.candidate_placement
    }
}

pub(super) fn plan_program_storage_entry_continuation_staging(
    target: omega_target::NativeTarget,
    transfer: &ProgramStorageEntryWrapperTransferPlan,
) -> Result<ProgramStorageEntryContinuationStagingPlan, ProgramStorageEntryDiagnostic> {
    let receiver = match transfer.receiver() {
        ProgramStorageEntryWrapperReceiverTransfer::Free => None,
        ProgramStorageEntryWrapperReceiverTransfer::BorrowedActivationLoan(storage) => {
            Some(storage.clone())
        }
    };
    plan_from_facts(target, transfer.continuation_identity(), receiver)
}

fn plan_from_facts(
    target: omega_target::NativeTarget,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    receiver_storage: Option<ProgramEntryReceiverStoragePlan>,
) -> Result<ProgramStorageEntryContinuationStagingPlan, ProgramStorageEntryDiagnostic> {
    if !continuation_identity.is_valid() || continuation_identity.source_key().is_none() {
        return Err(ProgramStorageEntryDiagnostic(
            "continuation staging plan has no exact source continuation identity".into(),
        ));
    }
    let receiver = match receiver_storage {
        None => ProgramStorageEntryContinuationReceiverStagingPlan::FreeWithoutProductionExecutor,
        Some(storage) => {
            let pointer_size = u16::try_from(target.pointer_size).map_err(|_| {
                ProgramStorageEntryDiagnostic(
                    "continuation staging target pointer size exceeds the normalized ABI model"
                        .into(),
                )
            })?;
            let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| {
                ProgramStorageEntryDiagnostic(
                    "continuation staging target pointer alignment exceeds the normalized ABI model"
                        .into(),
                )
            })?;
            if pointer_size == 0 || pointer_alignment == 0 || !pointer_alignment.is_power_of_two() {
                return Err(ProgramStorageEntryDiagnostic(
                    "continuation staging target has an invalid pointer shape".into(),
                ));
            }
            let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
            let candidate = omega_calling_conventions::evaluate_call_plan(
                omega_calling_conventions::CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![pointer_shape],
                    result: None,
                },
            )
            .map_err(|diagnostic| {
                ProgramStorageEntryDiagnostic(format!(
                    "cannot stage compiler-private receiver placement candidate: {diagnostic}",
                ))
            })?;
            let Some(candidate_placement) = candidate.parameters.into_iter().next() else {
                return Err(ProgramStorageEntryDiagnostic(
                    "continuation receiver staging produced no candidate placement".into(),
                ));
            };
            ProgramStorageEntryContinuationReceiverStagingPlan::BorrowedActivationLoan {
                candidate_parameter_index: 0,
                storage,
                pointer_shape,
                candidate_placement,
            }
        }
    };
    Ok(ProgramStorageEntryContinuationStagingPlan {
        target,
        continuation_identity,
        receiver,
    })
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
        ProgramEntryReceiverStoragePlan::for_test("&mut Boot", 8, 8)
    }

    #[test]
    fn attached_receiver_stages_only_a_non_authoritative_first_position() {
        let plan = plan_from_facts(
            omega_target::NativeTarget::windows_x64(),
            continuation(),
            Some(receiver()),
        )
        .expect("attached receiver staging");
        let ProgramStorageEntryContinuationReceiverStagingPlan::BorrowedActivationLoan {
            candidate_parameter_index,
            candidate_placement,
            ..
        } = plan.receiver()
        else {
            panic!("attached entry must stage its receiver")
        };
        assert_eq!(*candidate_parameter_index, 0);
        assert!(
            candidate_placement
                .locations
                .iter()
                .any(|location| matches!(
                    location,
                    ValueLocation::Register {
                        register: MachineRegister::X86Rcx,
                        ..
                    }
                ))
        );
    }

    #[test]
    fn mapped_receiver_binding_checks_exact_storage_address_shape_and_live_loan() {
        let plan = plan_from_facts(
            omega_target::NativeTarget::windows_x64(),
            continuation(),
            Some(receiver()),
        )
        .expect("attached receiver staging");
        let exact = ProgramEntryReceiverPlacementRecord::for_test("&mut Boot", 0x8008, 8, 8);
        let binding = plan
            .bind_activation_loan(&exact, 8)
            .expect("exact mapped receiver loan");
        assert_eq!(binding.candidate_parameter_index(), 0);
        assert_eq!(binding.mapped_address(), 0x8008);

        for placement in [
            ProgramEntryReceiverPlacementRecord::for_test("&mut Other", 0x8008, 8, 8),
            ProgramEntryReceiverPlacementRecord::for_test("&mut Boot", 0x8008, 8, 0),
            ProgramEntryReceiverPlacementRecord::for_test("&mut Boot", 0x8003, 8, 8),
        ] {
            let error = plan
                .bind_activation_loan(&placement, 8)
                .expect_err("receiver staging drift must reject");
            assert!(error.0.contains("staging candidate"), "{error}");
        }
        let error = plan
            .bind_activation_loan(&exact, 7)
            .expect_err("short live loan must reject");
        assert!(error.0.contains("staging candidate"), "{error}");
    }

    #[test]
    fn free_form_is_explicitly_not_a_production_executor_path() {
        let plan = plan_from_facts(
            omega_target::NativeTarget::windows_x64(),
            continuation(),
            None,
        )
        .expect("free staging marker");
        assert!(matches!(
            plan.receiver(),
            ProgramStorageEntryContinuationReceiverStagingPlan::FreeWithoutProductionExecutor
        ));
        let unrelated = ProgramEntryReceiverPlacementRecord::for_test("internal", 0x8008, 8, 8);
        let error = plan
            .bind_activation_loan(&unrelated, 8)
            .expect_err("free entries cannot traverse the receiver activation executor");
        assert!(error.0.contains("no production"), "{error}");

        let error = plan_from_facts(
            omega_target::NativeTarget::windows_x64(),
            omega_control_flow::MachineFunctionIdentity::default(),
            None,
        )
        .expect_err("invalid continuation identity must reject");
        assert!(error.0.contains("exact source continuation"), "{error}");
    }
}

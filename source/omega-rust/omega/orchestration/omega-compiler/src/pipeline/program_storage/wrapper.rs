//! Address-free semantic transfer plan for a generated program-storage entry
//! wrapper.
//!
//! This module is intentionally narrower than target instruction lowering. It
//! records the facts already fixed by `ProgramStorageApplication`: both
//! physical roots are source-visible, their semantic ordinals and source-frame
//! captures are exact, and an attached receiver borrows the activation loan.
//! It does not invent the still-missing outbound source-call ABI, native body,
//! or direct-call relocation.

use super::program_storage_entry::{
    ProgramEntryReceiverStoragePlan, ProgramStorageEntryDiagnostic, ProgramStorageEntryParameter,
    ProgramStorageEntryPlanBinding,
};
use omega_calling_conventions::ValuePlacement;
pub use omega_program_storage::ProgramStorageEntryRootRole;
use std::ops::Range;

const IMAGE_PARAMETER_INDEX: usize = 0;
const INITIAL_STORAGE_PARAMETER_INDEX: usize = 1;

/// Exact mapping from one physical arrival root to the source-visible
/// continuation parameter selected by `ProgramStorageApplication`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryWrapperRootTransferPlan {
    role: ProgramStorageEntryRootRole,
    arrival_parameter_index: usize,
    source_parameter_index: usize,
    physical_arrival_placement: ValuePlacement,
    source_frame_byte_range: Range<usize>,
    source_capture_write_range: Range<usize>,
}

impl ProgramStorageEntryWrapperRootTransferPlan {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn arrival_parameter_index(&self) -> usize {
        self.arrival_parameter_index
    }

    pub const fn source_parameter_index(&self) -> usize {
        self.source_parameter_index
    }

    /// Placement in the physical arrival ABI. This is not an outbound
    /// source-call placement.
    pub const fn physical_arrival_placement(&self) -> &ValuePlacement {
        &self.physical_arrival_placement
    }

    pub const fn source_frame_byte_range(&self) -> &Range<usize> {
        &self.source_frame_byte_range
    }

    /// Exact instruction-row interval of the existing source-entry capture.
    pub const fn source_capture_write_range(&self) -> &Range<usize> {
        &self.source_capture_write_range
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramStorageEntryWrapperReceiverTransfer {
    Free,
    BorrowedActivationLoan(ProgramEntryReceiverStoragePlan),
}

/// Canonical semantic handoff retained before native wrapper lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryWrapperTransferPlan {
    wrapper_identity: omega_control_flow::MachineFunctionIdentity,
    continuation_identity: omega_control_flow::MachineFunctionIdentity,
    roots: [ProgramStorageEntryWrapperRootTransferPlan; 2],
    receiver: ProgramStorageEntryWrapperReceiverTransfer,
}

impl ProgramStorageEntryWrapperTransferPlan {
    pub const fn wrapper_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.wrapper_identity
    }

    pub const fn continuation_identity(&self) -> omega_control_flow::MachineFunctionIdentity {
        self.continuation_identity
    }

    pub const fn roots(&self) -> &[ProgramStorageEntryWrapperRootTransferPlan; 2] {
        &self.roots
    }

    pub const fn receiver(&self) -> &ProgramStorageEntryWrapperReceiverTransfer {
        &self.receiver
    }
}

pub(super) fn plan_program_storage_entry_wrapper_transfer(
    binding: &ProgramStorageEntryPlanBinding,
    continuation_key: omega_control_flow::StateKey,
) -> Result<ProgramStorageEntryWrapperTransferPlan, ProgramStorageEntryDiagnostic> {
    plan_from_facts(
        continuation_key,
        RootTransferFacts::from(binding.image()),
        RootTransferFacts::from(binding.initial_storage()),
        binding.receiver().cloned(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RootTransferFacts {
    parameter_index: usize,
    physical_arrival_placement: ValuePlacement,
    source_destination_byte_offset: usize,
    source_capture_write_range: Range<usize>,
}

impl From<&ProgramStorageEntryParameter> for RootTransferFacts {
    fn from(parameter: &ProgramStorageEntryParameter) -> Self {
        Self {
            parameter_index: parameter.parameter_index(),
            physical_arrival_placement: parameter.placement().clone(),
            source_destination_byte_offset: parameter.destination_byte_offset(),
            source_capture_write_range: parameter.write_range().clone(),
        }
    }
}

fn plan_from_facts(
    continuation_key: omega_control_flow::StateKey,
    image_facts: RootTransferFacts,
    initial_storage_facts: RootTransferFacts,
    receiver_storage: Option<ProgramEntryReceiverStoragePlan>,
) -> Result<ProgramStorageEntryWrapperTransferPlan, ProgramStorageEntryDiagnostic> {
    let wrapper_identity =
        omega_control_flow::MachineFunctionIdentity::program_storage_entry_wrapper(
            continuation_key,
        )
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "program-storage wrapper transfer has no exact generated identity".into(),
            )
        })?;
    let image = root_transfer(
        ProgramStorageEntryRootRole::Image,
        IMAGE_PARAMETER_INDEX,
        image_facts,
    )?;
    let initial_storage = root_transfer(
        ProgramStorageEntryRootRole::InitialStorage,
        INITIAL_STORAGE_PARAMETER_INDEX,
        initial_storage_facts,
    )?;
    if ranges_overlap(
        &image.source_frame_byte_range,
        &initial_storage.source_frame_byte_range,
    ) {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper source root frame ranges overlap".into(),
        ));
    }
    if ranges_overlap(
        &image.source_capture_write_range,
        &initial_storage.source_capture_write_range,
    ) {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper source root capture rows overlap".into(),
        ));
    }
    let receiver = receiver_storage.map_or(
        ProgramStorageEntryWrapperReceiverTransfer::Free,
        ProgramStorageEntryWrapperReceiverTransfer::BorrowedActivationLoan,
    );
    Ok(ProgramStorageEntryWrapperTransferPlan {
        wrapper_identity,
        continuation_identity: omega_control_flow::MachineFunctionIdentity::source(
            continuation_key,
        ),
        roots: [image, initial_storage],
        receiver,
    })
}

fn root_transfer(
    role: ProgramStorageEntryRootRole,
    expected_index: usize,
    facts: RootTransferFacts,
) -> Result<ProgramStorageEntryWrapperRootTransferPlan, ProgramStorageEntryDiagnostic> {
    if facts.parameter_index != expected_index {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-storage wrapper {role:?} root drifted from semantic parameter {expected_index}"
        )));
    }
    if facts.source_capture_write_range.is_empty() {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "program-storage wrapper {role:?} root has no source capture instructions"
        )));
    }
    let source_frame_end = facts
        .source_destination_byte_offset
        .checked_add(usize::from(
            facts.physical_arrival_placement.shape.byte_size,
        ))
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(format!(
                "program-storage wrapper {role:?} source frame range overflows"
            ))
        })?;
    Ok(ProgramStorageEntryWrapperRootTransferPlan {
        role,
        arrival_parameter_index: expected_index,
        source_parameter_index: expected_index,
        physical_arrival_placement: facts.physical_arrival_placement,
        source_frame_byte_range: facts.source_destination_byte_offset..source_frame_end,
        source_capture_write_range: facts.source_capture_write_range,
    })
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{ValuePlacement, ValueShape};
    use psi_symbols::SymbolHandle;

    fn key() -> omega_control_flow::StateKey {
        omega_control_flow::StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        }
    }

    fn facts(index: usize, destination: usize, writes: Range<usize>) -> RootTransferFacts {
        RootTransferFacts {
            parameter_index: index,
            physical_arrival_placement: ValuePlacement {
                shape: ValueShape::integer(16, 8),
                locations: Vec::new(),
            },
            source_destination_byte_offset: destination,
            source_capture_write_range: writes,
        }
    }

    #[test]
    fn wrapper_transfer_seals_visible_roots_without_claiming_outbound_abi() {
        let plan = plan_from_facts(key(), facts(0, 0, 0..2), facts(1, 16, 2..4), None)
            .expect("disjoint visible root captures should form a transfer plan");

        assert_eq!(
            plan.wrapper_identity().program_storage_entry_continuation(),
            Some(key())
        );
        assert_eq!(
            plan.continuation_identity(),
            omega_control_flow::MachineFunctionIdentity::source(key())
        );
        let [image, initial_storage] = plan.roots();
        assert_eq!(image.role(), ProgramStorageEntryRootRole::Image);
        assert_eq!(
            initial_storage.role(),
            ProgramStorageEntryRootRole::InitialStorage
        );
        assert_eq!(image.source_frame_byte_range(), &(0..16));
        assert_eq!(initial_storage.source_frame_byte_range(), &(16..32));
        assert_eq!(image.source_capture_write_range(), &(0..2));
        assert_eq!(
            plan.receiver(),
            &ProgramStorageEntryWrapperReceiverTransfer::Free
        );
    }

    #[test]
    fn wrapper_transfer_rejects_identity_ordinal_and_capture_tampering() {
        let error = plan_from_facts(
            omega_control_flow::StateKey::default(),
            facts(0, 0, 0..2),
            facts(1, 16, 2..4),
            None,
        )
        .expect_err("invalid continuation cannot acquire a generated identity");
        assert!(error.0.contains("generated identity"), "{error}");

        let error = plan_from_facts(key(), facts(1, 0, 0..2), facts(1, 16, 2..4), None)
            .expect_err("root roles cannot be redirected by ordinal");
        assert!(error.0.contains("Image root drifted"), "{error}");

        let error = plan_from_facts(key(), facts(0, 0, 0..0), facts(1, 16, 2..4), None)
            .expect_err("a visible root requires retained capture instructions");
        assert!(error.0.contains("no source capture"), "{error}");

        let error = plan_from_facts(key(), facts(0, 0, 0..2), facts(1, 8, 2..4), None)
            .expect_err("source root frame ranges must remain disjoint");
        assert!(error.0.contains("frame ranges overlap"), "{error}");

        let error = plan_from_facts(key(), facts(0, 0, 0..3), facts(1, 16, 2..4), None)
            .expect_err("source capture instruction ownership must remain disjoint");
        assert!(error.0.contains("capture rows overlap"), "{error}");
    }
}

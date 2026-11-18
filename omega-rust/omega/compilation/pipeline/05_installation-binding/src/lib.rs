//! Bind guarded installations against the complete retained physical program.
#![forbid(unsafe_code)]

use installation_admission::{BoundInstallationAuthority, PlannedInstallationResources};
use installation_evidence::{InstalledInterruptStackRequirement, InstalledPhysicalStackRoot};
use physical_planning::{PhysicalFrameProgram, installed_handlers::MeasuredInstalledHandler};
use std::{collections::BTreeMap, sync::Arc};

/// One canonical measurement per actually demanded handler in the final body roster.
#[derive(Debug, Default)]
pub struct RetainedHandlerMeasurements {
    rows: BTreeMap<
        (
            semantic_vocabulary::MachineId,
            semantic_vocabulary::OperationId,
            usize,
        ),
        InstalledInterruptStackRequirement,
    >,
}
impl RetainedHandlerMeasurements {
    pub fn new(
        rows: impl IntoIterator<Item = MeasuredInstalledHandler>,
    ) -> Result<Self, &'static str> {
        Self::from_rows(
            rows.into_iter()
                .map(MeasuredInstalledHandler::into_requirement),
        )
    }
    fn from_rows(
        rows: impl IntoIterator<Item = InstalledInterruptStackRequirement>,
    ) -> Result<Self, &'static str> {
        let mut measured = Self::default();
        for row in rows {
            if row.arrival.is_some()
                || row.own_bytes().is_none()
                || !row.alignment.is_power_of_two()
            {
                return Err("retained handler measurement is not a finite uncomposed stack demand");
            }
            if measured
                .rows
                .insert(
                    (row.installation_machine, row.operation, row.placement_index),
                    row,
                )
                .is_some()
            {
                return Err("retained installed handler measurement is duplicated");
            }
        }
        Ok(measured)
    }
    fn measure_guard(
        &self,
        guard: &InstalledInterruptStackRequirement,
    ) -> Result<InstalledInterruptStackRequirement, &'static str> {
        let mut measured = self
            .rows
            .get(&(
                guard.installation_machine,
                guard.operation,
                guard.placement_index,
            ))
            .ok_or("installed stack guard has no retained physical handler")?
            .clone();
        // Composition is checked below against the complete admitted scope.
        // Every other field must already match the retained handler itself.
        measured.arrival.clone_from(&guard.arrival);
        if &measured != guard {
            return Err("installed stack guard differs from the retained handler measurement");
        }
        measured.arrival = None;
        Ok(measured)
    }
}

/// Join each installer operand to the measured private body at that exact placement.
pub fn bind(
    frame: &PhysicalFrameProgram,
    resources: Arc<PlannedInstallationResources>,
    handlers: &RetainedHandlerMeasurements,
    root: Option<InstalledPhysicalStackRoot>,
) -> Result<BoundInstallationAuthority, &'static str> {
    let measured = resources
        .installed_stack_requirements()
        .iter()
        .map(|guard| handlers.measure_guard(guard))
        .collect::<Result<Vec<_>, _>>()?;
    BoundInstallationAuthority::verify(&frame.instructions, &frame.frame, resources, measured, root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};

    #[test]
    fn a_guard_must_match_the_actual_handler_at_its_placement() {
        let row = InstalledInterruptStackRequirement {
            installation_machine: semantic_vocabulary::MachineId::new(5).unwrap(),
            arrival: None,
            fault_exclusion: None,
            psi: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([8; 32]),
            },
            target: target::TargetProfile::UefiX64.native_target(),
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            placement_index: 2,
            entry: semantic_vocabulary::MachineId::new(4).unwrap(),
            entry_plan: InstalledInterruptEntryPlan::new(InstalledInterruptFrame::NoErrorCode),
            body_bytes: 128,
            bridge_bytes: 96,
            call_stack_bias: 8,
            alignment: 16,
        };
        let handlers = RetainedHandlerMeasurements::from_rows([row.clone()]).unwrap();
        assert_eq!(handlers.measure_guard(&row), Ok(row.clone()));
        assert!(RetainedHandlerMeasurements::from_rows([row.clone(), row.clone()]).is_err());
        let mut wrong = row.clone();
        wrong.bridge_bytes -= 8;
        wrong.body_bytes += 8;
        assert_eq!(wrong.own_bytes(), row.own_bytes());
        assert!(handlers.measure_guard(&wrong).is_err());
        let mut other_machine = row.clone();
        other_machine.installation_machine = semantic_vocabulary::MachineId::new(6).unwrap();
        assert!(handlers.measure_guard(&other_machine).is_err());
        let mut missing = row;
        missing.placement_index += 1;
        assert!(handlers.measure_guard(&missing).is_err());
    }
}

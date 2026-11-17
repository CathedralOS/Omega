//! Bind guarded installations against the complete retained physical program.
#![forbid(unsafe_code)]
use bound_installations::BoundInstalledEntry;
use installation_evidence::{InstalledInterruptStackRequirement, InstalledPhysicalStackRoot};
use physical_planning::{
    PhysicalFrameProgram, installation_resources::PlannedInstallationResources,
};
use std::{collections::BTreeMap, sync::Arc};

/// One canonical measurement per actually demanded handler in the final body roster.
#[derive(Debug, Default)]
pub struct RetainedHandlerMeasurements {
    rows: BTreeMap<(semantic_vocabulary::MachineId, semantic_vocabulary::OperationId, usize), InstalledInterruptStackRequirement>,
}
impl RetainedHandlerMeasurements {
    pub fn new(
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
                .insert((row.installation_machine, row.operation, row.placement_index), row)
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
            .get(&(guard.installation_machine, guard.operation, guard.placement_index))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInstallationAuthority(Arc<PlannedInstallationResources>);

impl BoundInstallationAuthority {
    pub fn bind(
        frame: &PhysicalFrameProgram,
        resources: Arc<PlannedInstallationResources>,
        handlers: &RetainedHandlerMeasurements,
        root: Option<InstalledPhysicalStackRoot>,
    ) -> Result<Self, &'static str> {
        let physical = &frame.instructions;
        let entry = BoundInstalledEntry {
            psi: physical.psi,
            target: physical.machine.target,
            machine: physical.entry,
        };
        if entry != resources.program() || root.is_some_and(|root| root.adapter != entry.machine) {
            return Err(
                "installation resources differ from the final physical program, target or entry",
            );
        }
        let expected = resources.installed_stack_requirements();
        let mut actual = expected
            .iter()
            .map(|guard| handlers.measure_guard(guard))
            .collect::<Result<Vec<_>, _>>()?;
        for scope in resources.installed_interrupts().scopes() {
            let first = actual.partition_point(|row| (row.installation_machine, row.operation) < (scope.machine(), scope.operation()));
            let end = actual.partition_point(|row| (row.installation_machine, row.operation) <= (scope.machine(), scope.operation()));
            if first == end {
                continue;
            }
            let entries = resources
                .installed_interrupts()
                .entries()
                .iter()
                .filter(|entry| entry.machine == scope.machine() && entry.operation == scope.operation())
                .collect::<Vec<_>>();
            physical_planning::installed_arrival::compose(
                scope,
                &entries,
                &mut actual[first..end],
                root,
            )
            .map_err(|error| match error {
                physical_planning::installed_arrival::ArrivalCompositionError::Mismatch(reason) => {
                    reason
                }
                physical_planning::installed_arrival::ArrivalCompositionError::Overflow => {
                    "installed arrival stack overflows"
                }
            })?;
        }
        if actual != expected {
            return Err(
                "installed stack guard differs from the retained body, bridge, fault premises or arrival demand",
            );
        }
        Ok(Self(resources))
    }
}
impl std::ops::Deref for BoundInstallationAuthority {
    type Target = PlannedInstallationResources;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
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
        let handlers = RetainedHandlerMeasurements::new([row.clone()]).unwrap();
        assert_eq!(handlers.measure_guard(&row), Ok(row.clone()));
        assert!(RetainedHandlerMeasurements::new([row.clone(), row.clone()]).is_err());
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

//! The receipt binds one physical program and checked frame to the exact handler measurements.
use crate::{PlannedInstallationResources, arrival};
use bound_installations::BoundInstalledEntry;
use frame_admission::frame_layout::ValidatedTargetFrameLayout;
use installation_conditions::{InstalledInterruptStackRequirement, InstalledPhysicalStackRoot};
use machine_code::TargetFrameLayoutPlan;
use post_allocation_machine::PostAllocationMachineProgram;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInstallationAuthority {
    resources: Arc<PlannedInstallationResources>,
    physical_program: [u8; 32],
    frame_layout: Arc<TargetFrameLayoutPlan>,
}

impl BoundInstallationAuthority {
    /// Whether this authority was issued for `program` and the frame plan
    /// `frame`. A shared plan compares by pointer, a copy field by field.
    pub fn matches_frame(&self, program: [u8; 32], frame: &Arc<TargetFrameLayoutPlan>) -> bool {
        self.physical_program == program && self.frame_layout == *frame
    }

    /// Compare the uncomposed measurements from the retained private bodies
    /// with the guards already present in this physical program. The physical
    /// frame plan ties the result to the same register-allocated machine.
    pub fn verify(
        physical: &PostAllocationMachineProgram,
        frame: &ValidatedTargetFrameLayout,
        resources: Arc<PlannedInstallationResources>,
        mut actual: Vec<InstalledInterruptStackRequirement>,
        root: Option<InstalledPhysicalStackRoot>,
    ) -> Result<Self, &'static str> {
        let entry = BoundInstalledEntry {
            psi: physical.psi,
            target: physical.machine.target,
            machine: physical.entry,
        };
        let plan = frame.plan();
        if entry != resources.program()
            || root.is_some_and(|root| root.adapter != entry.machine)
            || plan.target != entry.target
            || physical.identity != physical.recomputed_identity()
            || plan.post_allocation_machine != physical.machine
        {
            return Err(
                "installation resources differ from the final physical program, target or entry",
            );
        }
        let expected = resources.installed_stack_requirements();
        if actual.len() != expected.len() {
            return Err("installed stack guard has no retained physical handler");
        }
        for (measured, guard) in actual.iter().zip(expected) {
            if measured.arrival.is_some()
                || measured.own_bytes().is_none()
                || !measured.alignment.is_power_of_two()
            {
                return Err("retained handler measurement is not a finite uncomposed stack demand");
            }
            if (
                measured.installation_machine,
                measured.operation,
                measured.placement_index,
            ) != (
                guard.installation_machine,
                guard.operation,
                guard.placement_index,
            ) {
                return Err("installed stack guard differs from the retained handler measurement");
            }
        }
        for scope in resources.installed_interrupts().scopes() {
            let first = actual.partition_point(|row| {
                (row.installation_machine, row.operation) < (scope.machine(), scope.operation())
            });
            let end = actual.partition_point(|row| {
                (row.installation_machine, row.operation) <= (scope.machine(), scope.operation())
            });
            if first == end {
                continue;
            }
            let entries = resources
                .installed_interrupts()
                .entries()
                .iter()
                .filter(|entry| {
                    entry.machine == scope.machine() && entry.operation == scope.operation()
                })
                .collect::<Vec<_>>();
            arrival::compose(scope, &entries, &mut actual[first..end], root).map_err(|error| {
                match error {
                    arrival::ArrivalCompositionError::Mismatch(reason) => reason,
                    arrival::ArrivalCompositionError::Overflow => {
                        "installed arrival stack overflows"
                    }
                }
            })?;
        }
        if actual != expected {
            return Err(
                "installed stack guard differs from the retained body, bridge, fault premises or arrival demand",
            );
        }
        Ok(Self {
            resources,
            physical_program: physical.identity,
            frame_layout: frame.shared_plan(),
        })
    }
}
impl std::ops::Deref for BoundInstallationAuthority {
    type Target = PlannedInstallationResources;
    fn deref(&self) -> &Self::Target {
        &self.resources
    }
}

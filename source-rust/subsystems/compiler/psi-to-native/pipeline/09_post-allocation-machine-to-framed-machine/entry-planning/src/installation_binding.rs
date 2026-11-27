//! Stage 09's second phase binds guarded installations against the complete
//! retained physical program. The driver measures every retained installed
//! handler after all bodies are framed, then binds each body here; only this
//! module holds the issuer that lets stage 11 accept a bound body.

use installation_admission::{BoundInstallationAuthority, PlannedInstallationResources};
use installation_conditions::{InstalledInterruptStackRequirement, InstalledPhysicalStackRoot};
/// Bind planner-issued entry recipes for encoding. Callable entries leave their CPU
/// measurements here once installation binding has used them.
pub mod entries {
    use crate::{
        PhysicalRecipeOrigin as Planner,
        callable_entry::{PlannedCallableEntryTransport, PlannedPrivateEntry},
    };
    use bound_framed_machine::entries::{self, BoundEntry};
    use installation_binding_issuer::BINDER;
    use physical_recipes::{
        callable_entry::CallableInstructions, hosted_program::HostedProgram,
        native_foreign_bridge::ForeignAdapterPlan, physical_entry::PhysicalEntryProgram,
    };

    /// A foreign adapter must carry the frame planner's issuing type.
    /// ```compile_fail,E0308
    /// fn bind(adapter: physical_recipes::native_foreign_bridge::ForeignAdapterPlan<()>) {
    ///     let _ = entry_planning::installation_binding::entries::bind_foreign_adapter(adapter);
    /// }
    /// ```
    pub fn bind_foreign_adapter(
        plan: ForeignAdapterPlan<Planner>,
    ) -> entries::BoundForeignAdapter<Planner> {
        BoundEntry::from_binding(BINDER, plan.into_parts())
    }
    pub fn bind_hosted_entry<C>(
        plan: HostedProgram<Planner, C>,
    ) -> entries::BoundHostedEntry<Planner, C> {
        BoundEntry::from_binding(BINDER, plan.into_parts())
    }
    pub fn bind_physical_entry<C>(
        plan: PhysicalEntryProgram<Planner, C>,
    ) -> entries::BoundPhysicalEntry<Planner, C> {
        BoundEntry::from_binding(BINDER, plan.into_parts())
    }
    pub fn bind_callable_entry(
        plan: PlannedCallableEntryTransport,
    ) -> entries::BoundCallableEntry<Planner> {
        let CallableInstructions {
            context, bridge, ..
        } = plan.into_parts();
        BoundEntry::from_binding(BINDER, (context, bridge))
    }
    pub fn bind_private_entry(plan: PlannedPrivateEntry) -> entries::BoundPrivateEntry<Planner> {
        let parts = plan.into_parts();
        BoundEntry::from_binding(
            BINDER,
            (parts.identity, bind_callable_entry(parts.transport)),
        )
    }
}

pub mod installed_handlers;
use frame_planning::PhysicalFrameProgram;
use installed_handlers::MeasuredInstalledHandler;
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

pub use physical_program::BoundBodyContracts;
pub type BoundBodyPublication = physical_program::BodyPublication<Arc<BoundBodyContracts>>;

pub type BoundFunctionBody = bound_framed_machine::BoundFunctionBody<
    frame_planning::FrameRecipeOrigin,
    crate::PhysicalRecipeOrigin,
    BoundInstallationAuthority,
    BoundBodyPublication,
>;

pub fn bind_function_body(
    body: crate::imports::PreparedFunctionBody,
    handlers: &RetainedHandlerMeasurements,
    root: Option<InstalledPhysicalStackRoot>,
) -> Result<BoundFunctionBody, &'static str> {
    let (program, facts, resources) = body.into_parts();
    let installations = Arc::new(bind_frame(&program.frame, resources, handlers, root)?);
    let publication = facts.map_facts(|facts| {
        Arc::new(BoundBodyContracts {
            facts,
            installations: Arc::clone(&installations),
        })
    });
    let subject = publication
        .facts
        .encoding_subject(program.frame.frame.shared_plan())?;
    let retain_original = publication.original_for_audit().is_some();
    let check_branches = publication.facts.needs_installed_branch_checks();
    let requirements = machine_code::layout::BoundEncodingRequirements::new(
        subject,
        publication,
        retain_original,
        check_branches,
    );
    Ok(BoundFunctionBody::from_binding(
        installation_binding_issuer::BINDER,
        program,
        installations,
        requirements,
    ))
}

/// Join each installer operand to the measured private body at that exact placement.
fn bind_frame(
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

//! Bind guarded installations against the complete retained physical program.
#![forbid(unsafe_code)]

use installation_admission::{BoundInstallationAuthority, PlannedInstallationResources};
use installation_conditions::{InstalledInterruptStackRequirement, InstalledPhysicalStackRoot};
/// Bind planner-issued entry recipes for encoding. Callable entries leave their CPU
/// measurements here once installation binding has used them.
pub mod entries {
    use entry_planning::{
        PhysicalRecipeOrigin as Planner,
        callable_entry::{PlannedCallableEntryTransport, PlannedPrivateEntry},
    };
    use function_identity::MachineFunctionIdentity;
    use physical_recipes::{
        callable_entry::{CallableContext, CallableInstructions},
        hosted_program::{HostedProgram, HostedProgramParts},
        imports::PhysicalImportProgram,
        native_entry_bridge::NativeEntryBridgePlan,
        native_foreign_bridge::{ForeignAdapterParts, ForeignAdapterPlan, NativeForeignBridgePlan},
        physical_entry::{PhysicalEntryParts, PhysicalEntryProgram},
    };

    pub struct BoundEntry<Parts>(Parts);

    impl<Parts> BoundEntry<Parts> {
        pub fn into_parts(self) -> Parts {
            self.0
        }
    }

    pub type BoundForeignAdapter = BoundEntry<
        ForeignAdapterParts<NativeForeignBridgePlan<Planner>, PhysicalImportProgram<Planner>>,
    >;
    pub type BoundHostedEntry<C> = BoundEntry<HostedProgramParts<Planner, C>>;
    pub type BoundPhysicalEntry<C> = BoundEntry<PhysicalEntryParts<Planner, C>>;
    pub type BoundCallableEntry =
        BoundEntry<(CallableContext, Option<NativeEntryBridgePlan<Planner>>)>;
    pub type BoundPrivateEntry = BoundEntry<(MachineFunctionIdentity, BoundCallableEntry)>;

    /// A foreign adapter must carry the frame planner's issuing type.
    /// ```compile_fail,E0308
    /// fn bind(adapter: physical_recipes::native_foreign_bridge::ForeignAdapterPlan<()>) {
    ///     let _ = installation_binding::entries::bind_foreign_adapter(adapter);
    /// }
    /// ```
    pub fn bind_foreign_adapter(plan: ForeignAdapterPlan<Planner>) -> BoundForeignAdapter {
        BoundEntry(plan.into_parts())
    }
    pub fn bind_hosted_entry<C>(plan: HostedProgram<Planner, C>) -> BoundHostedEntry<C> {
        BoundEntry(plan.into_parts())
    }
    pub fn bind_physical_entry<C>(plan: PhysicalEntryProgram<Planner, C>) -> BoundPhysicalEntry<C> {
        BoundEntry(plan.into_parts())
    }
    pub fn bind_callable_entry(plan: PlannedCallableEntryTransport) -> BoundCallableEntry {
        let CallableInstructions {
            context, bridge, ..
        } = plan.into_parts();
        BoundEntry((context, bridge))
    }
    pub fn bind_private_entry(plan: PlannedPrivateEntry) -> BoundPrivateEntry {
        let parts = plan.into_parts();
        BoundEntry((parts.identity, bind_callable_entry(parts.transport)))
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

pub struct BoundFunctionBody<Publication = BoundBodyPublication> {
    program: entry_planning::imports::PlannedFunctionBody,
    publication: Publication,
    subject: machine_code::program_context::BoundEncodingSubject,
    retain_original: bool,
    check_installed_branches: bool,
}

impl BoundFunctionBody<BoundBodyPublication> {
    pub fn program(&self) -> &entry_planning::imports::PlannedFunctionBody {
        &self.program
    }
    pub fn installations(&self) -> &BoundInstallationAuthority {
        &self.publication.facts.installations
    }
}

impl<Publication> BoundFunctionBody<Publication> {
    pub fn into_encoding_parts(
        self,
    ) -> (
        entry_planning::imports::PlannedFunctionBody,
        machine_code::layout::BoundEncodingRequirements<Publication>,
    ) {
        (
            self.program,
            machine_code::layout::BoundEncodingRequirements::new(
                self.subject,
                self.publication,
                self.retain_original,
                self.check_installed_branches,
            ),
        )
    }
}

pub fn bind_function_body(
    body: entry_planning::imports::PreparedFunctionBody,
    handlers: &RetainedHandlerMeasurements,
    root: Option<InstalledPhysicalStackRoot>,
) -> Result<BoundFunctionBody, &'static str> {
    let (program, facts, resources) = body.into_parts();
    let installations = Arc::new(bind_frame(&program.frame, resources, handlers, root)?);
    let publication = facts.map_facts(|facts| {
        Arc::new(BoundBodyContracts {
            facts,
            installations,
        })
    });
    Ok(BoundFunctionBody {
        subject: publication
            .facts
            .encoding_subject(program.frame.frame.receipt().identity())?,
        retain_original: publication.original_for_audit().is_some(),
        check_installed_branches: publication.facts.needs_installed_branch_checks(),
        program,
        publication,
    })
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

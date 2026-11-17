//! Bind admitted installation facts after the driver has measured handler stacks.
//!
//! Both native consumers share this checked receipt. It contains only the plain
//! bound facts and the existing sealed entry-execution evidence, never a program
//! representation. Keeping construction here preserves admission's issuance
//! checks without making stage 02 mutable after it publishes its plan.

use crate::{
    AdmittedInstalledEntryExecution, AdmittedInstalledInterruptCohort, AdmittedNativeCallbackCohort,
};
use bound_installations::{BoundInstallations, BoundInstalledEntry};
use installation_evidence::InstalledInterruptStackRequirement;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInstallationAuthority {
    program: BoundInstalledEntry,
    facts: Arc<BoundInstallations>,
    entry_execution: Option<AdmittedInstalledEntryExecution>,
    installed: AdmittedInstalledInterruptCohort,
    callbacks: AdmittedNativeCallbackCohort,
}

impl BoundInstallationAuthority {
    pub const fn program(&self) -> BoundInstalledEntry {
        self.program
    }

    pub fn facts(&self) -> &Arc<BoundInstallations> {
        &self.facts
    }

    pub fn installed_entry_execution(&self) -> Option<&AdmittedInstalledEntryExecution> {
        self.entry_execution.as_ref()
    }

    pub fn installed_stack_requirements(&self) -> &[InstalledInterruptStackRequirement] {
        &self.facts.stack_requirements
    }

    pub fn installed_interrupts(&self) -> &AdmittedInstalledInterruptCohort {
        &self.installed
    }

    pub fn native_callbacks(&self) -> &AdmittedNativeCallbackCohort {
        &self.callbacks
    }

    /// Joins this receipt to the original admitted leaf rosters without
    /// repeating their proofs or retaining an earlier program representation.
    pub fn matches_admission(
        &self,
        installed: &AdmittedInstalledInterruptCohort,
        callbacks: &AdmittedNativeCallbackCohort,
    ) -> bool {
        &self.installed == installed && &self.callbacks == callbacks
    }
}

pub fn bind_installation_authority(
    program: BoundInstalledEntry,
    cohort: &AdmittedInstalledInterruptCohort,
    callbacks: &AdmittedNativeCallbackCohort,
    entry_execution: Option<AdmittedInstalledEntryExecution>,
    mut requirements: Vec<InstalledInterruptStackRequirement>,
) -> Result<BoundInstallationAuthority, &'static str> {
    if cohort.psi() != program.psi || callbacks.psi() != program.psi {
        return Err("installed authority belongs to another program");
    }
    if let Some(execution) = &entry_execution {
        let entry = execution.entry().entry();
        if entry.psi() != program.psi
            || entry.target() != program.target
            || entry.machine() != program.machine
        {
            return Err("installed execution differs from the current private object");
        }
        execution.require_current(cohort)?;
    }
    let mut entries: Vec<_> = cohort
        .entries()
        .iter()
        .map(|entry| (entry.operation, entry.placement_index))
        .collect();
    validate_stack_requirements(program, &mut entries, &mut requirements)?;
    let facts = crate::installed_interrupt_helpers::bind_installations(
        cohort,
        entry_execution.as_ref(),
        requirements,
    );
    Ok(BoundInstallationAuthority {
        program,
        facts: Arc::new(facts),
        entry_execution,
        installed: cohort.clone(),
        callbacks: callbacks.clone(),
    })
}

fn validate_stack_requirements(
    program: BoundInstalledEntry,
    entries: &mut [(semantic_vocabulary::OperationId, usize)],
    requirements: &mut [InstalledInterruptStackRequirement],
) -> Result<(), &'static str> {
    entries.sort_unstable();
    requirements.sort_unstable_by_key(|row| (row.operation, row.placement_index));
    if entries.windows(2).any(|pair| pair[0] == pair[1])
        || requirements.windows(2).any(|pair| {
            (pair[0].operation, pair[0].placement_index)
                == (pair[1].operation, pair[1].placement_index)
        })
    {
        return Err("installed stack requirement or admitted placement is duplicated");
    }
    for requirement in requirements {
        if requirement.psi != program.psi
            || requirement.target != program.target
            || requirement.required_bytes().is_none()
            || requirement.alignment == 0
            || !requirement.alignment.is_power_of_two()
            || entries
                .binary_search(&(requirement.operation, requirement.placement_index))
                .is_err()
        {
            return Err("installed stack requirement has no matching admitted entry");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::MachineId;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn program() -> BoundInstalledEntry {
        BoundInstalledEntry {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([6; 32]),
            },
            target: target::TargetProfile::WindowsX64.native_target(),
            machine: MachineId::new(1).unwrap(),
        }
    }

    #[test]
    fn binding_shares_both_admitted_rosters_and_refuses_a_foreign_program() {
        let program = program();
        let installed = AdmittedInstalledInterruptCohort::empty(program.psi);
        let callbacks = AdmittedNativeCallbackCohort::empty(program.psi);
        let authority =
            bind_installation_authority(program, &installed, &callbacks, None, Vec::new()).unwrap();
        assert_eq!(authority.program(), program);
        assert!(authority.matches_admission(&installed, &callbacks));
        assert!(Arc::ptr_eq(
            &authority.installed.entries,
            &installed.entries
        ));
        assert!(Arc::ptr_eq(&authority.installed.scopes, &installed.scopes));
        assert!(Arc::ptr_eq(
            &authority.callbacks.callbacks,
            &callbacks.callbacks
        ));
        assert!(authority.installed_entry_execution().is_none());
        assert!(authority.installed_stack_requirements().is_empty());

        let mut foreign_psi = program.psi;
        foreign_psi.program_fingerprint = SemanticFingerprint::from_bytes([9; 32]);
        let foreign_installed = AdmittedInstalledInterruptCohort::empty(foreign_psi);
        let foreign_callbacks = AdmittedNativeCallbackCohort::empty(foreign_psi);
        assert!(!authority.matches_admission(&foreign_installed, &callbacks));
        assert!(!authority.matches_admission(&installed, &foreign_callbacks));
        assert_eq!(
            bind_installation_authority(program, &foreign_installed, &callbacks, None, Vec::new())
                .unwrap_err(),
            "installed authority belongs to another program"
        );
        assert_eq!(
            bind_installation_authority(program, &installed, &foreign_callbacks, None, Vec::new())
                .unwrap_err(),
            "installed authority belongs to another program"
        );
    }

    #[test]
    fn measured_stacks_keep_exact_roots_unique_entries_and_bounded_geometry() {
        use runtime_abi::static_interrupts::{
            InstalledInterruptEntryPlan, InstalledInterruptFrame,
        };
        let program = program();
        let requirement = InstalledInterruptStackRequirement {
            arrival: None,
            fault_exclusion: None,
            psi: program.psi,
            target: program.target,
            operation: semantic_vocabulary::OperationId::new(2).unwrap(),
            placement_index: 0,
            entry: program.machine,
            entry_plan: InstalledInterruptEntryPlan::new(InstalledInterruptFrame::NoErrorCode),
            body_bytes: 16,
            bridge_bytes: 160,
            call_stack_bias: 8,
            alignment: 16,
        };
        let key = (requirement.operation, requirement.placement_index);
        assert_eq!(
            validate_stack_requirements(program, &mut [key], &mut [requirement.clone()]),
            Ok(())
        );
        let changes: [fn(&mut InstalledInterruptStackRequirement); 5] = [
            |row| row.psi.program_fingerprint = SemanticFingerprint::from_bytes([9; 32]),
            |row| row.target = target::TargetProfile::LinuxX64.native_target(),
            |row| row.alignment = 3,
            |row| row.body_bytes = u64::MAX,
            |row| row.placement_index = 1,
        ];
        for change in changes {
            let mut invalid = requirement.clone();
            change(&mut invalid);
            assert_eq!(
                validate_stack_requirements(program, &mut [key], &mut [invalid]),
                Err("installed stack requirement has no matching admitted entry")
            );
        }
        assert_eq!(
            validate_stack_requirements(program, &mut [key, key], &mut [requirement.clone()]),
            Err("installed stack requirement or admitted placement is duplicated")
        );
        assert_eq!(
            validate_stack_requirements(
                program,
                &mut [key],
                &mut [requirement.clone(), requirement],
            ),
            Err("installed stack requirement or admitted placement is duplicated")
        );
    }
}

//! Fixtures shared by the external-root tests: fuel schedules, installed
//! code, boundary plans, candidates and slots with fixed seeds.

mod boundary_fixtures;
mod entry_stack_epochs;
mod installed_code_fixtures;
mod interrupt_and_progress_fixtures;
mod interrupt_entries;
mod program_local_epochs;
mod program_local_extents;
mod program_local_fixtures;
mod program_local_root_joins;
mod progress_profiles;
mod provider_execution;
mod required_root_closures;
mod root_admission_custody;
mod root_installation;
mod root_installation_fixtures;
mod stack_and_fuel_composition;

use boundary_fixtures::{
    admitted_arrival_contexts, body_domains, candidate, candidate_for_code, entry_writer,
    generated_program_storage_adapter_bound_input, generated_program_storage_boundary,
    interrupted_boundary, provider_selected_boundary, provider_selected_masked_boundary,
    selected_interrupt_completion, selected_interrupt_completion_for, slot, writer_site,
};
pub(crate) use boundary_fixtures::{
    boundary, generated_program_storage_adapter_live_frame_demand, provider_execution,
    provider_execution_for, stack_demand, stack_epoch_input,
};
use installed_code_fixtures::{
    entry_id, fuel_schedule, installed_code_with_fill_and_installation_identity,
    installed_program_storage_wrapper,
};
pub(crate) use installed_code_fixtures::{
    extent_id, extent_provider_issuance, installed_code, installed_code_in_placement,
    installed_code_in_placement_with_entries, installed_code_with_fill,
    minted_secondary_processor_state, root_id, secondary_processor_boundary,
};
use interrupt_and_progress_fixtures::{
    admitted_progress_receipt, interrupt_boundary, interrupt_candidate,
    interrupt_candidate_for_code, interrupt_candidate_for_code_with_completion,
    progress_installation_fixture, provider_occurrence_binding,
};
pub(crate) use interrupt_and_progress_fixtures::{
    interrupt_boundary_on, interrupt_boundary_shaped, interrupt_candidate_shaped,
    interrupt_entry_receipt, interrupt_entry_receipt_in_context, interrupt_table_candidates,
};
use program_local_fixtures::{
    installed_backing_extent, join_program_local, program_local_activation, program_local_claim,
    program_local_claim_at, program_local_epoch_lease, program_local_extent_module,
    program_local_extent_subject, program_local_extent_subject_at, program_local_lifecycle,
    program_local_root_catalog, program_local_root_module, program_local_subject,
    program_local_subject_at, program_local_terminal_object,
    program_local_two_schema_extent_module, program_local_two_schema_module,
    publish_program_local_era, sole_rejected_cohort_lease,
};
pub(crate) use root_installation_fixtures::install_test_root;
use root_installation_fixtures::{
    install_program_local_required_root, install_program_local_two_parameter_roots,
    install_test_root_pair_with_ids, install_test_root_pair_with_ids_unsealed,
    program_local_required_root_slot_closure,
};

use installation_evidence::{ObjectEvidence, StackDemandEvidence};
use layout_plans::EntryStubId;
use std::collections::BTreeSet;

#[derive(Debug)]
struct TestObject {
    identity: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    bytes: Vec<u8>,
}

impl ObjectEvidence for TestObject {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }

    fn target(&self) -> target::NativeTarget {
        target::NativeTarget::linux_x64()
    }

    fn text_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn function_text_offset(&self, machine: semantic_vocabulary::MachineId) -> Option<usize> {
        (machine == self.entry).then_some(16)
    }
}

struct TestStackDemand {
    identity: terminal_psi::TerminalPsiIdentity,
    entry: semantic_vocabulary::MachineId,
    contributing: BTreeSet<semantic_vocabulary::MachineId>,
    admitted_reports: BTreeSet<u64>,
    admitted_commitments: BTreeSet<[u8; 32]>,
}

impl StackDemandEvidence for TestStackDemand {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }

    fn architecture(&self) -> target::Architecture {
        target::Architecture::X86_64
    }

    fn entry(&self) -> semantic_vocabulary::MachineId {
        self.entry
    }

    fn ceiling_bytes(&self) -> u64 {
        64
    }

    fn stack_alignment(&self) -> u32 {
        16
    }

    fn contributing_machines(&self) -> &BTreeSet<semantic_vocabulary::MachineId> {
        &self.contributing
    }

    fn admitted_stack_contribution_report_identities(&self) -> BTreeSet<u64> {
        self.admitted_reports.clone()
    }

    fn admitted_stack_contribution_commitments(&self) -> BTreeSet<[u8; 32]> {
        self.admitted_commitments.clone()
    }
}

/// One declared interrupt-table member fixture: its root identity seed,
/// selected entry, declared dedicated stack class, and acknowledgement
/// obligation.
pub(crate) struct InterruptTableMemberFixture {
    pub root_identity: u64,
    pub entry: EntryStubId,
    pub stack_class: u16,
    pub acknowledged: bool,
}

//! Root installation fixtures: single roots, root pairs and program-local
//! required roots.

use super::boundary_fixtures::{
    boundary, candidate_for_code_with_root, candidate_for_code_with_root_on_boundary,
    provider_execution, two_parameter_boundary,
};
use super::installed_code_fixtures::root_id;
use crate::{
    ExternalRootEntryClaim, InstalledExternalRoot, InstalledRootLedger, RootAdmission,
    RootAdmissionId, RootSlotAuthority, RootSlotId, RootSlotOwnerId, StackNestingRelation,
    TargetRequiredRootSlotSelection, VerifiedRequiredRootSlotClosure,
    compose_bound_entry_stack_epochs, validate_external_root,
    verify_target_required_root_slot_closure,
};
use calling_conventions::ValidatedBoundaryEntryPlan;
use executable_installation::InstalledCode;
use layout_plans::EntryStubId;
use std::collections::BTreeSet;

pub(crate) fn install_test_root<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    install_test_root_with_ids(code, entry, 1, 20, 21, 22, Vec::new())
}

pub(super) fn install_test_root_with_ids<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    root_identity: u64,
    slot_identity: u64,
    owner_identity: u64,
    admission_identity: u64,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> (InstalledRootLedger, InstalledExternalRoot<'code>) {
    let mut ledger = InstalledRootLedger::claim(code).expect("canonical root ledger");
    let installed = install_test_root_in_ledger(
        &mut ledger,
        code,
        entry,
        root_identity,
        slot_identity,
        owner_identity,
        admission_identity,
        entry_claims,
    );
    (ledger, installed)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn install_test_root_in_ledger<'code>(
    ledger: &mut InstalledRootLedger,
    code: &'code InstalledCode,
    entry: EntryStubId,
    root_identity: u64,
    slot_identity: u64,
    owner_identity: u64,
    admission_identity: u64,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> InstalledExternalRoot<'code> {
    let mut candidate = candidate_for_code_with_root(entry, code, root_identity);
    candidate.entry_claims = entry_claims;
    let validated = validate_external_root(candidate, &boundary()).expect("root plan");
    let authority = RootSlotAuthority::from_admitted_owner(
        root_id(slot_identity, RootSlotId::from_normalized_identity),
        root_id(owner_identity, RootSlotOwnerId::from_normalized_identity),
    );
    let execution = provider_execution(&validated);
    let admission = RootAdmission::from_admitted_provider(
        root_id(
            admission_identity,
            RootAdmissionId::from_normalized_identity,
        ),
        &validated,
        &execution,
        code,
        &authority,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("root admission");
    ledger
        .install(code, validated, authority, admission)
        .expect("installed external root")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn install_test_root_pair_with_ids_unsealed<'code>(
    code: &'code mut InstalledCode,
    first: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    second: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    entry: EntryStubId,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    install_test_root_pair_with_ids_unsealed_on_boundary(code, first, second, entry, &boundary())
}

pub(super) fn install_test_root_pair_with_ids_unsealed_on_boundary<'code>(
    code: &'code mut InstalledCode,
    first: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    second: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    entry: EntryStubId,
    boundary: &ValidatedBoundaryEntryPlan,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    let mut first_candidate =
        candidate_for_code_with_root_on_boundary(entry, code, first.0, boundary);
    first_candidate.entry_claims = first.4;
    let mut second_candidate =
        candidate_for_code_with_root_on_boundary(entry, code, second.0, boundary);
    second_candidate.entry_claims = second.4;
    let first_input = first_candidate
        .stack
        .realization
        .input(first_candidate.identity)
        .expect("first root stack input")
        .clone();
    let second_input = second_candidate
        .stack
        .realization
        .input(second_candidate.identity)
        .expect("second root stack input")
        .clone();
    let relation = StackNestingRelation {
        identity: first_candidate.nesting_relation,
        edges: BTreeSet::new(),
    };
    let composition = compose_bound_entry_stack_epochs(&relation, [&first_input, &second_input])
        .expect("artifact-wide two-root stack composition");
    first_candidate.stack.realization = composition.clone();
    second_candidate.stack.realization = composition;

    let first_validated =
        validate_external_root(first_candidate, boundary).expect("first root plan");
    let second_validated =
        validate_external_root(second_candidate, boundary).expect("second root plan");
    let target_profile = target::TargetProfile::UefiX64;
    let target_slot = target_profile.program_entry_slot();
    let first_slot = RootSlotAuthority::for_target_program_entry(target_slot)
        .expect("target program-entry authority");
    let second_slot = RootSlotAuthority::from_admitted_owner(
        root_id(second.1, RootSlotId::from_normalized_identity),
        root_id(second.2, RootSlotOwnerId::from_normalized_identity),
    );
    let first_execution = provider_execution(&first_validated);
    let second_execution = provider_execution(&second_validated);
    let first_admission = RootAdmission::from_admitted_provider(
        root_id(first.3, RootAdmissionId::from_normalized_identity),
        &first_validated,
        &first_execution,
        code,
        &first_slot,
        first_validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("first root admission");
    let second_admission = RootAdmission::from_admitted_provider(
        root_id(second.3, RootAdmissionId::from_normalized_identity),
        &second_validated,
        &second_execution,
        code,
        &second_slot,
        second_validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("second root admission");
    let mut ledger = InstalledRootLedger::claim(code).expect("canonical two-root ledger");
    let first_installed = ledger
        .install(code, first_validated, first_slot, first_admission)
        .expect("first installed root");
    let second_installed = ledger
        .install(code, second_validated, second_slot, second_admission)
        .expect("second installed root");
    (ledger, first_installed, second_installed)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn install_test_root_pair_with_ids<'code>(
    code: &'code mut InstalledCode,
    first: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    second: (u64, u64, u64, u64, Vec<ExternalRootEntryClaim>),
    entry: EntryStubId,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    let (mut ledger, first_installed, second_installed) =
        install_test_root_pair_with_ids_unsealed(code, first, second, entry);
    ledger
        .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
        .expect("installed required root-slot closure");
    (ledger, first_installed, second_installed)
}

pub(super) fn program_local_required_root_slot_closure(
    entry: EntryStubId,
) -> VerifiedRequiredRootSlotClosure {
    let target_profile = target::TargetProfile::UefiX64;
    verify_target_required_root_slot_closure(
        target_profile,
        [TargetRequiredRootSlotSelection::for_program_entry(
            target_profile.program_entry_slot(),
            entry,
            "TestRoot::entry",
        )
        .expect("required program-entry selection")],
    )
    .expect("complete required root-slot closure")
}

pub(super) fn install_program_local_two_parameter_roots<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    let (mut ledger, first, second) = install_test_root_pair_with_ids_unsealed_on_boundary(
        code,
        (1, 20, 21, 22, entry_claims.clone()),
        (101, 120, 121, 122, entry_claims),
        entry,
        &two_parameter_boundary(),
    );
    ledger
        .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
        .expect("installed required root-slot closure");
    (ledger, first, second)
}

pub(super) fn install_program_local_required_root<'code>(
    code: &'code mut InstalledCode,
    entry: EntryStubId,
    entry_claims: Vec<ExternalRootEntryClaim>,
) -> (
    InstalledRootLedger,
    InstalledExternalRoot<'code>,
    InstalledExternalRoot<'code>,
) {
    install_test_root_pair_with_ids(
        code,
        (1, 20, 21, 22, entry_claims.clone()),
        (101, 120, 121, 122, entry_claims),
        entry,
    )
}

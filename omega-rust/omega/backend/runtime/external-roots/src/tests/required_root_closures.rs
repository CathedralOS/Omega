use super::{
    entry_id, install_program_local_required_root, install_test_root,
    install_test_root_pair_with_ids_unsealed, installed_code,
    installed_code_with_fill_and_installation_identity, program_local_claim,
    program_local_required_root_slot_closure, root_id,
};
use crate::{
    ExternalRootId, InstalledRootRemoval, InterruptInvocationId, RootRemovalReceipt,
    RootRemovalReceiptId, TargetRequiredRootSlotSelection,
    verify_target_required_root_slot_closure,
};

#[test]
fn installed_required_root_closure_owns_one_cohort_verifier_and_freezes_members() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (mut root_ledger, required_root, open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);

    let installed_closure = root_ledger
        .required_root_slots()
        .expect("required root-slot closure retained in the installation");
    assert_eq!(installed_closure.slots().len(), 1);
    assert_eq!(
        installed_closure
            .slots()
            .next()
            .expect("required member")
            .root(),
        required_root.root()
    );
    assert!(
        root_ledger
            .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
            .expect_err("installed closure sealing is one-shot")
            .0
            .contains("already sealed")
    );

    let cohort_verifier = root_ledger
        .claim_program_local_root_installation_ledger()
        .expect("one cohort verifier");
    drop(cohort_verifier);
    assert!(
        root_ledger
            .claim_program_local_root_installation_ledger()
            .expect_err("dropping the verifier never reopens issuance")
            .0
            .contains("already issued")
    );

    let open_receipt = RootRemovalReceipt::from_provider(
        root_id(900, RootRemovalReceiptId::from_normalized_identity),
        &open_root,
        true,
        true,
    );
    root_ledger
        .remove(open_root, open_receipt)
        .expect("an unrelated runtime-open root is not frozen");
    let required_receipt = RootRemovalReceipt::from_provider(
        root_id(901, RootRemovalReceiptId::from_normalized_identity),
        &required_root,
        true,
        true,
    );
    let removal = root_ledger
        .remove(required_root, required_receipt)
        .expect_err("a sealed required root remains frozen");
    assert!(
        removal
            .diagnostic()
            .0
            .contains("keeps that installed root frozen")
    );
}

#[test]
fn complete_installed_root_teardown_preserves_history_and_returns_slots_in_caller_order() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (root_ledger, required_root, open_root) =
        install_program_local_required_root(&mut code, entry, vec![program_local_claim()]);
    let required_slot = required_root.slot();
    let open_slot = open_root.slot();
    let required_receipt = RootRemovalReceipt::from_provider(
        root_id(900, RootRemovalReceiptId::from_normalized_identity),
        &required_root,
        true,
        true,
    );
    let open_receipt = RootRemovalReceipt::from_provider(
        root_id(901, RootRemovalReceiptId::from_normalized_identity),
        &open_root,
        true,
        true,
    );

    let (root_ledger, returned_slots) = root_ledger
        .teardown_installed_roots(vec![
            InstalledRootRemoval::new(open_root, open_receipt),
            InstalledRootRemoval::new(required_root, required_receipt),
        ])
        .expect("complete teardown may retire a sealed required-root cohort");

    assert!(root_ledger.live_external_roots_are_empty());
    assert_eq!(returned_slots.len(), 2);
    assert_eq!(returned_slots[0].slot(), open_slot);
    assert_eq!(returned_slots[1].slot(), required_slot);
    assert!(root_ledger.required_root_slots().is_some());
    assert!(root_ledger.binds_installed_code(&code));
    drop(code);
}

#[test]
fn installed_root_teardown_is_atomic_and_returns_exact_rows_for_retry() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (root_ledger, first, second) = install_test_root_pair_with_ids_unsealed(
        &mut code,
        (1, 20, 21, 22, Vec::new()),
        (101, 120, 121, 122, Vec::new()),
        entry,
    );
    let first_receipt = RootRemovalReceipt::from_provider(
        root_id(900, RootRemovalReceiptId::from_normalized_identity),
        &first,
        true,
        true,
    );
    let second_receipt = RootRemovalReceipt::from_provider(
        root_id(901, RootRemovalReceiptId::from_normalized_identity),
        &second,
        true,
        false,
    );
    let error = root_ledger
        .teardown_installed_roots(vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ])
        .expect_err("a late non-quiesced row rejects the whole teardown");
    assert!(error.diagnostic().0.contains("quiesced"));
    let (root_ledger, mut rows) = error.into_parts();
    assert_eq!(root_ledger.records().count(), 2);
    assert!(!root_ledger.live_external_roots_are_empty());
    rows[1].receipt.executions_quiesced = true;

    let (root_ledger, returned_slots) = root_ledger
        .teardown_installed_roots(rows)
        .expect("returned exact custody retries successfully");
    assert!(root_ledger.live_external_roots_are_empty());
    assert_eq!(returned_slots.len(), 2);
}

#[test]
fn installed_root_teardown_preflight_rejects_incomplete_ambiguous_and_stale_state() {
    let entry = entry_id(1);

    {
        let mut code = installed_code(1, entry);
        let (ledger, first, _second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let rows = vec![InstalledRootRemoval::new(first, receipt)];
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("missing row")
                .0
                .contains("completely cover")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        let duplicate = rows[0].root.root;
        rows[1].root.root = duplicate;
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("duplicate row")
                .0
                .contains("duplicate")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        rows[1].root.root = root_id(999, ExternalRootId::from_normalized_identity);
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("extra row")
                .0
                .contains("outside the live ledger")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let other_code = installed_code_with_fill_and_installation_identity(1, entry, 1, 300);
        let (ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        rows[1].root.installed_code = &other_code;
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("wrong full code context")
                .0
                .contains("different installed-code occurrence")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (mut ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            false,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let mut rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("reachable entry")
                .0
                .contains("unreachable")
        );
        ledger.active_interrupts.insert(
            (
                rows[0].root.root,
                root_id(999, InterruptInvocationId::from_normalized_identity),
            ),
            crate::interrupts::interrupt_entries::ActiveInterruptEntry {
                arrival_context: calling_conventions::ArrivalContextId::new(1)
                    .expect("arrival context"),
                stage: calling_conventions::EntryStackStage::Body,
                depth: 1,
                interrupted: None,
                fatal: false,
            },
        );
        rows[0].receipt.entry_unreachable = true;
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("active interrupt")
                .0
                .contains("active interrupt")
        );
    }

    {
        let mut code = installed_code(1, entry);
        let (mut ledger, first, second) = install_test_root_pair_with_ids_unsealed(
            &mut code,
            (1, 20, 21, 22, Vec::new()),
            (101, 120, 121, 122, Vec::new()),
            entry,
        );
        let first_receipt = RootRemovalReceipt::from_provider(
            root_id(900, RootRemovalReceiptId::from_normalized_identity),
            &first,
            true,
            true,
        );
        let second_receipt = RootRemovalReceipt::from_provider(
            root_id(901, RootRemovalReceiptId::from_normalized_identity),
            &second,
            true,
            true,
        );
        let rows = vec![
            InstalledRootRemoval::new(first, first_receipt),
            InstalledRootRemoval::new(second, second_receipt),
        ];
        let stale = ledger
            .root_evidence
            .values()
            .next()
            .expect("live evidence")
            .clone();
        ledger.root_evidence.insert(
            root_id(999, ExternalRootId::from_normalized_identity),
            stale,
        );
        assert!(
            ledger
                .preflight_installed_root_teardown(&rows)
                .expect_err("stale evidence")
                .0
                .contains("exact live root, evidence, and slot state")
        );
    }
}

#[test]
fn failed_installed_required_root_replay_is_transactional() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (mut root_ledger, _required_root, _open_root) = install_test_root_pair_with_ids_unsealed(
        &mut code,
        (1, 20, 21, 22, vec![program_local_claim()]),
        (101, 120, 121, 122, vec![program_local_claim()]),
        entry,
    );
    let profile = target::TargetProfile::UefiX64;
    let wrong = verify_target_required_root_slot_closure(
        profile,
        [TargetRequiredRootSlotSelection::for_program_entry(
            profile.program_entry_slot(),
            entry,
            "OtherRoot::entry",
        )
        .expect("wrong requirement selection remains descriptive")],
    )
    .expect("wrong requirement still forms a target closure");
    assert!(
        root_ledger
            .seal_required_root_slot_closure(wrong)
            .expect_err("installed requirement substitution rejects")
            .0
            .contains("does not match the exact required slot")
    );
    assert!(root_ledger.required_root_slots().is_none());
    root_ledger
        .seal_required_root_slot_closure(program_local_required_root_slot_closure(entry))
        .expect("failed replay leaves exact closure sealable");
}

#[test]
fn program_local_cohort_verifier_requires_an_installed_required_closure() {
    let entry = entry_id(1);
    let mut code = installed_code(1, entry);
    let (mut root_ledger, _root) = install_test_root(&mut code, entry);
    assert!(
        root_ledger
            .claim_program_local_root_installation_ledger()
            .expect_err("an orphan root ledger has no program-local cohort")
            .0
            .contains("requires a sealed required root-slot closure")
    );
}

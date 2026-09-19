use super::{
    EntryStack, EntryStubId, InstalledCode, MachineRegime, MachineRegimeId,
    SecondaryProcessorAccount, SecondaryProcessorOccurrenceId, SecondaryProcessorQuiescenceOutcome,
    SecondaryProcessorQuiescenceReceipt, SecondaryProcessorQuiescenceReceiptId,
    SecondaryProcessorStarted, SecondaryProcessorStartupInvocation,
    SecondaryProcessorStartupInvocationId, SecondaryProcessorStartupLedger,
    SecondaryProcessorStartupOutcome, SecondaryProcessorStartupProfile,
    SecondaryProcessorStartupProfileId, SecondaryProcessorStartupReceipt,
    SecondaryProcessorStartupReceiptId, SecondaryProcessorStartupVerdict,
    bind_secondary_processor_trampoline,
};
use calling_conventions::CallingPolicy;
use layout_plans::{
    ArtifactInstallationScopeId, PlacementAddressRange, PlacementConstraints, PlacementPhase,
};
use target::Architecture;

const STARTUP_ALIGNMENT: u64 = 0x1000;
const LOW_MEMORY_LIMIT: u64 = 0x10_0000;
const TRAMPOLINE_BASE: u64 = 0x8000;
const TRAMPOLINE_LENGTH: u64 = 0x1000;
const TRAMPOLINE_RANGE_END: u64 = LOW_MEMORY_LIMIT;
const WCSU_BYTES: u64 = 0x800;
const WCSU_ALIGNMENT: u64 = 0x100;

fn profile_id(identity: u64) -> SecondaryProcessorStartupProfileId {
    SecondaryProcessorStartupProfileId::from_normalized_identity(identity)
        .expect("normalized startup profile identity")
}

fn processor_id(identity: u64) -> SecondaryProcessorOccurrenceId {
    SecondaryProcessorOccurrenceId::from_normalized_identity(identity)
        .expect("normalized secondary-processor identity")
}

fn invocation_id(identity: u64) -> SecondaryProcessorStartupInvocationId {
    SecondaryProcessorStartupInvocationId::from_normalized_identity(identity)
        .expect("normalized startup invocation identity")
}

fn receipt_id(identity: u64) -> SecondaryProcessorStartupReceiptId {
    SecondaryProcessorStartupReceiptId::from_normalized_identity(identity)
        .expect("normalized startup receipt identity")
}

fn quiescence_receipt_id(identity: u64) -> SecondaryProcessorQuiescenceReceiptId {
    SecondaryProcessorQuiescenceReceiptId::from_normalized_identity(identity)
        .expect("normalized quiescence receipt identity")
}

fn startup_entry() -> EntryStubId {
    EntryStubId::from_normalized_identity(0x700).expect("normalized entry identity")
}

fn arrival_regime() -> MachineRegimeId {
    MachineRegimeId::from_normalized_identity(0x71).expect("normalized machine regime identity")
}

fn startup_profile_on(installed_regime: MachineRegime) -> SecondaryProcessorStartupProfile {
    SecondaryProcessorStartupProfile::new(
        profile_id(700),
        startup_entry(),
        arrival_regime(),
        installed_regime,
        LOW_MEMORY_LIMIT,
        STARTUP_ALIGNMENT,
    )
    .expect("secondary-processor startup profile")
}

fn startup_profile() -> SecondaryProcessorStartupProfile {
    startup_profile_on(MachineRegime::X86Long64)
}

fn trampoline_constraints(
    regime: Option<MachineRegimeId>,
    range_end: u64,
    alignment: u64,
) -> PlacementConstraints {
    PlacementConstraints::new(
        Some(PlacementAddressRange::new(0x1000, range_end).expect("placement range")),
        alignment,
        PlacementPhase::PostHandoff,
        regime,
        Some(
            ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope"),
        ),
    )
    .expect("placement constraints")
}

fn installed_trampoline(
    architecture: Architecture,
    bytes: Vec<u8>,
    constraints: PlacementConstraints,
    extent_base: u64,
    extent_length: u64,
) -> InstalledCode {
    crate::tests::installed_code_in_placement(
        0x600,
        startup_entry(),
        bytes,
        0x601,
        architecture,
        constraints,
        extent_base,
        extent_length,
    )
}

fn installed_x86_trampoline(bytes: Vec<u8>) -> InstalledCode {
    installed_trampoline(
        Architecture::X86_64,
        bytes,
        trampoline_constraints(
            Some(arrival_regime()),
            TRAMPOLINE_RANGE_END,
            STARTUP_ALIGNMENT,
        ),
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    )
}

fn bound_ledger(code: &InstalledCode) -> SecondaryProcessorStartupLedger<'_> {
    SecondaryProcessorStartupLedger::new(
        bind_secondary_processor_trampoline(
            startup_profile(),
            code,
            TRAMPOLINE_BASE,
            TRAMPOLINE_LENGTH,
        )
        .expect("bound secondary-processor trampoline"),
    )
}

fn processor_account(
    processor: u64,
    stack_class: u16,
    state_base: u64,
    state_length: u64,
) -> SecondaryProcessorAccount {
    SecondaryProcessorAccount::new(
        processor_id(processor),
        crate::tests::secondary_processor_boundary(
            CallingPolicy::SystemVAMD64,
            MachineRegime::X86Long64,
            EntryStack::Dedicated { class: stack_class },
        ),
        stack_class,
        WCSU_BYTES,
        WCSU_ALIGNMENT,
        crate::tests::minted_secondary_processor_state(2000 + processor, state_base, state_length),
    )
    .expect("secondary-processor account")
}

#[test]
fn startup_profile_rejects_non_vector_granularity_alignment() {
    for alignment in [0, 0x1800] {
        assert!(
            SecondaryProcessorStartupProfile::new(
                profile_id(701),
                startup_entry(),
                arrival_regime(),
                MachineRegime::X86Long64,
                LOW_MEMORY_LIMIT,
                alignment,
            )
            .is_err(),
            "startup alignment {alignment:#x} is not a nonzero power of two"
        );
    }
}

#[test]
fn startup_profile_rejects_low_memory_bound_off_granularity() {
    for limit in [0, LOW_MEMORY_LIMIT + 0x800] {
        assert!(
            SecondaryProcessorStartupProfile::new(
                profile_id(702),
                startup_entry(),
                arrival_regime(),
                MachineRegime::X86Long64,
                limit,
                STARTUP_ALIGNMENT,
            )
            .is_err(),
            "low-memory bound {limit:#x} is not a nonzero multiple of the startup alignment"
        );
    }
}

#[test]
fn bound_trampoline_replays_installed_geometry_and_retains_placed_bytes() {
    let bytes = vec![0xCC; 96];
    let code = installed_x86_trampoline(bytes.clone());
    let trampoline = bind_secondary_processor_trampoline(
        startup_profile(),
        &code,
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    )
    .expect("bound secondary-processor trampoline");

    assert_eq!(
        trampoline.startup_vector(),
        TRAMPOLINE_BASE / STARTUP_ALIGNMENT
    );
    assert_eq!(trampoline.extent_base(), TRAMPOLINE_BASE);
    assert_eq!(trampoline.extent_length(), TRAMPOLINE_LENGTH);
    assert_eq!(trampoline.profile().startup_entry(), startup_entry());
    assert_eq!(trampoline.profile().arrival_regime(), arrival_regime());

    // The borrow retains the installed occurrence: the exact placed bytes and
    // the declared arrival regime stay observable through installed-code
    // custody instead of a restated address.
    assert_eq!(trampoline.installed_code.identity(), code.identity());
    assert!(
        trampoline
            .installed_code
            .binds_exact_unrelocated_artifact_bytes(&bytes)
    );
    assert!(
        !trampoline
            .installed_code
            .binds_exact_unrelocated_artifact_bytes(&[0x90; 96])
    );
    assert_eq!(
        trampoline
            .installed_code
            .placement_constraints()
            .machine_regime(),
        Some(arrival_regime())
    );
}

#[test]
fn bind_rejects_startup_entry_the_installation_does_not_admit() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let foreign_entry_profile = SecondaryProcessorStartupProfile::new(
        profile_id(703),
        EntryStubId::from_normalized_identity(0x999).expect("normalized entry identity"),
        arrival_regime(),
        MachineRegime::X86Long64,
        LOW_MEMORY_LIMIT,
        STARTUP_ALIGNMENT,
    )
    .expect("secondary-processor startup profile");
    let error = bind_secondary_processor_trampoline(
        foreign_entry_profile,
        &code,
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    )
    .expect_err("unadmitted startup entry must not bind");
    assert!(error.diagnostic().0.contains("does not admit"));
}

#[test]
fn bind_rejects_installed_regime_on_a_foreign_architecture() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let error = bind_secondary_processor_trampoline(
        startup_profile_on(MachineRegime::Aarch64A64 { exception_level: 1 }),
        &code,
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    )
    .expect_err("an aarch64 installed regime must not bind an x86 trampoline");
    assert!(error.diagnostic().0.contains("architecture"));
}

#[test]
fn bind_rejects_drifted_placement_geometry() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    for (base, length) in [
        (TRAMPOLINE_BASE + STARTUP_ALIGNMENT, TRAMPOLINE_LENGTH),
        (TRAMPOLINE_BASE, TRAMPOLINE_LENGTH + STARTUP_ALIGNMENT),
    ] {
        let error = bind_secondary_processor_trampoline(startup_profile(), &code, base, length)
            .expect_err("drifted placement geometry must not bind");
        assert!(error.diagnostic().0.contains("does not match"));
    }
}

#[test]
fn bind_rejects_base_off_the_startup_vector_granularity() {
    let code = installed_trampoline(
        Architecture::X86_64,
        vec![0xCC; 96],
        trampoline_constraints(Some(arrival_regime()), TRAMPOLINE_RANGE_END, 0x100),
        0x8100,
        0x100,
    );
    let error = bind_secondary_processor_trampoline(startup_profile(), &code, 0x8100, 0x100)
        .expect_err("a trampoline base off the startup granularity must not bind");
    assert!(error.diagnostic().0.contains("aligned"));
}

#[test]
fn bind_rejects_extent_overrunning_the_low_memory_bound() {
    let code = installed_trampoline(
        Architecture::X86_64,
        vec![0xCC; 96],
        trampoline_constraints(Some(arrival_regime()), 0x12_0000, STARTUP_ALIGNMENT),
        0xF_F000,
        0x2000,
    );
    let error = bind_secondary_processor_trampoline(startup_profile(), &code, 0xF_F000, 0x2000)
        .expect_err("an extent beyond the low-memory bound must not bind");
    assert!(error.diagnostic().0.contains("low-memory bound"));
}

#[test]
fn bind_rejects_declared_range_overrunning_the_low_memory_bound() {
    let code = installed_trampoline(
        Architecture::X86_64,
        vec![0xCC; 96],
        trampoline_constraints(Some(arrival_regime()), 0x20_0000, STARTUP_ALIGNMENT),
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    );
    let error = bind_secondary_processor_trampoline(
        startup_profile(),
        &code,
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    )
    .expect_err("a declared range beyond the low-memory bound must not bind");
    assert!(error.diagnostic().0.contains("placement range"));
}

#[test]
fn bind_rejects_declared_alignment_below_startup_granularity() {
    let code = installed_trampoline(
        Architecture::X86_64,
        vec![0xCC; 96],
        trampoline_constraints(Some(arrival_regime()), TRAMPOLINE_RANGE_END, 0x800),
        TRAMPOLINE_BASE,
        0x800,
    );
    let error =
        bind_secondary_processor_trampoline(startup_profile(), &code, TRAMPOLINE_BASE, 0x800)
            .expect_err("a declared alignment below the startup granularity must not bind");
    assert!(error.diagnostic().0.contains("granularity"));
}

#[test]
fn bind_rejects_missing_or_foreign_arrival_regime() {
    for regime in [
        None,
        Some(
            MachineRegimeId::from_normalized_identity(0x99)
                .expect("normalized machine regime identity"),
        ),
    ] {
        let code = installed_trampoline(
            Architecture::X86_64,
            vec![0xCC; 96],
            trampoline_constraints(regime, TRAMPOLINE_RANGE_END, STARTUP_ALIGNMENT),
            TRAMPOLINE_BASE,
            TRAMPOLINE_LENGTH,
        );
        let error = bind_secondary_processor_trampoline(
            startup_profile(),
            &code,
            TRAMPOLINE_BASE,
            TRAMPOLINE_LENGTH,
        )
        .expect_err("a missing or foreign arrival regime must not bind");
        assert!(error.diagnostic().0.contains("arrival machine regime"));
    }
}

#[test]
fn admitted_processors_hold_separate_stacks_and_state() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);

    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("first secondary processor admits");
    ledger
        .admit_secondary_processor(processor_account(0x11, 10, 0x3_0000, 0x1000))
        .expect("second secondary processor admits");

    let first = ledger.record(processor_id(0x10)).expect("first record");
    assert_eq!(first.stack_class(), 9);
    assert_eq!(first.wcsu_bytes(), WCSU_BYTES);
    assert_eq!(first.transition().arrival_regime(), arrival_regime());
    assert_eq!(
        first.transition().installed_regime(),
        MachineRegime::X86Long64
    );
    assert!(!first.is_started());
    let second = ledger.record(processor_id(0x11)).expect("second record");
    assert_eq!(second.stack_class(), 10);
    assert_eq!(ledger.records().count(), 2);
}

#[test]
fn admission_rejects_boundary_beginning_outside_the_installed_regime() {
    let code = installed_trampoline(
        Architecture::Aarch64,
        vec![0xCC; 96],
        trampoline_constraints(
            Some(arrival_regime()),
            TRAMPOLINE_RANGE_END,
            STARTUP_ALIGNMENT,
        ),
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    );
    let mut ledger = SecondaryProcessorStartupLedger::new(
        bind_secondary_processor_trampoline(
            startup_profile_on(MachineRegime::Aarch64A64 { exception_level: 2 }),
            &code,
            TRAMPOLINE_BASE,
            TRAMPOLINE_LENGTH,
        )
        .expect("bound secondary-processor trampoline"),
    );

    let mut account = processor_account(0x10, 9, 0x2_0000, 0x1000);
    account.boundary = crate::tests::secondary_processor_boundary(
        CallingPolicy::Aapcs64,
        MachineRegime::Aarch64A64 { exception_level: 1 },
        EntryStack::Dedicated { class: 9 },
    );
    let error = ledger
        .admit_secondary_processor(account)
        .expect_err("an entry beginning outside the installed regime must not admit");
    assert!(error.diagnostic().0.contains("installed machine regime"));
}

#[test]
fn admission_rejects_entry_not_arriving_on_the_accounted_dedicated_stack() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);

    for stack in [
        EntryStack::Interrupted,
        EntryStack::ProviderSelected,
        EntryStack::Dedicated { class: 8 },
    ] {
        let account = SecondaryProcessorAccount::new(
            processor_id(0x10),
            crate::tests::secondary_processor_boundary(
                CallingPolicy::SystemVAMD64,
                MachineRegime::X86Long64,
                stack,
            ),
            9,
            WCSU_BYTES,
            WCSU_ALIGNMENT,
            crate::tests::minted_secondary_processor_state(2100, 0x2_0000, 0x1000),
        )
        .expect("secondary-processor account");
        let error = ledger
            .admit_secondary_processor(account)
            .expect_err("an entry off the accounted stack must not admit");
        assert!(error.diagnostic().0.contains("dedicated stack class"));
    }
}

#[test]
fn admission_rejects_duplicate_shared_or_overlapping_accounts() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("first secondary processor admits");

    let duplicate = ledger
        .admit_secondary_processor(processor_account(0x10, 11, 0x4_0000, 0x1000))
        .expect_err("a repeated processor occurrence must not admit");
    assert!(duplicate.diagnostic().0.contains("already admitted"));

    let shared_class = ledger
        .admit_secondary_processor(processor_account(0x11, 9, 0x4_0000, 0x1000))
        .expect_err("a stack class already accounted to another processor must not admit");
    assert!(shared_class.diagnostic().0.contains("already accounted"));

    let overlapping = ledger
        .admit_secondary_processor(processor_account(0x11, 10, 0x2_0800, 0x1000))
        .expect_err("a state extent overlapping held backing must not admit");
    assert!(overlapping.diagnostic().0.contains("overlaps"));

    let disjoint = ledger
        .admit_secondary_processor(processor_account(0x11, 10, 0x3_0000, 0x1000))
        .expect("a disjoint account admits");
    assert_eq!(disjoint.stack_class(), 10);
}

#[test]
fn account_rejects_state_geometry_that_cannot_host_the_stack() {
    let boundary = || {
        crate::tests::secondary_processor_boundary(
            CallingPolicy::SystemVAMD64,
            MachineRegime::X86Long64,
            EntryStack::Dedicated { class: 9 },
        )
    };
    assert!(
        SecondaryProcessorAccount::new(
            processor_id(0x10),
            boundary(),
            9,
            0,
            WCSU_ALIGNMENT,
            crate::tests::minted_secondary_processor_state(2200, 0x2_0000, 0x1000),
        )
        .is_err(),
        "zero stack demand must not form an account"
    );
    assert!(
        SecondaryProcessorAccount::new(
            processor_id(0x10),
            boundary(),
            9,
            WCSU_BYTES,
            0x180,
            crate::tests::minted_secondary_processor_state(2201, 0x2_0000, 0x1000),
        )
        .is_err(),
        "a non-power-of-two stack alignment must not form an account"
    );
    assert!(
        SecondaryProcessorAccount::new(
            processor_id(0x10),
            boundary(),
            9,
            WCSU_BYTES,
            WCSU_ALIGNMENT,
            crate::tests::minted_secondary_processor_state(2202, 0x2_0000, 0x400),
        )
        .is_err(),
        "state smaller than the stack demand must not form an account"
    );
    assert!(
        SecondaryProcessorAccount::new(
            processor_id(0x10),
            boundary(),
            9,
            WCSU_BYTES,
            WCSU_ALIGNMENT,
            crate::tests::minted_secondary_processor_state(2203, 0x2_0080, 0x1000),
        )
        .is_err(),
        "state off the stack alignment must not form an account"
    );
}

#[test]
fn startup_invocation_binds_the_exact_installed_entry() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");

    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");
    assert_eq!(carrier.invocation(), invocation_id(0x20));
    assert_eq!(carrier.processor(), processor_id(0x10));
    assert_eq!(
        carrier.startup_vector(),
        TRAMPOLINE_BASE / STARTUP_ALIGNMENT
    );
    assert_eq!(carrier.startup_entry(), startup_entry());
    assert_eq!(carrier.transition().arrival_regime(), arrival_regime());
    assert_eq!(
        carrier.transition().installed_regime(),
        MachineRegime::X86Long64
    );
    assert_eq!(carrier.installed_code, code.identity());
    assert_eq!(carrier.installed_code_context, code.receipt_context());
    assert_eq!(carrier.artifact, code.artifact());
}

#[test]
fn begin_rejects_unknown_replayed_or_live_startup_invocations() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");

    let unknown = ledger
        .begin_secondary_processor_startup(processor_id(0x12), invocation_id(0x20))
        .expect_err("an unadmitted processor must not invoke");
    assert!(unknown.diagnostic().0.contains("not admitted"));

    ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");

    let live = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x21))
        .expect_err("an invoked account must not invoke again");
    assert!(live.diagnostic().0.contains("pending"));

    let replayed = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect_err("an issued invocation identity must not replay");
    assert!(replayed.diagnostic().0.contains("already issued"));
}

#[test]
fn refused_startup_returns_the_account_to_pending_custody_for_retry() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");

    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");
    let refusal_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x30),
        &carrier,
        SecondaryProcessorStartupVerdict::DefiniteNondispatch,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, refusal_receipt)
        .expect("refused startup completes");
    let SecondaryProcessorStartupOutcome::Refused(refusal) = outcome else {
        panic!("a refused receipt must not mark the processor started");
    };
    assert_eq!(refusal.processor(), processor_id(0x10));
    assert_eq!(refusal.receipt(), receipt_id(0x30));
    assert!(
        !ledger
            .record(processor_id(0x10))
            .expect("record")
            .is_started()
    );

    // The account returned to pending custody: a fresh invocation retries
    // without re-binding the trampoline or re-accounting the stack.
    let retry = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x21))
        .expect("retried startup invocation issues");
    let retry_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x31),
        &retry,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(retry, retry_receipt)
        .expect("retried startup completes");
    assert!(matches!(
        outcome,
        SecondaryProcessorStartupOutcome::Started(_)
    ));
}

#[test]
fn unconfirmed_dispatch_holds_the_account_invoked_until_a_definitive_receipt() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");

    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");
    let unconfirmed = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x30),
        &carrier,
        SecondaryProcessorStartupVerdict::DispatchUnconfirmed,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, unconfirmed)
        .expect("unconfirmed startup completes");
    let SecondaryProcessorStartupOutcome::Unconfirmed(unresolved) = outcome else {
        panic!("an unconfirmed dispatch must not resolve the account");
    };
    assert_eq!(unresolved.processor(), processor_id(0x10));
    assert_eq!(unresolved.invocation(), invocation_id(0x20));
    assert_eq!(unresolved.receipt(), receipt_id(0x30));

    // An outstanding carrier may still be dispatching: the account is
    // neither withdrawable nor open to a fresh invocation.
    let withdrawal = ledger
        .withdraw_secondary_processor(processor_id(0x10))
        .expect_err("an unconfirmed dispatch must not release the account");
    assert!(withdrawal.diagnostic().0.contains("pending"));
    let reissue = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x21))
        .expect_err("an unconfirmed dispatch must not open a fresh invocation");
    assert!(reissue.diagnostic().0.contains("pending"));

    // The returned carrier still answers: a confirmed arrival on the same
    // outstanding invocation marks the processor started.
    let carrier = unresolved.into_carrier();
    let definitive = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x31),
        &carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, definitive)
        .expect("definitive receipt completes the outstanding attempt");
    assert!(matches!(
        outcome,
        SecondaryProcessorStartupOutcome::Started(_)
    ));
    assert!(
        ledger
            .record(processor_id(0x10))
            .expect("record")
            .is_started()
    );
}

#[test]
fn unconfirmed_dispatch_resolved_to_nondispatch_returns_pending_custody() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");

    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");
    let unconfirmed = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x30),
        &carrier,
        SecondaryProcessorStartupVerdict::DispatchUnconfirmed,
    );
    let SecondaryProcessorStartupOutcome::Unconfirmed(unresolved) = ledger
        .complete_secondary_processor_startup(carrier, unconfirmed)
        .expect("unconfirmed startup completes")
    else {
        panic!("an unconfirmed dispatch must not resolve the account");
    };

    // A later definite nondispatch answers the outstanding carrier: only
    // then does the account return to pending, withdrawable custody.
    let carrier = unresolved.into_carrier();
    let definitive = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x31),
        &carrier,
        SecondaryProcessorStartupVerdict::DefiniteNondispatch,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, definitive)
        .expect("definitive nondispatch completes the outstanding attempt");
    assert!(matches!(
        outcome,
        SecondaryProcessorStartupOutcome::Refused(_)
    ));
    let withdrawal = ledger
        .withdraw_secondary_processor(processor_id(0x10))
        .expect("a confirmed nondispatch permits withdrawal");
    assert_eq!(withdrawal.processor(), processor_id(0x10));
}

#[test]
fn started_processor_holds_its_account_until_a_quiescence_edge() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");

    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");
    let started_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x30),
        &carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, started_receipt)
        .expect("started startup completes");
    let SecondaryProcessorStartupOutcome::Started(started) = outcome else {
        panic!("a started receipt must mint started evidence");
    };
    assert_eq!(started.processor(), processor_id(0x10));
    assert_eq!(started.invocation(), invocation_id(0x20));
    assert_eq!(started.receipt(), receipt_id(0x30));
    assert_eq!(
        started.startup_vector(),
        TRAMPOLINE_BASE / STARTUP_ALIGNMENT
    );
    assert_eq!(started.startup_entry(), startup_entry());
    assert!(
        ledger
            .record(processor_id(0x10))
            .expect("record")
            .is_started()
    );

    let reinvoke = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x21))
        .expect_err("a started processor must not invoke again");
    assert!(reinvoke.diagnostic().0.contains("pending"));
    let withdrawal = ledger
        .withdraw_secondary_processor(processor_id(0x10))
        .expect_err("a started processor's account stays held");
    assert!(withdrawal.diagnostic().0.contains("pending"));
}

#[test]
fn completion_rejects_drifted_foreign_or_replayed_receipts() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");
    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");

    // A carrier naming drifted vector geometry is foreign even when it cites
    // the same installed occurrence.
    let drifted = SecondaryProcessorStartupInvocation {
        invocation: carrier.invocation,
        processor: carrier.processor,
        startup_vector: carrier.startup_vector + 1,
        startup_entry: carrier.startup_entry,
        transition: carrier.transition,
        installed_code: carrier.installed_code,
        installed_code_context: carrier.installed_code_context.clone(),
        artifact: carrier.artifact,
    };
    let drifted_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x30),
        &drifted,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let error = ledger
        .complete_secondary_processor_startup(drifted, drifted_receipt)
        .expect_err("a drifted carrier must not complete");
    assert!(error.diagnostic().0.contains("foreign, stale, or drifted"));

    // A carrier minted under a different installed occurrence is foreign.
    let foreign_code = crate::tests::installed_code_in_placement(
        0x610,
        startup_entry(),
        vec![0xCC; 96],
        0x611,
        Architecture::X86_64,
        trampoline_constraints(
            Some(arrival_regime()),
            TRAMPOLINE_RANGE_END,
            STARTUP_ALIGNMENT,
        ),
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    );
    let foreign = SecondaryProcessorStartupInvocation {
        invocation: carrier.invocation,
        processor: carrier.processor,
        startup_vector: carrier.startup_vector,
        startup_entry: carrier.startup_entry,
        transition: carrier.transition,
        installed_code: foreign_code.identity(),
        installed_code_context: foreign_code.receipt_context(),
        artifact: foreign_code.artifact(),
    };
    let foreign_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x31),
        &foreign,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let error = ledger
        .complete_secondary_processor_startup(foreign, foreign_receipt)
        .expect_err("a foreign carrier must not complete");
    assert!(error.diagnostic().0.contains("foreign, stale, or drifted"));

    // A receipt answering a different invocation does not bind the carrier.
    let mut mismatched = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x32),
        &carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    mismatched.invocation = invocation_id(0x99);
    let error = ledger
        .complete_secondary_processor_startup(carrier, mismatched)
        .expect_err("a receipt naming another invocation must not complete");
    assert!(error.diagnostic().0.contains("does not bind"));
    let (carrier, _) = error.into_parts();

    // The exact receipt completes once; a replayed completion is reordered.
    let transition = ledger
        .record(processor_id(0x10))
        .expect("record")
        .transition();
    let exact_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x33),
        &carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, exact_receipt)
        .expect("exact startup completion");
    assert!(matches!(
        outcome,
        SecondaryProcessorStartupOutcome::Started(_)
    ));
    let replayed_carrier = SecondaryProcessorStartupInvocation {
        invocation: invocation_id(0x20),
        processor: processor_id(0x10),
        startup_vector: TRAMPOLINE_BASE / STARTUP_ALIGNMENT,
        startup_entry: startup_entry(),
        transition,
        installed_code: code.identity(),
        installed_code_context: code.receipt_context(),
        artifact: code.artifact(),
    };
    let replayed_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x34),
        &replayed_carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let replayed = ledger
        .complete_secondary_processor_startup(replayed_carrier, replayed_receipt)
        .expect_err("a replayed completion must not land");
    assert!(replayed.diagnostic().0.contains("reordered or replayed"));
}

#[test]
fn withdrawal_returns_the_complete_never_invoked_account() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");
    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x10), invocation_id(0x20))
        .expect("startup invocation issues");
    ledger
        .admit_secondary_processor(processor_account(0x11, 10, 0x3_0000, 0x1000))
        .expect("second secondary processor admits");

    let invoked = ledger
        .withdraw_secondary_processor(processor_id(0x10))
        .expect_err("an outstanding invocation keeps the account held");
    assert!(invoked.diagnostic().0.contains("pending"));
    let unknown = ledger
        .withdraw_secondary_processor(processor_id(0x12))
        .expect_err("an unadmitted processor has nothing to withdraw");
    assert!(unknown.diagnostic().0.contains("not admitted"));

    let withdrawal = ledger
        .withdraw_secondary_processor(processor_id(0x11))
        .expect("never-invoked processor withdraws");
    assert_eq!(withdrawal.processor(), processor_id(0x11));
    let (boundary, stack_class, wcsu_bytes, wcsu_alignment, state, transition) =
        withdrawal.into_parts();
    assert_eq!(stack_class, 10);
    assert_eq!(wcsu_bytes, WCSU_BYTES);
    assert_eq!(wcsu_alignment, WCSU_ALIGNMENT);
    assert_eq!(state.base(), 0x3_0000);
    assert_eq!(state.length(), 0x1000);
    assert_eq!(transition.arrival_regime(), arrival_regime());
    assert_eq!(
        boundary.plan().state.stack,
        EntryStack::Dedicated { class: 10 }
    );
    assert!(ledger.record(processor_id(0x11)).is_none());
    assert_eq!(ledger.records().count(), 1);

    // The withdrawn carrier on the first processor can still complete: its
    // invocation stays outstanding until a receipt answers it.
    let landed_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x30),
        &carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, landed_receipt)
        .expect("the outstanding invocation still completes");
    assert!(matches!(
        outcome,
        SecondaryProcessorStartupOutcome::Started(_)
    ));
}

/// Drive one admitted processor through issuance and a started receipt,
/// returning the minted started evidence the quiescence edge consumes.
fn start_processor(
    ledger: &mut SecondaryProcessorStartupLedger<'_>,
    processor: u64,
    invocation: u64,
    receipt: u64,
) -> SecondaryProcessorStarted {
    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(processor), invocation_id(invocation))
        .expect("startup invocation issues");
    let started_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(receipt),
        &carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    match ledger
        .complete_secondary_processor_startup(carrier, started_receipt)
        .expect("started startup completes")
    {
        SecondaryProcessorStartupOutcome::Started(started) => started,
        _ => panic!("a started receipt must mint started evidence"),
    }
}

#[test]
fn retirement_returns_the_exact_started_account_and_frees_its_resources() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("first secondary processor admits");
    ledger
        .admit_secondary_processor(processor_account(0x11, 10, 0x3_0000, 0x1000))
        .expect("second secondary processor admits");
    let started = start_processor(&mut ledger, 0x10, 0x20, 0x30);
    start_processor(&mut ledger, 0x11, 0x21, 0x31);

    let quiescent = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x40),
        &started,
        true,
    );
    assert!(quiescent.quiescent());
    let outcome = ledger
        .retire_secondary_processor(started, quiescent)
        .expect("a quiescent started processor retires");
    let SecondaryProcessorQuiescenceOutcome::Retired(retirement) = outcome else {
        panic!("a quiescent receipt must retire the account");
    };
    assert_eq!(retirement.processor(), processor_id(0x10));
    assert_eq!(retirement.invocation(), invocation_id(0x20));
    assert_eq!(retirement.startup_receipt(), receipt_id(0x30));
    assert_eq!(retirement.quiescence_receipt(), quiescence_receipt_id(0x40));
    let (boundary, stack_class, wcsu_bytes, wcsu_alignment, state, transition) =
        retirement.into_parts();
    assert_eq!(stack_class, 9);
    assert_eq!(wcsu_bytes, WCSU_BYTES);
    assert_eq!(wcsu_alignment, WCSU_ALIGNMENT);
    assert_eq!(state.base(), 0x2_0000);
    assert_eq!(state.length(), 0x1000);
    assert_eq!(transition.arrival_regime(), arrival_regime());
    assert_eq!(transition.installed_regime(), MachineRegime::X86Long64);
    assert_eq!(
        boundary.plan().state.stack,
        EntryStack::Dedicated { class: 9 }
    );

    // Only the retired account left the ledger; the other started processor
    // keeps its hold untouched.
    assert!(ledger.record(processor_id(0x10)).is_none());
    assert!(
        ledger
            .record(processor_id(0x11))
            .expect("second record")
            .is_started()
    );
    assert_eq!(ledger.records().count(), 1);

    // Custody actually returned: the retired stack class and state backing
    // are accountable to a fresh processor again.
    let readmitted = ledger
        .admit_secondary_processor(processor_account(0x12, 9, 0x2_0000, 0x1000))
        .expect("the retired stack class and state backing admit again");
    assert_eq!(readmitted.stack_class(), 9);
    assert!(!readmitted.is_started());
}

#[test]
fn quiescence_refusal_keeps_the_started_account_held_for_retry() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");
    let started = start_processor(&mut ledger, 0x10, 0x20, 0x30);
    let expected = started.clone();

    let incomplete_drain = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x40),
        &started,
        false,
    );
    let outcome = ledger
        .retire_secondary_processor(started, incomplete_drain)
        .expect("an accepted receipt without quiescence is held, not rejected");
    let SecondaryProcessorQuiescenceOutcome::Held(refusal) = outcome else {
        panic!("an incomplete drain must not retire the account");
    };
    assert_eq!(refusal.processor(), processor_id(0x10));
    assert_eq!(refusal.receipt(), quiescence_receipt_id(0x40));

    // The account stays exactly as it was: started, unwithdrawable, and the
    // stack class and state backing still accounted.
    assert!(
        ledger
            .record(processor_id(0x10))
            .expect("record")
            .is_started()
    );
    let withdrawal = ledger
        .withdraw_secondary_processor(processor_id(0x10))
        .expect_err("a held processor's account stays unwithdrawable");
    assert!(withdrawal.diagnostic().0.contains("pending"));
    let shared_class = ledger
        .admit_secondary_processor(processor_account(0x11, 9, 0x3_0000, 0x1000))
        .expect_err("a held stack class is still accounted");
    assert!(shared_class.diagnostic().0.contains("already accounted"));

    // The returned started evidence is the exact one minted at startup, and
    // a later quiescent receipt retires on it.
    let started = refusal.into_started();
    assert_eq!(started, expected);
    let quiescent = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x41),
        &started,
        true,
    );
    let outcome = ledger
        .retire_secondary_processor(started, quiescent)
        .expect("the retried quiescence receipt retires");
    assert!(matches!(
        outcome,
        SecondaryProcessorQuiescenceOutcome::Retired(_)
    ));
    assert!(ledger.record(processor_id(0x10)).is_none());
}

#[test]
fn retirement_rejects_never_started_accounts() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("pending secondary processor admits");
    ledger
        .admit_secondary_processor(processor_account(0x11, 10, 0x3_0000, 0x1000))
        .expect("invoked secondary processor admits");
    let carrier = ledger
        .begin_secondary_processor_startup(processor_id(0x11), invocation_id(0x21))
        .expect("startup invocation issues");

    // Forged started evidence citing this ledger's exact trampoline still
    // cannot retire an account the provider never acknowledged as started.
    let pending_transition = ledger
        .record(processor_id(0x10))
        .expect("pending record")
        .transition();
    let invoked_transition = ledger
        .record(processor_id(0x11))
        .expect("invoked record")
        .transition();
    let forged = |processor: u64, invocation: u64, transition| SecondaryProcessorStarted {
        processor: processor_id(processor),
        invocation: invocation_id(invocation),
        receipt: receipt_id(0x30),
        startup_vector: TRAMPOLINE_BASE / STARTUP_ALIGNMENT,
        startup_entry: startup_entry(),
        transition,
        installed_code: code.identity(),
        installed_code_context: code.receipt_context(),
        artifact: code.artifact(),
    };

    let pending = forged(0x10, 0x20, pending_transition);
    let pending_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x40),
        &pending,
        true,
    );
    let error = ledger
        .retire_secondary_processor(pending, pending_receipt)
        .expect_err("a pending account must not retire");
    assert!(error.diagnostic().0.contains("requires a started account"));

    // An outstanding invocation is not a start: the provider has not
    // acknowledged it, so there is nothing quiescence could release.
    let invoked = forged(0x11, 0x21, invoked_transition);
    let invoked_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x41),
        &invoked,
        true,
    );
    let error = ledger
        .retire_secondary_processor(invoked, invoked_receipt)
        .expect_err("an invoked account must not retire");
    assert!(error.diagnostic().0.contains("requires a started account"));

    let unknown = forged(0x12, 0x22, pending_transition);
    let unknown_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x42),
        &unknown,
        true,
    );
    let error = ledger
        .retire_secondary_processor(unknown, unknown_receipt)
        .expect_err("an unadmitted processor has nothing to retire");
    assert!(error.diagnostic().0.contains("names no admitted processor"));

    // Nothing moved: both accounts remain, and the outstanding invocation
    // still lands on its own receipt.
    assert_eq!(ledger.records().count(), 2);
    let landed_receipt = SecondaryProcessorStartupReceipt::from_provider(
        receipt_id(0x31),
        &carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let outcome = ledger
        .complete_secondary_processor_startup(carrier, landed_receipt)
        .expect("the outstanding invocation still completes");
    assert!(matches!(
        outcome,
        SecondaryProcessorStartupOutcome::Started(_)
    ));
}

#[test]
fn retirement_rejects_stale_foreign_or_replayed_started_evidence() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");
    let started = start_processor(&mut ledger, 0x10, 0x20, 0x30);

    // Started evidence naming drifted vector geometry is foreign even when
    // it cites the same installed occurrence.
    let mut drifted = started.clone();
    drifted.startup_vector += 1;
    let drifted_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x40),
        &drifted,
        true,
    );
    let error = ledger
        .retire_secondary_processor(drifted, drifted_receipt)
        .expect_err("drifted started evidence must not retire");
    assert!(error.diagnostic().0.contains("foreign, stale, or drifted"));

    // Started evidence minted under a different installed occurrence is
    // foreign.
    let foreign_code = crate::tests::installed_code_in_placement(
        0x610,
        startup_entry(),
        vec![0xCC; 96],
        0x611,
        Architecture::X86_64,
        trampoline_constraints(
            Some(arrival_regime()),
            TRAMPOLINE_RANGE_END,
            STARTUP_ALIGNMENT,
        ),
        TRAMPOLINE_BASE,
        TRAMPOLINE_LENGTH,
    );
    let mut foreign = started.clone();
    foreign.installed_code = foreign_code.identity();
    foreign.installed_code_context = foreign_code.receipt_context();
    foreign.artifact = foreign_code.artifact();
    let foreign_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x41),
        &foreign,
        true,
    );
    let error = ledger
        .retire_secondary_processor(foreign, foreign_receipt)
        .expect_err("foreign started evidence must not retire");
    assert!(error.diagnostic().0.contains("foreign, stale, or drifted"));

    // Every rejection returned both inputs and left the account held.
    let (returned, _) = error.into_parts();
    assert_eq!(returned.installed_code, foreign_code.identity());
    assert!(
        ledger
            .record(processor_id(0x10))
            .expect("record")
            .is_started()
    );

    // The exact evidence retires once; replaying it afterwards finds no
    // account.
    let quiescent = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x42),
        &started,
        true,
    );
    let outcome = ledger
        .retire_secondary_processor(started.clone(), quiescent)
        .expect("exact started evidence retires");
    assert!(matches!(
        outcome,
        SecondaryProcessorQuiescenceOutcome::Retired(_)
    ));
    let replayed_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x43),
        &started,
        true,
    );
    let error = ledger
        .retire_secondary_processor(started.clone(), replayed_receipt)
        .expect_err("a replayed retirement must not land");
    assert!(error.diagnostic().0.contains("names no admitted processor"));

    // Once the same processor is admitted and started again, the earlier
    // startup's evidence is stale against the account's current startup.
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("the retired processor admits again");
    let restarted = start_processor(&mut ledger, 0x10, 0x21, 0x31);
    let stale_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x44),
        &started,
        true,
    );
    let error = ledger
        .retire_secondary_processor(started, stale_receipt)
        .expect_err("stale started evidence must not retire the re-admitted account");
    assert!(error.diagnostic().0.contains("current startup"));
    assert!(
        ledger
            .record(processor_id(0x10))
            .expect("re-admitted record")
            .is_started()
    );
    let current_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x45),
        &restarted,
        true,
    );
    let outcome = ledger
        .retire_secondary_processor(restarted, current_receipt)
        .expect("the current startup's evidence retires");
    assert!(matches!(
        outcome,
        SecondaryProcessorQuiescenceOutcome::Retired(_)
    ));
    assert_eq!(ledger.records().count(), 0);
}

#[test]
fn retirement_rejects_receipts_off_the_exact_started_evidence() {
    let code = installed_x86_trampoline(vec![0xCC; 96]);
    let mut ledger = bound_ledger(&code);
    ledger
        .admit_secondary_processor(processor_account(0x10, 9, 0x2_0000, 0x1000))
        .expect("secondary processor admits");
    let started = start_processor(&mut ledger, 0x10, 0x20, 0x30);

    let mut other_invocation = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x40),
        &started,
        true,
    );
    other_invocation.invocation = invocation_id(0x99);
    let mut other_processor = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x41),
        &started,
        true,
    );
    other_processor.processor = processor_id(0x11);
    let mut other_startup_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x42),
        &started,
        true,
    );
    other_startup_receipt.startup_receipt = receipt_id(0x99);
    let mut other_vector = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x43),
        &started,
        true,
    );
    other_vector.startup_vector += 1;

    let mut started = started;
    for (receipt, label) in [
        (other_invocation, "another invocation"),
        (other_processor, "another processor"),
        (other_startup_receipt, "another startup receipt"),
        (other_vector, "another vector"),
    ] {
        let Err(error) = ledger.retire_secondary_processor(started, receipt) else {
            panic!("a receipt naming {label} must not retire");
        };
        assert!(
            error.diagnostic().0.contains("does not bind"),
            "{label}: {}",
            error.diagnostic().0
        );
        let (returned, _) = error.into_parts();
        started = returned;
        assert!(
            ledger
                .record(processor_id(0x10))
                .expect("record")
                .is_started(),
            "{label} left the account held"
        );
    }

    let exact = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x44),
        &started,
        true,
    );
    let outcome = ledger
        .retire_secondary_processor(started, exact)
        .expect("the exact receipt retires");
    assert!(matches!(
        outcome,
        SecondaryProcessorQuiescenceOutcome::Retired(_)
    ));
}

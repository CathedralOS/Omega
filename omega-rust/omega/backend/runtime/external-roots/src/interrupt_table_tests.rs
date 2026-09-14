use super::*;
use crate::{
    InterruptAcknowledgementReceipt, InterruptAcknowledgementReceiptId, RootAdmission,
    RootAdmissionId, RootRemovalReceipt, RootRemovalReceiptId, RootSlotAuthority, RootSlotOwnerId,
    validate_external_root,
};
use calling_conventions::{X86_64GateKind, X86_64InstalledInterruptStack};
use executable_installation::{ArtifactEntry, DestinationPreparationReceipt, InstalledCode};
use extents::{
    ExtentRights, MappedExtent, MappingGrant, MappingGrantId, MappingId, MappingSourceMode,
    TranslationActivationReceipt, TranslationInstallObligations, TranslationReleaseObligations,
    map_owned,
};
use layout_plans::{
    ArtifactInstallationScopeId, MachineRegimeId, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, PlacementSite,
};
use target::Architecture;

const DIVIDE_ERROR: u8 = 0;
const GENERAL_PROTECTION: u8 = 13;
const PAGE_FAULT: u8 = 14;
const TIMER_TICK: u8 = 0x20;

fn profile_id(identity: u64) -> InterruptTableProfileId {
    InterruptTableProfileId::from_normalized_identity(identity)
        .expect("normalized interrupt-table profile identity")
}

fn establishment_id(identity: u64) -> InterruptTableEstablishmentId {
    InterruptTableEstablishmentId::from_normalized_identity(identity)
        .expect("normalized interrupt-table establishment identity")
}

fn authority_id(identity: u64) -> InterruptTablePublicationAuthorityId {
    InterruptTablePublicationAuthorityId::from_normalized_identity(identity)
        .expect("normalized interrupt-table publication authority identity")
}

fn publication_id(identity: u64) -> InterruptTablePublicationId {
    InterruptTablePublicationId::from_normalized_identity(identity)
        .expect("normalized interrupt-table publication identity")
}

fn publication_receipt_id(identity: u64) -> InterruptTablePublicationReceiptId {
    InterruptTablePublicationReceiptId::from_normalized_identity(identity)
        .expect("normalized interrupt-table publication receipt identity")
}

/// One declared member's gate descriptor: the consumer's code selector 0x08,
/// ring-0 privilege, and the IST slot its dedicated critical stack arrives
/// through. Fatal exceptions run on trap gates; the timer runs on an
/// interrupt gate so arrivals mask further interrupts until it settles.
fn gate_descriptor(gate: X86_64GateKind, ist: Option<u8>) -> InterruptTableGateDescriptor {
    InterruptTableGateDescriptor {
        gate,
        selector: 0x08,
        entry_privilege: 0,
        interrupt_stack_table_slot: ist,
    }
}

fn fatal_member(vector: u8, stack_class: u16, ist: u8) -> InterruptTableMemberPlan {
    InterruptTableMemberPlan {
        vector,
        dedicated_stack_class: stack_class,
        obligation: InterruptTableObligation::FatalException,
        descriptor: gate_descriptor(X86_64GateKind::Trap, Some(ist)),
    }
}

fn timer_member(vector: u8, stack_class: u16, ist: u8) -> InterruptTableMemberPlan {
    InterruptTableMemberPlan {
        vector,
        dedicated_stack_class: stack_class,
        obligation: InterruptTableObligation::AcknowledgedInterrupt,
        descriptor: gate_descriptor(X86_64GateKind::Interrupt, Some(ist)),
    }
}

/// The declared board-item table: every fatal exception entry on its own
/// critical stack class plus the minimal timer root on a dedicated class.
/// Each member's declared IST slot resolves its class in `declared_tss`.
fn table_profile(identity: u64) -> InterruptTableProfile {
    InterruptTableProfile::new(
        profile_id(identity),
        [
            fatal_member(DIVIDE_ERROR, 11, 1),
            fatal_member(GENERAL_PROTECTION, 12, 2),
            fatal_member(PAGE_FAULT, 13, 3),
            timer_member(TIMER_TICK, 14, 4),
        ],
    )
    .expect("interrupt-table profile")
}

/// The installed TSS the descriptor validation joins through: IST slot i
/// provisions dedicated critical stack class 10+i for the fixture members.
fn declared_tss() -> calling_conventions::X86_64InstalledTaskStateSegmentRealization {
    calling_conventions::X86_64InstalledTaskStateSegmentRealization {
        privilege_stacks: Vec::new(),
        interrupt_stacks: [1, 2, 3, 4]
            .into_iter()
            .map(|slot| X86_64InstalledInterruptStack {
                slot,
                dedicated_class: 10 + u16::from(slot),
            })
            .collect(),
    }
}

/// One installed member row for `interrupt_table_candidates`.
fn member(
    root_identity: u64,
    entry: u64,
    stack_class: u16,
    acknowledged: bool,
) -> crate::tests::InterruptTableMemberFixture {
    crate::tests::InterruptTableMemberFixture {
        root_identity,
        entry: EntryStubId::from_normalized_identity(entry).expect("entry identity"),
        stack_class,
        acknowledged,
    }
}

/// The four declared table members.
fn member_fixtures() -> Vec<crate::tests::InterruptTableMemberFixture> {
    vec![
        member(0x101, 0x201, 11, false),
        member(0x102, 0x202, 12, false),
        member(0x103, 0x203, 13, false),
        member(0x104, 0x204, 14, true),
    ]
}

/// The declared members plus a spare installed row the profile does not
/// declare — a handle for duplicate/undeclared admission controls.
fn member_fixtures_with_spare() -> Vec<crate::tests::InterruptTableMemberFixture> {
    let mut members = member_fixtures();
    members.push(member(0x105, 0x205, 15, true));
    members
}

fn member_vectors() -> [u8; 4] {
    [DIVIDE_ERROR, GENERAL_PROTECTION, PAGE_FAULT, TIMER_TICK]
}

fn table_installed_code(
    artifact_identity: u64,
    installed_code_identity: u64,
    members: &[crate::tests::InterruptTableMemberFixture],
) -> InstalledCode {
    let entries = members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            ArtifactEntry::from_canonical_decode(member.entry, 16 * (index as u64 + 1))
        })
        .collect();
    crate::tests::installed_code_in_placement_with_entries(
        artifact_identity,
        vec![0; 16 * (members.len() + 1)],
        installed_code_identity,
        Architecture::X86_64,
        PlacementConstraints::new(
            Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
            4096,
            PlacementPhase::PostHandoff,
            Some(MachineRegimeId::from_normalized_identity(0x71).expect("machine regime")),
            Some(
                ArtifactInstallationScopeId::from_normalized_identity(61)
                    .expect("installation scope"),
            ),
        )
        .expect("placement constraints"),
        0x1000,
        4096,
        entries,
    )
}

/// Install every fixture member into one ledger, returning the retained
/// linear handles in fixture order.
fn install_members<'code>(
    ledger: &mut InstalledRootLedger,
    code: &'code InstalledCode,
    members: &[crate::tests::InterruptTableMemberFixture],
) -> Vec<InstalledExternalRoot<'code>> {
    crate::tests::interrupt_table_candidates(code, members)
        .into_iter()
        .enumerate()
        .map(|(index, (root, _boundary))| {
            let authority = RootSlotAuthority::from_admitted_owner(
                crate::tests::root_id(0x300 + index as u64, RootSlotId::from_normalized_identity),
                crate::tests::root_id(0x21, RootSlotOwnerId::from_normalized_identity),
            );
            let execution = crate::tests::provider_execution_for(&root, 0x400 + index as u64);
            let admission = RootAdmission::from_admitted_provider(
                crate::tests::root_id(
                    0x500 + index as u64,
                    RootAdmissionId::from_normalized_identity,
                ),
                &root,
                &execution,
                code,
                &authority,
                root.candidate().trust_receipts.iter().copied(),
            )
            .expect("root admission");
            ledger
                .install(code, root, authority, admission)
                .expect("installed interrupt-table member")
        })
        .collect()
}

fn table_destination(seed: u64, base: u64, length: u64) -> Extent {
    crate::tests::minted_secondary_processor_state(seed, base, length)
}

/// The consumer-established table value describing exactly the ledger's
/// admitted member set on `code`.
fn established_table(
    identity: u64,
    profile: &InterruptTableProfile,
    code_identity: InstalledCodeId,
    artifact: ArtifactId,
    table: &InterruptTableLedger<'_>,
    destination: Extent,
) -> EstablishedInterruptTable {
    let rows = table
        .members()
        .map(|member| {
            (
                member.vector(),
                InterruptTableEstablishedMember {
                    root: member.root().root(),
                    entry: member.entry(),
                },
            )
        })
        .collect::<Vec<_>>();
    EstablishedInterruptTable::from_consumer(
        establishment_id(identity),
        profile,
        code_identity,
        artifact,
        rows,
        destination,
    )
    .expect("established interrupt table")
}

struct AdmittedTable<'code> {
    ledger: InstalledRootLedger,
    table: InterruptTableLedger<'code>,
    profile: InterruptTableProfile,
    _code: &'code InstalledCode,
}

/// Drive the complete admitted state: four declared members installed and
/// admitted under one installed-code occurrence, with a spare installed row
/// left outside the table.
fn admitted_table() -> AdmittedTable<'static> {
    let members = member_fixtures();
    let code = Box::leak(Box::new(table_installed_code(1, 300, &members)));
    let mut ledger = InstalledRootLedger::claim(code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, code, &members);
    let profile = table_profile(0x600);
    let mut table = InterruptTableLedger::new(profile.clone(), &ledger);
    for (vector, handle) in member_vectors().into_iter().zip(handles) {
        table
            .admit_interrupt_table_member(&ledger, vector, handle)
            .expect("admitted interrupt-table member");
    }
    AdmittedTable {
        ledger,
        table,
        profile,
        _code: code,
    }
}

fn established_for(admitted: &AdmittedTable<'_>, identity: u64) -> EstablishedInterruptTable {
    established_table(
        identity,
        &admitted.profile,
        admitted.ledger.installed_code(),
        admitted.ledger.artifact(),
        &admitted.table,
        table_destination(0x900 + identity, 0x8_0000, 0x1000),
    )
}

#[test]
fn interrupt_table_profile_rejects_an_empty_member_set() {
    let error = InterruptTableProfile::new(profile_id(1), Vec::new())
        .expect_err("an empty member set cannot describe a table");
    assert!(error.0.contains("no member vectors"));
}

#[test]
fn interrupt_table_profile_rejects_repeated_vector_declarations() {
    let error = InterruptTableProfile::new(
        profile_id(1),
        [
            fatal_member(PAGE_FAULT, 11, 1),
            fatal_member(PAGE_FAULT, 12, 2),
        ],
    )
    .expect_err("one vector cannot be declared twice");
    assert!(error.0.contains("more than once"));
}

#[test]
fn interrupt_table_profile_rejects_shared_dedicated_stack_classes() {
    let error = InterruptTableProfile::new(
        profile_id(1),
        [
            fatal_member(DIVIDE_ERROR, 11, 1),
            fatal_member(GENERAL_PROTECTION, 11, 2),
            timer_member(TIMER_TICK, 14, 3),
        ],
    )
    .expect_err("two members cannot share one critical stack class");
    assert!(error.0.contains("more than one vector"));
}

#[test]
fn interrupt_table_member_admission_rejects_undeclared_vectors() {
    let members = member_fixtures_with_spare();
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
    let mut handles = handles.into_iter();
    let spare = handles.nth(4).expect("spare member handle");
    let error = table
        .admit_interrupt_table_member(&ledger, 0x2e, spare)
        .expect_err("an undeclared vector cannot enter the table");
    assert!(error.diagnostic().0.contains("declares no member"));
    let _ = error.into_root();
}

#[test]
fn interrupt_table_member_admission_rejects_duplicate_vectors() {
    let members = member_fixtures_with_spare();
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
    let mut handles = handles.into_iter();
    for vector in member_vectors() {
        table
            .admit_interrupt_table_member(&ledger, vector, handles.next().expect("member handle"))
            .expect("admitted interrupt-table member");
    }
    let spare = handles.next().expect("spare member handle");
    let error = table
        .admit_interrupt_table_member(&ledger, TIMER_TICK, spare)
        .expect_err("an occupied vector cannot be re-admitted");
    assert!(error.diagnostic().0.contains("already admitted"));
    let _ = error.into_root();
}

#[test]
fn interrupt_table_member_admission_requires_the_declared_critical_stack_class() {
    let mut members = member_fixtures();
    // The page-fault handler's boundary arrives on a different dedicated class
    // than the profile declares for that vector.
    members[2].stack_class = 99;
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
    let mut handles = handles.into_iter();
    for (vector, handle) in member_vectors().into_iter().zip(&mut handles) {
        if vector == PAGE_FAULT {
            let error = table
                .admit_interrupt_table_member(&ledger, vector, handle)
                .expect_err("the declared critical stack class is exact");
            assert!(
                error
                    .diagnostic()
                    .0
                    .contains("dedicated critical stack class")
            );
            let _ = error.into_root();
        } else {
            table
                .admit_interrupt_table_member(&ledger, vector, handle)
                .expect("admitted interrupt-table member");
        }
    }
}

#[test]
fn interrupt_table_member_admission_rejects_non_dedicated_stacks() {
    let stack = EntryStack::Interrupted;
    {
        let members = member_fixtures();
        let mut code = table_installed_code(1, 300, &members);
        let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
        let entry = members[0].entry;
        let mut candidate = crate::tests::interrupt_candidate_shaped(entry, &code, 0x101, false);
        let boundary = crate::tests::interrupt_boundary_on(stack);
        candidate.stack.realization = crate::tests::stack_demand(
            candidate.identity,
            candidate.provider,
            candidate.nesting_relation,
            &boundary,
            &code,
            entry,
            stack,
            2048,
        );
        let validated = validate_external_root(candidate, &boundary).expect("root plan");
        let authority = RootSlotAuthority::from_admitted_owner(
            crate::tests::root_id(0x300, RootSlotId::from_normalized_identity),
            crate::tests::root_id(0x21, RootSlotOwnerId::from_normalized_identity),
        );
        let execution = crate::tests::provider_execution_for(&validated, 0x400);
        let admission = RootAdmission::from_admitted_provider(
            crate::tests::root_id(0x500, RootAdmissionId::from_normalized_identity),
            &validated,
            &execution,
            &code,
            &authority,
            validated.candidate().trust_receipts.iter().copied(),
        )
        .expect("root admission");
        let handle = ledger
            .install(&code, validated, authority, admission)
            .expect("installed member");
        let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
        let error = table
            .admit_interrupt_table_member(&ledger, DIVIDE_ERROR, handle)
            .expect_err("a non-dedicated stack is not a critical stack");
        assert!(
            error
                .diagnostic()
                .0
                .contains("dedicated critical stack class")
        );
        let _ = error.into_root();
    }
}

#[test]
fn interrupt_table_member_obligations_require_the_declared_acknowledgement_shape() {
    // A timer member whose root mints no acknowledgement cannot satisfy the
    // acknowledged-interrupt obligation.
    let mut members = member_fixtures();
    members[3].acknowledged = false;
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
    let mut handles = handles.into_iter();
    for (vector, handle) in member_vectors().into_iter().zip(&mut handles) {
        if vector == TIMER_TICK {
            let error = table
                .admit_interrupt_table_member(&ledger, vector, handle)
                .expect_err("the timer member must mint a settle-able acknowledgement");
            assert!(error.diagnostic().0.contains("acknowledgement contract"));
            let _ = error.into_root();
        } else {
            table
                .admit_interrupt_table_member(&ledger, vector, handle)
                .expect("admitted interrupt-table member");
        }
    }
    drop(table);
    drop(ledger);

    // And a fatal exception entry must not mint a settle-able
    // acknowledgement: a terminal fault handler has no controller debt.
    let mut members = member_fixtures();
    members[0].acknowledged = true;
    let mut code = table_installed_code(2, 301, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
    let mut handles = handles.into_iter();
    let error = table
        .admit_interrupt_table_member(
            &ledger,
            DIVIDE_ERROR,
            handles.next().expect("member handle"),
        )
        .expect_err("a fatal exception entry carries no acknowledgement obligation");
    assert!(error.diagnostic().0.contains("acknowledgement contract"));
    let _ = error.into_root();
}

#[test]
fn interrupt_table_member_admission_rejects_roots_outside_the_ledger() {
    let members = member_fixtures();
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let _handles = install_members(&mut ledger, &code, &members);

    let mut foreign_code = table_installed_code(2, 301, &members);
    let mut foreign_ledger =
        InstalledRootLedger::claim(&mut foreign_code).expect("foreign root ledger");
    let foreign_handles = install_members(&mut foreign_ledger, &foreign_code, &members);

    let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
    let mut foreign_handles = foreign_handles.into_iter();

    // Admitting through the foreign ledger rejects at the occurrence check.
    let error = table
        .admit_interrupt_table_member(
            &foreign_ledger,
            DIVIDE_ERROR,
            foreign_handles.next().expect("foreign handle"),
        )
        .expect_err("a foreign ledger cannot admit into this table");
    assert!(
        error
            .diagnostic()
            .0
            .contains("different installed-code occurrence")
    );
    let _ = error.into_root();

    // A foreign handle presented against our ledger collides on the same
    // root identity but its evidence names another installed occurrence.
    let error = table
        .admit_interrupt_table_member(
            &ledger,
            DIVIDE_ERROR,
            foreign_handles.next().expect("foreign handle"),
        )
        .expect_err("a foreign member handle is not this ledger's retained root");
    assert!(
        error
            .diagnostic()
            .0
            .contains("does not bind the ledger's exact retained root")
    );
    let _ = error.into_root();
}

#[test]
fn interrupt_table_publication_requires_the_complete_declared_set() {
    let members = member_fixtures();
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let profile = table_profile(0x600);
    let mut table = InterruptTableLedger::new(profile.clone(), &ledger);
    let mut handles = handles.into_iter();
    for (vector, handle) in member_vectors().into_iter().zip(&mut handles) {
        if vector != TIMER_TICK {
            table
                .admit_interrupt_table_member(&ledger, vector, handle)
                .expect("admitted interrupt-table member");
        }
    }
    assert!(!table.is_complete());

    let established = established_table(
        0x610,
        &profile,
        code.identity(),
        code.artifact(),
        &table,
        table_destination(0x910, 0x8_0000, 0x1000),
    );
    let error = table
        .begin_interrupt_table_publication(
            &ledger,
            established,
            publication_id(0x620),
            authority_id(0x630),
        )
        .expect_err("publication cannot issue over a missing fatal entry");
    assert!(
        error
            .diagnostic()
            .0
            .contains("complete declared member set")
    );
    let _ = error.into_established();
}

#[test]
fn interrupt_table_publication_rejects_a_mismatched_established_set() {
    let mut admitted = admitted_table();
    let code_identity = admitted.ledger.installed_code();
    let artifact = admitted.ledger.artifact();

    // A swapped member row: the established table claims a different root at
    // the timer vector than the admitted custody.
    let wrong_rows = admitted
        .table
        .members()
        .map(|member| {
            (
                member.vector(),
                InterruptTableEstablishedMember {
                    root: if member.vector() == TIMER_TICK {
                        crate::tests::root_id(0x777, ExternalRootId::from_normalized_identity)
                    } else {
                        member.root().root()
                    },
                    entry: member.entry(),
                },
            )
        })
        .collect::<Vec<_>>();
    let established = EstablishedInterruptTable::from_consumer(
        establishment_id(0x611),
        &admitted.profile,
        code_identity,
        artifact,
        wrong_rows,
        table_destination(0x911, 0x8_0000, 0x1000),
    )
    .expect("established interrupt table");
    let error = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x621),
            authority_id(0x631),
        )
        .expect_err("an established table naming foreign roots cannot publish");
    assert!(error.diagnostic().0.contains("member rows"));
    let _ = error.into_established();

    // A missing member row rejects the same way.
    let short_rows = admitted
        .table
        .members()
        .filter(|member| member.vector() != TIMER_TICK)
        .map(|member| {
            (
                member.vector(),
                InterruptTableEstablishedMember {
                    root: member.root().root(),
                    entry: member.entry(),
                },
            )
        })
        .collect::<Vec<_>>();
    let established = EstablishedInterruptTable::from_consumer(
        establishment_id(0x612),
        &admitted.profile,
        code_identity,
        artifact,
        short_rows,
        table_destination(0x912, 0x8_0000, 0x1000),
    )
    .expect("established interrupt table");
    let error = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x622),
            authority_id(0x632),
        )
        .expect_err("an established table missing the timer row cannot publish");
    assert!(error.diagnostic().0.contains("member rows"));
    let _ = error.into_established();

    // An established value claiming another installed occurrence also rejects.
    let member_rows = admitted
        .table
        .members()
        .map(|member| {
            (
                member.vector(),
                InterruptTableEstablishedMember {
                    root: member.root().root(),
                    entry: member.entry(),
                },
            )
        })
        .collect::<Vec<_>>();
    let foreign = EstablishedInterruptTable::from_consumer(
        establishment_id(0x613),
        &admitted.profile,
        InstalledCodeId::from_normalized_identity(0x999).expect("installed-code identity"),
        artifact,
        member_rows,
        table_destination(0x913, 0x8_0000, 0x1000),
    )
    .expect("established interrupt table");
    let error = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            foreign,
            publication_id(0x623),
            authority_id(0x633),
        )
        .expect_err("an established table under another occurrence cannot publish");
    assert!(error.diagnostic().0.contains("installed realization"));
    let _ = error.into_established();
}

#[test]
fn interrupt_table_publication_refusal_returns_the_established_value_for_retry() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x614);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x624),
            authority_id(0x634),
        )
        .expect("issued publication carrier");

    let refusal_receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x640),
        &carrier,
        false,
    );
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, refusal_receipt)
        .expect("refusal consumes the exact carrier");
    let InterruptTablePublicationOutcome::Refused(refusal) = outcome else {
        panic!("a false receipt must refuse the publication")
    };
    assert_eq!(refusal.publication(), publication_id(0x624));
    assert_eq!(refusal.receipt(), publication_receipt_id(0x640));
    let established = refusal.into_established();
    assert_eq!(established.establishment(), establishment_id(0x614));

    // Admission-phase custody is restored: the same established value retries
    // under a fresh publication identity and publishes.
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x625),
            authority_id(0x634),
        )
        .expect("a refused attempt leaves admission-phase custody");
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x641),
        &carrier,
        true,
    );
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the retried carrier publishes");
    let InterruptTablePublicationOutcome::Published(published) = outcome else {
        panic!("the exact retry receipt publishes the table")
    };
    assert_eq!(published.publication(), publication_id(0x625));
    assert_eq!(published.receipt(), publication_receipt_id(0x641));
    assert_eq!(published.establishment(), establishment_id(0x614));
    assert_eq!(published.members().len(), 4);
}

#[test]
fn interrupt_table_publication_rejects_foreign_and_replayed_carriers() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x615);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x626),
            authority_id(0x635),
        )
        .expect("issued publication carrier");

    // A receipt naming a different publication than the carrier rejects, and
    // custody of both returns.
    let mismatched = InterruptTablePublicationReceipt {
        publication: publication_id(0xbeef),
        ..InterruptTablePublicationReceipt::from_provider(
            publication_receipt_id(0x642),
            &carrier,
            true,
        )
    };
    let error = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, mismatched)
        .expect_err("a receipt must name the exact issued carrier");
    assert!(error.diagnostic().0.contains("exact issued carrier"));
    let (carrier, _mismatched) = error.into_parts();

    // A carrier this ledger never issued is foreign.
    let foreign = InterruptTablePublication {
        publication: publication_id(0xdead),
        authority: authority_id(0x635),
        established: established_for(&admitted, 0x616),
    };
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x643),
        &carrier,
        true,
    );
    let error = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, foreign, receipt)
        .expect_err("a carrier this ledger never issued is foreign");
    assert!(
        error
            .diagnostic()
            .0
            .contains("foreign, replayed, or out of order")
    );
    let _ = error.into_parts();

    // After a refusal, replaying the consumed publication identity still
    // cannot issue a second carrier for it.
    let refusal = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x644),
        &carrier,
        false,
    );
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, refusal)
        .expect("refusal consumes the exact carrier");
    let InterruptTablePublicationOutcome::Refused(refusal) = outcome else {
        panic!("a false receipt must refuse the publication")
    };
    let established = refusal.into_established();
    let error = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x626),
            authority_id(0x635),
        )
        .expect_err("a consumed publication identity cannot be replayed");
    assert!(error.diagnostic().0.contains("already issued"));
    let _ = error.into_established();
}

#[test]
fn published_timer_member_enters_and_settles_its_acknowledgement() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x617);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x627),
            authority_id(0x636),
        )
        .expect("issued publication carrier");
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x645),
        &carrier,
        true,
    );
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the exact receipt publishes the table");
    let InterruptTablePublicationOutcome::Published(published) = outcome else {
        panic!("the exact receipt publishes the table")
    };
    assert_eq!(published.members().len(), 4);
    assert_eq!(
        published
            .members()
            .find(|(vector, _)| *vector == TIMER_TICK)
            .expect("published timer member")
            .1
            .entry,
        EntryStubId::from_normalized_identity(0x204).expect("entry identity")
    );

    // The published timer member's retained handle still enters through the
    // ordinary interrupt-entry path: the provider receipt mints the Pending
    // acknowledgement and the settled exit completes it.
    let timer = admitted
        .table
        .member(TIMER_TICK)
        .expect("admitted timer member");
    let obligations = admitted
        .ledger
        .begin_interrupt_entry(
            timer.root(),
            crate::tests::interrupt_entry_receipt(timer.root(), 90, Some(7), Some(91)),
        )
        .expect("admitted timer entry");
    let (pending, control, acknowledgement) = obligations.into_parts();
    let acknowledgement =
        acknowledgement.expect("the timer entry mints its Pending acknowledgement");
    let acknowledgement_receipt = InterruptAcknowledgementReceipt::from_provider(
        crate::tests::root_id(
            99,
            InterruptAcknowledgementReceiptId::from_normalized_identity,
        ),
        &acknowledgement,
        "InterruptCompletion::complete",
    )
    .expect("exact installed completion route");
    let completed = acknowledgement
        .complete(acknowledgement_receipt)
        .expect("settled acknowledgement");
    let completed = admitted
        .ledger
        .finish_interrupt_entry(pending, control, Some(completed))
        .expect("settled timer exit");
    assert_eq!(completed.root, timer.root().root());
}

#[test]
fn closing_the_table_returns_every_member_handle() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x618);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x628),
            authority_id(0x637),
        )
        .expect("issued publication carrier");
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x646),
        &carrier,
        true,
    );
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the exact receipt publishes the table");
    assert!(matches!(
        outcome,
        InterruptTablePublicationOutcome::Published(_)
    ));

    let closed = admitted.table.close();
    assert_eq!(
        closed.publication(),
        Some((publication_id(0x628), publication_receipt_id(0x646)))
    );
    let roots = closed.into_member_roots();
    assert_eq!(roots.len(), 4);
    for root in roots {
        let receipt = RootRemovalReceipt::from_provider(
            crate::tests::root_id(0x700, RootRemovalReceiptId::from_normalized_identity),
            &root,
            true,
            true,
        );
        admitted
            .ledger
            .remove(root, receipt)
            .expect("returned member handles release through ordinary removal");
    }
    assert!(admitted.ledger.live_external_roots_are_empty());
}

// ---------------------------------------------------------------------
// Checked publication-instruction provider edge (x86-64 `lidt` contract).
// ---------------------------------------------------------------------

/// Consumer-issued authority token with an explicit scope declaration.
fn publication_authority(
    identity: u64,
    code_identity: InstalledCodeId,
    artifact: ArtifactId,
    scopes: &[InterruptTablePublicationScope],
) -> InterruptTablePublicationAuthority {
    InterruptTablePublicationAuthority::from_consumer(
        authority_id(identity),
        code_identity,
        artifact,
        scopes.iter().copied(),
    )
    .expect("publication authority")
}

/// The complete authority the checked edge requires: both scope legs bound
/// to the carrier's installed realization.
fn full_publication_authority(
    identity: u64,
    code_identity: InstalledCodeId,
    artifact: ArtifactId,
) -> InterruptTablePublicationAuthority {
    publication_authority(
        identity,
        code_identity,
        artifact,
        &[
            InterruptTablePublicationScope::ProcessorTableControl,
            InterruptTablePublicationScope::TablePublication,
        ],
    )
}

/// An operand read site minted outside the shared fixture address space
/// (50): `minted_secondary_processor_state` only provisions space 50, and
/// the edge must reject reads staged in a foreign space.
fn foreign_space_site(seed: u64, base: u64, length: u64) -> Extent {
    let offset = seed * 16;
    extents::ExtentRootGrant::from_admitted_provider(
        extents::ExtentProviderIssuance::from_normalized_identities([
            offset + 1,
            offset + 2,
            offset + 3,
            offset + 4,
            offset + 5,
            offset + 6,
            offset + 7,
            offset + 8,
            offset + 9,
            offset + 10,
            offset + 11,
            offset + 12,
            offset + 13,
        ])
        .expect("normalized provider issuance"),
        extents::ExtentLineageId::from_normalized_identity(seed + 1000).expect("lineage identity"),
        extents::AddressSpaceId::from_normalized_identity(77).expect("foreign address space"),
        extents::ExtentRights::from_normalized_identities([
            extents::ExtentRightId::from_normalized_identity(51).expect("extent right"),
        ]),
        extents::ExtentProvenanceId::from_normalized_identity(seed + 2000).expect("provenance"),
        extents::MappingEraId::from_normalized_identity(seed + 3000).expect("mapping era"),
    )
    .mint(base, length)
    .expect("foreign-space operand site")
}

/// The declared pseudo-descriptor operand naming `destination` exactly: a
/// separate 10-byte read site in the shared fixture address space.
fn descriptor_operand(seed: u64, destination: &Extent) -> InterruptTableDescriptorOperand {
    InterruptTableDescriptorOperand::from_provider(
        crate::tests::minted_secondary_processor_state(
            seed,
            0x9_0000,
            INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES,
        ),
        u16::try_from(destination.length() - 1).expect("pseudo-descriptor limit"),
        destination.base(),
    )
}

/// The carrier's exercised authority token for `admitted`'s installed
/// realization.
fn carrier_authority(
    admitted: &AdmittedTable<'_>,
    identity: u64,
) -> InterruptTablePublicationAuthority {
    full_publication_authority(
        identity,
        admitted.ledger.installed_code(),
        admitted.ledger.artifact(),
    )
}

#[test]
fn publication_authority_rejects_an_empty_scope_declaration() {
    let error = InterruptTablePublicationAuthority::from_consumer(
        authority_id(0x640),
        InstalledCodeId::from_normalized_identity(300).expect("installed-code identity"),
        ArtifactId::from_normalized_identity(1).expect("artifact identity"),
        [],
    )
    .expect_err("an authority declaring no scope can never answer a carrier");
    assert!(error.0.contains("no scope"));
}

#[test]
fn checked_publication_publishes_the_table_under_exact_authority_and_operand() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x620);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x630),
            authority_id(0x640),
        )
        .expect("issued publication carrier");
    let authority = carrier_authority(&admitted, 0x640);
    let operand = descriptor_operand(0x650, carrier.destination());
    let answer = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x660),
            true,
        )
        .expect("the exact authority and operand publish the table");

    // The provider answer accounts for the contract: the operand read is
    // retained, the `lidt` staging register clobber is recorded, and the
    // installed register state names exactly the published destination.
    assert_eq!(answer.scratch_clobber(), MachineRegister::X86R10);
    assert!(answer.is_published());
    assert!(answer.receipt().published());
    assert_eq!(answer.operand().site().base(), 0x9_0000);
    let state = answer.installed_state().expect("published register state");
    assert_eq!(state.publication(), publication_id(0x630));
    assert_eq!(state.base(), 0x8_0000);
    assert_eq!(state.limit(), 0x0fff);

    let (carrier, receipt, _operand, _state) = answer.into_parts();
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the minted receipt completes the carrier");
    let InterruptTablePublicationOutcome::Published(published) = outcome else {
        panic!("the checked provider answer publishes the table")
    };
    assert_eq!(published.publication(), publication_id(0x630));
    assert_eq!(published.receipt(), publication_receipt_id(0x660));
    assert_eq!(published.members().len(), 4);
}

#[test]
fn checked_publication_rejects_an_unbound_authority_identity() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x621);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x631),
            authority_id(0x641),
        )
        .expect("issued publication carrier");

    // The exercised token names a different authority identity.
    let authority = carrier_authority(&admitted, 0x6ff);
    let operand = descriptor_operand(0x651, carrier.destination());
    let error = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x661),
            true,
        )
        .expect_err("an unbound authority cannot answer the carrier");
    assert!(error.diagnostic().0.contains("bound publication authority"));

    // Carrier and operand custody survive for a corrected attempt.
    let (carrier, operand) = error.into_parts();
    let authority = carrier_authority(&admitted, 0x641);
    let answer = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x662),
            true,
        )
        .expect("the bound authority answers the retained carrier");
    let (carrier, receipt, _operand, _state) = answer.into_parts();
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the corrected answer completes");
    assert!(matches!(
        outcome,
        InterruptTablePublicationOutcome::Published(_)
    ));
}

#[test]
fn checked_publication_rejects_authority_scoped_to_a_foreign_realization() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x622);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x632),
            authority_id(0x642),
        )
        .expect("issued publication carrier");

    // The bound identity matches but the token's realization scope does not.
    let authority = full_publication_authority(
        0x642,
        InstalledCodeId::from_normalized_identity(0x999).expect("foreign installed code"),
        admitted.ledger.artifact(),
    );
    let operand = descriptor_operand(0x652, carrier.destination());
    let error = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x663),
            true,
        )
        .expect_err("a foreign-realization authority cannot answer the carrier");
    assert!(
        error
            .diagnostic()
            .0
            .contains("different installed realization")
    );
    let _ = error.into_parts();
}

#[test]
fn checked_publication_requires_both_authority_scope_legs() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x623);
    let mut carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x633),
            authority_id(0x643),
        )
        .expect("issued publication carrier");
    let mut operand = descriptor_operand(0x653, carrier.destination());

    for scopes in [
        &[InterruptTablePublicationScope::ProcessorTableControl][..],
        &[InterruptTablePublicationScope::TablePublication][..],
    ] {
        let authority = publication_authority(
            0x643,
            admitted.ledger.installed_code(),
            admitted.ledger.artifact(),
            scopes,
        );
        let error = carrier
            .execute_checked_publication(
                admitted._code,
                &authority,
                operand,
                publication_receipt_id(0x664),
                true,
            )
            .expect_err("a single-scope authority cannot publish the table");
        assert!(error.diagnostic().0.contains("does not declare both"));
        let parts = error.into_parts();
        carrier = parts.0;
        operand = parts.1;
    }
    let _ = (carrier, operand);
}

#[test]
fn checked_publication_replays_the_operand_against_the_established_destination() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x624);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x634),
            authority_id(0x644),
        )
        .expect("issued publication carrier");
    let authority = carrier_authority(&admitted, 0x644);

    // A pseudo-descriptor naming a different base cannot publish this table.
    let operand = InterruptTableDescriptorOperand::from_provider(
        crate::tests::minted_secondary_processor_state(
            0x654,
            0x9_0000,
            INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES,
        ),
        0x0fff,
        0x8_0800,
    );
    let error = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x665),
            true,
        )
        .expect_err("a descriptor naming another base cannot publish");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact established destination")
    );
    let (carrier, _operand) = error.into_parts();

    // A pseudo-descriptor naming a different limit rejects the same way.
    let operand = InterruptTableDescriptorOperand::from_provider(
        crate::tests::minted_secondary_processor_state(
            0x655,
            0x9_0000,
            INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES,
        ),
        0x0ffe,
        0x8_0000,
    );
    let error = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x666),
            true,
        )
        .expect_err("a descriptor naming another limit cannot publish");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact established destination")
    );
    let (carrier, _operand) = error.into_parts();

    // The accounted read site must be exactly the 10-byte pseudo-descriptor.
    let operand = InterruptTableDescriptorOperand::from_provider(
        crate::tests::minted_secondary_processor_state(0x656, 0x9_0000, 9),
        0x0fff,
        0x8_0000,
    );
    let error = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x667),
            true,
        )
        .expect_err("a mis-sized operand read cannot publish");
    assert!(error.diagnostic().0.contains("10-byte pseudo-descriptor"));
    let (carrier, _operand) = error.into_parts();

    // The accounted read site must live in the published table's own
    // address space.
    let operand = InterruptTableDescriptorOperand::from_provider(
        foreign_space_site(0x657, 0x9_0000, INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES),
        0x0fff,
        0x8_0000,
    );
    let error = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x668),
            true,
        )
        .expect_err("a foreign-space operand read cannot publish");
    assert!(
        error
            .diagnostic()
            .0
            .contains("outside the published table's address space")
    );
    let (carrier, _operand) = error.into_parts();

    // The accounted operand read must not alias the published table bytes.
    let operand = InterruptTableDescriptorOperand::from_provider(
        crate::tests::minted_secondary_processor_state(
            0x658,
            0x8_0ff8,
            INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES,
        ),
        0x0fff,
        0x8_0000,
    );
    let error = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x669),
            true,
        )
        .expect_err("an operand read aliasing the table cannot publish");
    assert!(error.diagnostic().0.contains("must not alias"));
    let _ = error.into_parts();
}

#[test]
fn checked_publication_rejects_a_foreign_installed_realization() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x625);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x635),
            authority_id(0x645),
        )
        .expect("issued publication carrier");
    let authority = carrier_authority(&admitted, 0x645);
    let operand = descriptor_operand(0x655, carrier.destination());

    // A different installed-code occurrence cannot answer this carrier even
    // under a correctly bound authority.
    let foreign_code = table_installed_code(2, 0x999, &member_fixtures());
    let error = carrier
        .execute_checked_publication(
            &foreign_code,
            &authority,
            operand,
            publication_receipt_id(0x66a),
            true,
        )
        .expect_err("a foreign installed realization cannot answer the carrier");
    assert!(
        error
            .diagnostic()
            .0
            .contains("different installed realization")
    );
    let _ = error.into_parts();
}

#[test]
fn checked_publication_rejects_a_foreign_architecture_realization() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x626);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x636),
            authority_id(0x646),
        )
        .expect("issued publication carrier");
    let authority = carrier_authority(&admitted, 0x646);
    let operand = descriptor_operand(0x656, carrier.destination());

    // The identity/artifact pair matches the carrier's binding; only the
    // target architecture is foreign.
    let members = member_fixtures();
    let foreign_arch = {
        let entries = members
            .iter()
            .enumerate()
            .map(|(index, member)| {
                ArtifactEntry::from_canonical_decode(member.entry, 16 * (index as u64 + 1))
            })
            .collect();
        crate::tests::installed_code_in_placement_with_entries(
            1,
            vec![0; 16 * (members.len() + 1)],
            300,
            Architecture::Aarch64,
            PlacementConstraints::new(
                Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
                4096,
                PlacementPhase::PostHandoff,
                Some(MachineRegimeId::from_normalized_identity(0x71).expect("machine regime")),
                Some(
                    ArtifactInstallationScopeId::from_normalized_identity(61)
                        .expect("installation scope"),
                ),
            )
            .expect("placement constraints"),
            0x1000,
            4096,
            entries,
        )
    };
    let error = carrier
        .execute_checked_publication(
            &foreign_arch,
            &authority,
            operand,
            publication_receipt_id(0x66b),
            true,
        )
        .expect_err("the x86-64 contract cannot publish a foreign-architecture table");
    assert!(error.diagnostic().0.contains("foreign-architecture"));
    let _ = error.into_parts();
}

#[test]
fn checked_publication_decline_returns_the_established_value_for_retry() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x627);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x637),
            authority_id(0x647),
        )
        .expect("issued publication carrier");
    let authority = carrier_authority(&admitted, 0x647);
    let operand = descriptor_operand(0x657, carrier.destination());

    // A declined attempt installs no register state but still mints a
    // receipt naming this exact carrier and authority.
    let answer = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x66c),
            false,
        )
        .expect("a declined attempt still answers under the checks");
    assert!(!answer.is_published());
    assert!(answer.installed_state().is_none());
    assert!(!answer.receipt().published());
    let (carrier, receipt, _operand, _state) = answer.into_parts();
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the declined receipt refuses the carrier");
    let InterruptTablePublicationOutcome::Refused(refusal) = outcome else {
        panic!("a declined answer must refuse the publication")
    };
    let established = refusal.into_established();
    assert_eq!(established.establishment(), establishment_id(0x627));

    // Admission-phase custody is restored: the same established value
    // retries under a fresh publication identity and publishes through the
    // checked edge.
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x638),
            authority_id(0x648),
        )
        .expect("a refused attempt leaves admission-phase custody");
    let authority = carrier_authority(&admitted, 0x648);
    let operand = descriptor_operand(0x658, carrier.destination());
    let answer = carrier
        .execute_checked_publication(
            admitted._code,
            &authority,
            operand,
            publication_receipt_id(0x66d),
            true,
        )
        .expect("the retried carrier publishes");
    let (carrier, receipt, _operand, _state) = answer.into_parts();
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the retried receipt completes");
    let InterruptTablePublicationOutcome::Published(published) = outcome else {
        panic!("the retried carrier publishes the table")
    };
    assert_eq!(published.publication(), publication_id(0x638));
    assert_eq!(published.establishment(), establishment_id(0x627));
}

// ---------------------------------------------------------------------
// Descriptor-table byte materialization and consumer gate/selector/IST
// validation.
// ---------------------------------------------------------------------

const TABLE_BASE: u64 = 0x8_0000;
const TABLE_BYTES: u64 = 528; // vectors 0..=0x20 at 16 bytes each

fn member_slot_base(vector: u8) -> usize {
    usize::from(vector) * X86_64_GATE_DESCRIPTOR_BYTES as usize
}

/// An activated owned mapping for a descriptor-table destination, mirroring
/// the executable-installation writer fixture: minted source and
/// destination extents in the shared fixture space, provider activation
/// receipt, and mapped rights that the preparation receipt then requires.
fn activated_table_mapping(seed: u64, base: u64, length: u64) -> MappedExtent<'static> {
    let rights = |identity: u64| {
        ExtentRights::from_normalized_identities([crate::tests::extent_id(
            identity,
            extents::ExtentRightId::from_normalized_identity,
        )])
    };
    let mint = |seed: u64, extent_base: u64| {
        extents::ExtentRootGrant::from_admitted_provider(
            crate::tests::extent_provider_issuance(seed),
            crate::tests::extent_id(
                seed + 1000,
                extents::ExtentLineageId::from_normalized_identity,
            ),
            crate::tests::extent_id(50, extents::AddressSpaceId::from_normalized_identity),
            rights(51),
            crate::tests::extent_id(
                seed + 2000,
                extents::ExtentProvenanceId::from_normalized_identity,
            ),
            crate::tests::extent_id(seed + 3000, extents::MappingEraId::from_normalized_identity),
        )
        .mint(extent_base, length)
        .expect("table mapping extent")
    };
    let activation = crate::tests::extent_id(
        seed + 4000,
        extents::TranslationActivationFactId::from_normalized_identity,
    );
    let grant = MappingGrant::from_admitted_provider(
        crate::tests::extent_id(seed + 5000, MappingGrantId::from_normalized_identity),
        MappingSourceMode::Owned,
        crate::tests::extent_id(50, extents::AddressSpaceId::from_normalized_identity),
        crate::tests::extent_id(50, extents::AddressSpaceId::from_normalized_identity),
        rights(51),
        rights(51),
        rights(51),
        crate::tests::extent_id(
            seed + 6000,
            extents::ExtentProvenanceId::from_normalized_identity,
        ),
        crate::tests::extent_id(seed + 7000, extents::MappingEraId::from_normalized_identity),
        TranslationInstallObligations::from_normalized_facts([activation]),
        TranslationReleaseObligations::default(),
    );
    let pending = map_owned(
        mint(seed + 8000, 0x20_0000),
        mint(seed + 9000, base),
        crate::tests::extent_id(seed + 10_000, MappingId::from_normalized_identity),
        &grant,
    )
    .expect("table pending mapping");
    let receipt = TranslationActivationReceipt::from_admitted_provider(
        &pending.receipt_context(),
        true,
        [activation],
    );
    pending.complete(receipt).expect("activated table mapping")
}

/// The provider's prepared destination: the activated mapping, its
/// pinned/writable/unpublished preparation receipt, and the staged image as
/// the concrete byte view.
fn prepared_table_destination(
    seed: u64,
    base: u64,
    image: Vec<u8>,
) -> executable_installation::PreparedPostHandoffWriterDestination<'static, 'static> {
    let length = u64::try_from(image.len()).expect("image length");
    let mapping = activated_table_mapping(seed, base, length);
    let receipt = DestinationPreparationReceipt::from_admitted_provider(
        executable_installation::DestinationPreparationReceiptId::from_normalized_identity(
            seed + 11_000,
        )
        .expect("normalized destination-preparation receipt identity"),
        &mapping.receipt_context(),
        ExtentRights::from_normalized_identities([crate::tests::extent_id(
            51,
            extents::ExtentRightId::from_normalized_identity,
        )]),
        true,
        true,
    );
    let bytes = Box::leak(image.into_boxed_slice());
    executable_installation::PreparedPostHandoffWriterDestination::claim(
        mapping,
        receipt,
        PlacementSite {
            base_address: base,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        },
        bytes,
    )
    .expect("activated pinned writable unpublished destination")
}

/// Drive the whole checked materialization path over `image`: the provider
/// stages the image, the sealed writer resolves member entry targets once
/// through its populated context, and the produced destination returns under
/// exact replay for the consumer edge.
fn written_table(
    code: &InstalledCode,
    plan: &layout_plans::PostHandoffWriterPlan,
    base: u64,
    image: Vec<u8>,
) -> executable_installation::ValidatedWrittenPostHandoffWriterDestination<'static, 'static> {
    let site = PlacementSite {
        base_address: base,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let context = code
        .populate_post_handoff_entry_writer_context(plan, image.len(), site)
        .expect("writer context over the exact installed realization");
    let destination = prepared_table_destination(0x710, base, image)
        .into_validated_for_writer_preparation()
        .expect("replayed prepared destination");
    code.write_prepared_post_handoff_destination(context, plan, destination)
        .expect("sealed writer executes over the staged image")
        .into_validated_for_consumer(code)
        .expect("exact written destination replay")
}

/// Decode a produced gate descriptor's 64-bit entry offset.
fn decoded_gate_offset(bytes: &[u8], vector: u8) -> u64 {
    let base = member_slot_base(vector);
    u64::from(u16::from_le_bytes([bytes[base], bytes[base + 1]]))
        | (u64::from(u16::from_le_bytes([bytes[base + 6], bytes[base + 7]])) << 16)
        | (u64::from(u32::from_le_bytes(
            bytes[base + 8..base + 12]
                .try_into()
                .expect("offset-high field"),
        )) << 32)
}

#[test]
fn descriptor_table_materialization_produces_the_declared_table() {
    let mut admitted = admitted_table();
    let code = admitted._code;

    let plan = admitted
        .table
        .descriptor_table_writer_plan(code)
        .expect("the declared table derives its checked writer");
    // Three sealed offset fragments per declared member; the image covers
    // gate slots for vectors 0 through the highest declared vector.
    assert_eq!(plan.steps.len(), 12);
    assert_eq!(plan.byte_len as u64, TABLE_BYTES);

    let image = admitted
        .table
        .descriptor_table_staged_image()
        .expect("the declared table stages its canonical image");
    assert_eq!(image.len() as u64, TABLE_BYTES);
    // The timer member's staged descriptor already carries its declared
    // selector, IST slot, and interrupt-gate attributes; the sealed offset
    // stays zero for the writer.
    let timer = member_slot_base(TIMER_TICK);
    assert_eq!(&image[timer..timer + 2], &[0, 0]);
    assert_eq!(&image[timer + 2..timer + 4], &0x08_u16.to_le_bytes());
    assert_eq!(image[timer + 4], 4);
    assert_eq!(image[timer + 5], 0x8e);
    assert_eq!(&image[timer + 12..timer + 16], &[0; 4]);

    let written = written_table(code, &plan, TABLE_BASE, image);
    let established = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &declared_tss(),
            establishment_id(0x700),
            table_destination(0x701, TABLE_BASE, TABLE_BYTES),
        )
        .expect("the produced image validates as the declared table");
    assert_eq!(established.destination().base(), TABLE_BASE);
    assert_eq!(established.destination().length(), TABLE_BYTES);

    // The sealed offsets were materialized by the writer: each member's
    // produced descriptor decodes to its exact installed entry address
    // (placement base 0x1000 plus the fixture's per-member code offset).
    for (index, vector) in member_vectors().into_iter().enumerate() {
        assert_eq!(
            decoded_gate_offset(written.bytes(), vector),
            0x1000 + 16 * (index as u64 + 1),
            "vector {vector}'s produced gate offset",
        );
    }

    // The minted value enters the existing publication path directly.
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x702),
            authority_id(0x703),
        )
        .expect("the byte-validated table issues its publication carrier");
    let authority = carrier_authority(&admitted, 0x703);
    let operand = descriptor_operand(0x704, carrier.destination());
    let answer = carrier
        .execute_checked_publication(
            code,
            &authority,
            operand,
            publication_receipt_id(0x705),
            true,
        )
        .expect("the exact authority and operand publish the table");
    let (carrier, receipt, _operand, _state) = answer.into_parts();
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, receipt)
        .expect("the minted receipt completes the carrier");
    let InterruptTablePublicationOutcome::Published(published) = outcome else {
        panic!("the checked provider answer publishes the table")
    };
    assert_eq!(published.publication(), publication_id(0x702));
    assert_eq!(published.establishment(), establishment_id(0x700));
    assert_eq!(published.members().len(), 4);
}

#[test]
fn descriptor_table_materialization_requires_the_complete_member_set() {
    let members = member_fixtures();
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(table_profile(0x600), &ledger);
    let mut handles = handles.into_iter();
    for (vector, handle) in member_vectors().into_iter().zip(&mut handles) {
        if vector != TIMER_TICK {
            table
                .admit_interrupt_table_member(&ledger, vector, handle)
                .expect("admitted interrupt-table member");
        }
    }
    assert!(!table.is_complete());

    let error = table
        .descriptor_table_writer_plan(&code)
        .expect_err("an incomplete member set cannot materialize a table");
    assert!(error.0.contains("complete declared member set"));
    let error = table
        .descriptor_table_staged_image()
        .expect_err("an incomplete member set cannot stage a table image");
    assert!(error.0.contains("complete declared member set"));
}

#[test]
fn descriptor_table_materialization_rejects_a_foreign_installed_realization() {
    let admitted = admitted_table();
    let foreign_code = table_installed_code(2, 0x999, &member_fixtures());
    let error = admitted
        .table
        .descriptor_table_writer_plan(&foreign_code)
        .expect_err("the table cannot materialize under a foreign occurrence");
    assert!(error.0.contains("different installed-code occurrence"));
}

#[test]
fn descriptor_table_validation_replays_the_declared_constant_fields() {
    let admitted = admitted_table();
    let code = admitted._code;
    let plan = admitted
        .table
        .descriptor_table_writer_plan(code)
        .expect("checked writer");
    let image = admitted
        .table
        .descriptor_table_staged_image()
        .expect("staged image");

    for (mutate, reason) in [
        // Wrong code selector on the divide-error gate.
        (
            Box::new(|image: &mut [u8]| {
                image[member_slot_base(DIVIDE_ERROR) + 2] = 0x10;
            }) as Box<dyn Fn(&mut [u8])>,
            "gate selector",
        ),
        // Trap gate encoded where the timer declares an interrupt gate.
        (
            Box::new(|image: &mut [u8]| {
                image[member_slot_base(TIMER_TICK) + 5] = 0x8f;
            }),
            "gate kind, privilege, or present bit",
        ),
        // A ring-3 DPL where the member declares ring 0.
        (
            Box::new(|image: &mut [u8]| {
                image[member_slot_base(GENERAL_PROTECTION) + 5] = 0xef;
            }),
            "gate kind, privilege, or present bit",
        ),
        // A present bit cleared in the produced image.
        (
            Box::new(|image: &mut [u8]| {
                image[member_slot_base(PAGE_FAULT) + 5] = 0x0f;
            }),
            "gate kind, privilege, or present bit",
        ),
        // A different IST field than the member declares.
        (
            Box::new(|image: &mut [u8]| {
                image[member_slot_base(PAGE_FAULT) + 4] = 7;
            }),
            "IST field",
        ),
        // Nonzero reserved bytes at the descriptor tail.
        (
            Box::new(|image: &mut [u8]| {
                image[member_slot_base(TIMER_TICK) + 15] = 1;
            }),
            "reserved bytes",
        ),
        // Nonzero content in an undeclared vector's slot.
        (
            Box::new(|image: &mut [u8]| {
                image[member_slot_base(1) + 5] = 0x8e;
            }),
            "nonzero descriptor",
        ),
    ] {
        let mut mutated = image.clone();
        mutate(&mut mutated);
        let written = written_table(code, &plan, TABLE_BASE, mutated);
        let error = admitted
            .table
            .validate_written_descriptor_table(
                code,
                &written,
                &declared_tss(),
                establishment_id(0x710),
                table_destination(0x711, TABLE_BASE, TABLE_BYTES),
            )
            .expect_err("mutated produced content cannot validate");
        assert!(error.0.contains(reason), "{}: {}", reason, error.0);
    }
}

#[test]
fn descriptor_table_validation_joins_the_declared_ist_slot_through_the_tss() {
    let admitted = admitted_table();
    let code = admitted._code;
    let plan = admitted
        .table
        .descriptor_table_writer_plan(code)
        .expect("checked writer");
    let image = admitted
        .table
        .descriptor_table_staged_image()
        .expect("staged image");
    let written = written_table(code, &plan, TABLE_BASE, image);

    // The timer member's declared IST slot absent from the installed TSS.
    let mut tss = declared_tss();
    tss.interrupt_stacks.retain(|stack| stack.slot != 4);
    let error = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &tss,
            establishment_id(0x721),
            table_destination(0x722, TABLE_BASE, TABLE_BYTES),
        )
        .expect_err("an IST slot the TSS does not provision cannot resolve the stack class");
    assert!(error.0.contains("does not resolve"));

    // The declared slot provisioned to a different stack class.
    let mut tss = declared_tss();
    tss.interrupt_stacks[3].dedicated_class = 99;
    let error = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &tss,
            establishment_id(0x723),
            table_destination(0x724, TABLE_BASE, TABLE_BYTES),
        )
        .expect_err("an IST slot mapping to a foreign class cannot resolve");
    assert!(error.0.contains("does not resolve"));

    // A TSS naming one slot twice is not a canonical map.
    let mut tss = declared_tss();
    tss.interrupt_stacks.push(X86_64InstalledInterruptStack {
        slot: 4,
        dedicated_class: 99,
    });
    let error = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &tss,
            establishment_id(0x725),
            table_destination(0x726, TABLE_BASE, TABLE_BYTES),
        )
        .expect_err("a repeated IST slot is not a canonical TSS map");
    assert!(error.0.contains("repeats an interrupt-stack-table slot"));

    // A TSS naming a slot the architecture does not encode.
    let mut tss = declared_tss();
    tss.interrupt_stacks.push(X86_64InstalledInterruptStack {
        slot: 8,
        dedicated_class: 42,
    });
    let error = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &tss,
            establishment_id(0x727),
            table_destination(0x728, TABLE_BASE, TABLE_BYTES),
        )
        .expect_err("an out-of-range IST slot is not a canonical TSS map");
    assert!(error.0.contains("outside 1..=7"));

    // And the canonical TSS validates the same produced image.
    admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &declared_tss(),
            establishment_id(0x729),
            table_destination(0x72a, TABLE_BASE, TABLE_BYTES),
        )
        .expect("the canonical TSS resolves every member's stack class");
}

#[test]
fn descriptor_table_validation_requires_a_dedicated_stack_route() {
    // A member whose declared gate selects no IST slot cannot reach a
    // dedicated critical stack class through the produced table.
    let profile = InterruptTableProfile::new(
        profile_id(0x601),
        [
            fatal_member(DIVIDE_ERROR, 11, 1),
            fatal_member(GENERAL_PROTECTION, 12, 2),
            fatal_member(PAGE_FAULT, 13, 3),
            InterruptTableMemberPlan {
                descriptor: InterruptTableGateDescriptor {
                    interrupt_stack_table_slot: None,
                    ..timer_member(TIMER_TICK, 14, 4).descriptor
                },
                ..timer_member(TIMER_TICK, 14, 4)
            },
        ],
    )
    .expect("interrupt-table profile");
    let members = member_fixtures();
    let mut code = table_installed_code(1, 300, &members);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(profile, &ledger);
    for (vector, handle) in member_vectors().into_iter().zip(handles) {
        table
            .admit_interrupt_table_member(&ledger, vector, handle)
            .expect("admitted interrupt-table member");
    }
    let plan = table
        .descriptor_table_writer_plan(&code)
        .expect("checked writer");
    let image = table.descriptor_table_staged_image().expect("staged image");
    assert_eq!(image[member_slot_base(TIMER_TICK) + 4], 0);
    let written = written_table(&code, &plan, TABLE_BASE, image);
    let error = table
        .validate_written_descriptor_table(
            &code,
            &written,
            &declared_tss(),
            establishment_id(0x730),
            table_destination(0x731, TABLE_BASE, TABLE_BYTES),
        )
        .expect_err("a gate with no IST field selects no dedicated critical stack");
    assert!(error.0.contains("selects no IST slot"));
}

#[test]
fn descriptor_table_validation_requires_the_exact_writer_and_destination() {
    let admitted = admitted_table();
    let code = admitted._code;
    let plan = admitted
        .table
        .descriptor_table_writer_plan(code)
        .expect("checked writer");
    let image = admitted
        .table
        .descriptor_table_staged_image()
        .expect("staged image");

    // A destination produced by a different writer program — same sealed
    // targets but different fragment geometry — never binds this table's
    // invocation even when its bytes look identical.
    let mut foreign_plan = plan.clone();
    foreign_plan.steps[0].write.container_width_bits = 8;
    foreign_plan.steps[0].write.width = 8;
    let written = written_table(code, &foreign_plan, TABLE_BASE, image.clone());
    let error = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &declared_tss(),
            establishment_id(0x740),
            table_destination(0x741, TABLE_BASE, TABLE_BYTES),
        )
        .expect_err("a foreign writer's output cannot establish this table");
    assert!(error.0.contains("derived writer invocation"));

    // The exact writer's output still cannot establish a destination the
    // bytes do not occupy.
    let written = written_table(code, &plan, TABLE_BASE, image);
    for destination in [
        table_destination(0x742, TABLE_BASE + 16, TABLE_BYTES),
        table_destination(0x743, TABLE_BASE, TABLE_BYTES - 16),
    ] {
        let error = admitted
            .table
            .validate_written_descriptor_table(
                code,
                &written,
                &declared_tss(),
                establishment_id(0x744),
                destination,
            )
            .expect_err("a destination the image does not occupy cannot establish");
        assert!(error.0.contains("exact written table extent"));
    }
}

#[test]
fn descriptor_declarations_must_encode_and_stay_canonical() {
    for (descriptor, reason) in [
        (
            InterruptTableGateDescriptor {
                selector: 0,
                ..gate_descriptor(X86_64GateKind::Interrupt, Some(1))
            },
            "null gate selector",
        ),
        (
            InterruptTableGateDescriptor {
                entry_privilege: 4,
                ..gate_descriptor(X86_64GateKind::Interrupt, Some(1))
            },
            "privilege outside 0..=3",
        ),
        (
            gate_descriptor(X86_64GateKind::Interrupt, Some(0)),
            "outside 1..=7",
        ),
        (
            gate_descriptor(X86_64GateKind::Interrupt, Some(8)),
            "outside 1..=7",
        ),
    ] {
        let error = InterruptTableProfile::new(
            profile_id(0x650),
            [InterruptTableMemberPlan {
                vector: TIMER_TICK,
                dedicated_stack_class: 14,
                obligation: InterruptTableObligation::AcknowledgedInterrupt,
                descriptor,
            }],
        )
        .expect_err("a non-encodable descriptor cannot be declared");
        assert!(error.0.contains(reason), "{}: {}", reason, error.0);
    }
}

#[test]
fn descriptor_table_materialization_rejects_pre_resolved_writer_sources() {
    let admitted = admitted_table();
    let code = admitted._code;
    let plan = admitted
        .table
        .descriptor_table_writer_plan(code)
        .expect("checked writer");
    let image = admitted
        .table
        .descriptor_table_staged_image()
        .expect("staged image");
    let site = PlacementSite {
        base_address: TABLE_BASE,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let member_zero_target = layout_plans::RelocationTarget::Entry(
        EntryStubId::from_normalized_identity(0x201).expect("entry identity"),
    );

    // A stale pre-resolved value from another realization — 0x2010 is not
    // member zero's installed entry address — rejects at provider
    // preparation, before a destination byte can change.
    let mut stale = plan.clone();
    for step in &mut stale.steps {
        if step.write.target == member_zero_target {
            step.source = layout_plans::PostHandoffWriterSource::Resolved(0x2010);
        }
    }
    let error = code
        .populate_post_handoff_entry_writer_context(&stale, image.len(), site)
        .expect_err("a stale pre-resolved entry value cannot populate");
    assert!(
        error
            .0
            .contains("does not match the exact installed realization")
    );

    // Even the correct address pre-resolved outside the sealed resolver is a
    // different writer invocation: the produced bytes are identical yet the
    // consumer's replay does not bind this table's derived writer.
    let mut pre_resolved = plan.clone();
    for step in &mut pre_resolved.steps {
        if step.write.target == member_zero_target {
            step.source = layout_plans::PostHandoffWriterSource::Resolved(0x1010);
        }
    }
    let written = written_table(code, &pre_resolved, TABLE_BASE, image);
    let error = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &declared_tss(),
            establishment_id(0x760),
            table_destination(0x761, TABLE_BASE, TABLE_BYTES),
        )
        .expect_err("a pre-resolved writer's identical bytes cannot establish this table");
    assert!(error.0.contains("derived writer invocation"));
}

#[test]
fn failed_descriptor_table_write_returns_the_unchanged_destination() {
    let admitted = admitted_table();
    let code = admitted._code;
    let plan = admitted
        .table
        .descriptor_table_writer_plan(code)
        .expect("checked writer");
    let image = admitted
        .table
        .descriptor_table_staged_image()
        .expect("staged image");
    let site = PlacementSite {
        base_address: TABLE_BASE,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let context = code
        .populate_post_handoff_entry_writer_context(&plan, image.len(), site)
        .expect("writer context");
    let destination = prepared_table_destination(0x770, TABLE_BASE, image.clone())
        .into_validated_for_writer_preparation()
        .expect("replayed prepared destination");

    // Executing a different program under the sealed context rejects before
    // any byte changes, and the error returns both linear inputs.
    let mut drifted = plan.clone();
    drifted.steps[0].write.container_width_bits = 8;
    drifted.steps[0].write.width = 8;
    let error = code
        .write_prepared_post_handoff_destination(context, &drifted, destination)
        .expect_err("a drifted writer program cannot run under the sealed context");
    let (context, destination) = error.into_parts();

    // The returned destination still carries the staged image: re-running the
    // exact writer under its sealed context produces a table the consumer
    // accepts.
    let written = code
        .write_prepared_post_handoff_destination(context, &plan, destination)
        .expect("the unchanged destination accepts the exact writer")
        .into_validated_for_consumer(code)
        .expect("exact written destination replay");
    let established = admitted
        .table
        .validate_written_descriptor_table(
            code,
            &written,
            &declared_tss(),
            establishment_id(0x771),
            table_destination(0x772, TABLE_BASE, TABLE_BYTES),
        )
        .expect("the recovered destination still produces the declared table");
    assert_eq!(established.destination().base(), TABLE_BASE);
}

#[test]
fn descriptor_table_materialization_rejects_unqualified_destination_geometry() {
    let admitted = admitted_table();
    let code = admitted._code;
    let plan = admitted
        .table
        .descriptor_table_writer_plan(code)
        .expect("checked writer");
    let image = admitted
        .table
        .descriptor_table_staged_image()
        .expect("staged image");

    // A destination base that cannot hold a 16-byte-aligned gate image.
    let site = PlacementSite {
        base_address: TABLE_BASE + 8,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let error = code
        .populate_post_handoff_entry_writer_context(&plan, image.len(), site)
        .expect_err("a misaligned destination site cannot host the table");
    assert!(error.0.contains("aligned"));

    // A destination too short for the complete declared image.
    let site = PlacementSite {
        base_address: TABLE_BASE,
        phase: PlacementPhase::PostHandoff,
        machine_regime: None,
        installation_scope: None,
    };
    let error = code
        .populate_post_handoff_entry_writer_context(&plan, image.len() - 16, site)
        .expect_err("a truncated destination cannot host the table");
    assert!(error.0.contains("destination"));
}

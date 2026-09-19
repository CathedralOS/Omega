use super::{
    AdmittedTable, DIVIDE_ERROR, GENERAL_PROTECTION, PAGE_FAULT, TIMER_TICK, admitted_table,
    authority_id, established_for, established_table, establishment_id, fatal_member,
    install_members, member_fixtures, member_fixtures_with_spare, member_vectors, profile_id,
    publication_id, publication_receipt_id, table_destination, table_installed_code, table_profile,
    timer_member,
};
use crate::interrupts::interrupt_table::{
    EntryStack, EstablishedInterruptTable, InstalledCodeId, InstalledRootLedger,
    InterruptTableEstablishedMember, InterruptTableLedger, InterruptTableProfile,
    InterruptTablePublication, InterruptTablePublicationOutcome, InterruptTablePublicationReceipt,
};
use crate::{
    ExternalRootId, InterruptAcknowledgementReceipt, InterruptAcknowledgementReceiptId,
    InterruptEpochTurnReport, InterruptInvocationId, InterruptPreemptionReport, RootAdmission,
    RootAdmissionId, RootRemovalReceipt, RootRemovalReceiptId, RootSlotAuthority, RootSlotId,
    RootSlotOwnerId, validate_external_root,
};
use calling_conventions::{ArrivalContextId, EntryStackStage};
use layout_plans::EntryStubId;

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
// Hardware arrivals dispatch through the published table.
// ---------------------------------------------------------------------

#[test]
fn published_table_dispatches_the_timer_arrival_to_its_member_root() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x619);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x629),
            authority_id(0x638),
        )
        .expect("issued publication carrier");
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x647),
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

    // The timer vector resolves through the armed member row to the exact
    // installed root: the receipt mints the Pending acknowledgement and the
    // settled exit completes it.
    let timer_root = admitted
        .table
        .member(TIMER_TICK)
        .expect("admitted timer member")
        .root()
        .root();
    let obligations = admitted
        .table
        .begin_published_interrupt_entry(
            &mut admitted.ledger,
            TIMER_TICK,
            crate::tests::interrupt_entry_receipt(
                admitted
                    .table
                    .member(TIMER_TICK)
                    .expect("admitted timer member")
                    .root(),
                90,
                Some(7),
                Some(91),
            ),
        )
        .expect("the armed timer vector admits the entry");
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
    assert_eq!(completed.root, timer_root);

    // A fatal exception vector dispatches through the same edge and settles
    // with no acknowledgement obligation.
    let obligations = admitted
        .table
        .begin_published_interrupt_entry(
            &mut admitted.ledger,
            DIVIDE_ERROR,
            crate::tests::interrupt_entry_receipt(
                admitted
                    .table
                    .member(DIVIDE_ERROR)
                    .expect("admitted divide-error member")
                    .root(),
                93,
                None,
                None,
            ),
        )
        .expect("the armed divide-error vector admits the entry");
    let (pending, control, acknowledgement) = obligations.into_parts();
    assert!(acknowledgement.is_none());
    let completed = admitted
        .ledger
        .finish_interrupt_entry(pending, control, None)
        .expect("settled fatal exit");
    assert_eq!(
        completed.root,
        crate::tests::root_id(0x101, ExternalRootId::from_normalized_identity)
    );
}

#[test]
fn interrupt_entry_dispatch_rejects_arrivals_before_publication() {
    let mut admitted = admitted_table();
    let timer_receipt = |admitted: &AdmittedTable<'_>| {
        crate::tests::interrupt_entry_receipt(
            admitted
                .table
                .member(TIMER_TICK)
                .expect("admitted timer member")
                .root(),
            90,
            Some(7),
            Some(91),
        )
    };

    // A table whose carrier has not issued arms no vector.
    let receipt = timer_receipt(&admitted);
    let error = admitted
        .table
        .begin_published_interrupt_entry(&mut admitted.ledger, TIMER_TICK, receipt)
        .expect_err("an admitting table arms no vector");
    assert!(
        error
            .diagnostic()
            .0
            .contains("publication reaches hardware")
    );
    let _ = error.into_receipt();

    // An issued-but-unanswered carrier is still not published: the hardware
    // has not yet seen the member rows.
    let established = established_for(&admitted, 0x61a);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x62a),
            authority_id(0x639),
        )
        .expect("issued publication carrier");
    let receipt = timer_receipt(&admitted);
    let error = admitted
        .table
        .begin_published_interrupt_entry(&mut admitted.ledger, TIMER_TICK, receipt)
        .expect_err("an unanswered carrier arms no vector");
    assert!(
        error
            .diagnostic()
            .0
            .contains("publication reaches hardware")
    );
    let _ = error.into_receipt();

    // A refused carrier returns admission custody; arrivals still reject.
    let refusal = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x648),
        &carrier,
        false,
    );
    let outcome = admitted
        .table
        .complete_interrupt_table_publication(&admitted.ledger, carrier, refusal)
        .expect("refusal consumes the exact carrier");
    assert!(matches!(
        outcome,
        InterruptTablePublicationOutcome::Refused(_)
    ));
    let receipt = timer_receipt(&admitted);
    let error = admitted
        .table
        .begin_published_interrupt_entry(&mut admitted.ledger, TIMER_TICK, receipt)
        .expect_err("a refused publication arms no vector");
    assert!(
        error
            .diagnostic()
            .0
            .contains("publication reaches hardware")
    );
    let _ = error.into_receipt();
}

#[test]
fn published_table_rejects_unarmed_vectors_and_foreign_ledgers() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x61b);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x62b),
            authority_id(0x63a),
        )
        .expect("issued publication carrier");
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x649),
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

    // A vector the profile never declared is not armed by the published rows.
    let error = admitted
        .table
        .begin_published_interrupt_entry(
            &mut admitted.ledger,
            0x2e,
            crate::tests::interrupt_entry_receipt(
                admitted
                    .table
                    .member(TIMER_TICK)
                    .expect("admitted timer member")
                    .root(),
                94,
                Some(7),
                Some(94),
            ),
        )
        .expect_err("an undeclared vector is not an armed member");
    assert!(error.diagnostic().0.contains("not an armed member"));
    let _ = error.into_receipt();

    // A root ledger over a different installed realization cannot dispatch
    // through this table even on an armed vector.
    let members = member_fixtures();
    let mut foreign_code = table_installed_code(2, 301, &members);
    let mut foreign_ledger =
        InstalledRootLedger::claim(&mut foreign_code).expect("foreign root ledger");
    let error = admitted
        .table
        .begin_published_interrupt_entry(
            &mut foreign_ledger,
            TIMER_TICK,
            crate::tests::interrupt_entry_receipt(
                admitted
                    .table
                    .member(TIMER_TICK)
                    .expect("admitted timer member")
                    .root(),
                95,
                Some(7),
                Some(95),
            ),
        )
        .expect_err("a foreign installed realization cannot dispatch");
    assert!(
        error
            .diagnostic()
            .0
            .contains("different installed realization")
    );
    let _ = error.into_receipt();
}

#[test]
fn published_table_dispatch_rejoins_the_receipt_against_the_armed_member() {
    // The spare row is installed in the ledger but the profile does not
    // declare its vector, so it stays outside the armed member set.
    let members = member_fixtures_with_spare();
    let code = Box::leak(Box::new(table_installed_code(1, 300, &members)));
    let mut ledger = InstalledRootLedger::claim(code).expect("canonical root ledger");
    let handles = install_members(&mut ledger, code, &members);
    let profile = table_profile(0x600);
    let mut table = InterruptTableLedger::new(profile.clone(), &ledger);
    let mut handles = handles.into_iter();
    for vector in member_vectors() {
        table
            .admit_interrupt_table_member(&ledger, vector, handles.next().expect("member handle"))
            .expect("admitted interrupt-table member");
    }
    let spare = handles.next().expect("spare installed root");
    let established = established_table(
        0x61c,
        &profile,
        code.identity(),
        code.artifact(),
        &table,
        table_destination(0x91c, 0x8_0000, 0x1000),
    );
    let carrier = table
        .begin_interrupt_table_publication(
            &ledger,
            established,
            publication_id(0x62c),
            authority_id(0x63b),
        )
        .expect("issued publication carrier");
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x64a),
        &carrier,
        true,
    );
    let outcome = table
        .complete_interrupt_table_publication(&ledger, carrier, receipt)
        .expect("the exact receipt publishes the table");
    assert!(matches!(
        outcome,
        InterruptTablePublicationOutcome::Published(_)
    ));

    // The timer vector resolves to the timer member; a receipt naming the
    // spare installed root drifts from the armed row's exact root and the
    // ordinary edge rejects it with the receipt returned.
    let error = table
        .begin_published_interrupt_entry(
            &mut ledger,
            TIMER_TICK,
            crate::tests::interrupt_entry_receipt(&spare, 96, Some(7), Some(96)),
        )
        .expect_err("a receipt must bind the armed member's exact root");
    assert!(
        error
            .diagnostic()
            .0
            .contains("does not bind the exact installed interrupt root")
    );
    let _ = error.into_receipt();
}

#[test]
fn published_fatal_member_preempts_unconditionally_and_halts_on_settle() {
    let mut admitted = admitted_table();
    let established = established_for(&admitted, 0x61b);
    let carrier = admitted
        .table
        .begin_interrupt_table_publication(
            &admitted.ledger,
            established,
            publication_id(0x62b),
            authority_id(0x63a),
        )
        .expect("issued publication carrier");
    let receipt = InterruptTablePublicationReceipt::from_provider(
        publication_receipt_id(0x64b),
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

    // The timer member enters first and stays live at its fresh Body stage:
    // the fixture's arrival context realizes a single Body epoch.
    let timer_root = admitted
        .table
        .member(TIMER_TICK)
        .expect("admitted timer member")
        .root()
        .root();
    let timer_invocation =
        crate::tests::root_id(90, InterruptInvocationId::from_normalized_identity);
    let timer_obligations = admitted
        .table
        .begin_published_interrupt_entry(
            &mut admitted.ledger,
            TIMER_TICK,
            crate::tests::interrupt_entry_receipt(
                admitted
                    .table
                    .member(TIMER_TICK)
                    .expect("admitted timer member")
                    .root(),
                90,
                Some(7),
                Some(91),
            ),
        )
        .expect("the armed timer vector admits the entry");
    let (timer_pending, timer_control, _) = timer_obligations.into_parts();

    // An acknowledged arrival cannot preempt: the fixture declares no
    // stack-nesting edges, so a nested timer invocation rejects.
    let ordinary = admitted
        .table
        .begin_published_interrupt_entry(
            &mut admitted.ledger,
            TIMER_TICK,
            crate::tests::interrupt_entry_receipt_in_context(
                admitted
                    .table
                    .member(TIMER_TICK)
                    .expect("admitted timer member")
                    .root(),
                ArrivalContextId::new(1).expect("fixture arrival context"),
                Some(InterruptPreemptionReport::new(
                    timer_root,
                    timer_invocation,
                    EntryStackStage::Body,
                )),
                95,
                Some(7),
                Some(95),
            ),
        )
        .expect_err("a nested acknowledged entry requires a declared nesting edge");
    assert!(ordinary.diagnostic().0.contains("stack-nesting edge"));

    // The fatal member preempts unconditionally: the processor fault cannot
    // wait on a declared edge, and its dedicated critical stack owes no
    // parent depth bound. The report still names the innermost live
    // invocation and rejoins its retained stage.
    let fatal_invocation =
        crate::tests::root_id(96, InterruptInvocationId::from_normalized_identity);
    let fatal_obligations = admitted
        .table
        .begin_published_interrupt_entry(
            &mut admitted.ledger,
            DIVIDE_ERROR,
            crate::tests::interrupt_entry_receipt_in_context(
                admitted
                    .table
                    .member(DIVIDE_ERROR)
                    .expect("admitted fatal member")
                    .root(),
                ArrivalContextId::new(1).expect("fixture arrival context"),
                Some(InterruptPreemptionReport::new(
                    timer_root,
                    timer_invocation,
                    EntryStackStage::Body,
                )),
                96,
                None,
                None,
            ),
        )
        .expect("a fatal entry preempts unconditionally on its critical stack");
    let (fatal_pending, fatal_control, fatal_acknowledgement) = fatal_obligations.into_parts();
    assert!(fatal_acknowledgement.is_none());

    // Settling the fatal entry halts the ledger: the interrupted chain never
    // resumes ordinary work.
    let completed = admitted
        .ledger
        .finish_interrupt_entry(fatal_pending, fatal_control, None)
        .expect("the fatal entry settles at its terminal stage");
    assert!(completed.fatal);
    assert_eq!(
        admitted.ledger.halted_by(),
        Some((
            admitted
                .table
                .member(DIVIDE_ERROR)
                .expect("admitted fatal member")
                .root()
                .root(),
            fatal_invocation,
        ))
    );

    // The preempted timer cannot settle, cannot turn its epoch, and the
    // halted ledger admits no further arrivals — the live entry remains
    // held as halted evidence.
    let error = admitted
        .ledger
        .finish_interrupt_entry(timer_pending, timer_control, None)
        .expect_err("a halted ledger resumes nothing");
    assert!(error.diagnostic().0.contains("halted"));
    let _ = error.into_parts();

    let error = admitted
        .ledger
        .turn_interrupt_epoch_stage(
            admitted
                .table
                .member(TIMER_TICK)
                .expect("admitted timer member")
                .root(),
            InterruptEpochTurnReport::new(timer_root, timer_invocation, EntryStackStage::Exit),
        )
        .expect_err("a halted ledger turns no epochs");
    assert!(error.diagnostic().0.contains("halted"));

    let error = admitted
        .table
        .begin_published_interrupt_entry(
            &mut admitted.ledger,
            GENERAL_PROTECTION,
            crate::tests::interrupt_entry_receipt(
                admitted
                    .table
                    .member(GENERAL_PROTECTION)
                    .expect("admitted fatal member")
                    .root(),
                97,
                None,
                None,
            ),
        )
        .expect_err("a halted ledger admits no further arrivals");
    assert!(error.diagnostic().0.contains("halted"));
}

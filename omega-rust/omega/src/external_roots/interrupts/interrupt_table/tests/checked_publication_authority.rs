use super::{
    admitted_table, authority_id, carrier_authority, descriptor_operand, established_for,
    establishment_id, foreign_space_site, full_publication_authority, member_fixtures,
    publication_authority, publication_id, publication_receipt_id, table_installed_code,
};
use crate::executable_installation::ArtifactEntry;
use crate::external_roots::interrupts::interrupt_table::{
    ArtifactId, INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES, InstalledCodeId,
    InterruptTableDescriptorOperand, InterruptTablePublicationAuthority,
    InterruptTablePublicationOutcome, InterruptTablePublicationScope,
};
use abstract_operations_to_target_operations::calling_conventions::MachineRegister;
use target::Architecture;
use terminal_psi::layout_plans::{
    ArtifactInstallationScopeId, MachineRegimeId, PlacementAddressRange, PlacementConstraints,
    PlacementPhase,
};

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
        crate::external_roots::tests::minted_secondary_processor_state(
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
        crate::external_roots::tests::minted_secondary_processor_state(
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
        crate::external_roots::tests::minted_secondary_processor_state(0x656, 0x9_0000, 9),
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
        crate::external_roots::tests::minted_secondary_processor_state(
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
        crate::external_roots::tests::installed_code_in_placement_with_entries(
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

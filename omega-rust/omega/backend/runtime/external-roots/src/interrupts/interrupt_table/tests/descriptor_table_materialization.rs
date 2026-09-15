use super::{
    DIVIDE_ERROR, GENERAL_PROTECTION, PAGE_FAULT, TABLE_BASE, TABLE_BYTES, TIMER_TICK,
    admitted_table, authority_id, carrier_authority, declared_tss, decoded_gate_offset,
    descriptor_operand, establishment_id, fatal_member, gate_descriptor, install_members,
    member_fixtures, member_slot_base, member_vectors, prepared_table_destination, profile_id,
    publication_id, publication_receipt_id, table_destination, table_installed_code, table_profile,
    timer_member, written_table,
};
use crate::interrupts::interrupt_table::{
    InstalledRootLedger, InterruptTableGateDescriptor, InterruptTableLedger,
    InterruptTableMemberPlan, InterruptTableObligation, InterruptTableProfile,
    InterruptTablePublicationOutcome,
};
use calling_conventions::{X86_64GateKind, X86_64InstalledInterruptStack};
use layout_plans::{EntryStubId, PlacementPhase, PlacementSite};

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

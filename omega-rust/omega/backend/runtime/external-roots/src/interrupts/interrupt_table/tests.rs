//! Fixtures shared by the interrupt table tests: declared members, gate
//! descriptors, established tables, publication authorities and the
//! descriptor operand sites the checked publication edge replays.

mod checked_publication_authority;
mod member_admission_and_publication;

use crate::interrupts::interrupt_table::{
    ArtifactId, EstablishedInterruptTable, INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES,
    InstalledCodeId, InstalledExternalRoot, InstalledRootLedger, InterruptTableDescriptorOperand,
    InterruptTableEstablishedMember, InterruptTableGateDescriptor, InterruptTableLedger,
    InterruptTableMemberAdmission, InterruptTableMemberFacts, InterruptTableMemberPlan,
    InterruptTableObligation, InterruptTableProfile,
    InterruptTablePublicationAuthority, InterruptTablePublicationAuthorityId,
    InterruptTablePublicationId, InterruptTablePublicationReceiptId,
    InterruptTablePublicationScope,
};
use crate::{
    InterruptTableEstablishmentId, InterruptTableProfileId, RootAdmission, RootAdmissionId,
    RootSlotAuthority, RootSlotId, RootSlotOwnerId,
};
use calling_conventions::X86_64GateKind;
use executable_installation::{ArtifactEntry, InstalledCode};
use extents::Extent;
use layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, MachineRegimeId, PlacementAddressRange,
    PlacementConstraints, PlacementPhase,
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

/// The member-arrival facts one declared row expects verbatim from the
/// installed record: interrupt-return exit, arrival on the declared
/// dedicated class, and the declared obligation's acknowledgement shape.
/// Minting the record for facts the fixture's record does not carry is how
/// a binding rejection is driven.
fn declared_member_facts(
    stack_class: u16,
    acknowledged: bool,
) -> InterruptTableMemberFacts {
    InterruptTableMemberFacts {
        entry_interrupt_return: true,
        stack_dedicated_class: stack_class,
        acknowledgement_policy: acknowledged,
        acknowledgement_parameter: acknowledged,
    }
}

/// The admission record the consumer's authored verdict mints for one
/// declared member. Unit fixtures mint it directly; the authored
/// `TableMemberAdmission::admit` machine is the semantic warrant upstream.
fn member_admission(
    plan: InterruptTableMemberPlan,
    facts: InterruptTableMemberFacts,
) -> InterruptTableMemberAdmission {
    InterruptTableMemberAdmission::from_consumer(plan, facts)
}

/// The verdict the declared member's fixture row satisfies: facts derived
/// straight from the fixture so the binding replays cleanly.
fn fixture_member_admission(
    plan: InterruptTableMemberPlan,
    fixture: &crate::tests::InterruptTableMemberFixture,
) -> InterruptTableMemberAdmission {
    member_admission(plan, declared_member_facts(fixture.stack_class, fixture.acknowledged))
}

/// The declared board-item table: every fatal exception entry on its own
/// critical stack class plus the minimal timer root on a dedicated class.
/// Each member's declared IST slot names the installed-TSS binding the
/// consumer's authored validator resolves its class through.
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
    for ((vector, fixture), handle) in member_vectors()
        .into_iter()
        .zip(&members)
        .zip(handles)
    {
        table
            .admit_interrupt_table_member(
                &ledger,
                vector,
                handle,
                fixture_member_admission(
                    *profile.member(vector).expect("declared member"),
                    fixture,
                ),
            )
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

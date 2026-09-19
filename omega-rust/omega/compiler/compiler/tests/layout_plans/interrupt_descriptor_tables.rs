//! The descriptor-table leg of the exception-roots-and-timer board item:
//! Cathedral's interrupt descriptor table authored as an ordinary source
//! layout drives the compiler's generic post-handoff writer end to end.
//!
//! `tests/omega/pass/memory/interrupt_table_canary` carries the package
//! surface — `cathedral::interrupt_table` declares the semantic gate and
//! table records plus the build-time layout policies that place them on
//! the x86-64 wire encoding. This module compiles that program, computes
//! the validated layout plans, binds each declared member's sealed entry
//! stub as a symbolic value on the `gates[vector].entry` record path, and
//! derives the post-handoff writer. The writer then executes through the
//! checked `InstalledCode` destination protocol — sealed entry targets
//! resolve once inside `populate_post_handoff_entry_writer_context`, the
//! staged image lands in an activated, pinned, unpublished destination,
//! and `write_prepared_post_handoff_destination` returns produced bytes
//! under exact replay.
//!
//! The package's validator replays the written destination through the
//! gate layout — `decode_scalar_layout` reassembles the split `entry`
//! fragments and the packed flag byte — before minting the
//! `EstablishedInterruptTable` value the publication carrier consumes. It
//! mirrors the compiler-owned `interrupt_table` model's checks (declared
//! constants, IST slot join through the installed TSS, reserved-range
//! zeros, absent-vector all-zero slots) without any hand-rolled byte
//! indexing: the authored layout is the only placement vocabulary on this
//! path.
//!
//! The published leg completes the path: test-shaped installed external
//! roots fill the compiler-owned member ledger, the validator's
//! established value issues the exact publication carrier, and the checked
//! `lidt` provider edge — `execute_checked_publication` — mints the
//! receipt that publishes the table. The authored layout is the only
//! placement vocabulary on the whole path; what remains on the seam is
//! spelled in `TASKS.md` — Cathedral-authored exception/timer roots (the
//! ledger's candidates are still test-constructed shapes) and the
//! package-side relocation of the Rust model's published-root records.

use build_time_evaluation::compute_layout_plan;
use calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, BoundaryEntryPlan, CallSignature, CallingPolicy,
    EntryControl, EntryStack, EntryStackEpoch, EntryStackRealization, EntryStackStage,
    MachineRegime, MachineRegister, MachineState, MachineStateSet, Preemption,
    ProviderExitRealization, RegisterSet, StackDomainRef, StateFootprintEvidence, StatePlan,
    ValidatedBoundaryEntryPlan, ValueShape, X86_64GateKind, X86_64InstalledInterruptStack,
    X86_64InstalledTaskStateSegmentRealization, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan, validate_entry_stack_realization,
};
use compiler::{CheckedCompileRequest, compile_to_checked};
use executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, CodePlacementAuthority,
    CodePlacementId, DestinationPreparationReceipt, DestinationPreparationReceiptId, EntrySetId,
    FinalValidationCertificate, FinalValidationId, InstallAuthority, InstallationAudience,
    InstallationReceipt, InstallationScopeId, InstalledCode, MachineContractSetId,
    MachineFootprintId, MaterializationReceipt, PlacementPlanId,
    PreparedPostHandoffWriterDestination, RelocationSetId,
    ValidatedWrittenPostHandoffWriterDestination, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use extents::{
    AddressSpaceId, Extent, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappedExtent, MappingEraId, MappingGrant, MappingGrantId,
    MappingId, MappingSourceMode, TranslationActivationFactId, TranslationActivationReceipt,
    TranslationInstallObligations, TranslationReleaseObligations, map_owned,
};
use external_roots::{
    AcknowledgementPolicyId, BoundEpochStackCompositionInput, ComponentArtifactId,
    ComponentContractId, ComponentProviderId, ComponentVersionPin, ComponentVersionPinId,
    EstablishedInterruptTable, ExternalRootCandidate, ExternalRootDiagnostic,
    ExternalRootEntryClaim, ExternalRootId, ExternalRootResultClaim, FixedFuelCall,
    FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity, FuelValidationReceiptId,
    InstalledExternalRoot, InstalledRootLedger, InterruptTableDescriptorOperand,
    InterruptTableEstablishedMember, InterruptTableEstablishmentId, InterruptTableGateDescriptor,
    InterruptTableLedger, InterruptTableMemberPlan, InterruptTableObligation,
    InterruptTableProfile, InterruptTableProfileId, InterruptTablePublicationAuthority,
    InterruptTablePublicationAuthorityId, InterruptTablePublicationId,
    InterruptTablePublicationOutcome, InterruptTablePublicationReceiptId,
    InterruptTablePublicationScope, LogicalFuelResourceColumn, MachineStateResourceColumn,
    NestingRelationId, OpaqueProviderExitAssurance, ProviderExecution, ProviderExecutionId,
    ProviderFuelSummaryId, ProviderFuelValidationReceiptId, ProviderPlanId, ProviderStackSummary,
    ResolvedRootServiceReach, RootAdmission, RootAdmissionId, RootEffectId, RootProviderId,
    RootSlotAuthority, RootSlotId, RootSlotOwnerId, StackNestingRelation, StackResourceColumn,
    StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
    admit_opaque_arrival_context_set, bind_opaque_adapter_stack_realization,
    compose_bound_entry_stack_epochs, compose_fixed_fuel, validate_external_root,
};
use layout_plans::{
    AggregateFieldSchema, AggregateFieldValue, ByteOrder, ConsumptionInstant, EntryStubId,
    LayoutPlacementReport, LayoutPlanReport, MaterializationAction, MaterializationContext,
    PlacementAddressRange, PlacementConstraints, PlacementPhase, PlacementSite,
    PostHandoffWriterInvocationPlan, RelocationTarget, ScalarFieldSchema, ScalarFieldValue,
    SymbolicFieldInnerLayout, SymbolicFieldPathSegment, SymbolicFieldValue, decode_scalar_layout,
    derive_symbolic_materialization_with_inner_layouts, materialize_aggregate_layout_into,
    materialize_scalar_layout_into,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use target::Architecture;

// The board item's declared member set: three fatal exception entries on
// their own critical stack classes (trap gates) plus the minimal timer
// root on its own class (interrupt gate, so arrivals mask further
// interrupts until it settles).
const DIVIDE_ERROR: u8 = 0;
const GENERAL_PROTECTION: u8 = 13;
const PAGE_FAULT: u8 = 14;
const TIMER_TICK: u8 = 0x20;

// The authored table's fixed geometry: one 16-byte gate slot per vector
// 0..=0x20, matching `descriptor_table_staged_image`'s declared extent in
// the compiler-owned model this source layout replaces.
const GATE_BYTES: usize = 16;
const TABLE_VECTORS: u8 = 33;
const TABLE_BYTES: usize = TABLE_VECTORS as usize * GATE_BYTES;
const TABLE_BASE: u64 = 0x8_0000;

// Installed-member fixture geometry: entry stubs 0x201..0x204 sit at code
// offsets 16..=64 inside the artifact placed at 0x1000, so their sealed
// targets are 0x1010, 0x1020, 0x1030, 0x1040.
const PLACEMENT_BASE: u64 = 0x1000;
const SELECTOR: u16 = 0x08;

// x86-64 long-mode gate type codes as semantic values of the authored
// `gate_kind` field.
const GATE_TYPE_INTERRUPT: u64 = 0x0e;
const GATE_TYPE_TRAP: u64 = 0x0f;

/// One declared member's complete contract: the profile row the package
/// declares, the sealed entry stub the writer resolves, and the installed
/// root identity the established table's member rows carry.
#[derive(Debug, Clone, Copy)]
struct DeclaredMember {
    profile: InterruptTableMemberPlan,
    entry: EntryStubId,
    root: ExternalRootId,
}

fn identity<T>(value: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(value).expect("normalized external-root identity")
}

fn install_identity<T>(
    value: u64,
    constructor: fn(u64) -> Result<T, executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(value).expect("normalized installation identity")
}

fn extent_identity<T>(value: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(value).expect("normalized extent identity")
}

fn declared_members() -> BTreeMap<u8, DeclaredMember> {
    let fatal = |vector: u8, stack_class: u16, ist: u8, entry: u64, root: u64| DeclaredMember {
        profile: InterruptTableMemberPlan {
            vector,
            dedicated_stack_class: stack_class,
            obligation: InterruptTableObligation::FatalException,
            descriptor: InterruptTableGateDescriptor {
                gate: X86_64GateKind::Trap,
                selector: SELECTOR,
                entry_privilege: 0,
                interrupt_stack_table_slot: Some(ist),
            },
        },
        entry: EntryStubId::from_normalized_identity(entry).expect("entry stub"),
        root: identity(root, ExternalRootId::from_normalized_identity),
    };
    let timer = |vector: u8, stack_class: u16, ist: u8, entry: u64, root: u64| DeclaredMember {
        profile: InterruptTableMemberPlan {
            vector,
            dedicated_stack_class: stack_class,
            obligation: InterruptTableObligation::AcknowledgedInterrupt,
            descriptor: InterruptTableGateDescriptor {
                gate: X86_64GateKind::Interrupt,
                selector: SELECTOR,
                entry_privilege: 0,
                interrupt_stack_table_slot: Some(ist),
            },
        },
        entry: EntryStubId::from_normalized_identity(entry).expect("entry stub"),
        root: identity(root, ExternalRootId::from_normalized_identity),
    };
    [
        fatal(DIVIDE_ERROR, 11, 1, 0x201, 0x101),
        fatal(GENERAL_PROTECTION, 12, 2, 0x202, 0x102),
        fatal(PAGE_FAULT, 13, 3, 0x203, 0x103),
        timer(TIMER_TICK, 14, 4, 0x204, 0x104),
    ]
    .into_iter()
    .map(|member| (member.profile.vector, member))
    .collect()
}

fn table_profile(identity_value: u64) -> InterruptTableProfile {
    InterruptTableProfile::new(
        identity(
            identity_value,
            InterruptTableProfileId::from_normalized_identity,
        ),
        declared_members().values().map(|member| member.profile),
    )
    .expect("declared interrupt-table profile")
}

/// The installed TSS the package validator joins through: declared IST
/// slot i provisions dedicated critical stack class 10+i.
fn declared_tss() -> X86_64InstalledTaskStateSegmentRealization {
    X86_64InstalledTaskStateSegmentRealization {
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

/// Resolve a source-level name that may carry module qualification: the
/// corpus package declares `InterruptGate` inside `cathedral`, so the
/// typed trees retain whatever qualification spelling the front end
/// assigned — match the unqualified tail exactly.
fn qualified_data_name(typed: &typed_trees::TypedTrees, suffix: &str) -> String {
    typed
        .data_definitions()
        .iter()
        .map(|data| data.name.as_str().to_owned())
        .find(|name| name == suffix || name.ends_with(&format!("::{suffix}")))
        .unwrap_or_else(|| panic!("no data definition spelling `{suffix}`"))
}

fn qualified_machine_name(typed: &typed_trees::TypedTrees, suffix: &str) -> String {
    typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str().to_owned())
        .find(|name| name == suffix || name.ends_with(&format!("::{suffix}")))
        .unwrap_or_else(|| panic!("no machine spelling `{suffix}`"))
}

fn layout_plan_for(
    typed: &typed_trees::TypedTrees,
    data_suffix: &str,
    policy_suffix: &str,
) -> LayoutPlanReport {
    let schema = qualified_data_name(typed, data_suffix);
    let policy = qualified_machine_name(typed, policy_suffix);
    compute_layout_plan(typed, &policy, &schema, None).unwrap_or_else(|diagnostic| {
        panic!("layout policy `{policy}` over `{schema}`: {diagnostic}")
    })
}

fn canary_main() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/pass/memory/interrupt_table_canary/main.omg")
}

/// Compile the authored canary and compute both validated layout plans:
/// the gate record's split-offset wire plan and the table's per-vector
/// repeated-slot plan.
fn authored_layouts() -> (LayoutPlanReport, LayoutPlanReport) {
    let checked = compile_to_checked(CheckedCompileRequest::new(&canary_main(), None))
        .expect("interrupt table canary compiles");
    let gate = layout_plan_for(&checked.typed, "InterruptGate", "InterruptGateLayout::plan");
    let table = layout_plan_for(
        &checked.typed,
        "InterruptDescriptorTable",
        "DescriptorTableLayout::plan",
    );
    (gate, table)
}

/// The member entry addresses the exact installed realization resolves:
/// entry stub i sits at code offset 16*(i+1) inside the artifact placed at
/// 0x1000. The package validator never sees these numbers — writer custody
/// seals them — but the test asserts the produced bytes carry exactly the
/// sealed addresses.
fn entry_address(entry: EntryStubId, members: &BTreeMap<u8, DeclaredMember>) -> u64 {
    let index = members
        .values()
        .position(|member| member.entry == entry)
        .expect("declared member entry") as u64;
    PLACEMENT_BASE + 16 * (index + 1)
}

/// The installed-code fixture whose artifact admits every declared
/// member's entry stub, mirroring the external-roots fixture ladder
/// (`installed_code_in_placement_with_entries`).
fn table_installed_code(members: &BTreeMap<u8, DeclaredMember>) -> InstalledCode {
    let entries = members
        .values()
        .enumerate()
        .map(|(index, member)| {
            ArtifactEntry::from_canonical_decode(member.entry, 16 * (index as u64 + 1))
        })
        .collect();
    let bytes = vec![0; 16 * (members.len() + 1)];
    let constraints = PlacementConstraints::new(
        Some(PlacementAddressRange::new(PLACEMENT_BASE, 0x1_0000).expect("range")),
        4096,
        PlacementPhase::PostHandoff,
        Some(
            layout_plans::MachineRegimeId::from_normalized_identity(0x71).expect("machine regime"),
        ),
        Some(
            layout_plans::ArtifactInstallationScopeId::from_normalized_identity(61)
                .expect("installation scope"),
        ),
    )
    .expect("placement constraints");
    let contracts = install_identity(30, MachineContractSetId::from_normalized_identity);
    let footprint = install_identity(31, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_identity(
            1,
            executable_installation::ArtifactId::from_normalized_identity,
        ),
        Architecture::X86_64,
        bytes,
        contracts,
        footprint,
        install_identity(32, PlacementPlanId::from_normalized_identity),
        constraints,
        install_identity(33, EntrySetId::from_normalized_identity),
        entries,
        install_identity(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            constraints
                .machine_regime()
                .map(|regime| (regime, b"test-machine-regime-v1".as_slice())),
            constraints
                .installation_scope()
                .map(|scope| (scope, b"test-installation-scope-v1".as_slice())),
        ),
    )
    .expect("canonical artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_identity(40, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([extent_identity(
        51,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = extent_grant(100, PLACEMENT_BASE, 4096, rights.clone());
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_identity(100, CodePlacementId::from_normalized_identity),
        install_identity(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::FutureFetcher,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: PLACEMENT_BASE,
            phase: constraints.phase(),
            machine_regime: constraints.machine_regime(),
            installation_scope: constraints.installation_scope(),
        },
    )
    .claim(extent)
    .expect("placement claim");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("artifact without relocations materializes");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_identity(71, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen placement");
    let certificate = FinalValidationCertificate::from_validator(
        install_identity(180, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &certificate).expect("validated placement");
    let install_authority = InstallAuthority::from_admitted_provider(&validated);
    let installation_receipt = InstallationReceipt::from_provider(
        install_identity(
            300,
            executable_installation::InstalledCodeId::from_normalized_identity,
        ),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, install_authority, installation_receipt).expect("installed code")
}

fn extent_grant(seed: u64, base: u64, length: u64, rights: ExtentRights) -> Extent {
    let issuance_base = seed * 16;
    ExtentRootGrant::from_admitted_provider(
        extents::ExtentProviderIssuance::from_normalized_identities([
            issuance_base + 1,
            issuance_base + 2,
            issuance_base + 3,
            issuance_base + 4,
            issuance_base + 5,
            issuance_base + 6,
            issuance_base + 7,
            issuance_base + 8,
            issuance_base + 9,
            issuance_base + 10,
            issuance_base + 11,
            issuance_base + 12,
            issuance_base + 13,
        ])
        .expect("normalized provider issuance"),
        extent_identity(seed + 1000, ExtentLineageId::from_normalized_identity),
        extent_identity(50, AddressSpaceId::from_normalized_identity),
        rights,
        extent_identity(seed + 2000, ExtentProvenanceId::from_normalized_identity),
        extent_identity(seed + 3000, MappingEraId::from_normalized_identity),
    )
    .mint(base, length)
    .expect("minted extent")
}

/// An activated owned mapping for a descriptor-table destination,
/// mirroring the external-roots writer fixture: minted source and
/// destination extents in the shared fixture space, provider activation
/// receipt, and mapped rights that the preparation receipt then requires.
fn activated_table_mapping(seed: u64, base: u64, length: u64) -> MappedExtent<'static> {
    let rights = ExtentRights::from_normalized_identities([extent_identity(
        51,
        ExtentRightId::from_normalized_identity,
    )]);
    let activation = extent_identity(
        seed + 4000,
        TranslationActivationFactId::from_normalized_identity,
    );
    let grant = MappingGrant::from_admitted_provider(
        extent_identity(seed + 5000, MappingGrantId::from_normalized_identity),
        MappingSourceMode::Owned,
        extent_identity(50, AddressSpaceId::from_normalized_identity),
        extent_identity(50, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        rights.clone(),
        rights.clone(),
        extent_identity(seed + 6000, ExtentProvenanceId::from_normalized_identity),
        extent_identity(seed + 7000, MappingEraId::from_normalized_identity),
        TranslationInstallObligations::from_normalized_facts([activation]),
        TranslationReleaseObligations::default(),
    );
    let pending = map_owned(
        extent_grant(seed + 8000, 0x20_0000, length, rights.clone()),
        extent_grant(seed + 9000, base, length, rights),
        extent_identity(seed + 10_000, MappingId::from_normalized_identity),
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
/// pinned/writable/unpublished preparation receipt, and the staged image
/// as the concrete byte view.
fn prepared_table_destination<'a>(
    seed: u64,
    base: u64,
    image: &'a mut [u8],
) -> PreparedPostHandoffWriterDestination<'static, 'a> {
    let mapping = activated_table_mapping(seed, base, image.len() as u64);
    let receipt = DestinationPreparationReceipt::from_admitted_provider(
        install_identity(
            seed + 11_000,
            DestinationPreparationReceiptId::from_normalized_identity,
        ),
        &mapping.receipt_context(),
        ExtentRights::from_normalized_identities([extent_identity(
            51,
            ExtentRightId::from_normalized_identity,
        )]),
        true,
        true,
    );
    PreparedPostHandoffWriterDestination::claim(
        mapping,
        receipt,
        PlacementSite {
            base_address: base,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: None,
        },
        image,
    )
    .expect("activated pinned writable unpublished destination")
}

/// Stage one gate slot through the gate layout: every field is semantic
/// content and `entry` stays zero — the checked writer owns it.
fn staged_gate(
    gate_layout: &LayoutPlanReport,
    member: Option<&DeclaredMember>,
) -> [u8; GATE_BYTES] {
    let descriptor = member.map(|member| member.profile.descriptor);
    let mut slot = [0_u8; GATE_BYTES];
    materialize_scalar_layout_into(
        gate_layout,
        &[
            ScalarFieldValue::new("entry", 64, 0).expect("staged entry placeholder"),
            ScalarFieldValue::new(
                "selector",
                16,
                u64::from(descriptor.map_or(0, |descriptor| descriptor.selector)),
            )
            .expect("selector"),
            ScalarFieldValue::new(
                "ist",
                3,
                u64::from(
                    descriptor
                        .and_then(|descriptor| descriptor.interrupt_stack_table_slot)
                        .unwrap_or(0),
                ),
            )
            .expect("ist slot"),
            ScalarFieldValue::new(
                "gate_kind",
                4,
                descriptor.map_or(0, |descriptor| match descriptor.gate {
                    X86_64GateKind::Interrupt => GATE_TYPE_INTERRUPT,
                    X86_64GateKind::Trap => GATE_TYPE_TRAP,
                }),
            )
            .expect("gate kind"),
            ScalarFieldValue::new(
                "privilege",
                2,
                u64::from(descriptor.map_or(0, |descriptor| descriptor.entry_privilege)),
            )
            .expect("privilege"),
            ScalarFieldValue::new("present", 1, u64::from(member.is_some())).expect("present"),
        ],
        ByteOrder::LittleEndian,
        &mut slot,
    )
    .expect("gate slot stages through its layout");
    slot
}

/// The package's staged image: each member's declared constants staged
/// through the gate layout (entry left at zero for the writer), assembled
/// into the table image through the table layout's repeated-slot copies.
/// This is the source-layout counterpart of the model's hand-rolled
/// `descriptor_table_staged_image`.
fn staged_table_image(
    gate_layout: &LayoutPlanReport,
    table_layout: &LayoutPlanReport,
    members: &BTreeMap<u8, DeclaredMember>,
) -> Vec<u8> {
    let mut elements = Vec::with_capacity(TABLE_BYTES);
    for vector in 0..TABLE_VECTORS {
        elements.extend_from_slice(&staged_gate(gate_layout, members.get(&vector)));
    }
    let mut image = vec![0_u8; TABLE_BYTES];
    materialize_aggregate_layout_into(
        table_layout,
        &[AggregateFieldSchema::new_repeated(
            "gates",
            GATE_BYTES as u64,
            8,
            u64::from(TABLE_VECTORS),
        )
        .expect("repeated gate schema")],
        &[AggregateFieldValue::new("gates", elements).expect("staged gate array")],
        ByteOrder::LittleEndian,
        &mut image,
    )
    .expect("table image stages through its layout");
    image
}

/// The package's semantic validator: replay the still-unpublished written
/// destination against the declared member set and decode every slot
/// through the gate layout — no byte offset is spelled outside the
/// validated plan. Rejections are `Err`; success mints the established
/// table value publication consumes.
#[allow(clippy::too_many_arguments)]
fn validate_descriptor_table_image(
    code: &InstalledCode,
    written: &ValidatedWrittenPostHandoffWriterDestination<'_, '_>,
    invocation: &PostHandoffWriterInvocationPlan,
    gate_layout: &LayoutPlanReport,
    profile: &InterruptTableProfile,
    members: &BTreeMap<u8, DeclaredMember>,
    tss: &X86_64InstalledTaskStateSegmentRealization,
    establishment: InterruptTableEstablishmentId,
    destination: Extent,
) -> Result<EstablishedInterruptTable, String> {
    // The produced destination must bind this exact installed realization
    // and this exact writer invocation: a table written by another writer
    // or against another artifact fails before contents are read.
    if written.installed_code() != code.identity() {
        return Err("written destination does not bind the installed code".into());
    }
    if written.artifact() != code.artifact() {
        return Err("written destination does not bind the installed artifact".into());
    }
    if !written.binds_invocation(invocation) {
        return Err("written destination was not produced by the declared writer".into());
    }
    if written.site().base_address != destination.base()
        || u64::try_from(written.bytes().len()).ok() != Some(destination.length())
    {
        return Err("written destination does not cover the declared table extent".into());
    }
    if written.bytes().len() != TABLE_BYTES {
        return Err(format!(
            "written destination holds {} bytes, the declared table spans {TABLE_BYTES}",
            written.bytes().len()
        ));
    }

    // Every declared profile member must be covered by the member set the
    // validator was asked to describe, and vice versa — a missing member is
    // a missing row, not an absent byte.
    for plan in profile.members() {
        if !members.contains_key(&plan.vector) {
            return Err(format!(
                "declared member vector {} has no covered row",
                plan.vector
            ));
        }
    }

    let decode_fields = |slot: &[u8]| -> Result<BTreeMap<String, u64>, String> {
        decode_scalar_layout(
            gate_layout,
            &gate_field_schemas(),
            ByteOrder::LittleEndian,
            slot,
        )
        .map(|values| {
            values
                .into_iter()
                .map(|value| (value.field, value.value))
                .collect()
        })
        .map_err(|diagnostic| {
            format!("gate slot does not decode through its layout: {diagnostic:?}")
        })
    };

    for vector in 0..TABLE_VECTORS {
        let start = usize::from(vector) * GATE_BYTES;
        let slot = &written.bytes()[start..start + GATE_BYTES];
        let fields = decode_fields(slot)?;
        let reserved_clean = slot[12..16].iter().all(|byte| *byte == 0);
        match members.get(&vector) {
            Some(member) => {
                let descriptor = member.profile.descriptor;
                if fields["entry"] == 0 {
                    return Err(format!("member vector {vector} carries no resolved entry"));
                }
                if fields["selector"] != u64::from(descriptor.selector) {
                    return Err(format!("member vector {vector} selector mismatch"));
                }
                if fields["ist"] != u64::from(descriptor.interrupt_stack_table_slot.unwrap_or(0)) {
                    return Err(format!("member vector {vector} ist slot mismatch"));
                }
                if fields["gate_kind"]
                    != match descriptor.gate {
                        X86_64GateKind::Interrupt => GATE_TYPE_INTERRUPT,
                        X86_64GateKind::Trap => GATE_TYPE_TRAP,
                    }
                {
                    return Err(format!("member vector {vector} gate-kind mismatch"));
                }
                if fields["privilege"] != u64::from(descriptor.entry_privilege) {
                    return Err(format!("member vector {vector} privilege mismatch"));
                }
                if fields["present"] != 1 {
                    return Err(format!("member vector {vector} is not present"));
                }
                if !reserved_clean {
                    return Err(format!("member vector {vector} reserved bytes are nonzero"));
                }
                // The declared IST slot must resolve through the installed
                // TSS to the member's dedicated critical stack class — a
                // fatal entry may never be accounted onto a stack the TSS
                // does not provision.
                let declared_slot = descriptor.interrupt_stack_table_slot.ok_or_else(|| {
                    format!("member vector {vector} declares no dedicated stack switch")
                })?;
                let dedicated = tss
                    .interrupt_stacks
                    .iter()
                    .find(|stack| stack.slot == declared_slot)
                    .ok_or_else(|| format!("installed TSS provisions no IST slot {declared_slot}"))?
                    .dedicated_class;
                if dedicated != member.profile.dedicated_stack_class {
                    return Err(format!(
                        "member vector {vector} IST slot {declared_slot} resolves to class {dedicated}, not {}",
                        member.profile.dedicated_stack_class
                    ));
                }
            }
            None => {
                if fields.values().any(|value| *value != 0) || !reserved_clean {
                    return Err(format!(
                        "undeclared vector {vector} carries nonzero gate content"
                    ));
                }
            }
        }
    }

    EstablishedInterruptTable::from_consumer(
        establishment,
        profile,
        code.identity(),
        code.artifact(),
        members.iter().map(|(vector, member)| {
            (
                *vector,
                InterruptTableEstablishedMember {
                    root: member.root,
                    entry: member.entry,
                },
            )
        }),
        destination,
    )
    .map_err(|diagnostic| format!("established table value rejected: {diagnostic:?}"))
}

/// Decode one produced slot's `entry` through the gate layout — the same
/// vocabulary the validator uses — for the byte-level assertion that the
/// sealed writer materialized the exact installed address.
fn decoded_gate_entry(gate_layout: &LayoutPlanReport, slot: &[u8]) -> u64 {
    decode_scalar_layout(
        gate_layout,
        &gate_field_schemas(),
        ByteOrder::LittleEndian,
        slot,
    )
    .expect("entry decodes through the layout")
    .into_iter()
    .find(|value| value.field == "entry")
    .map(|value| value.value)
    .expect("decoded entry")
}

/// Every gate field's decode schema: the decoder refuses a layout whose
/// declared fields are not all covered.
fn gate_field_schemas() -> [ScalarFieldSchema; 6] {
    [
        ScalarFieldSchema::new("entry", 64).expect("entry schema"),
        ScalarFieldSchema::new("selector", 16).expect("selector schema"),
        ScalarFieldSchema::new("ist", 3).expect("ist schema"),
        ScalarFieldSchema::new("gate_kind", 4).expect("gate-kind schema"),
        ScalarFieldSchema::new("privilege", 2).expect("privilege schema"),
        ScalarFieldSchema::new("present", 1).expect("present schema"),
    ]
}

/// Assemble the checked destination pipeline for one staged image:
/// seal-and-validate the writer context, claim the activated pinned
/// destination, execute the writer, and return the validated written
/// carrier the package validator consumes.
fn written_table<'a>(
    code: &InstalledCode,
    plan: &layout_plans::PostHandoffWriterPlan,
    base: u64,
    image: &'a mut [u8],
) -> ValidatedWrittenPostHandoffWriterDestination<'static, 'a> {
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

fn table_destination_extent(seed: u64, base: u64, length: u64) -> Extent {
    extent_grant(
        seed,
        base,
        length,
        ExtentRights::from_normalized_identities([extent_identity(
            51,
            ExtentRightId::from_normalized_identity,
        )]),
    )
}

#[test]
fn authored_table_layout_places_the_declared_wire_shape() {
    let (gate, table) = authored_layouts();

    // The gate policy's plan is exactly the long-mode descriptor encoding:
    // the 64-bit `entry` tiles three disjoint containers and the flag byte
    // keeps its reserved bits unplaced.
    assert_eq!(gate.size, Some(16));
    assert_eq!(gate.align, 8);
    let gate_entries = gate
        .entries
        .iter()
        .map(|entry| (entry.field.as_str(), entry.placement))
        .collect::<Vec<_>>();
    assert_eq!(
        gate_entries,
        vec![
            (
                "entry",
                LayoutPlacementReport::Bits {
                    container: 0,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 16
                }
            ),
            (
                "entry",
                LayoutPlacementReport::Bits {
                    container: 6,
                    container_width: 16,
                    destination_lsb: 0,
                    source_lsb: 16,
                    width: 16
                }
            ),
            (
                "entry",
                LayoutPlacementReport::Bits {
                    container: 8,
                    container_width: 32,
                    destination_lsb: 0,
                    source_lsb: 32,
                    width: 32
                }
            ),
            ("selector", LayoutPlacementReport::At { offset: 2 }),
            (
                "ist",
                LayoutPlacementReport::Bits {
                    container: 4,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 3
                }
            ),
            (
                "gate_kind",
                LayoutPlacementReport::Bits {
                    container: 5,
                    container_width: 8,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 4
                }
            ),
            (
                "privilege",
                LayoutPlacementReport::Bits {
                    container: 5,
                    container_width: 8,
                    destination_lsb: 5,
                    source_lsb: 0,
                    width: 2
                }
            ),
            (
                "present",
                LayoutPlacementReport::Bits {
                    container: 5,
                    container_width: 8,
                    destination_lsb: 7,
                    source_lsb: 0,
                    width: 1
                }
            ),
        ]
    );

    // The table policy places every vector's slot at its stride offset.
    assert_eq!(table.size, Some(TABLE_BYTES as u64));
    assert_eq!(table.align, 8);
    let gate_placements = table
        .entries
        .iter()
        .map(|entry| {
            assert_eq!(entry.field, "gates");
            entry.placement
        })
        .collect::<Vec<_>>();
    assert_eq!(gate_placements.len(), usize::from(TABLE_VECTORS));
    for (vector, placement) in gate_placements.iter().enumerate() {
        assert_eq!(
            *placement,
            LayoutPlacementReport::At {
                offset: vector as u64 * GATE_BYTES as u64
            }
        );
    }
}

#[test]
fn authored_descriptor_table_materializes_through_checked_writer() {
    let (gate_layout, table_layout) = authored_layouts();
    let members = declared_members();
    let profile = table_profile(0x600);
    let code = table_installed_code(&members);

    // Bind every declared member's sealed entry stub to its vector's
    // `gates[v].entry` record path. The inner-layout carrier is the gate's
    // own validated plan, so the split-offset fragments the writer derives
    // are exactly the authored placements.
    let interior = SymbolicFieldInnerLayout::new_record_array(
        "gates",
        gate_layout.clone(),
        u64::from(TABLE_VECTORS),
        GATE_BYTES as u64,
    );
    let symbolics = members
        .values()
        .map(|member| {
            SymbolicFieldValue::new_indexed(
                "gates",
                u64::from(member.profile.vector),
                64,
                RelocationTarget::Entry(member.entry),
            )
            .expect("indexed gate entry symbolic")
            .with_inner_segment(SymbolicFieldPathSegment::new("entry"))
        })
        .collect::<Vec<_>>();
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &table_layout,
        std::slice::from_ref(&interior),
        &symbolics,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("the authored table layout derives member entry materialization");
    assert_eq!(materialization.byte_len, TABLE_BYTES);

    // Three split-offset fragments per member: the writer's program is the
    // authored `Bits` placements, nothing hand-rolled.
    assert_eq!(
        materialization.actions.len(),
        members.len() * 3,
        "every member's entry derives its three authored split-offset writes"
    );
    for member in members.values() {
        for (container, width, source_lsb) in [(0_u64, 16_u16, 0_u16), (6, 16, 16), (8, 32, 32)] {
            let offset = u64::from(member.profile.vector) * GATE_BYTES as u64 + container;
            let action = materialization
                .actions
                .iter()
                .find(|action| match action {
                    MaterializationAction::RuntimeWriter(write) => {
                        write.container_byte_offset == offset
                            && write.target == RelocationTarget::Entry(member.entry)
                    }
                    _ => false,
                })
                .unwrap_or_else(|| {
                    panic!(
                        "member vector {} derives its {source_lsb}-bit fragment at {offset}",
                        member.profile.vector
                    )
                });
            let MaterializationAction::RuntimeWriter(write) = action else {
                unreachable!("filtered above")
            };
            assert_eq!(write.width, width);
            assert_eq!(write.source_lsb, source_lsb);
            assert_eq!(write.destination_lsb, 0);
        }
    }

    let writer = materialization
        .derive_post_handoff_writer()
        .expect("the entry fragments derive the post-handoff writer");
    let invocation = writer
        .lower_reusable_fragment()
        .expect("writer lowers to its reusable invocation");

    // The package stages the image through its own layouts; the checked
    // writer then seals member entry addresses into the produced slots.
    let mut image = staged_table_image(&gate_layout, &table_layout, &members);
    let written = written_table(&code, &writer, TABLE_BASE, &mut image);
    let bytes = written.bytes().to_vec();

    // Byte-level witness: each member's produced slot carries exactly the
    // entry address its installed realization resolved — decoded through
    // the authored layout — and non-member slots stayed all-zero.
    for member in members.values() {
        let start = usize::from(member.profile.vector) * GATE_BYTES;
        assert_eq!(
            decoded_gate_entry(&gate_layout, &bytes[start..start + GATE_BYTES]),
            entry_address(member.entry, &members),
            "member vector {} carries its sealed entry address",
            member.profile.vector
        );
    }

    // The package validator produces the established table value over the
    // same produced bytes.
    let destination = table_destination_extent(0x772, TABLE_BASE, TABLE_BYTES as u64);
    let established = validate_descriptor_table_image(
        &code,
        &written,
        &invocation,
        &gate_layout,
        &profile,
        &members,
        &declared_tss(),
        identity(
            0x771,
            InterruptTableEstablishmentId::from_normalized_identity,
        ),
        destination,
    )
    .expect("the package validator accepts the produced descriptor table");
    assert_eq!(established.destination().base(), TABLE_BASE);
    assert_eq!(
        established.establishment(),
        identity(
            0x771,
            InterruptTableEstablishmentId::from_normalized_identity
        )
    );

    // The same writer lowers to native code on both Linux ISAs and, on a
    // matching host, executes to the same produced bytes: a zero-filled
    // destination receives exactly the sealed entry fragments.
    let mut writer_only_image = vec![0_u8; TABLE_BYTES];
    for member in members.values() {
        let address = entry_address(member.entry, &members);
        let start = usize::from(member.profile.vector) * GATE_BYTES;
        writer_only_image[start..start + 2].copy_from_slice(&(address as u16).to_le_bytes());
        writer_only_image[start + 6..start + 8]
            .copy_from_slice(&((address >> 16) as u16).to_le_bytes());
        writer_only_image[start + 8..start + 12]
            .copy_from_slice(&((address >> 32) as u32).to_le_bytes());
    }
    super::lower_writer_on_both_linux_isas(&writer, 0, &writer_only_image, |target| match target {
        RelocationTarget::Entry(entry) => entry_address(entry, &members),
        RelocationTarget::Data(_) => panic!("the table writer resolves no data symbol"),
    });
}

#[test]
fn descriptor_table_validator_rejects_content_violations() {
    let (gate_layout, table_layout) = authored_layouts();
    let members = declared_members();
    let profile = table_profile(0x600);
    let code = table_installed_code(&members);
    let interior = SymbolicFieldInnerLayout::new_record_array(
        "gates",
        gate_layout.clone(),
        u64::from(TABLE_VECTORS),
        GATE_BYTES as u64,
    );
    let symbolics = members
        .values()
        .map(|member| {
            SymbolicFieldValue::new_indexed(
                "gates",
                u64::from(member.profile.vector),
                64,
                RelocationTarget::Entry(member.entry),
            )
            .expect("indexed gate entry symbolic")
            .with_inner_segment(SymbolicFieldPathSegment::new("entry"))
        })
        .collect::<Vec<_>>();
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &table_layout,
        std::slice::from_ref(&interior),
        &symbolics,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("member entry materialization");
    let writer = materialization
        .derive_post_handoff_writer()
        .expect("post-handoff writer");
    let invocation = writer.lower_reusable_fragment().expect("invocation");
    let establishment = identity(
        0x771,
        InterruptTableEstablishmentId::from_normalized_identity,
    );

    // A declared member whose staged slot never became present — the
    // writer still seals its entry, but the package's semantic validator
    // refuses the table.
    let mut image = staged_table_image(&gate_layout, &table_layout, &members);
    image[usize::from(TIMER_TICK) * GATE_BYTES + 5] &= !0x80;
    let written = written_table(&code, &writer, TABLE_BASE, &mut image);
    let error = validate_descriptor_table_image(
        &code,
        &written,
        &invocation,
        &gate_layout,
        &profile,
        &members,
        &declared_tss(),
        establishment,
        table_destination_extent(0x773, TABLE_BASE, TABLE_BYTES as u64),
    )
    .expect_err("a member whose produced gate is not present cannot establish");
    assert!(error.contains("present"), "unexpected refusal: {error}");

    // An undeclared vector carrying nonzero gate content rejects: the
    // table may not describe members the profile never declared.
    let mut image = staged_table_image(&gate_layout, &table_layout, &members);
    let foreign = staged_gate(
        &gate_layout,
        Some(&DeclaredMember {
            profile: InterruptTableMemberPlan {
                vector: 7,
                dedicated_stack_class: 15,
                obligation: InterruptTableObligation::FatalException,
                descriptor: InterruptTableGateDescriptor {
                    gate: X86_64GateKind::Trap,
                    selector: SELECTOR,
                    entry_privilege: 0,
                    interrupt_stack_table_slot: Some(5),
                },
            },
            entry: members[&DIVIDE_ERROR].entry,
            root: members[&DIVIDE_ERROR].root,
        }),
    );
    image[7 * GATE_BYTES..8 * GATE_BYTES].copy_from_slice(&foreign);
    let written = written_table(&code, &writer, TABLE_BASE, &mut image);
    let error = validate_descriptor_table_image(
        &code,
        &written,
        &invocation,
        &gate_layout,
        &profile,
        &members,
        &declared_tss(),
        establishment,
        table_destination_extent(0x774, TABLE_BASE, TABLE_BYTES as u64),
    )
    .expect_err("an undeclared vector cannot carry gate content");
    assert!(error.contains("undeclared"), "unexpected refusal: {error}");
}

// ---------------------------------------------------------------------------
// The published leg: installed-member ledgers and the checked `lidt` edge.
//
// The root-installation fixtures below replicate the external-roots crate's
// test-only candidate shapes over its public API — no source-spelled root
// installation exists yet, so these are test-constructed admitted members of
// the compiler-owned `InstalledRootLedger`. The table they arm is still the
// package's authored layout: the established value the carrier carries is the
// one `validate_descriptor_table_image` minted over writer-produced bytes.
// ---------------------------------------------------------------------------

/// One member's boundary: the interrupt-return realization arriving on its
/// dedicated critical stack class. Table members take distinct classes; the
/// nesting ceiling stays masked so a fatal entry never preempts into itself.
fn member_boundary(stack_class: u16) -> ValidatedBoundaryEntryPlan {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let ordinary = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary x86 boundary");
    let mut call = ordinary.plan().call.clone();
    call.ordinary_clobbers = RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ]);
    call.entry_control = EntryControl::InterruptReturn;
    let interrupted_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::VectorRegisters,
    ]);
    let saved_state = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime: MachineRegime::X86Long64,
                interrupted_state,
                saved_state,
                restored_state: saved_state,
                permitted_transitive_use: MachineStateSet::new([
                    MachineState::GeneralRegisters,
                    MachineState::Flags,
                ]),
                stack: EntryStack::Dedicated { class: stack_class },
                preemption: Preemption::Masked,
            },
        },
        &signature,
    )
    .expect("interrupt-return boundary on the dedicated class")
}

/// The timer member's selected completion provider — the ordinary interrupt
/// acknowledgement reach closes through the PIC-shaped plan row.
fn selected_interrupt_completion() -> effects::SelectedProviderPlanFacts {
    let requirement_identity = "InterruptCompletion::complete".to_owned();
    let plan = effects::provider_plan::ProviderPlan {
        name: "LegacyPic".into(),
        provider_type: "LegacyPicController".into(),
        provider_type_package_identity: None,
        target: "x86_64-unknown-none".into(),
        schema: effects::provider_plan::ServiceSchema {
            trait_name: "InterruptCompletion".into(),
            trait_package_identity: None,
            methods: vec![effects::provider_plan::ServiceMethod {
                name: "complete".into(),
                requirement_owner: "InterruptCompletion".into(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement_identity.clone(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["InterruptCompletion".into()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![effects::provider_plan::ProviderPlanRow {
            method: "complete".into(),
            requirement_identity: requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: effects::provider_plan::ProviderBinding::CheckedAdapter {
                machine_identity: "LegacyPicController::complete".into(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    };
    let identity = plan.report_fingerprint();
    effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("selected interrupt completion provider")
    .with_installation_reach_resolutions(vec![effects::InstallationReachResolution {
        requirement_identity,
        provider_plan_report_identity: identity,
        upper_bound: vec!["PortIo".into(), "MachineControl".into()],
        resolved_row: vec!["PortIo".into()],
    }])
    .expect("provider reach refines the interrupt completion bound")
}

fn fixed_fuel() -> external_roots::ComposedFuelDemand {
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        identity(31, ProviderFuelSummaryId::from_normalized_identity),
        identity(12, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        5,
        BTreeSet::new(),
        identity(
            41,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let root = FixedFuelProviderSummary::from_admitted_provider(
        identity(30, ProviderFuelSummaryId::from_normalized_identity),
        identity(2, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        2,
        BTreeSet::from([FixedFuelCall {
            callee: leaf.identity,
            maximum_invocations: 1,
        }]),
        identity(
            40,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    compose_fixed_fuel(root.identity, [&root, &leaf]).expect("fixed-fuel composition")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(0x55).expect("fuel schedule")
}

/// One member's bound epoch input: one Body-stage epoch on its dedicated
/// stack domain inside the artifact-wide stack composition the ledger
/// requires all installed roots of one artifact to share.
fn stack_demand_input(
    root: ExternalRootId,
    provider: RootProviderId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
    resolved_stack: EntryStack,
) -> BoundEpochStackCompositionInput {
    let active_domain = StackDomainRef::from(resolved_stack);
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain,
                occupancy_by_domain: Vec::new(),
                nesting: boundary.plan().state.preemption,
            }],
        }],
    })
    .expect("epoch realization");
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        2048,
        16,
        identity(49, StackValidationReceiptId::from_normalized_identity),
    );
    let contexts = admit_opaque_arrival_context_set(
        &summary,
        boundary,
        code,
        entry,
        [ArrivalContextId::new(1).expect("arrival context")]
            .into_iter()
            .collect(),
        identity(48, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("admitted opaque arrival-context closure");
    bind_opaque_adapter_stack_realization(&summary, boundary, code, entry, realization, contexts)
        .expect("epoch evidence binding")
}

/// One member's candidate record: the compiler-ledger's validated root shape,
// whose entry stub is exactly the sealed entry the writer materializes.
fn member_candidate(
    member: &DeclaredMember,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
) -> ExternalRootCandidate {
    let root = member.root;
    let provider = identity(2, RootProviderId::from_normalized_identity);
    let nesting_relation = identity(6, NestingRelationId::from_normalized_identity);
    let acknowledged = member.profile.obligation == InterruptTableObligation::AcknowledgedInterrupt;
    ExternalRootCandidate {
        identity: root,
        entry: member.entry,
        provider,
        provider_plan: identity(55, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: effects::provider_plan::ProviderPlan::default().identity_digest(),
        requirement_identity: if acknowledged {
            "TimerRoot::tick".into()
        } else {
            "FatalExceptionRoot::enter".into()
        },
        entry_claims: if acknowledged {
            vec![ExternalRootEntryClaim {
                parameter_index: 0,
                domain: "InterruptAcknowledgement::Pending".into(),
                effective_carry: language_semantics::CarryPolicy::STRICT,
            }]
        } else {
            Vec::new()
        },
        acknowledgement_parameter_index: acknowledged.then_some(0),
        interrupt_mask_guard_claim: Some(ExternalRootResultClaim {
            provider_plan: identity(56, ProviderPlanId::from_normalized_identity),
            provider_plan_digest: effects::provider_plan::ProviderPlan::default().identity_digest(),
            requirement_identity: "InterruptMaskControl::save_and_mask".into(),
            domain: "InterruptMaskGuard::Active".into(),
            effective_carry: language_semantics::CarryPolicy::STRICT,
        }),
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            vec!["InterruptCompletion::complete".into()],
            &selected_interrupt_completion(),
        )
        .expect("selected completion closes the installed interrupt reach"),
        effects: [identity(3, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [identity(4, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation,
        acknowledgement_policy: acknowledged
            .then(|| identity(7, AcknowledgementPolicyId::from_normalized_identity)),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: compose_bound_entry_stack_epochs(
                &StackNestingRelation {
                    identity: nesting_relation,
                    edges: BTreeSet::new(),
                },
                [stack_demand_input(
                    root,
                    provider,
                    boundary,
                    code,
                    member.entry,
                    boundary.plan().state.stack,
                )]
                .iter(),
            )
            .expect("bound epoch stack composition"),
            validation_receipt: identity(50, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
            provision: identity(53, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: fixed_fuel(),
            validation_receipt: identity(51, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: identity(52, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: [ComponentVersionPin {
            contract: identity(8, ComponentContractId::from_normalized_identity),
            artifact: identity(9, ComponentArtifactId::from_normalized_identity),
            provider: identity(10, ComponentProviderId::from_normalized_identity),
            version: identity(11, ComponentVersionPinId::from_normalized_identity),
        }]
        .into_iter()
        .collect(),
    }
}

/// Build every member's validated root over the one artifact-wide stack
/// composition the ledger requires, then install them all.
fn installed_member_roots<'code>(
    ledger: &mut InstalledRootLedger,
    code: &'code InstalledCode,
    members: &BTreeMap<u8, DeclaredMember>,
) -> Vec<InstalledExternalRoot<'code>> {
    let mut inputs = Vec::with_capacity(members.len());
    let mut shaped = Vec::with_capacity(members.len());
    for member in members.values() {
        let boundary = member_boundary(member.profile.dedicated_stack_class);
        let candidate = member_candidate(member, &boundary, code);
        let input = stack_demand_input(
            candidate.identity,
            candidate.provider,
            &boundary,
            code,
            member.entry,
            EntryStack::Dedicated {
                class: member.profile.dedicated_stack_class,
            },
        );
        inputs.push(input);
        shaped.push((candidate, boundary));
    }
    let relation = StackNestingRelation {
        identity: shaped
            .first()
            .expect("at least one table member")
            .0
            .nesting_relation,
        edges: BTreeSet::new(),
    };
    let composition =
        compose_bound_entry_stack_epochs(&relation, inputs.iter()).expect("shared composition");
    let validated = shaped
        .into_iter()
        .map(|(mut candidate, boundary)| {
            candidate.stack.realization = composition.clone();
            (
                validate_external_root(candidate, &boundary).expect("member root plan"),
                boundary,
            )
        })
        .collect::<Vec<_>>();

    validated
        .into_iter()
        .enumerate()
        .map(|(index, (root, _boundary))| {
            let authority = RootSlotAuthority::from_admitted_owner(
                identity(0x300 + index as u64, RootSlotId::from_normalized_identity),
                identity(0x21, RootSlotOwnerId::from_normalized_identity),
            );
            let execution = ProviderExecution::from_admitted_provider(
                identity(
                    0x400 + index as u64,
                    ProviderExecutionId::from_normalized_identity,
                ),
                &root,
                Some(OpaqueProviderExitAssurance::AcceptedClaim {
                    realization: ProviderExitRealization {
                        control: root.boundary().call.entry_control,
                        restored_state: root.boundary().state.restored_state,
                    },
                    validation_receipt: identity(4, TrustReceiptId::from_normalized_identity),
                }),
            )
            .expect("admitted provider exit");
            let admission = RootAdmission::from_admitted_provider(
                identity(
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

/// The minted pseudo-descriptor operand for `lidt`: a separate 10-byte read
/// site in the same fixture address space naming the established extent.
fn descriptor_operand(seed: u64, destination: &Extent) -> InterruptTableDescriptorOperand {
    InterruptTableDescriptorOperand::from_provider(
        table_destination_extent(
            seed,
            0x9_0000,
            external_roots::INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES,
        ),
        u16::try_from(destination.length() - 1).expect("pseudo-descriptor limit"),
        destination.base(),
    )
}

#[test]
fn established_table_publishes_through_the_checked_lidt_edge() {
    let (gate_layout, table_layout) = authored_layouts();
    let members = declared_members();
    let profile = table_profile(0x600);
    let mut code = table_installed_code(&members);

    // Install every member root into the artifact's ledger and admit them
    // under the declared profile — the same custody the compiled board
    // model requires before any `lidt` publication may be issued.
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("canonical root ledger");
    let handles = installed_member_roots(&mut ledger, &code, &members);
    let mut table = InterruptTableLedger::new(profile.clone(), &ledger);
    for (member, handle) in members.values().zip(handles) {
        table
            .admit_interrupt_table_member(&ledger, member.profile.vector, handle)
            .expect("admitted interrupt-table member");
    }
    assert!(table.is_complete());

    // Stage the authored image and drive the checked writer — identical to
    // the materialization witness — then let the package validator mint the
    // established value the publication carrier consumes.
    let interior = SymbolicFieldInnerLayout::new_record_array(
        "gates",
        gate_layout.clone(),
        u64::from(TABLE_VECTORS),
        GATE_BYTES as u64,
    );
    let symbolics = members
        .values()
        .map(|member| {
            SymbolicFieldValue::new_indexed(
                "gates",
                u64::from(member.profile.vector),
                64,
                RelocationTarget::Entry(member.entry),
            )
            .expect("indexed gate entry symbolic")
            .with_inner_segment(SymbolicFieldPathSegment::new("entry"))
        })
        .collect::<Vec<_>>();
    let materialization = derive_symbolic_materialization_with_inner_layouts(
        &table_layout,
        std::slice::from_ref(&interior),
        &symbolics,
        MaterializationContext {
            consumption: ConsumptionInstant::AfterOmegaHandoff,
            byte_order: ByteOrder::LittleEndian,
            native_pointer_relocation_bits: Some(64),
            placement: PlacementConstraints::unconstrained(PlacementPhase::PostHandoff),
        },
        |_| None,
    )
    .expect("member entry materialization");
    let writer = materialization
        .derive_post_handoff_writer()
        .expect("post-handoff writer");
    let invocation = writer.lower_reusable_fragment().expect("invocation");
    let mut image = staged_table_image(&gate_layout, &table_layout, &members);
    let written = written_table(&code, &writer, TABLE_BASE, &mut image);
    let destination = table_destination_extent(0x881, TABLE_BASE, TABLE_BYTES as u64);
    let established = validate_descriptor_table_image(
        &code,
        &written,
        &invocation,
        &gate_layout,
        &profile,
        &members,
        &declared_tss(),
        identity(
            0x771,
            InterruptTableEstablishmentId::from_normalized_identity,
        ),
        destination,
    )
    .expect("the produced image establishes the declared table");

    // Issue the carrier over the validator-produced value, exercise the
    // checked `lidt` provider edge under the exact authority and operand,
    // and complete the publication.
    let authority = InterruptTablePublicationAuthority::from_consumer(
        identity(
            0x640,
            InterruptTablePublicationAuthorityId::from_normalized_identity,
        ),
        ledger.installed_code(),
        ledger.artifact(),
        [
            InterruptTablePublicationScope::ProcessorTableControl,
            InterruptTablePublicationScope::TablePublication,
        ]
        .into_iter(),
    )
    .expect("publication authority");
    let carrier = table
        .begin_interrupt_table_publication(
            &ledger,
            established,
            identity(0x630, InterruptTablePublicationId::from_normalized_identity),
            identity(
                0x640,
                InterruptTablePublicationAuthorityId::from_normalized_identity,
            ),
        )
        .expect("the complete declared member set issues the carrier");
    let operand = descriptor_operand(0x650, carrier.destination());
    let answer = carrier
        .execute_checked_publication(
            &code,
            &authority,
            operand,
            identity(
                0x660,
                InterruptTablePublicationReceiptId::from_normalized_identity,
            ),
            true,
        )
        .expect("the exact authority and operand publish the table");

    // The provider answer accounts for the contract: the operand read is
    // retained, the `lidt` staging-register clobber is recorded, and the
    // installed register state names exactly the published destination.
    assert_eq!(answer.scratch_clobber(), MachineRegister::X86R10);
    assert!(answer.is_published());
    assert_eq!(answer.operand().site().base(), 0x9_0000);
    let state = answer.installed_state().expect("published register state");
    assert_eq!(state.base(), TABLE_BASE);
    assert_eq!(state.limit(), (TABLE_BYTES - 1) as u16);

    let (carrier, receipt, _operand, _state) = answer.into_parts();
    let outcome = table
        .complete_interrupt_table_publication(&ledger, carrier, receipt)
        .expect("the exact provider receipt publishes the table");
    let InterruptTablePublicationOutcome::Published(published) = outcome else {
        panic!("a refused publication cannot complete as published")
    };
    assert_eq!(
        published.publication(),
        identity(0x630, InterruptTablePublicationId::from_normalized_identity)
    );
    assert_eq!(
        published.establishment(),
        identity(
            0x771,
            InterruptTableEstablishmentId::from_normalized_identity
        )
    );
    assert_eq!(published.members().len(), members.len());
    assert_eq!(published.destination().base(), TABLE_BASE);
}

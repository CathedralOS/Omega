//! The authored-route leg of secondary-processor startup: Cathedral's
//! source-level startup contract drives the compiler's startup ledger end
//! to end.
//!
//! `tests/omega/pass/memory/secondary_processor_canary` carries the package
//! surface — `cathedral::processor_startup` authors the linear
//! `StartupEnvelope` contract (dispatch establishes `Pending`; the arriving
//! processor resolves it through `StartupEnvelope::confirm`), the per-slot
//! `Calling` policies that place each processor's arrivals on its own
//! dedicated critical-stack class, and the physical carrier representation;
//! `cathedral::roster` authors the machine's declared roster (per-processor
//! stack class/demand/state) plus the provider-declared startup geometry —
//! trampoline bytes, alignment, and the low-memory bound. This module
//! compiles the package, evaluates the authored declaration at build time,
//! and drives the compiler-owned `SecondaryProcessorStartupLedger`:
//! trampoline bind against the installed artifact, account admission,
//! dispatch, acknowledgement, settlement, and retirement.
//!
//! The ledger checks installed entry, placement, resources, and evidence —
//! never an APIC or firmware driver. What stays fixture on this seam: the
//! sealed entry/occurrence/invocation identities, the provider receipts
//! (the ledger's consumed evidence, minted `from_provider` against the exact
//! outstanding carrier), and the arrival machine-regime identity the
//! profile binds to the artifact's placement constraints.

use calling_conventions::{
    EntryControl, EntryStack, MachineRegime, Preemption, ValidatedBoundaryEntryPlan,
};
use checked_interpreter::{
    BuildMachineEvaluationRequest, BuildTimeValue, evaluate_build_time_machine,
};
use compiler::{CheckedCompileRequest, compile_to_checked};
use executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, CodePlacementAuthority,
    CodePlacementId, FinalValidationCertificate, FinalValidationId, InstallAuthority,
    InstallationAudience, InstallationReceipt, InstallationScopeId, InstalledCode,
    MachineContractSetId, MachineFootprintId, MaterializationReceipt, PlacementPlanId,
    RelocationSetId, WxEnforcement, admit_executable, install_validated,
    materialize_admitted_artifact, materialize_and_freeze, validate_final_placement,
};
use extents::{
    AddressSpaceId, Extent, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use external_roots::{
    ExternalRootDiagnostic, InstalledSecondaryProcessorTrampoline, SecondaryProcessorAccount,
    SecondaryProcessorOccurrenceId, SecondaryProcessorQuiescenceOutcome,
    SecondaryProcessorQuiescenceReceipt, SecondaryProcessorQuiescenceReceiptId,
    SecondaryProcessorSettlementOutcome, SecondaryProcessorSettlementReceipt,
    SecondaryProcessorSettlementReceiptId, SecondaryProcessorStartupInvocationId,
    SecondaryProcessorStartupLedger, SecondaryProcessorStartupOutcome,
    SecondaryProcessorStartupProfile, SecondaryProcessorStartupProfileId,
    SecondaryProcessorStartupReceipt, SecondaryProcessorStartupReceiptId,
    SecondaryProcessorStartupVerdict, bind_secondary_processor_trampoline,
};
use layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, PlacementSite,
};
use provider_planning::selected_external_root_provider_plan;
use std::path::{Path, PathBuf};
use target::Architecture;

// The fixture identities the authored package deliberately does not name:
// the sealed startup entry stub and the processor occurrence / invocation /
// receipt identities are compiler-ledger vocabulary; the arrival machine
// regime is the mechanism constant the artifact's placement constraints and
// the provider-declared profile must agree on.
const STARTUP_ENTRY_SEED: u64 = 0x700;
const ARRIVAL_REGIME_SEED: u64 = 0x71;
const INSTALLATION_SCOPE_SEED: u64 = 61;

// The startup trampoline's installed geometry, inside the authored
// low-memory bound.
const TRAMPOLINE_BASE: u64 = 0x8_0000;
const TRAMPOLINE_LENGTH: u64 = 0x1000;
const TRAMPOLINE_RANGE_START: u64 = 0x1_0000;

// The sealed inputs the emitted trampoline binds: provider/supply-side
// installation facts the ledger route hands to emission (the shared
// page-table root, the account's provisioned stack top, and the installed
// semantic entry the trampoline reaches). Normalized fixture values stand
// in for them until the boundary install route supplies them.
const TRAMPOLINE_PAGE_TABLE_ROOT: u64 = 0x70_0000;
const TRAMPOLINE_STACK_TOP: u64 = 0x9_0000;
const TRAMPOLINE_SEMANTIC_ENTRY: u64 = 0x10_0000;

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

fn struct_field<'a>(fields: &'a [(String, BuildTimeValue)], name: &str) -> &'a BuildTimeValue {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("authored record carries no field `{name}`"))
}

fn int_field(fields: &[(String, BuildTimeValue)], name: &str) -> i64 {
    let BuildTimeValue::Int(value) = struct_field(fields, name) else {
        panic!("authored field `{name}` is not an integer")
    };
    *value
}

fn record_fields<'a>(value: &'a BuildTimeValue, what: &str) -> &'a [(String, BuildTimeValue)] {
    let BuildTimeValue::Struct { fields, .. } = value else {
        panic!("{what} evaluated to {value:?}, not a record")
    };
    fields
}

fn qualified_machine_name(typed: &typed_trees::TypedTrees, suffix: &str) -> String {
    typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str().to_owned())
        .find(|name| name == suffix || name.ends_with(&format!("::{suffix}")))
        .unwrap_or_else(|| panic!("no machine spelling `{suffix}`"))
}

fn canary_main() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/pass/memory/secondary_processor_canary/main.omg")
}

/// One declared secondary-processor slot exactly as the canary's
/// `cathedral::roster::SecondaryProcessorStartup::declare` returned it.
#[derive(Debug, Clone, Copy)]
struct AuthoredSlot {
    stack_class: u16,
    stack_bytes: u64,
    stack_alignment: u64,
    state_bytes: u64,
}

/// The evaluated authored startup declaration: the per-processor roster
/// rows and the provider-declared profile geometry — the startup-vector
/// alignment and the low-memory bound. The trampoline's byte content is
/// compiler-emitted, not authored.
struct AuthoredStartup {
    slots: Vec<AuthoredSlot>,
    startup_alignment: u64,
    low_memory_limit: u64,
}

fn authored_startup(typed: &typed_trees::TypedTrees) -> AuthoredStartup {
    let machine = qualified_machine_name(typed, "SecondaryProcessorStartup::declare");
    let value = evaluate_build_time_machine(
        typed,
        BuildMachineEvaluationRequest::named(&machine, Vec::new()),
    )
    .unwrap_or_else(|reason| panic!("the authored startup declaration does not evaluate: {reason}"))
    .into_value();
    let startup = record_fields(&value, "SecondaryProcessorStartup::declare");
    let BuildTimeValue::Array(slot_rows) = struct_field(startup, "processors") else {
        panic!("the authored startup declaration carries no processor array")
    };
    let slots = slot_rows
        .iter()
        .map(|row| {
            let fields = record_fields(row, "declared processor slot");
            AuthoredSlot {
                stack_class: u16::try_from(int_field(fields, "stack_class"))
                    .expect("declared stack class"),
                stack_bytes: u64::try_from(int_field(fields, "stack_bytes"))
                    .expect("declared stack bytes"),
                stack_alignment: u64::try_from(int_field(fields, "stack_alignment"))
                    .expect("declared stack alignment"),
                state_bytes: u64::try_from(int_field(fields, "state_bytes"))
                    .expect("declared state bytes"),
            }
        })
        .collect();
    AuthoredStartup {
        slots,
        startup_alignment: u64::try_from(int_field(startup, "startup_alignment"))
            .expect("declared startup alignment"),
        low_memory_limit: u64::try_from(int_field(startup, "low_memory_limit"))
            .expect("declared low-memory bound"),
    }
}

/// The startup trampoline's emitted content: the canonical compiler recipe
/// sealed against the realized placement base and the contract's sealed
/// inputs, replayed before installation. These seals model the
/// provider/supply-side values the install route delivers; the authored
/// declaration carries only the contract geometry (alignment, bound).
fn emitted_trampoline_bytes() -> Vec<u8> {
    let template = machine_emission::emit_x86_64_startup_trampoline();
    machine_emission::resolve_x86_64_startup_trampoline(
        &template,
        machine_emission::X86_64StartupTrampolineResolution {
            placement_base: TRAMPOLINE_BASE,
            page_table_root: TRAMPOLINE_PAGE_TABLE_ROOT,
            stack_top: TRAMPOLINE_STACK_TOP,
            entry: TRAMPOLINE_SEMANTIC_ENTRY,
        },
    )
    .expect("startup trampoline resolution")
    .bytes()
    .to_vec()
}

/// The installed trampoline artifact: the emitted trampoline bytes, one
/// admitted startup entry at code offset 0, placement constrained to the
/// declared low-memory window in the arrival machine regime. Mirrors the
/// external-roots fixture ladder (`installed_code_in_placement`).
fn startup_trampoline_code(authored: &AuthoredStartup) -> InstalledCode {
    let constraints = PlacementConstraints::new(
        Some(
            PlacementAddressRange::new(TRAMPOLINE_RANGE_START, authored.low_memory_limit)
                .expect("startup placement range"),
        ),
        authored.startup_alignment,
        PlacementPhase::PostHandoff,
        Some(arrival_regime()),
        Some(
            ArtifactInstallationScopeId::from_normalized_identity(INSTALLATION_SCOPE_SEED)
                .expect("installation scope"),
        ),
    )
    .expect("placement constraints");
    let contracts = install_identity(30, MachineContractSetId::from_normalized_identity);
    let footprint = install_identity(31, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_identity(
            0x600,
            executable_installation::ArtifactId::from_normalized_identity,
        ),
        Architecture::X86_64,
        emitted_trampoline_bytes(),
        contracts,
        footprint,
        install_identity(32, PlacementPlanId::from_normalized_identity),
        constraints,
        install_identity(
            33,
            executable_installation::EntrySetId::from_normalized_identity,
        ),
        vec![ArtifactEntry::from_canonical_decode(startup_entry(), 0)],
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
    let extent = extent_grant(100, TRAMPOLINE_BASE, TRAMPOLINE_LENGTH, rights.clone());
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_identity(100, CodePlacementId::from_normalized_identity),
        install_identity(
            INSTALLATION_SCOPE_SEED,
            InstallationScopeId::from_normalized_identity,
        ),
        InstallationAudience::FutureFetcher,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: TRAMPOLINE_BASE,
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
            0x601,
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

/// A per-processor private state extent: the shared fixture address space
/// under a distinct lineage so the ledger's overlap checks compare real
/// custody geometry.
fn processor_state(seed: u64, base: u64, length: u64) -> Extent {
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

fn startup_entry() -> EntryStubId {
    EntryStubId::from_normalized_identity(STARTUP_ENTRY_SEED).expect("normalized entry identity")
}

fn arrival_regime() -> layout_plans::MachineRegimeId {
    layout_plans::MachineRegimeId::from_normalized_identity(ARRIVAL_REGIME_SEED)
        .expect("normalized machine regime identity")
}

fn processor_id(seed: u64) -> SecondaryProcessorOccurrenceId {
    identity(
        seed,
        SecondaryProcessorOccurrenceId::from_normalized_identity,
    )
}

fn invocation_id(seed: u64) -> SecondaryProcessorStartupInvocationId {
    identity(
        seed,
        SecondaryProcessorStartupInvocationId::from_normalized_identity,
    )
}

fn startup_receipt_id(seed: u64) -> SecondaryProcessorStartupReceiptId {
    identity(
        seed,
        SecondaryProcessorStartupReceiptId::from_normalized_identity,
    )
}

fn quiescence_receipt_id(seed: u64) -> SecondaryProcessorQuiescenceReceiptId {
    identity(
        seed,
        SecondaryProcessorQuiescenceReceiptId::from_normalized_identity,
    )
}

fn settlement_receipt_id(seed: u64) -> SecondaryProcessorSettlementReceiptId {
    identity(
        seed,
        SecondaryProcessorSettlementReceiptId::from_normalized_identity,
    )
}

/// The authored contract evidence: the selected provider plan for one
/// rooted startup trait — its single `enter` requirement, the Pending
/// entry claim the ledger later matches, and the bounded `MachineControl`
/// service reach — plus the retained boundary calling-plan realization
/// replayed to its validated entry plan.
fn authored_boundary(
    checked: &compiler::CheckedCompilation,
    trait_name: &str,
) -> ValidatedBoundaryEntryPlan {
    let facts = checked.selected_provider_plans();
    let selected = selected_external_root_provider_plan(facts, trait_name)
        .unwrap_or_else(|_| panic!("the authored `{trait_name}` selects a provider plan"));
    assert_eq!(selected.schema.trait_name, trait_name);
    let [entry] = selected.schema.methods.as_slice() else {
        panic!("`{trait_name}` inherits one exact secondary-processor entry requirement")
    };
    assert_eq!(entry.name, "enter");
    assert_eq!(entry.requirement_owner, "SecondaryProcessorEntry");

    // The requirement carries the authored Pending qualification as a strict
    // entry claim on the envelope parameter.
    let entry_claims = selected
        .entry_claims(&entry.requirement_identity)
        .expect("the selected plan lowers its Pending entry claim");
    let [pending] = entry_claims.as_slice() else {
        panic!("secondary-processor entry publishes one Pending claim")
    };
    assert_eq!(pending.parameter_index, 0);
    assert_eq!(pending.domain, "StartupEnvelope::Pending");
    assert_eq!(
        pending.effective_carry,
        language_semantics::CarryPolicy::STRICT
    );

    // The root's bounded requirement resolves through its own selected
    // provider plan — both roots inherit `SecondaryProcessorEntry::enter`,
    // so the resolution is addressed by (requirement, root plan) and binds
    // the authored `MachineControl` row.
    let resolution = facts
        .installation_reach_resolution_for_plan(
            selected.identity.normalized_identity(),
            &entry.requirement_identity,
        )
        .expect("the root's own plan resolves the shared entry requirement");
    assert_eq!(resolution.resolved_row, ["MachineControl".to_owned()]);
    assert_eq!(resolution.upper_bound, ["MachineControl".to_owned()]);

    // The validated boundary is the authored policy's retained realization:
    // call/return exit, the installed machine regime, the slot's dedicated
    // stack class.
    let matching = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| {
            checked
                .symbols
                .display_path(realization.boundary_trait, "::")
                .ends_with(trait_name)
        })
        .collect::<Vec<_>>();
    let [realization] = matching.as_slice() else {
        panic!("one retained boundary calling-plan realization for `{trait_name}`")
    };
    assert_eq!(
        entry.calling_plan_report_fingerprint,
        Some(realization.report_fingerprint),
        "the selected schema joins the retained boundary realization"
    );
    let (boundary, boundary_fingerprint, boundary_commitment) = realization
        .replayed_validated_application()
        .expect("the authored policy's validated entry plan replays");
    assert_eq!(boundary_fingerprint, realization.report_fingerprint);
    assert_eq!(boundary_commitment, realization.commitment);
    assert_eq!(boundary.plan().call.entry_control, EntryControl::CallReturn);
    assert_eq!(
        boundary.plan().state.initial_regime,
        MachineRegime::X86Long64
    );
    assert_eq!(boundary.plan().state.preemption, Preemption::NotApplicable);
    boundary
}

/// The bound startup trampoline: the provider-declared profile built from
/// the authored geometry joined to the authored policy's installed regime,
/// bound inside the low-memory window with its authored bytes visible at
/// the admitted startup entry. The borrow keeps the installed trampoline
/// unretirable while any admitted attempt can reach it.
fn bound_trampoline<'code>(
    authored: &AuthoredStartup,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &'code InstalledCode,
) -> InstalledSecondaryProcessorTrampoline<'code> {
    let profile = SecondaryProcessorStartupProfile::new(
        identity(
            0x500,
            SecondaryProcessorStartupProfileId::from_normalized_identity,
        ),
        startup_entry(),
        arrival_regime(),
        boundary.plan().state.initial_regime,
        authored.low_memory_limit,
        authored.startup_alignment,
    )
    .expect("secondary-processor startup profile");
    let trampoline =
        bind_secondary_processor_trampoline(profile, code, TRAMPOLINE_BASE, TRAMPOLINE_LENGTH)
            .expect("bound secondary-processor trampoline");
    assert_eq!(
        trampoline.startup_vector(),
        TRAMPOLINE_BASE / authored.startup_alignment
    );
    assert_eq!(trampoline.extent_base(), TRAMPOLINE_BASE);
    assert_eq!(trampoline.extent_length(), TRAMPOLINE_LENGTH);
    // The emitted trampoline content is visible at the admitted entry: the
    // installed occurrence binds the exact bytes the compiler emitted.
    let emitted = emitted_trampoline_bytes();
    assert!(code.binds_exact_materialized_entry_bytes(startup_entry(), &emitted));
    assert!(code.binds_placement_geometry(TRAMPOLINE_BASE, TRAMPOLINE_LENGTH));
    trampoline
}

/// One admitted account for a declared slot: the authored boundary joins
/// the slot's declared class and demand to a provisioned private state
/// extent.
fn authored_account(
    boundary: ValidatedBoundaryEntryPlan,
    slot: &AuthoredSlot,
    processor_seed: u64,
    state_base: u64,
) -> SecondaryProcessorAccount {
    let EntryStack::Dedicated { class } = boundary.plan().state.stack else {
        panic!("the authored startup entry arrives on a dedicated stack")
    };
    assert_eq!(
        class, slot.stack_class,
        "the authored policy's dedicated class is the roster's declared class"
    );
    SecondaryProcessorAccount::new(
        processor_id(processor_seed),
        boundary,
        slot.stack_class,
        slot.stack_bytes,
        slot.stack_alignment,
        processor_state(2000 + processor_seed, state_base, slot.state_bytes),
    )
    .expect("secondary-processor account")
}

/// The authored Cathedral startup reaches the installed entry: both
/// declared processors admit on dedicated nonoverlapping resources, the
/// first's confirmed arrival retires cleanly, and the second's
/// cancellation settles custody.
#[test]
fn authored_secondary_processor_startup_reaches_the_installed_entry() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&canary_main(), None))
        .expect("secondary-processor canary compiles");
    let authored = authored_startup(&checked.typed);
    let [first_slot, second_slot] = authored.slots.as_slice() else {
        panic!("the authored roster declares exactly two secondary processors")
    };
    assert_ne!(first_slot.stack_class, second_slot.stack_class);

    let first_boundary = authored_boundary(&checked, "FirstSecondaryProcessorRoot");
    let second_boundary = authored_boundary(&checked, "SecondSecondaryProcessorRoot");
    assert_eq!(
        first_boundary.plan().state.stack,
        EntryStack::Dedicated {
            class: first_slot.stack_class
        }
    );
    assert_eq!(
        second_boundary.plan().state.stack,
        EntryStack::Dedicated {
            class: second_slot.stack_class
        }
    );

    let code = startup_trampoline_code(&authored);
    let mut ledger =
        SecondaryProcessorStartupLedger::new(bound_trampoline(&authored, &first_boundary, &code));

    // Admission joins each slot's declared class/demand to dedicated
    // custody: two processors, distinct stack classes, nonoverlapping
    // private state extents.
    let first = processor_id(0x101);
    let second = processor_id(0x102);
    ledger
        .admit_secondary_processor(authored_account(
            first_boundary.clone(),
            first_slot,
            0x101,
            0x10_0000,
        ))
        .expect("the first authored processor admits");
    ledger
        .admit_secondary_processor(authored_account(
            second_boundary.clone(),
            second_slot,
            0x102,
            0x10_1000,
        ))
        .expect("the second authored processor admits on disjoint custody");

    // Dispatch: the carrier names the installed entry and the bound
    // startup vector the supervisor publishes.
    let first_invocation = invocation_id(0x201);
    let first_carrier = ledger
        .begin_secondary_processor_startup(first, first_invocation)
        .expect("the first processor's startup carrier issues");
    assert_eq!(first_carrier.startup_entry(), startup_entry());
    assert_eq!(first_carrier.processor(), first);
    assert_eq!(
        first_carrier.startup_vector(),
        TRAMPOLINE_BASE / authored.startup_alignment
    );

    // Confirmed arrival: the processor executed the authored `confirm`
    // edge on the outstanding envelope.
    let first_started_receipt = SecondaryProcessorStartupReceipt::from_provider(
        startup_receipt_id(0x301),
        &first_carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let first_started =
        match ledger.complete_secondary_processor_startup(first_carrier, first_started_receipt) {
            Ok(SecondaryProcessorStartupOutcome::Started(started)) => started,
            other => panic!("a confirmed first arrival must start the processor, got {other:?}"),
        };
    assert_eq!(first_started.processor(), first);
    assert_eq!(first_started.invocation(), first_invocation);
    assert_eq!(first_started.startup_entry(), startup_entry());

    // Retirement: the provider's quiescence receipt returns the account's
    // complete custody.
    let retired_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x401),
        &first_started,
        true,
    );
    let retired = match ledger.retire_secondary_processor(first_started, retired_receipt) {
        Ok(SecondaryProcessorQuiescenceOutcome::Retired(retired)) => retired,
        other => panic!("a quiescent first processor must retire, got {other:?}"),
    };
    assert_eq!(retired.processor(), first);
    let (_, stack_class, wcsu_bytes, _, state, _) = retired.into_parts();
    assert_eq!(stack_class, first_slot.stack_class);
    assert_eq!(wcsu_bytes, first_slot.stack_bytes);
    assert_eq!(state.base(), 0x10_0000);
    assert_eq!(state.length(), first_slot.state_bytes);

    // Cancellation before confirmation: the provider's settlement receipt
    // attests both premises against the exact outstanding carrier, so the
    // second attempt's custody returns without a started processor.
    let second_invocation = invocation_id(0x202);
    let second_carrier = ledger
        .begin_secondary_processor_startup(second, second_invocation)
        .expect("the second processor's startup carrier issues");
    let settled_receipt = SecondaryProcessorSettlementReceipt::from_provider(
        settlement_receipt_id(0x402),
        &second_carrier,
        true,
        true,
    );
    let settled = match ledger.settle_secondary_processor_startup(second_carrier, settled_receipt) {
        Ok(SecondaryProcessorSettlementOutcome::Settled(settled)) => settled,
        other => panic!("both settlement premises must return custody, got {other:?}"),
    };
    assert_eq!(settled.processor(), second);
    assert_eq!(settled.invocation(), second_invocation);
    let (_, _, _, _, second_state, _) = settled.into_parts();
    assert_eq!(second_state.base(), 0x10_1000);
    assert_eq!(second_state.length(), second_slot.state_bytes);
}

/// Definite nondispatch returns the account to pending and withdrawable;
/// an unconfirmed attempt keeps custody until a settlement receipt attests
/// both premises, and a late arrival on the outstanding carrier still
/// resolves it.
#[test]
fn authored_startup_nondispatch_withdraws_and_late_arrival_starts() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&canary_main(), None))
        .expect("secondary-processor canary compiles");
    let authored = authored_startup(&checked.typed);
    let [first_slot, second_slot] = authored.slots.as_slice() else {
        panic!("the authored roster declares exactly two secondary processors")
    };
    let first_boundary = authored_boundary(&checked, "FirstSecondaryProcessorRoot");
    let second_boundary = authored_boundary(&checked, "SecondSecondaryProcessorRoot");
    let _code = startup_trampoline_code(&authored);
    let mut ledger =
        SecondaryProcessorStartupLedger::new(bound_trampoline(&authored, &first_boundary, &_code));

    let first = processor_id(0x111);
    let second = processor_id(0x112);
    ledger
        .admit_secondary_processor(authored_account(
            first_boundary,
            first_slot,
            0x111,
            0x10_0000,
        ))
        .expect("the first authored processor admits");
    ledger
        .admit_secondary_processor(authored_account(
            second_boundary,
            second_slot,
            0x112,
            0x10_1000,
        ))
        .expect("the second authored processor admits");

    // Definite nondispatch: the provider attests the vector never
    // dispatched, so the account returns to pending and its custody can
    // withdraw intact.
    let first_carrier = ledger
        .begin_secondary_processor_startup(first, invocation_id(0x211))
        .expect("the first processor's startup carrier issues");
    let refused_receipt = SecondaryProcessorStartupReceipt::from_provider(
        startup_receipt_id(0x311),
        &first_carrier,
        SecondaryProcessorStartupVerdict::DefiniteNondispatch,
    );
    let refused = match ledger.complete_secondary_processor_startup(first_carrier, refused_receipt)
    {
        Ok(SecondaryProcessorStartupOutcome::Refused(refused)) => refused,
        other => panic!("definite nondispatch must refuse, got {other:?}"),
    };
    assert_eq!(refused.processor(), first);
    let withdrawn = ledger
        .withdraw_secondary_processor(first)
        .expect("a refused, never-arrived processor withdraws");
    let (_, _, _, _, withdrawn_state, _) = withdrawn.into_parts();
    assert_eq!(withdrawn_state.base(), 0x10_0000);

    // Timeout without evidence: dispatch unconfirmed keeps the account
    // invoked and held, and returns the outstanding carrier.
    let second_invocation = invocation_id(0x212);
    let second_carrier = ledger
        .begin_secondary_processor_startup(second, second_invocation)
        .expect("the second processor's startup carrier issues");
    let unconfirmed_receipt = SecondaryProcessorStartupReceipt::from_provider(
        startup_receipt_id(0x312),
        &second_carrier,
        SecondaryProcessorStartupVerdict::DispatchUnconfirmed,
    );
    let unconfirmed =
        match ledger.complete_secondary_processor_startup(second_carrier, unconfirmed_receipt) {
            Ok(SecondaryProcessorStartupOutcome::Unconfirmed(unconfirmed)) => unconfirmed,
            other => panic!("an unconfirmed dispatch keeps the attempt outstanding, got {other:?}"),
        };
    assert_eq!(unconfirmed.invocation(), second_invocation);
    assert!(
        ledger
            .withdraw_secondary_processor(second)
            .is_err_and(|error| error.processor() == second),
        "an unconfirmed attempt is not withdrawable"
    );
    let second_carrier = unconfirmed.into_carrier();

    // Cancellation with only one premise attested holds custody and hands
    // the carrier back — timeout alone cannot release stack or state.
    let held_receipt = SecondaryProcessorSettlementReceipt::from_provider(
        settlement_receipt_id(0x412),
        &second_carrier,
        true,
        false,
    );
    let held = match ledger.settle_secondary_processor_startup(second_carrier, held_receipt) {
        Ok(SecondaryProcessorSettlementOutcome::Held(held)) => held,
        other => panic!("a single settlement premise must hold custody, got {other:?}"),
    };
    let second_carrier = held.into_carrier();

    // Late arrival: the outstanding carrier's definitive receipt resolves
    // the same attempt the held settlement answered.
    let second_started_receipt = SecondaryProcessorStartupReceipt::from_provider(
        startup_receipt_id(0x313),
        &second_carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let second_started =
        match ledger.complete_secondary_processor_startup(second_carrier, second_started_receipt) {
            Ok(SecondaryProcessorStartupOutcome::Started(started)) => started,
            other => panic!("a late confirmed arrival starts the processor, got {other:?}"),
        };
    assert_eq!(second_started.invocation(), second_invocation);

    // A non-quiescent retirement receipt keeps custody held and returns the
    // started evidence; the quiescent retry then retires it.
    let held_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x413),
        &second_started,
        false,
    );
    let held = match ledger.retire_secondary_processor(second_started, held_receipt) {
        Ok(SecondaryProcessorQuiescenceOutcome::Held(held)) => held,
        other => panic!("a non-quiescent retirement receipt must hold custody, got {other:?}"),
    };
    let second_started = held.into_started();
    let retired_receipt = SecondaryProcessorQuiescenceReceipt::from_provider(
        quiescence_receipt_id(0x414),
        &second_started,
        true,
    );
    let retired = match ledger.retire_secondary_processor(second_started, retired_receipt) {
        Ok(SecondaryProcessorQuiescenceOutcome::Retired(retired)) => retired,
        other => panic!("the quiescent retry retires the processor, got {other:?}"),
    };
    assert_eq!(retired.processor(), second);
}

/// The ledger refuses foreign or replayed evidence and overlapping
/// resources: settlement/retirement receipts name the exact outstanding
/// carrier or started evidence, and admission keeps dedicated classes and
/// state extents nonoverlapping.
#[test]
fn authored_startup_rejects_stale_evidence_and_resource_conflicts() {
    let checked = compile_to_checked(CheckedCompileRequest::new(&canary_main(), None))
        .expect("secondary-processor canary compiles");
    let authored = authored_startup(&checked.typed);
    let [first_slot, second_slot] = authored.slots.as_slice() else {
        panic!("the authored roster declares exactly two secondary processors")
    };
    let first_boundary = authored_boundary(&checked, "FirstSecondaryProcessorRoot");
    let second_boundary = authored_boundary(&checked, "SecondSecondaryProcessorRoot");
    let _code = startup_trampoline_code(&authored);
    let mut ledger =
        SecondaryProcessorStartupLedger::new(bound_trampoline(&authored, &first_boundary, &_code));

    let first = processor_id(0x121);
    let second = processor_id(0x122);
    ledger
        .admit_secondary_processor(authored_account(
            first_boundary.clone(),
            first_slot,
            0x121,
            0x10_0000,
        ))
        .expect("the first authored processor admits");

    // Resource conflicts: an overlapping private state extent and a reused
    // dedicated stack class both refuse admission, returning the account.
    let overlapping = authored_account(second_boundary.clone(), second_slot, 0x122, 0x10_0800);
    let overlap_error = ledger
        .admit_secondary_processor(overlapping)
        .expect_err("an overlapping state extent refuses admission");
    assert_eq!(overlap_error.into_account().processor(), second);

    let duplicate_class = SecondaryProcessorAccount::new(
        processor_id(0x123),
        first_boundary.clone(),
        first_slot.stack_class,
        first_slot.stack_bytes,
        first_slot.stack_alignment,
        processor_state(0x123 + 2000, 0x10_4000, first_slot.state_bytes),
    )
    .expect("the conflicting account constructs");
    let class_error = ledger
        .admit_secondary_processor(duplicate_class)
        .expect_err("a reused dedicated stack class refuses admission");
    assert_eq!(class_error.into_account().processor(), processor_id(0x123));

    ledger
        .admit_secondary_processor(authored_account(
            second_boundary,
            second_slot,
            0x122,
            0x10_1000,
        ))
        .expect("the second authored processor admits on disjoint custody");

    // Replayed evidence: a receipt minted against the first invocation
    // cannot answer the second attempt's outstanding carrier.
    let first_invocation = invocation_id(0x221);
    let first_carrier = ledger
        .begin_secondary_processor_startup(first, first_invocation)
        .expect("the first processor's startup carrier issues");
    let stale_receipt = SecondaryProcessorStartupReceipt::from_provider(
        startup_receipt_id(0x321),
        &first_carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let settled_receipt = SecondaryProcessorSettlementReceipt::from_provider(
        settlement_receipt_id(0x421),
        &first_carrier,
        true,
        true,
    );
    let settled = match ledger.settle_secondary_processor_startup(first_carrier, settled_receipt) {
        Ok(SecondaryProcessorSettlementOutcome::Settled(settled)) => settled,
        other => panic!("both settlement premises must return custody, got {other:?}"),
    };
    assert_eq!(settled.invocation(), first_invocation);
    // Settlement returned the account's custody; a fresh startup of the
    // re-admitted processor issues a distinct carrier.
    let (boundary, stack_class, wcsu_bytes, wcsu_alignment, state, _) = settled.into_parts();
    ledger
        .admit_secondary_processor(
            SecondaryProcessorAccount::new(
                first,
                boundary,
                stack_class,
                wcsu_bytes,
                wcsu_alignment,
                state,
            )
            .expect("the settled processor's account re-admits"),
        )
        .expect("the settled processor re-admits on its returned custody");
    let fresh_invocation = invocation_id(0x222);
    let fresh_carrier = ledger
        .begin_secondary_processor_startup(first, fresh_invocation)
        .expect("a settled account accepts a fresh invocation");
    let replay_error = ledger
        .complete_secondary_processor_startup(fresh_carrier, stale_receipt)
        .expect_err("a receipt bound to the settled carrier is stale");
    let (fresh_carrier, _) = replay_error.into_parts();

    // Foreign evidence: the first attempt's carrier cannot answer the
    // second processor's attempt, and vice versa.
    let second_invocation = invocation_id(0x223);
    let second_carrier = ledger
        .begin_secondary_processor_startup(second, second_invocation)
        .expect("the second processor's startup carrier issues");
    let foreign_receipt = SecondaryProcessorStartupReceipt::from_provider(
        startup_receipt_id(0x322),
        &fresh_carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let foreign_error = ledger
        .complete_secondary_processor_startup(second_carrier, foreign_receipt)
        .expect_err("a receipt minted against another carrier is foreign");
    let (second_carrier, _) = foreign_error.into_parts();

    // The exact carrier's own definitive receipt still resolves.
    let second_started_receipt = SecondaryProcessorStartupReceipt::from_provider(
        startup_receipt_id(0x323),
        &second_carrier,
        SecondaryProcessorStartupVerdict::ConfirmedArrival,
    );
    let second_started =
        match ledger.complete_secondary_processor_startup(second_carrier, second_started_receipt) {
            Ok(SecondaryProcessorStartupOutcome::Started(started)) => started,
            other => panic!("the second attempt's own receipt confirms arrival, got {other:?}"),
        };
    assert_eq!(second_started.invocation(), second_invocation);
}

/// The authored startup contract survives both production boundaries the
/// installed-entry leg must cross: the canary binds host program entries, so
/// the package produces a retained Terminal artifact whose native proposal
/// still selects each root's exact `enter` requirement — the strict Pending
/// entry claim and the plan-scoped `MachineControl` reach — and the native
/// artifact carries the same two provider plans. The provider bodies are
/// selected boundary entries, not machines reachable from `main`, so their
/// bytes stay absent from the emitted image until the entry/stub emission
/// leg lands; the plans that leg must bind are what production retains.
#[test]
fn authored_startup_contract_survives_terminal_and_native_production() {
    use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};

    let options = CompileOptions {
        root_path: canary_main(),
        build_dir: None,
        target_name: Some("linux_x86_64".to_owned()),
    };
    let request = CompileRequest::new(options)
        .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    let report = compile(request)
        .unwrap_or_else(|diagnostics| {
            panic!("secondary-processor canary Terminal production rejected: {diagnostics:?}")
        })
        .into_single_report()
        .expect("one compile report");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal production retains its artifact");
    retained
        .validate()
        .expect("the retained terminal artifact replays");

    // The retained product carries the Omega-side proposal the later
    // native realization re-joins: the provider plans cross the
    // source-free boundary here, not at checked time.
    let proposal = retained
        .native_realization_proposal()
        .expect("retained terminal artifact carries its native proposal");
    let facts = proposal.selected_provider_plans();
    for trait_name in [
        "FirstSecondaryProcessorRoot",
        "SecondSecondaryProcessorRoot",
    ] {
        let selected = selected_external_root_provider_plan(facts, trait_name)
            .unwrap_or_else(|_| panic!("the retained proposal selects `{trait_name}`"));
        let [entry] = selected.schema.methods.as_slice() else {
            panic!("`{trait_name}` still inherits one exact entry requirement")
        };
        assert_eq!(entry.name, "enter");
        assert_eq!(entry.requirement_owner, "SecondaryProcessorEntry");
        let claims = selected
            .entry_claims(&entry.requirement_identity)
            .expect("the retained plan lowers its Pending claim");
        let [pending] = claims.as_slice() else {
            panic!("the retained plan publishes one Pending entry claim")
        };
        assert_eq!(pending.parameter_index, 0);
        assert_eq!(pending.domain, "StartupEnvelope::Pending");
        assert_eq!(
            pending.effective_carry,
            language_semantics::CarryPolicy::STRICT
        );
        let resolution = facts
            .installation_reach_resolution_for_plan(
                selected.identity.normalized_identity(),
                &entry.requirement_identity,
            )
            .expect("the retained plan still resolves the entry's bounded reach");
        assert_eq!(resolution.resolved_row, ["MachineControl".to_owned()]);
        assert_eq!(resolution.upper_bound, ["MachineControl".to_owned()]);
    }

    // Native realization keeps both provider plans as retained custody: the
    // emitted image carries only the program entry today — the boundary
    // providers' bodies emit through the entry/stub lane this item still
    // owes — but the artifact's exact plan identities are the rows that lane
    // joins when it lands.
    let options = CompileOptions {
        root_path: canary_main(),
        build_dir: None,
        target_name: Some("linux_x86_64".to_owned()),
    };
    let request = CompileRequest::new(options)
        .with_requested_product(RequestedCompileProduct::NativeArtifact);
    let report = compile(request)
        .unwrap_or_else(|diagnostics| {
            panic!("secondary-processor canary native production rejected: {diagnostics:?}")
        })
        .into_single_report()
        .expect("one compile report");
    let artifact = report
        .into_retained_native_artifact()
        .expect("native production retains its artifact");
    artifact
        .validate()
        .expect("the retained native artifact replays");
    let plans = artifact.selected_provider_plans();
    assert_eq!(
        plans.len(),
        2,
        "the native artifact retains both secondary-processor provider plans"
    );
    assert_ne!(
        plans[0].report_identity(),
        plans[1].report_identity(),
        "the two startup roots retain distinct provider plans"
    );
    for plan in plans {
        let [requirement] = plan.requirement_identities() else {
            panic!("each startup provider plan carries exactly one requirement")
        };
        assert!(
            requirement.contains("SecondaryProcessorEntry::enter"),
            "the retained requirement is the shared entry contract"
        );
        assert!(
            requirement.contains("StartupEnvelope::Pending"),
            "the retained requirement keeps the Pending domain constraint"
        );
    }
}

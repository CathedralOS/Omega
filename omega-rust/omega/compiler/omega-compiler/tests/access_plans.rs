//! Source-visible L6b policy evaluation. These tests keep the source record
//! vocabulary, build-time interpreter, and sealed normalized access model on
//! one end-to-end path.

use std::fs;
use std::path::PathBuf;

use omega_compiler::{compile_to_checked, compile_to_checked_with_packages};
use omega_layout::{DataShape, build_layout_plan};
use omega_package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use omega_target::NativeTarget;
use psi_access_plans::{
    AccessExposure, AccessOperation, AtomicAccessOperation, AtomicCapability, AtomicPermissions,
    AtomicTransferRule, BoundaryReach, EffectiveSupplyKind, ExternalCapability, ExternalRead,
    ExternalReadBehavior, FieldAccess, ObservationModel, PlacedOccurrenceId, PlacementAdmissionId,
    ResourceProfile, ResourceProfileGrant, ResourceProfileReceiptId, ResourceRegion,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant,
    StableCapability, StableDeviceInstanceId, TransferRule, admit_owned_placement, admit_placement,
    adopt_owned_atomic, adopt_owned_stable, bind_schema_correspondence_to_placement, place,
};
use psi_build_time_evaluation::{compute_access_plan, compute_layout_plan, compute_placement_plan};
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::PackageKeyIdentity;
use psi_extents::{
    AddressSpaceId, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLineageId,
    ExtentProvenanceId, ExtentProviderIssuance, ExtentRightId, ExtentRights, ExtentRootGrant,
    MappingEraId, ResidentClaimId,
};
use psi_language_core::ReferenceAccess;
use psi_language_core::atomic::{
    AtomicObservingCompareExchangeOperation, AtomicObservingCompareExchangeResultShape,
    AtomicOrderingPlan, MemoryOrdering,
};
use psi_language_semantics::Multiplicity;

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("omega-access-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create access-plan test directory");
    let main = directory.join("main.omg");
    fs::write(&main, source).expect("write access-plan test program");
    main
}

fn package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

fn extent_identity<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
) -> T {
    constructor(identity).expect("nonzero normalized extent identity")
}

fn provider_issuance(seed: u64) -> ExtentProviderIssuance {
    let base = seed * 16;
    ExtentProviderIssuance::from_normalized_identities([
        base + 1,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 7,
        base + 8,
        base + 9,
        base + 10,
        base + 11,
        base + 12,
        base + 13,
    ])
    .expect("normalized provider issuance")
}

fn write_cross_package_program(name: &str, consumer: &str) -> (PathBuf, PackageCompilationInputs) {
    write_cross_package_program_with_policy(name, consumer, POLICY_SOURCE)
}

fn write_cross_package_program_with_policy(
    name: &str,
    consumer: &str,
    policy_source: &str,
) -> (PathBuf, PackageCompilationInputs) {
    let directory =
        std::env::temp_dir().join(format!("omega-access-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    let root_directory = directory.join("root");
    let policy_directory = directory.join("policy");
    fs::create_dir_all(&root_directory).expect("create root package directory");
    fs::create_dir_all(&policy_directory).expect("create policy package directory");
    let policy_end = policy_source
        .find("data Main {}")
        .expect("policy fixture main marker");
    fs::write(
        policy_directory.join("policy.omg"),
        &policy_source[..policy_end],
    )
    .expect("write policy package");
    fs::write(
        policy_directory.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"policy\"); }\n",
    )
    .expect("write policy package declaration");
    fs::write(
        root_directory.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("access-plans");
    builder.depend_as("policy", Source::Path { location: "../policy" });
}
"#,
    )
    .expect("write root build manifest");
    let main = root_directory.join("main.omg");
    fs::write(
        &main,
        format!(
            r#"
use policy::policy;

{consumer}

data Main {{}}
machine Main::main(&mut self) {{}}
"#
        ),
    )
    .expect("write root consumer");
    let inputs = PackageCompilationInputs::new_package(
        package_identity(1),
        vec![
            PackageSourceBinding::new(package_identity(1), "access-plans", root_directory),
            PackageSourceBinding::new(package_identity(2), "policy", policy_directory),
        ],
        vec![PackageDependencyBinding::new(
            package_identity(1),
            "policy",
            package_identity(2),
        )],
    )
    .expect("cross-package access fixture should form a closed package graph");
    (main, inputs)
}

const POLICY_SOURCE: &str = r#"
use omega::language::core::layout;

pub data Registers {
    status: u32;
    transmit: u8;
    snapshot: u16;
    counter: u64;
    reserved: u8;
}

data UartLayout {
    entries: [FieldEntry; 64];
}

machine UartLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    self.entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 4 }
    };
    self.entries[2] = FieldEntry {
        key: schema.fields[2].key,
        placement: FieldPlan::At { offset: 6 }
    };
    self.entries[3] = FieldEntry {
        key: schema.fields[3].key,
        placement: FieldPlan::At { offset: 8 }
    };
    self.entries[4] = FieldEntry {
        key: schema.fields[4].key,
        placement: FieldPlan::At { offset: 16 }
    };
    Plan {
        entries: self.entries,
        entry_count: 5,
        size_fixed: 24,
        size_is_dynamic: false,
        align: 8
    }
}

data UartAccess {}

machine UartAccess::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(schema);
    transition layout.size_fixed == 24
        && layout.size_is_dynamic == false
        && layout.align == 8 {
        true -> allow_status(schema, plan)
        _ -> (plan)
    }

    state allow_status(schema: Schema, plan: AccessPlan) -> AccessPlan {
        transition { _ -> allow_transmit(
            schema,
            plan.with(
                schema.fields[0].key,
                FieldAccess::External {
                    read: ExternalRead::Read,
                    write: false,
                    exposure: Exposure::Exported
                }
            )
        ) }
    }

    state allow_transmit(schema: Schema, plan: AccessPlan) -> AccessPlan {
        transition { _ -> allow_snapshot(
            schema,
            plan.with(
                schema.fields[1].key,
                FieldAccess::External {
                    read: ExternalRead::None,
                    write: true,
                    exposure: Exposure::Exported
                }
            )
        ) }
    }

    state allow_snapshot(schema: Schema, plan: AccessPlan) -> AccessPlan {
        transition { _ -> allow_counter(
            schema,
            plan.with(
                schema.fields[2].key,
                FieldAccess::Stable {
                    read: true,
                    write: true,
                    exposure: Exposure::BindingPrivate
                }
            )
        ) }
    }

    state allow_counter(schema: Schema, plan: AccessPlan) -> AccessPlan {
        plan.with(
            schema.fields[3].key,
            FieldAccess::Atomic {
                operations: AtomicOperations {
                    load: true,
                    store: false,
                    fetch_add: true,
                    fetch_sub: false,
                    fetch_xor: false,
                    fetch_or: false,
                    fetch_and: false,
                    swap: false,
                    compare_exchange: false,
                    compare_exchange_once: false,
                    try_exchange: false,
                    try_exchange_once: false
                },
                exposure: Exposure::Exported
            }
        )
    }
}

pub data UartPlacement {
    layout_entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine UartPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    self.layout_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    self.layout_entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 4 }
    };
    self.layout_entries[2] = FieldEntry {
        key: schema.fields[2].key,
        placement: FieldPlan::At { offset: 6 }
    };
    self.layout_entries[3] = FieldEntry {
        key: schema.fields[3].key,
        placement: FieldPlan::At { offset: 8 }
    };
    self.layout_entries[4] = FieldEntry {
        key: schema.fields[4].key,
        placement: FieldPlan::At { offset: 16 }
    };
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    transition { _ -> place_status(schema, access) }

    state place_status(
        &mut self,
        schema: Schema,
        access: AccessPlan
    ) -> PlacementPlan {
        transition { _ -> place_transmit(
            schema,
            access.with(
                schema.fields[0].key,
                FieldAccess::External {
                    read: ExternalRead::Read,
                    write: false,
                    exposure: Exposure::Exported
                }
            )
        ) }
    }

    state place_transmit(
        &mut self,
        schema: Schema,
        access: AccessPlan
    ) -> PlacementPlan {
        transition { _ -> place_snapshot(
            schema,
            access.with(
                schema.fields[1].key,
                FieldAccess::External {
                    read: ExternalRead::None,
                    write: true,
                    exposure: Exposure::Exported
                }
            )
        ) }
    }

    state place_snapshot(
        &mut self,
        schema: Schema,
        access: AccessPlan
    ) -> PlacementPlan {
        transition { _ -> place_counter(
            schema,
            access.with(
                schema.fields[2].key,
                FieldAccess::Stable {
                    read: true,
                    write: true,
                    exposure: Exposure::BindingPrivate
                }
            )
        ) }
    }

    state place_counter(
        &mut self,
        schema: Schema,
        access: AccessPlan
    ) -> PlacementPlan {
        transition { _ -> finish(
            access.with(
                schema.fields[3].key,
                FieldAccess::Atomic {
                    operations: AtomicOperations {
                        load: true,
                        store: false,
                        fetch_add: true,
                        fetch_sub: false,
                        fetch_xor: false,
                        fetch_or: false,
                        fetch_and: false,
                        swap: false,
                        compare_exchange: false,
                        compare_exchange_once: false,
                        try_exchange: false,
                        try_exchange_once: false
                    },
                    exposure: Exposure::Exported
                }
            )
        ) }
    }

    state finish(&mut self, access: AccessPlan) -> PlacementPlan {
    self.services[0] = 19;
    PlacementPlan {
        layout: Plan {
            entries: self.layout_entries,
            entry_count: 5,
            size_fixed: 24,
            size_is_dynamic: false,
            align: 8
        },
        access: access,
        reach: BoundaryReach {
            services: self.services,
            service_count: 1
        }
    }
}
}

data Main {}
machine Main::main(&mut self) {}
"#;

#[test]
fn compiler_accessor_templates_are_inert_without_placed_views() {
    let main = write_program(
        "no-placed-view",
        r#"
data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked =
        compile_to_checked(&main, None).expect("ordinary program should ignore accessor templates");
    assert!(
        checked
            .typed
            .machines()
            .iter()
            .all(|machine| !machine.name.as_str().starts_with("PlacedField::")),
        "compiler-only accessor templates must not enter the typed program"
    );
}

#[test]
fn source_access_policy_evaluates_against_validated_layout() {
    let main = write_program("source-access", POLICY_SOURCE);
    let checked = compile_to_checked(&main, None).expect("source policy should compile");
    let layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers")
        .expect("layout should validate before access evaluation");
    let access = compute_access_plan(&checked.typed, "UartAccess::plan", "Registers", &layout)
        .expect("source access policy should evaluate and normalize");

    assert_ne!(access.identity().compatibility_fingerprint(), 0);
    assert_eq!(access.field_descriptors().len(), 4);
    assert_eq!(access.plan().entries().len(), 5);
    assert!(matches!(
        access
            .plan()
            .entries()
            .iter()
            .find(|entry| entry.field() == "reserved")
            .expect("reserved source decision")
            .access(),
        FieldAccess::Inaccessible
    ));

    let status = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "status")
        .expect("status descriptor");
    assert_eq!(status.transfer_width_bits(), 32);
    assert_eq!(status.observation(), ObservationModel::External);
    assert!(status.permissions().read);
    assert!(!status.permissions().write);

    let transmit = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "transmit")
        .expect("transmit descriptor");
    assert_eq!(transmit.transfer_width_bits(), 8);
    assert_eq!(transmit.observation(), ObservationModel::External);
    assert!(!transmit.permissions().read);
    assert!(transmit.permissions().write);

    let snapshot = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "snapshot")
        .expect("snapshot descriptor");
    assert_eq!(snapshot.transfer_width_bits(), 16);
    assert_eq!(snapshot.observation(), ObservationModel::Stable);
    assert_eq!(snapshot.exposure(), AccessExposure::BindingPrivate);

    let counter = access
        .field_descriptors()
        .iter()
        .find(|field| field.field() == "counter")
        .expect("counter descriptor");
    assert_eq!(counter.transfer_width_bits(), 64);
    assert_eq!(counter.observation(), ObservationModel::Atomic);
    assert!(counter.permissions().atomic.load);
    assert!(counter.permissions().atomic.fetch_add);
    assert!(!counter.permissions().atomic.store);

    assert!(matches!(
        access
            .plan()
            .entries()
            .iter()
            .find(|entry| entry.field() == "status")
            .expect("status source decision")
            .access(),
        FieldAccess::External {
            read: ExternalRead::Read,
            ..
        }
    ));
}

#[test]
fn numbered_access_policy_rejoins_a_retained_layout_after_field_rename() {
    let legacy = write_program(
        "numbered-access-legacy-layout",
        r#"
use omega::language::core::layout;

data RetainedLayout { entries: [FieldEntry; 64]; }
machine RetainedLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 4, size_is_dynamic: false, align: 4 }
}

data Registers { #7 legacy_status: u32; }
data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let legacy = compile_to_checked(&legacy, None).expect("legacy numbered schema should check");
    let retained = compute_layout_plan(&legacy.typed, "RetainedLayout::plan", "Registers")
        .expect("legacy numbered layout should validate");
    assert_eq!(retained.entries[0].field, "legacy_status");
    assert_eq!(retained.entries[0].member_identity, Some(7));

    let renamed = write_program(
        "numbered-access-renamed-schema",
        r#"
use omega::language::core::layout;

data Registers { #7 status: u32; }
data RegisterAccess {}
machine RegisterAccess::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(schema);
    transition layout.entry_count == 1 && layout.size_fixed == 4 {
        true -> (plan.with(
            schema.fields[0].key,
            FieldAccess::Stable {
                read: true,
                write: false,
                exposure: Exposure::Exported,
            },
        ))
        _ -> (plan)
    }
}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let renamed = compile_to_checked(&renamed, None).expect("renamed numbered schema should check");
    let access = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &retained,
    )
    .expect("stable identity should rejoin the retained layout for access evaluation");
    let [descriptor] = access.field_descriptors() else {
        panic!("one accessible field should produce one descriptor")
    };
    assert_eq!(descriptor.field(), "status");
    assert_eq!(descriptor.container_byte_offset(), 0);
    assert_eq!(descriptor.transfer_width_bits(), 32);
    assert_eq!(descriptor.observation(), ObservationModel::Stable);

    let mut drifted = retained.clone();
    drifted.entries[0].member_identity = Some(8);
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &drifted,
    )
    .expect_err("retained layout identity drift must reject before sealing access");
    assert!(
        error.contains("stable identity #8") && error.contains("outside the reflected schema"),
        "unexpected diagnostic: {error}"
    );

    let mut positional = retained.clone();
    positional.entries[0].member_identity = None;
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &positional,
    )
    .expect_err("an unnumbered spelling cannot claim the numbered field identity");
    assert!(
        error.contains("positional field `legacy_status`")
            && error.contains("outside the reflected schema"),
        "unexpected diagnostic: {error}"
    );

    let mut aliased = retained.clone();
    let mut forged_alias = aliased.entries[0].clone();
    forged_alias.field = "forged_alias".into();
    aliased.entries.push(forged_alias);
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &aliased,
    )
    .expect_err("one stable identity cannot be replayed under two presentation names");
    assert!(
        error.contains("not a canonical field-identity set")
            && error.contains("identity names both `legacy_status` and `forged_alias`"),
        "unexpected diagnostic: {error}"
    );

    let mut ambiguous = retained;
    let mut forged_identity = ambiguous.entries[0].clone();
    forged_identity.member_identity = Some(8);
    ambiguous.entries.push(forged_identity);
    let error = compute_access_plan(
        &renamed.typed,
        "RegisterAccess::plan",
        "Registers",
        &ambiguous,
    )
    .expect_err("one presentation name cannot replay two stable identities");
    assert!(
        error.contains("not a canonical field-identity set")
            && error.contains(
                "identifies both stable member identity #7 and stable member identity #8"
            ),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn source_placement_policy_normalizes_layout_access_and_reach_together() {
    let main = write_program("source-placement", POLICY_SOURCE);
    let checked = compile_to_checked(&main, None).expect("source policy should compile");
    let placement = compute_placement_plan(&checked.typed, "UartPlacement::plan", "Registers")
        .expect("source placement policy should evaluate and normalize");

    assert_ne!(placement.identity().compatibility_fingerprint(), 0);
    assert_eq!(placement.layout().size, Some(24));
    assert_eq!(placement.access().field_descriptors().len(), 4);
    assert_eq!(placement.reach().services().len(), 1);
    assert_eq!(
        placement
            .reach()
            .services()
            .next()
            .expect("service reach")
            .normalized_identity(),
        19
    );
}

#[test]
fn placed_view_exposes_derived_source_accessors() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.read();
    view.transmit.write(1);
    let snapshot: u16 = view.snapshot.read();
    view.snapshot.write(snapshot);
}

data Main {}
"#,
    );
    let main = write_program("placed-view-accessors", &source);
    let checked =
        compile_to_checked(&main, None).expect("derived placed-view accessors should compile");
    let status_read = checked
        .typed
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == "PlacedField<UartPlacement,Registers,status>::read"
        })
        .expect("status read accessor");
    let conformances = checked.typed.machine_trait_conformances(status_read);
    assert_eq!(conformances.len(), 1);
    assert_eq!(conformances[0].name.as_str(), "Readable");
    assert_eq!(
        conformances[0]
            .requirement
            .as_ref()
            .expect("single readable requirement")
            .as_str(),
        "read"
    );

    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("placed-view consumer");
    let entry = checked
        .typed
        .machine_states(inspect)
        .first()
        .expect("placed-view consumer entry");
    let inputs = checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| input.machine == inspect.symbol)
        .collect::<Vec<_>>();
    let [input] = inputs.as_slice() else {
        panic!("one direct checked placed-view input")
    };
    let view = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "UartPlacement")
        .expect("source-derived placed-view plan");
    assert_eq!(input.state, entry.symbol);
    assert_eq!(input.position, 0);
    assert_eq!(input.reference_access, ReferenceAccess::Mutable);
    assert_eq!(input.view, view.data_symbol);
    assert_eq!(input.policy, view.policy_symbol);
    assert_eq!(input.policy_plan_machine, view.policy_plan_machine_symbol);
    assert_eq!(input.schema, view.schema_symbol);
    assert_eq!(input.placement, view.placement);
}

#[test]
fn subordinate_placed_view_input_retains_exact_checked_state_custody() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-subordinate-input",
        r#"
data Inspector {}
machine Inspector::inspect(
    &mut self,
    view: &mut Placed<UartPlacement, Registers>
) {
    state inspect_again(view: &mut Placed<UartPlacement, Registers>) {}
}
"#,
    );
    let checked = compile_to_checked_with_packages(&main, None, inputs)
        .expect("subordinate placed-view input should compile to checked custody");
    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inspector::inspect")
        .expect("placed-view consumer");
    let [entry, subordinate] = checked.typed.machine_states(inspect) else {
        panic!("entry and subordinate state")
    };
    let inputs = checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| input.machine == inspect.symbol)
        .collect::<Vec<_>>();
    let [entry_input, subordinate_input] = inputs.as_slice() else {
        panic!("entry and subordinate checked placed-view inputs")
    };
    assert_eq!(entry_input.state, entry.symbol);
    assert_eq!(entry_input.position, 1);
    assert_eq!(subordinate_input.state, subordinate.symbol);
    assert_eq!(subordinate_input.position, 0);
    assert_ne!(entry_input.parameter, subordinate_input.parameter);
    assert_eq!(entry_input.reference_access, ReferenceAccess::Mutable);
    assert_eq!(subordinate_input.reference_access, ReferenceAccess::Mutable);
    assert_eq!(entry_input.view, subordinate_input.view);
    assert_eq!(entry_input.policy, subordinate_input.policy);
    assert_eq!(
        entry_input.policy_plan_machine,
        subordinate_input.policy_plan_machine
    );
    assert_eq!(entry_input.schema, subordinate_input.schema);
    assert_eq!(entry_input.placement, subordinate_input.placement);
}

#[test]
fn direct_placed_view_input_crosses_terminal_with_exact_source_custody() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-terminal-input",
        r#"
data Sink {}
machine Sink::fill(destination: &write i32) {
    destination = 2;
}

data Inspector {}
machine Inspector::inspect(
    &mut self,
    view: &mut Placed<UartPlacement, Registers>,
    destination: &mut i32
) {
    Sink::fill(&write destination);
}
"#,
    );
    let checked = compile_to_checked_with_packages(&main, None, inputs)
        .expect("direct placed-view input should compile");
    let inspect = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Inspector::inspect")
        .expect("placed-view consumer");
    let input = checked
        .facts
        .placed_view_inputs
        .iter()
        .find(|input| input.machine == inspect.symbol)
        .expect("checked placed-view input");
    let lowered = lower_machine(&checked, "Inspector::inspect")
        .expect("direct placed-view input should cross the Terminal boundary");
    let [terminal_input] = lowered.semantic_module.placed_view_inputs.as_slice() else {
        panic!("one direct Terminal placed-view input")
    };
    assert_eq!(terminal_input.machine, lowered.semantic_module.entry);
    assert_eq!(terminal_input.position, input.position);
    assert_eq!(
        terminal_input.access,
        psi_terminal::StructuralAccess::MutableBorrow
    );
    assert_eq!(
        terminal_input.source_machine_identity,
        checked
            .typed
            .normalized_hermetic_symbol_identity(input.machine)
            .unwrap()
    );
    assert_eq!(
        terminal_input.source_state_identity,
        checked
            .typed
            .normalized_hermetic_symbol_identity(input.state)
            .unwrap()
    );
    assert_eq!(
        terminal_input.source_parameter_identity,
        checked
            .typed
            .normalized_hermetic_symbol_identity(input.parameter)
            .unwrap()
    );
    assert_eq!(
        terminal_input.placement_report_fingerprint,
        input.placement.identity().compatibility_fingerprint()
    );
    assert_eq!(
        terminal_input.placement_commitment,
        input.placement.content_interpretation().commitment()
    );
    let encoded = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("placed-view Terminal module should encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&encoded),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn placed_view_input_custody_excludes_open_and_nonchecked_machines() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine generic_inspect<Element>(view: &Placed<UartPlacement, Registers>) {}
machine lifetime_inspect<'view>(view: &'view Placed<UartPlacement, Registers>) {}
boundary machine boundary_inspect(view: &Placed<UartPlacement, Registers>);

data Main {}
"#,
    );
    let main = write_program("placed-view-input-fences", &source);
    let checked = compile_to_checked(&main, None)
        .expect("open and non-checked placed-view declarations should remain fenced");
    for name in ["generic_inspect", "lifetime_inspect", "boundary_inspect"] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("fenced placed-view declaration");
        assert!(
            checked
                .facts
                .placed_view_inputs
                .iter()
                .all(|input| input.machine != machine.symbol),
            "{name} must not gain checked placed-view input custody"
        );
    }
}

#[test]
fn compiler_derived_placed_accessors_retain_runtime_addresses() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.read();
    view.transmit.write(1);
    let snapshot: u16 = view.snapshot.read();
    view.snapshot.write(snapshot);
}

data Main {}
"#,
    );
    let main = write_program("placed-accessor-runtime-layout", &source);
    let checked = compile_to_checked(&main, None).expect("derived placed accessors should compile");
    let [view] = checked.typed.placed_view_plans.as_slice() else {
        panic!("fixture should derive exactly one placed view")
    };
    assert_eq!(view.fields.len(), 4);
    assert!(
        view.fields
            .iter()
            .all(|field| field.field_name != "reserved")
    );

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let layouts = build_layout_plan(&checked, target, &[]).expect("placed layout should build");
        for field in &view.fields {
            let accessor = layouts
                .data_layouts
                .iter()
                .map(|(_, layout)| layout)
                .find(|layout| layout.name.as_str() == field.accessor_name)
                .unwrap_or_else(|| {
                    panic!(
                        "accessor `{}` is missing from runtime layouts {:?}",
                        field.accessor_name,
                        layouts
                            .data_layouts
                            .iter()
                            .map(|(_, layout)| layout.name.as_str())
                            .collect::<Vec<_>>()
                    )
                });
            assert_eq!(accessor.layout.size, target.pointer_size);
            assert_eq!(accessor.layout.alignment, target.pointer_alignment);
            assert!(
                matches!(&accessor.shape, DataShape::Record { fields } if fields.is_empty()),
                "the address carrier must remain opaque rather than expose source fields"
            );
        }

        let placed = layouts
            .data_layouts
            .iter()
            .map(|(_, layout)| layout)
            .find(|layout| layout.name.as_str() == view.data_name)
            .expect("derived Placed record should have a runtime layout");
        let DataShape::Record { fields } = &placed.shape else {
            panic!("derived Placed data should remain a record")
        };
        let fields = layouts.fields.span_or_empty(*fields);
        assert_eq!(fields.len(), view.fields.len());
        assert_eq!(
            placed.layout.size,
            target.pointer_size * view.fields.len(),
            "one exact address carrier is retained for each admitted field"
        );
        assert_eq!(placed.layout.alignment, target.pointer_alignment);
        assert!(fields.iter().all(|field| field.name.as_str() != "reserved"));
        for field in fields {
            let expected = view
                .fields
                .iter()
                .find(|expected| expected.field_name == field.name.as_str())
                .expect("every runtime field should come from the exact placed-view plan");
            assert_eq!(field.type_name.as_ref(), expected.accessor_name);
            assert_eq!(field.layout.size, target.pointer_size);
            assert_eq!(field.layout.alignment, target.pointer_alignment);
        }
    }
}

#[test]
fn placed_view_plan_retains_and_replays_exact_nominal_and_member_identities() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.read();
}

data Main {}
"#,
    );
    let main = write_program("placed-view-exact-identities", &source);
    let mut checked =
        compile_to_checked(&main, None).expect("exact placed-view identities should compile");
    let [view] = checked.typed.placed_view_plans.as_slice() else {
        panic!("fixture should derive exactly one placed view")
    };
    let schema = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Registers")
        .expect("source schema");
    let view_data = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == view.data_name)
        .expect("synthesized placed view");
    let schema_symbol = schema.symbol;
    let view_data_symbol = view_data.symbol;
    let policy_symbol = view.policy_symbol;
    let policy_plan_machine_symbol = view.policy_plan_machine_symbol;
    assert_eq!(view.schema_symbol, schema_symbol);
    assert_eq!(view.data_symbol, view_data_symbol);
    let status = view
        .fields
        .iter()
        .find(|field| field.field_name == "status")
        .expect("retained status field");
    assert_eq!(status.member_identity, None);
    let [status_read_target] = status.accessor_targets.as_slice() else {
        panic!("readable status field should retain one exact generated operation target")
    };
    assert_eq!(status_read_target.operation, "read");
    let status_read_state_symbol = status_read_target.state_symbol;
    let status_field_symbol = status.field_symbol;
    let status_accessor_name = status.accessor_name.clone();
    let accessor_data = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == status.accessor_data_symbol)
        .expect("exact generated status accessor data");
    assert_eq!(accessor_data.name.as_str(), status.accessor_name);
    let status_accessor_type = status.accessor_type;
    assert_eq!(
        checked
            .typed
            .placed_field_plan_for_type_reference(status_accessor_type)
            .map(|field| field.field_symbol),
        Some(status_field_symbol)
    );
    let schema_status = checked
        .typed
        .data_members(schema)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if field.name.as_str() == "status" => {
                Some(field)
            }
            _ => None,
        })
        .expect("source status field");
    let schema_status_type = schema_status.type_reference;
    assert_eq!(status_field_symbol, schema_status.symbol);

    checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_symbol == status_field_symbol)
        .expect("retained status field")
        .accessor_name = "diagnostic-only-accessor-name".to_owned();
    assert_eq!(
        checked
            .typed
            .placed_field_plan_for_type_reference(status_accessor_type)
            .map(|field| field.field_symbol),
        Some(status_field_symbol),
        "typed accessor lookup must not use presentation spelling"
    );
    checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_symbol == status_field_symbol)
        .expect("retained status field")
        .accessor_name = status_accessor_name;
    psi_validation::validate_program(&checked.typed)
        .expect("independent exact placed-view replay should accept retained identities");

    checked.typed.placed_view_plans[0].policy_symbol = schema_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted placement-policy identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact placement-policy binding")
    }));

    checked.typed.placed_view_plans[0].policy_symbol = policy_symbol;
    checked.typed.placed_view_plans[0].policy_plan_machine_symbol = view_data_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted placement-policy plan machine must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no longer names its exact placement-policy plan machine")
    }));

    checked.typed.placed_view_plans[0].policy_plan_machine_symbol = policy_plan_machine_symbol;

    let status_plan_index = checked.typed.placed_view_plans[0]
        .fields
        .iter()
        .position(|field| field.field_symbol == status_field_symbol)
        .expect("retained status field");
    let status_plan = checked.typed.placed_view_plans[0]
        .fields
        .remove(status_plan_index);
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("missing accessible field plan must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact accessible field inventory")
    }));
    checked.typed.placed_view_plans[0]
        .fields
        .insert(status_plan_index, status_plan);

    checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_name == "status")
        .expect("retained status field")
        .member_identity = Some(8);
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted stable member identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("field `status` changed its exact source member binding")
    }));

    checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_name == "status")
        .expect("retained status field")
        .member_identity = None;
    checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_name == "status")
        .expect("retained status field")
        .accessor_type = schema_status_type;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted synthesized accessor type must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("field `status` changed its exact synthesized accessor binding")
    }));

    checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_name == "status")
        .expect("retained status field")
        .accessor_type = status_accessor_type;
    checked.typed.placed_view_plans[0].schema_symbol = view_data_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted source schema identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("field `status` no longer names its exact source field identity")
    }));

    checked.typed.placed_view_plans[0].schema_symbol = schema_symbol;
    checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_name == "status")
        .expect("retained status field")
        .accessor_targets[0]
        .state_symbol = schema_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted generated accessor state must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operation `read` changed its exact generated accessor target")
    }));

    let status = checked.typed.placed_view_plans[0]
        .fields
        .iter_mut()
        .find(|field| field.field_name == "status")
        .expect("retained status field");
    status.accessor_targets[0].state_symbol = status_read_state_symbol;
    status.accessor_data_symbol = schema_symbol;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("substituted generated accessor data identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("field `status` changed its exact generated accessor data binding")
    }));
}

#[test]
fn placed_view_allows_exported_accessors_across_package_boundary() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-exported-package",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.read();
    view.transmit.write(1);
}
"#,
    );
    let checked = compile_to_checked_with_packages(&main, None, inputs)
        .expect("exported placed accessors should remain callable from a dependent package");
    let plan = checked
        .typed
        .placed_view_plans
        .first()
        .expect("placed view plan");
    assert_eq!(
        checked
            .typed
            .symbols
            .symbol_package_identity(plan.policy_symbol),
        Some(package_identity(2))
    );
    assert_eq!(
        checked
            .typed
            .symbols
            .symbol_package_identity(plan.schema_symbol),
        Some(package_identity(2))
    );
    assert_eq!(
        checked
            .typed
            .symbols
            .symbol_package_identity(plan.data_symbol),
        None,
        "the synthetic shell is compiler-owned; its plan retains the exact package-owned policy and schema identities"
    );
}

#[test]
fn placed_view_rejects_a_private_policy_from_a_dependency() {
    let private_policy = POLICY_SOURCE.replacen("pub data UartPlacement", "data UartPlacement", 1);
    let (main, inputs) = write_cross_package_program_with_policy(
        "placed-view-private-policy-package",
        "machine inspect(view: &Placed<UartPlacement, Registers>) {}",
        &private_policy,
    );
    let diagnostics = compile_to_checked_with_packages(&main, None, inputs)
        .expect_err("a private dependency policy must not publish a placed shell");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot use private placement policy `UartPlacement` from another package")
    }));
}

#[test]
fn placed_view_rejects_a_private_schema_from_a_dependency() {
    let private_schema = POLICY_SOURCE.replacen("pub data Registers", "data Registers", 1);
    let (main, inputs) = write_cross_package_program_with_policy(
        "placed-view-private-schema-package",
        "machine inspect(view: &Placed<UartPlacement, Registers>) {}",
        &private_schema,
    );
    let diagnostics = compile_to_checked_with_packages(&main, None, inputs)
        .expect_err("a private dependency schema must not publish a placed shell");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot use private schema `Registers` from another package")
    }));
}

#[test]
fn placed_view_rejects_private_local_inputs_before_public_signature_erasure() {
    let source = POLICY_SOURCE
        .replacen("pub data Registers", "data Registers", 1)
        .replacen("pub data UartPlacement", "data UartPlacement", 1)
        .replacen(
            "data Main {}",
            "pub machine inspect(view: &Placed<UartPlacement, Registers>) {}\n\ndata Main {}",
            1,
        );
    let main = write_program("placed-view-private-public-signature", &source);
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("placed erasure must not launder a private input through a public signature");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("placement policy `UartPlacement`")
            && diagnostic.message.contains("must be public")
    }));
}

#[test]
fn placed_view_schema_requires_direct_dependency_authority() {
    const REGISTERS: &str = r#"pub data Registers {
    status: u32;
    transmit: u8;
    snapshot: u16;
    counter: u64;
    reserved: u8;
}
"#;

    let directory = std::env::temp_dir().join(format!(
        "omega-access-placed-transitive-schema-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    let root_directory = directory.join("root");
    let middle_directory = directory.join("middle");
    let leaf_directory = directory.join("leaf");
    for package in [&root_directory, &middle_directory, &leaf_directory] {
        fs::create_dir_all(package).expect("create package directory");
    }

    let root_source = POLICY_SOURCE.replacen(REGISTERS, "", 1).replacen(
        "data Main {}",
        "machine inspect(view: &Placed<UartPlacement, Registers>) {}\n\ndata Main {}",
        1,
    );
    fs::write(
        root_directory.join("main.omg"),
        format!("use middle::middle;\n{root_source}"),
    )
    .expect("write root source");
    fs::write(
        root_directory.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("root");
    builder.depend_as("middle", Source::Path { location: "../middle" });
}
"#,
    )
    .expect("write root build declaration");
    fs::write(
        middle_directory.join("middle.omg"),
        "use leaf::schema;\ndata Middle {}\n",
    )
    .expect("write middle source");
    fs::write(
        middle_directory.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("middle");
    builder.depend_as("leaf", Source::Path { location: "../leaf" });
}
"#,
    )
    .expect("write middle build declaration");
    fs::write(leaf_directory.join("schema.omg"), REGISTERS).expect("write leaf schema");
    fs::write(
        leaf_directory.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"leaf\"); }\n",
    )
    .expect("write leaf build declaration");

    let inputs = PackageCompilationInputs::new_package(
        package_identity(1),
        vec![
            PackageSourceBinding::new(package_identity(1), "root", root_directory.clone()),
            PackageSourceBinding::new(package_identity(2), "middle", middle_directory),
            PackageSourceBinding::new(package_identity(3), "leaf", leaf_directory),
        ],
        vec![
            PackageDependencyBinding::new(package_identity(1), "middle", package_identity(2)),
            PackageDependencyBinding::new(package_identity(2), "leaf", package_identity(3)),
        ],
    )
    .expect("transitive schema fixture should form a closed package graph");
    let diagnostics =
        compile_to_checked_with_packages(&root_directory.join("main.omg"), None, inputs)
            .expect_err("a transitive-only schema must not survive placed type erasure");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("placed schema selects package")
            && rendered.contains("without direct dependency authority"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_rejects_same_spelled_policy_declarations_across_packages() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-ambiguous-policy-package",
        r#"
pub data UartPlacement {}

machine inspect(view: &Placed<UartPlacement, Registers>) {}
"#,
    );
    let diagnostics = compile_to_checked_with_packages(&main, None, inputs)
        .expect_err("same-spelled package policies must not be joined by load order");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot select one exact placement policy `UartPlacement`")
    }));
}

#[test]
fn placed_view_rejects_binding_private_accessors_outside_the_policy_package() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-private-package",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let snapshot: u16 = view.snapshot.read();
}
"#,
    );
    let diagnostics = compile_to_checked_with_packages(&main, None, inputs)
        .expect_err("binding-private access must remain in the nominal policy package");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("binding-private placed accessor `snapshot`")
            && rendered.contains("placement policy `UartPlacement`'s package"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_rejects_binding_private_statement_calls_outside_the_policy_package() {
    let (main, inputs) = write_cross_package_program(
        "placed-view-private-statement-package",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    view.snapshot.write(1);
}
"#,
    );
    let diagnostics = compile_to_checked_with_packages(&main, None, inputs)
        .expect_err("a binding-private statement call must remain in the policy package");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("binding-private placed accessor `snapshot`")
            && rendered.contains("placement policy `UartPlacement`'s package"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_distinguishes_destructive_take_from_read() {
    let source = POLICY_SOURCE
        .replace("ExternalRead::Read", "ExternalRead::Take")
        .replace(
            "data Main {}",
            r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.take();
}

data Main {}
"#,
        );
    let main = write_program("placed-view-take", &source);
    let checked =
        compile_to_checked(&main, None).expect("destructive placed-view accessor should compile");
    assert!(checked.typed.machines().iter().any(|machine| {
        machine.name.as_str() == "PlacedField<UartPlacement,Registers,status>::take"
    }));
    assert!(!checked.typed.machines().iter().any(|machine| {
        machine.name.as_str() == "PlacedField<UartPlacement,Registers,status>::read"
    }));
}

#[test]
fn placed_view_omits_inaccessible_fields() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) -> u8 {
    view.reserved
}

data Main {}
"#,
    );
    let main = write_program("placed-view-inaccessible", &source);
    let diagnostics =
        compile_to_checked(&main, None).expect_err("inaccessible fields must not project");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("reserved"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_omits_operations_not_admitted_by_the_policy() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let value: u8 = view.transmit.read();
}

data Main {}
"#,
    );
    let main = write_program("placed-view-operation", &source);
    let diagnostics =
        compile_to_checked(&main, None).expect_err("write-only fields must not acquire read");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("read"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_exposes_admitted_atomic_operations() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let observed: u64 = view.counter.load(NoOrdering);
    let prior: u64 = view.counter.fetch_add(1, NoOrdering);
}

data Main {}
"#,
    );
    let main = write_program("placed-view-atomic", &source);
    let checked = compile_to_checked(&main, None)
        .expect("the admitted atomic operation subset should compile");
    let field = checked
        .typed
        .placed_view_plans
        .iter()
        .flat_map(|view| view.fields.iter())
        .find(|field| field.field_name == "counter")
        .expect("retained atomic field plan");
    assert!(field.accessor_name.starts_with("AtomicU64#PlacedField<"));
    assert!(matches!(
        &field.access,
        FieldAccess::Atomic { operations, .. }
            if operations.load && operations.fetch_add && !operations.store
    ));
}

#[test]
fn source_access_policy_retains_each_compare_exchange_axis_independently() {
    let source = POLICY_SOURCE
        .replace("try_exchange: false", "try_exchange: true")
        .replace(
            "data Main {}",
            r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let observed: u64 = view.counter.load(NoOrdering);
}

data Main {}
"#,
        );
    let main = write_program("placed-view-atomic-exchange-axes", &source);
    let checked = compile_to_checked(&main, None)
        .expect("non-observing decisive permission should remain a distinct plan fact");
    let field = checked
        .typed
        .placed_view_plans
        .iter()
        .flat_map(|view| view.fields.iter())
        .find(|field| field.field_name == "counter")
        .expect("retained atomic field plan");
    assert!(matches!(
        &field.access,
        FieldAccess::Atomic { operations, .. }
            if operations.try_exchange
                && !operations.compare_exchange
                && !operations.compare_exchange_once
                && !operations.try_exchange_once
    ));
    assert!(
        field.atomic_resident.is_none(),
        "source-formable try-only access retains no observing or selected-encoding authority"
    );
}

#[test]
fn compiler_atomic_compare_exchange_axes_retain_distinct_ordering_and_authorization() {
    let decisive = AtomicAccessOperation::CompareExchange {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    };
    let once = AtomicAccessOperation::CompareExchangeOnce {
        success: MemoryOrdering::ReceivePublish,
        failure: MemoryOrdering::Receive,
    };
    assert_eq!(
        decisive.ordering_plan(),
        AtomicOrderingPlan::CompareExchange {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        }
    );
    assert_eq!(
        once.ordering_plan(),
        AtomicOrderingPlan::CompareExchangeOnce {
            success: MemoryOrdering::ReceivePublish,
            failure: MemoryOrdering::Receive,
        }
    );
    assert_ne!(decisive.ordering_plan(), once.ordering_plan());

    for (permission, admitted, rejected) in [
        ("compare_exchange", decisive, once),
        ("compare_exchange_once", once, decisive),
    ] {
        let source = POLICY_SOURCE
            .replace(
                &format!("{permission}: false"),
                &format!("{permission}: true"),
            )
            .replace(
                "data Main {}",
                "machine retain(view: &Placed<UartPlacement, Registers>) {}\n\ndata Main {}",
            );
        let main = write_program(&format!("placed-atomic-ordering-{permission}"), &source);
        let checked = compile_to_checked(&main, None)
            .expect("one observing compare-exchange permission should compile");
        let view = checked
            .typed
            .placed_view_plans
            .iter()
            .find(|view| view.policy_name == "UartPlacement")
            .expect("compiler-derived UartPlacement plan");
        let entry = view
            .placement
            .access()
            .plan()
            .entries()
            .iter()
            .find(|entry| entry.field() == "counter")
            .expect("compiler-derived Atomic counter entry");

        view.placement
            .access()
            .authorize(
                entry.key(),
                psi_access_plans::BorrowPolarity::Shared,
                psi_access_plans::BorrowPolarity::Shared,
                AccessOperation::Atomic(admitted),
            )
            .expect("the exact authored compare-exchange axis should remain authorized");
        let diagnostic = view
            .placement
            .access()
            .authorize(
                entry.key(),
                psi_access_plans::BorrowPolarity::Shared,
                psi_access_plans::BorrowPolarity::Shared,
                AccessOperation::Atomic(rejected),
            )
            .expect_err("the sibling compare-exchange axis must not substitute");
        assert!(diagnostic.0.contains("does not permit"), "{diagnostic}");
    }
}

#[test]
fn checked_atomic_resident_contract_replays_observing_axes_and_result_shapes() {
    let source = POLICY_SOURCE
        .replace("compare_exchange: false", "compare_exchange: true")
        .replace(
            "compare_exchange_once: false",
            "compare_exchange_once: true",
        )
        .replace(
            "data Main {}",
            "machine retain(view: &Placed<UartPlacement, Registers>) {}\n\ndata Main {}",
        );
    let main = write_program("placed-atomic-resident-contract", &source);
    let mut checked = compile_to_checked(&main, None)
        .expect("copyable resident should retain both observing result contracts");
    let view_index = checked
        .typed
        .placed_view_plans
        .iter()
        .position(|view| {
            view.fields
                .iter()
                .any(|field| field.field_name == "counter")
        })
        .expect("Registers placed-view plan with its Atomic counter");
    let field_index = checked.typed.placed_view_plans[view_index]
        .fields
        .iter()
        .position(|field| field.field_name == "counter")
        .expect("retained Atomic counter field");
    let field = &checked.typed.placed_view_plans[view_index].fields[field_index];
    let retained = field
        .atomic_resident
        .as_ref()
        .expect("Atomic field retains a checked resident contract");
    assert_eq!(retained.field_symbol, field.field_symbol);
    assert_eq!(retained.resident_type, field.value_type);
    assert_eq!(retained.multiplicity, Multiplicity::Unrestricted);
    assert_eq!(retained.transfer_width_bits, 64);
    assert!(retained.compare_exchange);
    assert!(retained.compare_exchange_once);
    assert_eq!(
        retained
            .observing_results
            .iter()
            .map(|row| (row.operation, row.result_shape))
            .collect::<Vec<_>>(),
        vec![
            (
                AtomicObservingCompareExchangeOperation::Decisive,
                AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved,
            ),
            (
                AtomicObservingCompareExchangeOperation::SingleAttempt,
                AtomicObservingCompareExchangeResultShape::
                    ExchangedOrMismatchedOrUncommittedObserved,
            ),
        ]
    );
    psi_validation::validate_program(&checked.typed)
        .expect("independent replay accepts the exact resident contract");

    for (permission, expected_operation, expected_shape) in [
        (
            "compare_exchange",
            AtomicObservingCompareExchangeOperation::Decisive,
            AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved,
        ),
        (
            "compare_exchange_once",
            AtomicObservingCompareExchangeOperation::SingleAttempt,
            AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedOrUncommittedObserved,
        ),
    ] {
        let source = POLICY_SOURCE
            .replace(
                &format!("{permission}: false"),
                &format!("{permission}: true"),
            )
            .replace(
                "data Main {}",
                "machine retain(view: &Placed<UartPlacement, Registers>) {}\n\ndata Main {}",
            );
        let main = write_program(&format!("placed-atomic-resident-{permission}"), &source);
        let checked = compile_to_checked(&main, None)
            .expect("each observing permission forms one exact resident/result row");
        let resident = checked
            .typed
            .placed_view_plans
            .iter()
            .flat_map(|view| view.fields.iter())
            .find(|field| field.field_name == "counter")
            .and_then(|field| field.atomic_resident.as_ref())
            .expect("independent observing resident contract");
        assert_eq!(resident.compare_exchange, permission == "compare_exchange");
        assert_eq!(
            resident.compare_exchange_once,
            permission == "compare_exchange_once"
        );
        assert_eq!(resident.observing_results.len(), 1);
        assert_eq!(resident.observing_results[0].operation, expected_operation);
        assert_eq!(resident.observing_results[0].result_shape, expected_shape);
    }

    let original = retained.clone();
    let sibling_symbol = checked.typed.placed_view_plans[view_index]
        .fields
        .iter()
        .find(|field| field.field_name == "status")
        .expect("sibling status field")
        .field_symbol;
    let sibling_type = checked.typed.placed_view_plans[view_index]
        .fields
        .iter()
        .find(|field| field.field_name == "status")
        .expect("sibling status field")
        .value_type;

    let mut reject_drift = |mutated: psi_typed_trees::typed_trees::PlacedAtomicResidentContract,
                            description: &str| {
        checked.typed.placed_view_plans[view_index].fields[field_index].atomic_resident =
            Some(mutated);
        let diagnostics = psi_validation::validate_program(&checked.typed)
            .expect_err("resident-contract drift must fail closed");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("changed its checked Atomic resident/result contract")),
            "{description}: {diagnostics:?}"
        );
        checked.typed.placed_view_plans[view_index].fields[field_index].atomic_resident =
            Some(original.clone());
    };

    let mut drifted = original.clone();
    drifted.field_symbol = sibling_symbol;
    reject_drift(drifted, "sibling field substitution");
    let mut drifted = original.clone();
    drifted.resident_type = sibling_type;
    reject_drift(drifted, "resident type substitution");
    let mut drifted = original.clone();
    drifted.multiplicity = Multiplicity::Affine;
    reject_drift(drifted, "multiplicity substitution");
    let mut drifted = original.clone();
    drifted.transfer_width_bits = 32;
    reject_drift(drifted, "transfer-width substitution");
    let mut drifted = original.clone();
    drifted.compare_exchange = false;
    reject_drift(drifted, "decisive observing permission-axis substitution");
    let mut drifted = original.clone();
    drifted.compare_exchange_once = false;
    reject_drift(
        drifted,
        "single-attempt observing permission-axis substitution",
    );
    let mut drifted = original.clone();
    drifted.observing_results.push(drifted.observing_results[0]);
    reject_drift(drifted, "duplicate observing result row");
    let mut drifted = original.clone();
    drifted.observing_results.swap(0, 1);
    reject_drift(drifted, "canonical result-row order substitution");
    let mut drifted = original.clone();
    drifted.observing_results[0].operation = AtomicObservingCompareExchangeOperation::SingleAttempt;
    reject_drift(drifted, "observing operation-identity substitution");
    let mut drifted = original.clone();
    drifted.observing_results[1].result_shape =
        AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved;
    reject_drift(drifted, "single-attempt result-shape substitution");

    checked.typed.placed_view_plans[view_index].fields[field_index].atomic_resident = None;
    let diagnostics = psi_validation::validate_program(&checked.typed)
        .expect_err("missing resident contract must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its checked Atomic resident/result contract")
    }));
}

#[test]
fn checked_atomic_resident_contract_joins_provider_backed_runtime_custody() {
    let source = r#"
use omega::language::core::layout;

pub data Counter {
    value: u64;
}

pub data AtomicPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine AtomicPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 8
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Atomic {
                operations: AtomicOperations {
                    load: true,
                    store: false,
                    fetch_add: false,
                    fetch_sub: false,
                    fetch_xor: false,
                    fetch_or: false,
                    fetch_and: false,
                    swap: false,
                    compare_exchange: true,
                    compare_exchange_once: true,
                    try_exchange: false,
                    try_exchange_once: false
                },
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plan(counter: &Placed<AtomicPlacement, Counter>) {}

data Main {}
machine Main::main(&mut self) {}
"#;
    let main = write_program("checked-atomic-runtime-resident-join", source);
    let checked = compile_to_checked(&main, None)
        .expect("observing Atomic contract should reach checked custody");
    let view = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "AtomicPlacement")
        .expect("checked Atomic placement");
    let field = view
        .fields
        .iter()
        .find(|field| field.field_name == "value")
        .expect("checked Atomic field");
    let entry = view
        .placement
        .access()
        .plan()
        .entries()
        .iter()
        .find(|entry| !matches!(entry.access(), FieldAccess::Inaccessible))
        .expect("canonical Atomic access entry");
    let operations = match entry.access() {
        FieldAccess::Atomic { operations, .. } => *operations,
        other => panic!("expected Atomic access, got {other:?}"),
    };

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        451,
        ExtentRightId::from_normalized_identity,
    )]);
    let (extent, content) = ExtentRootGrant::from_admitted_provider(
        provider_issuance(29),
        extent_identity(452, ExtentLineageId::from_normalized_identity),
        extent_identity(453, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(454, ExtentProvenanceId::from_normalized_identity),
        extent_identity(455, MappingEraId::from_normalized_identity),
    )
    .mint_provider_existing_content(
        0xb000,
        8,
        view.placement.content_interpretation(),
        extent_identity(456, ResidentClaimId::from_normalized_identity),
        extent_identity(
            457,
            ExtentContentValidityReceiptId::from_normalized_identity,
        ),
        extent_identity(458, ExtentContentCustodyReceiptId::from_normalized_identity),
    )
    .expect("provider-backed Atomic content");
    let profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(459).expect("profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("Atomic profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
            stable: StableCapability::None,
            external: ExternalCapability::None,
            atomic: AtomicCapability::Access {
                transfers: vec![AtomicTransferRule {
                    transfer: TransferRule {
                        width_bits: 64,
                        alignment_bytes: 8,
                    },
                    operations,
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted Atomic profile");
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(460).expect("placement admission");
    let admission = admit_owned_placement(admission_id, extent, &view.placement, &profile)
        .expect("owned Atomic placement admission");
    let dormant = adopt_owned_atomic(admission, content).expect("Atomic resident adoption");
    let resident_claim = dormant.resident_claim();
    let occurrence = PlacedOccurrenceId::from_normalized_identity(461).expect("placed occurrence");
    let established = dormant.view(occurrence).expect("Atomic resident view");

    let request_snapshot = |access: &psi_access_plans::AtomicPrimitiveAccessRequest<'_, '_>| {
        let request = access.primitive_request();
        (
            access.operation(),
            request.plan(),
            request.admission(),
            request.effective_supply().key(),
            request.transfer_width_bits(),
            request.resident_claim(),
            request.placed_occurrence(),
        )
    };

    let projection = established
        .project(entry.key())
        .expect("provider-backed Atomic projection");
    let access = projection
        .atomic_compare_exchange_once(
            psi_language_core::atomic::MemoryOrdering::ReceivePublish,
            psi_language_core::atomic::MemoryOrdering::Receive,
        )
        .expect("single-attempt observing access")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("Atomic specialization");
    let snapshot = request_snapshot(&access);

    let rejection = psi_validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.policy_symbol,
        field.field_symbol,
        access,
    )
    .expect_err("a policy symbol cannot substitute the exact placed-view identity");
    assert!(
        rejection
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("no exact placed-view identity"))
    );
    let (access, diagnostics) = rejection.into_parts();
    assert!(!diagnostics.is_empty());
    assert_eq!(request_snapshot(&access), snapshot);

    let rejection = psi_validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        view.policy_symbol,
        access,
    )
    .expect_err("a policy symbol cannot substitute the exact checked field identity");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no exact checked Atomic field identity")
    }));
    let (access, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&access), snapshot);

    let joined = psi_validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        access,
    )
    .expect("exact checked/runtime resident join");
    assert_eq!(
        joined.resident_contract(),
        field.atomic_resident.as_ref().expect("resident contract")
    );
    assert_eq!(
        joined.observing_result().operation,
        AtomicObservingCompareExchangeOperation::SingleAttempt
    );
    assert_eq!(
        joined.observing_result().result_shape,
        AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedOrUncommittedObserved
    );
    assert_eq!(
        joined.atomic_access().primitive_request().resident_claim(),
        Some(resident_claim)
    );
    assert_eq!(
        joined
            .atomic_access()
            .primitive_request()
            .placed_occurrence(),
        Some(occurrence)
    );
    joined
        .validate_for_result_custody()
        .expect("post-construction replay preserves both authorities");
    let access = joined.into_atomic_access();
    assert_eq!(request_snapshot(&access), snapshot);

    let mut drifted = checked.clone();
    let drifted_view_symbol = view.data_symbol;
    let drifted_field_symbol = field.field_symbol;
    {
        let drifted_view = drifted
            .typed
            .placed_view_plans
            .iter_mut()
            .find(|candidate| candidate.data_symbol == drifted_view_symbol)
            .expect("drifted checked view");
        let drifted_field = drifted_view
            .fields
            .iter_mut()
            .find(|candidate| candidate.field_symbol == drifted_field_symbol)
            .expect("drifted checked field");
        drifted_field
            .atomic_resident
            .as_mut()
            .expect("drifted resident contract")
            .observing_results[1]
            .result_shape =
            AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved;
    }
    let rejection = psi_validation::bind_checked_atomic_resident_access(
        &drifted.typed,
        drifted_view_symbol,
        drifted_field_symbol,
        access,
    )
    .expect_err("result-shape drift must reject before runtime custody handoff");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its checked Atomic resident/result contract")
    }));
    let (access, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&access), snapshot);
    let joined = psi_validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        access,
    )
    .expect("unchanged request supports corrected checked-contract retry");
    let _access = joined.into_atomic_access();

    let projection = established
        .project(entry.key())
        .expect("provider-backed Atomic load projection");
    let load = projection
        .atomic_load(psi_language_core::atomic::MemoryOrdering::Receive)
        .expect("resident Atomic load")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("Atomic load specialization");
    let rejection = psi_validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        load,
    )
    .expect_err("non-observing Atomic operations cannot consume the result-shape contract");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("accepts only observing decisive or single-attempt")
    }));
    let (load, _) = rejection.into_parts();
    load.validate_for_lowering()
        .expect("rejection returns the unchanged non-observing request");

    let shifted_source = source
        .replace(
            "placement: FieldPlan::At { offset: 0 }",
            "placement: FieldPlan::At { offset: 8 }",
        )
        .replace("size_fixed: 8", "size_fixed: 16");
    let shifted_main = write_program("checked-atomic-runtime-resident-shifted", &shifted_source);
    let shifted = compile_to_checked(&shifted_main, None).expect("shifted checked Atomic plan");
    let shifted_view = shifted
        .typed
        .placed_view_plans
        .iter()
        .find(|candidate| candidate.policy_name == "AtomicPlacement")
        .expect("shifted checked view");
    let shifted_field = shifted_view
        .fields
        .iter()
        .find(|candidate| candidate.field_name == "value")
        .expect("shifted checked field");
    let projection = established
        .project(entry.key())
        .expect("provider-backed decisive projection");
    let decisive = projection
        .atomic_compare_exchange(
            psi_language_core::atomic::MemoryOrdering::ReceivePublish,
            psi_language_core::atomic::MemoryOrdering::Receive,
        )
        .expect("decisive observing access")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("decisive Atomic specialization");
    let decisive_snapshot = request_snapshot(&decisive);
    let rejection = psi_validation::bind_checked_atomic_resident_access(
        &shifted.typed,
        shifted_view.data_symbol,
        shifted_field.field_symbol,
        decisive,
    )
    .expect_err("a distinct checked placement cannot substitute for runtime custody");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("placement structure differs from the independently checked placement")
    }));
    let (decisive, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&decisive), decisive_snapshot);
    let joined = psi_validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        decisive,
    )
    .expect("decisive observing contract joins exact resident custody");
    assert_eq!(
        joined.observing_result().operation,
        AtomicObservingCompareExchangeOperation::Decisive
    );
    let _decisive = joined.into_atomic_access();

    let ordinary_extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(30),
        extent_identity(462, ExtentLineageId::from_normalized_identity),
        extent_identity(463, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(464, ExtentProvenanceId::from_normalized_identity),
        extent_identity(465, MappingEraId::from_normalized_identity),
    )
    .mint(0xb100, 8)
    .expect("ordinary Atomic extent");
    let ordinary_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(466).expect("ordinary receipt"),
        &ordinary_extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("ordinary Atomic profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
            stable: StableCapability::None,
            external: ExternalCapability::None,
            atomic: AtomicCapability::Access {
                transfers: vec![AtomicTransferRule {
                    transfer: TransferRule {
                        width_bits: 64,
                        alignment_bytes: 8,
                    },
                    operations,
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("ordinary Atomic profile");
    let ordinary_loan = ordinary_extent.loan(0, 8).expect("ordinary Atomic loan");
    let ordinary_view = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(467).expect("ordinary admission"),
            ordinary_loan,
            &view.placement,
            &ordinary_profile,
        )
        .expect("ordinary Atomic placement admission"),
    )
    .expect("ordinary Atomic placed view");
    let ordinary_projection = ordinary_view
        .project(entry.key())
        .expect("ordinary Atomic projection");
    let ordinary = ordinary_projection
        .atomic_compare_exchange_once(
            psi_language_core::atomic::MemoryOrdering::ReceivePublish,
            psi_language_core::atomic::MemoryOrdering::Receive,
        )
        .expect("ordinary observing request")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("ordinary Atomic specialization");
    let ordinary_snapshot = request_snapshot(&ordinary);
    let rejection = psi_validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        ordinary,
    )
    .expect_err("correspondence-free ordinary Atomic storage has no resident custody");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lacks runtime resident custody")
    }));
    let (ordinary, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&ordinary), ordinary_snapshot);
    ordinary
        .validate_for_lowering()
        .expect("missing-custody rejection returns the exact Atomic request");
}

#[test]
fn placed_view_exposes_each_individually_admitted_atomic_family() {
    let source = POLICY_SOURCE
        .replace("store: false", "store: true")
        .replace("fetch_sub: false", "fetch_sub: true")
        .replace("fetch_xor: false", "fetch_xor: true")
        .replace("fetch_or: false", "fetch_or: true")
        .replace("fetch_and: false", "fetch_and: true")
        .replace("swap: false", "swap: true")
        .replace("compare_exchange: false", "compare_exchange: true")
        .replace(
            "data Main {}",
            r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let observed: u64 = view.counter.load(NoOrdering);
    view.counter.store(observed, NoOrdering);
    let added: u64 = view.counter.fetch_add(1, NoOrdering);
    let subtracted: u64 = view.counter.fetch_sub(1, NoOrdering);
    let xored: u64 = view.counter.fetch_xor(1, NoOrdering);
    let ored: u64 = view.counter.fetch_or(1, NoOrdering);
    let anded: u64 = view.counter.fetch_and(1, NoOrdering);
    let swapped: u64 = view.counter.swap(1, NoOrdering);
    let exchanged: u64 = view.counter.compare_exchange(1, 2, NoOrdering, NoOrdering);
}

data Main {}
"#,
        );
    let main = write_program("placed-view-atomic-families", &source);
    compile_to_checked(&main, None)
        .expect("each individually admitted atomic operation family should compile");
}

#[test]
fn placed_view_atomic_accessor_cannot_materialize_as_an_ordinary_value() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let leaked: u64 = view.counter;
}

data Main {}
"#,
    );
    let main = write_program("placed-view-atomic-leak", &source);
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("an atomic accessor must not coerce into its carried primitive");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("counter") && rendered.contains("accessor, not an ordinary value"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_cannot_recast_around_its_admitted_policy() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine reinterpret(view: Placed<UartPlacement, Registers>) {
    let alias: &Placed<UartPlacement, Registers> =
        &view as &Placed<UartPlacement, Registers>;
}

data Main {}
"#,
    );
    let main = write_program("placed-view-recast", &source);
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a placed view must not be reconstructed through recast");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("placed-view recast")
            && rendered.contains("explicitly admit the intended placement"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_rejects_atomic_operations_outside_the_plan() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    view.counter.store(1, NoOrdering);
    let subtracted: u64 = view.counter.fetch_sub(1, NoOrdering);
    let xored: u64 = view.counter.fetch_xor(1, NoOrdering);
    let ored: u64 = view.counter.fetch_or(1, NoOrdering);
    let anded: u64 = view.counter.fetch_and(1, NoOrdering);
    let swapped: u64 = view.counter.swap(1, NoOrdering);
    let exchanged: u64 = view.counter.compare_exchange(1, 2, NoOrdering, NoOrdering);
}

data Main {}
"#,
    );
    let main = write_program("placed-view-atomic-denied", &source);
    let diagnostics =
        compile_to_checked(&main, None).expect_err("the placement does not admit atomic store");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for operation in [
        "store",
        "fetch_sub",
        "fetch_xor",
        "fetch_or",
        "fetch_and",
        "swap",
        "compare_exchange",
    ] {
        assert!(
            rendered.contains(&format!("does not admit `{operation}`")),
            "missing `{operation}` diagnostic: {rendered}"
        );
    }
}

#[test]
fn placed_view_rejects_atomic_access_for_an_unsupported_schema_primitive() {
    let source = POLICY_SOURCE
        .replace("counter: u64;", "counter: u16;")
        .replace(
            "data Main {}",
            r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {}

data Main {}
"#,
        );
    let main = write_program("placed-view-atomic-width", &source);
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("placed atomics are currently limited to supported atomic primitives");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("counter")
            && rendered.contains("requires schema type `bool`, `u32`, or `u64`"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn source_access_policy_requires_one_decision_per_schema_field() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
data Missing {
    entries: [AccessFieldEntry; 32];
}
machine Missing::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(schema);
    transition { _ -> truncate(plan) }
    state truncate(plan: AccessPlan) -> AccessPlan {
        let mut partial: AccessPlan = plan;
        partial.field_count = 1;
        partial
    }
}
data Main {}
"#,
    );
    let main = write_program("missing-access-slot", &source);
    let checked = compile_to_checked(&main, None).expect("invalid policy source should compile");
    let layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers")
        .expect("layout should validate");
    let error = compute_access_plan(&checked.typed, "Missing::plan", "Registers", &layout)
        .expect_err("a partial source access plan must reject");
    assert!(
        error.contains("requires exactly 5 decisions"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn aggregate_fields_admit_only_inaccessible_access_decisions() {
    let main = write_program(
        "aggregate-access",
        r#"
use omega::language::core::layout;

data Samples { values: [u16; 3]; }
data ArrayLayout { entries: [FieldEntry; 64]; }
machine ArrayLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    Plan { entries: self.entries, entry_count: 1,
           size_fixed: 6, size_is_dynamic: false, align: 2 }
}

data ArrayAccess {}
machine ArrayAccess::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(schema);
    plan.with(
        schema.fields[0].key,
        FieldAccess::Stable {
            read: true,
            write: false,
            exposure: Exposure::BindingPrivate,
        },
    )
}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(&main, None).expect("access policy source should type");
    let layout = compute_layout_plan(&checked.typed, "ArrayLayout::plan", "Samples")
        .expect("the aggregate At layout should validate");
    let error = compute_access_plan(&checked.typed, "ArrayAccess::plan", "Samples", &layout)
        .expect_err("aggregate Stable access must remain outside this layout slice");
    assert!(error.contains("admits only Inaccessible for aggregate fields"));
}

#[test]
fn inaccessible_seed_rejects_foreign_replacement_keys() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
data Forged {}
machine Forged::plan(schema: Schema, layout: Plan) -> AccessPlan
satisfies Access::plan
{
    let plan: AccessPlan = AccessPlan::inaccessible(schema);
    transition { _ -> replace(plan) }
    state replace(plan: AccessPlan) -> AccessPlan {
        plan.with(
            999,
            FieldAccess::Stable {
                read: true,
                write: false,
                exposure: Exposure::Exported
            }
        )
    }
}
data Main {}
"#,
    );
    let main = write_program("forged-access-key", &source);
    let checked = compile_to_checked(&main, None).expect("forged policy source should compile");
    let layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers")
        .expect("layout should validate");
    let error = compute_access_plan(&checked.typed, "Forged::plan", "Registers", &layout)
        .expect_err("foreign schema keys must not be ignored by the seed replacement helper");
    assert!(
        error.contains("access field_count is 6") && error.contains("requires exactly 5 decisions"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn access_evaluation_rejects_a_forged_layout_report() {
    let main = write_program("forged-layout-report", POLICY_SOURCE);
    let checked = compile_to_checked(&main, None).expect("source policy should compile");
    let mut layout = compute_layout_plan(&checked.typed, "UartLayout::plan", "Registers")
        .expect("layout should validate");
    layout.offsets = Some(vec![0, 4, 6, 8, 17]);

    let error = compute_access_plan(&checked.typed, "UartAccess::plan", "Registers", &layout)
        .expect_err("access evaluation must revalidate its supposedly validated layout input");
    assert!(
        error.contains("is not the canonical validated layout"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn source_derived_stable_plan_binds_owned_content_and_preserves_retry_custody() {
    let main = write_program(
        "source-stable-owned-adoption",
        r#"
use omega::language::core::layout;

pub data Word {
    word: u32;
}

pub data HomePlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine HomePlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Stable {
                read: true,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

pub data ShiftedPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine ShiftedPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 }
    };
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Stable {
                read: true,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plans(
    home: &Placed<HomePlacement, Word>,
    shifted: &Placed<ShiftedPlacement, Word>
) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(&main, None)
        .expect("both source-derived Stable placements should reach checked custody");
    let home = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "HomePlacement")
        .expect("home checked placement row");
    let shifted = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "ShiftedPlacement")
        .expect("shifted checked placement row");
    assert_ne!(home.policy_symbol, shifted.policy_symbol);
    assert_ne!(home.placement.identity(), shifted.placement.identity());
    assert_eq!(home.placement.layout().size, Some(8));
    assert_eq!(shifted.placement.layout().size, Some(8));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        401,
        ExtentRightId::from_normalized_identity,
    )]);
    let (extent, content) = ExtentRootGrant::from_admitted_provider(
        provider_issuance(25),
        extent_identity(402, ExtentLineageId::from_normalized_identity),
        extent_identity(403, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(404, ExtentProvenanceId::from_normalized_identity),
        extent_identity(405, MappingEraId::from_normalized_identity),
    )
    .mint_provider_existing_content(
        0x8000,
        8,
        home.placement.content_interpretation(),
        extent_identity(406, ResidentClaimId::from_normalized_identity),
        extent_identity(
            407,
            ExtentContentValidityReceiptId::from_normalized_identity,
        ),
        extent_identity(408, ExtentContentCustodyReceiptId::from_normalized_identity),
    )
    .expect("provider-owned existing Stable content");
    let extent_snapshot = (
        extent.origin(),
        extent.lineage_root(),
        extent.base(),
        extent.length(),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent.era(),
    );
    let content_snapshot = (
        content.origin(),
        content.lineage_root(),
        content.base(),
        content.length(),
        content.address_space(),
        content.provenance(),
        content.era(),
        content.interpretation(),
        content.resident_claim(),
        content.validity_receipt(),
        content.custody_receipt(),
    );
    let profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(409).expect("profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("provider profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
            stable: StableCapability::ReadWrite,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted Stable profile");
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(410).expect("placement admission");
    let mismatched = admit_owned_placement(admission_id, extent, &shifted.placement, &profile)
        .expect("shifted placement is independently geometry/resource compatible");
    let rejection = adopt_owned_stable(mismatched, content)
        .expect_err("provider content must bind the exact checked source placement");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("interpretation commitment does not match the admitted placement")
    );
    let (returned_admission, returned_content, _) = rejection.into_parts();
    assert_eq!(returned_admission.identity(), admission_id);
    assert_eq!(returned_admission.placement_plan(), &shifted.placement);
    assert_eq!(
        (
            returned_admission.extent().origin(),
            returned_admission.extent().lineage_root(),
            returned_admission.extent().base(),
            returned_admission.extent().length(),
            returned_admission.extent().address_space(),
            returned_admission.extent().rights().clone(),
            returned_admission.extent().provenance(),
            returned_admission.extent().era(),
        ),
        extent_snapshot,
    );
    assert_eq!(
        (
            returned_content.origin(),
            returned_content.lineage_root(),
            returned_content.base(),
            returned_content.length(),
            returned_content.address_space(),
            returned_content.provenance(),
            returned_content.era(),
            returned_content.interpretation(),
            returned_content.resident_claim(),
            returned_content.validity_receipt(),
            returned_content.custody_receipt(),
        ),
        content_snapshot,
    );

    let returned_extent = returned_admission.withdraw();
    let corrected = admit_owned_placement(admission_id, returned_extent, &home.placement, &profile)
        .expect("the returned exact extent supports corrected admission");
    let dormant = adopt_owned_stable(corrected, returned_content)
        .expect("returned content supports exact source-plan retry");
    assert_eq!(dormant.admission(), admission_id);
    assert_eq!(dormant.placement_plan(), &home.placement);
    assert_eq!(
        (
            dormant.extent().origin(),
            dormant.extent().lineage_root(),
            dormant.extent().base(),
            dormant.extent().length(),
            dormant.extent().address_space(),
            dormant.extent().rights().clone(),
            dormant.extent().provenance(),
            dormant.extent().era(),
        ),
        extent_snapshot,
    );
    assert_eq!(dormant.resident_claim(), content_snapshot.8);
    assert_eq!(dormant.validity_receipt(), content_snapshot.9);
    assert_eq!(dormant.custody_receipt(), content_snapshot.10);
}

#[test]
fn source_derived_external_plan_binds_correspondence_and_preserves_retry_custody() {
    let main = write_program(
        "source-external-correspondence",
        r#"
use omega::language::core::layout;

pub data Register {
    value: u32;
}

pub data HomePlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine HomePlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::External {
                read: ExternalRead::Read,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

pub data ShiftedPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine ShiftedPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 4 }
    };
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::External {
                read: ExternalRead::Read,
                write: true,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plans(
    home: &Placed<HomePlacement, Register>,
    shifted: &Placed<ShiftedPlacement, Register>
) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(&main, None)
        .expect("both source-derived External placements should reach checked custody");
    let home = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "HomePlacement")
        .expect("home checked placement row");
    let shifted = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "ShiftedPlacement")
        .expect("shifted checked placement row");
    assert_ne!(home.policy_symbol, shifted.policy_symbol);
    assert_ne!(home.placement.identity(), shifted.placement.identity());
    assert_eq!(home.placement.layout().size, Some(8));
    assert_eq!(shifted.placement.layout().size, Some(8));
    assert!(matches!(
        home.placement
            .access()
            .plan()
            .entries()
            .first()
            .expect("home External field")
            .access(),
        FieldAccess::External {
            read: ExternalRead::Read,
            write: true,
            ..
        }
    ));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        411,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(26),
        extent_identity(412, ExtentLineageId::from_normalized_identity),
        extent_identity(413, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(414, ExtentProvenanceId::from_normalized_identity),
        extent_identity(415, MappingEraId::from_normalized_identity),
    )
    .mint(0x9000, 8)
    .expect("provider External extent");
    let loan_snapshot = (
        extent.origin(),
        extent.lineage_root(),
        extent.base(),
        extent.length(),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent.era(),
    );
    let external_profile = ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
            stable: StableCapability::None,
            external: ExternalCapability::Access {
                read: ExternalReadBehavior::Repeatable,
                write: true,
                transfers: vec![TransferRule {
                    width_bits: 32,
                    alignment_bytes: 4,
                }],
            },
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    };
    let exact_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(416).expect("exact profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("exact provider profile grant")
    .admit(external_profile.clone())
    .expect("exact admitted External profile");
    let alternate_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(417).expect("alternate profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("alternate provider profile grant")
    .admit(external_profile)
    .expect("alternate admitted External profile");

    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(418)
        .expect("correspondence provider");
    let device =
        StableDeviceInstanceId::from_normalized_identity(419).expect("stable device identity");
    let source =
        SchemaCorrespondenceSourceId::from_normalized_identity(420).expect("correspondence source");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        source,
        &home.placement,
        exact_profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&home.placement, &exact_profile)
    .expect("source-derived correspondence admission");
    let correspondence_snapshot = (
        correspondence.provider(),
        correspondence.device(),
        correspondence.source(),
        correspondence.placement(),
        correspondence.profile_receipt(),
    );
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(421).expect("placement admission");

    let loan = extent.loan(0, 8).expect("shared External loan");
    let wrong_plan = admit_placement(admission_id, loan, &shifted.placement, &exact_profile)
        .expect("shifted source plan is independently resource-compatible");
    let rejection = bind_schema_correspondence_to_placement(wrong_plan, correspondence)
        .expect_err("correspondence must reject a different retained source plan");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (wrong_plan, correspondence, _) = rejection.into_parts();
    assert_eq!(wrong_plan.identity(), admission_id);
    assert_eq!(wrong_plan.profile_receipt(), exact_profile.receipt());
    assert_eq!(
        (
            correspondence.provider(),
            correspondence.device(),
            correspondence.source(),
            correspondence.placement(),
            correspondence.profile_receipt(),
        ),
        correspondence_snapshot,
    );
    let loan = wrong_plan.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let wrong_profile = admit_placement(admission_id, loan, &home.placement, &alternate_profile)
        .expect("alternate profile is independently resource-compatible");
    let rejection = bind_schema_correspondence_to_placement(wrong_profile, correspondence)
        .expect_err("correspondence must reject a different admitted profile receipt");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("resource-profile receipt")
    );
    let (wrong_profile, correspondence, _) = rejection.into_parts();
    assert_eq!(wrong_profile.identity(), admission_id);
    assert_eq!(wrong_profile.profile_receipt(), alternate_profile.receipt());
    assert_eq!(
        (
            correspondence.provider(),
            correspondence.device(),
            correspondence.source(),
            correspondence.placement(),
            correspondence.profile_receipt(),
        ),
        correspondence_snapshot,
    );
    let loan = wrong_profile.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let corrected = admit_placement(admission_id, loan, &home.placement, &exact_profile)
        .expect("returned loan supports exact source-plan/profile retry");
    let bound = bind_schema_correspondence_to_placement(corrected, correspondence)
        .expect("returned correspondence supports exact retry");
    assert_eq!(bound.admission(), admission_id);
    assert_eq!(
        (
            bound.correspondence().provider(),
            bound.correspondence().device(),
            bound.correspondence().source(),
            bound.correspondence().placement(),
            bound.correspondence().profile_receipt(),
        ),
        correspondence_snapshot,
    );
    let (loan, correspondence) = bound.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );
    assert_eq!(
        (
            correspondence.provider(),
            correspondence.device(),
            correspondence.source(),
            correspondence.placement(),
            correspondence.profile_receipt(),
        ),
        correspondence_snapshot,
    );
}

#[test]
fn source_derived_atomic_plan_rejects_underpowered_profile_and_preserves_retry_custody() {
    let main = write_program(
        "source-atomic-profile-retry",
        r#"
use omega::language::core::layout;

pub data Counter {
    value: u32;
}

pub data AtomicPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine AtomicPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 4,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Atomic {
                operations: AtomicOperations {
                    load: true,
                    store: false,
                    fetch_add: true,
                    fetch_sub: false,
                    fetch_xor: false,
                    fetch_or: false,
                    fetch_and: false,
                    swap: false,
                    compare_exchange: false,
                    compare_exchange_once: false,
                    try_exchange: false,
                    try_exchange_once: false
                },
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plan(counter: &Placed<AtomicPlacement, Counter>) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(&main, None)
        .expect("source-derived Atomic placement should reach checked custody");
    let retained = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "AtomicPlacement")
        .expect("checked Atomic placement row");
    let atomic_access = retained
        .placement
        .access()
        .plan()
        .entries()
        .first()
        .expect("retained Atomic field")
        .access();
    assert!(matches!(
        atomic_access,
        FieldAccess::Atomic { operations, .. }
            if operations.load && operations.fetch_add && !operations.store
    ));
    assert_eq!(retained.placement.layout().size, Some(4));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        422,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(27),
        extent_identity(423, ExtentLineageId::from_normalized_identity),
        extent_identity(424, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(425, ExtentProvenanceId::from_normalized_identity),
        extent_identity(426, MappingEraId::from_normalized_identity),
    )
    .mint(0xa000, 4)
    .expect("provider Atomic extent");
    let loan_snapshot = (
        extent.origin(),
        extent.lineage_root(),
        extent.base(),
        extent.length(),
        extent.address_space(),
        extent.rights().clone(),
        extent.provenance(),
        extent.era(),
    );
    let underpowered_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(427)
            .expect("underpowered profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("underpowered provider profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 4,
            stable: StableCapability::None,
            external: ExternalCapability::None,
            atomic: AtomicCapability::Access {
                transfers: vec![AtomicTransferRule {
                    transfer: TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    },
                    operations: AtomicPermissions {
                        load: true,
                        ..AtomicPermissions::default()
                    },
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted load-only Atomic profile");
    let exact_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(428).expect("exact profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("exact provider profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 4,
            stable: StableCapability::None,
            external: ExternalCapability::None,
            atomic: AtomicCapability::Access {
                transfers: vec![AtomicTransferRule {
                    transfer: TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    },
                    operations: AtomicPermissions {
                        load: true,
                        fetch_add: true,
                        ..AtomicPermissions::default()
                    },
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted exact Atomic profile");

    let admission_id =
        PlacementAdmissionId::from_normalized_identity(429).expect("Atomic placement admission");
    let loan = extent.loan(0, 4).expect("shared Atomic loan");
    let rejection = admit_placement(
        admission_id,
        loan,
        &retained.placement,
        &underpowered_profile,
    )
    .expect_err("load-only supply must not satisfy source-requested fetch-add");
    assert!(
        rejection.diagnostic().0.contains("value")
            && rejection.diagnostic().0.contains("operation families"),
        "unexpected Atomic profile diagnostic: {}",
        rejection.diagnostic().0,
    );
    let (loan, _) = rejection.into_parts();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let admission = admit_placement(admission_id, loan, &retained.placement, &exact_profile)
        .expect("returned loan supports exact Atomic plan/profile retry");
    assert_eq!(admission.identity(), admission_id);
    assert_eq!(admission.profile_receipt(), exact_profile.receipt());
    assert_eq!(
        admission.resources().placement(),
        retained.placement.identity()
    );
    assert_eq!(
        admission.resources().profile(),
        exact_profile.profile().identity(),
    );
    let fields = admission.resources().fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind(), EffectiveSupplyKind::Atomic);
    assert_eq!(fields[0].offset(), 0);
    assert_eq!(fields[0].width_bits(), 32);
    assert_eq!(fields[0].alignment_bytes(), 4);

    let loan = admission.withdraw();
    assert_eq!(
        (
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );
}

#[test]
fn source_derived_take_plan_rejects_repeatable_profile_and_preserves_retry_custody() {
    let main = write_program(
        "source-take-profile-retry",
        r#"
use omega::language::core::layout;

pub data Fifo {
    sample: u32;
}

pub data DestructivePlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine DestructivePlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 4,
            size_is_dynamic: false,
            align: 4
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::External {
                read: ExternalRead::Take,
                write: false,
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plan(fifo: &mut Placed<DestructivePlacement, Fifo>) {}

data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let checked = compile_to_checked(&main, None)
        .expect("source-derived destructive External placement should reach checked custody");
    let retained = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "DestructivePlacement")
        .expect("checked destructive External placement row");
    assert!(matches!(
        retained
            .placement
            .access()
            .plan()
            .entries()
            .first()
            .expect("retained destructive External field")
            .access(),
        FieldAccess::External {
            read: ExternalRead::Take,
            write: false,
            ..
        }
    ));
    assert_eq!(retained.placement.layout().size, Some(4));

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        430,
        ExtentRightId::from_normalized_identity,
    )]);
    let mut extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(28),
        extent_identity(431, ExtentLineageId::from_normalized_identity),
        extent_identity(432, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(433, ExtentProvenanceId::from_normalized_identity),
        extent_identity(434, MappingEraId::from_normalized_identity),
    )
    .mint(0xb000, 4)
    .expect("provider destructive External extent");
    let repeatable_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(435)
            .expect("Repeatable profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("Repeatable provider profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 4,
            stable: StableCapability::None,
            external: ExternalCapability::Access {
                read: ExternalReadBehavior::Repeatable,
                write: false,
                transfers: vec![TransferRule {
                    width_bits: 32,
                    alignment_bytes: 4,
                }],
            },
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted Repeatable External profile");
    let destructive_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(436)
            .expect("Destructive profile receipt"),
        &extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("Destructive provider profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 4,
            stable: StableCapability::None,
            external: ExternalCapability::Access {
                read: ExternalReadBehavior::Destructive,
                write: false,
                transfers: vec![TransferRule {
                    width_bits: 32,
                    alignment_bytes: 4,
                }],
            },
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted Destructive External profile");

    let admission_id = PlacementAdmissionId::from_normalized_identity(437)
        .expect("destructive External placement admission");
    let loan = extent
        .loan_mut(0, 4)
        .expect("exclusive destructive External loan");
    let loan_snapshot = (
        loan.polarity(),
        loan.origin(),
        loan.lineage_root(),
        loan.base(),
        loan.length(),
        loan.address_space(),
        loan.rights().clone(),
        loan.provenance(),
        loan.era(),
    );
    let rejection = admit_placement(admission_id, loan, &retained.placement, &repeatable_profile)
        .expect_err("Repeatable supply must not satisfy source-requested destructive Take");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("field `sample` requests incompatible External 32-bit read=Take write=false"),
        "unexpected destructive External diagnostic: {}",
        rejection.diagnostic().0,
    );
    let (loan, _) = rejection.into_parts();
    assert_eq!(
        (
            loan.polarity(),
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );

    let admission = admit_placement(
        admission_id,
        loan,
        &retained.placement,
        &destructive_profile,
    )
    .expect("returned loan supports exact destructive External plan/profile retry");
    assert_eq!(admission.identity(), admission_id);
    assert_eq!(admission.profile_receipt(), destructive_profile.receipt());
    assert_eq!(
        admission.resources().placement(),
        retained.placement.identity()
    );
    assert_eq!(
        admission.resources().profile(),
        destructive_profile.profile().identity(),
    );
    let fields = admission.resources().fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind(), EffectiveSupplyKind::External);
    assert_eq!(fields[0].offset(), 0);
    assert_eq!(fields[0].width_bits(), 32);
    assert_eq!(fields[0].alignment_bytes(), 4);

    let loan = admission.withdraw();
    assert_eq!(
        (
            loan.polarity(),
            loan.origin(),
            loan.lineage_root(),
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.rights().clone(),
            loan.provenance(),
            loan.era(),
        ),
        loan_snapshot,
    );
}

//! Source-visible L6b policy evaluation. These tests keep the source record
//! vocabulary, build-time interpreter, and sealed normalized access model on
//! one end-to-end path.

use std::fs;
use std::path::PathBuf;

use omega_compiler::{
    compile_to_checked, compute_access_plan, compute_layout_plan, compute_placement_plan,
};
use omega_layout::{DataShape, build_layout_plan};
use omega_target::NativeTarget;
use psi_access_plans::{AccessExposure, ExternalRead, FieldAccess, ObservationModel};

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("omega-access-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create access-plan test directory");
    let main = directory.join("main.omg");
    fs::write(&main, source).expect("write access-plan test program");
    main
}

fn write_cross_package_program(name: &str, consumer: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("omega-access-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    let policy_directory = directory.join("policy");
    fs::create_dir_all(&policy_directory).expect("create policy package directory");
    let policy_end = POLICY_SOURCE
        .find("data Main {}")
        .expect("policy fixture main marker");
    fs::write(
        policy_directory.join("policy.omg"),
        &POLICY_SOURCE[..policy_end],
    )
    .expect("write policy package");
    fs::write(
        directory.join("build.omg"),
        r#"
machine build(b: &mut Build) {
    b.depend("policy", path("policy"));
}
"#,
    )
    .expect("write root build manifest");
    let main = directory.join("main.omg");
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
    main
}

const POLICY_SOURCE: &str = r#"
use omega::language::core::layout;

data Registers {
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

data UartPlacement {
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

    assert_ne!(access.identity().normalized_identity(), 0);
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

    assert_ne!(placement.identity().normalized_identity(), 0);
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
        let layouts = build_layout_plan(&checked, target).expect("placed layout should build");
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
    assert_eq!(status.field_symbol, schema_status.symbol);
    psi_validation::validate_program(&checked.typed)
        .expect("independent exact placed-view replay should accept retained identities");

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
}

#[test]
fn placed_view_allows_exported_accessors_across_package_boundary() {
    let main = write_cross_package_program(
        "placed-view-exported-package",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    let status: u32 = view.status.read();
    view.transmit.write(1);
}
"#,
    );
    compile_to_checked(&main, None)
        .expect("exported placed accessors should remain callable from a dependent package");
}

#[test]
fn placed_view_rejects_binding_private_accessors_outside_the_policy_package() {
    let main = write_cross_package_program(
        "placed-view-private-package",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let snapshot: u16 = view.snapshot.read();
}
"#,
    );
    let diagnostics = compile_to_checked(&main, None)
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
    let main = write_cross_package_program(
        "placed-view-private-statement-package",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    view.snapshot.write(1);
}
"#,
    );
    let diagnostics = compile_to_checked(&main, None)
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

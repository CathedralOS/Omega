//! Source-visible L6b policy evaluation. These tests keep the source record
//! vocabulary, build-time interpreter, and sealed normalized access model on
//! one end-to-end path.

use std::fs;
use std::path::PathBuf;

use omega_access_plans::{AccessExposure, ExternalRead, FieldAccess, ObservationModel};
use omega_compiler::{
    compile_to_checked, compute_access_plan, compute_layout_plan, compute_placement_plan,
};

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("omega-access-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create access-plan test directory");
    let main = directory.join("main.omg");
    fs::write(&main, source).expect("write access-plan test program");
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
                    compare_exchange: false
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
                        compare_exchange: false
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
fn placed_view_keeps_atomic_projection_closed_until_exact_accessors_land() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let value: u64 = view.counter.load(Relaxed);
}

data Main {}
"#,
    );
    let main = write_program("placed-view-atomic-closed", &source);
    let diagnostics = compile_to_checked(&main, None)
        .expect_err("a built-in atomic carrier would widen the admitted operation subset");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("counter"),
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

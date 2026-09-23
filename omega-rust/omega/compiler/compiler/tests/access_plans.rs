//! Source-visible L6b policy evaluation. These tests keep the source record
//! vocabulary, build-time interpreter, and sealed normalized access model on
//! one end-to-end path.
//!
//! Fixtures shared by the access plan tests: corpus sources and
//! cross-package programs.

#[path = "access_plans/atomic_contracts_and_forged_reports.rs"]
mod atomic_contracts_and_forged_reports;
#[path = "fixture_rosters/access_plans.rs"]
mod fixture_roster;
// Reuse the host linker/executor without modifying the shared differential owner.
#[path = "../../../../../tests/native-differential/tests/common/native_function.rs"]
#[allow(dead_code)]
mod native_function;
#[path = "access_plans/placed_view_authority.rs"]
mod placed_view_authority;
#[path = "access_plans/source_access_policies.rs"]
mod source_access_policies;

use std::fs;
use std::path::PathBuf;

use extents::ExtentProviderIssuance;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;

fn corpus_source(fixture: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass")
        .join(fixture)
        .join("main.omg")
}

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
    constructor: fn(u64) -> Result<T, extents::ExtentDiagnostic>,
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
    builder.application("access_plans");
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
            PackageSourceBinding::new(package_identity(1), "access_plans", root_directory),
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
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    owned_entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 4 }
    };
    owned_entries[2] = FieldEntry {
        key: schema.fields[2].key,
        placement: FieldPlan::At { offset: 6 }
    };
    owned_entries[3] = FieldEntry {
        key: schema.fields[3].key,
        placement: FieldPlan::At { offset: 8 }
    };
    owned_entries[4] = FieldEntry {
        key: schema.fields[4].key,
        placement: FieldPlan::At { offset: 16 }
    };
    Plan {
        entries: owned_entries,
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
    let plan: AccessPlan = AccessPlan::inaccessible(&schema);
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
    services: [u64; 32];
}

machine UartPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut layout_entries: [FieldEntry; 64];
    layout_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    layout_entries[1] = FieldEntry {
        key: schema.fields[1].key,
        placement: FieldPlan::At { offset: 4 }
    };
    layout_entries[2] = FieldEntry {
        key: schema.fields[2].key,
        placement: FieldPlan::At { offset: 6 }
    };
    layout_entries[3] = FieldEntry {
        key: schema.fields[3].key,
        placement: FieldPlan::At { offset: 8 }
    };
    layout_entries[4] = FieldEntry {
        key: schema.fields[4].key,
        placement: FieldPlan::At { offset: 16 }
    };
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    transition { _ -> place_status(schema, access, layout_entries) }

    state place_status(
        &mut self,
        schema: Schema,
        access: AccessPlan,
        layout_entries: [FieldEntry; 64]
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
            ),
            layout_entries
        ) }
    }

    state place_transmit(
        &mut self,
        schema: Schema,
        access: AccessPlan,
        layout_entries: [FieldEntry; 64]
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
            ),
            layout_entries
        ) }
    }

    state place_snapshot(
        &mut self,
        schema: Schema,
        access: AccessPlan,
        layout_entries: [FieldEntry; 64]
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
            ),
            layout_entries
        ) }
    }

    state place_counter(
        &mut self,
        schema: Schema,
        access: AccessPlan,
        layout_entries: [FieldEntry; 64]
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
            ),
            layout_entries
        ) }
    }

    state finish(&mut self, access: AccessPlan, layout_entries: [FieldEntry; 64]) -> PlacementPlan {
    self.services[0] = 19;
    PlacementPlan {
        layout: Plan {
            entries: layout_entries,
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

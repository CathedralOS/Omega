use super::{
    POLICY_SOURCE, package_identity, write_cross_package_program,
    write_cross_package_program_with_policy, write_program,
};
use access_plans::{AccessOperation, AtomicAccessOperation, FieldAccess};
use compiler::{CheckedCompileRequest, compile_to_checked};
use language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::fs;

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
    let mut checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact placed-view identities should compile")
        .into_program();
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
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "status" => {
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
    validation::validate_program(&checked.typed)
        .expect("independent exact placed-view replay should accept retained identities");

    checked.typed.placed_view_plans[0].policy_symbol = schema_symbol;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("substituted placement-policy identity must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its exact placement-policy binding")
    }));

    checked.typed.placed_view_plans[0].policy_symbol = policy_symbol;
    checked.typed.placed_view_plans[0].policy_plan_machine_symbol = view_data_symbol;
    let diagnostics = validation::validate_program(&checked.typed)
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
    let diagnostics = validation::validate_program(&checked.typed)
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
    let diagnostics = validation::validate_program(&checked.typed)
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
    let diagnostics = validation::validate_program(&checked.typed)
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
    let diagnostics = validation::validate_program(&checked.typed)
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
    let diagnostics = validation::validate_program(&checked.typed)
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
    let diagnostics = validation::validate_program(&checked.typed)
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
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
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
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
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
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
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
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
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
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root_directory.join("main.omg"), None)
    })
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
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
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
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
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
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, None)
    })
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("destructive placed-view accessor should compile");
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
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("inaccessible fields must not project");
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
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("write-only fields must not acquire read");
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
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
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
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
        let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
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
                access_plans::BorrowPolarity::Shared,
                access_plans::BorrowPolarity::Shared,
                AccessOperation::Atomic(admitted),
            )
            .expect("the exact authored compare-exchange axis should remain authorized");
        let diagnostic = view
            .placement
            .access()
            .authorize(
                entry.key(),
                access_plans::BorrowPolarity::Shared,
                access_plans::BorrowPolarity::Shared,
                AccessOperation::Atomic(rejected),
            )
            .expect_err("the sibling compare-exchange axis must not substitute");
        assert!(diagnostic.0.contains("does not permit"), "{diagnostic}");
    }
}

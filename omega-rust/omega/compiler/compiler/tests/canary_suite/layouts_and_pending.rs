use super::*;

#[path = "../fixture_rosters/layouts_and_pending.rs"]
pub(super) mod fixture_roster;

#[test]
fn plan_laid_value_field_exit_canary_runs() {
    // PLAN-LAID VALUE TYPES (layouts L4): `gdt: Spread16<Gdtish>` places every
    // field on its own 16-byte slot -- deliberately NOT the native packing --
    // and the program writes, whole-value-copies, and reads back through those
    // baked plan offsets. Exit 71 = some consumer recomputed native offsets
    // independently (a read landed in a padding gap).
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_VALUE_FIELD_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-plan-laid-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("plan-laid value canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("plan-laid value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("plan-laid value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the spread-placed fields (7/20/3/40) to survive the copy and read back \
         intact (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn plan_laid_erased_field_is_semantic_but_not_physical() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_ERASED_FIELD_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("plan-laid erased field should reach checked semantics");

    let semantic = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Spread16<Certified>")
        .expect("synthesized plan-laid semantic definition");
    let semantic_fields = checked.typed.data_members(semantic);
    assert_eq!(semantic_fields.len(), 3, "semantic identity retains proof");
    assert!(semantic_fields.iter().any(|member| {
        matches!(member, typed_trees::data::DataMember::Field(field)
            if field.name.as_str() == "proof" && field.relevance.is_erased())
    }));

    let plan = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|plan| plan.data_name == "Spread16<Certified>")
        .expect("validated physical plan");
    assert_eq!(plan.offsets, [0, 16], "erased proof has no plan entry");
    assert_eq!(plan.size, 32);

    let layouts = layout::build_layout_plan(&checked, target::NativeTarget::host(), &[])
        .expect("erased-stripped native layout should build");
    let physical = layouts
        .data_layouts
        .iter()
        .find(|(_, layout)| layout.name.as_str() == "Spread16<Certified>")
        .map(|(_, layout)| layout)
        .expect("synthesized physical layout");
    let layout::DataShape::Record { fields } = physical.shape else {
        panic!("plan-laid value must remain a record");
    };
    let physical_fields = layouts.fields.span_or_empty(fields);
    assert_eq!(
        physical_fields
            .iter()
            .map(|field| (field.name.as_str(), field.offset))
            .collect::<Vec<_>>(),
        [("left", 0), ("right", 16)]
    );
    assert_eq!(interpret(&checked, &[]).exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-erased-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("plan-laid erased field should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("plan-laid erased-field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("run plan-laid erased-field canary");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(output.status.code(), Some(70));
}

#[test]
fn distinct_closed_erased_generic_sums_run_with_exact_identities() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISTINCT_CLOSED_ERASED_SUMS_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("distinct closed erased sums should reach checked semantics");

    for expected in ["Maybe<i32>", "Maybe<bool>"] {
        let definition = checked
            .typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == expected)
            .unwrap_or_else(|| panic!("missing exact synthesized identity {expected}"));
        let some = checked
            .typed
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == "Some" =>
                {
                    Some(variant)
                }
                _ => None,
            })
            .expect("Some variant");
        assert!(
            checked
                .typed
                .data_payload_fields(some)
                .iter()
                .any(|field| { field.name.as_str() == "proof" && field.relevance.is_erased() })
        );
    }
    assert_eq!(interpret(&checked, &[]).exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-distinct-closed-erased-sums-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("distinct closed erased sums should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("distinct closed erased sums should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("run distinct closed erased sums canary");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(output.status.code(), Some(70));
}

#[test]
fn mixed_closed_generic_erasure_runs_with_common_and_payload_fields() {
    let canary = pass_canary(fixture_roster::RUNTIME_MIXED_GENERIC_ERASED_SUM_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("mixed closed erased sum should reach checked semantics");
    let definition = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Mixed<i32 in Wrapping>")
        .expect("exact synthesized mixed identity");
    assert!(checked.typed.data_members(definition).iter().any(|member| {
        matches!(member, typed_trees::data::DataMember::Field(field)
            if field.name.as_str() == "proof" && field.relevance.is_erased())
    }));
    assert_eq!(interpret(&checked, &[]).exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-mixed-closed-erased-sum-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("mixed closed erased sum should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mixed closed erased sum should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("run mixed closed erased sum canary");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(output.status.code(), Some(70));
}

#[test]
fn generic_erased_literals_use_exact_call_and_return_contexts() {
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_EXACT_CALL_RETURN_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("exact call/return contexts should reach checked semantics");
    assert_eq!(interpret(&checked, &[]).exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-generic-exact-call-return-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("exact call/return contexts should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("exact call/return generic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("run exact call/return generic canary");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(output.status.code(), Some(70));
}

#[test]
fn wire_erased_field_is_semantic_but_not_encoded() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ERASED_FIELD_ROUNDTRIP_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("wire erased field should reach checked semantics");
    let schema = checked
        .typed
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == "CertifiedMessage")
        .expect("wire schema");
    let fields = checked
        .typed
        .wire_members(schema.members)
        .iter()
        .filter_map(|member| match member {
            typed_trees::wire::WireMember::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 4, "semantic schema retains erased evidence");
    assert!(
        fields
            .iter()
            .any(|field| { field.name.as_str() == "proof" && field.relevance.is_erased() })
    );
    assert!(
        fields
            .iter()
            .any(|field| { field.name.as_str() == "certificate" && field.relevance.is_erased() })
    );
    let plan = checked
        .typed
        .wire_schema_plans
        .iter()
        .find(|plan| plan.schema == schema.symbol)
        .expect("normalized wire plan");
    assert_eq!(
        checked
            .typed
            .wire_placements
            .span_or_empty(plan.placements)
            .iter()
            .map(|placement| placement.tag())
            .collect::<Vec<_>>(),
        [0, 2]
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None, "wire interpreter error");
    assert_eq!(interpreted.exit_code, 70);

    let build_dir =
        std::env::temp_dir().join(format!("omega-wire-erased-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("wire erased field should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("wire erased field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("run wire erased field canary");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(output.status.code(), Some(70));
}

#[test]
fn nested_wire_erased_field_is_not_encoded() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_NESTED_ERASED_FIELD_ROUNDTRIP_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("nested erased wire field should reach checked semantics");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None, "nested wire interpreter error");
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-wire-nested-erased-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("nested erased wire field should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested erased wire field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("run nested erased wire field canary");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(output.status.code(), Some(70));
}

#[test]
fn plan_laid_compact_bits_exit_canary_runs_and_cross_compiles() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_COMPACT_BITS_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("compact-bit plan-laid canary should compile to checked trees");
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must preserve compact logical fields"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-plan-laid-bits-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("compact-bit plan-laid canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("compact-bit plan-laid canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("compact-bit plan-laid canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native compact-bit writes and reads must preserve every sibling and fragment; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-bits-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target)
        .unwrap_or_else(|diagnostics| {
            panic!(
                "compact-bit plan-laid projection should cross-compile for {target}: {diagnostics:?}"
            )
        });
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_integer_at_projection_exit_canary_runs_and_cross_compiles() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_INTEGER_AT_PROJECTION_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("IntegerAt projection canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter must apply the same signed/unsigned stored-width decode"
    );
    let build_dir =
        std::env::temp_dir().join(format!("omega-plan-laid-integer-at-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("IntegerAt projection canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("IntegerAt projection canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("IntegerAt projection canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "stored signed/unsigned integers must extend into their semantic carriers; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-integer-at-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target).unwrap_or_else(
            |diagnostics| {
                panic!("IntegerAt projection should cross-compile for {target}: {diagnostics:?}")
            },
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_integer_at_total_write_exit_canary_runs_and_cross_compiles() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_INTEGER_AT_TOTAL_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-integer-at-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("total IntegerAt mutation canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("total IntegerAt mutation canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("total IntegerAt mutation canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-integer-at-write-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "total IntegerAt mutation should cross-compile for {target}: {diagnostics:?}"
                )
            },
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_integer_at_proved_write_exit_canary_runs_and_cross_compiles() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_INTEGER_AT_PROVED_WRITE_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("proved-fit IntegerAt mutation canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.exit_code, 72,
        "interpreter must preserve proved-fit direct and mutable-recast IntegerAt writes"
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-integer-at-proved-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("proved-fit IntegerAt mutation canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("proved-fit IntegerAt mutation canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("proved-fit IntegerAt mutation canary should run");
    assert_eq!(output.status.code(), Some(72));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-integer-at-proved-write-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                "proved-fit IntegerAt mutation should cross-compile for {target}: {diagnostics:?}"
            )
            },
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_integer_at_unproved_write_stays_rejected() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_INTEGER_AT_PROVED_WRITE_EXIT);
    let source = fs::read_to_string(canary.join("main.omg"))
        .expect("read proved-fit IntegerAt canary")
        .replace("source: i64 [-128..=127];", "source: i64;")
        .replace("    self.source = -9;\n", "");
    let temp_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-integer-at-unproved-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create unproved IntegerAt source directory");
    let main_path = temp_dir.join("main.omg");
    fs::write(&main_path, source).expect("write unproved IntegerAt source");

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(temp_dir.join("build")),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect_err("an unconstrained value must not narrow into IntegerAt storage");
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn plan_laid_integer_at_unproved_mutable_recast_write_stays_rejected() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_INTEGER_AT_PROVED_WRITE_EXIT);
    let source = fs::read_to_string(canary.join("main.omg"))
        .expect("read proved-fit IntegerAt canary")
        .replace("source: i64 [-128..=127];", "source: i64;")
        .replace("    self.source = -9;\n", "")
        .replace("    self.packed.value = self.source;\n", "");
    let temp_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-integer-at-unproved-recast-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create unproved IntegerAt recast source directory");
    let main_path = temp_dir.join("main.omg");
    fs::write(&main_path, source).expect("write unproved IntegerAt recast source");

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(temp_dir.join("build")),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect_err("an unconstrained value must not narrow through an IntegerAt mutable recast");
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn plan_laid_value_by_value_param_exit_canary_runs() {
    // PLAN-LAID VALUE TYPES across a BY-VALUE parameter (layouts L4): the same
    // `Spread16<Gdtish>` spread placement must survive being handed to a state
    // by value and threaded across further edges. Exit 71 = a callee copy
    // recomputed native packing (a read landed in a padding gap).
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_VALUE_BY_VALUE_PARAM_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-plan-laid-byval-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("plan-laid by-value-param canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("plan-laid by-value-param canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("plan-laid by-value-param canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected spread-placed fields (7/20/3/40) to survive a by-value param pass \
         (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn plan_laid_record_view_exit_canary_runs() {
    // A byte-region record view whose only field is a plan-laid foreign
    // record. Both engines must consume the plan's low@8/high@24 offsets and
    // preserve projected scalar loads through u16->u64 widening and u64->u16
    // narrowing; native packing would read the gaps and exit 71.
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_RECORD_VIEW_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("plan-laid record-view canary should compile to checked trees");
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must decode the validated plan offsets"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-plan-laid-view-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("plan-laid record-view canary should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("plan-laid record-view canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native projection must consume the validated plan offsets; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-view-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target)
        .unwrap_or_else(|diagnostics| {
            panic!("plan-laid projected scalar widening/narrowing should cross-compile for {target}: {diagnostics:?}")
        });
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_fixed_array_record_view_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_FIXED_ARRAY_VIEW_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("plan-laid fixed-array view should compile to checked trees");
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must tile the fixed array at its validated field offset"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-fixed-array-view-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("plan-laid fixed-array view should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("plan-laid fixed-array view should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("plan-laid fixed-array view should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-fixed-array-view-{target}-{}",
            std::process::id()
        ));
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "plan-laid fixed-array view should cross-compile for {target}: {diagnostics:?}"
                )
            },
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_fixed_array_mutable_view_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_FIXED_ARRAY_MUTABLE_WRITE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("mutable plan-laid fixed-array view should compile to checked trees");
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must write array elements through the validated extent"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-fixed-array-mutable-view-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("mutable plan-laid fixed-array view should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable plan-laid fixed-array view should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mutable plan-laid fixed-array view should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native array writes must consume the validated extent; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-fixed-array-mutable-view-{target}-{}",
            std::process::id()
        ));
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "mutable plan-laid fixed-array view should cross-compile for {target}: {diagnostics:?}"
                )
            },
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_nested_fixed_array_mutable_view_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_PLAN_LAID_NESTED_FIXED_ARRAY_MUTABLE_WRITE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("mutable plan-laid nested-array view should compile to checked trees");
    assert_eq!(interpret(&checked, &[]).exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-nested-array-mutable-view-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("mutable plan-laid nested-array view should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable plan-laid nested-array view should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mutable plan-laid nested-array view should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-nested-array-mutable-view-{target}-{}",
            std::process::id()
        ));
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "mutable plan-laid nested-array view should cross-compile for {target}: {diagnostics:?}"
                )
            },
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn plan_laid_nested_record_mutable_view_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_NESTED_RECORD_MUTABLE_WRITE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("mutable plan-laid nested-record view should compile to checked trees");
    assert_eq!(interpret(&checked, &[]).exit_code, 70);

    let scratch = std::env::temp_dir().join(format!(
        "omega-plan-laid-nested-record-mutable-view-{}",
        std::process::id()
    ));
    let host_scratch = scratch.join("host");
    let compilation = compile_rooted_canary_for_native_host(&canary, host_scratch.join("out"))
        .expect("mutable plan-laid nested-record view should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable plan-laid nested-record view should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mutable plan-laid nested-record view should run");
    assert_eq!(output.status.code(), Some(70));

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_scratch = scratch.join(target);
        compile_rooted_canary_for_target(&canary, cross_scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "mutable plan-laid nested-record view should cross-compile for {target}: {diagnostics:?}"
                )
            },
        );
    }
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn plan_laid_fixed_record_array_mutable_view_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_RECORD_ARRAY_MUTABLE_WRITE_EXIT);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("mutable plan-laid fixed-record-array view should compile to checked trees");
    assert_eq!(interpret(&checked, &[]).exit_code, 70);

    let scratch = std::env::temp_dir().join(format!(
        "omega-plan-laid-record-array-mutable-view-{}",
        std::process::id()
    ));
    let host_scratch = scratch.join("host");
    let compilation = compile_rooted_canary_for_native_host(&canary, host_scratch.join("out"))
        .expect("mutable plan-laid fixed-record-array view should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable plan-laid fixed-record-array view should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mutable plan-laid fixed-record-array view should run");
    assert_eq!(output.status.code(), Some(70));

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_scratch = scratch.join(target);
        compile_rooted_canary_for_target(&canary, cross_scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "mutable plan-laid fixed-record-array view should cross-compile for {target}: {diagnostics:?}"
                )
            },
        );
    }
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn plan_laid_mutable_record_view_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_PLAN_LAID_RECORD_MUTABLE_WRITE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("mutable plan-laid record view should compile to checked trees");
    assert_eq!(
        interpret(&checked, &[]).exit_code,
        70,
        "the interpreter must write and reread the validated plan offsets"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-plan-laid-mutable-view-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("mutable plan-laid record view should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable plan-laid record view should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mutable plan-laid record-view canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native writes must consume the validated plan offsets; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["windows_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-plan-laid-mutable-view-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compile_rooted_canary_for_target(&canary, cross_dir.join("build"), target).unwrap_or_else(
            |diagnostics| {
                panic!(
                "mutable plan-laid record view should cross-compile for {target}: {diagnostics:?}"
            )
            },
        );
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn value_call_sequential_result_slots_exit_canary_runs() {
    // Two sequential value-position calls where callee 1 (`f`) has an internal
    // `let rr = r * r` binding and callee 2 (`g`) takes MORE arguments.
    //
    // Root cause: the leaf branch expansion for `f` fired at the StateCall op,
    // emitting a copy from frame[rr] to frame[a1_result] BEFORE the callee's
    // spliced LocalStorage op wrote `rr = r*r = 9` into frame[rr].  The stale
    // read (rr == 0 at that point) set a1_result = 0, so `a1 + 61` yielded 61
    // and the program exited 71.
    //
    // After the fix: the deferral condition detects callee-body LocalStorage
    // ops after the StateCall and defers the leaf expansion to after the LAST
    // such op, so `rr` is written before the copy fires.
    let canary = pass_canary(fixture_roster::VALUE_CALL_SEQUENTIAL_RESULT_SLOTS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-sequential-result-slots-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sequential result slots canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("sequential result slots canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sequential result slots canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a1 = f(3) = 9, a2 = g(5,8) = 40, self.v = a1 + 61 = 70 (exit 70); \
         exit 71 = a1 was 0 (stale read: leaf expansion fired before rr was written), \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn arithmetic_domain_saturating_const_fold_exit_canary_runs() {
    // Decision 17 / task #39: const*const Saturating overflow must clamp, not wrap.
    let canary = pass_canary(fixture_roster::ARITHMETIC_DOMAIN_SATURATING_CONST_FOLD_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-sat-const-fold-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("saturating const-fold canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating const-fold canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating const-fold canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u8 Saturating 100*100 to clamp to 255 (exit 70); exit 71 = wrapped to 16          (const-fold dropped the domain). got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn value_call_sequential_self_capture_exit_canary_runs() {
    // Regression coverage for sequential value-position calls whose terminal
    // value is a SELF-CAPTURED local (`let s = self.seed; transition { _ -> s }`)
    // -- a bare-local return, not arithmetic. This is the self-capture variant
    // of the leaf-expansion stale-read family; it is already handled by the
    // callee-body LocalStorage deferral landed for
    // value_call_sequential_result_slots_exit (the captured-field READ is a
    // LocalStorage op the deferral waits on). Pinned so the bare self-capture
    // shape cannot silently regress. exit 70 = a1 = cap() = self.seed = 9,
    // a2 = add(40) = 49, self.v = a1 + 61 = 70.
    let canary = pass_canary(fixture_roster::VALUE_CALL_SEQUENTIAL_SELF_CAPTURE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-sequential-self-capture-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sequential self-capture canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("sequential self-capture canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sequential self-capture canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a1 = cap() = self.seed = 9, a2 = add(40) = 49, self.v = a1 + 61 = 70; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_f64_state_arg_exit_canary_runs() {
    // Verifies that f64 values forwarded through a transition arm state
    // argument (`transition { _ -> store_flt(x) }`) arrive with the correct
    // IEEE-754 bits in the callee state.  Previously the Float literal path
    // was absent from `static_runtime_argument_value`, so the 8-byte parameter
    // slot was never written and the callee received zero-bits.  Bug 11
    // (2026-06-12): fixed in argument_materialization.rs by adding an explicit
    // ExpressionNode::Float branch that writes the bit-pattern via
    // WriteRuntimeStorageInteger.  exit 72 = bad_flt (wrong bits); exit 71 =
    // bad_int (regression); exit 70 = both args arrived correctly.
    let canary = pass_canary(fixture_roster::RUNTIME_F64_STATE_ARG_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-f64-state-arg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("f64 state arg canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f64 state arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f64 state arg (3.14 > 3.0) and i32 state arg (42 == 42) to pass (exit 70); \
         exit 72 = f64 bits wrong, exit 71 = i32 wrong, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_let_local_nested_state_arg_exit_canary_runs() {
    // Verifies that a `let`-bound local whose initializer is a pure place read
    // (e.g. `let slot: i32 = self.s.count`) is forwarded correctly through a
    // nested state argument chain across repeated calls.  Previously argument
    // materialization folded the Name expression back to its initializer
    // (re-evaluating `self.s.count`) instead of reading the LocalStorage frame
    // slot that captured the pre-mutation value.  On the second call the
    // already-incremented count was substituted, causing `try1` to take the
    // wrong dispatch arm.  Bug 10 (2026-06-12): fixed in
    // argument_materialization.rs by blocking the fold when the local has a
    // LocalStorage slot and its initializer is a pure place expression.  exit
    // 72 = wrong arm (set2 taken instead of set1); exit 70 = correct.
    let canary = pass_canary(fixture_roster::RUNTIME_LET_LOCAL_NESTED_STATE_ARG_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-let-local-nested-state-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("let-local nested state arg canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("let-local nested state arg canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("let-local nested state arg canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected arr[0]==500 and arr[1]==200 after two `put` calls (exit 70); \
         exit 72 = set2 arm wrongly taken (slot re-read post-increment), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn pending_canaries_reproduce_known_gaps() {
    // COLLECT-ALL, not first-panic: a drifted member is a PROMOTION signal,
    // and one promotion must not hide another (the serial-umbrella masking
    // pattern).
    let mut drifted: Vec<String> = Vec::new();

    for canary in ACTIVE_PENDING_CANARIES {
        let canary_dir = pending_canary(canary.path);
        let result = compile_canary_without_output(&canary_dir);
        match canary.expectation {
            PendingCanaryExpectation::CurrentlyAccepts => {
                if let Err(diagnostics) = result {
                    let combined = diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n");
                    drifted.push(format!(
                        "{} now REJECTS. Promote it to fail/ and update the suite.\nactual diagnostics:\n{}",
                        canary_dir.display(),
                        combined
                    ));
                }
            }
            PendingCanaryExpectation::CurrentlyRejects { fragment } => {
                let diagnostics = match result {
                    Ok(report) => {
                        drifted.push(format!(
                            "{} no longer rejects. Promote it to pass/fail and update the suite: {}",
                            canary_dir.display(),
                            report.summary()
                        ));
                        continue;
                    }
                    Err(diagnostics) => diagnostics,
                };
                let combined = diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n");

                if !combined.contains(fragment) {
                    drifted.push(format!(
                        "{} rejected DIFFERENTLY than expected (fragment {:?}).\nactual diagnostics:\n{}",
                        canary_dir.display(),
                        fragment,
                        combined
                    ));
                }
            }
        }
    }

    assert!(
        drifted.is_empty(),
        "{} pending canary(ies) drifted:\n\n{}",
        drifted.len(),
        drifted.join("\n\n")
    );
}

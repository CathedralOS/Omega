use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile,
    compile_rooted_canary_for_native_host,
    compile_rooted_canary_for_native_host_with_auxiliary_artifacts, fs,
    hosted_main_program_entry_build_for, pass_canary,
};

#[test]
fn runtime_entry_computed_result_exit_canary_runs() {
    // An ordinary value helper returns its computed terminal through result
    // scratch; the rooted Unit entry consumes it and exits explicitly.
    let canary = pass_canary(fixture_roster::RUNTIME_ENTRY_RETURN_FIELD_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-entry-return-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation =
        compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, build_dir.clone())
            .expect("computed helper return canary should compile");
    let footprint_artifact = fs::read_to_string(build_dir.join("08_boundary_footprints.json"))
        .expect("computed entry return footprint evidence should be written");
    assert!(
        footprint_artifact.contains("\"origin\": \"exit_result_registers\"")
            && footprint_artifact.contains("\"enumeration_complete\": false"),
        "runtime helper result load must retain result-register evidence without claiming final completeness"
    );
    assert_native_exit_code(
        &compilation,
        200,
        "computed entry-result canary",
        "the rooted entry should return the computed helper value",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_entry_unary_result_exit_canary_runs() {
    // A runtime logical-NOT terminal computes through one-byte helper-result
    // scratch; the rooted Unit entry dispatches on the returned bool.
    let canary = pass_canary(fixture_roster::RUNTIME_ENTRY_UNARY_RESULT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-entry-unary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime unary helper return canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "unary entry-result canary",
        "the rooted entry should dispatch on the returned logical negation",
    );
    let _ = fs::remove_dir_all(&build_dir);

    let cross_dir =
        std::env::temp_dir().join(format!("omega-entry-unary-arm64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&cross_dir);
    let src_dir = cross_dir.join("src");
    let out_dir = cross_dir.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "linux_arm64"),
    )
    .expect("write build source");
    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("runtime unary entry return should cross-compile for AArch64");
    fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let _ = fs::remove_dir_all(&cross_dir);
}

#[test]
fn runtime_entry_cast_result_exit_canary_runs() {
    // A runtime u8-to-i32 terminal cast uses the ordinary conversion writer in
    // helper-result scratch, then returns the widened value to the Unit entry.
    let canary = pass_canary(fixture_roster::RUNTIME_ENTRY_CAST_RESULT_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-entry-cast-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime cast helper return canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "cast entry-result canary",
        "the rooted entry should return the widened u8 helper value",
    );
    let _ = fs::remove_dir_all(&build_dir);

    let cross_dir =
        std::env::temp_dir().join(format!("omega-entry-cast-arm64-{}", std::process::id()));
    let _ = fs::remove_dir_all(&cross_dir);
    let src_dir = cross_dir.join("src");
    let out_dir = cross_dir.join("out");
    fs::create_dir_all(&src_dir).expect("scratch source directory");
    fs::copy(canary.join("main.omg"), src_dir.join("main.omg")).expect("copy canary");
    fs::write(
        src_dir.join("build.omg"),
        hosted_main_program_entry_build_for(&canary, "linux_arm64"),
    )
    .expect("write build source");
    compile(CanaryCompileSpec {
        root_path: src_dir.join("main.omg"),
        build_dir: Some(out_dir.clone()),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("runtime cast entry return should cross-compile for AArch64");
    fs::read(out_dir.join("omega-program")).expect("read emitted AArch64 ELF");
    let _ = fs::remove_dir_all(&cross_dir);
}

#[test]
fn runtime_entry_nested_binary_result_exit_canary_runs() {
    // Recursive runtime value operands preserve nested arithmetic instead of
    // requiring each immediate child of the terminal binary to be a place.
    let canary = pass_canary(fixture_roster::RUNTIME_ENTRY_NESTED_BINARY_RESULT_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-entry-nested-binary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested binary helper return canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "nested-binary entry-result canary",
        "the rooted entry should return the nested arithmetic helper value",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_entry_scalar_operation_results_exit_canaries_run() {
    // The shared pre-resolved scalar writer covers both builtin calls and
    // comparison-valued binaries at an entry terminal.
    for fixture in fixture_roster::ENTRY_SCALAR_OPERATION_RESULTS {
        let name = fixture.name;
        let expected = fixture.expected;
        let canary = pass_canary(fixture.path);
        let build_dir = std::env::temp_dir().join(format!("omega-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile(CanaryCompileSpec {
            root_path: canary.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        })
        .expect("scalar-operation entry return canary should compile");
        let executable = compilation.checked_native_executable_path().unwrap_or_else(|| {
            panic!("scalar-operation entry return canary `{name}` lost its exact executable receipt")
        });
        let output = Command::new(executable)
            .output()
            .expect("scalar-operation entry return canary should run");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "unexpected entry result for {name}; got {:?}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn free_standing_helper_result_canary_runs() {
    let canary = pass_canary(fixture_roster::FREE_STANDING_MACHINE_HELPER_COMPILE);
    let build_dir = std::env::temp_dir().join(format!("omega-entry-helper-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("free-standing terminal helper should compile");
    assert_native_exit_code(
        &compilation,
        7,
        "free-standing helper-result canary",
        "the free-standing add helper should return its exact result",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_loop_patterns_exit_canary_runs() {
    // Loop patterns via self-transition: a LARGE counting loop (1..10000) stays
    // iterative (no stack growth) and nested loops re-initialize the inner counter.
    // Guards the state-recursion lowering that serious apps lean on.
    let canary = pass_canary(fixture_roster::RUNTIME_LOOP_PATTERNS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-loop-patterns-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("loop-patterns canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "loop-patterns canary",
        "the iterative counting and nested loops should self-check",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_composite_initializer_local_arg_exit_canary_runs() {
    // A let-local whose initializer is a composite (binary / unary / cast) reading a
    // prior local or field, forwarded as a transition argument. The dispatch-arg fold
    // must recurse into the composite to resolve the inner local; missing Cast/Binary/
    // Unary arms re-materialized it in the target frame (no slot) and read 0.
    let canary = pass_canary(fixture_roster::RUNTIME_COMPOSITE_INITIALIZER_LOCAL_ARG_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-composite-initializer-arg-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("composite-initializer-local-arg canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "composite-initializer argument canary",
        "composite local initializers should retain their source frame when forwarded",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_captured_local_remutated_field_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CAPTURED_LOCAL_REMUTATED_FIELD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-captured-local-remutated-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("captured-local-remutated-field canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "captured-local remutated-field canary",
        "the captured local slot should survive later mutation of its source field",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 carrier compare through a POINTEE in a VALUE-CALL guard: the value-call
// `Finder::check(level) -> i32` branches on `r[0].label == "Gate"` where `r:
// &[Room]` indexes the by-value `level` param, so `r[0].label` is a carrier
// reached through the slice pointer. The guard resolves the pointee place and
// lowers the bounded-buffer compare; before the fix the resolver bailed, the
// leaf branch dropped the arm write (the literal-guard poison-skip), and the
// value-call returned a stale 0. Exits 70 (the `== "Gate"` true arm).

#[test]
fn runtime_bounded_carrier_pointee_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_CARRIER_POINTEE_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-pointee-guard-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier pointee guard canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bounded carrier pointee-guard canary",
        "the value-call guard should read the carrier through the slice-element pointee",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier field reached THROUGH a slice pointer:
// `cells[0].label = "Gate"` writes the carrier inline through the `&mut [Room]`
// pointer (a pointee write), then reads it back through the same pointer. Exits 70.

#[test]
fn runtime_bounded_carrier_slice_field_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_CARRIER_SLICE_FIELD_WRITE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-slice-field-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier slice field write canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bounded carrier slice-field write canary",
        "the carrier write should reach the field through the mutable slice pointer",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier through HOST OUTPUT, native: build a carrier
// by concat and `write_line` it. The host-call path reads the carrier with carrier
// addressing (len @ 0, content pointer = place + pointer_size). Prints "Room A1"
// and exits 70.

#[test]
fn runtime_bounded_carrier_write_line_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_CARRIER_WRITE_LINE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-write-line-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier write_line canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded carrier write_line canary lost its exact executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded carrier write_line canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the carrier write_line canary to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
        "Room A1",
        "expected the carrier `write_line` to print the materialized content `Room A1`",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 builder over NESTED-field carriers, CROSS-STATE: `self.line.text = "Room " +
// self.room.label` is built in `main` and `write_line`d in a later `shutdown`
// state. The nested fields carry their declared `in Utf8` domain across the state
// transition (entry-invariant seeded for nested fields, enforced at the nested
// write), so the carrier persists and prints. Prints "Room A1", exits 0.

#[test]
fn runtime_text_builder_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TEXT_BUILDER);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-text-builder-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested-field carrier builder canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-field carrier builder canary lost its exact executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-field carrier builder canary should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected the nested-field carrier builder canary to exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
        "Room A1",
        "expected the cross-state nested-field carrier builder to print `Room A1`",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 (return a `&[u8] in Utf8` view from a machine): a value-position call
// returning a `&[u8] in Utf8` literal view flows as a real 16-byte `{ptr,len}`
// descriptor into a `==` content compare. `pick() == "Gate"` matches and exits 70;
// the interpreter agrees. Exercises the value-call-result descriptor reaching the
// TextEquals leaf.

#[test]
fn utf8_return_view_equals_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::UTF8_RETURN_VIEW_EQUALS_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-utf8-return-view-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 return view canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "utf8 return-view equality canary",
        "the returned Utf8 view descriptor should compare equal to its literal content",
    );

    let _ = fs::remove_dir_all(&scratch);
}
